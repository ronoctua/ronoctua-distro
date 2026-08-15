//! The protection against freezing: what is in force, and how to loosen it.
//!
//! # The numbers are not here
//!
//! They are in `nix/modules/memory.nix`, and they reach this crate through a
//! table the module writes to `/etc` — invariant 5 says no Nix logic gets
//! rewritten in Rust, and two copies of `70%` would be two copies to keep in
//! step. What this module does is READ the table, APPLY one of its two
//! profiles, and read back what systemd says is actually in force.
//!
//! # Why "what systemd says" and not "what we asked for"
//!
//! Because the two diverge exactly when the interesting thing happened: a
//! property systemd refused, a drop-in that did not load, a reboot that undid a
//! transient change. A screen that shows the request instead of the state is a
//! screen that lies at the only moment it matters.

use std::path::Path;
use std::process::Command;

use ronoctua_protocol::{Guard, GuardMode};

/// The table the NixOS module writes.
///
/// Text, and deliberately dumb: one profile per line, `key=value` pairs whose
/// values are passed to systemd VERBATIM. Parsing `70%` into a number here and
/// printing it back would be this crate re-deriving what Nix already decided.
pub const PROFILES_PATH: &str = "/etc/ronoctua/memory-profiles.conf";

/// `systemctl`, by store path.
///
/// A user unit's `PATH` is minimal; the name would fail far from the cause.
const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(p) => p,
    None => "systemctl",
};

/// `journalctl`, for counting what the protection has already done.
const JOURNALCTL: &str = match option_env!("RONOCTUA_JOURNALCTL") {
    Some(p) => p,
    None => "journalctl",
};

/// One profile's three numbers, as strings, exactly as systemd takes them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Profile {
    /// `MemoryHigh` on the applications' slice — `70%`.
    pub ceiling: String,
    /// `ManagedOOMMemoryPressureLimit` — `50%`, a fraction of TIME.
    pub pressure: String,
    /// `ManagedOOMMemoryPressureDurationSec` — `15s`.
    pub duration: String,
}

/// The whole table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Profiles {
    /// What the flake declares — what comes back after a reboot.
    pub declared: GuardMode,
    /// The default numbers.
    pub strict: Profile,
    /// The loosened ones.
    pub tolerant: Profile,
}

impl Profiles {
    /// The profile of a mode, when it is one of the two that have numbers.
    #[must_use]
    pub fn of(&self, mode: GuardMode) -> Option<&Profile> {
        match mode {
            GuardMode::Strict => Some(&self.strict),
            GuardMode::Tolerant => Some(&self.tolerant),
            _ => None,
        }
    }
}

/// Reads the table.
///
/// Format, one line each:
///
/// ```text
/// declared strict
/// strict ceiling=70% pressure=50% duration=15s
/// tolerant ceiling=90% pressure=70% duration=60s
/// ```
#[must_use]
pub fn parse_profiles(content: &str) -> Profiles {
    let mut out = Profiles::default();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let Some(head) = fields.next() else { continue };
        if head == "declared" {
            out.declared = match fields.next() {
                Some("tolerant") => GuardMode::Tolerant,
                Some("strict") => GuardMode::Strict,
                // A table naming a profile this build does not know is a table
                // from another version: say `Unknown` rather than pick one.
                _ => GuardMode::Unknown,
            };
            continue;
        }
        let mut p = Profile::default();
        for f in fields {
            let Some((k, v)) = f.split_once('=') else {
                continue;
            };
            match k {
                "ceiling" => p.ceiling = v.to_owned(),
                "pressure" => p.pressure = v.to_owned(),
                "duration" => p.duration = v.to_owned(),
                _ => {}
            }
        }
        match head {
            "strict" => out.strict = p,
            "tolerant" => out.tolerant = p,
            _ => {}
        }
    }
    out
}

/// Loads the table from disk, or an empty one.
#[must_use]
pub fn profiles() -> Profiles {
    crate::read(Path::new(PROFILES_PATH))
        .as_deref()
        .map(parse_profiles)
        .unwrap_or_default()
}

/// `systemctl show` output, as `key=value` lines.
///
/// A pure function so the whole reading can be tested against a captured
/// output, without systemd.
#[must_use]
pub fn parse_show(content: &str, key: &str) -> Option<String> {
    content
        .lines()
        .find_map(|l| l.split_once('=').filter(|(k, _)| *k == key))
        .map(|(_, v)| v.trim().to_owned())
}

/// systemd's `uint32` scale as PER MILLE.
///
/// `UINT32_SCALE_FROM_PERMYRIAD` maps `0..=100%` onto `0..=UINT32_MAX`, so the
/// way back is a proportion. `u64` throughout: `value * 1000` overflows a
/// `u32` at 4.3 million, which is every value this reads.
#[must_use]
pub fn per_mille_of_scale(value: u64) -> u16 {
    let scale = u64::from(u32::MAX);
    u16::try_from(value.min(scale) * 1000 / scale).unwrap_or(1000)
}

/// A systemd timespan (`1min 30s`, `15s`) in whole seconds.
///
/// # It is not microseconds, whatever the property is called
///
/// `ManagedOOMMemoryPressureDurationUSec` prints `1min 30s`, and
/// `[not set]` when it is unset — `systemctl show` FORMATS its `USec`
/// properties. Parsing the field as an integer, which the name invites, gives
/// zero for every value: a screen saying the protection acts "after 0s", which
/// is both wrong and alarming.
///
/// `None` for what has no duration: unset, or infinity.
#[must_use]
pub fn seconds_of_timespan(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() || text.starts_with('[') || text == "infinity" {
        return None;
    }
    // A bare number IS microseconds — some properties print that way, and the
    // two forms are told apart by having a unit or not.
    if let Ok(us) = text.parse::<u64>() {
        return u32::try_from(us / 1_000_000).ok();
    }

    let mut total = 0u64;
    let mut number = String::new();
    let mut unit = String::new();
    // `1min 30s` and `1min30s` both occur; what separates a pair is a digit
    // arriving after letters.
    for c in text.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() && !unit.is_empty() {
            total += span(&number, &unit)?;
            number.clear();
            unit.clear();
        }
        if c.is_ascii_digit() {
            number.push(c);
        } else if c.is_ascii_alphabetic() {
            unit.push(c);
        } else if !number.is_empty() {
            total += span(&number, &unit)?;
            number.clear();
            unit.clear();
        }
    }
    u32::try_from(total).ok()
}

/// One `<number><unit>` pair, in seconds.
fn span(number: &str, unit: &str) -> Option<u64> {
    let n: u64 = number.parse().ok()?;
    Some(match unit {
        "us" | "usec" => n / 1_000_000,
        "ms" | "msec" => n / 1000,
        "s" | "sec" | "" => n,
        "min" | "m" => n * 60,
        "h" => n * 3600,
        "d" => n * 86400,
        // A unit this build does not know is not a zero: refusing is what makes
        // the screen say "cannot tell" instead of "after 0s".
        _ => return None,
    })
}

/// What systemd says is in force on the applications' slice.
///
/// `ram_total` is needed because `MemoryHigh` comes back in BYTES: systemd
/// resolves the percentage when it loads the unit, and what it reports is the
/// resolved value. Turning it back into a percentage is the only way the screen
/// can compare it with the table.
#[must_use]
pub fn from_show(content: &str, ram_total: u64) -> Guard {
    let mut g = Guard::default();

    // A FRACTION OF `UINT32_MAX`, and this is not what it looks like.
    //
    // The unit file says `50%`; systemd parses it with `parse_permyriad` and
    // then scales it — `UINT32_SCALE_FROM_PERMYRIAD` — so what `systemctl show`
    // reports is 2147483648, which is exactly half of 2³²−1. Reading it as
    // permyriad would have printed `214748364.8%`; reading it as a percentage,
    // the same absurdity.
    //
    // THIS WAS A GUESS AND THE VM TEST REFUSED IT. The first version divided by
    // ten, on the reasonable belief that the wire carried permyriad. The
    // machine answered `2147483648`, and that is the whole reason the test
    // asserts the number instead of trusting the reasoning.
    if let Some(v) = parse_show(content, "ManagedOOMMemoryPressureLimit")
        .and_then(|v| v.parse::<u64>().ok())
        .map(per_mille_of_scale)
    {
        g.pressure_limit = v;
    }
    if let Some(v) = parse_show(content, "ManagedOOMMemoryPressureDurationUSec")
        .and_then(|v| seconds_of_timespan(&v))
    {
        g.pressure_seconds = v;
    }
    if ram_total > 0
        && let Some(bytes) = parse_show(content, "MemoryHigh").and_then(|v| v.parse::<u64>().ok())
    {
        g.ceiling = u16::try_from(bytes.saturating_mul(1000) / ram_total).unwrap_or(1000);
    }
    // ARMED is not "the daemon is running": the daemon runs by default and
    // watches nothing. What arms it is `kill` on this slice, and nothing else.
    g.mode = match parse_show(content, "ManagedOOMMemoryPressure").as_deref() {
        Some("kill") => GuardMode::Strict,
        Some("auto") => GuardMode::Off,
        _ => GuardMode::Unknown,
    };
    g
}

/// The full reading: systemd's state, the table, and how many times it acted.
///
/// # It spawns processes, so it does NOT belong on a tick
///
/// `systemctl show` and `journalctl` are two `fork`+`exec` each time. Cheap for
/// a key press, not for a clock — the survey's numbers come from pseudo-files
/// precisely so the periodic path never has to do this.
#[must_use]
pub fn state(ram_total: u64) -> Guard {
    let table = profiles();
    let show = Command::new(SYSTEMCTL)
        .args([
            "--user",
            "show",
            "app.slice",
            "-p",
            "ManagedOOMMemoryPressure",
            "-p",
            "ManagedOOMMemoryPressureLimit",
            "-p",
            "ManagedOOMMemoryPressureDurationUSec",
            "-p",
            "MemoryHigh",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();

    let mut g = from_show(&show, ram_total);
    g.persisted = table.declared;
    g.acted = acted();
    g.zram = crate::read(Path::new("/proc/swaps")).is_some_and(|s| s.contains("zram"));

    // WHICH profile is in force is decided by comparing with the table, and not
    // by remembering what we asked for: a transient change made in another
    // session, or undone by a reboot, is exactly what the screen has to show.
    if g.mode == GuardMode::Strict
        && let Some(t) = table.of(GuardMode::Tolerant)
        && matches_profile(&show, t)
    {
        g.mode = GuardMode::Tolerant;
    }
    g
}

/// Whether what systemd reports is this profile's numbers.
///
/// Compares the DURATION only, and on purpose: it is the one of the three that
/// systemd reports in a unit we can rebuild exactly (microseconds), with no
/// rounding on the way. The ceiling passes through a percentage of RAM and the
/// limit through permyriad — comparing those would make "which profile" depend
/// on a rounding.
fn matches_profile(show: &str, p: &Profile) -> bool {
    let Some(want) = p.duration.trim_end_matches('s').parse::<u64>().ok() else {
        return false;
    };
    parse_show(show, "ManagedOOMMemoryPressureDurationUSec")
        .and_then(|v| seconds_of_timespan(&v))
        .is_some_and(|s| u64::from(s) == want)
}

/// How many times the protection has acted since boot.
///
/// From `systemd-oomd`'s own journal, and matching ITS sentence — not the word
/// "killed", which the user manager also logs and which a kernel OOM kill would
/// also produce. Counting those would report the failure this whole design
/// exists to avoid as if it were the design working.
fn acted() -> u32 {
    let out = Command::new(JOURNALCTL)
        .args(["-u", "systemd-oomd", "--boot", "--no-pager", "-o", "cat"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    u32::try_from(
        out.lines()
            .filter(|l| l.contains("killing due to memory pressure"))
            .count(),
    )
    .unwrap_or(u32::MAX)
}

/// Applies a profile for THIS SESSION.
///
/// # `--runtime` is the whole difference
///
/// `systemctl set-property` without it "stores on disk for future boots"
/// (`systemctl(1)`), which would create a second permanent truth fighting the
/// flake — and nobody would see it. With it, the change lives in `/run` and
/// dies at the next boot, which is what "for this session" means.
///
/// # Errors
///
/// systemd refused the property or the command did not run. The message is
/// systemd's own: not every property may be changed at runtime, and if this one
/// may not, the screen has to say so instead of showing a state that is not in
/// force.
pub fn apply(p: &Profile) -> anyhow::Result<()> {
    anyhow::ensure!(
        !p.ceiling.is_empty() && !p.pressure.is_empty() && !p.duration.is_empty(),
        "a tabela de perfis em {PROFILES_PATH} nao tem os tres numeros"
    );
    let out = Command::new(SYSTEMCTL)
        .args([
            "--user",
            "--runtime",
            "set-property",
            "app.slice",
            &format!("MemoryHigh={}", p.ceiling),
            &format!("ManagedOOMMemoryPressureLimit={}", p.pressure),
            &format!("ManagedOOMMemoryPressureDurationSec={}", p.duration),
        ])
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "o systemd recusou a mudanca: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(())
}

/// Writes the permanent option into the machine's own file.
///
/// It goes through a system unit — `ronoctua-memory-profile@<mode>.service` —
/// because `/etc/nixos/host.nix` belongs to root and this process does not.
/// The unit is a oneshot the module ships, and a NARROW polkit rule lets this
/// user start those two instances and nothing else: the same shape as
/// `keyboard-restore`, for the same reason.
///
/// It does not rebuild: that takes minutes, and this key is pressed by someone
/// who wants the change to survive a reboot, not someone who wants to wait. The
/// screen says what is missing.
///
/// # Errors
///
/// polkit refused, or the unit failed. The message carries systemd's stderr.
pub fn persist(mode: GuardMode) -> anyhow::Result<()> {
    let name = match mode {
        GuardMode::Tolerant => "tolerant",
        GuardMode::Strict => "strict",
        other => anyhow::bail!("nao ha perfil permanente para {other:?}"),
    };
    let out = Command::new(SYSTEMCTL)
        .args(["start", &format!("ronoctua-memory-profile@{name}.service")])
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "nao consegui gravar o perfil: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
# gerado por nix/modules/memory.nix
declared strict
strict ceiling=70% pressure=50% duration=15s
tolerant ceiling=90% pressure=70% duration=60s
";

    #[test]
    fn the_table_carries_the_numbers_as_systemd_takes_them() {
        let t = parse_profiles(TABLE);
        assert_eq!(t.declared, GuardMode::Strict);
        assert_eq!(t.strict.ceiling, "70%");
        assert_eq!(t.tolerant.duration, "60s");
        // The values stay STRINGS: `70%` goes to systemd exactly as Nix wrote
        // it. Parsing it into a number here would be this crate re-deriving
        // what the module already decided.
        assert_eq!(
            t.of(GuardMode::Tolerant).map(|p| p.pressure.as_str()),
            Some("70%")
        );
        assert_eq!(t.of(GuardMode::Unknown), None);
    }

    #[test]
    fn a_table_from_another_version_does_not_pick_a_profile() {
        let t = parse_profiles("declared generoso\n");
        assert_eq!(t.declared, GuardMode::Unknown);
    }

    /// Captured from `systemctl --user show app.slice` on an armed machine.
    const SHOW: &str = "\
ManagedOOMMemoryPressure=kill
ManagedOOMMemoryPressureLimit=2147483648
ManagedOOMMemoryPressureDurationUSec=15s
MemoryHigh=10478301184
";

    #[test]
    fn the_limit_comes_back_from_a_fraction_of_uint32() {
        // MEASURED IN A VM, not reasoned about: `50%` in the unit file is
        // reported as 2147483648 — half of 2³²−1. The first version of this
        // code divided by ten, believing the wire carried permyriad, and the
        // test is what said otherwise.
        let g = from_show(
            "ManagedOOMMemoryPressureLimit=2147483648\n\
             ManagedOOMMemoryPressureDurationUSec=15s\n",
            0,
        );
        assert_eq!(g.pressure_limit, 500);
        assert_eq!(g.pressure_seconds, 15);

        // The ends, so an off-by-one in the proportion shows up here.
        assert_eq!(per_mille_of_scale(0), 0);
        assert_eq!(per_mille_of_scale(u64::from(u32::MAX)), 1000);
        // 70% is what the tolerant profile asks for.
        assert_eq!(per_mille_of_scale(3_006_477_107), 700);
    }

    #[test]
    fn the_ceiling_comes_back_as_a_fraction_of_this_machines_ram() {
        // 10478301184 of 14969096 KiB is 68,4% — and the number only means
        // anything against the RAM of the machine that resolved it.
        let g = from_show(SHOW, 14_969_096 * 1024);
        assert!(
            (680..=690).contains(&g.ceiling),
            "o teto saiu {} por mil",
            g.ceiling
        );
    }

    #[test]
    fn armed_is_kill_and_nothing_else() {
        // `auto` is the default and watches NOTHING: the daemon runs and the
        // monitored list is empty. Reading it as armed would report the exact
        // state this whole policy was written to fix.
        assert_eq!(
            from_show("ManagedOOMMemoryPressure=auto\n", 0).mode,
            GuardMode::Off
        );
        assert_eq!(
            from_show("ManagedOOMMemoryPressure=kill\n", 0).mode,
            GuardMode::Strict
        );
        // And a slice that reports nothing at all is UNKNOWN — not off.
        assert_eq!(from_show("", 0).mode, GuardMode::Unknown);
    }

    #[test]
    fn a_timespan_is_read_the_way_systemd_prints_it() {
        // MEASURED: `systemctl show` FORMATS its `USec` properties. Parsing the
        // field as the integer its name promises gives zero for every value.
        assert_eq!(seconds_of_timespan("15s"), Some(15));
        assert_eq!(seconds_of_timespan("1min"), Some(60));
        assert_eq!(seconds_of_timespan("1min 30s"), Some(90));
        assert_eq!(seconds_of_timespan("2h 5min"), Some(7500));
        // A bare number is microseconds — the other form the same field takes.
        assert_eq!(seconds_of_timespan("15000000"), Some(15));
        // And what has no duration says so, instead of saying zero.
        assert_eq!(seconds_of_timespan("[not set]"), None);
        assert_eq!(seconds_of_timespan("infinity"), None);
        assert_eq!(seconds_of_timespan("3fortnights"), None);
    }

    #[test]
    fn the_profile_in_force_is_told_by_the_duration() {
        let t = parse_profiles(TABLE);
        let tolerant = t.of(GuardMode::Tolerant).expect("tolerante");
        assert!(matches_profile(
            "ManagedOOMMemoryPressureDurationUSec=1min\n",
            tolerant
        ));
        assert!(!matches_profile(
            "ManagedOOMMemoryPressureDurationUSec=15s\n",
            tolerant
        ));
    }
}
