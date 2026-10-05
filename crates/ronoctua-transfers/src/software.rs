//! The software engine: the software center's CLI, as a child, in its own
//! process group.
//!
//! # Why a child and not a module of this daemon
//!
//! Because of what the work needs and what this daemon promises not to carry.
//! Installing a person's program reaches Flathub, GitHub and a web app's own
//! page over TLS and decodes icons; this unit runs for as long as the machine
//! is on and keeps no TLS stack and no decoder (`transfers-unit-is-narrow`).
//! So the queue does exactly what it does for yt-dlp ([`crate::media`]): it
//! starts a program, reads its lines, and kills its group when told to. The
//! crate it links is `ronoctua-software` with NO features — the order, the
//! progress line and the outcome, and nothing that opens a socket.
//!
//! # Why inside a scope of its own
//!
//! `nix` evaluating nixpkgs can take gigabytes, and this unit runs with
//! `OOMPolicy=stop`: an out-of-memory kill anywhere in it would take every copy
//! in flight down with it. `systemd-run --user --scope --slice=app.slice -p
//! OOMPolicy=kill` moves the child into a scope of its own before it `exec`s, so
//! the kernel's choice falls on the order and never on the queue. The scope is
//! SYNCHRONOUS, which is harmless here: it blocks this job's thread and nothing
//! else (the footer froze on that once, and the footer is not this).
//!
//! # What the lines are
//!
//! [`ronoctua_software::progress`]: `RONOCTUA<TAB>step…` while it works,
//! `…done` or `…failed` at the end. Everything else is the programs' own
//! chatter, of which the last [`TAIL`] lines are kept — they are the trouble
//! when the child dies without saying why.
//!
//! # What it does not honour
//!
//! Pause ([`crate::job::Engine::pauses`]). Cancel is SIGTERM to the group, and
//! SIGKILL to whatever of it is still there [`GRACE`] later.

use std::collections::VecDeque;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use ronoctua_files::watch::{Control, Flow};
use ronoctua_software::progress::{Beat, Phase};
use ronoctua_software::{Failure, Outcome, Refusal};

use crate::engine::{POLL, Progress};
use crate::job::{Measure, Stage, Work};
use crate::ledger::Shared;

/// How many of the child's last lines are kept for the trouble — the same
/// number the CLI keeps of ITS children, so the two tails read alike.
pub const TAIL: usize = ronoctua_software::run::TAIL;

/// How long a cancelled child has to leave on its own before the group is
/// killed. Long enough for `nix` to let go of its temporary roots and
/// `flatpak` of its transaction; short enough that "give up" still feels like
/// an answer.
pub const GRACE: Duration = Duration::from_secs(3);

/// How long the pipes may stay open once the child itself is gone.
///
/// The `done` line is the LAST one, and it can still be in the pipe when the
/// exit is noticed — so the reading goes on until both streams end. But a
/// grandchild that inherited the pipe can hold it open for ever, and a job
/// that never finishes because of somebody else's descriptor is worse than
/// one whose last chatter line is lost.
const DRAIN: Duration = Duration::from_secs(2);

/// How often a cancelled group is looked at while it leaves.
const LOOK: Duration = Duration::from_millis(50);

/// The command line, program first: `systemd-run` putting the CLI in a scope of
/// its own, the CLI told to read one order and print its progress.
///
/// A pure function, and tested as DATA: a test may not start `systemd-run`
/// (`tests-do-not-touch-the-machine`), so what it checks is the shape.
#[must_use]
pub fn command_line(systemd_run: &str, software: &str) -> Vec<String> {
    [
        systemd_run,
        "--user",
        "--scope",
        "--quiet",
        // A scope that FAILED — an order killed for memory — would otherwise
        // stay loaded and listed as failed until the session ends, one per
        // killed order.
        "--collect",
        // WHERE THE PERSON'S APPS LIVE, and not `session.slice`: the memory
        // protection there is for the desktop, and an install is not the
        // desktop.
        "--slice=app.slice",
        "-p",
        // The child is the one that dies, and the queue — `OOMPolicy=stop` —
        // never sees it.
        "OOMPolicy=kill",
        // END OF OPTIONS: everything after is the command.
        "--",
        software,
        "run",
        "--progress",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

/// Where the software center's CLI is, read from the ENVIRONMENT.
///
/// The unit sets it, inside the `mkIf` that decides whether the software
/// center exists at all — the reason [`ronoctua_ytdlp::program`] gives for
/// `RONOCTUA_YT_DLP`: baked in with `option_env!`, a store path is a REFERENCE,
/// and it would drag the CLI's TLS stack into this unit's closure. There is no
/// fallback to a bare name: without it, the job fails with a sentence.
fn software() -> Option<String> {
    std::env::var("RONOCTUA_SOFTWARE")
        .ok()
        .filter(|v| !v.is_empty())
}

/// Where `systemd-run` is. A path from the unit when it gives one; the bare
/// name otherwise, which works in a development shell and is why the unit
/// sets it.
fn systemd_run() -> String {
    std::env::var("RONOCTUA_SYSTEMD_RUN")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "systemd-run".to_owned())
}

/// What the counter counts while a phase is going on.
///
/// Bytes while something DOWNLOADS, things otherwise — that is the contract of
/// a step's two numbers ([`ronoctua_software::progress::Beat::Step`]), and the
/// screen formats them by [`crate::job::Job::measure`]: a download of fifty
/// mebibytes read as "52428800 of 52428800 things" would be a number nobody
/// can use.
#[must_use]
pub const fn measure_of(phase: Phase) -> Measure {
    match phase {
        Phase::Downloading => Measure::Bytes,
        Phase::Resolving
        | Phase::Waiting
        | Phase::Verifying
        | Phase::Unpacking
        | Phase::Building
        | Phase::Installing
        | Phase::Integrating
        | Phase::Removing
        | Phase::Starting
        | Phase::Indexing
        | Phase::Preparing => Measure::Items,
    }
}

/// How a job ended.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ending {
    /// It worked, and this is what it left.
    Done(Option<Outcome>),
    /// It did not.
    Failed {
        /// What to show, in English (invariant 18).
        trouble: String,
        /// The reason, when it is one the screen has a sentence for.
        refusal: Option<Refusal>,
    },
    /// Somebody gave it up.
    Cancelled,
}

/// One step, as the row takes it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    phase: Phase,
    done: u64,
    total: u64,
    now: String,
}

/// What the child has said so far.
#[derive(Debug, Default)]
struct Heard {
    outcome: Option<Outcome>,
    failure: Option<Failure>,
    tail: VecDeque<String>,
}

impl Heard {
    /// One line from the child. A step comes back to be shown; the two ends
    /// are kept; everything else goes to the tail.
    ///
    /// Beats are read from STDOUT only, which is where the CLI prints them:
    /// what arrives on stderr is chatter whatever it looks like.
    fn line(&mut self, text: &str, from_stdout: bool) -> Option<Step> {
        if from_stdout && let Some(beat) = Beat::of_line(text) {
            match beat {
                Beat::Step {
                    phase,
                    done,
                    total,
                    now,
                } => {
                    return Some(Step {
                        phase,
                        done,
                        total,
                        now,
                    });
                }
                Beat::Done(outcome) => self.outcome = Some(outcome),
                Beat::Failed(failure) => self.failure = Some(failure),
            }
            return None;
        }
        let text = text.trim_end();
        if !text.trim().is_empty() {
            self.tail.push_back(text.to_owned());
            if self.tail.len() > TAIL {
                self.tail.pop_front();
            }
        }
        None
    }

    /// How it ended, given how the child exited.
    ///
    /// A `failed` line wins over everything: it is the child saying, in the
    /// closed set, why. Without one, the EXIT decides — a child that printed
    /// nothing and exited zero did what it was asked, and one that exited
    /// non-zero without a word leaves its last lines as the reason, because
    /// "failed" alone sends somebody to the journal.
    fn ending(self, success: bool, status: &str) -> Ending {
        if let Some(failure) = self.failure {
            let mut trouble = failure.to_string();
            if trouble.is_empty() {
                trouble = format!("the software program exited with {status}");
            }
            return Ending::Failed {
                trouble,
                refusal: failure.refusal,
            };
        }
        if success {
            return Ending::Done(self.outcome);
        }
        let trouble = if self.tail.is_empty() {
            format!("the software program exited with {status}")
        } else {
            Vec::from(self.tail).join("\n")
        };
        Ending::Failed {
            trouble,
            refusal: None,
        }
    }
}

/// Runs one order to the end, or to whatever the switch says.
///
/// The job's own thread: it blocks on the child, and nothing else waits on it.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    let Work::Software { order, .. } = &work else {
        tracing::error!("the software engine got a job that is not software");
        shared.touch(id, |j| j.stage = Stage::Failed);
        return;
    };
    // AN INSTALLATION WITHOUT THE PROGRAM gets a sentence at the door, and not
    // a row that sits at nothing per cent for ever.
    let Some(software) = software() else {
        finish(
            shared,
            id,
            Ending::Failed {
                trouble: "this installation does not have the program that installs software \
                          (RONOCTUA_SOFTWARE is not set)"
                    .to_owned(),
                refusal: None,
            },
        );
        return;
    };
    let order = match serde_json::to_vec(order) {
        Ok(bytes) => bytes,
        Err(e) => {
            finish(
                shared,
                id,
                Ending::Failed {
                    trouble: format!("could not write the order: {e}"),
                    refusal: None,
                },
            );
            return;
        }
    };
    let argv = command_line(&systemd_run(), &software);
    let ending = carry(shared, id, switch, &argv, order, GRACE);
    finish(shared, id, ending);
}

/// Writes how it ended onto the row.
fn finish(shared: &Arc<Shared>, id: u64, ending: Ending) {
    shared.touch(id, |j| match ending {
        Ending::Done(outcome) => {
            j.stage = Stage::Done;
            j.outcome = outcome;
            // A PHASE IS WHERE INSIDE `Running` IT IS, and a finished job is
            // not inside anything: "installing" beside "done" would read as
            // one of the two being wrong.
            j.phase = None;
            // A finished order is a hundred per cent even when the last step
            // said otherwise: the numbers were the last phase's.
            if j.total == 0 || j.done > j.total {
                j.total = j.done;
            } else {
                j.done = j.total;
            }
        }
        Ending::Failed { trouble, refusal } => {
            // The phase STAYS: "failed while downloading" is half of the
            // diagnosis.
            j.stage = Stage::Failed;
            j.trouble = Some(trouble);
            j.refusal = refusal;
        }
        Ending::Cancelled => j.stage = Stage::Cancelled,
    });
}

/// One line, and which stream it came from.
struct Line {
    text: String,
    from_stdout: bool,
}

/// Starts `argv` in a group of its own, hands it `order` on stdin, and follows
/// it to the end.
///
/// `argv` and `grace` are parameters so a test can run a `/bin/sh` script in
/// place of `systemd-run` and not wait three seconds to see a group die.
fn carry(
    shared: &Arc<Shared>,
    id: u64,
    switch: &Arc<Control>,
    argv: &[String],
    order: Vec<u8>,
    grace: Duration,
) -> Ending {
    // Given up before it began: nothing to start, nothing to take back.
    if switch.flow() == Flow::Stop {
        return Ending::Cancelled;
    }
    let Some((program, args)) = argv.split_first() else {
        return Ending::Failed {
            trouble: "no program to run".to_owned(),
            refusal: None,
        };
    };
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // ITS OWN GROUP: one signal reaches the CLI and every `nix` or `flatpak` it
    // started — and a signal meant for them never reaches this daemon.
    // `systemd-run --scope` `exec`s in place, so the group survives it.
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            return Ending::Failed {
                trouble: format!("could not run {program}: {e}"),
                refusal: None,
            };
        }
    };
    let pid = child.id();
    shared.touch(id, |j| j.stage = Stage::Running);

    // THE ORDER ON STDIN, from a thread: a child that answers before it has
    // read everything must not deadlock against a full pipe. Dropping the pipe
    // is the end of the order.
    if let Some(mut input) = child.stdin.take() {
        let spawned = std::thread::Builder::new()
            .name("software-in".into())
            .spawn(move || {
                use std::io::Write as _;
                let _ = input.write_all(&order);
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "could not start the thread that hands the order over");
        }
    }

    // TWO READERS AND A CHANNEL, because reading a line BLOCKS and the switch
    // has to stay answerable — `media.rs`'s shape. The splitter is the CLI's
    // own, so a line that is not UTF-8 is replaced instead of ending the
    // stream (and a stream nobody reads is a child blocked on a full pipe).
    let (lines, arriving) = mpsc::channel::<Line>();
    reader(child.stdout.take(), lines.clone(), true);
    reader(child.stderr.take(), lines, false);

    let mut progress = Progress::new(shared, id);
    let mut phase: Option<Phase> = None;
    let mut heard = Heard::default();
    let mut exited: Option<(std::process::ExitStatus, Instant)> = None;

    loop {
        if exited.is_none() && switch.flow() == Flow::Stop {
            progress.flush();
            stop(&mut child, pid, grace);
            return Ending::Cancelled;
        }
        match arriving.recv_timeout(POLL) {
            Ok(line) => {
                if let Some(step) = heard.line(&line.text, line.from_stdout) {
                    if phase != Some(step.phase) {
                        // THE NUMBERS CHANGE MEANING WITH THE PHASE, so what
                        // was held back is written under the old one first,
                        // and the new ones go out at once under the new one.
                        progress.flush();
                        let now = step.phase;
                        shared.touch(id, |j| {
                            j.phase = Some(now);
                            j.measure = measure_of(now);
                        });
                        phase = Some(now);
                        progress.set(step.done, step.total, &step.now);
                        progress.flush();
                    } else {
                        progress.set(step.done, step.total, &step.now);
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            // Both streams ended.
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if exited.is_none() {
            match child.try_wait() {
                Ok(Some(status)) => exited = Some((status, Instant::now())),
                Ok(None) => {}
                Err(e) => {
                    progress.flush();
                    stop(&mut child, pid, grace);
                    return Ending::Failed {
                        trouble: format!("could not wait for the software program: {e}"),
                        refusal: None,
                    };
                }
            }
        }
        if let Some((_, since)) = exited
            && since.elapsed() >= DRAIN
        {
            break;
        }
    }
    progress.flush();
    let status = match exited {
        Some((status, _)) => status,
        None => match child.wait() {
            Ok(status) => status,
            Err(e) => {
                return Ending::Failed {
                    trouble: format!("could not wait for the software program: {e}"),
                    refusal: None,
                };
            }
        },
    };
    heard.ending(status.success(), &status.to_string())
}

/// Asks the group to leave, and makes it leave `grace` later.
///
/// SIGTERM first, because what is running is `nix` and `flatpak` in the middle
/// of a transaction, and both clean up on it. Then SIGKILL to whatever of the
/// GROUP is left — not only the CLI: a grandchild that ignored the polite ask
/// would otherwise go on holding the lane.
pub(crate) fn stop(child: &mut Child, pid: u32, grace: Duration) {
    signal(pid, rustix::process::Signal::TERM);
    let until = Instant::now() + grace;
    let mut reaped = false;
    while Instant::now() < until {
        // The leader is reaped first: a zombie still counts as a member, and
        // the group would look alive for as long as nobody waited on it.
        if !reaped && matches!(child.try_wait(), Ok(Some(_))) {
            reaped = true;
        }
        if reaped && !group_alive(pid) {
            return;
        }
        std::thread::sleep(LOOK);
    }
    signal(pid, rustix::process::Signal::KILL);
    let _ = child.wait();
}

/// The pid as rustix takes it, or nothing for one that cannot be a group.
fn group_of(pid: u32) -> Option<rustix::process::Pid> {
    rustix::process::Pid::from_raw(i32::try_from(pid).ok()?)
}

/// Signals the whole group, and says nothing when there is no group left.
fn signal(pid: u32, sig: rustix::process::Signal) {
    if let Some(group) = group_of(pid) {
        let _ = rustix::process::kill_process_group(group, sig);
    }
}

/// Whether anything of the group is still there.
fn group_alive(pid: u32) -> bool {
    group_of(pid).is_some_and(|g| rustix::process::test_kill_process_group(g).is_ok())
}

/// Pumps a stream into the channel until it ends.
fn reader<R: std::io::Read + Send + 'static>(
    from: Option<R>,
    into: mpsc::Sender<Line>,
    from_stdout: bool,
) {
    let Some(from) = from else {
        return;
    };
    let built = std::thread::Builder::new()
        .name(
            if from_stdout {
                "software-out"
            } else {
                "software-err"
            }
            .into(),
        )
        .spawn(move || {
            ronoctua_software::run::split_lines(from, &|text| {
                into.send(Line { text, from_stdout }).is_ok()
            });
        });
    if let Err(e) = built {
        tracing::warn!(error = %e, "could not start the software reading thread");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::Engine;
    use ronoctua_software::AppKey;
    use ronoctua_software::order::{Consent, Order};
    use std::path::{Path, PathBuf};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-engine-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test directory");
        dir
    }

    fn an_order() -> Order {
        Order::Install {
            key: AppKey::Nix {
                attr: "hello".into(),
            },
            consent: Consent::default(),
        }
    }

    fn a_job(shared: &Arc<Shared>) -> (u64, Arc<Control>) {
        shared.take(
            Engine::Software,
            Work::Software {
                order: Box::new(an_order()),
                label: "Hello".into(),
            },
        )
    }

    /// A script in place of `systemd-run` and the CLI: `/bin/sh` is the one
    /// program a test may start.
    fn a_child(dir: &Path, script: &str) -> Vec<String> {
        let file = dir.join("child.sh");
        std::fs::write(&file, script).expect("write the script");
        vec![
            "/bin/sh".into(),
            file.display().to_string(),
            dir.display().to_string(),
        ]
    }

    fn the_job(shared: &Shared, id: u64) -> crate::job::Job {
        shared
            .snapshot()
            .0
            .into_iter()
            .find(|j| j.id == id)
            .expect("the job is on the list")
    }

    #[test]
    fn the_child_goes_into_a_scope_of_its_own_and_is_told_to_print_its_progress() {
        let argv = command_line(
            "/run/current-system/sw/bin/systemd-run",
            "/nix/store/x/bin/ronoctua-software",
        );
        assert_eq!(
            argv.first().map(String::as_str),
            Some("/run/current-system/sw/bin/systemd-run")
        );
        for flag in [
            "--user",
            "--scope",
            "--quiet",
            "--collect",
            "--slice=app.slice",
        ] {
            assert!(
                argv.iter().any(|a| a == flag),
                "{flag} is missing from {argv:?}"
            );
        }
        assert!(
            argv.windows(2).any(|w| w == ["-p", "OOMPolicy=kill"]),
            "the child is not the one the kernel kills"
        );
        // Everything after the end of options is the command, and the command
        // is the CLI reading one order.
        let end = argv
            .iter()
            .position(|a| a == "--")
            .expect("an end of options");
        assert_eq!(
            argv[end + 1..],
            ["/nix/store/x/bin/ronoctua-software", "run", "--progress"]
        );
        // No option of systemd-run's after it: it would be the CLI's argument.
        assert!(argv[..end].iter().all(|a| a != "run"));
    }

    #[test]
    fn a_step_and_the_two_ends_are_read_and_everything_else_is_kept() {
        let mut heard = Heard::default();
        let step = heard.line("RONOCTUA\tstep\tdownloading\t10\t100\tGIMP", true);
        assert_eq!(
            step,
            Some(Step {
                phase: Phase::Downloading,
                done: 10,
                total: 100,
                now: "GIMP".into()
            })
        );
        // A beat on stderr is chatter, whatever it looks like.
        assert_eq!(
            heard.line("RONOCTUA\tstep\tdownloading\t10\t100\tGIMP", false),
            None
        );
        assert_eq!(heard.line("copying path '/nix/store/…'", false), None);
        assert_eq!(heard.line("   ", true), None, "a blank line is not a word");
        assert_eq!(heard.tail.len(), 2);
        assert!(
            heard
                .line(
                    "RONOCTUA\tdone\t{\"key\":null,\"desktop-ids\":[\"gimp.desktop\"]}",
                    true
                )
                .is_none()
        );
        assert_eq!(
            heard.ending(true, "exit status: 0"),
            Ending::Done(Some(Outcome {
                desktop_ids: vec!["gimp.desktop".into()],
                ..Outcome::default()
            }))
        );
    }

    #[test]
    fn the_tail_keeps_the_last_lines_only() {
        let mut heard = Heard::default();
        for n in 0..(TAIL + 5) {
            heard.line(&format!("line {n}"), false);
        }
        assert_eq!(heard.tail.len(), TAIL);
        assert_eq!(
            heard.tail.back().map(String::as_str),
            Some(format!("line {}", TAIL + 4).as_str())
        );
        let Ending::Failed { trouble, refusal } = heard.ending(false, "exit status: 1") else {
            panic!("a child that exits non-zero failed");
        };
        assert!(
            trouble.starts_with("line 5\n") && trouble.ends_with(&format!("line {}", TAIL + 4)),
            "{trouble}"
        );
        assert_eq!(refusal, None);
    }

    #[test]
    fn a_failed_line_wins_and_its_refusal_is_kept() {
        let mut heard = Heard::default();
        heard.line("error: unable to download", false);
        heard.line(
            "RONOCTUA\tfailed\t{\"refusal\":\"offline\",\"trouble\":\"no route to host\"}",
            true,
        );
        // Even with a zero exit: the child said, in the closed set, that it
        // did not do it.
        assert_eq!(
            heard.ending(true, "exit status: 0"),
            Ending::Failed {
                trouble: "offline: no route to host".into(),
                refusal: Some(Refusal::Offline)
            }
        );
    }

    #[test]
    fn a_child_that_dies_without_a_word_says_how_it_exited() {
        let heard = Heard::default();
        assert_eq!(
            heard.ending(false, "exit status: 9"),
            Ending::Failed {
                trouble: "the software program exited with exit status: 9".into(),
                refusal: None
            }
        );
        // And one that said nothing and exited zero did what it was asked.
        assert_eq!(
            Heard::default().ending(true, "exit status: 0"),
            Ending::Done(None)
        );
    }

    #[test]
    fn the_measure_follows_the_phase() {
        assert_eq!(measure_of(Phase::Downloading), Measure::Bytes);
        for p in Phase::ALL.into_iter().filter(|p| *p != Phase::Downloading) {
            assert_eq!(measure_of(p), Measure::Items, "{p:?}");
        }
    }

    #[test]
    fn an_order_is_handed_over_on_stdin_and_carried_to_the_end() {
        let dir = scratch("done");
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = a_job(&shared);
        let argv = a_child(
            &dir,
            "cat > \"$1/order.json\"\n\
             printf 'RONOCTUA\\tstep\\tdownloading\\t10\\t100\\thello\\n'\n\
             echo 'some chatter'\n\
             printf 'RONOCTUA\\tstep\\tinstalling\\t1\\t2\\thello\\n'\n\
             printf 'RONOCTUA\\tdone\\t{\"key\":{\"source\":\"nix\",\"attr\":\"hello\"},\"desktop-ids\":[\"hello.desktop\"]}\\n'\n",
        );
        let order = serde_json::to_vec(&an_order()).unwrap();
        let ending = carry(&shared, id, &switch, &argv, order.clone(), GRACE);
        assert_eq!(
            ending,
            Ending::Done(Some(Outcome {
                key: Some(AppKey::Nix {
                    attr: "hello".into()
                }),
                desktop_ids: vec!["hello.desktop".into()],
                ..Outcome::default()
            }))
        );
        assert_eq!(
            std::fs::read(dir.join("order.json")).unwrap(),
            order,
            "the order did not arrive whole"
        );

        // The row, before and after the end is written.
        let running = the_job(&shared, id);
        assert_eq!(running.stage, Stage::Running);
        assert_eq!(running.phase, Some(Phase::Installing));
        assert_eq!(
            running.measure,
            Measure::Items,
            "the measure did not follow the phase"
        );
        assert_eq!(
            (running.done, running.total, running.now.as_str()),
            (1, 2, "hello")
        );
        finish(&shared, id, ending);
        let done = the_job(&shared, id);
        assert_eq!(done.stage, Stage::Done);
        assert_eq!(done.phase, None, "a finished job is inside no phase");
        assert_eq!((done.done, done.total), (2, 2));
        assert_eq!(
            done.outcome.map(|o| o.desktop_ids),
            Some(vec!["hello.desktop".to_owned()])
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refusal_is_kept_with_the_phase_it_happened_in() {
        let dir = scratch("failed");
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = a_job(&shared);
        let argv = a_child(
            &dir,
            "cat > /dev/null\n\
             printf 'RONOCTUA\\tstep\\tdownloading\\t512\\t0\\thello\\n'\n\
             printf 'RONOCTUA\\tfailed\\t{\"refusal\":\"offline\",\"trouble\":\"no route to host\"}\\n'\n\
             exit 1\n",
        );
        let ending = carry(&shared, id, &switch, &argv, b"{}".to_vec(), GRACE);
        finish(&shared, id, ending);
        let job = the_job(&shared, id);
        assert_eq!(job.stage, Stage::Failed);
        assert_eq!(job.refusal, Some(Refusal::Offline));
        assert_eq!(job.trouble.as_deref(), Some("offline: no route to host"));
        assert_eq!(
            job.phase,
            Some(Phase::Downloading),
            "the phase is half of the diagnosis"
        );
        assert_eq!(job.measure, Measure::Bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_child_that_fails_without_saying_why_leaves_its_last_words() {
        let dir = scratch("words");
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = a_job(&shared);
        let argv = a_child(
            &dir,
            "cat > /dev/null\necho 'first'\necho 'error: the last word' >&2\nexit 3\n",
        );
        let Ending::Failed { trouble, refusal } =
            carry(&shared, id, &switch, &argv, Vec::new(), GRACE)
        else {
            panic!("a child that exits 3 failed");
        };
        assert!(
            trouble.contains("first") && trouble.contains("error: the last word"),
            "{trouble}"
        );
        assert_eq!(refusal, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn giving_up_ends_the_whole_group_even_a_member_that_ignores_the_polite_ask() {
        let dir = scratch("cancel");
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = a_job(&shared);
        // A shell that ignores SIGTERM, and a grandchild that inherits that —
        // the shape of a `nix` that is slow to let go.
        let argv = a_child(
            &dir,
            "trap '' TERM\n\
             cat > /dev/null\n\
             sleep 60 &\n\
             echo $! > \"$1/grandchild\"\n\
             printf 'RONOCTUA\\tstep\\tbuilding\\t0\\t0\\thello\\n'\n\
             wait\n",
        );
        let pressing = Arc::clone(&switch);
        let watched = Arc::clone(&shared);
        let presser = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(30);
            while the_job(&watched, id).phase != Some(Phase::Building) {
                assert!(
                    Instant::now() < until,
                    "the child never said it was building"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            pressing.set(Flow::Stop);
        });
        let started = Instant::now();
        let grace = Duration::from_millis(300);
        let ending = carry(&shared, id, &switch, &argv, Vec::new(), grace);
        presser.join().unwrap();
        assert_eq!(ending, Ending::Cancelled);
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "giving up took {:?}",
            started.elapsed()
        );

        // The grandchild went with the group: gone, or a zombie on its way.
        let grandchild = std::fs::read_to_string(dir.join("grandchild")).unwrap();
        let stat = PathBuf::from(format!("/proc/{}/stat", grandchild.trim()));
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let state = std::fs::read_to_string(&stat).ok();
            let alive = state.as_deref().is_some_and(|s| {
                s.rsplit_once(") ")
                    .and_then(|(_, rest)| rest.chars().next())
                    .is_some_and(|c| c != 'Z')
            });
            if !alive {
                break;
            }
            assert!(
                Instant::now() < until,
                "a member of the group outlived the cancel: {state:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_job_given_up_before_it_began_starts_nothing() {
        let dir = scratch("early");
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = a_job(&shared);
        switch.set(Flow::Stop);
        let argv = a_child(&dir, "touch \"$1/started\"\n");
        assert_eq!(
            carry(&shared, id, &switch, &argv, Vec::new(), GRACE),
            Ending::Cancelled
        );
        assert!(
            !dir.join("started").exists(),
            "the child was started anyway"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
