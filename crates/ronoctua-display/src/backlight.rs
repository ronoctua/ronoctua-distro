//! The brightness of a built-in panel — `/sys/class/backlight/<device>`.
//!
//! Molded on `ronoctua_power::sysfs`, and for the same reason: these are
//! root-owned attributes that a udev rule opens to one group, so whether they
//! can be written is a QUESTION and not an assumption. The answer travels in
//! the reading, so the row on screen can say "the rule is not there" instead
//! of failing when somebody presses a key.
//!
//! # Why `brightness` and not `actual_brightness`
//!
//! Both exist. `brightness` is what was last asked for; `actual_brightness`
//! is what the hardware reports, and on a panel that fades it walks towards
//! the first over some tens of milliseconds. Reading the second would make
//! the number under the cursor jitter while the eye is on it, and a value
//! that moves on its own is a value nobody trusts. The setpoint is the
//! preference; the fade is the hardware's business.

use std::path::Path;

use anyhow::{Context as _, Result};

/// What was asked for.
pub const NOW: &str = "brightness";
/// The ceiling. It is NOT 100 — panels report 255, 4437, 65535, 96000.
pub const MAX: &str = "max_brightness";

/// THE LOWEST A SCREEN MAY BE TAKEN TO, in percent.
///
/// Not zero, and this is the whole reason the number exists: on most panels
/// zero is black, and a black screen is one you cannot find the key on to
/// undo it. One percent is dim enough to be useful in the dark and bright
/// enough to read the row that says how to get back.
pub const FLOOR: u8 = 1;

/// A panel's light, as it is right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    /// The device's name — `amdgpu_bl1`, `intel_backlight`, `acpi_video0`.
    /// It is shown, because which of them a machine has explains a lot when
    /// the brightness does not move.
    pub at: String,
    /// The raw value, in the panel's own units.
    pub now: u32,
    /// The largest that value may be.
    pub max: u32,
    /// Whether THIS process may write it. False means the udev rule is not
    /// there, and the row says so rather than the key failing.
    pub writable: bool,
}

impl Panel {
    /// The brightness as a person reads it.
    #[must_use]
    pub fn percent(&self) -> u8 {
        percent_of(self.now, self.max)
    }
}

/// Reads a backlight directory. `None` when it is not one.
#[must_use]
pub fn read_in(dir: &Path) -> Option<Panel> {
    let max = number(&dir.join(MAX))?;
    // A CEILING OF ZERO IS NOT A PANEL. It happens on a stub driver, and
    // every percentage computed from it would divide by zero.
    if max == 0 {
        tracing::debug!(at = %dir.display(), "a backlight whose maximum is zero");
        return None;
    }
    Some(Panel {
        at: dir.file_name()?.to_string_lossy().into_owned(),
        now: number(&dir.join(NOW))?.min(max),
        max,
        writable: can_write(&dir.join(NOW)),
    })
}

/// Writes a raw value and answers with what the file says AFTERWARDS.
///
/// The truth is the read-back, never the request: a driver may quantise the
/// value, or clamp it, and the screen has to show the panel rather than the
/// intention. It is `ronoctua_power::sysfs`'s rule, and it was learned there.
///
/// # Errors
///
/// The attribute cannot be written — no udev rule, or no such panel — or it
/// vanished between the write and the read, which is a monitor unplugged
/// mid-gesture.
pub fn write_in(dir: &Path, raw: u32) -> Result<Panel> {
    let max = number(&dir.join(MAX)).context("this is not a backlight")?;
    anyhow::ensure!(max > 0, "this backlight has no maximum");
    // CLAMPED HERE AND NOT BY THE CALLER, because there are two callers — a
    // key and a row — and a value out of range is a black screen from one of
    // them.
    let value = raw.clamp(floor_raw(max), max);
    std::fs::write(dir.join(NOW), value.to_string()).with_context(|| {
        format!(
            "could not write {}. The udev rule that opens it to the group is \
             what makes this possible without root.",
            dir.join(NOW).display()
        )
    })?;
    read_in(dir).context("the backlight vanished after the write")
}

/// The raw value as a percentage, rounded.
///
/// Rounded and not truncated so that it round-trips: `raw_of(percent_of(v))`
/// comes back to `v` on the panels this was measured against, and a
/// brightness that drifts down by one unit every time it is read is a
/// brightness that walks to the floor over an afternoon.
#[must_use]
pub fn percent_of(now: u32, max: u32) -> u8 {
    if max == 0 {
        return 0;
    }
    let scaled = (u64::from(now) * 100 + u64::from(max) / 2) / u64::from(max);
    u8::try_from(scaled.min(100)).unwrap_or(100)
}

/// A percentage as a raw value, never below the floor and never above the top.
///
/// A `max` of zero answers zero. It is not a range, and every door refuses it
/// before getting here — `read_in` and `write_in` on a panel, `ddc::understand`
/// on a cable — but this function is `pub`, the number on the DDC side comes
/// off the wire, and `clamp(floor_raw(0), 0)` is `clamp(1, 0)`, which PANICS.
/// A panic here is not a wrong pixel: it kills the reader thread, and the only
/// thing on screen is that every row stops changing.
#[must_use]
pub fn raw_of(percent: u8, max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    let wanted = percent.clamp(FLOOR, 100);
    let scaled = (u64::from(max) * u64::from(wanted) + 50) / 100;
    u32::try_from(scaled)
        .unwrap_or(max)
        // THE FLOOR IS ITSELF CLAMPED. `floor_raw` is `.max(1)`, so on a screen
        // whose maximum is 1 the floor would be the same as the top — fine —
        // but the pair must never arrive at the clamp the wrong way round.
        .clamp(floor_raw(max).min(max), max)
}

/// The lowest raw value that is still a lit screen.
///
/// At least one unit: on a panel whose maximum is 24 the floor percentage
/// rounds to zero, and zero is the thing this exists to avoid.
#[must_use]
pub fn floor_raw(max: u32) -> u32 {
    let scaled = (u64::from(max) * u64::from(FLOOR) + 50) / 100;
    u32::try_from(scaled).unwrap_or(1).max(1)
}

/// Moves a percentage by a step, staying inside the range.
///
/// The keys on the keyboard use it, and so does the row: one arithmetic, so
/// holding the key and holding the arrow arrive at the same numbers.
#[must_use]
pub fn stepped(percent: u8, by: i16) -> u8 {
    let moved = i32::from(percent) + i32::from(by);
    u8::try_from(moved.clamp(i32::from(FLOOR), 100)).unwrap_or(FLOOR)
}

/// The raw value one step away — and it always MOVES.
///
/// # Why a percentage is not enough
///
/// A percentage only addresses a value that a percentage can name. Brute
/// forced over every ceiling from 1 to 3000: **100 is the smallest `max`
/// where every percent survives the round trip**, and below it whole bands of
/// them collapse onto one raw value. `acpi_video0` — the device the lone
/// backlight fallback in [`crate::connector`] exists to rescue — commonly
/// reports 7, 10 or 15 steps.
///
/// On a fifteen-step panel at raw 1, a step of minus five percent lands back
/// on raw 1, and the key does nothing, repeatedly and silently. So when the
/// percentage fails to move the raw value, this moves it by one unit in the
/// direction asked for — which on such a panel is the smallest change the
/// hardware can make anyway.
#[must_use]
pub fn stepped_raw(now: u32, max: u32, by: i16) -> u32 {
    // THE FLOOR NEVER EXCEEDS THE TOP: `floor_raw` is `.max(1)`, so without
    // this a `max` of zero makes a step DOWN answer 1 — a value above the
    // maximum — where `raw_of` answers 0. One rule, both directions.
    let floor = floor_raw(max).min(max);
    let wanted = raw_of(stepped(percent_of(now, max), by), max);
    if wanted != now || by == 0 {
        return wanted;
    }
    if by > 0 {
        now.saturating_add(1).min(max)
    } else {
        now.saturating_sub(1).max(floor)
    }
}

fn number(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// May this process write the file?
///
/// Asked by opening it, which on sysfs does nothing until something is
/// written — and answers with the kernel's own permission check instead of a
/// guess from the mode bits. Taken from `ronoctua_power::sysfs`.
fn can_write(path: &Path) -> bool {
    std::fs::OpenOptions::new().write(true).open(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_maximum_of_zero_does_not_panic() {
        // THE ARITHMETIC IS TOTAL, whatever a monitor said. `ddc::understand`
        // now refuses a zero maximum, and both sysfs doors refused it already
        // — but these are `pub`, the DDC number comes off a wire, and the old
        // `clamp(floor_raw(max), max)` was `clamp(1, 0)`: a panic, on a thread,
        // that killed the reader for the rest of the session.
        assert_eq!(raw_of(50, 0), 0);
        assert_eq!(raw_of(100, 0), 0);
        assert_eq!(stepped_raw(0, 0, 5), 0);
        // DOWN too: `floor_raw(0)` is 1, so the floor had to be clamped to the
        // top as well, or a step down would answer a value ABOVE the maximum.
        assert_eq!(stepped_raw(0, 0, -5), 0);
        // AND THE SMALLEST REAL RANGE still behaves: a maximum of 1 has a
        // floor of 1, and the clamp must not arrive the wrong way round there
        // either.
        assert_eq!(raw_of(1, 1), 1);
        assert_eq!(raw_of(100, 1), 1);
    }

    fn fake(name: &str, now: &str, max: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-backlight-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        // AN INVENTED DEVICE NAME. `read_in` takes the device's own directory
        // and reports its LAST component as `at`, so the name is part of what
        // is under test — and a real one, beside a real ceiling and a real
        // setting, names the hardware it was read off (invariants 2 and 13).
        let dir = dir.join("panel");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(NOW), now).unwrap();
        std::fs::write(dir.join(MAX), max).unwrap();
        dir
    }

    #[test]
    fn a_panel_with_a_large_ceiling_reads_as_a_percentage() {
        // A CEILING THAT IS NOT 100, which is the ordinary case and the whole
        // reason `percent()` exists: panels report thousands of raw steps, so
        // a raw value read as a percentage would be nonsense. 65535 is the
        // largest a 16-bit ceiling can be, and 51773 of it is 79%.
        //
        // INVENTED, like every fixture here: a device name, a ceiling and a
        // setting together name the hardware they were read off, and this
        // repository is public (invariants 2 and 13).
        let dir = fake("panel", "51773", "65535");
        let p = read_in(&dir).unwrap();
        assert_eq!(p.at, "panel");
        assert_eq!(p.now, 51773);
        assert_eq!(p.max, 65535);
        assert_eq!(p.percent(), 79);
    }

    #[test]
    fn a_percentage_survives_the_round_trip() {
        // Truncating instead of rounding loses a unit per pass, and a value
        // read and written on every frame walks to the floor over an
        // afternoon. Checked across the four ceilings real panels report.
        // 100 is the smallest ceiling where EVERY percent survives, brute
        // forced from 1 to 3000. Below it the round trip is not a property to
        // assert but a defect to work around, which `stepped_raw` does.
        for max in [100u32, 255, 4437, 65535, 96000] {
            for percent in FLOOR..=100 {
                let raw = raw_of(percent, max);
                assert_eq!(
                    percent_of(raw, max),
                    percent,
                    "{percent}% of {max} came back as {}",
                    percent_of(raw, max)
                );
            }
        }
    }

    #[test]
    fn nothing_reaches_a_black_screen() {
        // The whole reason `FLOOR` exists: zero is a screen you cannot find
        // the key on to undo it.
        for max in [24u32, 255, 65535] {
            assert!(raw_of(0, max) >= 1, "{max}");
            assert!(raw_of(FLOOR, max) >= 1, "{max}");
            assert_eq!(stepped(FLOOR, -50), FLOOR);
            assert_eq!(stepped(3, -5), FLOOR);
        }
        // And a panel so coarse that one percent rounds to nothing still
        // lands on a lit unit.
        assert_eq!(raw_of(FLOOR, 24), 1);
    }

    #[test]
    fn a_step_moves_a_coarse_panel_that_a_percentage_cannot() {
        // The defect this catches: on a fifteen-step panel a percentage step
        // lands on the raw value it started from, and the brightness key does
        // nothing, over and over, with nothing saying why. Brute-forced: 100
        // is the smallest ceiling where every percent round-trips, and
        // `acpi_video0` reports 7, 10 or 15.
        for max in [7u32, 10, 15, 24] {
            for now in 1..=max {
                let up = stepped_raw(now, max, 5);
                let down = stepped_raw(now, max, -5);
                if now < max {
                    assert!(up > now, "max {max}: {now} did not go up ({up})");
                }
                if now > floor_raw(max) {
                    assert!(down < now, "max {max}: {now} did not go down ({down})");
                }
                assert!(up <= max && down >= floor_raw(max));
            }
        }
        // On a panel a percentage CAN address, the percentage still decides —
        // the nudge is a floor under the gesture, not a second arithmetic.
        assert_eq!(stepped_raw(raw_of(50, 65535), 65535, 5), raw_of(55, 65535));
        // And a step of nothing moves nothing.
        assert_eq!(stepped_raw(100, 255, 0), raw_of(percent_of(100, 255), 255));
        // The floor still holds: it can go down, but never dark.
        assert_eq!(stepped_raw(floor_raw(15), 15, -50), floor_raw(15));
    }

    #[test]
    fn a_step_stays_inside_the_range() {
        assert_eq!(stepped(50, 5), 55);
        assert_eq!(stepped(98, 5), 100);
        assert_eq!(stepped(100, 5), 100);
        assert_eq!(stepped(50, -5), 45);
        assert_eq!(stepped(2, -5), FLOOR);
    }

    #[test]
    fn a_write_answers_with_the_file_and_not_with_the_request() {
        // A driver may quantise or clamp. Showing the request would put a
        // number on screen that the panel is not at.
        let dir = fake("write", "100", "255");
        let after = write_in(&dir, 200).unwrap();
        assert_eq!(after.now, 200);
        assert_eq!(std::fs::read_to_string(dir.join(NOW)).unwrap(), "200");
        // Out of range on both sides, clamped by the writer and not by the caller.
        assert_eq!(write_in(&dir, 9_999).unwrap().now, 255);
        assert_eq!(write_in(&dir, 0).unwrap().now, floor_raw(255));
    }

    #[test]
    fn a_directory_that_is_not_a_backlight_is_not_one() {
        let dir = std::env::temp_dir().join("ronoctua-backlight-absent/nothing");
        assert!(read_in(&dir).is_none());
        assert!(write_in(&dir, 10).is_err());
    }

    #[test]
    fn a_maximum_of_zero_is_refused_instead_of_dividing_by_it() {
        // A stub driver reports it, and every percentage computed from it
        // would divide by zero.
        let dir = fake("zero", "0", "0");
        assert!(read_in(&dir).is_none());
        assert_eq!(percent_of(10, 0), 0);
    }

    #[test]
    fn a_value_above_the_ceiling_is_read_as_the_ceiling() {
        // It happens after a suspend on some drivers: `brightness` keeps a
        // value the new `max_brightness` no longer allows, and a percentage
        // above 100 would be drawn as one.
        let dir = fake("over", "999", "255");
        let p = read_in(&dir).unwrap();
        assert_eq!(p.now, 255);
        assert_eq!(p.percent(), 100);
    }
}
