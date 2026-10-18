//! The meter: how busy the machine is, read for the daemon by the crate that
//! is allowed to look.
//!
//! The READING — [`Pulse`], a filesystem's [`Room`], which of them decides
//! ([`fullest`]) — is the library's (`ronoctua_brain::pulse`), re-exported
//! here whole, so `crate::pulse::…` names both halves. What is here takes one:
//! the [`Meter`] the runner asks every tick, the real [`Machine`] behind it,
//! and the filesystems a reading measures ([`disks`], [`rooms`]). Everything
//! comes from [`ronoctua_memory`], already read and already a number — this is
//! not a second border with `/proc`.

pub use ronoctua_brain::pulse::*;

use crate::moment::Moment;

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
            rooms: rooms.into_iter().flatten().collect(),
        }
    }
}

/// A machine that reports whatever the test says.
///
/// Here and not inside `mod tests`, because the runner's tests need it too and
/// a fake that lives inside one module's test block is a fake that gets copied.
#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub struct Fake(pub Pulse);

#[cfg(test)]
impl Meter for Fake {
    fn read(&self, at: Moment) -> Pulse {
        Pulse {
            at,
            ..self.0.clone()
        }
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
        assert!(p.rooms.is_empty(), "it invented a filesystem");
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
        // ITS SIZE NAMES THE FILESYSTEM, and its free space does not: that is
        // a live number, and anything writing there between this reading and
        // the one above moves it — a battery building beside this test did,
        // by 77 KB.
        let alone = rooms(std::slice::from_ref(&here))[0];
        assert!(alone.is_some(), "the temporary directory has no filesystem");
        // THE ONE THAT ANSWERED, and only it: the path that is not there is
        // no room at all, never a room of zero bytes.
        assert_eq!(
            read.rooms.iter().map(|r| r.total).collect::<Vec<_>>(),
            alone.into_iter().map(|r| r.total).collect::<Vec<_>>(),
            "the measured filesystem is not the one carried"
        );
        // AND THE DISK IS READ AS THAT ONE, out of the same reading.
        assert_eq!(
            read.disk_free_per_mille,
            read.rooms.first().copied().map(Room::free_per_mille)
        );
        assert!(read.disk_free_per_mille.is_some_and(|pm| pm <= 1000));
        // AND NOTHING MEASURABLE IS NONE, never a zero that would start a
        // relief on a machine nobody measured.
        let none = Machine::at(nowhere)
            .measuring(vec![std::path::PathBuf::from("/does-not-exist")])
            .read(1);
        assert_eq!(none.disk_free_per_mille, None);
        assert!(none.rooms.is_empty(), "a disk nobody measured was carried");
    }
}
