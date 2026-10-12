//! The swapfile's size: the machine's RAM, unless the disk is too small for it.
//!
//! # Why the installer decides this, and not the hardware report alone
//!
//! `ronoctua.swapSize` already defaults to the RAM the hardware report lists
//! (`nix/modules/host.nix`), and that is the size hibernation needs: the
//! whole of memory has to fit in the file. What the report cannot know is the
//! DISK: a 32 GiB laptop on a 128 GiB drive would give a quarter of it to a
//! file that is empty almost always. The installer knows both — the RAM from
//! `ronoctua-memory` (the one boundary with `/proc`), the disk the person
//! chose from `lsblk` — so it writes the size into `host.nix`, and caps it:
//! at most [`DISK_SHARE_PERCENT`] of the disk. A capped swap is smaller than
//! the RAM, so it cannot hold a hibernation image, and the machine is
//! installed without one (no `resume_offset`) instead of with one that fails
//! the day it is used.
//!
//! When nothing could be measured (a dry run on a machine without the disk)
//! nothing is written, and the hardware report's default stands.

/// The most of the disk the swapfile may take, in percent.
pub const DISK_SHARE_PERCENT: u64 = 25;

/// One gibibyte, in bytes.
const GIB: u64 = 1 << 30;

/// The swapfile this machine gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Swap {
    /// Its size, in whole gibibytes.
    pub gib: u64,
    /// Whether it holds the RAM — whether the machine hibernates.
    pub hibernates: bool,
}

impl Swap {
    /// The value `ronoctua.swapSize` takes (`"16G"`).
    #[must_use]
    pub fn size(self) -> String {
        format!("{}G", self.gib)
    }
}

/// The swap for this RAM on this disk, both in bytes; `None` when either is
/// unknown or the disk could not spare a single gibibyte. Pure.
#[must_use]
pub fn swap_of(ram_bytes: u64, disk_bytes: u64) -> Option<Swap> {
    if ram_bytes == 0 || disk_bytes == 0 {
        return None;
    }
    // UP to whole gibibytes: a file a byte short of the RAM is a hibernation
    // that fails in use.
    let ram_gib = ram_bytes.div_ceil(GIB);
    // In 128 bits: a share taken before the division is exact, and a
    // percentage of a byte count does not overflow.
    let most_gib = u64::try_from(
        u128::from(disk_bytes) * u128::from(DISK_SHARE_PERCENT) / 100 / u128::from(GIB),
    )
    .unwrap_or(u64::MAX);
    if most_gib == 0 {
        return None;
    }
    Some(if ram_gib <= most_gib {
        Swap {
            gib: ram_gib,
            hibernates: true,
        }
    } else {
        Swap {
            gib: most_gib,
            hibernates: false,
        }
    })
}

/// The RAM of this machine, in bytes, through `ronoctua-memory`.
#[must_use]
pub fn ram_bytes() -> Option<u64> {
    ronoctua_memory::machine::readings(&ronoctua_memory::Places::default())
        .into_iter()
        .find(|r| r.of == ronoctua_protocol::Gauge::RamTotal)
        .map(|r| r.bytes)
}

/// The size of the whole disk at `disk`, in bytes (`lsblk -bdno SIZE`).
#[must_use]
pub fn disk_bytes(disk: &str) -> Option<u64> {
    let out = crate::executor::command("lsblk")
        .args(["-bdno", "SIZE", disk])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// The swap for this machine and the disk the person chose; `None` when
/// either could not be measured.
#[must_use]
pub fn measure(disk: &str) -> Option<Swap> {
    swap_of(ram_bytes()?, disk_bytes(disk)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the swap is smaller than the RAM while it says the machine
    /// hibernates, takes more than its share of a small disk, or is written
    /// at all when nothing could be measured.
    #[test]
    fn the_swap_is_the_ram_unless_the_disk_cannot_spare_it() {
        // 15.3 GiB of RAM on a 1 TB disk: 16 GiB, rounded UP, hibernating.
        let ram = 15 * GIB + GIB / 3;
        let big = 1_000_000_000_000;
        assert_eq!(
            swap_of(ram, big),
            Some(Swap {
                gib: 16,
                hibernates: true
            })
        );
        // Exactly 8 GiB stays 8.
        assert_eq!(swap_of(8 * GIB, big).map(|s| s.gib), Some(8));
        // 32 GiB on a 64 GiB disk: a quarter, 16, and no hibernation.
        let capped = swap_of(32 * GIB, 64 * GIB).unwrap();
        assert_eq!(
            capped,
            Swap {
                gib: 16,
                hibernates: false
            }
        );
        assert!(capped.gib * GIB * 100 <= 64 * GIB * DISK_SHARE_PERCENT);
        // A disk too small to spare a gibibyte, and what was not measured.
        assert_eq!(swap_of(8 * GIB, 3 * GIB), None);
        assert_eq!(swap_of(0, big), None);
        assert_eq!(swap_of(ram, 0), None);
        assert_eq!(capped.size(), "16G");
    }
}
