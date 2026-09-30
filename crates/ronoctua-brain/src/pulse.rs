//! The pulse: how busy the machine is, taken from the crate that is allowed to
//! look.
//!
//! # This is not a second border with `/proc`
//!
//! `ronoctua-memory` is the only crate that reads `/proc`, the cgroups and
//! systemd's memory policy, in the same way `ronoctua-scroll` is the only one
//! that speaks to the compositor and `ronoctua-network` the only one that
//! speaks to NetworkManager. Everything here comes from [`ronoctua_memory`],
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
    /// Whether any of those filesystems is under its FLOOR — a share of it,
    /// capped at a size (`tasks/space.rs`), so it is a question about bytes
    /// and not the per mille above. `tasks/space.rs` starts a relief on it.
    /// `None` when neither could be measured.
    pub disk_short: Option<bool>,
}

/// Whoever can answer "how busy is this machine".
///
/// A trait, and not a function, for the reason every root in this repository is
/// a parameter: a test drives a machine that is on fire without setting fire to
/// the one it runs on.
pub trait Meter: Send + Sync {
    /// One reading, now.
    fn read(&self, at: Moment) -> Pulse;
}

/// The real one: this machine, through [`ronoctua_memory`].
///
/// It holds its `Places` rather than building them per tick, because
/// `Places::default` asks the kernel for the uid and the tick is every half
/// minute for the life of the session.
#[derive(Debug, Default)]
pub struct Machine {
    places: ronoctua_memory::Places,
    /// The paths whose filesystems the disk reading measures — see [`disks`].
    disks: Vec<std::path::PathBuf>,
}

impl Machine {
    /// The machine this process is running on.
    #[must_use]
    pub fn here() -> Self {
        Self {
            places: ronoctua_memory::Places::default(),
            disks: disks(),
        }
    }

    /// A machine rooted somewhere else — a tree a test wrote. It measures no
    /// disk unless told to ([`Machine::measuring`]).
    #[must_use]
    pub const fn at(places: ronoctua_memory::Places) -> Self {
        Self {
            places,
            disks: Vec::new(),
        }
    }

    /// The same, measuring the filesystems holding these paths.
    #[must_use]
    pub fn measuring(mut self, disks: Vec<std::path::PathBuf>) -> Self {
        self.disks = disks;
        self
    }
}

/// The paths whose filesystems a relief is about: the store's, and the home's.
///
/// The store because a rebuild that cannot write there cannot update the
/// machine; the home because that is where the person's own work and the
/// caches live. On the distro's layout they are one btrfs; on a machine with a
/// separate `/home` the fuller of the two is the one that decides.
#[must_use]
pub fn disks() -> Vec<std::path::PathBuf> {
    let mut out = vec![std::path::PathBuf::from("/nix/store")];
    if let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        out.push(std::path::PathBuf::from(home));
    }
    out
}

/// One filesystem's size and what of it is free, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// The size and the free space of the filesystem holding each path — `None`
/// for a path that could not be measured.
#[must_use]
pub fn rooms(paths: &[std::path::PathBuf]) -> Vec<Option<Room>> {
    paths
        .iter()
        .map(|p| {
            let (total, free) = ronoctua_memory::machine::filesystem(p.to_str()?)?;
            (total > 0).then_some(Room { total, free })
        })
        .collect()
}

impl Meter for Machine {
    fn read(&self, at: Moment) -> Pulse {
        use ronoctua_protocol::Gauge;

        let readings = ronoctua_memory::machine::readings(&self.places);
        let rooms = rooms(&self.disks);
        let of = |g: Gauge| {
            readings
                .iter()
                .find(|r| r.of == g)
                .map(|r| r.bytes)
                .filter(|b| *b > 0)
        };
        // USED and not "total minus free": `MemAvailable` is what a new process
        // could actually get, and the difference between the two is the cache,
        // which the kernel hands back under pressure. A machine that reads 90%
        // "used" by the other arithmetic is often perfectly healthy, and a gate
        // built on it would never let anything run.
        let ram_used_pct = of(Gauge::RamTotal).and_then(|total| {
            let available = of(Gauge::RamAvailable)?;
            let used = total.saturating_sub(available);
            u16::try_from(used.saturating_mul(100) / total).ok()
        });

        Pulse {
            at,
            ram_used_pct,
            memory_stall: Some(ronoctua_memory::machine::pressure(&self.places).some10),
            cpu_stall: ronoctua_memory::machine::cpu_pressure(&self.places).map(|p| p.some10),
            on_battery: ronoctua_memory::machine::on_battery(&self.places),
            disk_free_per_mille: fullest(&rooms),
            disk_short: crate::tasks::space::short_of(&rooms),
        }
    }
}

/// A machine that reports whatever the test says.
///
/// Here and not inside `mod tests`, because the runner's tests need it too and
/// a fake that lives inside one module's test block is a fake that gets copied.
#[cfg(test)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Fake(pub Pulse);

#[cfg(test)]
impl Meter for Fake {
    fn read(&self, at: Moment) -> Pulse {
        Pulse { at, ..self.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_machine_did_not_report_stays_unknown() {
        // THE DEFECT THIS CATCHES: a kernel with no processor pressure reading
        // as "no stall", and the gate waving a build through onto a machine it
        // never managed to look at.
        let nowhere = ronoctua_memory::Places {
            procfs: std::path::PathBuf::from("/does-not-exist/proc"),
            sysfs: std::path::PathBuf::from("/does-not-exist/sys"),
            cgroup: std::path::PathBuf::from("/does-not-exist/cgroup"),
            uid: 0,
        };
        let p = Machine::at(nowhere).read(1000);
        assert_eq!(p.at, 1000);
        assert_eq!(p.ram_used_pct, None, "it invented a RAM reading");
        assert_eq!(p.cpu_stall, None, "it invented a CPU reading");
        assert_eq!(p.on_battery, None, "it invented a battery reading");
        assert_eq!(p.disk_free_per_mille, None, "it invented a disk reading");
        assert_eq!(p.disk_short, None, "it invented a disk verdict");
    }

    #[test]
    fn the_disk_is_read_as_the_fuller_of_its_filesystems() {
        // A PATH THAT IS NOT THERE says nothing, and does not hide the one
        // that answers.
        let nowhere = ronoctua_memory::Places {
            procfs: std::path::PathBuf::from("/does-not-exist/proc"),
            sysfs: std::path::PathBuf::from("/does-not-exist/sys"),
            cgroup: std::path::PathBuf::from("/does-not-exist/cgroup"),
            uid: 0,
        };
        let here = std::env::temp_dir();
        let read = Machine::at(nowhere.clone())
            .measuring(vec![
                std::path::PathBuf::from("/does-not-exist"),
                here.clone(),
            ])
            .read(1);
        let alone = rooms(std::slice::from_ref(&here))[0].map(Room::free_per_mille);
        assert!(alone.is_some(), "the temporary directory has no filesystem");
        assert_eq!(read.disk_free_per_mille, alone);
        assert!(read.disk_free_per_mille.is_some_and(|pm| pm <= 1000));
        assert!(read.disk_short.is_some(), "a measured disk was not judged");
        // AND NOTHING MEASURABLE IS NONE, never a zero that would start a
        // relief on a machine nobody measured.
        let none = Machine::at(nowhere)
            .measuring(vec![std::path::PathBuf::from("/does-not-exist")])
            .read(1);
        assert_eq!(none.disk_free_per_mille, None);
        assert_eq!(none.disk_short, None, "a disk nobody measured was judged");
    }

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
