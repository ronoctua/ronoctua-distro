//! Answering the socket: what each ask means, and what goes back.
//!
//! # Why this is in the library and not in the binary
//!
//! Because a protocol only a running daemon exercises is a protocol nobody
//! tests. Here a test binds a socket in a temporary directory, drives it with
//! the real client and reads the real answers — the arrangement
//! `ronoctua-transfers::serve` already uses, and for the reason it writes down:
//! `XDG_RUNTIME_DIR` is process-global, and a test that changed it would race
//! every other test in the binary.

use std::io::{BufReader, BufWriter, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;

use anyhow::{Context as _, Result};

use crate::mind::Shared;
use crate::prefs::Where;
use crate::registry::Registry;
use crate::task::Step;
use crate::wire::{Ask, Say, read_line, write_line};

/// Everything the socket needs to answer.
///
/// A struct and not four arguments, because every answer needs the same four
/// things and a function that took them one by one would grow a fifth the first
/// time anything did.
pub struct Desk {
    /// The state.
    pub shared: Arc<Shared>,
    /// What tasks exist — so an id nobody answers to is refused HERE, with a
    /// sentence, rather than sitting in a queue nothing will ever take.
    pub tasks: Arc<Registry>,
    /// Where the preferences are written.
    pub store: Where,
}

/// Takes the socket and answers on it until the listener dies.
///
/// **A second brain LEAVES rather than taking the socket by force.** The footer
/// does take it — it owns the screen and only one may draw — but here the one
/// already running is the one holding the schedule and, quite possibly, a
/// rebuild in flight. Evicting it would leave that rebuild with nobody watching
/// and nobody able to stop it.
///
/// # Errors
///
/// When the socket cannot be bound.
pub fn listen_on(path: &std::path::Path, desk: &Arc<Desk>) -> Result<()> {
    if UnixStream::connect(path).is_ok() {
        tracing::info!(path = %path.display(), "there is already a brain listening; leaving");
        return Ok(());
    }
    // An orphan socket of a dead process prevents the bind, and after the test
    // above what is left here belongs to a process that no longer answers.
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)
        .with_context(|| format!("open the socket at {}", path.display()))?;
    tracing::info!(path = %path.display(), "brain opened");

    for connection in listener.incoming() {
        let Ok(stream) = connection else { continue };
        let mine = Arc::clone(desk);
        // A NAMED thread: an unnamed one says `ronoctua-brain` in `top` and
        // nothing about what it is doing, which is the question one opens `top`
        // to answer.
        if let Err(e) = std::thread::Builder::new()
            .name("client".into())
            .spawn(move || serve(&mine, stream))
        {
            tracing::error!(error = %e, "could not serve the client");
        }
    }
    Ok(())
}

/// One client, for as long as it stays.
fn serve(desk: &Arc<Desk>, stream: UnixStream) {
    let Ok(twin) = stream.try_clone() else {
        return;
    };
    let mut input = BufReader::new(twin);
    let mut out = BufWriter::new(stream);
    loop {
        match read_line::<Ask>(&mut input) {
            Ok(Some(ask)) => {
                if answer(desk, &ask, &mut out).is_err() {
                    return;
                }
                // A watch never comes back: the connection IS the subscription.
                if matches!(ask, Ask::Watch) {
                    return;
                }
            }
            Ok(None) => return,
            // A line we could not read is the client's problem, not the
            // brain's. Saying so and dropping the connection beats taking the
            // daemon down over somebody's typo in `socat`.
            Err(e) => {
                tracing::warn!(error = %e, "a request I did not understand");
                let _ = write_line(
                    &mut out,
                    &Say::Refused {
                        why: format!("{e:#}"),
                    },
                );
                return;
            }
        }
    }
}

/// Carries out one ask.
///
/// Every one of them answers with the WHOLE mind, so that whoever asked sees
/// what their ask did without a second round trip — and so there is one shape
/// to draw instead of one per message.
fn answer(desk: &Arc<Desk>, ask: &Ask, out: &mut impl Write) -> Result<()> {
    match ask {
        Ask::Look => {}
        Ask::Watch => return stream_it(desk, out),
        Ask::Run { task, rehearsal } => {
            if let Some(why) = unknown(desk, task) {
                return write_line(out, &Say::Refused { why });
            }
            if *rehearsal {
                // The rehearsal flag rides with the ASK and not with the task,
                // for the reason `Report::is_rehearsal` gives: it is a property
                // of this run. The runner reads it from here when it starts.
                desk.shared.change(|m| m.rehearsing = Some(task.clone()));
            }
            desk.shared.queue_up(task);
        }
        Ask::Abort { task } => {
            if let Some(why) = unknown(desk, task) {
                return write_line(out, &Say::Refused { why });
            }
            desk.shared.abort(task);
        }
        // PROCEED IS THE DEADLINE MOVED, and not a second way to start
        // something. Everything else about a run — the gate, the queue, the
        // journal — stays exactly where it was, so there is one path into a run
        // and not two.
        Ask::Proceed => desk.shared.change(|m| {
            if let Some(a) = m.announcement.as_mut() {
                a.deadline = m.now;
            }
        }),
        // NOT NOW — and it has to mean something, or the tick half a minute
        // later asks again. It leaves the queue here, and the RUNNER writes it
        // down as an event: what moves the next moment is the journal, and the
        // journal belongs to that thread.
        Ask::Postpone => {
            let (mind, _) = desk.shared.snapshot();
            let put_off = mind
                .announcement
                .map(|a| a.task)
                .or_else(|| mind.queue.first().cloned());
            if let Some(task) = put_off {
                desk.shared.unqueue(&task);
                desk.shared.change(|m| m.postponed = Some(task.clone()));
            }
        }
        Ask::Toggle { task, step } => {
            if let Some(why) = unknown(desk, task) {
                return write_line(out, &Say::Refused { why });
            }
            // A STEP THE TASK DOES NOT HAVE is refused too, and for the same
            // reason: the person's file would grow an id that nothing matches,
            // and it would sit there for ever switching nothing off.
            if !has_step(desk, task, step) {
                return write_line(
                    out,
                    &Say::Refused {
                        why: format!("`{task}` has no step called `{step}`"),
                    },
                );
            }
            desk.shared.change(|m| {
                let mine = m.prefs.mine(task);
                if !mine.steps_off.remove(step) {
                    mine.steps_off.insert(step.clone());
                }
            });
            keep(desk);
        }
        Ask::Reset { task } => {
            if let Some(why) = unknown(desk, task) {
                return write_line(out, &Say::Refused { why });
            }
            desk.shared.change(|m| m.prefs.mine(task).steps_off.clear());
            keep(desk);
        }
        Ask::Tune { prefs } => {
            desk.shared.change(|m| m.prefs = (**prefs).clone());
            keep(desk);
        }
    }
    let (mind, _) = desk.shared.snapshot();
    write_line(
        out,
        &Say::Mind {
            mind: Box::new(mind),
        },
    )
}

/// The sentence for an id nothing answers to, or `None` when it is known.
fn unknown(desk: &Arc<Desk>, task: &str) -> Option<String> {
    if desk.tasks.find(task).is_some() {
        return None;
    }
    // Naming what DOES exist, because the caller is usually somebody at a
    // keyboard typing into `socat` — and a refusal that only says no is a
    // refusal they have to go and read the source about.
    let known: Vec<&str> = desk.tasks.all().map(crate::task::Task::id).collect();
    Some(format!(
        "I do not know the task `{task}`; I know: {}",
        known.join(", ")
    ))
}

/// Whether a task really has that step.
fn has_step(desk: &Arc<Desk>, task: &str, step: &str) -> bool {
    desk.tasks
        .find(task)
        .is_some_and(|t| t.steps().iter().any(|s: &Step| s.id == step))
}

/// Writes the preferences down, and says so when it cannot.
///
/// Not fatal: a read-only home is not a reason for the brain to stop deciding.
/// It is loud, because what is lost is a choice the person made, and the
/// symptom would be it coming back on the next start.
fn keep(desk: &Arc<Desk>) {
    let (mind, _) = desk.shared.snapshot();
    if let Err(e) = desk.store.save(&mind.prefs) {
        tracing::error!(error = %e, "could not write the brain's preferences");
    }
}

/// The whole mind now, and again on every change, until the client goes.
fn stream_it(desk: &Arc<Desk>, out: &mut impl Write) -> Result<()> {
    // THE FIRST ONE IS NOT A CHANGE. A client that has drawn nothing has to be
    // correct from its first frame; without this it would show an empty screen
    // until something happened to change.
    let (mind, mut seen) = desk.shared.snapshot();
    write_line(
        out,
        &Say::Mind {
            mind: Box::new(mind),
        },
    )?;
    loop {
        let (mind, now) = desk
            .shared
            .wait_past(seen, std::time::Duration::from_secs(60));
        if now == seen {
            // The wait timed out with nothing changed. Nothing is sent — a
            // watcher that received an identical mind every minute would cost
            // a frame a minute for the length of the session.
            continue;
        }
        write_line(
            out,
            &Say::Mind {
                mind: Box::new(mind),
            },
        )?;
        seen = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client;
    use crate::mind::Mind;
    use crate::prefs::Prefs;
    use crate::tasks;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-brain-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn a_desk(root: &Path) -> Arc<Desk> {
        let zone: Arc<dyn crate::moment::Offset> = Arc::new(crate::moment::Fixed(0));
        Arc::new(Desk {
            shared: Arc::new(Shared::of(Prefs::default())),
            tasks: Arc::new(Registry::of(tasks::tasks(zone)).expect("registro")),
            store: Where {
                config: Some(root.to_path_buf()),
                state: Some(root.to_path_buf()),
            },
        })
    }

    /// A brain on a socket of its own, and the path to reach it.
    fn a_brain(root: &Path) -> (Arc<Desk>, PathBuf) {
        let desk = a_desk(root);
        let socket = root.join("brain.sock");
        let mine = Arc::clone(&desk);
        let its = socket.clone();
        std::thread::spawn(move || {
            let _ = listen_on(&its, &mine);
        });
        // Wait for the bind rather than sleeping a guessed amount: a fixed
        // sleep is either slow or flaky, and on a loaded machine it is both.
        let until = Instant::now() + Duration::from_secs(5);
        while UnixStream::connect(&socket).is_err() {
            assert!(Instant::now() < until, "the brain did not come up");
            std::thread::sleep(Duration::from_millis(5));
        }
        (desk, socket)
    }

    fn mind_of(socket: &Path, ask: &Ask) -> Mind {
        match client::ask_at(socket, ask).expect("ask") {
            Say::Mind { mind } => *mind,
            Say::Refused { why } => panic!("refused: {why}"),
        }
    }

    #[test]
    fn a_second_brain_leaves_instead_of_taking_the_socket() {
        // THE DEFECT THIS CATCHES: the second daemon evicting the first, which
        // is holding a rebuild — and leaving that rebuild with nobody watching
        // it and nobody able to stop it.
        let root = scratch("second");
        let (_desk, socket) = a_brain(&root);
        let second = a_desk(&root);
        // It comes back at once, and it comes back Ok: leaving is not an error.
        listen_on(&socket, &second).expect("the second one should leave in peace");
        // And the first one is still answering — which is the whole point:
        // the second daemon left, and nothing that was in flight lost its
        // watcher. (The task ROWS are the runner's to build, and this test
        // raises no runner: what is being proved here is the socket.)
        let mind = mind_of(&socket, &Ask::Look);
        assert_eq!(mind.prefs, Prefs::default());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_id_nobody_answers_to_is_refused_with_the_list() {
        let root = scratch("unknown");
        let (_desk, socket) = a_brain(&root);
        let said = client::ask_at(
            &socket,
            &Ask::Run {
                task: "does-not-exist".to_owned(),
                rehearsal: false,
            },
        )
        .expect("ask");
        match said {
            Say::Refused { why } => {
                assert!(why.contains("system-update"), "{why}");
            }
            other => panic!("{other:?}"),
        }
        // And nothing was queued: a refusal that still queued would be a row
        // waiting for a task that does not exist.
        assert!(mind_of(&socket, &Ask::Look).queue.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_step_toggles_both_ways_and_is_written_down() {
        let root = scratch("toggle");
        let (_desk, socket) = a_brain(&root);
        // THE ID COMES FROM THE TABLE, and is not written here.
        //
        // It was `"system-rebuild"`, spelled out — which is a second place the
        // sequence is named, and the second place is exactly what this project
        // just spent a day removing: the brain used to declare three steps of
        // its own and they came out disjoint from the twelve that actually run.
        // `update-has-one-list` refuses a step's name anywhere outside the
        // declaration, and it refused this line, in a test, correctly.
        let step = crate::tasks::update::steps()
            .first()
            .expect("the table declares at least one step")
            .id;
        let off = Ask::Toggle {
            task: crate::tasks::update::ID.to_owned(),
            step: step.to_owned(),
        };
        let mind = mind_of(&socket, &off);
        assert!(
            mind.prefs
                .of(crate::tasks::update::ID)
                .steps_off
                .contains(step),
            "it did not switch off"
        );
        // The file is written at once: a choice that only lives in memory is a
        // choice that is gone the next time the daemon starts.
        assert_eq!(
            crate::prefs::read_from(&root)
                .of("system-update")
                .steps_off
                .len(),
            1
        );
        let mind = mind_of(&socket, &off);
        assert!(
            mind.prefs.of("system-update").steps_off.is_empty(),
            "the same key did not switch it back on"
        );

        // A step the task does not have is refused rather than written down.
        let said = client::ask_at(
            &socket,
            &Ask::Toggle {
                task: "system-update".to_owned(),
                step: "does-not-exist".to_owned(),
            },
        )
        .expect("ask");
        assert!(matches!(said, Say::Refused { .. }));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tuning_replaces_the_preferences_and_keeps_them() {
        let root = scratch("tune");
        let (_desk, socket) = a_brain(&root);
        let mut wanted = Prefs::default();
        wanted.mine("system-update").interval_days = 1;
        wanted.gate.ram_ceiling_pct = 55;
        let mind = mind_of(
            &socket,
            &Ask::Tune {
                prefs: Box::new(wanted.clone()),
            },
        );
        assert_eq!(mind.prefs, wanted);
        assert_eq!(crate::prefs::read_from(&root), wanted);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_watcher_is_correct_from_its_first_frame() {
        // A client that has drawn nothing has to be right immediately. Without
        // the first send, the screen would stay empty until something happened
        // to change — and "the screen is broken" and "nothing is scheduled"
        // look identical.
        let root = scratch("watch");
        let (desk, socket) = a_brain(&root);
        desk.shared.queue_up("system-update");

        let stream = UnixStream::connect(&socket).expect("connect");
        let mut out = BufWriter::new(stream.try_clone().expect("duplicate"));
        write_line(&mut out, &Ask::Watch).expect("pedir");
        let mut input = BufReader::new(stream);
        let first: Say = read_line(&mut input)
            .expect("read")
            .expect("nothing in the first frame");
        match first {
            Say::Mind { mind } => assert_eq!(mind.queue, vec!["system-update".to_owned()]),
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
