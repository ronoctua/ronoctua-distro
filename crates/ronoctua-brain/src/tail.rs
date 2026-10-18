//! The daemon's own log, kept in memory for a screen to follow — as its
//! lines cross the socket. The ring that keeps them, and the layer that
//! copies every event into it, are the daemon's
//! (`ronoctua_brain_daemon::tail`).
//!
//! # A copy of the output, and nothing else
//!
//! Every event the daemon writes to its log — the tick, what it decided, what
//! it started, every warning and error — is ALSO kept here, as the same line,
//! in a ring of the last [`KEPT`]. The brain's page shows it live (`Ask::Tail`).
//! The owner asked for exactly that: a copy of each entry, each tick and each
//! error, and nothing costly.
//!
//! So it costs what a line costs. The ring is fixed and in memory — a restart
//! empties it, and what has to outlive one already does: every run in the
//! brain's journal, every line in the system's. Nobody is woken unless
//! somebody is following: the bell here is its own, and not the mind's, whose
//! every bump wakes every screen that watches the brain.
//!
//! # Why the words are not translated
//!
//! A log line is DATA, the way a file's contents are: it is the daemon's own
//! words (English, invariant 18), shown as it wrote them. The owner approved
//! that reading of invariant 8 for this page (2026-09-25); the frame around
//! the lines — the title, the keys — is keyed as every screen's is.

use serde::{Deserialize, Serialize};

/// How many lines are kept. Five hundred is four hours of ticks alone, and a
/// few tens of kilobytes.
pub const KEPT: usize = 500;

/// How loud a line is — the log's own levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// Something failed.
    Error,
    /// Something is wrong, and the brain carried on.
    Warn,
    /// What it did.
    Info,
    /// Detail.
    Debug,
    /// More detail.
    Trace,
}

impl From<&tracing::Level> for Level {
    fn from(l: &tracing::Level) -> Self {
        match *l {
            tracing::Level::ERROR => Self::Error,
            tracing::Level::WARN => Self::Warn,
            tracing::Level::INFO => Self::Info,
            tracing::Level::DEBUG => Self::Debug,
            tracing::Level::TRACE => Self::Trace,
        }
    }
}

/// One line of the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Line {
    /// Its place in the whole log, counted from the daemon's start. A
    /// follower asks for what comes after the last one it has.
    pub seq: u64,
    /// When, in milliseconds since the epoch.
    pub at_ms: i64,
    /// How loud.
    pub level: Level,
    /// Where it came from and what it says — `target: message key=value …`,
    /// the line the log itself prints after its time and level.
    pub text: String,
}
