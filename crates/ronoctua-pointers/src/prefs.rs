//! What the person chose: `$XDG_CONFIG_HOME/ronoctua/pointers.json`.
//!
//! # EVERY ORDER IS A READ, ONE CHANGE, AND A WRITE
//!
//! Two doors run a thread each — the footer and the window — and each would
//! hold its own copy of this if it were read once. The display view paid
//! twice for that shape (see `ronoctua-display`'s `wrote_the_screens`). Here
//! no copy outlives an order: every order reads the file, changes
//! the one field the order names, and writes it back, so a door can never
//! write back a field it did not touch.

use std::collections::BTreeMap;

use ronoctua_compositor::{FocusFollows, PointerSettings};
use serde::{Deserialize, Serialize};

/// The file's name inside the config directory.
pub const FILE: &str = "pointers.json";

/// How many distinct cursor colours a theme solves. A cursor past the eighth
/// takes the colour of the one eight before it — the owner's choice, since
/// past eight colours stop being told apart at the size of a cursor.
pub const COLOURS: u8 = 8;

/// The cursor that is the compositor's own first seat. It always exists: it
/// holds the keyboards, and a device nobody placed falls onto it.
pub const FIRST: u16 = 1;

/// The cursor size a machine with no file draws at.
pub const SIZE: u16 = 24;

/// The sizes the cursor themes carry, and so the only ones the screen
/// offers — `nix/cursors.nix` builds exactly these, and a size between two
/// of them would be a scaled bitmap.
pub const SIZES: [u16; 4] = [24, 32, 48, 64];

/// What was chosen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Whether a pointing device nobody has placed gets a cursor of its own.
    pub detect: bool,
    /// Settings for every mouse, under each mouse's own.
    pub mice: PointerSettings,
    /// Settings for every touchpad, under each touchpad's own.
    pub touchpads: PointerSettings,
    /// Each device that was ever seen, by the compositor's identifier.
    pub devices: BTreeMap<String, DevicePrefs>,
    /// Each cursor, by its number. Cursor [`FIRST`] is the first seat.
    pub cursors: BTreeMap<u16, CursorPrefs>,
    /// The cursor size, in pixels.
    pub size: u16,
    /// Hide a still cursor after this many milliseconds; `0` never does.
    pub hide_after_ms: u32,
    /// Hide the cursor while typing.
    pub hide_while_typing: bool,
    /// Whether focus follows the pointer — one setting for every cursor.
    pub focus: FocusFollows,
    /// Which cursor the keyboards type through.
    pub typing: crate::typing::Typing,
    /// Whether a virtual machine's own pointers move anything while a real
    /// one is present (`plan::plan_with`). OFF, the owner's choice.
    pub emulated: bool,
    /// Every mouse and touchpad moves ONE cursor, whatever each remembers.
    /// Where each device was is kept, so turning this off puts every hand
    /// back on its own cursor.
    pub together: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            // OFF, the owner's choice: a desk with several pointers is not
            // the common case, and one cursor for every hand is what anyone
            // expects on a first boot. `LEADER s i` turns it on.
            detect: false,
            mice: PointerSettings::default(),
            // WHAT THE COMPOSITOR'S CONFIG USED TO SAY by hand, in
            // `nix/modules/scroll.nix`: a tap is a click and content follows
            // the fingers. It lives here now, in one place, and the screen
            // can change it.
            touchpads: PointerSettings {
                tap: Some(true),
                natural_scroll: Some(true),
                ..PointerSettings::default()
            },
            devices: BTreeMap::new(),
            cursors: BTreeMap::new(),
            size: SIZE,
            hide_after_ms: 0,
            hide_while_typing: false,
            // A CLICK FOCUSES, the owner's choice: a window changing under a
            // hand that only crossed it is a surprise, and with several
            // cursors it is several.
            focus: FocusFollows::No,
            // EVERY CURSOR'S CLICK MOVES THE FOCUS, the owner's choice: the
            // border and cursor 1's keyboards go where the last click went.
            typing: crate::typing::Typing::Latest,
            emulated: false,
            together: false,
        }
    }
}

/// One device's own choices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DevicePrefs {
    /// Whether it works at all.
    pub enabled: bool,
    /// Which cursor it drives. `None` until it is placed — by detection, or
    /// by the person.
    pub cursor: Option<u16>,
    /// Its own settings, over the ones for its kind.
    pub settings: PointerSettings,
    /// The name it gave itself when last seen, so a screen can list a device
    /// that is not plugged in right now by something other than an id.
    pub name: String,
}

impl Default for DevicePrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            cursor: None,
            settings: PointerSettings::default(),
            name: String::new(),
        }
    }
}

/// One cursor's own choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CursorPrefs {
    /// Whether it is drawn and its devices work.
    pub enabled: bool,
    /// Which of the [`COLOURS`] its inside is painted with, from `0`.
    pub colour: u8,
}

impl Default for CursorPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            colour: 0,
        }
    }
}

impl Preferences {
    /// The cursor numbered `n`, with the colour it would be born with when it
    /// has not been given one.
    #[must_use]
    pub fn cursor(&self, n: u16) -> CursorPrefs {
        self.cursors.get(&n).copied().unwrap_or(CursorPrefs {
            enabled: true,
            colour: born_colour(n),
        })
    }
}

/// The colour a cursor is born with: its number, round the palette.
#[must_use]
pub fn born_colour(n: u16) -> u8 {
    u8::try_from((n.max(FIRST) - FIRST) % u16::from(COLOURS)).unwrap_or(0)
}

/// The directory the file lives in — asked once, here, so no door finds it
/// on its own.
#[must_use]
pub fn home() -> Option<std::path::PathBuf> {
    ronoctua_store::config_dir()
}

/// Where the file lives.
#[must_use]
pub fn store() -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(home(), FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_with_no_file_has_one_cursor_and_taps() {
        let p = Preferences::default();
        assert!(!p.detect);
        assert_eq!(p.focus, FocusFollows::No);
        assert_eq!(p.touchpads.tap, Some(true));
        assert_eq!(p.size, SIZE);
    }

    #[test]
    fn a_file_from_an_older_version_still_reads() {
        let old: Preferences = serde_json::from_str("{}").unwrap();
        assert_eq!(old, Preferences::default());
        let half: Preferences =
            serde_json::from_str(r#"{"devices":{"1:2:M":{"cursor":3}}}"#).unwrap();
        assert!(half.devices["1:2:M"].enabled, "enabled when not said");
        assert_eq!(half.devices["1:2:M"].cursor, Some(3));
    }

    #[test]
    fn the_colours_go_round_after_eight() {
        assert_eq!(born_colour(1), 0);
        assert_eq!(born_colour(8), 7);
        assert_eq!(born_colour(9), 0, "the ninth reuses the first");
        assert_eq!(Preferences::default().cursor(10).colour, 1);
    }

    #[test]
    fn the_whole_thing_survives_the_disk() {
        let mut p = Preferences::default();
        p.devices.insert(
            "1:2:Mouse".into(),
            DevicePrefs {
                cursor: Some(2),
                ..DevicePrefs::default()
            },
        );
        p.cursors.insert(
            2,
            CursorPrefs {
                enabled: false,
                colour: 5,
            },
        );
        let text = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Preferences>(&text).unwrap(), p);
    }
}
