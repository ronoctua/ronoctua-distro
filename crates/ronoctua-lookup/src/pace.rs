//! One pace per host: the least time between two requests to it, for the
//! whole process.
//!
//! STATIC, one per [`Host`], because the limit is the host's and not a
//! caller's: two lookups running on two threads share it, and MusicBrainz
//! counts requests per address, not per thread. The daemon is the one
//! process that asks (a door asks the daemon), so a static here is the
//! machine's limiter.
//!
//! A caller TAKES the slot under the lock, or waits on it and looks again,
//! so two threads that arrive together never go together — a
//! check-then-sleep outside the lock would let both through at once.
//!
//! COUNTED FROM THE END of a request: a [`Turn`] is held while the request
//! is on the wire, no other request to the host leaves meanwhile, and
//! letting it go puts the next slot `every` after that instant. The host counts the requests that ARRIVE,
//! and a first request that still has to open its connection (name, TCP,
//! TLS) arrives later after its start than a second one that reuses it: paced
//! from the starts alone, the two arrived 0.97 s apart under a pace of one
//! second. Waiting for the end, the next request leaves after the last one
//! arrived and was answered, so it cannot arrive less than `every` after it.

use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::Host;

/// The least time between two requests to one host.
#[derive(Debug)]
pub struct Pace {
    every: Duration,
    slot: Mutex<Slot>,
    /// Rung when a turn ends.
    ended: Condvar,
}

/// When the next request to a host may go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    /// `None` before the first request.
    next: Option<Instant>,
    /// A request is on the wire: nobody goes until it ends.
    held: bool,
}

/// What [`Pace::take`] answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taken {
    /// The slot is the caller's.
    Now,
    /// Free in this long; look again then.
    In(Duration),
    /// A request is on the wire; look again when it ends.
    Held,
}

impl Pace {
    /// A pace of one request per `every`.
    #[must_use]
    pub const fn new(every: Duration) -> Self {
        Self {
            every,
            slot: Mutex::new(Slot {
                next: None,
                held: false,
            }),
            ended: Condvar::new(),
        }
    }

    /// The interval.
    #[must_use]
    pub const fn every(&self) -> Duration {
        self.every
    }

    /// Take the slot at `now` if it is free, or say why not.
    pub fn take(&self, now: Instant) -> Taken {
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        take(&mut slot, now)
    }

    /// The request that took the slot ended at `now`: the next one goes no
    /// sooner than `every` after it.
    pub fn finish(&self, now: Instant) {
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        let after = now + self.every;
        slot.next = Some(slot.next.map_or(after, |n| n.max(after)));
        slot.held = false;
        drop(slot);
        self.ended.notify_all();
    }

    /// Wait for a slot and take it. Blocks: a lookup's thread only. The turn
    /// is held while the request is sent and answered; letting it go is
    /// [`Pace::finish`].
    #[must_use = "the turn is held until the request is answered"]
    pub fn wait(&self) -> Turn<'_> {
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            slot = match take(&mut slot, Instant::now()) {
                Taken::Now => return Turn(self),
                Taken::In(wait) => {
                    self.ended
                        .wait_timeout(slot, wait)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
                Taken::Held => self
                    .ended
                    .wait(slot)
                    .unwrap_or_else(PoisonError::into_inner),
            };
        }
    }
}

/// [`Pace::take`], on the slot under its lock.
fn take(slot: &mut Slot, now: Instant) -> Taken {
    match slot.next {
        _ if slot.held => Taken::Held,
        Some(n) if n > now => Taken::In(n - now),
        _ => {
            slot.held = true;
            Taken::Now
        }
    }
}

/// A slot taken: dropped when the request it was taken for is answered (or
/// failed), which moves the host's next slot to `every` after that.
#[derive(Debug)]
pub struct Turn<'a>(&'a Pace);

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        self.0.finish(Instant::now());
    }
}

/// The pace of `host`: one static each, for the whole process.
#[must_use]
pub fn of(host: Host) -> &'static Pace {
    static LRCLIB: Pace = Pace::new(Host::Lrclib.every());
    static SPONSOR_BLOCK: Pace = Pace::new(Host::SponsorBlock.every());
    static MUSIC_BRAINZ: Pace = Pace::new(Host::MusicBrainz.every());
    static COVER_ART_ARCHIVE: Pace = Pace::new(Host::CoverArtArchive.every());
    static WIKIDATA: Pace = Pace::new(Host::Wikidata.every());
    static WIKIDATA_QUERY: Pace = Pace::new(Host::WikidataQuery.every());
    static COMMONS: Pace = Pace::new(Host::Commons.every());
    static TMDB: Pace = Pace::new(Host::Tmdb.every());
    static TMDB_IMAGES: Pace = Pace::new(Host::TmdbImages.every());
    static OPEN_SUBTITLES: Pace = Pace::new(Host::OpenSubtitles.every());
    static SUB_DL: Pace = Pace::new(Host::SubDl.every());
    static SUB_DL_FILES: Pace = Pace::new(Host::SubDlFiles.every());
    static PODNAPISI: Pace = Pace::new(Host::Podnapisi.every());
    static VIRUS_TOTAL: Pace = Pace::new(Host::VirusTotal.every());
    static XPOSED_OR_NOT: Pace = Pace::new(Host::XposedOrNot.every());
    static PWNED_PASSWORDS: Pace = Pace::new(Host::PwnedPasswords.every());
    match host {
        Host::Lrclib => &LRCLIB,
        Host::SponsorBlock => &SPONSOR_BLOCK,
        Host::MusicBrainz => &MUSIC_BRAINZ,
        Host::CoverArtArchive => &COVER_ART_ARCHIVE,
        Host::Wikidata => &WIKIDATA,
        Host::WikidataQuery => &WIKIDATA_QUERY,
        Host::Commons => &COMMONS,
        Host::Tmdb => &TMDB,
        Host::TmdbImages => &TMDB_IMAGES,
        Host::OpenSubtitles => &OPEN_SUBTITLES,
        Host::SubDl => &SUB_DL,
        Host::SubDlFiles => &SUB_DL_FILES,
        Host::Podnapisi => &PODNAPISI,
        Host::VirusTotal => &VIRUS_TOTAL,
        Host::XposedOrNot => &XPOSED_OR_NOT,
        Host::PwnedPasswords => &PWNED_PASSWORDS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_that_arrive_together_get_slots_a_pace_apart() {
        let pace = Pace::new(Duration::from_secs(1));
        let t0 = Instant::now();
        assert_eq!(pace.take(t0), Taken::Now, "the first goes at once");
        assert_eq!(pace.take(t0), Taken::Held, "never while it is on the wire");
        pace.finish(t0);
        assert_eq!(
            pace.take(t0),
            Taken::In(Duration::from_secs(1)),
            "never the same slot"
        );
        assert_eq!(
            pace.take(t0 + Duration::from_millis(500)),
            Taken::In(Duration::from_millis(500))
        );
        assert_eq!(pace.take(t0 + Duration::from_secs(1)), Taken::Now);
        pace.finish(t0 + Duration::from_secs(1));
        // Long after, nobody waits.
        assert_eq!(pace.take(t0 + Duration::from_secs(60)), Taken::Now);
    }

    /// FAILS IF: the pace counts only from the starts, so a request that
    /// spent its first 300 ms opening a connection lets the next one ARRIVE
    /// less than a pace after it — the 0.97 s MusicBrainz was asked at.
    #[test]
    fn the_next_slot_counts_from_the_end_of_the_last_request() {
        let pace = Pace::new(Duration::from_secs(1));
        let t0 = Instant::now();
        assert_eq!(pace.take(t0), Taken::Now);
        pace.finish(t0 + Duration::from_millis(300));
        assert_eq!(
            pace.take(t0 + Duration::from_secs(1)),
            Taken::In(Duration::from_millis(300))
        );
        assert_eq!(pace.take(t0 + Duration::from_millis(1300)), Taken::Now);
        pace.finish(t0 + Duration::from_millis(1300));
        // An end sooner than the slot already set does not bring it closer.
        pace.finish(t0 + Duration::from_millis(1000));
        assert_eq!(
            pace.take(t0 + Duration::from_millis(2000)),
            Taken::In(Duration::from_millis(300))
        );
    }

    #[test]
    fn waiting_really_waits() {
        let pace = Pace::new(Duration::from_millis(60));
        let t0 = Instant::now();
        drop(pace.wait());
        drop(pace.wait());
        drop(pace.wait());
        assert!(
            t0.elapsed() >= Duration::from_millis(120),
            "three requests are two paces apart, at least: {:?}",
            t0.elapsed()
        );
    }

    /// FAILS IF: a request held longer than the pace lets the next one leave
    /// before it is answered — a waiter must look again after sleeping.
    #[test]
    fn a_waiter_waits_for_the_turn_before_it_to_end() {
        let pace = Pace::new(Duration::from_millis(40));
        let t0 = Instant::now();
        std::thread::scope(|s| {
            let held = pace.wait();
            let next = s.spawn(|| {
                let _turn = pace.wait();
                t0.elapsed()
            });
            std::thread::sleep(Duration::from_millis(150));
            drop(held);
            let left = next.join().unwrap();
            assert!(
                left >= Duration::from_millis(190),
                "the second left {left:?} after the first, before its end and a pace"
            );
        });
    }

    #[test]
    fn two_threads_never_share_a_slot() {
        let pace = Pace::new(Duration::from_millis(40));
        let t0 = Instant::now();
        std::thread::scope(|s| {
            let a = s.spawn(|| pace.take(t0));
            let b = s.spawn(|| pace.take(t0));
            let got = [a.join().unwrap(), b.join().unwrap()];
            assert!(
                got.contains(&Taken::Now) && got.contains(&Taken::Held),
                "{got:?}"
            );
        });
    }

    #[test]
    fn every_host_has_its_own_pace_at_its_own_interval() {
        for host in Host::ALL {
            assert_eq!(of(host).every(), host.every(), "{host:?}");
            for other in Host::ALL {
                if other != host {
                    assert!(
                        !std::ptr::eq(of(host), of(other)),
                        "{host:?} and {other:?} share a limiter"
                    );
                }
            }
        }
        assert_eq!(of(Host::MusicBrainz).every(), Duration::from_secs(1));
    }
}
