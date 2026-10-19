//! The scan engine: one file sent to VirusTotal, through `ronoctua-scan` as a
//! child in its own process group.
//!
//! # Why a child
//!
//! The upload is TLS and this daemon carries none (`transfers-unit-is-narrow`)
//! — the software engine's reason. So the queue does what it does for yt-dlp:
//! starts a program, reads its lines, and kills its group when told. The
//! program is `ronoctua-scan` (`ronoctua-lookup`'s `[[bin]]`), by the store
//! path the unit hands in as `RONOCTUA_SCAN`; without it the row fails with a
//! sentence and nothing is sent.
//!
//! # What the lines are
//!
//! [`Beat`]: hashing, sending (both in bytes), waiting, and one last line,
//! done with the verdict or failed with the reason. The verdict itself is the
//! child's to keep, in `scans.json`, which the security centre reads; the row
//! here says the file was checked or why it was not.
//!
//! # What it does not honour
//!
//! Pause: a paused upload is a socket a host times out. Cancel is SIGTERM to
//! the group, and SIGKILL [`GRACE`] later — and then the file's row in
//! `scans.json` stops saying it is on its way, which the killed child
//! cannot write (`virustotal::cancelled`).

use std::io::BufRead as _;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use ronoctua_files::watch::{Control, Flow};
use ronoctua_lookup::virustotal::Beat;

use crate::engine::{POLL, Progress};
use crate::job::{Measure, Stage, Work};
use crate::ledger::Shared;

/// How long a cancelled child has to leave before its group is killed.
pub const GRACE: Duration = Duration::from_secs(3);

/// The variable naming the child's store path.
pub const VARIABLE: &str = "RONOCTUA_SCAN";

/// The command line, program first. Pure — a test may not start the real one.
#[must_use]
pub fn command_line(program: &str, path: &std::path::Path, sha256: &str) -> Vec<String> {
    vec![
        program.to_owned(),
        path.to_string_lossy().into_owned(),
        sha256.to_owned(),
    ]
}

/// How one scan ended.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ending {
    Done,
    Failed(String),
    Cancelled,
}

/// What a beat does to the row: its measure, and the two numbers.
fn step_of(beat: &Beat) -> Option<(Measure, u64, u64)> {
    match beat {
        Beat::Hashing { done, of } | Beat::Sending { done, of } => {
            Some((Measure::Bytes, *done, *of))
        }
        // WAITING HAS NO END ANYBODY KNOWS: a total of zero is "not known",
        // and the row says the stage instead of a bar.
        Beat::Waiting => Some((Measure::Items, 0, 0)),
        Beat::Done(_) | Beat::Failed(_) => None,
    }
}

/// Runs one scan to the end, or to whatever the switch says. The job's own
/// thread.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    let Work::Scan { path, sha256 } = &work else {
        tracing::error!("the scan engine got a job that is not a scan");
        shared.touch(id, |j| j.stage = Stage::Failed);
        return;
    };
    let ending = match std::env::var(VARIABLE).ok().filter(|v| !v.is_empty()) {
        None => Ending::Failed(
            "this installation does not have the program that sends a file to VirusTotal \
             (RONOCTUA_SCAN is not set)"
                .to_owned(),
        ),
        Some(program) => carry(
            shared,
            id,
            switch,
            &command_line(&program, path, sha256),
            GRACE,
        ),
    };
    let cancelled = ending == Ending::Cancelled;
    shared.touch(id, |j| match ending {
        Ending::Done => {
            j.stage = Stage::Done;
            if j.total == 0 || j.done > j.total {
                j.total = j.done;
            } else {
                j.done = j.total;
            }
        }
        Ending::Failed(trouble) => {
            j.stage = Stage::Failed;
            j.trouble = Some(trouble);
        }
        Ending::Cancelled => j.stage = Stage::Cancelled,
    });
    if cancelled
        && let Err(e) =
            ronoctua_lookup::virustotal::cancelled(&ronoctua_lookup::virustotal::scans(), sha256)
    {
        tracing::warn!(error = %e, "scans.json could not say the send was cancelled");
    }
}

/// Starts `argv` in a group of its own and follows it to the end. `argv` and
/// `grace` are parameters so a test runs a `/bin/sh` script instead.
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
    shared.under_way(id, |_| {});

    // THE READERS ON THREADS, so the switch stays answerable while a line is
    // awaited — the software engine's shape.
    let (lines, arriving) = mpsc::channel::<(String, bool)>();
    for (from_stdout, stream) in [
        (
            true,
            child
                .stdout
                .take()
                .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        ),
        (
            false,
            child
                .stderr
                .take()
                .map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        ),
    ] {
        let Some(stream) = stream else { continue };
        let into = lines.clone();
        let started = std::thread::Builder::new()
            .name("scan-lines".into())
            .spawn(move || {
                for line in std::io::BufReader::new(stream).lines() {
                    let Ok(line) = line else { break };
                    if into.send((line, from_stdout)).is_err() {
                        break;
                    }
                }
            });
        if let Err(e) = started {
            tracing::warn!(error = %e, "could not start the scan's reading thread");
        }
    }
    drop(lines);

    let mut progress = Progress::new(shared, id);
    let mut measure = Measure::Bytes;
    let mut last: Option<Beat> = None;
    let mut chatter: Option<String> = None;
    loop {
        if switch.flow() == Flow::Stop {
            progress.flush();
            stop(&mut child, pid, grace);
            return Ending::Cancelled;
        }
        match arriving.recv_timeout(POLL) {
            Ok((line, true)) => {
                if let Some(beat) = Beat::parse(&line) {
                    if let Some((m, done, total)) = step_of(&beat) {
                        if m != measure {
                            progress.flush();
                            shared.touch(id, |j| j.measure = m);
                            measure = m;
                        }
                        progress.set(done, total, "");
                    }
                    if beat.is_last() {
                        last = Some(beat);
                    }
                }
            }
            Ok((line, false)) => {
                if !line.trim().is_empty() {
                    chatter = Some(line);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    progress.flush();
    let status = child.wait();
    match (last, status) {
        (Some(Beat::Done(_)), Ok(s)) if s.success() => Ending::Done,
        // THE CLOSED WORD, for the log and the row; the security centre's own
        // row says it in a sentence (`scans.json`).
        (Some(Beat::Failed(why)), _) => Ending::Failed(why.id().to_owned()),
        (_, Ok(s)) => {
            Ending::Failed(chatter.unwrap_or_else(|| format!("the scan exited with {s}")))
        }
        (_, Err(e)) => Ending::Failed(format!("could not wait for the scan: {e}")),
    }
}

/// SIGTERM to the group, and SIGKILL `grace` later if anything is left.
fn stop(child: &mut std::process::Child, pid: u32, grace: Duration) {
    let group = i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw);
    if let Some(g) = group {
        let _ = rustix::process::kill_process_group(g, rustix::process::Signal::TERM);
    }
    let until = Instant::now() + grace;
    while Instant::now() < until {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if let Some(g) = group {
        let _ = rustix::process::kill_process_group(g, rustix::process::Signal::KILL);
    }
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::Engine;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-scan-engine-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn script(dir: &std::path::Path, body: &str) -> Vec<String> {
        let path = dir.join("child.sh");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        vec![
            "/bin/sh".to_owned(),
            path.to_string_lossy().into_owned(),
            dir.to_string_lossy().into_owned(),
        ]
    }

    fn a_job(shared: &Arc<Shared>) -> (u64, Arc<Control>) {
        shared.take(
            Engine::Scan,
            Work::Scan {
                path: PathBuf::from("/home/p/a.exe"),
                sha256: "a".repeat(64),
            },
        )
    }

    /// FAILS IF: a `done` line and a clean exit stop ending the row done, or
    /// the bytes sent stop reaching it.
    #[test]
    fn the_beats_move_the_row_and_done_ends_it() {
        let dir = scratch("done");
        let argv = script(
            &dir,
            "printf 'RONOCTUA-SCAN\\tsending\\t50\\t100\\n'\n\
             printf 'RONOCTUA-SCAN\\twaiting\\n'\n\
             printf 'RONOCTUA-SCAN\\tdone\\tclean\\n'",
        );
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = a_job(&shared);
        assert_eq!(carry(&shared, id, &switch, &argv, GRACE), Ending::Done);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a child that says why it failed is not believed, or one that
    /// dies without a word leaves no trouble on the row.
    #[test]
    fn a_failure_says_why() {
        let dir = scratch("failed");
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = a_job(&shared);
        let said = script(&dir, "printf 'RONOCTUA-SCAN\\tfailed\\tno-key\\n'\nexit 1");
        assert_eq!(
            carry(&shared, id, &switch, &said, GRACE),
            Ending::Failed("no-key".to_owned())
        );
        let mute = script(&dir, "echo 'the key file is gone' >&2\nexit 3");
        assert_eq!(
            carry(&shared, id, &switch, &mute, GRACE),
            Ending::Failed("the key file is gone".to_owned())
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a scan given up before it began starts anything, or one
    /// given up while it runs is not stopped.
    #[test]
    fn a_cancel_stops_the_child() {
        let dir = scratch("cancel");
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = a_job(&shared);
        let argv = script(&dir, "touch \"$1/started\"\nsleep 30");
        switch.set(Flow::Stop);
        assert_eq!(
            carry(&shared, id, &switch, &argv, Duration::from_millis(100)),
            Ending::Cancelled
        );
        assert!(!dir.join("started").exists(), "a scan given up began");
        let (id, switch) = a_job(&shared);
        let stopper = Arc::clone(&switch);
        let later = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            stopper.set(Flow::Stop);
        });
        let began = Instant::now();
        assert_eq!(
            carry(&shared, id, &switch, &argv, Duration::from_millis(100)),
            Ending::Cancelled
        );
        later.join().unwrap();
        assert!(
            began.elapsed() < Duration::from_secs(10),
            "the child was not stopped"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_command_line_is_the_program_the_file_and_its_hash() {
        let sha = "b".repeat(64);
        assert_eq!(
            command_line(
                "/nix/store/x/bin/ronoctua-scan",
                std::path::Path::new("/a b/c"),
                &sha
            ),
            ["/nix/store/x/bin/ronoctua-scan", "/a b/c", sha.as_str()]
        );
        assert_eq!(step_of(&Beat::Waiting), Some((Measure::Items, 0, 0)));
    }
}
