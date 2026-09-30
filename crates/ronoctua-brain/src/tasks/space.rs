//! Making room on the disk when it is short.
//!
//! # Why the brain owns the WHEN and nothing else
//!
//! The work is a unit and a binary of its own (`nix/modules/space.nix`,
//! `ronoctua-disk-relief`), for the sweep's three measured reasons: it blocks
//! nothing here, it runs at idle priority, and it stays reachable by hand.
//! What is left for the brain is the one thing it can see that nobody else
//! does: its heartbeat already reads the filesystems under the store and the
//! home ([`crate::pulse::Pulse::disk_short`]).
//!
//! # Not a calendar, and not behind the gate
//!
//! A disk running short is a STATE, and the answer to it is "now". So this
//! task is never due by the calendar ([`Task::due`] says why not, on screen) and
//! comes due through [`Task::wanted_now`]: below the floor, once
//! `reliefGapMinutes` have passed since the last run — a rehearsal included.
//!
//! # The floor is a share with a cap
//!
//! `diskFloorPercent` of each filesystem, and never more than
//! `diskFloorCapGiB`: ten per cent of an 8 TB disk that holds mostly the
//! person's own files is 800 GB, and a relief every hour for a disk with
//! hundreds of gigabytes free would be noise the person learns to ignore.
//! Nix's own floor has the same shape (`nix/modules/store.nix`).
//!
//! # A relief that could not reach the floor does not repeat itself
//!
//! When what fills the disk is the person's — files, programs, five system
//! closures — nothing more can go without them, and a relief every hour would
//! walk the cache and collect the store for nothing, saying "still short" each
//! time. So after one that left the disk short, the next waits, besides the
//! gap, until the disk has FILLED further than that relief left it: only then
//! is there something new to free.
//!
//! And the general gate may not hold it ([`Task::gated`]): the machine is
//! busiest exactly while a build is filling its disk, and a relief that waited
//! for the memory to calm down would wait for the disk to be full. It runs at
//! idle priority, so yielding is the scheduler's job, not a hold's.
//!
//! # What it tells the person
//!
//! What it freed and how much is free now ([`crate::task::Relieved`]), measured
//! here before and after — the footer says it on the line under the minibuffer,
//! or says the disk is still short when nothing more could go without them.
//!
//! # The numbers are the catalogue's
//!
//! `assets/cache.nix`, through `nix/cache.nix` — the same table the unit's
//! arguments come from, so the floor the brain watches and the goal the relief
//! aims at cannot drift apart.

use std::sync::Mutex;

use crate::moment::Moment;
use crate::prefs::{PartOfDay, Prefs, TaskPrefs};
use crate::pulse::{Pulse, Room};
use crate::systemd;
use crate::task::{Due, Outcome, Relieved, Report, Task, Verdict, Watched};

/// The task's id — English, kebab-case, and the key of everything the person
/// sees about it (`brain-task-disk-space`). It is also the name of the report
/// the unit writes (`disk-space.json`), which is what the runner watches.
pub const ID: &str = "disk-space";

/// The unit the flake ships, and the only thing this task starts.
///
/// A CONSTANT for `update.rs`'s reason: a unit name that came out of a
/// configuration file would be a configuration file that can start units.
pub const UNIT: &str = "ronoctua-disk-relief.service";

/// The same relief, deleting nothing — what `m e` asks for.
pub const REHEARSAL_UNIT: &str = "ronoctua-disk-relief-rehearsal.service";

/// The policy, generated from `assets/cache.nix` by `nix/cache.nix`.
///
/// The FLOOR, its cap and the GAP are read here; the goal, the ceiling and the
/// old compiler's age are the unit's, because the unit is what deletes.
#[allow(dead_code)]
mod policy {
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// The relief.
pub struct DiskSpace {
    /// The paths whose filesystems it measures before and after — handed in,
    /// so a test measures a directory of its own.
    disks: Vec<std::path::PathBuf>,
    /// What the last real relief left, which decides whether another is worth
    /// running ([`short_and_rested`]). In memory: a brain that restarted runs
    /// one more, which is the cheaper mistake.
    left: Mutex<Option<Relieved>>,
}

impl Default for DiskSpace {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskSpace {
    /// One, measuring the store's filesystem and the home's.
    #[must_use]
    pub fn new() -> Self {
        Self::measuring(crate::pulse::disks())
    }

    /// One measuring somewhere else — what a test hands in.
    #[must_use]
    pub const fn measuring(disks: Vec<std::path::PathBuf>) -> Self {
        Self {
            disks,
            left: Mutex::new(None),
        }
    }

    /// What the last real relief left, if this brain saw one.
    fn left(&self) -> Option<Relieved> {
        self.left.lock().map_or(None, |l| *l)
    }
}

/// A gibibyte, for the floor's cap.
const GIB: u64 = 1024 * 1024 * 1024;

/// The floor of a filesystem of `total` bytes: `diskFloorPercent` of it, and
/// never more than `diskFloorCapGiB`.
#[must_use]
pub fn floor_of(total: u64) -> u64 {
    (total / 100 * u64::from(policy::DISK_FLOOR_PERCENT))
        .min(u64::from(policy::DISK_FLOOR_CAP_GIB).saturating_mul(GIB))
}

/// Whether any of these filesystems is under its floor — `None` when none was
/// measured, which is never short: a relief started on a machine nobody
/// measured would be a guess acting on the disk.
#[must_use]
pub fn short_of(rooms: &[Option<Room>]) -> Option<bool> {
    let mut measured = rooms.iter().flatten().peekable();
    measured.peek()?;
    Some(measured.any(|r| r.free < floor_of(r.total)))
}

/// How long a relief waits after the last one, in seconds.
const GAP_SECONDS: i64 = policy::RELIEF_GAP_MINUTES as i64 * 60;

/// AN HOUR AT LEAST, checked by the COMPILER as `emacs.rs` checks its own
/// cadence: `nix/cache.nix` refuses less, and this is the line that says why a
/// change there matters here — a disk nothing more can be freed on would get a
/// relief every heartbeat.
const _: () = assert!(
    GAP_SECONDS >= 3600,
    "a relief that may run more than once an hour spends the disk it is saving"
);

/// Whether the disk is short, the last relief is far enough behind, and —
/// when that one left the disk short — the disk has filled further since.
///
/// PURE, so the rule is tested without a disk to fill. `short` of `None` —
/// nothing was measured — is never short. `last` is when this last RAN, a
/// rehearsal included; `left` is what the last real relief left.
#[must_use]
pub fn short_and_rested(
    short: Option<bool>,
    free_per_mille: Option<u16>,
    now: Moment,
    last: Option<Moment>,
    left: Option<Relieved>,
) -> bool {
    if short != Some(true) {
        return false;
    }
    if last.is_some_and(|at| now.saturating_sub(at) < GAP_SECONDS) {
        return false;
    }
    // NOTHING NEW TO FREE: the last relief took what it could and the disk
    // stayed short; until it fills further, another would find the same.
    match (left, free_per_mille) {
        (Some(l), Some(free)) if l.short => free < l.free_per_mille,
        _ => true,
    }
}

/// What a relief did, from what was measured before and after it.
///
/// The gain is the LARGEST among the filesystems — on the distro's layout
/// they are one, and where they are two the relief freed on each — and a path
/// measured on only one side counts for nothing. What is left free is the
/// fuller one's, which is the one that decides whether the disk is still short.
#[must_use]
pub fn relief_of(before: &[Option<Room>], after: &[Option<Room>]) -> Option<Relieved> {
    let free_per_mille = crate::pulse::fullest(after)?;
    let freed_bytes = before
        .iter()
        .zip(after)
        .filter_map(|(b, a)| Some(a.as_ref()?.free.saturating_sub(b.as_ref()?.free)))
        .max()
        .unwrap_or(0);
    Some(Relieved {
        at: 0,
        freed_bytes,
        free_per_mille,
        short: short_of(after) == Some(true),
    })
}

impl Task for DiskSpace {
    fn id(&self) -> &'static str {
        ID
    }

    fn defaults(&self) -> TaskPrefs {
        TaskPrefs {
            // NO WINDOW AND NO COUNTDOWN. A disk short of space has no good
            // hour, and a warning before making room would be a countdown the
            // person can only answer by letting the disk fill.
            part_of_day: PartOfDay::Any,
            countdown_seconds: 0,
            // AN HOUR OF PATIENCE: a collection of a large store at idle
            // priority is minutes, not the two hours of a rebuild.
            runaway_minutes: 60,
            ..TaskPrefs::default()
        }
    }

    fn due(&self, _now: Moment, prefs: &Prefs, _last: Option<Moment>) -> Due {
        if !prefs.of_task(self).enabled {
            return Due::Not { why: "off" };
        }
        if !systemd::reachable() {
            return Due::Not {
                why: "no-systemctl",
            };
        }
        // NEVER BY THE CALENDAR, and the row says so instead of a date.
        Due::Not {
            why: "when-the-disk-is-short",
        }
    }

    fn wanted_now(&self, now: Moment, pulse: &Pulse, prefs: &Prefs, last: Option<Moment>) -> bool {
        prefs.of_task(self).enabled
            && systemd::reachable()
            && short_and_rested(
                pulse.disk_short,
                pulse.disk_free_per_mille,
                now,
                last,
                self.left(),
            )
    }

    fn gated(&self) -> bool {
        false
    }

    fn needs(&self, _pulse: &Pulse, _prefs: &Prefs) -> Verdict {
        Verdict::Go
    }

    fn countdown(&self, prefs: &Prefs) -> u16 {
        prefs.of_task(self).countdown_seconds
    }

    fn run(&self, report: &Report) -> Outcome {
        let unit = if report.is_rehearsal() {
            REHEARSAL_UNIT
        } else {
            UNIT
        };
        let before = crate::pulse::rooms(&self.disks);
        // THE MARK BEFORE THE START, for the sweep's reason.
        let mark = report.mark();
        if let Err(e) = systemd::start(unit) {
            return Outcome::Failed {
                at: String::new(),
                why: e.to_string(),
            };
        }
        match report.watch(mark) {
            Watched::Ended(sheet) => {
                // A REHEARSAL CHANGED NOTHING, and says nothing about the disk.
                if !report.is_rehearsal()
                    && let Some(relieved) = relief_of(&before, &crate::pulse::rooms(&self.disks))
                {
                    if let Ok(mut left) = self.left.lock() {
                        *left = Some(relieved);
                    }
                    report.relieve(relieved);
                }
                if sheet.trouble.is_empty() {
                    Outcome::Done(sheet.steps)
                } else {
                    // NO `at`: the steps' ids are the unit's words, and the
                    // screen draws `at` as the update's step names. The trouble
                    // names the step itself (`store-garbage: …`).
                    Outcome::Failed {
                        at: String::new(),
                        why: sheet.trouble,
                    }
                }
            }
            Watched::Runaway(_) => Outcome::Failed {
                at: String::new(),
                why: "runaway".to_owned(),
            },
            Watched::Aborted(_) => {
                // GIVING UP IS WHAT STOPS IT, for the sweep's reason.
                if let Err(e) = systemd::stop(unit) {
                    tracing::warn!(error = %e, "could not stop the disk's relief");
                }
                Outcome::Failed {
                    at: String::new(),
                    why: "aborted".to_owned(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3600;

    fn room(free: u64, total: u64) -> Option<Room> {
        Some(Room { total, free })
    }

    fn left(free_per_mille: u16, short: bool) -> Option<Relieved> {
        Some(Relieved {
            at: 0,
            freed_bytes: 0,
            free_per_mille,
            short,
        })
    }

    #[test]
    fn the_floor_is_a_share_and_never_more_than_its_cap() {
        let gib = GIB;
        // A SMALL DISK: the share governs.
        let small = 100 * gib;
        assert_eq!(
            floor_of(small),
            small / 100 * u64::from(policy::DISK_FLOOR_PERCENT)
        );
        // A BIG ONE: the cap. Ten per cent of 8 TiB would be 819 GiB, and a
        // disk with hundreds of gigabytes free is not short.
        let big = 8 * 1024 * gib;
        assert_eq!(floor_of(big), u64::from(policy::DISK_FLOOR_CAP_GIB) * gib);
        assert_eq!(short_of(&[room(200 * gib, big)]), Some(false));
        assert_eq!(short_of(&[room(floor_of(big) - 1, big)]), Some(true));
        // THE FULLER OF TWO decides, and one nobody measured says nothing.
        assert_eq!(short_of(&[room(90, 100), None, room(1, small)]), Some(true));
        assert_eq!(short_of(&[None, None]), None);
        assert_eq!(short_of(&[]), None);
    }

    #[test]
    fn short_is_under_the_floor_and_rested_is_past_the_gap() {
        // NOT SHORT, or not measured: nothing to do.
        assert!(!short_and_rested(
            Some(false),
            Some(900),
            10 * HOUR,
            None,
            None
        ));
        assert!(!short_and_rested(None, None, 10 * HOUR, None, None));
        // SHORT, never relieved before: now.
        assert!(short_and_rested(
            Some(true),
            Some(50),
            10 * HOUR,
            None,
            None
        ));
        // SHORT, the last run a minute ago — a rehearsal counts: wait out the
        // gap, which is what keeps a relief from starting every tick.
        assert!(!short_and_rested(
            Some(true),
            Some(50),
            10 * HOUR,
            Some(10 * HOUR - 60),
            None
        ));
        assert!(short_and_rested(
            Some(true),
            Some(50),
            10 * HOUR,
            Some(10 * HOUR - GAP_SECONDS),
            None
        ));
    }

    #[test]
    fn a_relief_that_left_the_disk_short_waits_for_it_to_fill_further() {
        // THE DEFECT THIS CATCHES: a disk filled by the person's own files
        // got a relief every hour — a walk of the cache, a collection of the
        // store — and "still short" every hour, piling up unread.
        let long_after = Some(10 * HOUR - 5 * GAP_SECONDS);
        assert!(
            !short_and_rested(Some(true), Some(42), 10 * HOUR, long_after, left(42, true)),
            "the same short disk got another relief"
        );
        assert!(!short_and_rested(
            Some(true),
            Some(60),
            10 * HOUR,
            long_after,
            left(42, true)
        ));
        // FILLED FURTHER: something new may be there to free.
        assert!(short_and_rested(
            Some(true),
            Some(41),
            10 * HOUR,
            long_after,
            left(42, true)
        ));
        // AND ONE THAT REACHED THE FLOOR says nothing about the next episode.
        assert!(short_and_rested(
            Some(true),
            Some(80),
            10 * HOUR,
            long_after,
            left(150, false)
        ));
    }

    #[test]
    fn a_relief_says_the_largest_gain_and_the_fullest_disk() {
        let before = [room(100, 1000), room(500, 1000)];
        let after = [room(1_100, 10_000), room(900, 1000)];
        let r = relief_of(&before, &after).expect("measured");
        assert_eq!(r.freed_bytes, 1_000);
        assert_eq!(r.free_per_mille, 110);
        assert_eq!(r.short, short_of(&after) == Some(true));
        // A DISK THAT FILLED MEANWHILE freed nothing — never a negative.
        let r = relief_of(&[room(900, 1000)], &[room(800, 1000)]).expect("measured");
        assert_eq!(r.freed_bytes, 0);
        // NOTHING MEASURED AFTER is no relief to report.
        assert_eq!(relief_of(&[room(1, 1)], &[None]), None);
        // AND A SIDE MEASURED ONLY ONCE counts for nothing.
        let r =
            relief_of(&[None, room(10, 100)], &[room(999, 1000), room(20, 100)]).expect("measured");
        assert_eq!(r.freed_bytes, 10);
    }

    #[test]
    fn it_is_never_due_by_the_calendar_and_says_why() {
        let t = DiskSpace::measuring(Vec::new());
        let why = match t.due(0, &Prefs::default(), None) {
            Due::Not { why } => why,
            other => panic!("a relief came due by the calendar: {other:?}"),
        };
        if systemd::reachable() {
            assert_eq!(why, "when-the-disk-is-short");
        } else {
            assert_eq!(why, "no-systemctl");
        }
        assert!(
            !t.gated(),
            "the gate would hold a relief until the disk is full"
        );
    }

    #[test]
    fn it_is_wanted_only_when_short_switched_on_and_able_to_ask() {
        let t = DiskSpace::measuring(Vec::new());
        let short = Pulse {
            disk_free_per_mille: Some(50),
            disk_short: Some(true),
            ..Pulse::default()
        };
        let roomy = Pulse {
            disk_free_per_mille: Some(300),
            disk_short: Some(false),
            ..Pulse::default()
        };
        assert_eq!(
            t.wanted_now(10 * HOUR, &short, &Prefs::default(), None),
            systemd::reachable()
        );
        assert!(!t.wanted_now(10 * HOUR, &roomy, &Prefs::default(), None));
        let mut off = Prefs::default();
        off.mine(ID).enabled = false;
        assert!(
            !t.wanted_now(10 * HOUR, &short, &off, None),
            "switched off, it ran"
        );
        // AND AFTER A RELIEF THAT LEFT IT JUST AS SHORT, not again.
        *t.left.lock().unwrap() = left(50, true);
        assert!(!t.wanted_now(10 * HOUR, &short, &Prefs::default(), None));
    }
}
