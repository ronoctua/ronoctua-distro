//! The disk's arithmetic: where a relief starts, where the modeline's mark
//! starts, whether a relief is worth running now, and what one did.
//!
//! PURE, every function of it: bytes and moments in, a verdict out. So each
//! rule is tested without a disk to fill, and `module.rs` is left with what
//! touches the machine.
//!
//! # Two lines, both a share with a cap
//!
//! The FLOOR (`diskFloorPercent`, never more than `diskFloorCapGiB`): under it
//! the relief makes room. The NEARLY-FULL LINE (`diskWarnPercent`, never more
//! than `diskWarnCapGiB`), UNDER the floor: under it the modeline carries a
//! mark. A share alone misfires on a big disk — ten per cent of 8 TB is 800 GB,
//! and a disk that holds mostly the person's own files would be "short" with
//! hundreds of gigabytes free. Nix's own floor has the same shape
//! (`nix/modules/store.nix`), and `nix/cache.nix` refuses the build when the
//! nearly-full line is not under the floor.

use crate::moment::Moment;
use crate::pulse::Room;

// `super` is `module.rs`; the manifest and its tables are one level up, in
// the shim the build writes around both.
use super::super::tables::cache as policy;

/// A gibibyte, for the caps.
const GIB: u64 = 1024 * 1024 * 1024;

/// How long a relief waits after the last one, in seconds.
pub(super) const GAP_SECONDS: i64 = policy::RELIEF_GAP_MINUTES as i64 * 60;

/// AN HOUR AT LEAST, checked by the COMPILER: `nix/cache.nix` refuses less,
/// and this is the line that says why a change there matters here — a disk
/// nothing more can be freed on would get a relief every heartbeat.
const _: () = assert!(
    GAP_SECONDS >= 3600,
    "a relief that may run more than once an hour spends the disk it is saving"
);

/// `percent` of a filesystem of `total` bytes, never more than `cap_gib`.
fn share(total: u64, percent: u8, cap_gib: u32) -> u64 {
    (total / 100 * u64::from(percent)).min(u64::from(cap_gib).saturating_mul(GIB))
}

/// The floor of a filesystem of `total` bytes: under it, room is made.
#[must_use]
pub(super) fn floor_of(total: u64) -> u64 {
    share(
        total,
        policy::DISK_FLOOR_PERCENT,
        policy::DISK_FLOOR_CAP_GIB,
    )
}

/// The nearly-full line of a filesystem of `total` bytes: under it, the
/// modeline says so.
#[must_use]
pub(super) fn nearly_full_of(total: u64) -> u64 {
    share(total, policy::DISK_WARN_PERCENT, policy::DISK_WARN_CAP_GIB)
}

/// Whether any of these filesystems is under its floor — `None` when none was
/// measured, which is never short: a relief started on a machine nobody
/// measured would be a guess acting on the disk.
#[must_use]
pub(super) fn short_of<'a>(rooms: impl IntoIterator<Item = &'a Room>) -> Option<bool> {
    let mut measured = rooms.into_iter().peekable();
    measured.peek()?;
    Some(measured.any(|r| r.free < floor_of(r.total)))
}

/// Whether any of these filesystems is under its nearly-full line. None
/// measured is NOT full: a mark about a disk nobody measured would be a mark
/// that lies.
#[must_use]
pub(super) fn nearly_full<'a>(rooms: impl IntoIterator<Item = &'a Room>) -> bool {
    rooms.into_iter().any(|r| r.free < nearly_full_of(r.total))
}

/// The filesystem that decides: the one with the least of it free. `None` when
/// none was measured.
#[must_use]
pub(super) fn deciding<'a>(rooms: impl IntoIterator<Item = &'a Room>) -> Option<Room> {
    rooms
        .into_iter()
        .copied()
        .min_by_key(|r| r.free_per_mille())
}

/// What a relief did, measured before and after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Relief {
    /// How much free space it gained, in bytes: the largest gain among the
    /// filesystems it measured (another process writing meanwhile can only
    /// make this smaller than what was freed).
    pub freed_bytes: u64,
    /// How much of the fullest of them is free now, in per mille.
    pub free_per_mille: u16,
    /// Whether one is still under its floor — nothing more could be freed
    /// without the person.
    pub short: bool,
}

/// What a relief did, from what was measured before and after it.
///
/// The gain is the LARGEST among the filesystems — on the distro's layout
/// they are one, and where they are two the relief freed on each — and a path
/// measured on only one side counts for nothing. What is left free is the
/// fuller one's, which is the one that decides whether the disk is still short.
#[must_use]
pub(super) fn relief_of(before: &[Option<Room>], after: &[Option<Room>]) -> Option<Relief> {
    let free_per_mille = crate::pulse::fullest(after)?;
    let freed_bytes = before
        .iter()
        .zip(after)
        .filter_map(|(b, a)| Some(a.as_ref()?.free.saturating_sub(b.as_ref()?.free)))
        .max()
        .unwrap_or(0);
    Some(Relief {
        freed_bytes,
        free_per_mille,
        short: short_of(after.iter().flatten()) == Some(true),
    })
}

/// Whether the disk is short, the last relief is far enough behind, and —
/// when that one left the disk short — the disk has filled further since.
///
/// `short` of `None` — nothing was measured — is never short. `last` is when
/// this last RAN, a rehearsal included; `left` is what the last real relief
/// left.
///
/// # A relief that could not reach the floor does not repeat itself
///
/// When what fills the disk is the person's — files, programs, five system
/// closures — nothing more can go without them, and a relief every hour would
/// walk the cache and collect the store for nothing, saying "still short" each
/// time. So after one that left the disk short, the next waits, besides the
/// gap, until the disk has FILLED further than that relief left it: only then
/// is there something new to free.
#[must_use]
pub(super) fn short_and_rested(
    short: Option<bool>,
    free_per_mille: Option<u16>,
    now: Moment,
    last: Option<Moment>,
    left: Option<Relief>,
) -> bool {
    if short != Some(true) {
        return false;
    }
    if last.is_some_and(|at| now.saturating_sub(at) < GAP_SECONDS) {
        return false;
    }
    match (left, free_per_mille) {
        (Some(l), Some(free)) if l.short => free < l.free_per_mille,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3600;

    fn room(free: u64, total: u64) -> Room {
        Room { total, free }
    }

    fn left(free_per_mille: u16, short: bool) -> Option<Relief> {
        Some(Relief {
            freed_bytes: 0,
            free_per_mille,
            short,
        })
    }

    #[test]
    fn each_line_is_a_share_and_never_more_than_its_cap() {
        // A SMALL DISK: the share governs.
        let small = 100 * GIB;
        assert_eq!(
            floor_of(small),
            small / 100 * u64::from(policy::DISK_FLOOR_PERCENT)
        );
        assert_eq!(
            nearly_full_of(small),
            small / 100 * u64::from(policy::DISK_WARN_PERCENT)
        );
        // A BIG ONE: the caps. Ten per cent of 8 TiB would be 819 GiB, and a
        // disk with hundreds of gigabytes free is neither short nor full.
        let big = 8 * 1024 * GIB;
        assert_eq!(floor_of(big), u64::from(policy::DISK_FLOOR_CAP_GIB) * GIB);
        assert_eq!(
            nearly_full_of(big),
            u64::from(policy::DISK_WARN_CAP_GIB) * GIB
        );
        assert_eq!(short_of(&[room(200 * GIB, big)]), Some(false));
        assert_eq!(short_of(&[room(floor_of(big) - 1, big)]), Some(true));
        // AND THE MARK'S LINE IS UNDER THE FLOOR, whatever the size: the
        // relief acts first, and the mark is for what it could not do.
        for total in [GIB, 100 * GIB, big, 64 * 1024 * GIB] {
            assert!(
                nearly_full_of(total) < floor_of(total),
                "on {total} bytes the mark would come up before the relief"
            );
        }
    }

    #[test]
    fn the_fuller_decides_and_nothing_measured_says_nothing() {
        let small = 100 * GIB;
        assert_eq!(short_of(&[room(90, 100), room(1, small)]), Some(true));
        assert_eq!(short_of(&[]), None);
        assert!(nearly_full(&[room(small, small), room(1, small)]));
        assert!(!nearly_full(&[]), "a disk nobody measured is marked full");
        // BETWEEN THE TWO LINES: short, and the relief's to fix — not full.
        let between = room(nearly_full_of(small) + GIB, small);
        assert_eq!(short_of(&[between]), Some(true));
        assert!(!nearly_full(&[between]));
        // THE ONE THAT DECIDES is the one with the least of it free, by share.
        assert_eq!(
            deciding(&[room(600, 1000), room(50, 1000)]),
            Some(room(50, 1000))
        );
        assert_eq!(deciding(&[]), None);
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
        let r = |free, total| Some(room(free, total));
        let before = [r(100, 1000), r(500, 1000)];
        let after = [r(1_100, 10_000), r(900, 1000)];
        let relief = relief_of(&before, &after).expect("measured");
        assert_eq!(relief.freed_bytes, 1_000);
        assert_eq!(relief.free_per_mille, 110);
        assert_eq!(relief.short, short_of(after.iter().flatten()) == Some(true));
        // A DISK THAT FILLED MEANWHILE freed nothing — never a negative.
        let relief = relief_of(&[r(900, 1000)], &[r(800, 1000)]).expect("measured");
        assert_eq!(relief.freed_bytes, 0);
        // NOTHING MEASURED AFTER is no relief to report.
        assert_eq!(relief_of(&[r(1, 1)], &[None]), None);
        // AND A SIDE MEASURED ONLY ONCE counts for nothing.
        let relief = relief_of(&[None, r(10, 100)], &[r(999, 1000), r(20, 100)]).expect("measured");
        assert_eq!(relief.freed_bytes, 10);
    }
}
