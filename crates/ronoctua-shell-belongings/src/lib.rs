//! `ronoctua-shell-belongings` — the belongings (`LEADER s v`): the person's
//! configuration, kept encrypted in a private repository they name.
//!
//! One view and its two doors — the footer's region and a window — over one
//! errand handler both carry out ([`belongings_region::carry_out`]), the
//! window's camera and the keymap's contribution
//! ([`belongings::KEYS`]). A crate of its own, so a line of the belongings
//! recompiles the belongings and what stands on them — the binary, the
//! update and the software centre — and nothing else.

#![deny(unsafe_code)]

// THE KIT, THE DOOR AND THE KEEP, by the names these modules already use
// (`crate::chrome::…`, `crate::door::…`, `crate::keep::…`).
use ronoctua_shell_door::{door, photo, region, sudo};
use ronoctua_shell_keep::{belongings_words, keep};
use ronoctua_shell_kit::{
    bindings, chrome, consent_rows, echo, fields, line, security_words, sizes, wake, when,
};

pub mod belongings;
pub mod belongings_photograph;
pub mod belongings_region;
pub mod belongings_window;
pub mod key;

/// THE BELONGINGS' WINDOW the binary runs by name (`ronoctua-shell
/// belongings`, `LEADER s v w`): the SAME `belongings::View` the footer's
/// region draws.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "belongings",
    run: belongings_door,
}];

/// THE BELONGINGS' WINDOW'S CAMERA (`photograph --door belongings`).
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "belongings",
    take: belongings_photograph::taken,
}];

/// `ronoctua-shell belongings` — the belongings in a window.
fn belongings_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    belongings_window::host_it().context("host the belongings")
}
