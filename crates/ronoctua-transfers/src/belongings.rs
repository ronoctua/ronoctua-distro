//! The belongings engine: their CLI, as a child, in its own process group.
//!
//! # Why a child
//!
//! The software engine's reason ([`crate::software`]) and a sharper one: the
//! CLI links `age`, and this daemon — up for as long as the machine is —
//! must not. The queue starts `ronoctua-belongings <op> --progress`, reads its
//! lines ([`ronoctua_belongings::order::Beat`]), and kills its group when told
//! to. Nothing secret crosses: the passphrase is a `0600` runtime file a door
//! wrote, and the CLI reads it, and deletes it, itself.
//!
//! # Where the CLI is
//!
//! `RONOCTUA_BELONGINGS`, which the unit sets (`nix/modules/belongings.nix`)
//! — by store path, never a name on a unit's minimal `PATH`. The development
//! loop has no unit: there the CLI is the one BESIDE this daemon, which cargo
//! built into the same `bin/`.
//!
//! # What it does not honour
//!
//! Pause ([`crate::job::Engine::pauses`]). Cancel is SIGTERM to the group, and
//! SIGKILL to what is left [`GRACE`] later; the CLI's own `abandon` is what
//! drops a session, and a door runs it.

use std::collections::VecDeque;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use ronoctua_belongings::Why;
use ronoctua_belongings::order::{Beat, Op};
use ronoctua_files::watch::{Control, Flow};

use crate::engine::{POLL, Progress};
use crate::job::{Stage, Work};
use crate::ledger::Shared;

/// How long a cancelled child has to leave before its group is killed.
pub const GRACE: Duration = Duration::from_secs(2);

/// How many of the child's other lines are kept for the trouble.
const TAIL: usize = 12;

/// The variable naming the CLI's store path.
pub const VARIABLE: &str = "RONOCTUA_BELONGINGS";

/// Where the CLI is: the unit's variable, else beside this daemon (the
/// development loop). Never a bare name.
fn program() -> Option<std::path::PathBuf> {
    std::env::var_os(VARIABLE)
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let me = std::env::current_exe().ok()?;
            let it = me.parent()?.join("ronoctua-belongings");
            it.exists().then_some(it)
        })
}

/// The command line, program first. Pure, and tested as data.
#[must_use]
pub fn command_line(program: &str, op: Op) -> Vec<String> {
    vec![
        program.to_owned(),
        op.word().to_owned(),
        "--progress".to_owned(),
    ]
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ending {
    Done,
    /// The CLI said why, on its own line: it ended its own way, and dealt
    /// with its secrets itself.
    Refused(String),
    /// It died, or could not be started.
    Failed(String),
    Cancelled,
}

/// What the child said so far.
#[derive(Debug, Default)]
struct Heard {
    failed: Option<Why>,
    done: bool,
    tail: VecDeque<String>,
}

impl Heard {
    /// One line; a step comes back to be shown.
    fn line(&mut self, text: &str, from_stdout: bool) -> Option<(u64, u64)> {
        if from_stdout && let Some(beat) = Beat::of_line(text) {
            match beat {
                Beat::Step { done, total, .. } => return Some((done, total)),
                Beat::Done { .. } => self.done = true,
                Beat::Failed(why) => self.failed = Some(why),
            }
            return None;
        }
        let text = text.trim_end();
        if !text.is_empty() {
            self.tail.push_back(text.to_owned());
            if self.tail.len() > TAIL {
                self.tail.pop_front();
            }
        }
        None
    }

    /// How it ended, given the exit. A `failed` line wins; then the exit.
    fn ending(self, success: bool, status: &str) -> Ending {
        if let Some(why) = self.failed {
            return Ending::Refused(format!("the belongings refused: {}", why.id()));
        }
        if success {
            return Ending::Done;
        }
        Ending::Failed(if self.tail.is_empty() {
            format!("the belongings program exited with {status}")
        } else {
            Vec::from(self.tail).join("\n")
        })
    }
}

/// Runs one order to its end, or to what the switch says. The job's thread.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    let Work::Belongings { op } = work else {
        tracing::error!("the belongings engine got a job that is not theirs");
        shared.touch(id, |j| j.stage = Stage::Failed);
        return;
    };
    let ending = match program() {
        Some(program) => carry(
            shared,
            id,
            switch,
            &command_line(&program.to_string_lossy(), op),
            GRACE,
        ),
        None => Ending::Failed(format!(
            "this installation does not have the belongings program ({VARIABLE} is not set)"
        )),
    };
    // A RUN THAT DID NOT END ITS OWN WAY leaves no secret behind: a CLI
    // cancelled or killed half way never reached the line that deletes the
    // passphrase's file, and it would sit in the runtime folder until the
    // machine stops. One that ended its own way — done, waiting for
    // decisions, or refusing with its reason — dealt with it itself: a sync
    // waiting keeps it for `apply`, and a run refused as BUSY leaves it to
    // the run in flight, which is using it.
    if matches!(ending, Ending::Cancelled | Ending::Failed(_))
        && let Some(places) = ronoctua_belongings::places::Places::from_env()
    {
        forget_the_passphrase(&places);
    }
    shared.touch(id, |j| match ending {
        Ending::Done => {
            j.stage = Stage::Done;
            if j.total == 0 || j.done > j.total {
                j.total = j.done;
            } else {
                j.done = j.total;
            }
        }
        Ending::Refused(trouble) | Ending::Failed(trouble) => {
            j.stage = Stage::Failed;
            j.trouble = Some(trouble);
        }
        Ending::Cancelled => j.stage = Stage::Cancelled,
    });
}

/// Deletes the passphrase's files a run left — the old one and the new one.
fn forget_the_passphrase(places: &ronoctua_belongings::places::Places) {
    for file in [places.passphrase(), places.new_passphrase()] {
        match std::fs::remove_file(&file) {
            Ok(()) => {
                tracing::info!("the belongings' passphrase, left by a run cut short, is gone")
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(error = %e, "the belongings' passphrase could not be deleted"),
        }
    }
}

/// One line, and which stream it came from.
struct Line {
    text: String,
    from_stdout: bool,
}

/// Starts `argv` in a group of its own and follows it to the end.
fn carry(
    shared: &Arc<Shared>,
    id: u64,
    switch: &Arc<Control>,
    argv: &[String],
    grace: Duration,
) -> Ending {
    if switch.flow() == Flow::Stop {
        return Ending::Cancelled;
    }
    let Some((program, args)) = argv.split_first() else {
        return Ending::Failed("no program to run".to_owned());
    };
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => return Ending::Failed(format!("could not run {program}: {e}")),
    };
    let pid = child.id();
    shared.touch(id, |j| j.stage = Stage::Running);
    let (lines, arriving) = mpsc::channel::<Line>();
    reader(child.stdout.take(), lines.clone(), true);
    reader(child.stderr.take(), lines, false);
    let mut progress = Progress::new(shared, id);
    let mut heard = Heard::default();
    loop {
        if switch.flow() == Flow::Stop {
            progress.flush();
            crate::software::stop(&mut child, pid, grace);
            return Ending::Cancelled;
        }
        match arriving.recv_timeout(POLL) {
            Ok(line) => {
                if let Some((done, total)) = heard.line(&line.text, line.from_stdout) {
                    progress.set(done, total, "");
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    progress.flush();
    match child.wait() {
        Ok(status) => heard.ending(status.success(), &status.to_string()),
        Err(e) => Ending::Failed(format!("could not wait for the belongings program: {e}")),
    }
}

/// Pumps a stream into the channel, line by line, until it ends.
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
                "belongings-out"
            } else {
                "belongings-err"
            }
            .into(),
        )
        .spawn(move || {
            use std::io::BufRead as _;
            let mut from = std::io::BufReader::new(from);
            let mut bytes = Vec::new();
            loop {
                bytes.clear();
                match from.read_until(b'\n', &mut bytes) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let text = String::from_utf8_lossy(&bytes).into_owned();
                        if into.send(Line { text, from_stdout }).is_err() {
                            break;
                        }
                    }
                }
            }
        });
    if let Err(e) = built {
        tracing::warn!(error = %e, "could not start the belongings reading thread");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::Engine;

    #[test]
    fn the_command_is_the_cli_its_order_and_the_progress_flag() {
        assert_eq!(
            command_line(
                "/nix/store/x-ronoctua-belongings/bin/ronoctua-belongings",
                Op::Apply
            ),
            [
                "/nix/store/x-ronoctua-belongings/bin/ronoctua-belongings",
                "apply",
                "--progress"
            ]
        );
    }

    #[test]
    fn a_failed_line_wins_and_chatter_is_the_trouble() {
        let mut heard = Heard::default();
        assert_eq!(heard.line("git: fatal: nope", false), None);
        assert_eq!(
            heard.line("BELONGINGS\tstep\tencrypting\t2\t5", true),
            Some((2, 5))
        );
        assert_eq!(
            heard.line("BELONGINGS\tfailed\twrong-passphrase", true),
            None
        );
        assert_eq!(
            heard.ending(false, "exit status: 1"),
            Ending::Refused("the belongings refused: wrong-passphrase".to_owned())
        );
        let mut heard = Heard::default();
        heard.line("something broke", false);
        assert_eq!(
            heard.ending(false, "exit status: 101"),
            Ending::Failed("something broke".to_owned())
        );
        assert_eq!(Heard::default().ending(true, ""), Ending::Done);
    }

    /// FAILS IF: a run cut short leaves the passphrase's files behind.
    #[test]
    fn a_run_cut_short_leaves_no_passphrase() {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-transfers-belongings-forget-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let places = ronoctua_belongings::places::Places::under(&root);
        for file in [places.passphrase(), places.new_passphrase()] {
            ronoctua_belongings::secret::write_private(&file, b"secret").unwrap();
        }
        forget_the_passphrase(&places);
        assert!(!places.passphrase().exists() && !places.new_passphrase().exists());
        // AND WITH NOTHING THERE, nothing goes wrong.
        forget_the_passphrase(&places);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_script_in_the_clis_place_is_followed_to_the_end() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-belongings-engine-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("cli.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\nprintf 'BELONGINGS\\tstep\\treading\\t1\\t2\\n'\n\
             printf 'BELONGINGS\\tdone\\t0\\n'\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = shared.take(Engine::Belongings, Work::Belongings { op: Op::Prepare });
        let argv = vec!["/bin/sh".to_owned(), script.to_string_lossy().into_owned()];
        assert_eq!(carry(&shared, id, &switch, &argv, GRACE), Ending::Done);
        let (jobs, _) = shared.snapshot();
        assert_eq!(jobs[0].stage, Stage::Running);
        assert_eq!((jobs[0].done, jobs[0].total), (1, 2));
        let _ = std::fs::remove_dir_all(dir);
    }
}
