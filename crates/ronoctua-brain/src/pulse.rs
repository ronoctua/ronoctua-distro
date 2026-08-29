//! The pulse: how busy the machine is, taken from the crate that is allowed to
//! look.
//!
//! # This is not a second border with `/proc`
//!
//! `ronoctua-memory` is the only crate that reads `/proc`, the cgroups and
//! systemd's memory policy, in the same way `ronoctua-niri` is the only one
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
}

impl Machine {
    /// The machine this process is running on.
    #[must_use]
    pub fn here() -> Self {
        Self::default()
    }

    /// A machine rooted somewhere else — a tree a test wrote.
    #[must_use]
    pub const fn at(places: ronoctua_memory::Places) -> Self {
        Self { places }
    }
}

impl Meter for Machine {
    fn read(&self, at: Moment) -> Pulse {
        use ronoctua_protocol::Gauge;

        let readings = ronoctua_memory::machine::readings(&self.places);
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
    }
}
