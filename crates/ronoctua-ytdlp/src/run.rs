//! One run of yt-dlp that answers JSON: bounded in time and in bytes, and
//! never left behind.
//!
//! # Why a thread reads, and the caller only waits
//!
//! Reading a pipe BLOCKS, and a deadline has to be answerable while it does.
//! So one short-lived thread reads standard output to its end and hands the
//! bytes over a channel; the caller waits on the channel with the time that
//! is left. When the deadline passes first, the child is killed — which closes
//! the pipe, so the reader ends too and nothing is left running.
//!
//! # Kill on drop
//!
//! The child lives inside [`Held`], whose `Drop` kills and reaps it unless it
//! already ended. Every way out of [`json`] — an answer, a deadline, a panic
//! further up — passes through that `Drop`, so a search the person gave up on
//! is never a yt-dlp still talking to YouTube a minute later.

use std::io::{BufRead as _, Read as _};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::Trouble;

/// A child that is killed and reaped when this is dropped, unless it ended.
struct Held {
    child: Child,
    ended: bool,
}

impl Drop for Held {
    fn drop(&mut self) {
        if !self.ended {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// How long the wait for the exit status may sleep between two looks, once
/// standard output has ended.
///
/// yt-dlp closes its output by exiting, so the first look nearly always
/// answers; this bounds the pathological case of a process that closed the
/// pipe and kept running, without a clock armed for the whole run.
const LOOK: Duration = Duration::from_millis(5);

/// Runs `program` with `args` and gives back what it wrote on standard output,
/// when it ended well inside `deadline` and wrote at most `most_bytes`.
///
/// Blocks for up to `deadline`: call it on a thread, never on a drawing one.
///
/// # Errors
///
/// [`Trouble::Unstartable`] when the program cannot be run,
/// [`Trouble::Slow`] past the deadline (it is killed), [`Trouble::TooLarge`]
/// past the ceiling (killed too), and [`Trouble::Failed`] when it exited
/// with a failure.
pub(crate) fn json(
    program: &Path,
    args: &[String],
    deadline: Duration,
    most_bytes: usize,
) -> Result<Vec<u8>, Trouble> {
    ran(program, args, deadline, most_bytes, false, None)
        .map(|(bytes, _)| bytes)
        .map_err(|(trouble, _)| trouble)
}

/// [`json`], that also ends — the child killed, its pipes closed — the
/// moment `stop` is set: `None` then, an answer nobody waits for any more.
///
/// The wait for the answer looks at `stop` every [`STOP_LOOK`], so a search
/// the person typed past ends within that, and never runs on to its deadline
/// talking to YouTube behind a search box that moved on.
pub(crate) fn json_until(
    program: &Path,
    args: &[String],
    deadline: Duration,
    most_bytes: usize,
    stop: &AtomicBool,
) -> Option<Result<Vec<u8>, Trouble>> {
    if stop.load(Ordering::SeqCst) {
        return None;
    }
    let ran = ran(program, args, deadline, most_bytes, false, Some(stop));
    if stop.load(Ordering::SeqCst) {
        return None;
    }
    Some(ran.map(|(bytes, _)| bytes).map_err(|(trouble, _)| trouble))
}

/// How often a stoppable run looks at its stop while it waits.
pub(crate) const STOP_LOOK: Duration = Duration::from_millis(25);

/// [`json`], and what yt-dlp said on standard error: on a failure its last
/// `ERROR:` line — the one place it says WHY (a bot check, a 429, a private
/// video), which a caller that stops a whole run on some of them has to read
/// — and on a success its last [`WARNINGS_MOST`] `WARNING:` lines.
///
/// # Why a success says its warnings
///
/// With `--ignore-no-formats-error` yt-dlp answers the fields of a video it
/// cannot play and says WHY it cannot as a warning, exiting 0: a bot check,
/// a rate limit, a private or removed video all end as a success with an
/// answer. A caller that passes that flag reads the warnings, and must not
/// pass `--no-warnings` with it ([`crate::youtube::details`]).
///
/// Standard error is drained to its end by a second short-lived thread, so a
/// chatty run never stalls on a full pipe; each kept line is cut at
/// [`SAID_MOST`] characters.
///
/// # Errors
///
/// As [`json`], each with the error line when there was one.
pub(crate) fn json_telling(
    program: &Path,
    args: &[String],
    deadline: Duration,
    most_bytes: usize,
) -> Result<Answered, Told> {
    ran(program, args, deadline, most_bytes, true, None)
}

/// What a run that succeeded gives back: its standard output, and the
/// warnings it said on the way.
pub(crate) type Answered = (Vec<u8>, Vec<String>);

/// What a run that failed gives back: the trouble, and its last error line.
pub(crate) type Told = (Trouble, Option<String>);

/// The longest line [`json_telling`] keeps, in characters.
pub(crate) const SAID_MOST: usize = 400;

/// How many `WARNING:` lines [`json_telling`] keeps, the last ones: a
/// details run says a handful; a ceiling keeps a chatty one bounded.
pub(crate) const WARNINGS_MOST: usize = 64;

/// The line from yt-dlp's `mark` on (`ERROR:`, `WARNING:`), trimmed and cut at
/// [`SAID_MOST`] characters, when `line` carries the mark. Pure.
fn marked(line: &str, mark: &str) -> Option<String> {
    let at = line.find(mark)?;
    let line = line[at..].trim();
    (!line.is_empty()).then(|| line.chars().take(SAID_MOST).collect())
}

/// The longest piece of one line the error reader holds at once, in bytes: a
/// line longer than this is read in pieces, so what a run writes on standard
/// error costs a bounded buffer however much it writes.
const LINE_MOST: u64 = 16 * 1024;

/// What a run said on standard error, as far as a caller reads it.
#[derive(Debug, Default, PartialEq, Eq)]
struct Heard {
    /// The last `ERROR:` line.
    error: Option<String>,
    /// The last [`WARNINGS_MOST`] `WARNING:` lines, oldest first.
    warnings: std::collections::VecDeque<String>,
}

/// Reads `err` to its end, one line (or [`LINE_MOST`] bytes of one) at a
/// time, and keeps the last `ERROR:` line and the last `WARNING:` lines.
fn heard_of(err: impl std::io::Read) -> Heard {
    let mut reader = std::io::BufReader::new(err);
    let mut piece = Vec::new();
    let mut heard = Heard::default();
    loop {
        piece.clear();
        match (&mut reader).take(LINE_MOST).read_until(b'\n', &mut piece) {
            Ok(0) | Err(_) => return heard,
            Ok(_) => {
                let line = String::from_utf8_lossy(&piece);
                if let Some(error) = marked(&line, "ERROR:") {
                    heard.error = Some(error);
                } else if let Some(warning) = marked(&line, "WARNING:") {
                    if heard.warnings.len() == WARNINGS_MOST {
                        heard.warnings.pop_front();
                    }
                    heard.warnings.push_back(warning);
                }
            }
        }
    }
}

fn ran(
    program: &Path,
    args: &[String],
    deadline: Duration,
    most_bytes: usize,
    listen: bool,
    stop: Option<&AtomicBool>,
) -> Result<Answered, Told> {
    let stopped = || stop.is_some_and(|s| s.load(Ordering::SeqCst));
    let until = Instant::now() + deadline;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        // NOT READ, AND NOT PIPED, unless the caller listens: a pipe nobody
        // drains fills at 64 KiB and stalls the writer, and what yt-dlp says
        // there is for a person at a terminal — except its `ERROR:` line.
        .stderr(if listen {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    let mut held = Held {
        child: command.spawn().map_err(|_| (Trouble::Unstartable, None))?,
        ended: false,
    };
    let Some(mut out) = held.child.stdout.take() else {
        return Err((Trouble::Unstartable, None));
    };
    // WHAT IT SAYS, read to the end of the pipe on its own thread; joined
    // only once the child has ended or been killed, which closes the pipe.
    let said = held.child.stderr.take().and_then(|err| {
        std::thread::Builder::new()
            .name("yt-dlp-err".to_owned())
            .spawn(move || heard_of(err))
            .ok()
    });
    let heard = |held: &mut Held| -> Heard {
        // Killed first when it is still running, so the pipe closes and the
        // reader ends; then what it said.
        if !held.ended {
            let _ = held.child.kill();
            let _ = held.child.wait();
            held.ended = true;
        }
        said.and_then(|reader| reader.join().ok())
            .unwrap_or_default()
    };

    let (tell, told) = mpsc::channel::<std::io::Result<Vec<u8>>>();
    // ONE MORE BYTE THAN ALLOWED is read, so "exactly the ceiling" and "more
    // than it" are told apart without reading the rest.
    let cap = u64::try_from(most_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let Ok(reader) = std::thread::Builder::new()
        .name("yt-dlp-out".to_owned())
        .spawn(move || {
            let mut bytes = Vec::new();
            let read = (&mut out).take(cap).read_to_end(&mut bytes).map(|_| bytes);
            let _ = tell.send(read);
        })
    else {
        return Err((Trouble::Unstartable, heard(&mut held).error));
    };

    // IN SLICES when a stop is held, so a stop is heard within one: the
    // child is killed (`heard`), which closes the pipe and ends the reader.
    let bytes = loop {
        let left = until.saturating_duration_since(Instant::now());
        let slice = if stop.is_some() {
            left.min(STOP_LOOK)
        } else {
            left
        };
        match told.recv_timeout(slice) {
            Ok(Ok(bytes)) => break bytes,
            Ok(Err(_)) => return Err((Trouble::Failed, heard(&mut held).error)),
            Err(_) if stopped() => return Err((Trouble::Failed, heard(&mut held).error)),
            Err(_) if Instant::now() < until => {}
            // PAST THE DEADLINE: the child is killed, which closes the pipe
            // and ends the reader.
            Err(_) => return Err((Trouble::Slow, heard(&mut held).error)),
        }
    };
    let _ = reader.join();
    if bytes.len() > most_bytes {
        return Err((Trouble::TooLarge, heard(&mut held).error));
    }

    loop {
        match held.child.try_wait() {
            Ok(Some(status)) => {
                held.ended = true;
                return if status.success() {
                    // What a run that SUCCEEDED said on standard error is its
                    // warnings — the caller's to read (see `json_telling`).
                    Ok((bytes, heard(&mut held).warnings.into()))
                } else {
                    Err((Trouble::Failed, heard(&mut held).error))
                };
            }
            Ok(None) if stopped() => return Err((Trouble::Failed, heard(&mut held).error)),
            Ok(None) if Instant::now() < until => std::thread::sleep(LOOK),
            Ok(None) => return Err((Trouble::Slow, heard(&mut held).error)),
            Err(_) => return Err((Trouble::Failed, heard(&mut held).error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `/bin/sh -c script` — there on every machine and in the build sandbox,
    /// and a test that skipped itself without it would pass having proved
    /// nothing.
    fn sh(script: &str) -> (std::path::PathBuf, Vec<String>) {
        (
            std::path::PathBuf::from("/bin/sh"),
            vec!["-c".to_owned(), script.to_owned()],
        )
    }

    #[test]
    fn what_it_writes_is_the_answer() {
        let (p, a) = sh("printf '{\"a\":1}'");
        assert_eq!(
            json(&p, &a, Duration::from_secs(5), 64).unwrap(),
            b"{\"a\":1}"
        );
    }

    #[test]
    fn a_failure_is_said_even_with_output() {
        let (p, a) = sh("printf '{}'; exit 1");
        assert_eq!(
            json(&p, &a, Duration::from_secs(5), 64),
            Err(Trouble::Failed)
        );
    }

    #[test]
    fn past_the_deadline_it_is_stopped_and_said() {
        // A BUILTIN LOOP and not `sleep`: a build sandbox's PATH need not have
        // coreutils, and a missing `sleep` would end at once and say `Failed`.
        let (p, a) = sh("while :; do :; done");
        let started = Instant::now();
        assert_eq!(
            json(&p, &a, Duration::from_millis(200), 64),
            Err(Trouble::Slow)
        );
        // KILLED, and not waited for: a loop that never ends came back in
        // well under a few seconds.
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn more_than_the_ceiling_is_refused() {
        let (p, a) = sh("printf '0123456789'");
        assert_eq!(
            json(&p, &a, Duration::from_secs(5), 9),
            Err(Trouble::TooLarge)
        );
        assert_eq!(
            json(&p, &a, Duration::from_secs(5), 10).unwrap().len(),
            10,
            "exactly the ceiling is allowed"
        );
    }

    #[test]
    fn a_failure_says_its_last_error_line() {
        let (p, a) = sh(
            "printf 'WARNING: slow\\nERROR: [youtube] x: first\\n' >&2; \
             printf 'ERROR: [youtube] abc: Sign in to confirm you are not a bot\\n' >&2; exit 1",
        );
        assert_eq!(
            json_telling(&p, &a, Duration::from_secs(5), 64),
            Err((
                Trouble::Failed,
                Some("ERROR: [youtube] abc: Sign in to confirm you are not a bot".to_owned())
            ))
        );
        let (p, a) = sh("exit 2");
        assert_eq!(
            json_telling(&p, &a, Duration::from_secs(5), 64),
            Err((Trouble::Failed, None)),
            "silence is no line"
        );
        let (p, a) = sh("printf 'ERROR: ignored\\n' >&2; printf '{}'");
        assert_eq!(
            json_telling(&p, &a, Duration::from_secs(5), 64).unwrap(),
            (b"{}".to_vec(), Vec::new()),
            "a success is its output, whatever error it said"
        );
    }

    #[test]
    fn a_success_says_its_warnings() {
        // What `--ignore-no-formats-error` turns a private video into: its
        // fields on standard output, WHY it cannot play as a warning, exit 0.
        let (p, a) = sh("printf '[youtube] abc: Downloading webpage\\n' >&2; \
             printf 'WARNING: [youtube] Private video. Sign in if granted access\\n' >&2; \
             printf 'WARNING: Extractor failed to obtain \"title\"\\n' >&2; \
             printf '{\"id\":\"abc\"}'");
        assert_eq!(
            json_telling(&p, &a, Duration::from_secs(5), 64).unwrap(),
            (
                b"{\"id\":\"abc\"}".to_vec(),
                vec![
                    "WARNING: [youtube] Private video. Sign in if granted access".to_owned(),
                    "WARNING: Extractor failed to obtain \"title\"".to_owned(),
                ]
            )
        );
    }

    #[test]
    fn only_the_last_warnings_are_kept() {
        let mut said = String::new();
        for i in 0..WARNINGS_MOST + 5 {
            said.push_str(&format!("WARNING: {i}\n"));
        }
        let heard = heard_of(said.as_bytes());
        assert_eq!(heard.warnings.len(), WARNINGS_MOST);
        assert_eq!(
            heard.warnings.front().map(String::as_str),
            Some("WARNING: 5")
        );
        assert_eq!(heard.error, None);
    }

    #[test]
    fn a_chatty_run_never_stalls_on_its_error_pipe() {
        // More than a pipe's 64 KiB on standard error before the answer: read
        // by its own thread, so the child is never blocked writing it.
        let (p, a) = sh(
            "i=0; while [ $i -lt 3000 ]; do printf 'WARNING: %064d\\n' $i >&2; i=$((i+1)); done; \
             printf '{\"a\":1}'",
        );
        let (bytes, warnings) = json_telling(&p, &a, Duration::from_secs(20), 64).unwrap();
        assert_eq!(bytes, b"{\"a\":1}");
        assert_eq!(warnings.len(), WARNINGS_MOST, "the last ones, bounded");
    }

    #[test]
    fn the_last_error_line_is_cut_and_starts_at_its_mark() {
        let heard = heard_of("x\n[debug] ERROR: one\nERROR: two  \n".as_bytes());
        assert_eq!(heard.error, Some("ERROR: two".to_owned()));
        let heard = heard_of("WARNING: only  \n".as_bytes());
        assert_eq!(heard.error, None);
        assert_eq!(heard.warnings, ["WARNING: only"]);
        let long = format!("ERROR: {}", "a".repeat(SAID_MOST * 2));
        assert_eq!(
            heard_of(long.as_bytes()).error.unwrap().chars().count(),
            SAID_MOST
        );
    }

    /// FAILS IF: a stopped run goes on to its deadline, or answers as if
    /// somebody still waited for it.
    #[test]
    fn a_stopped_run_ends_within_a_look_and_answers_nothing() {
        let (p, a) = sh("while :; do :; done");
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let stopper = std::sync::Arc::clone(&stop);
        let started = Instant::now();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            stopper.store(true, Ordering::SeqCst);
        });
        assert_eq!(json_until(&p, &a, Duration::from_secs(30), 64, &stop), None);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
        thread.join().unwrap();
        assert_eq!(
            json_until(&p, &a, Duration::from_secs(30), 64, &stop),
            None,
            "already stopped"
        );
        let (p, a) = sh("printf '{}'");
        let go = AtomicBool::new(false);
        assert_eq!(
            json_until(&p, &a, Duration::from_secs(5), 64, &go),
            Some(Ok(b"{}".to_vec()))
        );
    }

    #[test]
    fn a_program_that_is_not_there_is_unstartable() {
        assert_eq!(
            json(
                Path::new("/nonexistent/ronoctua/yt-dlp"),
                &[],
                Duration::from_secs(1),
                64
            ),
            Err(Trouble::Unstartable)
        );
    }
}
