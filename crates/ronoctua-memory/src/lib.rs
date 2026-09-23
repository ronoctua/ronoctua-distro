//! The machine's memory — the only crate that reads `/proc`, the cgroups and
//! systemd's memory policy.
//!
//! # Why a crate of its own
//!
//! The same reason `ronoctua-scroll` is the only one that speaks the compositor's IPC and
//! `ronoctua-network` the only one that speaks NetworkManager: one border, one
//! crate. Nothing here knows Wayland, wgpu or the display list, and that is
//! what lets the whole grammar of `/proc/meminfo` be tested without a `/proc`.
//!
//! # The split that makes it testable
//!
//! The parsers are PURE: a `&str` goes in and a number comes out. Only
//! [`Places`] opens a file, and its paths are parameters — the same arrangement
//! `foreign::origin_at` already uses, and for the same reason: a fake tree in a
//! test is worth more than a virtual machine one has to boot.
//!
//! # What the buffer never sees
//!
//! It produces `ronoctua_protocol::Survey`, which carries numbers with names.
//! No path, no `/proc` line, no systemd property string crosses to the guest —
//! a buffer has no parser and is not going to get one.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

// WHAT EMACS IS DOING, which is a  question and therefore this crate's.
pub mod camera;
pub mod emacs;
pub mod guard;
pub mod machine;
pub mod net;
pub mod scan;

pub use machine::survey;

/// A value from `/proc/meminfo`, in bytes.
///
/// The file reports `kB` — which is KiB, in the kernel's own spelling — and the
/// rest of the system counts bytes. Converting here, once, is what keeps a
/// factor of 1024 from living in three places.
///
/// Returns `None` for a key that is not there: a kernel without `Shmem` is not
/// an error, it is a machine with less to show.
#[must_use]
pub fn meminfo_value(content: &str, key: &str) -> Option<u64> {
    for line in content.lines() {
        let (name, rest) = line.split_once(':')?;
        if name != key {
            continue;
        }
        let mut parts = rest.split_whitespace();
        let number: u64 = parts.next()?.parse().ok()?;
        // `kB` in the file, KiB in fact. A line with no unit (`HugePages_Total`)
        // is already a count, and none of ours is one.
        return Some(match parts.next() {
            Some("kB") => number * 1024,
            _ => number,
        });
    }
    None
}

/// A decimal percentage (`"12.34"`) as PER MILLE (`123`).
///
/// Integer arithmetic on purpose: the value comes from a file as text and goes
/// to the screen as one decimal place. Passing through a float would add a
/// rounding nobody can point at, in the one number this whole policy is about.
#[must_use]
pub fn per_mille(text: &str) -> u16 {
    let (whole, frac) = text.split_once('.').unwrap_or((text, "0"));
    let whole: u32 = whole.trim().parse().unwrap_or(0);
    // Two decimal places is what the kernel prints; the third would be noise.
    let mut digits = frac.bytes().filter(u8::is_ascii_digit);
    let tenths = u32::from(digits.next().unwrap_or(b'0') - b'0');
    let hundredths = u32::from(digits.next().unwrap_or(b'0') - b'0');
    // Round the hundredths into the tenths, so `9.99%` is `100`, not `99`.
    //
    // `saturating_mul`: this reads a file, and the same line that carries
    // `avg10=1.50` also carries `total=2999599462`. Multiplying that by ten
    // overflows — which a test found, and which in release would have wrapped
    // into a small number and looked like a machine at rest.
    let rounded = whole.saturating_mul(10) + tenths + u32::from(hundredths >= 5);
    u16::try_from(rounded.min(1000)).unwrap_or(1000)
}

/// `/proc/pressure/memory`, whole.
///
/// The file has two lines — `some` (someone stalled) and `full` (everyone) —
/// and it is the SAME quantity `ManagedOOMMemoryPressureLimit` compares
/// against: a fraction of TIME, never of memory.
#[must_use]
pub fn parse_pressure(content: &str) -> ronoctua_protocol::Pressure {
    let mut out = ronoctua_protocol::Pressure::default();
    for line in content.lines() {
        let mut fields = line.split_whitespace();
        let Some(kind) = fields.next() else { continue };
        for f in fields {
            let Some((name, value)) = f.split_once('=') else {
                continue;
            };
            // Converted INSIDE the arms: the same line carries `total=`, a
            // running microsecond count that is not a percentage at all.
            match (kind, name) {
                ("some", "avg10") => out.some10 = per_mille(value),
                ("some", "avg60") => out.some60 = per_mille(value),
                ("some", "avg300") => out.some300 = per_mille(value),
                ("full", "avg10") => out.full10 = per_mille(value),
                ("full", "avg60") => out.full60 = per_mille(value),
                _ => {}
            }
        }
    }
    out
}

/// `/proc/swaps`: total and used, in bytes, across every device.
///
/// The columns are in 1024-byte units, and the header line is skipped by having
/// no number where a number belongs — which is sturdier than counting lines.
#[must_use]
pub fn parse_swaps(content: &str) -> (u64, u64) {
    let (mut total, mut used) = (0u64, 0u64);
    for line in content.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            continue;
        }
        let (Ok(size), Ok(taken)) = (f[2].parse::<u64>(), f[3].parse::<u64>()) else {
            continue;
        };
        total += size * 1024;
        used += taken * 1024;
    }
    (total, used)
}

/// `/sys/block/zram0/mm_stat`: what was written, what it occupies compressed.
///
/// The pair is what turns compression into a MEASUREMENT. The module asks for
/// `zstd` at 50% of RAM, and that number is the ceiling — the ratio actually
/// achieved is these two columns and nothing else.
#[must_use]
pub fn parse_zram(content: &str) -> Option<(u64, u64)> {
    let f: Vec<&str> = content.split_whitespace().collect();
    // `orig_data_size compr_data_size mem_used_total ...`, in bytes.
    Some((f.first()?.parse().ok()?, f.get(1)?.parse().ok()?))
}

/// A single-value cgroup file (`memory.current`, `memory.min`, `memory.high`).
///
/// `max` means "no ceiling" and comes back as `None` — not as `u64::MAX`, which
/// would print as 16 exabytes on a screen that is trying to be read.
#[must_use]
pub fn parse_cgroup_value(content: &str) -> Option<u64> {
    let t = content.trim();
    if t == "max" { None } else { t.parse().ok() }
}

/// The unit a cgroup path ends in — `ronoctua-9f3a.scope`, `sshd.service`.
///
/// # Why this is NOT `foreign::scope_from_cgroup`
///
/// That one answers a different question: "is this one of OUR buffers", and it
/// strips the `.scope` to give the buffer's id. This one answers "which unit
/// owns this cgroup", keeps the suffix, and says yes to a service that has
/// nothing to do with us — because the question here is who is eating memory,
/// and the answer includes other people's daemons.
///
/// Two names for two questions. Merging them would make one of the two answers
/// wrong somewhere, quietly.
#[must_use]
pub fn cgroup_leaf(line: &str) -> Option<String> {
    let path = line.rsplit(':').next()?.trim();
    let last = path.rsplit('/').next()?;
    if last.ends_with(".scope") || last.ends_with(".service") {
        Some(last.to_owned())
    } else {
        None
    }
}

/// Where the machine's pseudo-filesystems are.
///
/// Parameters, and never literals inside the reader: a test points them at a
/// tree it wrote itself, and the same code proves it works without a machine
/// that has memory pressure.
#[derive(Debug, Clone)]
pub struct Places {
    /// `/proc`.
    pub procfs: PathBuf,
    /// `/sys`.
    pub sysfs: PathBuf,
    /// `/sys/fs/cgroup`.
    pub cgroup: PathBuf,
    /// The user's own id, for the session's cgroup path.
    pub uid: u32,
}

impl Default for Places {
    fn default() -> Self {
        Self {
            procfs: PathBuf::from("/proc"),
            sysfs: PathBuf::from("/sys"),
            cgroup: PathBuf::from("/sys/fs/cgroup"),
            uid: rustix::process::getuid().as_raw(),
        }
    }
}

impl Places {
    /// The session's slice — the compositor and the footer.
    #[must_use]
    pub fn session_slice(&self) -> PathBuf {
        self.user_manager().join("session.slice")
    }

    /// The applications' slice — every foreign buffer, and what `systemd-oomd`
    /// may kill.
    #[must_use]
    pub fn app_slice(&self) -> PathBuf {
        self.user_manager().join("app.slice")
    }

    /// The system's slice: daemons and builds.
    #[must_use]
    pub fn system_slice(&self) -> PathBuf {
        self.cgroup.join("system.slice")
    }

    fn user_manager(&self) -> PathBuf {
        self.cgroup
            .join("user.slice")
            .join(format!("user-{}.slice", self.uid))
            .join(format!("user@{}.service", self.uid))
    }
}

/// Reads a file and gives back `None` instead of an error.
///
/// Every read in this crate is of a pseudo-file that either exists or says
/// something about the machine by not existing — no zram, no cgroup v2, no
/// pressure support. A missing file is DATA here, not a failure.
pub(crate) fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real file's shape, copied from a running machine.
    const MEMINFO: &str = "\
MemTotal:       14969096 kB
MemFree:         2568612 kB
MemAvailable:    8416064 kB
Buffers:              16 kB
Cached:          5579400 kB
Shmem:            123456 kB
Dirty:               888 kB
AnonPages:       1351240 kB
SReclaimable:     456789 kB
HugePages_Total:       0
";

    #[test]
    fn a_meminfo_value_comes_out_in_bytes() {
        // The kernel writes `kB` and means KiB. Getting this wrong is a factor
        // of 1024 on every number on the screen — big enough to be obvious and
        // subtle enough to survive a review.
        assert_eq!(meminfo_value(MEMINFO, "MemTotal"), Some(14_969_096 * 1024));
        assert_eq!(meminfo_value(MEMINFO, "Buffers"), Some(16 * 1024));
        // A line with no unit is already a count.
        assert_eq!(meminfo_value(MEMINFO, "HugePages_Total"), Some(0));
        // And a key this kernel does not have is not an error.
        assert_eq!(meminfo_value(MEMINFO, "NaoExiste"), None);
    }

    #[test]
    fn a_key_is_not_a_prefix_of_another() {
        // `Mem` must not match `MemTotal`, and `Active` must not match
        // `Active(anon)`. Splitting on the colon is what guarantees it.
        assert_eq!(meminfo_value(MEMINFO, "Mem"), None);
    }

    #[test]
    fn a_percentage_becomes_per_mille_without_a_float() {
        assert_eq!(per_mille("0.00"), 0);
        assert_eq!(per_mille("12.34"), 123);
        // The hundredths round INTO the tenths: 9.99% is 100 per mille, and
        // truncating would print 9.9% for something that is 10%.
        assert_eq!(per_mille("9.99"), 100);
        assert_eq!(per_mille("100.00"), 1000);
        // A number bigger than a hundred cannot come out of the kernel, and if
        // it does the screen is not the place to find out.
        assert_eq!(per_mille("250.00"), 1000);
    }

    #[test]
    fn the_pressure_file_gives_five_numbers() {
        let p = parse_pressure(
            "some avg10=1.50 avg60=0.25 avg300=0.00 total=2999599462\n\
             full avg10=0.75 avg60=0.10 avg300=0.00 total=2531743598\n",
        );
        assert_eq!(p.some10, 15);
        assert_eq!(p.some60, 3); // 0.25 -> 2.5 per mille -> rounds to 3
        assert_eq!(p.full10, 8); // 0.75 -> 7.5 -> 8
        assert_eq!(p.some300, 0);
    }

    #[test]
    fn the_swap_header_does_not_become_a_device() {
        let (total, used) = parse_swaps(
            "Filename\t\t\t\tType\t\tSize\t\tUsed\t\tPriority\n\
             /dev/zram0                              partition\t14968828\t7061452\t\t100\n",
        );
        assert_eq!(total, 14_968_828 * 1024);
        assert_eq!(used, 7_061_452 * 1024);
    }

    #[test]
    fn zram_reports_what_was_written_and_what_it_occupies() {
        let (original, compressed) =
            parse_zram("7116926976 3882367435 3995357184 0 6591455232 86289 34328319 742957")
                .expect("mm_stat has two first columns");
        assert_eq!(original, 7_116_926_976);
        assert_eq!(compressed, 3_882_367_435);
        // The ratio is what the screen shows, and it comes from the pair — never
        // from the 50% the module asks for, which is only the ceiling.
        assert!(original > compressed);
    }

    #[test]
    fn a_cgroup_ceiling_of_max_is_not_a_number() {
        assert_eq!(parse_cgroup_value("2517630976\n"), Some(2_517_630_976));
        // `max` printed as 18446744073709551615 would be a screen nobody reads.
        assert_eq!(parse_cgroup_value("max\n"), None);
    }

    #[test]
    fn the_leaf_is_the_unit_and_only_when_it_is_one() {
        assert_eq!(
            cgroup_leaf(
                "0::/user.slice/user-1000.slice/user@1000.service/app.slice/ronoctua-9f3a.scope"
            )
            .as_deref(),
            Some("ronoctua-9f3a.scope")
        );
        assert_eq!(
            cgroup_leaf("0::/system.slice/NetworkManager.service").as_deref(),
            Some("NetworkManager.service")
        );
        // A bare slice is not a scope: it is a GROUP of them, and reporting it
        // as the owner would put every application under one name.
        assert_eq!(cgroup_leaf("0::/user.slice/user-1000.slice"), None);
    }

    #[test]
    fn the_session_and_the_app_slice_are_siblings() {
        // The whole protection rests on this shape: `session.slice` is a
        // SIBLING of `app.slice`, not an ancestor — which is why what is in the
        // first is outside the candidate set of what watches the second.
        let p = Places {
            procfs: PathBuf::from("/proc"),
            sysfs: PathBuf::from("/sys"),
            cgroup: PathBuf::from("/sys/fs/cgroup"),
            uid: 1000,
        };
        assert_eq!(p.session_slice().parent(), p.app_slice().parent());
        assert!(
            p.session_slice()
                .to_string_lossy()
                .contains("user@1000.service")
        );
    }
}
