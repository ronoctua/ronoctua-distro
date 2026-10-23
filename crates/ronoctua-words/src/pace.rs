//! One pace per host: the least time between two requests SENT to it, for
//! the whole process.
//!
//! STATIC and keyed by the host, because the limit is the host's and not a
//! caller's: the lyrics and a search asking MyMemory from two threads share
//! it. The shape of `ronoctua_lookup::pace`, with one difference that the
//! search app needs: a waiter RESERVES NOTHING. It takes the host's next slot
//! only when the slot is due, so a request aborted while it waited (the
//! person typed the next letter) leaves no slot behind it, and ten keystrokes
//! do not queue ten seconds of waiting for the eleventh.
//!
//! A wait longer than a person does is not waited: it is [`crate::Why::Paced`],
//! said on the screen.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::{DEEPL_FREE_HOST, DEEPL_HOST, MYMEMORY_HOST, WIKTIONARY_HOST};

/// The longest a request waits for its host's slot before it is refused as
/// paced. A person asked; five seconds of nothing is already a long time.
pub const MOST_WAIT: Duration = Duration::from_secs(5);

/// How often a waiter looks again — and so how soon an abort is heard.
const LOOK_AGAIN: Duration = Duration::from_millis(25);

/// The least time between two requests to `host`.
///
/// MyMemory's anonymous use is a daily quota of characters, and one request a
/// second keeps a song's few requests from reading as a burst. Wikimedia asks
/// clients to keep to a modest rate and name themselves (the User-Agent
/// does); a quarter of a second follows a person typing. DeepL's documented
/// limit is its quota, not a rate. A LibreTranslate the person runs is theirs,
/// and one they do not is somebody's small server.
#[must_use]
pub fn every(host: &str) -> Duration {
    match host {
        MYMEMORY_HOST => Duration::from_secs(1),
        WIKTIONARY_HOST => Duration::from_millis(250),
        DEEPL_HOST | DEEPL_FREE_HOST => Duration::from_millis(200),
        _ => Duration::from_secs(1),
    }
}

/// When each host may next be asked.
#[derive(Debug, Default)]
pub struct Pace {
    next: Mutex<BTreeMap<String, Instant>>,
}

impl Pace {
    /// A pace nobody has asked through.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes `host`'s slot when it is due at `now` (and moves the next one
    /// `every` later), or says how long until it is. Taking nothing when it is
    /// not due is the point: see the module's header.
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
    /// `Waited::TooLong` when the slot is further than [`MOST_WAIT`] away, or
    /// `Waited::Stopped` when `stopped` said so. Blocks: a thread only.
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

/// Why a wait ended without a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waited {
    /// The slot is further away than a person waits.
    TooLong,
    /// The caller stopped waiting (an abort).
    Stopped,
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

    #[test]
    fn a_host_is_asked_a_pace_apart_and_another_host_is_not_held_up() {
        let pace = Pace::new();
        let t0 = Instant::now();
        let second = Duration::from_secs(1);
        assert_eq!(pace.take("a.example", second, t0), Ok(()));
        assert_eq!(
            pace.take("a.example", second, t0 + Duration::from_millis(400)),
            Err(Duration::from_millis(600))
        );
        assert_eq!(
            pace.take("b.example", second, t0),
            Ok(()),
            "one pace per host"
        );
        assert_eq!(pace.take("a.example", second, t0 + second), Ok(()));
    }

    /// FAILS IF: a waiter that gave up (the person typed on) keeps a slot, so
    /// the next request waits behind requests that were never sent.
    #[test]
    fn a_waiter_that_gives_up_leaves_no_slot_behind() {
        let pace = Pace::new();
        let t0 = Instant::now();
        let second = Duration::from_secs(1);
        pace.take("h", second, t0).unwrap();
        for k in 1..10 {
            let _ = pace.take("h", second, t0 + Duration::from_millis(k * 10));
        }
        assert_eq!(
            pace.take("h", second, t0 + second),
            Ok(()),
            "nine refused asks took nothing"
        );
    }

    #[test]
    fn a_stopped_waiter_ends_at_once_and_a_long_wait_is_refused() {
        let pace = Pace::new();
        pace.take("slow.example", Duration::from_secs(60), Instant::now())
            .unwrap();
        assert_eq!(
            pace.wait("slow.example", &|| false),
            Err(Waited::TooLong),
            "a minute away is not waited"
        );
        assert_eq!(pace.wait("slow.example", &|| true), Err(Waited::Stopped));
        // A FRESH HOST IS TAKEN ON THE FIRST LOOK: the wait asks whether it
        // was stopped once, and never sleeps for a turn — counted, not timed,
        // so a busy machine reads the same.
        let looks = std::cell::Cell::new(0);
        let looked = || {
            looks.set(looks.get() + 1);
            false
        };
        assert_eq!(pace.wait("fresh.example", &looked), Ok(()));
        assert_eq!(looks.get(), 1, "a fresh host waited a turn");
    }

    #[test]
    fn every_known_host_has_a_pace() {
        for host in [MYMEMORY_HOST, WIKTIONARY_HOST, DEEPL_HOST, DEEPL_FREE_HOST] {
            assert!(every(host) >= Duration::from_millis(100), "{host}");
        }
    }
}
