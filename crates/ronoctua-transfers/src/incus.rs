//! The Incus engine: `ronoctua-incus-job` as a child in its own process
//! group.
//!
//! # Why a child
//!
//! A launch from a remote image is minutes of download, and an export a file
//! the size of a disk: both outlive the door that asked. The queue owns the
//! row, its cancel and its group — the scan engine's shape; what the work IS
//! (the daemon's calls, its operation followed, the bytes streamed) is
//! `ronoctua-incus`', in its job binary, by the store path the unit hands in
//! (`ronoctua_incus::JOB_VARIABLE`, set by `nix/modules/incus.nix` where
//! Incus is on). This crate's SOURCE names only the job's wire (the crate
//! without its features): no socket of the daemon's, no WebSocket, no
//! consent file. What the binary links is the workspace build's: Cargo
//! unifies features, so it is compiled against the whole crate, and the
//! client is left out only as code nobody calls.
//!
//! # What the row says
//!
//! The job's [`Line`]s: a percentage the daemon reported (things, of a
//! hundred), bytes streamed to or from a file (bytes), and the daemon's own
//! words for the stage (the row's `now` — data, as a podman line is). What
//! went wrong is the row's trouble, in English as every row's is: for a
//! refusal, what it means in words ([`refused_words`]), then the job's
//! stderr, its log — the daemon's own words, the last [`TAIL`] lines.
//!
//! # Pause and cancel
//!
//! No pause ([`crate::job::Engine::pauses`]): a stopped job would hold the
//! daemon's operation, and the instance with it. Cancel is SIGTERM to the
//! group: the job cancels the daemon's operation and removes the file it was
//! making, and [`GRACE`] later anything left is killed.

use std::collections::VecDeque;
use std::io::{BufRead as _, Write as _};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use ronoctua_files::watch::{Control, Flow};
use ronoctua_incus::{Job as IncusJob, Line, Step, Why};

use crate::engine::{POLL, Progress};
use crate::job::{Measure, Stage, Work};
use crate::ledger::Shared;

/// How long a cancelled job has to cancel the daemon's operation and remove
/// its half-made file before its group is killed.
pub const GRACE: Duration = Duration::from_secs(10);

/// How many lines of the job's log the trouble keeps.
pub const TAIL: usize = 12;

/// The longest line kept, in bytes.
const LINE_MOST_BYTES: usize = 4096;

/// How it ended.
#[derive(Debug, PartialEq, Eq)]
enum Ending {
    Done,
    Failed(String),
    Cancelled,
}

/// What the job said so far. Pure, so what a line does to a row is tested
/// without a child.
#[derive(Debug, Default)]
struct Heard {
    measure: Option<Measure>,
    done: u64,
    total: u64,
    now: String,
    verdict: Option<Line>,
    tail: VecDeque<String>,
}

impl Heard {
    /// A line of stdout: the row's measure, numbers and words when it moved
    /// them.
    fn out(&mut self, text: &str) -> Option<(Measure, u64, u64, String)> {
        match Line::parse(text)? {
            Line::Step(Step::Percent(p)) => {
                self.measure = Some(Measure::Items);
                (self.done, self.total) = (u64::from(p), 100);
            }
            Line::Step(Step::Bytes { done, total }) => {
                self.measure = Some(Measure::Bytes);
                (self.done, self.total) = (done, total.unwrap_or(0));
            }
            Line::Said(words) => self.now = words,
            verdict @ (Line::Done | Line::Refused(_)) => {
                self.verdict = Some(verdict);
                return None;
            }
        }
        Some((
            self.measure.unwrap_or(Measure::Items),
            self.done,
            self.total,
            self.now.clone(),
        ))
    }

    /// A line of the job's log, kept for the trouble.
    fn err(&mut self, text: &str) {
        let text = cut(text.trim());
        if text.is_empty() {
            return;
        }
        if self.tail.len() == TAIL {
            self.tail.pop_front();
        }
        self.tail.push_back(text);
    }

    /// How the job ended, from its last line and its status.
    fn ending(&self, status: &std::io::Result<std::process::ExitStatus>) -> Ending {
        match (&self.verdict, status) {
            (Some(Line::Done), Ok(s)) if s.success() => Ending::Done,
            // A REFUSAL SAYS WHAT IT IS FIRST, in words, and then the daemon's
            // own, from the log.
            (Some(Line::Refused(why)), _) => Ending::Failed(
                std::iter::once(refused_words(*why).to_owned())
                    .chain(self.tail.iter().cloned())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            (_, Ok(s)) => Ending::Failed(if self.tail.is_empty() {
                format!("the incus job exited with {s} and said nothing")
            } else {
                Vec::from(self.tail.clone()).join("\n")
            }),
            (_, Err(e)) => Ending::Failed(format!("could not wait for the incus job: {e}")),
        }
    }
}

/// What a refusal of the job's means, in words.
///
/// ENGLISH, as the rest of a row's trouble: `Job::trouble` is one of the
/// places one language fits (AGENTS.md, inv. 18), next to the daemon's own
/// words that follow it. The `incus-refused-<id>` sentences are the doors',
/// which hold a [`Why`] (`Refusal`); a row holds text.
const fn refused_words(why: Why) -> &'static str {
    match why {
        Why::NoIncus => "Incus refused: it is not on this machine, or its daemon is not running",
        Why::NotInGroup => {
            "Incus refused: this login is not in the incus-admin group yet; log out and in again"
        }
        Why::Gone => "Incus refused: the instance, image or file is not there",
        Why::Busy => "Incus refused: another operation holds it",
        Why::NotLet => {
            "Incus refused: a host the image comes from has not been allowed (consent.json)"
        }
        Why::Invalid => {
            "Incus refused: a name, path or limit it does not take, or a file that is already there"
        }
        Why::Failed => "Incus refused it",
    }
}

/// A line cut to what a row keeps, on a character boundary.
fn cut(text: &str) -> String {
    if text.len() <= LINE_MOST_BYTES {
        return text.to_owned();
    }
    let mut end = LINE_MOST_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// Runs one job to its end, or to whatever the switch says. The job's own
/// thread.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    // NO SECRET OUTLIVES A JOB — an Incus job carries none, and the rule
    // holds anyway.
    for secret in work.secrets() {
        crate::secrets::forget(secret);
    }
    let Work::Incus { job, .. } = &work else {
        tracing::error!("the incus engine got a job that is not an Incus one");
        shared.touch(id, |j| j.stage = Stage::Failed);
        return;
    };
    // AN INSTALLATION WITHOUT INCUS gets a sentence, not a row that waits.
    let ending = match std::env::var(ronoctua_incus::JOB_VARIABLE)
        .ok()
        .filter(|v| !v.is_empty())
    {
        None => Ending::Failed(
            "this installation has no Incus job for the queue (Incus is off in this profile)"
                .to_owned(),
        ),
        Some(program) => carry(shared, id, switch, &program, job, GRACE),
    };
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
}

/// Starts the job in a group of its own, hands it the job on stdin and
/// follows it to the end. `program` and `grace` are parameters so a test
/// runs a script and does not wait ten seconds.
fn carry(
    shared: &Arc<Shared>,
    id: u64,
    switch: &Arc<Control>,
    program: &str,
    job: &IncusJob,
    grace: Duration,
) -> Ending {
    if switch.flow() == Flow::Stop {
        return Ending::Cancelled;
    }
    let Ok(line) = serde_json::to_string(job) else {
        return Ending::Failed("the job could not be written".to_owned());
    };
    let mut command = Command::new(program);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // ITS OWN GROUP: one signal reaches the job and whatever it started.
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => return Ending::Failed(format!("could not run the incus job: {e}")),
    };
    let pid = child.id();
    // THE JOB, WHOLE, and then the end of its stdin: it reads one line.
    if let Some(mut stdin) = child.stdin.take()
        && stdin.write_all(format!("{line}\n").as_bytes()).is_err()
    {
        stop(&mut child, pid, grace);
        return Ending::Failed("could not hand the job to the incus job".to_owned());
    }
    shared.under_way(id, |_| {});

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
            .name("incus-lines".into())
            .spawn(move || {
                for line in std::io::BufReader::new(stream).lines() {
                    let Ok(line) = line else { break };
                    if into.send((line, from_stdout)).is_err() {
                        break;
                    }
                }
            });
        if let Err(e) = started {
            tracing::warn!(error = %e, "could not start the incus job's reading thread");
        }
    }
    drop(lines);

    let mut progress = Progress::new(shared, id);
    let mut heard = Heard::default();
    let mut measure = Measure::Items;
    loop {
        if switch.flow() == Flow::Stop {
            progress.flush();
            stop(&mut child, pid, grace);
            return Ending::Cancelled;
        }
        match arriving.recv_timeout(POLL) {
            Ok((line, true)) => {
                if let Some((m, done, total, now)) = heard.out(&line) {
                    if m != measure {
                        progress.flush();
                        shared.touch(id, |j| j.measure = m);
                        measure = m;
                    }
                    progress.set(done, total, &now);
                }
            }
            Ok((line, false)) => heard.err(&line),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    progress.flush();
    heard.ending(&child.wait())
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

// ── anchor: L13 ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{Engine, Job};
    use std::path::{Path, PathBuf};

    #[test]
    fn a_percentage_is_things_bytes_are_bytes_and_the_words_are_now() {
        let mut heard = Heard::default();
        assert_eq!(
            heard.out("percent 23"),
            Some((Measure::Items, 23, 100, String::new()))
        );
        assert_eq!(
            heard.out("said rootfs: 23% (12.30MB/s)"),
            Some((
                Measure::Items,
                23,
                100,
                "rootfs: 23% (12.30MB/s)".to_owned()
            ))
        );
        assert_eq!(
            heard.out("bytes 1048576/-"),
            Some((
                Measure::Bytes,
                1_048_576,
                0,
                "rootfs: 23% (12.30MB/s)".to_owned()
            )),
            "no total is invented"
        );
        assert_eq!(heard.out("chatter"), None);
        assert_eq!(heard.out("refused gone"), None);
        assert_eq!(
            heard.verdict,
            Some(Line::Refused(ronoctua_incus::Why::Gone))
        );
        heard.err("incus refused code=404 said=\"Instance not found\"");
        let failed = heard.ending(&Ok(std::process::ExitStatus::default()));
        assert_eq!(
            failed,
            Ending::Failed(
                "Incus refused: the instance, image or file is not there\n\
                 incus refused code=404 said=\"Instance not found\""
                    .to_owned()
            ),
            "the refusal in words, then the daemon's"
        );
        let mut quiet = Heard::default();
        quiet.out("refused not-let");
        assert_eq!(
            quiet.ending(&Ok(std::process::ExitStatus::default())),
            Ending::Failed(
                "Incus refused: a host the image comes from has not been allowed (consent.json)"
                    .to_owned()
            ),
            "never the bare id"
        );
    }

    /// FAILS IF: a refused job's row says the wire's id (`not-let`) instead
    /// of words, for any refusal.
    #[test]
    fn every_refusal_is_said_in_words() {
        for why in Why::ALL {
            let words = refused_words(why);
            assert!(
                words.starts_with("Incus refused") && !words.contains(why.id()),
                "{why:?}: {words}"
            );
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-incus-engine-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A stand-in job: a script, run as the job would be.
    fn stand_in(dir: &Path, body: &str) -> String {
        let program = dir.join("ronoctua-incus-job");
        crate::write_executable(&program, &format!("#!/bin/sh\n{body}\n"));
        program.to_string_lossy().into_owned()
    }

    fn export() -> IncusJob {
        IncusJob::Export {
            name: "web".to_owned(),
            to: "/home/p/web.tar.gz".into(),
        }
    }

    fn a_row(shared: &Arc<Shared>) -> (u64, Arc<Control>) {
        shared.take(
            Engine::Incus,
            Work::Incus {
                job: Box::new(export()),
                label: "web".to_owned(),
            },
        )
    }

    fn the_job(shared: &Shared, id: u64) -> Job {
        shared
            .snapshot()
            .0
            .into_iter()
            .find(|j| j.id == id)
            .expect("the job is on the list")
    }

    #[test]
    fn the_job_is_handed_over_whole_and_followed_to_its_end() {
        let dir = scratch("end");
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = a_row(&shared);
        let program = stand_in(
            &dir,
            &format!(
                "cat > '{}/got'\necho 'said Backing up instance'\necho 'bytes 1048576/2097152'\necho 'bytes 2097152/2097152'\necho done",
                dir.display()
            ),
        );
        let ending = carry(
            &shared,
            id,
            &switch,
            &program,
            &export(),
            Duration::from_millis(50),
        );
        assert_eq!(ending, Ending::Done);
        let got = std::fs::read_to_string(dir.join("got")).unwrap();
        assert_eq!(
            serde_json::from_str::<IncusJob>(got.trim()).unwrap(),
            export(),
            "the job crossed whole"
        );
        let row = the_job(&shared, id);
        assert_eq!(row.measure, Measure::Bytes);
        assert_eq!((row.done, row.total), (2_097_152, 2_097_152));
        assert_eq!(row.now, "Backing up instance");

        let refusing = stand_in(
            &dir,
            "cat > /dev/null\necho 'incus refused code=404 said=\"Instance not found\"' >&2\necho 'refused gone'\nexit 1",
        );
        let ending = carry(
            &shared,
            id,
            &switch,
            &refusing,
            &export(),
            Duration::from_millis(50),
        );
        assert_eq!(
            ending,
            Ending::Failed(
                "Incus refused: the instance, image or file is not there\n\
                 incus refused code=404 said=\"Instance not found\""
                    .to_owned()
            )
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_cancel_takes_the_whole_group_down() {
        let dir = scratch("cancel");
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = a_row(&shared);
        let pids = dir.join("pids");
        let program = stand_in(
            &dir,
            &format!(
                "cat > /dev/null\nsleep 30 &\necho $! > '{}'\necho 'percent 1'\nwait",
                pids.display()
            ),
        );
        let (tx, rx) = mpsc::channel();
        let (s, w) = (Arc::clone(&shared), Arc::clone(&switch));
        let thread = std::thread::spawn(move || {
            let _ = tx.send(carry(
                &s,
                id,
                &w,
                &program,
                &export(),
                Duration::from_millis(200),
            ));
        });
        // THE GRANDCHILD'S PID, once it is WRITTEN: the shell makes the file
        // before `echo` fills it, and an empty read is no pid.
        let deadline = Instant::now() + Duration::from_secs(10);
        let grandchild: u32 = loop {
            if let Some(pid) = std::fs::read_to_string(&pids)
                .ok()
                .and_then(|t| t.trim().parse().ok())
            {
                break pid;
            }
            assert!(Instant::now() < deadline, "the stand-in never started");
            std::thread::sleep(Duration::from_millis(20));
        };
        switch.set(Flow::Stop);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Ending::Cancelled
        );
        thread.join().unwrap();
        // GONE — or a zombie — WITHIN A REGIME, never at an instant: the
        // group's `SIGKILL` is delivered when the kernel gets to it, and a
        // machine busy with a battery read `/proc` before it did (measured in
        // the `most` of 2026-10-24). A cancel that did not take the group
        // down leaves `sleep 30` alive past the whole wait.
        let proc = PathBuf::from(format!("/proc/{grandchild}"));
        let gone = || {
            !proc.exists()
                || std::fs::read_to_string(proc.join("stat")).is_ok_and(|s| s.contains(") Z "))
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while !gone() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(gone(), "what the job started outlived the cancel");
        let _ = std::fs::remove_dir_all(dir);
    }
}
