//! Where the numbers come FROM — the only module here that opens a file.
//!
//! Everything it does is read a pseudo-file: `/proc/meminfo`, `/proc/swaps`,
//! `/proc/pressure/memory`, three cgroup files and `statvfs`. All of it costs
//! microseconds and touches no disk, which is why it can run on a one-second
//! tick inside an event loop without the trap `systemd-run --scope` set — that
//! one is synchronous and freezes whoever waits for it.
//!
//! What is NOT here, on purpose: the store's size, what a garbage collection
//! would free, and who is eating what. Those cost seconds and live in
//! [`crate::scan`], on a thread.

use ronoctua_protocol::{Gauge, Reading, Survey};

use crate::{Places, read};

/// The cheap half of the survey: what a tick may read.
///
/// The guard and the consumers are filled in by whoever calls — the first
/// because reading it spawns a process, the second because finding it walks the
/// machine.
#[must_use]
pub fn survey(p: &Places) -> Survey {
    Survey {
        readings: readings(p),
        pressure: pressure(p),
        ..Default::default()
    }
}

/// Stall, from `/proc/pressure/memory`.
///
/// A kernel without PSI gives zeros, and zeros are honest here: no stall
/// measured. The screen says whether the file exists — see
/// [`crate::guard::state`], which is what reports the protection as `Unknown`.
#[must_use]
pub fn pressure(p: &Places) -> ronoctua_protocol::Pressure {
    read(&p.procfs.join("pressure/memory"))
        .as_deref()
        .map(crate::parse_pressure)
        .unwrap_or_default()
}

/// Stall waiting for a PROCESSOR, from `/proc/pressure/cpu`.
///
/// # Why it is here and not in whoever wants it
///
/// Because this crate is the only border with `/proc` — the same rule that
/// makes `ronoctua-niri` the only one that speaks to the compositor. The first
/// caller is `ronoctua-brain`, which refuses to start an update while the
/// machine is busy, and a scheduler that opened `/proc` for one file would be
/// the second border this crate exists to prevent.
///
/// The file has the same grammar as the memory one — [`crate::parse_pressure`]
/// reads both — and only its `some` line means anything: `full` on a processor
/// is a line the kernel prints as zeros because everybody stalled at once is
/// not a state a runnable machine reaches.
///
/// `None`, and not zeros, when the file is not there. Zero stall is a real
/// reading on an idle machine, so it cannot also mean "this kernel has no PSI"
/// — the distinction the whole `readings` list is built around.
#[must_use]
pub fn cpu_pressure(p: &Places) -> Option<ronoctua_protocol::Pressure> {
    read(&p.procfs.join("pressure/cpu"))
        .as_deref()
        .map(crate::parse_pressure)
}

/// Whether the machine is running off its battery right now.
///
/// `None` when there is no battery at all — a desktop, a virtual machine —,
/// which is not the same answer as "plugged in": a caller that refuses to
/// compile a system on battery has nothing to refuse on a machine that has
/// none, and a `false` there would be a claim about hardware that was never
/// read. [`vram`] is the precedent for an optional `/sys` reading, and the
/// numbering is unstable in the same way — `BAT0` on one machine is `BAT1` on
/// the next, so the supplies are walked instead of named.
///
/// What decides is the SUPPLY's own `status`, not `AC/online`: a laptop docked
/// to a charger that is not charging still reports `Discharging`, and that is
/// the state a long build must not start in.
#[must_use]
pub fn on_battery(p: &Places) -> Option<bool> {
    let supplies = std::fs::read_dir(p.sysfs.join("class/power_supply")).ok()?;
    let mut found = None;
    for entry in supplies.flatten() {
        let path = entry.path();
        // Only a battery answers this question. A `Mains` supply has a
        // `status` too — `Unknown` — and reading it as "not discharging" would
        // say "plugged in" on a laptop whose battery is going flat.
        if read(&path.join("type")).as_deref().map(str::trim) != Some("Battery") {
            continue;
        }
        let Some(status) = read(&path.join("status")) else {
            continue;
        };
        // ANY battery discharging is enough: a machine with two of them is on
        // battery as soon as one is being drawn from.
        if status.trim() == "Discharging" {
            return Some(true);
        }
        found = Some(false);
    }
    found
}

/// Every number a tick can afford.
///
/// A gauge the machine does not have simply does not enter the list — no zram,
/// no `zram-original`. A fixed record would have to send zero, and zero is
/// indistinguishable from "there is one and it is empty".
#[must_use]
pub fn readings(p: &Places) -> Vec<Reading> {
    let mut out = Vec::with_capacity(24);
    let mut put = |of: Gauge, bytes: u64| out.push(Reading { of, bytes });

    if let Some(mem) = read(&p.procfs.join("meminfo")) {
        let value = |k: &str| crate::meminfo_value(&mem, k);
        for (gauge, key) in [
            (Gauge::RamTotal, "MemTotal"),
            (Gauge::RamAvailable, "MemAvailable"),
            (Gauge::RamFree, "MemFree"),
            (Gauge::RamCached, "Cached"),
            (Gauge::RamBuffers, "Buffers"),
            (Gauge::RamShmem, "Shmem"),
            (Gauge::RamSlabReclaimable, "SReclaimable"),
            (Gauge::RamDirty, "Dirty"),
            (Gauge::RamAnonymous, "AnonPages"),
        ] {
            if let Some(v) = value(key) {
                put(gauge, v);
            }
        }
        // THE KERNEL'S OWN, which no single line reports: stacks, page tables
        // and vmalloc. It is the part that never appears in "cached" and never
        // comes back under pressure — the difference between a machine that is
        // merely full and one that is in trouble.
        let kernel: u64 = ["KernelStack", "PageTables", "VmallocUsed"]
            .iter()
            .filter_map(|k| value(k))
            .sum();
        if kernel > 0 {
            put(Gauge::RamKernel, kernel);
        }
    }

    if let Some(swaps) = read(&p.procfs.join("swaps")) {
        let (total, used) = crate::parse_swaps(&swaps);
        if total > 0 {
            put(Gauge::SwapTotal, total);
            put(Gauge::SwapFree, total.saturating_sub(used));
        }
    }

    // zram0 and only zram0: the module creates exactly one device, and looking
    // for others would be inventing a configuration this system does not have.
    let zram = p.sysfs.join("block/zram0");
    if let Some((original, compressed)) = read(&zram.join("mm_stat")).as_deref().and_then(|s| {
        // `mm_stat` exists with the device even before anything is written.
        crate::parse_zram(s)
    }) {
        put(Gauge::ZramOriginal, original);
        put(Gauge::ZramCompressed, compressed);
        if let Some(limit) = read(&zram.join("disksize"))
            .as_deref()
            .and_then(|s| s.trim().parse().ok())
        {
            put(Gauge::ZramLimit, limit);
        }
    }

    // The three slices the policy names. `memory.min` is read EFFECTIVE, at the
    // leaf: it is capped by every ancestor, and the slice's own value would read
    // as protected when an ancestor already zeroed it.
    for (slice, current, ceiling) in [
        (
            p.session_slice(),
            Gauge::DesktopCurrent,
            Some(Gauge::DesktopMin),
        ),
        (p.app_slice(), Gauge::AppCurrent, Some(Gauge::AppCeiling)),
        (p.system_slice(), Gauge::SystemCurrent, None),
    ] {
        if let Some(v) = read(&slice.join("memory.current"))
            .as_deref()
            .and_then(crate::parse_cgroup_value)
        {
            put(current, v);
        }
        match ceiling {
            Some(Gauge::DesktopMin) => {
                if let Some(v) = read(&slice.join("memory.min"))
                    .as_deref()
                    .and_then(crate::parse_cgroup_value)
                {
                    put(Gauge::DesktopMin, v);
                }
            }
            Some(Gauge::AppCeiling) => {
                if let Some(v) = read(&slice.join("memory.high"))
                    .as_deref()
                    .and_then(crate::parse_cgroup_value)
                {
                    put(Gauge::AppCeiling, v);
                }
            }
            _ => {}
        }
    }

    // GRAPHICS MEMORY, from the DRM node that publishes it.
    //
    // `amdgpu` does; other drivers do not, and a machine whose driver stays
    // quiet simply has no such reading — which the list-shaped survey says
    // without having to send a zero.
    if let Some((total, used)) = vram(p) {
        put(Gauge::VramTotal, total);
        put(Gauge::VramUsed, used);
    }

    // The filesystem that holds the store — which on this system is the one that
    // holds everything, and the only one whose filling up stops a rebuild.
    if let Some((total, free)) = filesystem("/nix/store") {
        put(Gauge::DiskTotal, total);
        put(Gauge::DiskFree, free);
    }

    out
}

/// Graphics memory: total and used, from the first card that reports it.
///
/// The cards are tried in order because the numbering is not stable — a laptop
/// with an integrated GPU may have it on `card0` or `card1` depending on the
/// order the drivers came up in, and hard-coding either is how this reads zero
/// on the machine next door.
#[must_use]
pub fn vram(p: &Places) -> Option<(u64, u64)> {
    for n in 0..4 {
        let device = p.sysfs.join(format!("class/drm/card{n}/device"));
        let number = |file: &str| {
            read(&device.join(file))
                .as_deref()
                .and_then(|s| s.trim().parse::<u64>().ok())
        };
        if let (Some(total), Some(used)) =
            (number("mem_info_vram_total"), number("mem_info_vram_used"))
            && total > 0
        {
            return Some((total, used));
        }
    }
    None
}

/// Total and free bytes of the filesystem holding `path`.
///
/// `statvfs` and not a `df` we parse: the numbers come as numbers, and a
/// process spawned per tick is the cost this whole module is arranged to avoid.
#[must_use]
pub fn filesystem(path: &str) -> Option<(u64, u64)> {
    let s = rustix::fs::statvfs(path).ok()?;
    let block = s.f_frsize;
    // `f_bavail` and not `f_bfree`: what is free for THIS user, after the
    // reserve the filesystem keeps for root. Showing the bigger number would
    // promise space that a write cannot have.
    Some((s.f_blocks * block, s.f_bavail * block))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::tempdir;

    /// A `/proc` written by the test, which is the whole point of `Places`.
    fn fake() -> (tempdir::Dir, Places) {
        let dir = tempdir::Dir::fresh();
        let procfs = dir.path().join("proc");
        let sysfs = dir.path().join("sys");
        std::fs::create_dir_all(procfs.join("pressure")).expect("mkdir");
        std::fs::create_dir_all(sysfs.join("block/zram0")).expect("mkdir");
        std::fs::write(
            procfs.join("meminfo"),
            "MemTotal:       14969096 kB\nMemAvailable:    8416064 kB\n",
        )
        .expect("meminfo");
        std::fs::write(
            procfs.join("pressure/memory"),
            "some avg10=1.50 avg60=0.00 avg300=0.00 total=1\n\
             full avg10=0.00 avg60=0.00 avg300=0.00 total=1\n",
        )
        .expect("pressure");
        std::fs::write(
            procfs.join("swaps"),
            "Filename\tType\tSize\tUsed\tPriority\n",
        )
        .expect("swaps");
        std::fs::write(sysfs.join("block/zram0/mm_stat"), "1000 400 500 0 0 0 0 0")
            .expect("mm_stat");
        std::fs::write(sysfs.join("block/zram0/disksize"), "7484548096\n").expect("disksize");
        let places = Places {
            procfs,
            sysfs,
            cgroup: dir.path().join("cgroup"),
            uid: 1000,
        };
        (dir, places)
    }

    fn bytes_of(s: &Survey, g: Gauge) -> Option<u64> {
        s.readings.iter().find(|r| r.of == g).map(|r| r.bytes)
    }

    #[test]
    fn a_survey_comes_out_of_a_tree_the_test_wrote() {
        let (_dir, places) = fake();
        let s = survey(&places);
        assert_eq!(bytes_of(&s, Gauge::RamTotal), Some(14_969_096 * 1024));
        assert_eq!(bytes_of(&s, Gauge::ZramOriginal), Some(1000));
        assert_eq!(bytes_of(&s, Gauge::ZramLimit), Some(7_484_548_096));
        assert_eq!(s.pressure.some10, 15);
    }

    #[test]
    fn what_the_machine_does_not_have_does_not_appear() {
        // No swap device in the fake `/proc/swaps` — only the header. A zero
        // reading would read as "there is swap and it is empty", which is a
        // different machine.
        let (_dir, places) = fake();
        let s = survey(&places);
        assert_eq!(bytes_of(&s, Gauge::SwapTotal), None);
        // And no cgroup tree at all: the three slices simply do not report.
        assert_eq!(bytes_of(&s, Gauge::AppCurrent), None);
    }

    #[test]
    fn the_graphics_memory_comes_from_whichever_card_reports_it() {
        let (dir, places) = fake();
        // `card0` exists and says nothing — an integrated GPU may be either
        // number, and hard-coding one is how this reads zero on the machine
        // next door.
        std::fs::create_dir_all(dir.path().join("sys/class/drm/card0/device")).expect("mkdir");
        let card = dir.path().join("sys/class/drm/card1/device");
        std::fs::create_dir_all(&card).expect("mkdir");
        std::fs::write(card.join("mem_info_vram_total"), "1073741824\n").expect("total");
        std::fs::write(card.join("mem_info_vram_used"), "638713856\n").expect("used");

        assert_eq!(vram(&places), Some((1_073_741_824, 638_713_856)));
        let s = survey(&places);
        assert_eq!(bytes_of(&s, Gauge::VramTotal), Some(1_073_741_824));
    }

    #[test]
    fn a_driver_that_says_nothing_reports_nothing() {
        // No DRM node at all: no reading. A zero would say "there is graphics
        // memory and it is empty", which is a different machine.
        let (_dir, places) = fake();
        assert_eq!(vram(&places), None);
        assert_eq!(bytes_of(&survey(&places), Gauge::VramTotal), None);
    }

    #[test]
    fn a_kernel_without_processor_pressure_says_nothing_instead_of_zero() {
        // THE DEFECT THIS CATCHES: a scheduler reading "no stall" on a kernel
        // that does not measure it, and starting a compile on a machine that
        // is already on its knees. Zero is a real reading on an idle machine,
        // so the absence has to be a different answer.
        let (dir, places) = fake();
        assert_eq!(cpu_pressure(&places), None, "it invented a reading");

        std::fs::write(
            dir.path().join("proc/pressure/cpu"),
            "some avg10=12.34 avg60=2.00 avg300=0.00 total=999\n\
             full avg10=0.00 avg60=0.00 avg300=0.00 total=0\n",
        )
        .expect("pressure/cpu");
        let cpu = cpu_pressure(&places).expect("the file exists");
        // Per mille, rounded from the two decimals the kernel prints.
        assert_eq!(cpu.some10, 123);
        assert_eq!(cpu.some60, 20);
    }

    #[test]
    fn the_battery_answers_three_ways_and_a_charger_is_not_one_of_them() {
        let (dir, places) = fake();
        // No `power_supply` at all: a desktop or a VM. "There is no battery"
        // and "it is plugged in" are different answers, and only one of them
        // is true here.
        assert_eq!(on_battery(&places), None);

        let supplies = dir.path().join("sys/class/power_supply");
        // A charger, and nothing else. It has a `status` — `Unknown` — and
        // reading it would answer a question about a battery from a device
        // that is not one.
        let mains = supplies.join("ADP1");
        std::fs::create_dir_all(&mains).expect("mkdir");
        std::fs::write(mains.join("type"), "Mains\n").expect("type");
        std::fs::write(mains.join("status"), "Unknown\n").expect("status");
        assert_eq!(
            on_battery(&places),
            None,
            "the charger answered for the battery"
        );

        let bat = supplies.join("BAT0");
        std::fs::create_dir_all(&bat).expect("mkdir");
        std::fs::write(bat.join("type"), "Battery\n").expect("type");
        std::fs::write(bat.join("status"), "Charging\n").expect("status");
        assert_eq!(on_battery(&places), Some(false));

        std::fs::write(bat.join("status"), "Discharging\n").expect("status");
        assert_eq!(on_battery(&places), Some(true));
    }

    #[test]
    fn the_cgroup_values_are_read_at_the_leaf() {
        let (_dir, places) = fake();
        let app = places.app_slice();
        std::fs::create_dir_all(&app).expect("mkdir");
        std::fs::write(app.join("memory.current"), "2517630976\n").expect("current");
        std::fs::write(app.join("memory.high"), "max\n").expect("high");
        let s = survey(&places);
        assert_eq!(bytes_of(&s, Gauge::AppCurrent), Some(2_517_630_976));
        // `max` is not a number — see `parse_cgroup_value`.
        assert_eq!(bytes_of(&s, Gauge::AppCeiling), None);
    }
}

/// A temporary directory that removes itself, without a dependency.
///
/// `tempfile` would be one more crate in the tree for a dozen lines; the pid
/// plus a counter is unique enough inside a test binary, and `Drop` is what
/// keeps a failed run from leaving litter in `/tmp`.
#[cfg(test)]
mod tempdir {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    pub struct Dir(PathBuf);

    impl Dir {
        pub fn fresh() -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let p =
                std::env::temp_dir().join(format!("ronoctua-memory-{}-{n}", std::process::id()));
            std::fs::create_dir_all(&p).expect("create the test's directory");
            Self(p)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
