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

use crate::hub::{Core, Refused};
use crate::wire::{Ask, Say, read_line, write_line};

/// Everything the socket needs to answer: the brain's one core — the same
/// registry the runner runs and the watch thread feeds, so what a person asks
/// of a module here is asked of the module that runs.
pub type Desk = Core;

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
                if matches!(ask, Ask::Watch | Ask::Tail) {
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
/// to draw instead of one per message. The two exceptions are the streams
/// (`Watch`, `Tail`) and a module's page (`Screen`), which is never in the
/// mind.
fn answer(desk: &Arc<Desk>, ask: &Ask, out: &mut impl Write) -> Result<()> {
    let done = match ask {
        Ask::Look => Ok(()),
        Ask::Watch => return stream_it(desk, out),
        Ask::Tail => return tail_it(out),
        Ask::Screen { module } => {
            return match desk.screen(module) {
                Ok((generation, screen)) => write_line(
                    out,
                    &Say::Screen {
                        module: module.clone(),
                        generation,
                        screen,
                    },
                ),
                Err(why) => write_line(out, &Say::Refused { why }),
            };
        }
        Ask::Run { task, rehearsal } => run(desk, task, *rehearsal),
        Ask::Abort { task } => desk.module(task).map(|_| desk.shared.abort(task)),
        // PROCEED IS THE DEADLINE MOVED, and not a second way to start
        // something. Everything else about a run — the gate, the queue, the
        // journal — stays exactly where it was, so there is one path into a run
        // and not two.
        Ask::Proceed => {
            desk.shared.change(|m| {
                if let Some(a) = m.announcement.as_mut() {
                    a.deadline = m.now;
                }
            });
            Ok(())
        }
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
            Ok(())
        }
        // THE OLD STEP ASKS, through `Set`: one path that knows the module's
        // steps and its declared defaults.
        Ask::Toggle { task, step } => desk.toggle(task, step),
        Ask::Reset { task } => desk.set(task, &crate::wire::Knob::ResetSteps),
        Ask::Set { task, knob } => desk.set(task, knob),
        Ask::Gate { gate } => {
            desk.gate(*gate);
            Ok(())
        }
        Ask::Act {
            module,
            action,
            row,
            answer,
        } => match desk.act(module, action, row.clone(), answer.clone()) {
            Ok(()) => Ok(()),
            Err(Refused::Ask(why)) => Err(why),
            // THE MODULE DECLINED: its key, in a case of its own, for the
            // door to translate — and what it did first is published anyway.
            Err(Refused::Key(key)) => {
                desk.refresh();
                return write_line(out, &Say::Declined { key });
            }
        },
        Ask::Dismiss { module, notice } => desk.dismiss(module, notice),
        Ask::Snooze {
            module,
            notice,
            minutes,
        } => desk.snooze(module, notice, *minutes),
    };
    if let Err(why) = done {
        // A REFUSAL IS A SENTENCE, never a dropped line: a client that sent
        // something learns why nothing happened.
        return write_line(out, &Say::Refused { why });
    }
    // WHAT CHANGED IS DRAWN BEFORE THE ANSWER: the rows follow the new
    // preferences, and a module's face what it just did — with the pulse the
    // runner last took, since this thread has no meter.
    if matches!(
        ask,
        Ask::Toggle { .. } | Ask::Reset { .. } | Ask::Set { .. } | Ask::Gate { .. }
    ) {
        let pulse = desk.shared.snapshot().0.pulse;
        desk.publish(desk.now(), pulse);
    } else {
        desk.refresh();
    }
    let (mind, _) = desk.shared.snapshot();
    write_line(
        out,
        &Say::Mind {
            mind: Box::new(mind),
        },
    )
}

/// Puts a module's run in the queue, or says why not.
fn run(desk: &Arc<Desk>, task: &str, rehearsal: bool) -> Result<(), String> {
    let m = desk.module(task)?;
    // SWITCHED OFF BY THE SYSTEM: its units are gone with it, and the refusal
    // is the key of its row's own word.
    desk.system_off(m)?;
    // A WATCH HAS NO RUN: queued, it would wait for ever in front of every
    // run behind it.
    if m.scheduled().is_none() {
        return Err(format!("`{task}` only watches; it has no run of its own"));
    }
    if rehearsal {
        // The rehearsal flag rides with the ASK and not with the module, for
        // the reason `Report::is_rehearsal` gives: it is a property of this
        // run. The runner reads it from here when it starts.
        desk.shared.change(|m| m.rehearsing = Some(task.to_owned()));
    }
    desk.shared.queue_up(task);
    Ok(())
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

/// Streams the daemon's log: everything kept, then every new line.
///
/// The wait is the TAIL's bell and not the mind's, so a follower is woken by a
/// line and by nothing else — and a quiet brain writes nothing on the socket.
fn tail_it(out: &mut impl Write) -> Result<()> {
    let tail = crate::tail::global();
    let (lines, mut seen) = tail.after(0);
    write_line(out, &Say::Lines { lines })?;
    loop {
        let (lines, now) = tail.wait_after(seen, std::time::Duration::from_secs(60));
        if lines.is_empty() {
            continue;
        }
        write_line(out, &Say::Lines { lines })?;
        seen = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client;
    use crate::fake::{Fake, Hand, manifest};
    use crate::mind::{Mind, Shared};
    use crate::module::{
        Acted, Face, Level, Manifest, Module, Notice, NoticeKind, Screen, Section,
    };
    use crate::prefs::{Prefs, Where};
    use crate::registry::Registry;
    use crate::task::Step;
    use crate::wire::Knob;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-brain-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    static STEPS: &[Step] = &[Step {
        id: "extra",
        command: "x",
        needs_root: false,
        on_by_default: true,
        name_key: "",
        does_key: "",
    }];

    static DUE: NoticeKind = NoticeKind {
        id: "due",
        level: Level::Alarm,
        dismiss: true,
        snooze: &[10],
    };

    /// An action the ringer always declines.
    static NOPE: crate::module::Action = crate::module::Action {
        id: "nope",
        command: "ringer-nope",
        label: "cmd-ringer-nope",
        row: false,
        asks: None,
    };

    /// How many times the stepped module was told its preferences moved.
    static SAVED: AtomicUsize = AtomicUsize::new(0);

    /// A stepped routine that counts its `prefs_saved`, and a watch with an
    /// alarm and a page.
    fn modules() -> Vec<Box<dyn Module>> {
        struct Stepped(Fake);
        impl Module for Stepped {
            fn manifest(&self) -> &'static Manifest {
                self.0.manifest()
            }
            fn steps(&self) -> &'static [Step] {
                STEPS
            }
            fn scheduled(&self) -> Option<&dyn crate::module::Scheduled> {
                self.0.scheduled()
            }
            fn prefs_saved(&self, _: &crate::prefs::TaskPrefs, _: &Where) -> std::io::Result<()> {
                SAVED.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
        }
        vec![
            Box::new(Stepped(Fake::routine("stepped"))),
            Fake::new(Manifest {
                schedule: None,
                notices: std::slice::from_ref(&DUE),
                actions: std::slice::from_ref(&NOPE),
                ..manifest("ringer")
            })
            .face(|_| Face {
                notices: vec![Notice::of(&DUE, "ringer-says-due", 50)],
                screen: 4,
                ..Face::default()
            })
            .screen(|_| Screen {
                sections: vec![Section {
                    heading: "ringer-heading".into(),
                    rows: Vec::new(),
                }],
            })
            .acts(|act, _| {
                if act.action == NOPE.id {
                    Acted::Refused("rehearsing")
                } else {
                    Acted::Done
                }
            })
            .boxed(),
        ]
    }

    fn a_desk(root: &Path) -> Arc<Desk> {
        let desk = Arc::new(Core::new(
            Arc::new(Shared::of(Prefs::default())),
            Registry::of(modules()).expect("the registry"),
            Where {
                config: Some(root.to_path_buf()),
                state: Some(root.to_path_buf()),
            },
            Box::new(Hand::at(1000)),
        ));
        desk.publish(1000, crate::pulse::Pulse::default());
        desk
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
            other => panic!("not a mind: {other:?}"),
        }
    }

    fn refused(socket: &Path, ask: &Ask) -> String {
        match client::ask_at(socket, ask).expect("ask") {
            Say::Refused { why } => why,
            other => panic!("not refused: {other:?}"),
        }
    }

    /// A FOLLOWER GETS THE RING, THEN WHAT IS NEW — and a line written after
    /// it connected arrives without asking again.
    #[test]
    fn the_log_is_followed_line_by_line() {
        let root = scratch("tail");
        let (_desk, socket) = a_brain(&root);
        crate::tail::global().push(crate::tail::Level::Info, "before".to_owned());
        let stream = UnixStream::connect(&socket).expect("connect");
        let mut out = std::io::BufWriter::new(stream.try_clone().expect("twin"));
        write_line(&mut out, &Ask::Tail).expect("ask");
        let mut input = BufReader::new(stream);
        let Some(Say::Lines { lines }) = read_line::<Say>(&mut input).expect("read") else {
            panic!("the first answer was not lines");
        };
        assert!(
            lines.iter().any(|l| l.text == "before"),
            "the ring did not come first"
        );
        crate::tail::global().push(crate::tail::Level::Error, "after".to_owned());
        loop {
            let Some(Say::Lines { lines }) = read_line::<Say>(&mut input).expect("read") else {
                panic!("the stream ended");
            };
            if let Some(l) = lines.iter().find(|l| l.text == "after") {
                assert_eq!(l.level, crate::tail::Level::Error);
                break;
            }
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_second_brain_leaves_instead_of_taking_the_socket() {
        // THE DEFECT THIS CATCHES: the second daemon evicting the first, which
        // is holding a rebuild — and leaving that rebuild with nobody watching
        // it and nobody able to stop it.
        let root = scratch("second");
        let (_desk, socket) = a_brain(&root);
        let second = a_desk(&root);
        listen_on(&socket, &second).expect("the second one should leave in peace");
        let mind = mind_of(&socket, &Ask::Look);
        assert_eq!(mind.prefs, Prefs::default());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_id_nobody_answers_to_is_refused_with_the_list() {
        let root = scratch("unknown");
        let (_desk, socket) = a_brain(&root);
        let why = refused(
            &socket,
            &Ask::Run {
                task: "does-not-exist".to_owned(),
                rehearsal: false,
            },
        );
        assert!(why.contains("stepped") && why.contains("ringer"), "{why}");
        // A WATCH HAS NO RUN, and is refused rather than queued for ever.
        let why = refused(
            &socket,
            &Ask::Run {
                task: "ringer".to_owned(),
                rehearsal: false,
            },
        );
        assert!(why.contains("only watches"), "{why}");
        assert!(mind_of(&socket, &Ask::Look).queue.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_knob_is_set_written_down_and_handed_to_the_module() {
        // `Set` IS ANSWERED FOR REAL: the preference moves, the file is written
        // at once, and the module keeping a view of it is told.
        let root = scratch("set");
        let (_desk, socket) = a_brain(&root);
        let before = SAVED.load(Ordering::Relaxed);
        let mind = mind_of(
            &socket,
            &Ask::Set {
                task: "stepped".to_owned(),
                knob: Knob::Step {
                    id: "extra".to_owned(),
                    on: false,
                },
            },
        );
        // THE ANSWER ALREADY DRAWS IT: the row, not only the preferences.
        let row = mind.task("stepped").expect("row");
        assert!(!row.steps[0].on, "the answer still drew the step on");
        assert!(
            crate::prefs::read_from(&root).tasks["stepped"]
                .steps_flipped
                .contains("extra")
        );
        assert!(
            SAVED.load(Ordering::Relaxed) > before,
            "the module was not told"
        );
        // AND THE OLD TOGGLE is the same path, both ways.
        let mind = mind_of(
            &socket,
            &Ask::Toggle {
                task: "stepped".to_owned(),
                step: "extra".to_owned(),
            },
        );
        assert!(mind.task("stepped").expect("row").steps[0].on);
        let why = refused(
            &socket,
            &Ask::Toggle {
                task: "stepped".to_owned(),
                step: "nope".to_owned(),
            },
        );
        assert!(why.contains("no step"), "{why}");
        let why = refused(
            &socket,
            &Ask::Set {
                task: "ringer".to_owned(),
                knob: Knob::IntervalDays(2),
            },
        );
        assert!(why.contains("no calendar"), "{why}");
        // THE GATE, whole.
        let gate = crate::prefs::Gate {
            ram_ceiling_pct: 55,
            ..crate::prefs::Gate::default()
        };
        let mind = mind_of(&socket, &Ask::Gate { gate });
        assert_eq!(mind.prefs.gate.ram_ceiling_pct, 55);
        assert_eq!(crate::prefs::read_from(&root).gate.ram_ceiling_pct, 55);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_notice_is_put_away_and_a_page_is_asked_for_over_the_socket() {
        let root = scratch("modules");
        let (_desk, socket) = a_brain(&root);
        assert_eq!(mind_of(&socket, &Ask::Look).notices.len(), 1);
        let why = refused(
            &socket,
            &Ask::Snooze {
                module: "ringer".to_owned(),
                notice: "due".to_owned(),
                minutes: 11,
            },
        );
        assert!(why.contains("11 minutes"), "{why}");
        let mind = mind_of(
            &socket,
            &Ask::Snooze {
                module: "ringer".to_owned(),
                notice: "due".to_owned(),
                minutes: 10,
            },
        );
        assert!(mind.notices.is_empty(), "a snoozed notice is still up");
        // THE PAGE: its generation and its rows, never in the mind.
        match client::ask_at(
            &socket,
            &Ask::Screen {
                module: "ringer".to_owned(),
            },
        )
        .expect("ask")
        {
            Say::Screen {
                module,
                generation,
                screen,
            } => {
                assert_eq!(module, "ringer");
                assert_eq!(generation, 4);
                assert_eq!(screen.sections[0].heading, "ringer-heading");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            client::ask_at(
                &socket,
                &Ask::Screen {
                    module: "nobody".to_owned()
                }
            )
            .expect("ask"),
            Say::Refused { .. }
        ));
        // AN ACTION NOBODY DECLARED is refused; the module is never asked.
        let why = refused(
            &socket,
            &Ask::Act {
                module: "ringer".to_owned(),
                action: "fly".to_owned(),
                row: None,
                answer: None,
            },
        );
        assert!(why.contains("no action"), "{why}");
        // AND ONE THE MODULE DECLINES is its KEY, in a case of its own — a
        // door can tell what to translate from what to show as it came.
        assert_eq!(
            client::ask_at(
                &socket,
                &Ask::Act {
                    module: "ringer".to_owned(),
                    action: "nope".to_owned(),
                    row: None,
                    answer: None,
                },
            )
            .expect("ask"),
            Say::Declined {
                key: "brain-why-rehearsing".to_owned()
            }
        );
        let why = refused(
            &socket,
            &Ask::Dismiss {
                module: "ringer".to_owned(),
                notice: "gone".to_owned(),
            },
        );
        assert!(why.contains("declares no"), "{why}");
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
        desk.shared.queue_up("stepped");

        let stream = UnixStream::connect(&socket).expect("connect");
        let mut out = BufWriter::new(stream.try_clone().expect("duplicate"));
        write_line(&mut out, &Ask::Watch).expect("ask");
        let mut input = BufReader::new(stream);
        let first: Say = read_line(&mut input)
            .expect("read")
            .expect("nothing in the first frame");
        match first {
            Say::Mind { mind } => assert_eq!(mind.queue, vec!["stepped".to_owned()]),
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
