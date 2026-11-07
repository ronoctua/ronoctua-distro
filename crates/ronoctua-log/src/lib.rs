//! The log of our programs: `tracing` on standard error, and what a line is
//! worth when that stream is the journal.
//!
//! # A line on a stream is `info`
//!
//! systemd points a unit's standard error at a stream of journald, and
//! journald files every line of a stream at the unit's default priority unless
//! the line opens with `<N>`. So an `error!` of ours reached the journal as
//! `info`, with its colours as escapes inside `MESSAGE`, and no page that asks
//! for errors ever heard of it.
//!
//! When standard error IS that stream, [`layer`] opens every line with the
//! level's `<N>` and leaves out what the journal already keeps: the time
//! (`__REALTIME_TIMESTAMP`), the level (`PRIORITY`) and the colours, which it
//! has nowhere to show. Anywhere else a program writes the bytes it always
//! wrote, `NO_COLOR` included: nothing here says `with_ansi(true)`.
//!
//! # The variable proves nothing by itself
//!
//! `JOURNAL_STREAM` is how systemd tells a unit, as `dev:ino`, and it is
//! inherited: a shell started by a unit hands it to whatever runs on its pty.
//! So [`Stderr::detect`] compares the two numbers with what standard error
//! really is.

use std::fs::File;
use std::io;
use std::os::fd::AsFd as _;
use std::os::unix::fs::MetadataExt as _;

use tracing::{Level, Metadata, Subscriber};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer};

/// Standard error, and whether it is the journal. The writer of every log,
/// handed to a builder of its own where a program needs one.
#[derive(Debug, Clone, Copy)]
pub struct Stderr {
    journal: bool,
}

impl Stderr {
    /// What standard error is right now. Without `JOURNAL_STREAM`, or with one
    /// that names another file, it is not the journal, and a descriptor that
    /// cannot be asked is not either.
    #[must_use]
    pub fn detect() -> Self {
        let stream = std::env::var("JOURNAL_STREAM").ok();
        let journal = identity().is_ok_and(|(dev, ino)| matches(stream.as_deref(), dev, ino));
        Self { journal }
    }

    /// Whether a line written here lands in the journal.
    #[must_use]
    pub const fn is_the_journal(self) -> bool {
        self.journal
    }
}

/// One event on its way to standard error.
///
/// `fmt::Layer` hands a whole event to one `write_all`, so this sees all of
/// its lines at once: a multi-line event keeps its level on every line, and
/// stays in one `write`.
#[derive(Debug)]
pub struct Line {
    priority: Option<u8>,
}

impl io::Write for Line {
    fn write(&mut self, event: &[u8]) -> io::Result<usize> {
        let Some(priority) = self.priority else {
            return io::stderr().write(event);
        };
        io::stderr().write_all(&prefixed(priority, event))?;
        Ok(event.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stderr().flush()
    }
}

impl<'a> MakeWriter<'a> for Stderr {
    type Writer = Line;

    fn make_writer(&'a self) -> Line {
        Line { priority: None }
    }

    fn make_writer_for(&'a self, meta: &Metadata<'_>) -> Line {
        Line {
            priority: self.journal.then(|| priority_of(meta.level())),
        }
    }
}

/// The formatting layer of a program with a registry of its own.
///
/// Boxed because `without_time` changes the layer's type. The journal's
/// branch keeps the target, which is the one part of a line it has no field
/// for.
#[must_use]
pub fn layer<S>() -> Box<dyn Layer<S> + Send + Sync>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let stderr = Stderr::detect();
    let layer = tracing_subscriber::fmt::layer().with_writer(stderr);
    if stderr.is_the_journal() {
        layer
            .with_ansi(false)
            .without_time()
            .with_level(false)
            .boxed()
    } else {
        layer.boxed()
    }
}

/// Installs the log of a program: `RUST_LOG` when it parses, `default`
/// otherwise, through [`layer`].
pub fn init(default: &str) {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| default.into()))
        .with(layer())
        .init();
}

/// The device and inode of standard error, from the descriptor: no `/proc`,
/// and no `unsafe`.
fn identity() -> io::Result<(u64, u64)> {
    let meta = File::from(io::stderr().as_fd().try_clone_to_owned()?).metadata()?;
    Ok((meta.dev(), meta.ino()))
}

/// Whether `stream`, a value of `JOURNAL_STREAM`, names this device and inode.
fn matches(stream: Option<&str>, dev: u64, ino: u64) -> bool {
    stream
        .and_then(|stream| stream.split_once(':'))
        .is_some_and(|(d, i)| d.parse() == Ok(dev) && i.parse() == Ok(ino))
}

/// The syslog priority of a level: `sd-daemon.h`'s `<N>`.
const fn priority_of(level: &Level) -> u8 {
    match *level {
        Level::ERROR => 3,
        Level::WARN => 4,
        Level::INFO => 6,
        Level::DEBUG | Level::TRACE => 7,
    }
}

/// `event` with `<priority>` opening every line that has something on it:
/// journald reads the prefix line by line, and an empty line has no level to
/// carry.
fn prefixed(priority: u8, event: &[u8]) -> Vec<u8> {
    let prefix = format!("<{priority}>");
    let mut out = Vec::with_capacity(event.len() + prefix.len());
    for line in event.split_inclusive(|byte| *byte == b'\n') {
        if line != b"\n" {
            out.extend_from_slice(prefix.as_bytes());
        }
        out.extend_from_slice(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Set for [`the_child`]: it builds the log the way every program did
    /// before this crate, to measure [`init`] against.
    const BEFORE: &str = "LOG_TEST_BEFORE";

    /// What [`the_child`] logs, when standard error is the journal.
    const ON_THE_JOURNAL: &[u8] = b"<6>ronoctua_log::tests: one line count=3\n\
          <4>ronoctua_log::tests: first line\n\
          \n\
          <4>third line\n\
          <3>ronoctua_log::tests: the last\n";

    /// Standard error of [`the_child`] run on a file of its own, with
    /// `stream` of that file for `JOURNAL_STREAM`.
    fn run_the_child(
        stream: impl FnOnce(&std::fs::Metadata) -> String,
        before: bool,
        no_color: bool,
    ) -> Vec<u8> {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "ronoctua-log-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let file = File::create(&path).expect("a file for its standard error");
        let stream = stream(&file.metadata().expect("the file's numbers"));
        let mut child = Command::new(std::env::current_exe().expect("this binary"));
        child
            .args(["tests::the_child", "--exact", "--ignored", "--nocapture"])
            .env("JOURNAL_STREAM", stream)
            .env_remove("RUST_LOG")
            .env_remove("NO_COLOR")
            .env_remove(BEFORE)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(file);
        if before {
            child.env(BEFORE, "1");
        }
        if no_color {
            child.env("NO_COLOR", "1");
        }
        let status = child.status().expect("the child ran");
        let log = std::fs::read(&path).expect("what it wrote");
        std::fs::remove_file(&path).expect("no file left behind");
        assert!(status.success(), "{}", String::from_utf8_lossy(&log));
        log
    }

    /// `log` without the clock, the one thing two runs differ in: it opens
    /// the line that has one, and it ends at the first `Z`.
    fn timeless(log: &[u8]) -> Vec<u8> {
        log.split_inclusive(|byte| *byte == b'\n')
            .flat_map(|line| {
                let clock = line.iter().position(|byte| *byte == b'Z');
                &line[clock.map_or(0, |end| end + 1)..]
            })
            .copied()
            .collect()
    }

    fn the_true_stream(meta: &std::fs::Metadata) -> String {
        format!("{}:{}", meta.dev(), meta.ino())
    }

    fn another_stream(meta: &std::fs::Metadata) -> String {
        format!("{}:{}", meta.dev(), meta.ino() + 1)
    }

    #[test]
    #[ignore = "the child of the tests below: its standard error is what they read"]
    fn the_child() {
        if std::env::var_os(BEFORE).is_some() {
            tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "ronoctua_log=info".into()),
                )
                .with_writer(std::io::stderr)
                .init();
        } else {
            init("ronoctua_log=info");
        }
        tracing::debug!("under the filter");
        tracing::info!(count = 3, "one line");
        tracing::warn!("first line\n\nthird line");
        tracing::error!("the last");
    }

    #[test]
    fn on_the_journal_every_line_says_its_priority_and_nothing_else() {
        let log = run_the_child(the_true_stream, false, false);
        assert_eq!(
            String::from_utf8_lossy(&log),
            String::from_utf8_lossy(ON_THE_JOURNAL)
        );
    }

    #[test]
    fn with_another_stream_the_bytes_are_those_of_before() {
        let log = run_the_child(another_stream, false, false);
        let before = run_the_child(another_stream, true, false);
        assert!(log.contains(&0x1b), "the colours left a plain log");
        assert_eq!(
            String::from_utf8_lossy(&timeless(&log)),
            String::from_utf8_lossy(&timeless(&before))
        );
    }

    #[test]
    fn without_a_stream_no_color_still_means_no_colour() {
        let log = run_the_child(another_stream, false, true);
        let before = run_the_child(another_stream, true, true);
        assert!(!log.contains(&0x1b), "NO_COLOR was overruled");
        assert_eq!(
            String::from_utf8_lossy(&timeless(&log)),
            String::from_utf8_lossy(&timeless(&before))
        );
    }

    #[test]
    fn a_stream_is_both_numbers_and_nothing_more() {
        assert!(matches(Some("8:42"), 8, 42));
        for stream in [
            None,
            Some(""),
            Some("8"),
            Some("8:"),
            Some(":42"),
            Some("8:43"),
            Some("9:42"),
            Some("8:42:1"),
            Some("eight:42"),
        ] {
            assert!(!matches(stream, 8, 42), "{stream:?}");
        }
    }

    #[test]
    fn a_level_has_its_priority() {
        assert_eq!(priority_of(&Level::ERROR), 3);
        assert_eq!(priority_of(&Level::WARN), 4);
        assert_eq!(priority_of(&Level::INFO), 6);
        assert_eq!(priority_of(&Level::DEBUG), 7);
        assert_eq!(priority_of(&Level::TRACE), 7);
    }

    #[test]
    fn every_line_with_something_on_it_is_prefixed() {
        assert_eq!(prefixed(4, b"one\n"), b"<4>one\n");
        assert_eq!(prefixed(3, b"one\n\ntwo\n"), b"<3>one\n\n<3>two\n");
        assert_eq!(prefixed(6, b"no end"), b"<6>no end");
        assert_eq!(prefixed(6, b"\n"), b"\n");
        assert_eq!(prefixed(6, b""), b"");
    }
}
