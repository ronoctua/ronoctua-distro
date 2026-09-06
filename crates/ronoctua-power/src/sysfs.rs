//! The charge thresholds — the one thing this crate reads and writes on
//! `/sys`, through the supply directories `ronoctua-memory` walks.
//!
//! # Why sysfs and not UPower
//!
//! UPower 1.90 reports `ChargeStartThreshold`/`ChargeEndThreshold` and offers
//! `EnableChargeThreshold(bool)` — a switch between the daemon's configured
//! pair and "full", not a way to CHOOSE the pair. The owner asked for the pair
//! to be chosen from the screen (40–80 for a laptop that lives on the desk,
//! 60–80, full for a trip), and the attributes are where the kernel keeps it:
//! `charge_control_start_threshold` and `charge_control_end_threshold`, per
//! battery, written as a number. A udev rule in `nix/modules/power.nix` gives
//! the person's group write access to exactly those two files; without it the
//! row says so instead of failing on `RET` (see [`Thresholds::writable`]).
//!
//! # The write order is the kernel's
//!
//! A driver refuses `start >= end`. Raising the pair means writing `end`
//! first; lowering it means writing `start` first. Written the other way round
//! one of the two writes fails with `EINVAL` and the pair ends up half moved.
//! Whatever was written, the TRUTH is what is read back — a driver may accept
//! only some values (ThinkPads take any, some ASUS firmware takes 60/80/100),
//! and the screen shows the file, not the request.

use std::path::Path;

use anyhow::Context as _;

/// The attribute that stops charging at this percentage.
pub const END: &str = "charge_control_end_threshold";
/// The attribute below which charging starts again. Not every battery has it.
pub const START: &str = "charge_control_start_threshold";
/// `charge_behaviour` — `auto`, `inhibit-charge`, `force-discharge`, the
/// current one in brackets. Read only; a fact on the screen.
pub const BEHAVIOUR: &str = "charge_behaviour";
/// `charge_types` — `Fast`, `Standard`, `Long_Life`… the current one in
/// brackets. Read only; a fact on the screen.
pub const TYPES: &str = "charge_types";

/// What the first battery that has thresholds says.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Thresholds {
    /// The supply's directory name — `BAT0` — shown so that a two-battery
    /// laptop knows which one the pair is about.
    pub supply: String,
    /// The start threshold, when the battery has one.
    pub start: Option<u8>,
    /// The end threshold. Its presence is what "supports thresholds" means.
    pub end: u8,
    /// May THIS process write the pair? False when the udev rule has not
    /// reached the file, which is what the row then says.
    pub writable: bool,
    /// The bracketed value of `charge_behaviour`, when the file exists.
    pub behaviour: Option<String>,
    /// The bracketed value of `charge_types`, when the file exists.
    pub types: Option<String>,
}

/// Three pairs worth a row each, so the common choices are one key and not
/// two numbers typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// 40–80: a laptop that lives plugged in on a desk.
    Longevity,
    /// 60–80: mostly plugged in, sometimes not.
    Desk,
    /// Full: a trip, or a battery already too old to matter.
    Full,
}

impl Preset {
    /// In the order the view draws them.
    pub const ALL: [Self; 3] = [Self::Longevity, Self::Desk, Self::Full];

    /// The pair. `None` for a start means "leave the start where it is".
    #[must_use]
    pub const fn pair(self) -> (Option<u8>, u8) {
        match self {
            Self::Longevity => (Some(40), 80),
            Self::Desk => (Some(60), 80),
            Self::Full => (None, 100),
        }
    }

    /// The id the text keys and the commands use.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Longevity => "longevity",
            Self::Desk => "desk",
            Self::Full => "full",
        }
    }
}

/// Why a pair is refused before anything is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The end is not within 1 and 100.
    EndOutOfRange,
    /// The start is not below the end — the kernel's own rule.
    StartNotBelowEnd,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::EndOutOfRange => "the end is not within 1 and 100",
            Self::StartNotBelowEnd => "the start is not below the end",
        })
    }
}

impl std::error::Error for Refused {}

/// Is the pair one the kernel would take?
///
/// # Errors
///
/// The reason, as a value the screen turns into a sentence.
pub const fn admissible(start: Option<u8>, end: u8) -> Result<(), Refused> {
    if end == 0 || end > 100 {
        return Err(Refused::EndOutOfRange);
    }
    if let Some(s) = start
        && s >= end
    {
        return Err(Refused::StartNotBelowEnd);
    }
    Ok(())
}

/// The thresholds of the first battery that has them.
#[must_use]
pub fn read(places: &ronoctua_memory::Places) -> Option<Thresholds> {
    ronoctua_memory::machine::battery_supplies(places)
        .into_iter()
        .find_map(|dir| read_in(&dir))
}

/// One supply directory's thresholds, or `None` when it has no end threshold.
#[must_use]
pub fn read_in(dir: &Path) -> Option<Thresholds> {
    let end = number(&dir.join(END))?;
    Some(Thresholds {
        supply: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        start: number(&dir.join(START)),
        end,
        writable: can_write(&dir.join(END)),
        behaviour: bracketed(&dir.join(BEHAVIOUR)),
        types: bracketed(&dir.join(TYPES)),
    })
}

/// Writes the pair to the first battery that has thresholds and reads back
/// what the kernel kept.
///
/// # Errors
///
/// No battery has thresholds, the pair is not admissible, or a write was
/// refused — by permission (no udev rule) or by the driver.
pub fn write(
    places: &ronoctua_memory::Places,
    start: Option<u8>,
    end: u8,
) -> anyhow::Result<Thresholds> {
    let dir = ronoctua_memory::machine::battery_supplies(places)
        .into_iter()
        .find(|dir| dir.join(END).exists())
        .context("no battery with charge thresholds")?;
    write_in(&dir, start, end)
}

/// The same, for one supply directory. See the module header for the order.
///
/// # Errors
///
/// As [`write`].
pub fn write_in(dir: &Path, start: Option<u8>, end: u8) -> anyhow::Result<Thresholds> {
    admissible(start, end)?;
    let now = read_in(dir).context("no thresholds to write")?;
    let raising = end > now.end;
    // A START THE BATTERY HAS NOT GOT is not written: many laptops expose the
    // end alone, and writing the start there is ENOENT. The view already
    // disables the presets that need one; this is the same fact at the
    // lower door.
    let start = start.filter(|_| now.start.is_some());
    let put = |name: &str, value: u8| -> anyhow::Result<()> {
        std::fs::write(dir.join(name), format!("{value}\n"))
            .with_context(|| format!("writing {name}"))
    };
    if raising {
        put(END, end)?;
        if let Some(s) = start {
            put(START, s)?;
        }
    } else {
        if let Some(s) = start {
            put(START, s)?;
        }
        put(END, end)?;
    }
    read_in(dir).context("the thresholds vanished after the write")
}

fn number(path: &Path) -> Option<u8> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// The `[current]` choice out of `auto [inhibit-charge] force-discharge`.
fn bracketed(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let from = text.find('[')? + 1;
    let to = text[from..].find(']')? + from;
    Some(text[from..to].to_owned())
}

/// May this process write the file? Asked by opening it for writing, which on
/// sysfs has no effect until something is written — and answers with the
/// kernel's own permission check rather than a guess from the mode bits.
fn can_write(path: &Path) -> bool {
    std::fs::OpenOptions::new().write(true).open(path).is_ok()
}

/// Every supply directory of a battery, for a test that builds a fake `/sys`.
#[cfg(test)]
pub(crate) fn fake_supply(root: &Path, name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = root.join("class/power_supply").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("type"), "Battery\n").unwrap();
    for (file, content) in files {
        std::fs::write(dir.join(file), content).unwrap();
    }
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-power-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_battery_with_thresholds_is_read_whole() {
        let root = scratch("read");
        let dir = fake_supply(
            &root,
            "BAT0",
            &[
                (END, "80\n"),
                (START, "40\n"),
                (BEHAVIOUR, "[auto] inhibit-charge force-discharge\n"),
            ],
        );
        let t = read_in(&dir).unwrap();
        assert_eq!(t.supply, "BAT0");
        assert_eq!((t.start, t.end), (Some(40), 80));
        assert_eq!(t.behaviour.as_deref(), Some("auto"));
        assert_eq!(t.types, None);
        assert!(t.writable, "a temp file is writable");
    }

    #[test]
    fn a_battery_without_an_end_threshold_has_none() {
        let root = scratch("none");
        let dir = fake_supply(&root, "BAT0", &[("status", "Discharging\n")]);
        assert_eq!(read_in(&dir), None);
    }

    #[test]
    fn the_pair_is_refused_before_it_is_written() {
        assert_eq!(admissible(None, 0), Err(Refused::EndOutOfRange));
        assert_eq!(admissible(None, 101), Err(Refused::EndOutOfRange));
        assert_eq!(admissible(Some(80), 80), Err(Refused::StartNotBelowEnd));
        assert_eq!(admissible(Some(40), 80), Ok(()));
        assert_eq!(admissible(None, 100), Ok(()));
        for p in Preset::ALL {
            let (s, e) = p.pair();
            assert_eq!(admissible(s, e), Ok(()), "{p:?}");
        }
    }

    #[test]
    fn writing_reads_the_truth_back() {
        let root = scratch("write");
        let dir = fake_supply(&root, "BAT1", &[(END, "100\n"), (START, "0\n")]);
        let t = write_in(&dir, Some(60), 80).unwrap();
        assert_eq!((t.start, t.end), (Some(60), 80));
        // Lowering and raising both land, whatever the order inside.
        let t = write_in(&dir, Some(40), 80).unwrap();
        assert_eq!((t.start, t.end), (Some(40), 80));
        let t = write_in(&dir, None, 100).unwrap();
        assert_eq!((t.start, t.end), (Some(40), 100));
        assert!(write_in(&dir, Some(90), 80).is_err(), "start above end");
    }

    #[test]
    fn a_read_only_file_says_so_instead_of_failing_later() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = scratch("readonly");
        let dir = fake_supply(&root, "BAT0", &[(END, "80\n")]);
        std::fs::set_permissions(dir.join(END), std::fs::Permissions::from_mode(0o444)).unwrap();
        // Root writes anything; the assertion only holds for a person.
        if rustix_free_uid() != 0 {
            assert!(!read_in(&dir).unwrap().writable);
        }
    }

    fn rustix_free_uid() -> u32 {
        // `Places::default` already reads the uid; no second crate for one number.
        ronoctua_memory::Places::default().uid
    }
}
