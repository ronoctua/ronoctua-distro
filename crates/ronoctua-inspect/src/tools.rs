//! The external programs, and what happens when one is not there.
//!
//! # Why the path comes from the ENVIRONMENT and not from `option_env!`
//!
//! `option_env!` with a store path is a CLOSURE REFERENCE: the derivation that
//! compiles this crate would cite `ffmpeg-full`, and every binary built out of
//! this workspace would carry it whether it ever runs a preview or not. That is
//! the measured trap `AGENTS.md` records — "a switch that does not switch" —
//! and `nix/modules/media.nix` already went the other way once for
//! `RONOCTUA_SWAPPY`, which is the shape copied here.
//!
//! Reading the variable at runtime also buys the second thing: a test, or a
//! check, can unset it and see the refusal. A path baked in at compile time
//! cannot be taken away.
//!
//! # The refusal is a safety net, not a profile difference
//!
//! ffmpeg, exiftool and poppler are installed on every machine this distro
//! builds. [`Refusal::Missing`] is what a stripped container, a broken
//! environment or a deliberate test produces — and it must stay, because a
//! blank panel and a refused panel look the same and only one of them says
//! why.
//!
//! # Why only ffmpeg
//!
//! It was measured on the owner's machine, not preferred: `magick` **dumped
//! core** on a 17 MB Pixel DNG, while `ffmpeg` gave a 400 px PNG from the same
//! file in 0.21 s. HEIC took 0.09 s and seek-and-frame on a 355 MB mp4 took
//! 0.17 s. So one program covers video, HEIC, RAW and cover art, and a second
//! spawn path would be a second failure mode for formats the first already
//! reads. `libheif`'s `heif-convert` is refused for a different reason: it
//! writes a FILE, which would mean inventing a temporary path, a name that
//! does not collide and a cleanup that survives a crash.

use std::ffi::OsStr;
use std::io::Read as _;
use std::os::fd::AsFd as _;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::Refusal;

/// How long any one child is allowed to take.
///
/// Five seconds against a worst measured case of 0.21 s. The margin is that
/// wide because the number this guards against is not "a big file" — it is a
/// program waiting on something, and a file manager that stops answering for a
/// minute is indistinguishable from one that crashed.
pub const PATIENCE: Duration = Duration::from_secs(5);

/// How often the clock is looked at while the pipe is drained.
///
/// It is the poll's own timeout, so the loop does both jobs: fifty milliseconds
/// is a hundredth of [`PATIENCE`] and costs a hundred syscalls in the worst
/// case.
pub const TICK: Duration = Duration::from_millis(50);

/// The most a child may write to stdout.
///
/// Thirty-two mebibytes is a 512 px PNG a hundred times over. The ceiling is
/// here because a pipe with nobody reading it blocks the child, and a pipe with
/// somebody reading it forever fills this process instead — the second failure
/// is the quiet one.
pub const STDOUT_CEILING: u64 = 32 * 1024 * 1024;

/// A program this crate runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    /// Frames, stills and cover art, all as PNG on stdout.
    Ffmpeg,
    /// Streams and containers, as JSON.
    Ffprobe,
    /// What the camera wrote, as JSON.
    Exiftool,
    /// The first page of a PDF, as PNG on stdout.
    Pdftoppm,
    /// How many pages a PDF has.
    Pdfinfo,
    /// The words of a `.doc`, on stdout.
    ///
    /// # The three come as one package, and two of the three work
    ///
    /// Measured with the packaged binaries, on the owner's own files:
    /// `catdoc sample.doc` answered **8 377 bytes**, `xls2csv sample.xls`
    /// answered **364 057 bytes**, and `catppt sample.ppt` answered **zero
    /// bytes, exit status zero and nothing on stderr** — on a file `file`
    /// calls a real OLE compound document.
    ///
    /// So `.doc` and `.xls` are previewed, and `.ppt` gets the HONEST REFUSAL:
    /// `document::from_a_program` reads an empty answer as a failure, because
    /// a blank panel and a refused one look the same on screen and only one of
    /// them has a sentence.
    Catdoc,
    /// The cells of a `.xls`, as CSV on stdout.
    Xls2csv,
    /// The words of a `.ppt`, on stdout.
    Catppt,
}

impl Tool {
    /// The environment variable that names it.
    #[must_use]
    pub const fn variable(self) -> &'static str {
        match self {
            Self::Ffmpeg => "RONOCTUA_FFMPEG",
            Self::Ffprobe => "RONOCTUA_FFPROBE",
            Self::Exiftool => "RONOCTUA_EXIFTOOL",
            Self::Pdftoppm => "RONOCTUA_PDFTOPPM",
            Self::Pdfinfo => "RONOCTUA_PDFINFO",
            Self::Catdoc => "RONOCTUA_CATDOC",
            Self::Xls2csv => "RONOCTUA_XLS2CSV",
            Self::Catppt => "RONOCTUA_CATPPT",
        }
    }

    /// Where it is, or the refusal that names it.
    ///
    /// # Errors
    ///
    /// [`Refusal::Missing`] when the variable is unset or empty. An empty
    /// value is treated as unset on purpose: `FOO=` in a unit file is how one
    /// turns something off, and reading it as a program called "" would fail
    /// later and further away.
    pub fn path(self) -> Result<PathBuf, Refusal> {
        match std::env::var_os(self.variable()) {
            Some(v) if !v.is_empty() => Ok(PathBuf::from(v)),
            _ => Err(Refusal::Missing(self.variable())),
        }
    }

    /// Whether it is there at all — for a screen that wants to grey a row out
    /// rather than offer it and then refuse. The format buffer already draws
    /// the XFS row that way, and for the same reason.
    #[must_use]
    pub fn there(self) -> bool {
        self.path().is_ok()
    }

    /// Every tool, so a check can walk them.
    pub const ALL: [Self; 8] = [
        Self::Ffmpeg,
        Self::Ffprobe,
        Self::Exiftool,
        Self::Pdftoppm,
        Self::Pdfinfo,
        Self::Catdoc,
        Self::Xls2csv,
        Self::Catppt,
    ];
}

/// What a program says about a file, as text.
///
/// The three OLE readers all take one argument and write to stdout, which is
/// what every other tool of this crate does — so this is [`run`] with the
/// bytes turned into a string, and not a second way of starting a program.
///
/// # Errors
///
/// Whatever [`run`] refuses with, plus [`Refusal::NotText`] when what came
/// back is not UTF-8: `catdoc` has a `-d` for the output encoding and the
/// default is the locale's, so a file in another codepage comes back as bytes
/// nobody can draw. That is a sentence, not a panel of replacement characters
/// — the same choice `text::of` makes.
pub fn text_of(tool: Tool, path: &std::path::Path) -> Result<String, Refusal> {
    let out = run(tool, &[path.as_os_str()])?;
    String::from_utf8(out).map_err(|_| Refusal::NotText)
}

/// The program's name, out of the variable that names it.
///
/// A RULE and not a table, so a tool added tomorrow cannot be added to one and
/// forgotten in the other: strip the prefix, lowercase the rest. It exists
/// because "RONOCTUA_FFMPEG is not set" is a sentence about our build system,
/// and "ffmpeg is not installed" is a sentence about the machine — only the
/// second one is something a person can act on.
#[must_use]
pub fn program_of(variable: &str) -> String {
    variable
        .strip_prefix("RONOCTUA_")
        .unwrap_or(variable)
        .to_lowercase()
}

/// Runs a tool and gives back everything it wrote to stdout.
///
/// # Why one loop drains the pipe AND watches the clock
///
/// A pipe holds 64 KiB. A PNG does not fit, so a loop that only polled
/// `try_wait` would watch a child that is blocked writing, decide after five
/// seconds that it hung, and kill the one thing that was working. Draining it
/// on a second thread would work too, and is what most code does — but this
/// crate promises to spawn no threads, because the shell owns concurrency and
/// a crate that quietly starts its own is how two schedulers end up disagreeing.
///
/// `poll` with [`TICK`] as its timeout does both: it returns when there are
/// bytes, and it returns when the tick is up.
///
/// stderr goes to `/dev/null`. Every command line here already carries
/// `-v error`, and a second pipe would be a second thing that can fill and
/// block — the exit status is what says whether it worked.
///
/// # Errors
///
/// [`Refusal::Missing`] when the tool is not named, [`Refusal::TimedOut`] past
/// [`PATIENCE`], [`Refusal::TooBig`] past [`STDOUT_CEILING`], and
/// [`Refusal::Failed`] when it could not be started or exited badly.
pub fn run(tool: Tool, args: &[&OsStr]) -> Result<Vec<u8>, Refusal> {
    let program = tool.path()?;
    let began = Instant::now();
    let mut child = Command::new(&program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            tracing::warn!(program = %program.display(), error = %e, "could not start it");
            Refusal::Failed
        })?;
    let Some(mut pipe) = child.stdout.take() else {
        // Asked for, so it is there; refusing rather than unwrapping keeps a
        // surprise from becoming a panic inside the file manager.
        stop(&mut child);
        return Err(Refusal::Failed);
    };

    let deadline = began + PATIENCE;
    let tick = rustix::event::Timespec {
        tv_sec: 0,
        tv_nsec: i64::try_from(TICK.as_nanos()).unwrap_or(50_000_000),
    };
    let mut out: Vec<u8> = Vec::new();
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        if Instant::now() >= deadline {
            tracing::warn!(program = %program.display(), "it went past the patience");
            stop(&mut child);
            return Err(Refusal::TimedOut);
        }
        // The borrow of the pipe ends with this block, so the read below can
        // take it mutably. Building the `PollFd` fresh each turn costs nothing
        // and is what keeps the two borrows from overlapping.
        let ready = {
            let fd = pipe.as_fd();
            let mut fds = [rustix::event::PollFd::new(
                &fd,
                rustix::event::PollFlags::IN,
            )];
            match rustix::event::poll(&mut fds, Some(&tick)) {
                Ok(_) => fds[0].revents(),
                // A signal is a tick, not a failure.
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => {
                    tracing::warn!(program = %program.display(), error = %e, "the poll failed");
                    stop(&mut child);
                    return Err(Refusal::Failed);
                }
            }
        };
        if !ready.intersects(
            rustix::event::PollFlags::IN
                | rustix::event::PollFlags::HUP
                | rustix::event::PollFlags::ERR,
        ) {
            continue;
        }
        match pipe.read(&mut chunk) {
            // END OF FILE, which is the only orderly way out of this loop.
            Ok(0) => break,
            Ok(n) => {
                out.extend_from_slice(&chunk[..n]);
                if out.len() as u64 > STDOUT_CEILING {
                    tracing::warn!(program = %program.display(), bytes = out.len(), "it went over the ceiling");
                    stop(&mut child);
                    return Err(Refusal::TooBig(out.len() as u64));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => {
                tracing::warn!(program = %program.display(), error = %e, "the reading failed");
                stop(&mut child);
                return Err(Refusal::Failed);
            }
        }
    }

    // THE PIPE CLOSED, WHICH IS NOT THE SAME AS THE CHILD HAVING LEFT. A
    // program that closes stdout and then hangs would block a plain `wait`
    // for ever, so the clock keeps running here too.
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    tracing::debug!(
                        program = %program.display(),
                        code = status.code().unwrap_or(-1),
                        bytes = out.len(),
                        "it came out badly"
                    );
                    return Err(Refusal::Failed);
                }
                tracing::debug!(
                    program = %program.display(),
                    bytes = out.len(),
                    ms = began.elapsed().as_millis(),
                    "the external program finished"
                );
                return Ok(out);
            }
            Ok(None) if Instant::now() < deadline => std::thread::yield_now(),
            Ok(None) => {
                stop(&mut child);
                return Err(Refusal::TimedOut);
            }
            Err(e) => {
                tracing::warn!(program = %program.display(), error = %e, "the wait failed");
                stop(&mut child);
                return Err(Refusal::Failed);
            }
        }
    }
}

/// Kills and reaps, in that order.
///
/// The reap is not optional: a child nobody waits for is a zombie, and the
/// footer is a long-lived process that would collect one per refused preview.
fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::{PATIENCE, STDOUT_CEILING, Tool, program_of, run};
    use crate::Refusal;
    use std::ffi::OsStr;

    #[test]
    fn every_variable_yields_the_program_it_names() {
        // The rule, applied to every tool there is. A tool whose variable did
        // not follow the shape would come out with the prefix still on it, and
        // the sentence on screen would name our build system.
        let expected = ["ffmpeg", "ffprobe", "exiftool", "pdftoppm", "pdfinfo"];
        for (tool, want) in Tool::ALL.into_iter().zip(expected) {
            assert!(
                tool.variable().starts_with("RONOCTUA_"),
                "{tool:?} escapes the prefix"
            );
            assert_eq!(program_of(tool.variable()), want, "{tool:?}");
        }
    }

    #[test]
    fn a_tool_that_is_not_named_refuses_by_name_and_starts_nothing() {
        // Whichever machine this runs on, one of the two halves is true, and
        // both are worth asserting. `std::env::set_var` is `unsafe` from the
        // 2024 edition and this crate forbids `unsafe`, so the test asserts
        // the state the machine is in rather than arranging one.
        let ffmpeg = Tool::Ffmpeg;
        if ffmpeg.there() {
            // THE PATH IS WHAT THE VARIABLE SAID, never a name looked up on
            // `PATH` — the trap `AGENTS.md` records as "spawn in a systemd
            // unit: use the path, never the name".
            let named = std::env::var_os(ffmpeg.variable()).expect("the variable is set");
            assert_eq!(ffmpeg.path().expect("there is a path").as_os_str(), named);
        } else {
            // NO PROCESS IS STARTED: the refusal comes back before the spawn,
            // which is what lets the whole suite run with none of the
            // programs installed.
            assert_eq!(
                ffmpeg.path().unwrap_err(),
                Refusal::Missing("RONOCTUA_FFMPEG")
            );
            assert_eq!(
                run(ffmpeg, &[OsStr::new("-version")]).unwrap_err(),
                Refusal::Missing("RONOCTUA_FFMPEG")
            );
        }
    }

    #[test]
    fn a_variable_outside_the_family_still_yields_a_name() {
        // The rule has no table behind it, so it answers for anything.
        assert_eq!(program_of("RONOCTUA_NOTHING_AT_ALL"), "nothing_at_all");
        assert_eq!(program_of("PDFINFO"), "pdfinfo");
    }

    #[test]
    fn the_ceilings_are_stated_and_not_guessed() {
        assert_eq!(STDOUT_CEILING, 32 * 1024 * 1024);
        assert_eq!(PATIENCE.as_secs(), 5);
    }
}
