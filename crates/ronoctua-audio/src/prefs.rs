//! What is OURS to keep about sound, which is very little.
//!
//! # What is deliberately NOT here
//!
//! **The chosen device.** `default.configured.audio.sink` is PipeWire's own
//! record of what a person picked, `wpctl set-default` is what writes it, and
//! it survives reboots without us. Keeping a second copy would be a second
//! truth: the day they disagreed, the screen would show one and the sound
//! would come out of the other.
//!
//! Same for the volume, the mute and the profile. Every one of them is state
//! the daemon already persists, and a file of ours would be a race with it.
//!
//! What is left is what only this interface has an opinion about: how far one
//! press of a key moves the volume, whether it may go above what the hardware
//! calls full, and whether the line above the modeline appears at all.

use serde::{Deserialize, Serialize};

/// Where the person's word lives.
pub const FILE: &str = "audio.json";

/// The most the volume may ever be asked for, when going over is allowed.
///
/// PipeWire will take more, and more is a speaker asked to distort. This is
/// the ceiling every mixer that offers the gesture at all settles on.
pub const MOST: u8 = 150;

/// How far one press moves the volume, when nothing is chosen.
const STEP: u8 = 5;

/// What this interface keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// How many percentage points one press of a key moves the volume.
    pub step: u8,
    /// Whether the volume may go past 100%.
    pub over: bool,
    /// Whether changing the volume draws the line above the modeline.
    pub flash: bool,
}

impl Default for Preferences {
    /// Written out and not derived.
    ///
    /// `u8::default()` is ZERO, and a step of zero is a volume key that does
    /// nothing — the exact shape of a switch that does not switch. The
    /// `#[serde(default)]` above only covers a field missing from a file that
    /// EXISTS; `Store::read` hands back this for a file that is not there at
    /// all, which is every machine on its first day.
    fn default() -> Self {
        Self {
            step: STEP,
            over: false,
            flash: true,
        }
    }
}

impl Preferences {
    /// The most the volume may be set to under these preferences.
    #[must_use]
    pub const fn ceiling(self) -> u8 {
        if self.over { MOST } else { 100 }
    }

    /// One press of a volume key, from where it is now.
    ///
    /// It moves to the next MULTIPLE of the step rather than by the step, so
    /// a volume some other program left at 37% comes back onto the grid
    /// instead of walking 42, 47, 52 for ever.
    #[must_use]
    pub const fn stepped(self, now: u8, up: bool) -> u8 {
        let step = if self.step == 0 { STEP } else { self.step };
        let ceiling = self.ceiling();
        if up {
            let next = now / step * step + step;
            if next > ceiling { ceiling } else { next }
        } else if now == 0 {
            0
        } else {
            let under = now.div_ceil(step) * step;
            let next = if under > now {
                under - step
            } else {
                now - step
            };
            if next > ceiling { ceiling } else { next }
        }
    }
}

/// Where the file lives, for whoever wants to WATCH it.
///
/// # Why the shell asks this and not `config_dir()`
///
/// Because a function that reads or writes does not get to decide where. The
/// application finds the directory once and hands the answer over — otherwise
/// `cargo test` writes into the home of whoever runs it, which is what
/// happened to the manager's sidebar. `persistence-takes-its-root-as-a-parameter`
/// is what says so, and it caught the two doors of this view calling
/// `config_dir()` from inside their own set-up.
#[must_use]
pub fn home() -> Option<std::path::PathBuf> {
    ronoctua_store::config_dir()
}

/// The store the person's word lives in.
#[must_use]
pub fn store() -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(home(), FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_still_moves_the_volume() {
        // The failure this pins: a derived `Default` gives step 0, and every
        // volume key on a machine with no `audio.json` does nothing at all.
        let p = Preferences::default();
        assert!(p.step > 0);
        assert_ne!(p.stepped(50, true), 50);
        assert!(p.flash);
    }

    #[test]
    fn a_step_walks_the_grid() {
        let p = Preferences::default();
        assert_eq!(p.stepped(50, true), 55);
        assert_eq!(p.stepped(50, false), 45);
        // Off the grid, it comes back onto it rather than staying off.
        assert_eq!(p.stepped(37, true), 40);
        assert_eq!(p.stepped(37, false), 35);
    }

    #[test]
    fn the_ends_hold() {
        let p = Preferences::default();
        assert_eq!(p.stepped(0, false), 0);
        assert_eq!(p.stepped(3, false), 0);
        assert_eq!(p.stepped(98, true), 100);
        assert_eq!(p.stepped(100, true), 100);
    }

    #[test]
    fn going_over_is_a_choice_and_it_has_a_ceiling() {
        let quiet = Preferences::default();
        assert_eq!(quiet.ceiling(), 100);
        assert_eq!(quiet.stepped(100, true), 100);

        let loud = Preferences {
            over: true,
            ..Preferences::default()
        };
        assert_eq!(loud.ceiling(), MOST);
        assert_eq!(loud.stepped(100, true), 105);
        assert_eq!(loud.stepped(MOST, true), MOST);
    }

    #[test]
    fn on_the_grid_up_and_down_are_exact_inverses() {
        // WHAT WALKING THE GRID COSTS, written down so nobody "fixes" it. From
        // a value some other program left off the grid, up then down does NOT
        // come back: 38 goes to 40 and then to 35. That is deliberate — it is
        // what every mixer does, and it is how a volume gets back onto round
        // numbers at all. Once there, the two keys are exact inverses, which
        // is the property that actually matters in use.
        let p = Preferences::default();
        for start in [0_u8, 5, 25, 50, 95, 100] {
            let up = p.stepped(start, true);
            if up != start {
                assert_eq!(p.stepped(up, false), start, "from {start}%");
            }
        }
        assert_eq!(p.stepped(38, true), 40);
        assert_eq!(p.stepped(40, false), 35);
    }

    #[test]
    fn a_hand_edited_zero_step_does_not_freeze_the_keys() {
        // A file is something a person edits, and `"step": 0` is a plausible
        // thing to type. It must not be a volume key that stopped working.
        let broken = Preferences {
            step: 0,
            ..Preferences::default()
        };
        assert_eq!(broken.stepped(50, true), 55);
    }

    #[test]
    fn a_store_with_no_directory_keeps_nothing() {
        let nowhere: ronoctua_store::Store<Preferences> = ronoctua_store::Store::at(None, FILE);
        assert!(nowhere.path().is_none());
        assert_eq!(nowhere.read(), Preferences::default());
    }
}
