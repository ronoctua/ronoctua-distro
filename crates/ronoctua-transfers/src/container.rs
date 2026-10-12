//! The container engine: podman pulling, building and running compose, as a
//! child in its own process group.
//!
//! # Why the queue
//!
//! A pull is minutes of network and a build minutes of processor, and either
//! outlives the window that asked. The queue already owns a child's lines,
//! its cancel and its row ([`crate::software`], [`crate::media`]); what the
//! child IS — which podman, which arguments, in which folder — is
//! `ronoctua-containers`' (`Podman::job_command`), the one crate that runs
//! podman, and podman's path comes from the unit's environment, never from
//! here.
//!
//! # What the row says
//!
//! What podman prints, read by `ronoctua_containers::step_of`: a build counts
//! its steps (`STEP 3/7`), a pull counts the layers it copied, with no total,
//! and every job's `now` is its last line — data, podman's words. Nothing
//! invents a percentage. What went wrong is the last [`TAIL`] lines.
//!
//! # Pause and cancel
//!
//! No pause ([`crate::job::Engine::pauses`]): a `SIGSTOP` would freeze a pull
//! holding podman's storage lock, and every other podman of the person's —
//! the app's own listing included — would wait behind a row somebody forgot
//! was held. Cancel is SIGTERM to the group, and SIGKILL [`GRACE`] later.

use std::collections::VecDeque;
use std::io::{BufRead as _, BufReader};
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use ronoctua_containers::{Job as ContainerJob, Podman, Step};
use ronoctua_files::watch::{Control, Flow};

use crate::engine::{POLL, Progress};
use crate::job::{Stage, Work};
use crate::ledger::Shared;

/// How many of the child's last lines the trouble keeps.
pub const TAIL: usize = 12;

/// How long a cancelled group has to leave before it is killed: long enough
/// for podman to let go of its storage lock.
pub const GRACE: Duration = Duration::from_secs(3);

/// How often a cancelled group is looked at while it leaves.
const LOOK: Duration = Duration::from_millis(50);

/// How long the pipe may stay open once podman itself is gone: a helper
/// that inherited it (a container's monitor, started by `compose up`) could
/// hold it for as long as the container runs, and a job that never ends
/// because of somebody else's descriptor is worse than one whose last line is
/// lost.
const DRAIN: Duration = Duration::from_secs(2);

/// The longest line kept, in bytes — a build's `RUN` can print a megabyte on
/// one line, and the row shows one line.
const LINE_MOST_BYTES: usize = 4096;

/// How it ended.
#[derive(Debug, PartialEq, Eq)]
enum Ending {
    Done,
    Failed(String),
    Cancelled,
}

/// What the lines said so far: the progress, and the tail kept for the
/// trouble. Pure, so what a line does to a row is tested without a child.
#[derive(Debug, Default)]
struct Heard {
    layers: u64,
    step: Option<(u64, u64)>,
    last: String,
    tail: VecDeque<String>,
}

impl Heard {
    /// One line: where the job is now, as `(done, total, now)`.
    fn line(&mut self, text: &str) -> (u64, u64, String) {
        let text = cut(text.trim());
        if !text.is_empty() {
            if self.tail.len() == TAIL {
                self.tail.pop_front();
            }
            self.tail.push_back(text.clone());
            text.clone_into(&mut self.last);
        }
        match ronoctua_containers::step_of(&text) {
            Some(Step::Of { done, total }) => self.step = Some((done, total)),
            Some(Step::Layer) => self.layers += 1,
            None => {}
        }
        self.now()
    }

    /// Where it is: a build's step of its steps, else the layers counted
    /// with no total — and the last thing said.
    fn now(&self) -> (u64, u64, String) {
        let (done, total) = self.step.unwrap_or((self.layers, 0));
        (done, total, self.last.clone())
    }

    /// Why it failed, in podman's words.
    fn trouble(&self, status: &str) -> String {
        if self.tail.is_empty() {
            format!("podman ended with {status} and said nothing")
        } else {
            Vec::from(self.tail.clone()).join("\n")
        }
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
/// thread: it blocks on the child, and nothing else waits on it.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    let Work::Container { job, .. } = &work else {
        tracing::error!("the container engine got a job that is not a container's");
        shared.touch(id, |j| j.stage = Stage::Failed);
        return;
    };
    // AN INSTALLATION WITHOUT PODMAN gets a sentence at the door, and not a
    // row that sits at nothing for ever.
    let Some(podman) = Podman::from_env() else {
        finish(
            shared,
            id,
            Ending::Failed(
                "this installation has no podman for the queue (containers are off in this \
                 profile)"
                    .to_owned(),
            ),
        );
        return;
    };
    let ending = carry(shared, id, switch, &podman, job, GRACE);
    finish(shared, id, ending);
}

/// Writes how it ended onto the row.
fn finish(shared: &Arc<Shared>, id: u64, ending: Ending) {
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

/// Starts the job's podman in a group of its own and follows it to the end.
/// `grace` is a parameter so a test does not wait three seconds.
fn carry(
    shared: &Arc<Shared>,
    id: u64,
    switch: &Arc<Control>,
    podman: &Podman,
    job: &ContainerJob,
    grace: Duration,
) -> Ending {
    if switch.flow() == Flow::Stop {
        return Ending::Cancelled;
    }
    // ONE PIPE FOR BOTH STREAMS: podman says its progress on stderr and its
    // answer on stdout, and the order between them is the story.
    let Ok((reading, writing)) = std::io::pipe() else {
        return Ending::Failed("could not make a pipe for podman".to_owned());
    };
    let Ok(second) = writing.try_clone() else {
        return Ending::Failed("could not make a pipe for podman".to_owned());
    };
    let mut command = podman.job_command(job);
    command.stdin(Stdio::null()).stdout(writing).stderr(second);
    // ITS OWN GROUP: one signal reaches podman and every helper it started,
    // and a signal meant for them never reaches this daemon.
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let spawned = command.spawn();
    // The command holds the pipe's writing ends: dropped now, or the reader
    // would never see the end when the child does.
    drop(command);
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => return Ending::Failed(format!("could not run podman: {e}")),
    };
    let pid = child.id();
    shared.touch(id, |j| j.stage = Stage::Running);

    // A READER AND A CHANNEL, because reading a line blocks and the switch
    // has to stay answerable.
    let (lines, arriving) = mpsc::channel::<String>();
    let reader = std::thread::Builder::new()
        .name("container-out".into())
        .spawn(move || {
            let mut out = BufReader::new(reading);
            let mut raw = Vec::new();
            loop {
                raw.clear();
                match out.read_until(b'\n', &mut raw) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        // `\r` too: a progress bar rewrites its line.
                        for piece in String::from_utf8_lossy(&raw).split(['\n', '\r']) {
                            if !piece.trim().is_empty() && lines.send(piece.to_owned()).is_err() {
                                return;
                            }
                        }
                    }
                }
            }
        });
    if let Err(e) = reader {
        stop(&mut child, pid, grace);
        return Ending::Failed(format!("could not read podman: {e}"));
    }

    let mut progress = Progress::new(shared, id);
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
                let (done, total, now) = heard.line(&line);
                progress.set(done, total, &now);
            }
            Err(RecvTimeoutError::Timeout) => {}
            // The pipe ended: podman and everything it started are done
            // writing.
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if exited.is_none() {
            match child.try_wait() {
                Ok(Some(status)) => exited = Some((status, Instant::now())),
                Ok(None) => {}
                Err(e) => {
                    progress.flush();
                    stop(&mut child, pid, grace);
                    return Ending::Failed(format!("could not wait for podman: {e}"));
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
        Some((status, _)) => Ok(status),
        None => child.wait(),
    };
    match status {
        Ok(status) if status.success() => Ending::Done,
        Ok(status) => Ending::Failed(heard.trouble(&status.to_string())),
        Err(e) => Ending::Failed(format!("could not wait for podman: {e}")),
    }
}

/// Asks the group to leave, and makes it leave `grace` later.
fn stop(child: &mut Child, pid: u32, grace: Duration) {
    signal(pid, rustix::process::Signal::TERM);
    let until = Instant::now() + grace;
    let mut reaped = false;
    while Instant::now() < until {
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

/// Signals the whole group.
fn signal(pid: u32, sig: rustix::process::Signal) {
    if let Some(group) = group_of(pid) {
        let _ = rustix::process::kill_process_group(group, sig);
    }
}

/// Whether anything of the group is still there.
fn group_alive(pid: u32) -> bool {
    group_of(pid).is_some_and(|g| rustix::process::test_kill_process_group(g).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{Engine, Job};
    use std::path::PathBuf;

    #[test]
    fn a_build_counts_steps_a_pull_counts_layers_and_the_tail_is_the_trouble() {
        let mut build = Heard::default();
        build.line("STEP 1/3: FROM alpine");
        assert_eq!(build.line("STEP 2/3: RUN apk add curl").0, 2);
        let (done, total, now) = build.line("fetch https://dl-cdn.alpinelinux.org/");
        assert_eq!(
            (done, total),
            (2, 3),
            "a line that is no step moves nothing"
        );
        assert_eq!(now, "fetch https://dl-cdn.alpinelinux.org/");

        let mut pull = Heard::default();
        pull.line("Trying to pull docker.io/library/nginx:latest...");
        pull.line("Copying blob sha256:aaaa");
        let (done, total, _) = pull.line("Copying blob sha256:bbbb");
        assert_eq!(
            (done, total),
            (2, 0),
            "layers are counted and no total is invented"
        );

        let mut failing = Heard::default();
        for n in 0..(TAIL + 5) {
            failing.line(&format!("line {n}"));
        }
        let trouble = failing.trouble("exit status: 125");
        assert_eq!(trouble.lines().count(), TAIL);
        assert!(trouble.ends_with(&format!("line {}", TAIL + 4)));
        assert!(
            Heard::default()
                .trouble("exit status: 1")
                .contains("said nothing")
        );
        let long = "é".repeat(LINE_MOST_BYTES);
        assert!(cut(&long).len() <= LINE_MOST_BYTES);
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-container-engine-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A stand-in podman: a script, run as podman would be.
    fn stand_in(dir: &std::path::Path, body: &str) -> Podman {
        let program = dir.join("podman");
        crate::write_executable(&program, &format!("#!/bin/sh\n{body}\n"));
        Podman { program }
    }

    fn a_row(shared: &Arc<Shared>, job: ContainerJob) -> (u64, Arc<Control>) {
        shared.take(
            Engine::Container,
            Work::Container {
                job,
                label: "nginx".to_owned(),
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
    fn a_job_is_followed_to_its_end_and_a_failure_says_why() {
        let dir = scratch("end");
        let shared = Arc::new(Shared::fresh_one());
        let job = ContainerJob::Pull {
            image: "nginx".to_owned(),
        };
        let (id, switch) = a_row(&shared, job.clone());
        let podman = stand_in(
            &dir,
            "echo 'Copying blob sha256:aaaa' >&2\necho 'Copying blob sha256:bbbb' >&2\necho abc123",
        );
        let ending = carry(&shared, id, &switch, &podman, &job, LOOK);
        assert_eq!(ending, Ending::Done);
        finish(&shared, id, ending);
        let row = the_job(&shared, id);
        assert_eq!(row.stage, Stage::Done);
        assert_eq!(row.done, 2, "two layers were said");
        assert_eq!(row.now, "abc123", "the last line is the row's now");

        let failing = stand_in(&dir, "echo 'Error: image not known' >&2\nexit 125");
        let ending = carry(&shared, id, &switch, &failing, &job, LOOK);
        assert_eq!(ending, Ending::Failed("Error: image not known".to_owned()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_cancel_takes_the_whole_group_down() {
        let dir = scratch("cancel");
        let shared = Arc::new(Shared::fresh_one());
        let job = ContainerJob::ComposeUp {
            project: "site".to_owned(),
            files: vec![dir.join("compose.yaml")],
        };
        let (id, switch) = a_row(&shared, job.clone());
        let pids = dir.join("pids");
        let podman = stand_in(
            &dir,
            &format!(
                "sleep 30 &\necho $! > '{}'\necho started\nwait",
                pids.display()
            ),
        );
        let (tx, rx) = mpsc::channel();
        let (s, w) = (Arc::clone(&shared), Arc::clone(&switch));
        let thread = std::thread::spawn(move || {
            let _ = tx.send(carry(&s, id, &w, &podman, &job, LOOK));
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        while !pids.exists() {
            assert!(Instant::now() < deadline, "the stand-in never started");
            std::thread::sleep(LOOK);
        }
        switch.set(Flow::Stop);
        let ending = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(ending, Ending::Cancelled);
        thread.join().unwrap();
        let grandchild: u32 = std::fs::read_to_string(&pids)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let proc = PathBuf::from(format!("/proc/{grandchild}"));
        assert!(
            !proc.exists()
                || std::fs::read_to_string(proc.join("stat")).is_ok_and(|s| s.contains(") Z ")),
            "the helper podman started outlived the cancel"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
