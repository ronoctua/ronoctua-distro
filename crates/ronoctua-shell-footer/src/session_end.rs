//! THE SESSION ENDS AFTER WHAT HAS TO END BEFORE IT.
//!
//! # Why
//!
//! `session-quit` asks the compositor to leave, and a program that drew on
//! the compositor may not outlive it: the pure-GTK editor daemon, once it
//! opened a frame there, is ended by GDK (`_exit(1)`) when the display goes —
//! its own way out never runs, so nothing is auto-saved and its exit hook is
//! skipped (`tests/emacs-vm.nix`, `docs/ARCHITECTURE.md`). A stop of the
//! person's manager is safe, because the units are ordered; the compositor
//! leaving on its own is no stop of anybody's unit.
//!
//! So before it asks the compositor to leave, the footer RESTARTS the user
//! units the distro names, when they run: each gets its `SIGTERM` while the
//! compositor is still there and ends the way it ends on purpose, and the
//! instance systemd starts in its place has drawn on nothing, so it outlives
//! the compositor and is there at the next login.
//!
//! # What it waits for
//!
//! The OLD processes, and not the restart. A `try-restart` that blocks
//! returns when the new instance is ready, and a daemon is ready once the
//! person's configuration has loaded — seconds nobody who is leaving should
//! wait for. So [`end_first`] reads each unit's main PID and holds a pidfd on
//! it, asks for the restart with `--no-block` (systemd does the stop AND the
//! start whatever becomes of this process), and waits on the pidfds — an
//! event, not a clock — for [`DEADLINE`] at most. Past it, or when
//! `systemctl` fails, the session ends anyway: the person asked to quit.
//!
//! # Not the footer's names
//!
//! The footer names no program: the units come from the environment
//! ([`VARIABLE`]), which the module that owns each one fills in
//! (`ronoctua.session.restartBeforeQuit`, `nix/modules/base.nix`) — unit
//! names separated by spaces. Absent or empty, the session ends at once, as
//! it always did; the nested loop exports it empty (`nix/dev.nix`), so its
//! footer quitting never restarts the person's own daemons.

use std::os::fd::OwnedFd;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The units restarted before the session ends, separated by spaces.
pub const VARIABLE: &str = "RONOCTUA_SESSION_END_RESTARTS";

/// How long the footer waits for the old processes before it leaves anyway.
///
/// Measured in `tests/emacs-vm.nix`, through the real footer: the editor
/// daemon with a frame open and a buffer modified was gone 61 ms after the
/// PIDs were asked for (the footer logs `took_ms`). Five seconds leaves
/// room for an exit hook eighty times that, and stays far short of
/// systemd's ninety: a program still running then is one the compositor's
/// end takes, as it did before this existed.
pub const DEADLINE: Duration = Duration::from_secs(5);

/// The `systemctl`, by path and never by name: the footer is a unit, and a
/// unit's `PATH` is minimal. Outside Nix (`cargo run`), the name.
const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(c) => c,
    None => "systemctl",
};

/// The units a value of [`VARIABLE`] names: none when it is absent or
/// blank.
///
/// # Errors
///
/// The first word that is not the name of a service — a typo, or a word
/// `systemctl` would read as one of its options. The NixOS option refuses
/// both before a machine is built, so only a value set by hand gets here.
pub fn units_of(value: Option<&str>) -> Result<Vec<String>, String> {
    value
        .unwrap_or_default()
        .split_whitespace()
        .map(|word| {
            if is_a_service(word) {
                Ok(word.to_owned())
            } else {
                Err(word.to_owned())
            }
        })
        .collect()
}

/// `<name>.service`, the name made of what systemd allows in one and not
/// starting with `-`.
fn is_a_service(word: &str) -> bool {
    let Some(name) = word.strip_suffix(".service") else {
        return false;
    };
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '.' | '@' | '-'))
}

/// Where the footer is in ending the session.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// Nobody asked.
    #[default]
    Not,
    /// The units are ending; the compositor is asked when they have ended,
    /// at the deadline, or at a second quit — whichever comes first.
    Waiting,
    /// The compositor was asked to leave.
    Asked,
}

/// What the footer does next.
#[derive(Debug, PartialEq, Eq)]
pub enum Next {
    /// Ask the compositor to leave.
    Leave,
    /// End these units first, and wait.
    EndFirst(Vec<String>),
    /// Nothing: the compositor was asked already (a late answer, a late
    /// deadline).
    Nothing,
}

impl Ending {
    /// A person asked to quit, with the units the environment names.
    ///
    /// A second quit while the units end leaves at once: whoever asks again
    /// is the person who would rather lose what is still saving than wait.
    pub fn quit(&mut self, units: &[String]) -> Next {
        match self {
            Self::Not if !units.is_empty() => {
                *self = Self::Waiting;
                Next::EndFirst(units.to_vec())
            }
            // ASKED ALREADY, and the compositor is still here to be asked:
            // asking again is what the gesture means, as it did before.
            Self::Not | Self::Waiting | Self::Asked => {
                *self = Self::Asked;
                Next::Leave
            }
        }
    }

    /// The wait is over: the thread answered, or the deadline passed.
    pub fn over(&mut self) -> Next {
        if *self == Self::Waiting {
            *self = Self::Asked;
            Next::Leave
        } else {
            Next::Nothing
        }
    }
}

/// How the units ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Ended {
    /// Every old process is gone, this long after the restart was asked.
    Gone {
        /// From asking `systemctl` to the last old process gone.
        took: Duration,
    },
    /// Some old processes were still there at the deadline.
    Overdue {
        /// How many.
        left: usize,
    },
    /// `systemctl` could not be run, or refused, in its own words.
    Failed(String),
}

/// Restarts `units` when they run, and waits at most `deadline` for their
/// old processes to end. It BLOCKS: a thread's, never the loop's.
#[must_use]
pub fn end_first(units: &[String], deadline: Duration) -> Ended {
    end_first_with(SYSTEMCTL, units, deadline)
}

/// [`end_first`] with the `systemctl` named — a test's stand-in.
fn end_first_with(systemctl: &str, units: &[String], deadline: Duration) -> Ended {
    let asked = Instant::now();
    let until = asked + deadline;
    // THE OLD PROCESSES, held BEFORE the restart is asked: a PID read after
    // it could be the new instance's, and waiting on that would wait for the
    // deadline. A unit that does not run has none (`0`), and nothing is
    // waited for it.
    let shown = match run(
        systemctl,
        &["--user", "show", "--property=MainPID", "--value", "--"],
        units,
    ) {
        Ok(out) => out,
        Err(why) => return Ended::Failed(why),
    };
    let held: Vec<OwnedFd> = main_pids(&shown)
        .into_iter()
        .filter_map(|pid| {
            match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) {
                Ok(fd) => Some(fd),
                // GONE ALREADY: nothing left to wait for.
                Err(rustix::io::Errno::SRCH) => None,
                // ANYTHING ELSE (no descriptor left): this one cannot be
                // waited for, and the log says so — the others still are.
                Err(e) => {
                    let pid = pid.as_raw_pid();
                    tracing::warn!(pid, error = %e, "cannot wait for this process to end");
                    None
                }
            }
        })
        .collect();
    if let Err(why) = run(
        systemctl,
        &["--user", "try-restart", "--no-block", "--"],
        units,
    ) {
        return Ended::Failed(why);
    }
    wait_for(held, asked, until)
}

/// Every main PID `systemctl show --value` printed, one per unit — `0`
/// (a unit that does not run) left out.
fn main_pids(shown: &str) -> Vec<rustix::process::Pid> {
    shown
        .split_whitespace()
        .filter_map(|word| word.parse::<i32>().ok())
        .filter_map(rustix::process::Pid::from_raw)
        .collect()
}

/// Waits on the pidfds until every one is readable — the process ended —
/// or `until`.
fn wait_for(mut held: Vec<OwnedFd>, asked: Instant, until: Instant) -> Ended {
    use rustix::event::{PollFd, PollFlags, Timespec};
    while !held.is_empty() {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ended::Overdue { left: held.len() };
        }
        let timeout = Timespec {
            tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        let mut fds: Vec<PollFd<'_>> = held
            .iter()
            .map(|fd| PollFd::new(fd, PollFlags::IN))
            .collect();
        match rustix::event::poll(&mut fds, Some(&timeout)) {
            Ok(0) => return Ended::Overdue { left: held.len() },
            Ok(_) => {
                // WHICH ENDED, read before the pidfds are let go: the poll's
                // list borrows them.
                let mut ended = vec![false; fds.len()];
                for (slot, fd) in ended.iter_mut().zip(&fds) {
                    *slot = !fd.revents().is_empty();
                }
                drop(fds);
                let mut ended = ended.into_iter();
                held.retain(|_| !ended.next().unwrap_or(false));
            }
            // A signal is a wake-up, not a failure.
            Err(rustix::io::Errno::INTR) => {}
            Err(e) => return Ended::Failed(format!("waiting for the units: {e}")),
        }
    }
    Ended::Gone {
        took: asked.elapsed(),
    }
}

/// `systemctl <verb…> <units…>`: its standard output, or what it said
/// went wrong.
fn run(systemctl: &str, verb: &[&str], units: &[String]) -> Result<String, String> {
    let mut command = Command::new(systemctl);
    command
        .args(verb)
        .args(units)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // NOT THE FOOTER'S WATCHDOG: no child speaks to systemd for its unit.
    crate::watchdog::unshared(&mut command);
    let out = command.output().map_err(|e| format!("{systemctl}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let said = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        Err(if said.is_empty() {
            format!("{systemctl} {}: {}", verb.join(" "), out.status)
        } else {
            said
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn units(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_variable_names_services_or_nothing() {
        assert_eq!(units_of(None), Ok(vec![]));
        assert_eq!(units_of(Some("")), Ok(vec![]));
        assert_eq!(units_of(Some("   ")), Ok(vec![]));
        assert_eq!(
            units_of(Some("  a.service\tb-c@1.service ")),
            Ok(units(&["a.service", "b-c@1.service"]))
        );
        // NOT A SERVICE, AND NOT AN OPTION: refused with the word, never
        // dropped in silence.
        assert_eq!(units_of(Some("a.service b")), Err("b".into()));
        assert_eq!(units_of(Some("a.socket")), Err("a.socket".into()));
        assert_eq!(units_of(Some("--now.service")), Err("--now.service".into()));
        assert_eq!(units_of(Some(".service")), Err(".service".into()));
        assert_eq!(units_of(Some("a/b.service")), Err("a/b.service".into()));
    }

    #[test]
    fn with_nothing_to_end_first_the_session_ends_at_once() {
        let mut e = Ending::default();
        assert_eq!(e.quit(&[]), Next::Leave);
        assert_eq!(e, Ending::Asked);
        // AND THE ANSWER OF NO THREAD asks nothing more.
        assert_eq!(e.over(), Next::Nothing);
    }

    #[test]
    fn the_units_end_first_and_then_the_session() {
        let mut e = Ending::default();
        let named = units(&["a.service"]);
        assert_eq!(e.quit(&named), Next::EndFirst(named.clone()));
        assert_eq!(e, Ending::Waiting);
        // THE THREAD'S ANSWER (or the deadline) leaves, once…
        assert_eq!(e.over(), Next::Leave);
        // …and the other of the two, arriving after, asks nothing.
        assert_eq!(e.over(), Next::Nothing);
    }

    #[test]
    fn a_second_quit_while_waiting_leaves_at_once() {
        let mut e = Ending::default();
        let named = units(&["a.service"]);
        assert!(matches!(e.quit(&named), Next::EndFirst(_)));
        assert_eq!(e.quit(&named), Next::Leave);
        // The thread and the deadline come later, and do nothing.
        assert_eq!(e.over(), Next::Nothing);
        assert_eq!(e.over(), Next::Nothing);
    }

    /// A folder of this test's own, under the temporary directory.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-session-end-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        dir
    }

    /// A stand-in `systemctl`, written by a `/bin/sh` child and never by
    /// this process: a file this process holds open for writing when another
    /// test thread forks stays open in the child until its `exec`, and
    /// running it in that window is `ETXTBSY` at random.
    fn stand_in(dir: &Path, show: &str, restart: &str) -> PathBuf {
        let path = dir.join("systemctl");
        let log = dir.join("asked");
        // WHAT IT WAS ASKED, a line per call; then the verb's answer.
        let body = [
            "#!/bin/sh".to_owned(),
            format!("printf '%s\\n' \"$*\" >> '{}'", log.display()),
            "case \"$2\" in".to_owned(),
            format!("  show) {show} ;;"),
            format!("  try-restart) {restart} ;;"),
            "  *) exit 9 ;;".to_owned(),
            "esac\n".to_owned(),
        ]
        .join("\n");
        let status = Command::new("/bin/sh")
            .args([
                "-c",
                "printf '%s' \"$2\" > \"$1\" && chmod 700 \"$1\"",
                "sh",
            ])
            .arg(&path)
            .arg(&body)
            .status()
            .expect("a shell writes the stand-in");
        assert!(status.success(), "{}", path.display());
        path
    }

    /// A process that waits until it is killed (or its stdin closes): what
    /// a unit's main process stands in for.
    fn a_unit_process() -> std::process::Child {
        Command::new("/bin/sh")
            .args(["-c", "read line"])
            .stdin(Stdio::piped())
            .spawn()
            .expect("a process to end")
    }

    fn asked(dir: &Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("asked"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn the_old_process_ending_is_the_answer() {
        let dir = scratch("gone");
        let mut child = a_unit_process();
        let pid = child.id();
        let systemctl = stand_in(
            &dir,
            &format!("echo {pid}; echo; echo 0"),
            &format!("kill {pid}"),
        );
        let ended = end_first_with(
            systemctl.to_str().expect("a path"),
            &units(&["a.service", "b.service"]),
            Duration::from_secs(20),
        );
        let _ = child.wait();
        match ended {
            Ended::Gone { took } => assert!(took < Duration::from_secs(20), "{took:?}"),
            other => panic!("the process ended and the answer is {other:?}"),
        }
        // THE PIDS FIRST, then the restart that does not block — and the
        // units after `--`, so none is read as an option.
        assert_eq!(
            asked(&dir),
            vec![
                "--user show --property=MainPID --value -- a.service b.service".to_owned(),
                "--user try-restart --no-block -- a.service b.service".to_owned(),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_process_that_does_not_end_is_left_at_the_deadline() {
        let dir = scratch("overdue");
        let mut child = a_unit_process();
        let pid = child.id();
        let systemctl = stand_in(&dir, &format!("echo {pid}"), "true");
        let started = Instant::now();
        let ended = end_first_with(
            systemctl.to_str().expect("a path"),
            &units(&["a.service"]),
            Duration::from_millis(300),
        );
        let waited = started.elapsed();
        // The stand-in for a unit, ended by its stdin closing.
        drop(child.stdin.take());
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(ended, Ended::Overdue { left: 1 });
        assert!(waited >= Duration::from_millis(300), "{waited:?}");
        assert!(waited < Duration::from_secs(10), "{waited:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refused_restart_says_why() {
        let dir = scratch("refused");
        let systemctl = stand_in(
            &dir,
            "echo 0",
            "echo 'Failed to try-restart a.service: Access denied' >&2; exit 1",
        );
        let ended = end_first_with(
            systemctl.to_str().expect("a path"),
            &units(&["a.service"]),
            Duration::from_secs(20),
        );
        assert_eq!(
            ended,
            Ended::Failed("Failed to try-restart a.service: Access denied".into())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn units_that_do_not_run_are_nothing_to_wait_for() {
        let dir = scratch("idle");
        let systemctl = stand_in(&dir, "echo 0", "true");
        let ended = end_first_with(
            systemctl.to_str().expect("a path"),
            &units(&["a.service"]),
            Duration::from_secs(20),
        );
        assert!(matches!(ended, Ended::Gone { .. }), "{ended:?}");
        // The restart is still asked: a unit starting has no PID yet.
        assert_eq!(asked(&dir).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_systemctl_is_a_failure_with_a_sentence() {
        let ended = end_first_with(
            "/nonexistent/systemctl",
            &units(&["a.service"]),
            Duration::from_secs(1),
        );
        match ended {
            Ended::Failed(why) => assert!(why.starts_with("/nonexistent/systemctl: "), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_pids_are_read_one_per_unit_and_zero_is_none() {
        let pids: Vec<i32> = main_pids("123\n\n0\n\n456\n")
            .into_iter()
            .map(rustix::process::Pid::as_raw_pid)
            .collect();
        assert_eq!(pids, vec![123, 456]);
        assert!(main_pids("").is_empty());
    }
}
