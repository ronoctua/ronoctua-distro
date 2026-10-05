//! One pace per host: the least time between two searches SENT to it, for
//! the whole process — `ronoctua_words::pace`'s shape, and its one rule that
//! a search box needs: a waiter RESERVES NOTHING. It takes the host's slot
//! only when the slot is due, so a search aborted while it waited (the person
//! typed the next letter) leaves no slot behind it, and ten keystrokes do not
//! queue ten seconds of waiting for the eleventh.
//!
//! A wait longer than a person does is not waited: it is
//! [`crate::Why::Paced`], said on the screen.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::{BRAVE_HOST, DUCKDUCKGO_HOST};

/// The longest a search waits for its host's slot before it is refused as
/// paced: a person typing has moved on long before.
pub const MOST_WAIT: Duration = Duration::from_secs(3);

/// How often a waiter looks again — and so how soon an abort is heard.
const LOOK_AGAIN: Duration = Duration::from_millis(25);

/// The least time between two searches sent to `host`.
///
/// DuckDuckGo's Instant Answers is a free API with no published rate and
/// asks for restraint; one a second follows a person who pauses between
/// words. Wikimedia asks clients to keep to a modest rate; a quarter of a
/// second is a person typing. Brave's free plan is one query a second. A
/// SearXNG the person runs is theirs, and one they do not is somebody's
/// small server: one a second.
#[must_use]
pub fn every(host: &str) -> Duration {
    match host {
        DUCKDUCKGO_HOST | BRAVE_HOST => Duration::from_secs(1),
        h if h.ends_with(".wikipedia.org") => Duration::from_millis(250),
        _ => Duration::from_secs(1),
    }
}

/// When each host may next be asked.
#[derive(Debug, Default)]
pub struct Pace {
    next: Mutex<BTreeMap<String, Instant>>,
}

/// Why a wait ended without a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waited {
    /// The slot is further away than a person waits.
    TooLong,
    /// The caller stopped waiting (an abort).
    Stopped,
}

impl Pace {
    /// A pace nobody has asked through.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes `host`'s slot when it is due at `now` (and moves the next one
    /// `every` later), or says how long until it is.
    ///
    /// # Errors
    ///
    /// How long until the slot is due.
    pub fn take(&self, host: &str, every: Duration, now: Instant) -> Result<(), Duration> {
        let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
        match next.get(host) {
            Some(due) if *due > now => Err(*due - now),
            _ => {
                next.insert(host.to_owned(), now + every);
                Ok(())
            }
        }
    }

    /// Waits for `host`'s slot and takes it; `stopped` is asked between looks.
    ///
    /// # Errors
    ///
    /// [`Waited::TooLong`] past [`MOST_WAIT`], [`Waited::Stopped`] when
    /// `stopped` said so. Blocks: a thread only.
    pub fn wait(&self, host: &str, stopped: &dyn Fn() -> bool) -> Result<(), Waited> {
        let every = every(host);
        let began = Instant::now();
        loop {
            if stopped() {
                return Err(Waited::Stopped);
            }
            let now = Instant::now();
            match self.take(host, every, now) {
                Ok(()) => return Ok(()),
                Err(left) if now.duration_since(began) + left > MOST_WAIT => {
                    return Err(Waited::TooLong);
                }
                Err(left) => std::thread::sleep(left.min(LOOK_AGAIN)),
            }
        }
    }
}

/// The process's pace.
#[must_use]
pub fn shared() -> &'static Pace {
    static PACE: std::sync::OnceLock<Pace> = std::sync::OnceLock::new();
    PACE.get_or_init(Pace::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a waiter that gave up (the person typed on) keeps a slot, so
    /// the next search waits behind searches that were never sent.
    #[test]
    fn a_waiter_that_gives_up_leaves_no_slot_behind() {
        let pace = Pace::new();
        let t0 = Instant::now();
        let second = Duration::from_secs(1);
        pace.take("h", second, t0).unwrap();
        for k in 1..10 {
            let _ = pace.take("h", second, t0 + Duration::from_millis(k * 10));
        }
        assert_eq!(pace.take("h", second, t0 + second), Ok(()));
        assert_eq!(pace.take("other", second, t0), Ok(()), "one pace per host");
    }

    #[test]
    fn a_stopped_waiter_ends_at_once_and_a_long_wait_is_refused() {
        let pace = Pace::new();
        pace.take("slow.example", Duration::from_secs(60), Instant::now())
            .unwrap();
        assert_eq!(pace.wait("slow.example", &|| false), Err(Waited::TooLong));
        assert_eq!(pace.wait("slow.example", &|| true), Err(Waited::Stopped));
        assert!(every("pt.wikipedia.org") < every(DUCKDUCKGO_HOST));
    }
}
