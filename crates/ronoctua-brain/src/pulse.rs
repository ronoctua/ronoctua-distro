//! The pulse: how busy the machine is, as one reading that crosses the
//! socket. The daemon takes it (`ronoctua_brain_daemon::pulse::Meter`), from
//! the crate that is allowed to look.
//!
//! # This is not a second border with `/proc`
//!
//! `ronoctua-memory` is the only crate that reads `/proc`, the cgroups and
//! systemd's memory policy, in the same way `ronoctua-scroll` is the only one
//! that speaks to the compositor and `ronoctua-network` the only one that
//! speaks to NetworkManager. Everything here comes from `ronoctua_memory`,
//! already read and already a number — including the two readings that had to
//! be ADDED there for this crate to exist (`machine::cpu_pressure` and
//! `machine::on_battery`), which is the honest way to need something a border
//! did not offer yet.
//!
//! # Why every reading is an `Option`
//!
//! Because zero is a real answer. An idle machine measures zero stall, and a
//! kernel with no PSI measures nothing at all — collapsing the two would let
//! the gate wave a build through on a machine it never managed to look at. It
//! is the same rule `survey.readings` is a LIST for: what the machine does not
//! have does not report, and the screen says so instead of drawing a zero.
//!
//! A threshold whose reading is absent is SKIPPED rather than refused, and the
//! absence is on screen — the pulse is published whole, so a missing processor
//! reading is a dash where a number should be. Refusing instead would be a
//! machine that never updates itself and never says why; skipping silently
//! would be the same gate with a hole in it. Showing the hole is the third
//! option, and the only honest one.

use crate::moment::Moment;

/// What the machine is doing right now, as one reading.
///
/// NOT `Copy`: it carries the filesystems it measured ([`Pulse::rooms`]), a
/// list — handed by reference to whoever judges it, and cloned only where a
/// copy is kept (the mind).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Pulse {
    /// When it was taken.
    pub at: Moment,
    /// RAM in use, in per cent of the total. `None` when `/proc/meminfo` said
    /// nothing — which on a running Linux means the reading failed, not that
    /// the machine has no memory.
    pub ram_used_pct: Option<u16>,
    /// Memory stall (PSI `some avg10`), in per mille of a ten-second window.
    pub memory_stall: Option<u16>,
    /// Processor stall (PSI `some avg10`), in per mille of a ten-second window.
    ///
    /// `None` on a kernel without `/proc/pressure/cpu`, which is a real
    /// configuration and not a broken one.
    pub cpu_stall: Option<u16>,
    /// Running off the battery. `None` when there is no battery at all — a
    /// desktop, a virtual machine — which is a different answer from "plugged
    /// in" and must not be confused with it.
    pub on_battery: Option<bool>,
    /// How much of the disk is free, in per mille — the FULLER of the
    /// filesystems holding `/nix/store` and the home, which on most machines
    /// are one. `None` when neither could be measured.
    ///
    /// Read on the tick the runner already has: nothing tells a process that a
    /// disk is filling — inotify reports files, not capacity — and a
    /// `statvfs` is microseconds. It does not cross to the screen (`mind_wire`
    /// lists what does).
    pub disk_free_per_mille: Option<u16>,
    /// The size and the free space of each of those filesystems that could be
    /// measured, in the order of the daemon's `pulse::disks` — empty when
    /// none could.
    ///
    /// # Bytes, and no verdict
    ///
    /// Whether a disk is SHORT, or NEARLY FULL, is a module's question: a
    /// share of it with a cap, from the cache catalogue, which the module that
    /// answers it reads. The pulse used to carry the verdict, and that made the
    /// core name the one module that asked it. It carries what was measured
    /// instead, and every module that has a question about the disk asks it of
    /// the same reading.
    pub rooms: Vec<Room>,
}

/// One filesystem's size and what of it is free, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Room {
    /// How big it is.
    pub total: u64,
    /// How much of it this user may still write.
    pub free: u64,
}

impl Room {
    /// The free share, in per mille.
    #[must_use]
    pub fn free_per_mille(self) -> u16 {
        if self.total == 0 {
            return 0;
        }
        u16::try_from(u128::from(self.free.min(self.total)) * 1000 / u128::from(self.total))
            .unwrap_or(1000)
    }
}

/// The free share of the FULLEST of these filesystems, in per mille.
///
/// The fuller one decides: a relief is about the filesystem that runs out
/// first. `None` when none was measured — never a zero, which would start a
/// relief on a machine nobody measured.
#[must_use]
pub fn fullest(readings: &[Option<Room>]) -> Option<u16> {
    readings.iter().flatten().map(|r| r.free_per_mille()).min()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fullest_filesystem_decides() {
        // A SEPARATE `/home` at 5% free beside a store at 60% is a machine
        // short of disk — and the reading that says so is the smaller one.
        let room = |free, total| Some(Room { total, free });
        assert_eq!(fullest(&[room(600, 1000), room(50, 1000)]), Some(50));
        assert_eq!(fullest(&[None, room(1, 20)]), Some(50));
        assert_eq!(fullest(&[None, None]), None);
        assert_eq!(fullest(&[]), None);
    }
}
