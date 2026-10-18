//! `ronoctua-shell-display` — the screens' view (`LEADER s d`).
//!
//! Everything the view is: what it knows ([`display`]), what it says
//! ([`display_words`]), its window ([`display_window`]) and its region of the
//! footer ([`display_region`]) — the two doors, each carrying out the view's
//! errands through the one handler ([`display_region::carry_out`]). A crate of
//! its own so a line of it recompiles it and the binary, and nothing of the
//! footer or of any other view. What the footer says of the screens too — a
//! refusal of their reader, the brightness keys' step — is the leaf under
//! both (`ronoctua-shell-display-words`).

#![forbid(unsafe_code)]

// THE KIT AND THE DOOR, by the names the modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, photo, region};
use ronoctua_shell_kit::{bindings, chrome, daemon, echo, fields, toolkits};
// THE SCREENS AS THIS VIEW AND THE FOOTER SAY THEM: a leaf under both.
use ronoctua_shell_display_words::screens_words;
// What a test of the view reads back of a copy (`COPIED`), never the person's
// clipboard.
#[cfg(test)]
use ronoctua_shell_kit::clipboard_copy;

pub mod display;
pub mod display_region;
pub mod display_window;
pub mod display_words;
pub mod key;

/// THE SCREENS' SECOND DOOR the binary runs by name (`ronoctua-shell
/// display`), one monitor to a sheet of eighteen rows — the footer's capped
/// region is a glance, not a reading.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "display",
    run: screens_door,
}];

/// THE SCREENS' WINDOW'S CAMERA (`photograph --door display`), over a
/// scene handed in whole.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "display",
    take: display_window::camera,
}];

/// `ronoctua-shell display` — the screens' view in a window of its own.
fn screens_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    display_window::host_it().context("host the screens view")
}
