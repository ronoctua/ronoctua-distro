//! `ronoctua-shell-update` — the update (`LEADER s u`): the mark on the
//! modeline, the door that starts a run, and the view that watches one
//! happen; and the person's binary cache, a page of that view.
//!
//! One view and its two doors — the footer's region and a window — over one
//! errand handler both carry out ([`update_region::carry_out`]), the
//! window's camera and the keymap's contribution
//! ([`update_window::KEYS`]). A crate of its own, so a line of the update
//! recompiles the update and the binary, and nothing else. The mark the
//! footer draws while a run is under way, and the door a click on it opens,
//! are the leaf under both (`ronoctua-shell-update-words`). And the page of
//! the distro's apps that follow their releases ([`follow`]): the person's
//! option to let the routine update move them past the distro's pins. And
//! the machine's parts ([`parts`]): what arrives after the installation, and
//! the ways the person brings in a download that keeps failing.

#![deny(unsafe_code)]

// THE KIT, THE DOOR AND THE BELONGINGS, by the names these modules already
// use (`crate::chrome::…`, `crate::belongings::…`).
use ronoctua_shell_belongings::belongings;
use ronoctua_shell_door::{brainlink, door, file_picker, photo, region, sudo};
use ronoctua_shell_kit::{bindings, chrome, echo, fields, line, security_words, wake, wheel, when};
// THE MARK THE FOOTER DRAWS while a run is under way: a leaf under both.
use ronoctua_shell_update_words::update_mark;

pub mod binary_cache;
pub mod follow;
pub mod key;
pub mod parts;
pub mod update_region;
pub mod update_window;

/// THE UPDATE'S WINDOW the binary runs by name (`ronoctua-shell update`):
/// the update, live — the door the modeline's mark opens.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "update",
    run: update_door,
}];

/// THE UPDATE'S WINDOW OVER A WHOLE SCENE (`photograph --door
/// update-scene`) — the machine's records and its battery as well as the
/// reports.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "update-scene",
    take: update_window::photographed_scene,
}];

/// `ronoctua-shell update` — the update, live, in a window of its own.
///
/// It takes no argument BECAUSE it may name no step: what it draws is the
/// declared table the brain already carries and the two journals the units
/// write. It opens with nothing running too — a run that has finished is
/// exactly when somebody wants to read what it did.
fn update_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    update_window::host_it().context("host the update")
}
