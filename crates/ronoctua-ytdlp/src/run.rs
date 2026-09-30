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

use std::io::Read as _;
use std::path::Path;
use std::process::{Child, Command, Stdio};
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
    let until = Instant::now() + deadline;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        // NOT READ, AND NOT PIPED: a pipe nobody drains fills at 64 KiB and
        // stalls the writer, and what yt-dlp says there is for a person at a
        // terminal. The exit status is the answer this needs.
        .stderr(Stdio::null());
    let mut held = Held {
        child: command.spawn().map_err(|_| Trouble::Unstartable)?,
        ended: false,
    };
    let Some(mut out) = held.child.stdout.take() else {
        return Err(Trouble::Unstartable);
    };

    let (tell, told) = mpsc::channel::<std::io::Result<Vec<u8>>>();
    // ONE MORE BYTE THAN ALLOWED is read, so "exactly the ceiling" and "more
    // than it" are told apart without reading the rest.
    let cap = u64::try_from(most_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let reader = std::thread::Builder::new()
        .name("yt-dlp-out".to_owned())
        .spawn(move || {
            let mut bytes = Vec::new();
            let read = (&mut out).take(cap).read_to_end(&mut bytes).map(|_| bytes);
            let _ = tell.send(read);
        })
        .map_err(|_| Trouble::Unstartable)?;

    let left = until.saturating_duration_since(Instant::now());
    let bytes = match told.recv_timeout(left) {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(_)) => return Err(Trouble::Failed),
        // PAST THE DEADLINE: `held` kills the child on the way out, which
        // closes the pipe and ends the reader.
        Err(_) => return Err(Trouble::Slow),
    };
    let _ = reader.join();
    if bytes.len() > most_bytes {
        return Err(Trouble::TooLarge);
    }

    loop {
        match held.child.try_wait() {
            Ok(Some(status)) => {
                held.ended = true;
                return if status.success() {
                    Ok(bytes)
                } else {
                    Err(Trouble::Failed)
                };
            }
            Ok(None) if Instant::now() < until => std::thread::sleep(LOOK),
            Ok(None) => return Err(Trouble::Slow),
            Err(_) => return Err(Trouble::Failed),
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
