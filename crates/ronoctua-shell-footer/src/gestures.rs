//! Hand gestures seen by the camera (`LEADER m g`): the footer's half.
//!
//! # The studio sees, `ronoctua-gestures` understands, the footer acts
//!
//! The studio is the one process that opens the camera, and it answers only
//! WHERE the hands are ([`ronoctua_studio::client::hands`]). What a pose means
//! is the pure reader of `ronoctua-gestures`. What it DOES happens here,
//! because this is the process with a Wayland connection: the pointer through
//! [`crate::virtual_pointer`], `Esc` and the arrows through
//! [`crate::virtual_keys`], and every command through `run_with`, the same
//! path a key of the leader takes.
//!
//! # Off after a restart
//!
//! Nothing here is remembered but the calibrated area. A camera that turns
//! itself on at login is not a feature that is "easily turned off" — it is
//! one that has to be remembered to be turned off.

use crate::key;
use ronoctua_gestures::{Area, Direction, Event, Hand, Reading};
use serde::{Deserialize, Serialize};

/// The file the calibrated area lives in.
pub const FILE: &str = "gestures.json";

/// What is remembered.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// The part of the picture the screen maps onto, as calibrated:
    /// left, top, right, bottom.
    pub area: Option<[f32; 4]>,
}

impl Prefs {
    /// The area to map, calibrated or the default.
    #[must_use]
    pub fn area(&self) -> Area {
        self.area
            .map_or_else(Area::default, |[left, top, right, bottom]| Area {
                left,
                top,
                right,
                bottom,
            })
    }
}

/// The store the calibrated area lives in.
#[must_use]
pub fn store() -> ronoctua_store::Store<Prefs> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
}

/// The four corners calibration asks for, in order, and each one's sentence.
pub const CORNERS: [&str; 4] = [
    key::ECHO_GESTURES_CALIBRATE_TOP_LEFT,
    key::ECHO_GESTURES_CALIBRATE_TOP_RIGHT,
    key::ECHO_GESTURES_CALIBRATE_BOTTOM_RIGHT,
    key::ECHO_GESTURES_CALIBRATE_BOTTOM_LEFT,
];

/// Gestures while they are on.
pub(crate) struct On {
    /// Dropping it closes the camera.
    #[expect(dead_code, reason = "held for its Drop, which closes the camera")]
    pub feed: ronoctua_studio::client::HandsFeed,
    /// The meaning of the poses.
    pub reader: ronoctua_gestures::Reader,
    /// The corners taken so far, while calibrating.
    pub calibrating: Option<Vec<[f32; 2]>>,
    /// The machine's keymap, for `Esc` and the arrows.
    pub keymap: Option<String>,
    /// Whether a hand has been seen since they were turned on — logged once.
    pub saw_a_hand: bool,
}

/// One reading of the studio's, as the reader takes it. A hand with a
/// point count other than 21 is dropped: the models have 21.
#[must_use]
pub fn reading_of(seen: &ronoctua_studio::wire::Seen) -> Reading {
    Reading {
        at: seen.at,
        aspect: seen.aspect,
        hands: seen
            .hands
            .iter()
            .filter_map(|h| {
                let points: [[f32; 3]; 21] = h.points.clone().try_into().ok()?;
                Some(Hand {
                    points,
                    right: h.right,
                })
            })
            .collect(),
    }
}

/// The command an event runs, when it is one of the registry's.
#[must_use]
pub fn command_of(e: Event) -> Option<&'static str> {
    Some(match e {
        Event::Focus(Direction::Up) => "window-up",
        Event::Focus(Direction::Right) => "window-right",
        Event::Focus(Direction::Down) => "window-down",
        Event::Focus(Direction::Left) => "window-left",
        Event::Workspace(Direction::Up) => "workspace-previous",
        Event::Workspace(_) => "workspace-next",
        Event::Overview => "workspace-all",
        Event::Keyboard => "onscreen-keyboard",
        // THE COLUMN, and as a switch: the owner's choice — the rock sign opens
        // the player at the right of the screen and closes it again, the
        // same toggle as `LEADER p c`.
        Event::Player => "player-column",
        Event::Close => "buffer-close",
        _ => return None,
    })
}

/// The kernel code of a key an event sends.
#[must_use]
pub fn key_of(e: Event) -> Option<u16> {
    Some(match e {
        Event::Escape => crate::osk::Named::Esc.code(),
        Event::Arrow(Direction::Up) => crate::osk::Named::Up.code(),
        Event::Arrow(Direction::Right) => crate::osk::Named::Right.code(),
        Event::Arrow(Direction::Down) => crate::osk::Named::Down.code(),
        Event::Arrow(Direction::Left) => crate::osk::Named::Left.code(),
        _ => return None,
    })
}

/// What the echo area says a fired gesture did — every discrete one, so a
/// misfire is SEEN, and the pose that caused it learned.
#[must_use]
pub fn said_of(e: Event) -> Option<&'static str> {
    Some(match e {
        Event::Escape => key::ECHO_GESTURE_ESCAPE,
        Event::Arrow(_) => key::ECHO_GESTURE_ARROW,
        Event::Focus(_) => key::ECHO_GESTURE_FOCUS,
        Event::Workspace(_) => key::ECHO_GESTURE_WORKSPACE,
        Event::Overview => key::ECHO_GESTURE_OVERVIEW,
        Event::Keyboard => key::ECHO_GESTURE_KEYBOARD,
        Event::Player => key::ECHO_GESTURE_PLAYER,
        Event::Close => key::ECHO_GESTURE_CLOSE,
        Event::Off => key::ECHO_GESTURES_OFF,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_calibrated_area_is_the_one_mapped() {
        let p = Prefs {
            area: Some([0.1, 0.2, 0.9, 0.8]),
        };
        assert_eq!(p.area().left, 0.1);
        assert_eq!(Prefs::default().area(), Area::default());
    }

    #[test]
    fn a_hand_of_the_wrong_size_is_dropped() {
        let seen = ronoctua_studio::wire::Seen {
            aspect: 1.0,
            at: 5,
            hands: vec![
                ronoctua_studio::wire::SeenHand {
                    points: vec![[0.0; 3]; 21],
                    right: true,
                },
                ronoctua_studio::wire::SeenHand {
                    points: vec![[0.0; 3]; 3],
                    right: false,
                },
            ],
        };
        assert_eq!(reading_of(&seen).hands.len(), 1);
    }
}
