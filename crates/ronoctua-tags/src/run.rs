//! The programs, by path, with a deadline and a ceiling.
//!
//! # Its own runner, and not `ronoctua_inspect::tools`
//!
//! That one takes the inspect crate's `Tool` enum, and a new variant there
//! makes `document-formats-are-total` require the program in the FOOTER's
//! unit. `mkvextract`, `mkvmerge` and `mkvpropedit` belong to the player's
//! unit alone (and only under the media profile), so they are named here, by
//! the same `RONOCTUA_*` convention, and run by the same kind of loop.
//!
//! # The path comes from the environment, read once
//!
//! [`Tools::from_env`] reads the six variables when the engine starts, for
//! the reason `ronoctua_inspect::tools` gives: a store path baked in with
//! `option_env!` would be a closure reference in every binary that links
//! this crate. Tests build a [`Tools`] by hand instead — which is also how
//! the "not installed" path is tested without touching the environment.

#[cfg(test)]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::Read as _;
use std::os::fd::AsFd as _;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::sheet::{Program, Why};

/// Where each program is, when it is anywhere.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tools {
    paths: [Option<PathBuf>; Program::ALL.len()],
}

impl Tools {
    /// None of them — every question that needs one is refused by name.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// The six variables, as this process was started with them.
    ///
    /// An empty value is unset: `FOO=` in a unit is how a thing is turned
    /// off, and a program called "" would fail later and further away.
    #[must_use]
    pub fn from_env() -> Self {
        let mut tools = Self::none();
        for program in Program::ALL {
            if let Some(v) = std::env::var_os(program.variable())
                && !v.is_empty()
            {
                tools = tools.with(program, PathBuf::from(v));
            }
        }
        tools
    }

    /// The same, with this program at this path.
    #[must_use]
    pub fn with(mut self, program: Program, path: PathBuf) -> Self {
        self.paths[slot(program)] = Some(path);
        self
    }

    /// The same, without this program.
    #[must_use]
    pub fn without(mut self, program: Program) -> Self {
        self.paths[slot(program)] = None;
        self
    }

    /// Where it is.
    ///
    /// # Errors
    ///
    /// [`Why::Missing`] naming it.
    pub fn path(&self, program: Program) -> Result<&Path, Why> {
        self.paths[slot(program)]
            .as_deref()
            .ok_or(Why::Missing { program })
    }

    /// Whether it is there at all.
    #[must_use]
    pub fn has(&self, program: Program) -> bool {
        self.paths[slot(program)].is_some()
    }
}

/// A program's place in [`Tools`].
const fn slot(program: Program) -> usize {
    match program {
        Program::Ffprobe => 0,
        Program::Ffmpeg => 1,
        Program::Exiftool => 2,
        Program::Mkvextract => 3,
        Program::Mkvmerge => 4,
        Program::Mkvpropedit => 5,
    }
}

/// How long a program may take and how much it may say.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    /// The deadline, from the moment it starts.
    pub patience: Duration,
    /// The most bytes of stdout kept.
    pub ceiling: u64,
}

impl Limits {
    /// The readers: ffprobe, exiftool, mkvmerge, mkvextract.
    ///
    /// Ten seconds against a few hundred milliseconds measured for the
    /// inspector's ffprobe and exiftool; a network share is what the margin
    /// is for. Eight mebibytes of JSON is a file with thousands of tags.
    pub const READ: Self = Self {
        patience: Duration::from_secs(10),
        ceiling: 8 * 1024 * 1024,
    };
}

/// How often the clock and the stop flag are looked at.
const TICK: Duration = Duration::from_millis(50);

/// How much of stderr is kept, for the log line a failure writes.
const STDERR_TAIL: usize = 2048;

/// What a program left behind.
#[derive(Debug)]
pub(crate) struct Output {
    /// Its stdout, whole (unless lines were asked for).
    pub stdout: Vec<u8>,
    /// Its exit status.
    pub code: i32,
}

/// Runs a program and gives back what it wrote and how it left.
///
/// A status other than zero is NOT a refusal here, because the Matroska
/// tools leave with 1 for a warning and the work done; each caller says
/// which statuses mean success ([`Output::ok`]). Killed by a signal is a
/// refusal.
///
/// With `lines`, stdout is handed over a line at a time and not kept — the
/// remux's `-progress pipe:1`.
///
/// # Errors
///
/// [`Why::Missing`], [`Why::TimedOut`], [`Why::TooBig`] past the ceiling,
/// [`Why::Cancelled`] when `stop` was raised, and [`Why::ProgramFailed`]
/// when it could not be started or was killed.
pub(crate) fn run(
    tools: &Tools,
    program: Program,
    args: &[OsString],
    limits: Limits,
    stop: Option<&AtomicBool>,
    mut lines: Option<&mut dyn FnMut(&str)>,
) -> Result<Output, Why> {
    let path = tools.path(program)?;
    let began = Instant::now();
    let mut child = Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            tracing::warn!(program = %path.display(), error = %e, "could not start it");
            Why::ProgramFailed { program }
        })?;
    let (Some(mut out), Some(mut err)) = (child.stdout.take(), child.stderr.take()) else {
        stop_child(&mut child);
        return Err(Why::ProgramFailed { program });
    };

    let deadline = began + limits.patience;
    let mut kept: Vec<u8> = Vec::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut tail: Vec<u8> = Vec::new();
    let mut open = [true, true];
    let mut chunk = vec![0_u8; 64 * 1024];
    while open[0] || open[1] {
        if stop.is_some_and(|s| s.load(Ordering::Relaxed)) {
            stop_child(&mut child);
            return Err(Why::Cancelled);
        }
        if Instant::now() >= deadline {
            tracing::warn!(program = %path.display(), "it went past its patience");
            stop_child(&mut child);
            return Err(Why::TimedOut { program });
        }
        let ready = match wait_for(&out, &err, open) {
            Ok(r) => r,
            Err(()) => {
                stop_child(&mut child);
                return Err(Why::ProgramFailed { program });
            }
        };
        if ready[0] {
            match out.read(&mut chunk) {
                Ok(0) => open[0] = false,
                Ok(n) => {
                    if let Some(sink) = lines.as_deref_mut() {
                        pending.extend_from_slice(&chunk[..n]);
                        while let Some(at) = pending.iter().position(|&b| b == b'\n') {
                            let line: Vec<u8> = pending.drain(..=at).collect();
                            sink(String::from_utf8_lossy(&line).trim_end());
                        }
                        // A "line" with no newline for a whole ceiling is
                        // not a line; drop it rather than grow for ever.
                        if pending.len() as u64 > limits.ceiling {
                            pending.clear();
                        }
                    } else {
                        kept.extend_from_slice(&chunk[..n]);
                        if kept.len() as u64 > limits.ceiling {
                            tracing::warn!(program = %path.display(), bytes = kept.len(), "it said too much");
                            stop_child(&mut child);
                            return Err(Why::TooBig {
                                bytes: kept.len() as u64,
                                most_bytes: limits.ceiling,
                            });
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => open[0] = false,
            }
        }
        if ready[1] {
            match err.read(&mut chunk) {
                Ok(0) => open[1] = false,
                Ok(n) => {
                    tail.extend_from_slice(&chunk[..n]);
                    if tail.len() > STDERR_TAIL {
                        tail.drain(..tail.len() - STDERR_TAIL);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => open[1] = false,
            }
        }
    }

    // BOTH PIPES CLOSED, which is not the same as the child having left: a
    // program that closes its outputs and then hangs would block a plain
    // `wait` for ever, so the clock keeps running here too.
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let Some(code) = status.code() else {
                    tracing::warn!(program = %path.display(), "it was killed");
                    return Err(Why::ProgramFailed { program });
                };
                if code != 0 {
                    tracing::debug!(
                        program = %path.display(),
                        code,
                        said = %String::from_utf8_lossy(&tail),
                        "it left with a status"
                    );
                }
                return Ok(Output { stdout: kept, code });
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                stop_child(&mut child);
                return Err(Why::TimedOut { program });
            }
            Err(_) => {
                stop_child(&mut child);
                return Err(Why::ProgramFailed { program });
            }
        }
    }
}

impl Output {
    /// Its stdout, when the status is one the caller calls success.
    ///
    /// # Errors
    ///
    /// [`Why::ProgramFailed`] otherwise.
    pub(crate) fn ok(self, program: Program, success: &[i32]) -> Result<Vec<u8>, Why> {
        if success.contains(&self.code) {
            Ok(self.stdout)
        } else {
            Err(Why::ProgramFailed { program })
        }
    }
}

/// Waits a tick for either pipe, and says which have something (or closed).
fn wait_for(out: &ChildStdout, err: &ChildStderr, open: [bool; 2]) -> Result<[bool; 2], ()> {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    let tick = Timespec {
        tv_sec: 0,
        tv_nsec: i64::try_from(TICK.as_nanos()).unwrap_or(50_000_000),
    };
    let (o, e) = (out.as_fd(), err.as_fd());
    // ONLY THE OPEN ONES: a pipe at its end answers `HUP` at once, every
    // time, and polling it would turn the wait for the other into a spin.
    let mut which: Vec<usize> = Vec::with_capacity(2);
    let mut fds: Vec<PollFd<'_>> = Vec::with_capacity(2);
    if open[0] {
        which.push(0);
        fds.push(PollFd::new(&o, PollFlags::IN));
    }
    if open[1] {
        which.push(1);
        fds.push(PollFd::new(&e, PollFlags::IN));
    }
    match poll(&mut fds, Some(&tick)) {
        Ok(_) => {}
        // A signal is a tick, not a failure.
        Err(rustix::io::Errno::INTR) => return Ok([false, false]),
        Err(_) => return Err(()),
    }
    let wanted = PollFlags::IN | PollFlags::HUP | PollFlags::ERR;
    let mut ready = [false, false];
    for (i, fd) in which.into_iter().zip(&fds) {
        ready[i] = fd.revents().intersects(wanted);
    }
    Ok(ready)
}

/// Kills and reaps, in that order; a child nobody waits for is a zombie in
/// a daemon that lives for months.
fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// A private folder for a program's file answer, gone when dropped.
///
/// `mkvextract` writes its tags to a FILE, and `mkvpropedit` reads new tags
/// and a new cover from one. They are the person's tags and pictures, so
/// the folder is `0700`, in the runtime directory (a per-user tmpfs) when
/// there is one, and removed however the work ends.
pub(crate) struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    /// A new, empty one.
    ///
    /// # Errors
    ///
    /// [`Why::Unwritable`] when no folder could be made.
    pub(crate) fn new() -> Result<Self, Why> {
        use std::os::unix::fs::DirBuilderExt as _;
        use std::sync::atomic::AtomicU64;
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .unwrap_or_else(std::env::temp_dir);
        for _ in 0..16 {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir = base.join(format!("ronoctua-tags-{}-{n}", std::process::id()));
            match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
                Ok(()) => return Ok(Self { dir }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => break,
            }
        }
        Err(Why::Unwritable)
    }

    /// A name inside it.
    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// An argument list out of anything that looks like one.
#[cfg(test)]
pub(crate) fn args<I, S>(items: I) -> Vec<OsString>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    items.into_iter().map(|s| s.as_ref().to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::{Limits, Tools, args, run};
    use crate::sheet::{Program, Why};
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    #[test]
    fn a_program_that_is_not_named_is_refused_by_name_and_nothing_starts() {
        let tools = Tools::none();
        assert_eq!(
            run(&tools, Program::Mkvpropedit, &[], Limits::READ, None, None).unwrap_err(),
            Why::Missing {
                program: Program::Mkvpropedit
            }
        );
    }

    #[test]
    fn every_variable_is_in_the_family() {
        for p in Program::ALL {
            assert!(p.variable().starts_with("RONOCTUA_"), "{p:?}");
        }
        let tools = Tools::none().with(Program::Ffmpeg, PathBuf::from("/x"));
        assert!(tools.has(Program::Ffmpeg));
        assert!(!tools.without(Program::Ffmpeg).has(Program::Ffmpeg));
    }

    /// `/bin/sh` stands in for a program: the runner does not care what it
    /// runs, only how it leaves, and every Linux has one.
    fn shell() -> Option<Tools> {
        let sh = PathBuf::from("/bin/sh");
        sh.exists()
            .then(|| Tools::none().with(Program::Ffprobe, sh))
    }

    #[test]
    fn stdout_is_kept_and_the_status_is_reported_not_judged() {
        let Some(tools) = shell() else { return };
        let out = run(
            &tools,
            Program::Ffprobe,
            &args(["-c", "printf hello; echo oops >&2; exit 1"]),
            Limits::READ,
            None,
            None,
        )
        .expect("it ran");
        assert_eq!(out.stdout, b"hello");
        assert_eq!(out.code, 1);
        assert!(out.ok(Program::Ffprobe, &[0, 1]).is_ok());
    }

    #[test]
    fn a_program_past_its_patience_is_stopped() {
        let Some(tools) = shell() else { return };
        let limits = Limits {
            patience: Duration::from_millis(200),
            ceiling: 1024,
        };
        let began = std::time::Instant::now();
        let why = run(
            &tools,
            Program::Ffprobe,
            &args(["-c", "sleep 5"]),
            limits,
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(
            why,
            Why::TimedOut {
                program: Program::Ffprobe
            }
        );
        assert!(began.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn a_program_that_says_too_much_is_stopped_at_the_ceiling() {
        let Some(tools) = shell() else { return };
        let limits = Limits {
            patience: Duration::from_secs(5),
            ceiling: 1000,
        };
        let why = run(
            &tools,
            Program::Ffprobe,
            &args(["-c", "yes"]),
            limits,
            None,
            None,
        )
        .unwrap_err();
        assert!(
            matches!(
                why,
                Why::TooBig {
                    most_bytes: 1000,
                    ..
                }
            ),
            "{why:?}"
        );
    }

    #[test]
    fn a_raised_flag_stops_it() {
        let Some(tools) = shell() else { return };
        let stop = AtomicBool::new(true);
        let why = run(
            &tools,
            Program::Ffprobe,
            &args(["-c", "sleep 5"]),
            Limits::READ,
            Some(&stop),
            None,
        )
        .unwrap_err();
        assert_eq!(why, Why::Cancelled);
    }

    #[test]
    fn a_scratch_folder_is_private_and_goes_with_its_owner() {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = super::Scratch::new().expect("a folder");
        let inside = scratch.path("a.xml");
        std::fs::write(&inside, b"x").expect("written");
        let dir = inside.parent().expect("a parent").to_path_buf();
        let mode = std::fs::metadata(&dir).expect("there").permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        drop(scratch);
        assert!(!dir.exists());
    }

    #[test]
    fn lines_are_handed_over_one_at_a_time() {
        let Some(tools) = shell() else { return };
        let mut seen: Vec<String> = Vec::new();
        let mut sink = |line: &str| seen.push(line.to_owned());
        let out = run(
            &tools,
            Program::Ffprobe,
            &args(["-c", "printf 'a=1\\nb=2\\n'"]),
            Limits::READ,
            None,
            Some(&mut sink),
        )
        .expect("it ran");
        assert!(out.stdout.is_empty());
        assert_eq!(seen, ["a=1", "b=2"]);
    }
}
