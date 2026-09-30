//! One pace per host: the least time between two requests to it, for the
//! whole process.
//!
//! STATIC, one per [`Host`], because the limit is the host's and not a
//! caller's: two lookups running on two threads share it, and MusicBrainz
//! counts requests per address, not per thread. The daemon is the one
//! process that asks (a door asks the daemon), so a static here is the
//! machine's limiter.
//!
//! A caller RESERVES the next free slot and then waits for it, so two
//! threads that arrive together get two slots `every` apart and never the
//! same one — a check-then-sleep would let both through at once.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::Host;

/// The least time between two requests to one host.
#[derive(Debug)]
pub struct Pace {
    every: Duration,
    /// When the next request may go; `None` before the first.
    next: Mutex<Option<Instant>>,
}

impl Pace {
    /// A pace of one request per `every`.
    #[must_use]
    pub const fn new(every: Duration) -> Self {
        Self {
            every,
            next: Mutex::new(None),
        }
    }

    /// The interval.
    #[must_use]
    pub const fn every(&self) -> Duration {
        self.every
    }

    /// Reserve the next slot, at or after `now`, and say how long to wait for
    /// it. The slot after it is `every` later.
    pub fn reserve(&self, now: Instant) -> Duration {
        let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
        let slot = next.map_or(now, |n| n.max(now));
        *next = Some(slot + self.every);
        slot.saturating_duration_since(now)
    }

    /// Reserve a slot and sleep until it. Blocks: a lookup's thread only.
    pub fn wait(&self) {
        let wait = self.reserve(Instant::now());
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_that_arrive_together_get_slots_a_pace_apart() {
        let pace = Pace::new(Duration::from_secs(1));
        let t0 = Instant::now();
        assert_eq!(pace.reserve(t0), Duration::ZERO, "the first goes at once");
        assert_eq!(pace.reserve(t0), Duration::from_secs(1));
        assert_eq!(
            pace.reserve(t0),
            Duration::from_secs(2),
            "never the same slot"
        );
        // Half a second later, the fourth waits for its own slot (t0 + 3 s).
        assert_eq!(
            pace.reserve(t0 + Duration::from_millis(500)),
            Duration::from_millis(2500)
        );
        // Long after, nobody waits.
        assert_eq!(pace.reserve(t0 + Duration::from_secs(60)), Duration::ZERO);
    }

    #[test]
    fn waiting_really_waits() {
        let pace = Pace::new(Duration::from_millis(60));
        let t0 = Instant::now();
        pace.wait();
        pace.wait();
        pace.wait();
        assert!(
            t0.elapsed() >= Duration::from_millis(120),
            "three requests are two paces apart, at least: {:?}",
            t0.elapsed()
        );
    }

    #[test]
    fn two_threads_never_share_a_slot() {
        let pace = Pace::new(Duration::from_millis(40));
        let t0 = Instant::now();
        std::thread::scope(|s| {
            let a = s.spawn(|| pace.reserve(t0));
            let b = s.spawn(|| pace.reserve(t0));
            let mut waits = [a.join().unwrap(), b.join().unwrap()];
            waits.sort();
            assert_eq!(waits, [Duration::ZERO, Duration::from_millis(40)]);
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
