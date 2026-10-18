//! The daemon's own log, kept in memory for a screen to follow: the ring, and
//! the layer that fills it.
//!
//! A [`Line`] and its [`Level`] — what crosses the socket — are the library's
//! (`ronoctua_brain::tail`), re-exported here whole, so `crate::tail::…`
//! names both halves.
//!
//! # A copy of the output, and nothing else
//!
//! Every event the daemon writes to its log is ALSO kept here, as the same
//! line, in a ring of the last [`KEPT`]. It costs what a line costs: the ring
//! is fixed and in memory, and nobody is woken unless somebody is following —
//! the bell here is its own, and not the mind's.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer};

pub use ronoctua_brain::tail::*;

/// The ring, behind one lock, with a bell of its own.
#[derive(Debug, Default)]
pub struct Tail {
    state: Mutex<(VecDeque<Line>, u64)>,
    bell: Condvar,
}

impl Tail {
    /// Keeps one line, dropping the oldest past [`KEPT`], and rings.
    pub fn push(&self, level: Level, text: String) {
        let at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        let mut s = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        s.1 += 1;
        let seq = s.1;
        if s.0.len() == KEPT {
            s.0.pop_front();
        }
        s.0.push_back(Line {
            seq,
            at_ms,
            level,
            text,
        });
        drop(s);
        self.bell.notify_all();
    }

    /// Every kept line after `seen`, and the last number there is.
    #[must_use]
    pub fn after(&self, seen: u64) -> (Vec<Line>, u64) {
        let s = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (s.0.iter().filter(|l| l.seq > seen).cloned().collect(), s.1)
    }

    /// Blocks until there is a line after `seen`, or until `patience` runs
    /// out; then answers like [`Tail::after`].
    #[must_use]
    pub fn wait_after(&self, seen: u64, patience: Duration) -> (Vec<Line>, u64) {
        let s = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (s, _) = self
            .bell
            .wait_timeout_while(s, patience, |s| s.1 <= seen)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (s.0.iter().filter(|l| l.seq > seen).cloned().collect(), s.1)
    }
}

/// The log filter the daemon runs under, from the `RUST_LOG` it was given.
///
/// # Why it adds a line of its own
///
/// Because the variable is usually NOT about the brain. The footer starts the
/// daemon when nobody is home, and the daemon inherits the footer's
/// environment — in the development loop that is
/// `ronoctua_shell=debug,ronoctua_render=info`, which names no target of this
/// crate and so silenced every line it writes. The live log page opened on an
/// empty ring and stayed empty. So when the variable says nothing about
/// `ronoctua_brain`, the brain's own `info` is added; when it DOES name the
/// brain, whoever wrote it is obeyed.
#[must_use]
pub fn directives(asked: Option<&str>) -> String {
    const OURS: &str = "ronoctua_brain=info";
    match asked.map(str::trim).filter(|a| !a.is_empty()) {
        None => OURS.to_owned(),
        Some(a) if a.split(',').any(|d| d.trim().starts_with("ronoctua_brain")) => a.to_owned(),
        Some(a) => format!("{a},{OURS}"),
    }
}

/// The daemon's one tail. A global because the log is one: the layer that
/// fills it is installed once, at the start, before anything else exists.
#[must_use]
pub fn global() -> &'static Tail {
    static TAIL: OnceLock<Tail> = OnceLock::new();
    TAIL.get_or_init(Tail::default)
}

/// The layer that copies every event the log shows into [`global`].
///
/// It sits under the SAME filter as the output, so the ring holds what the log
/// prints and nothing it does not.
#[derive(Debug, Clone, Copy, Default)]
pub struct Copier;

impl<S: tracing::Subscriber> Layer<S> for Copier {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let meta = event.metadata();
        let mut words = Words::default();
        event.record(&mut words);
        global().push(meta.level().into(), words.line(meta.target()));
    }
}

/// An event's message and fields, in the order the log prints them.
#[derive(Default)]
struct Words {
    message: String,
    fields: String,
}

impl Words {
    fn line(self, target: &str) -> String {
        let mut line = format!("{target}: {}", self.message);
        if !self.fields.is_empty() {
            line.push(' ');
            line.push_str(self.fields.trim_start());
        }
        line
    }
}

impl Visit for Words {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            let _ = write!(self.fields, " {}={value:?}", field.name());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.fields, " {}={value:?}", field.name());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `RUST_LOG` meant for somebody else keeps the brain's own lines; one
    /// that names the brain is obeyed as written.
    #[test]
    fn an_inherited_filter_does_not_silence_the_brain() {
        assert_eq!(directives(None), "ronoctua_brain=info");
        assert_eq!(directives(Some("  ")), "ronoctua_brain=info");
        assert_eq!(
            directives(Some("ronoctua_shell=debug,ronoctua_render=info")),
            "ronoctua_shell=debug,ronoctua_render=info,ronoctua_brain=info"
        );
        assert_eq!(
            directives(Some("warn,ronoctua_brain=debug")),
            "warn,ronoctua_brain=debug"
        );
    }

    #[test]
    fn the_ring_keeps_the_last_ones_in_order() {
        let t = Tail::default();
        for n in 0..KEPT + 3 {
            t.push(Level::Info, format!("line {n}"));
        }
        let (all, last) = t.after(0);
        assert_eq!(all.len(), KEPT);
        assert_eq!(last, u64::try_from(KEPT + 3).expect("fits"));
        assert_eq!(all[0].text, "line 3");
        assert_eq!(
            all.last().map(|l| l.text.as_str()),
            Some(format!("line {}", KEPT + 2).as_str())
        );
    }

    #[test]
    fn a_follower_gets_only_what_is_new() {
        let t = Tail::default();
        t.push(Level::Info, "one".into());
        let (_, seen) = t.after(0);
        t.push(Level::Warn, "two".into());
        let (new, _) = t.after(seen);
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].level, Level::Warn);
    }

    #[test]
    fn nothing_new_waits_out_its_patience_and_answers_empty() {
        let t = Tail::default();
        t.push(Level::Info, "one".into());
        let (_, seen) = t.after(0);
        let (new, now) = t.wait_after(seen, Duration::from_millis(10));
        assert!(new.is_empty());
        assert_eq!(now, seen);
    }

    /// THE LINE IS THE LOG'S: target, message, and the fields after it.
    #[test]
    fn an_event_is_copied_as_the_log_prints_it() {
        use tracing_subscriber::layer::SubscriberExt as _;
        let subscriber = tracing_subscriber::registry().with(Copier);
        let (_, before) = global().after(0);
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(task = "fixture", ram = 31_u16, "a tick-shaped test line");
        });
        let (new, _) = global().after(before);
        let mine = new
            .iter()
            .find(|l| l.text.contains("a tick-shaped test line"))
            .expect("the event was not copied");
        assert_eq!(mine.level, Level::Warn);
        assert!(
            mine.text
                .ends_with(": a tick-shaped test line task=\"fixture\" ram=31"),
            "{}",
            mine.text
        );
    }
}
