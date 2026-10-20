//! `ronoctua-shell-lifeline` — the line that outlives the footer.
//!
//! Raised by systemd when the footer fails (`OnFailure=`): two lines saying
//! what happened and what can be done about it, in the footer's place, only
//! while the footer is dead. A crate of its own so the one screen that exists
//! when the footer does not depends on nothing of the footer's — the kit and
//! the door only.

// `deny`, not `forbid`: `lifeline.rs` needs ONE unsafe block to pass Wayland's
// pointers to wgpu, with its contract written beside it.
#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names the module already uses
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{cursor_shape, door, photo};
use ronoctua_shell_kit::{aim, chrome, hidpi, language, seats, toolkits};

pub mod key;
pub mod lifeline;

/// THE LINE THAT OUTLIVES THE FOOTER, the binary's door by name
/// (`ronoctua-shell lifeline <unit>`): raised by `OnFailure=` when
/// `ronoctua-footer.service` fails, and by the nested loop's starter when
/// the child exits badly. It does NOT go through the control socket, which
/// is the footer's: this exists precisely when the footer does not answer.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "lifeline",
    run: lifeline_door,
}];

/// THE LIFELINE'S CAMERA (`photograph --door lifeline`), over a scene
/// handed in whole.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "lifeline",
    take: lifeline::camera,
}];

/// `ronoctua-shell lifeline <unit>` — the line raised for that unit.
fn lifeline_door(words: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let unit = words
        .first()
        .context("usage: ronoctua-shell lifeline <unit>")?;
    lifeline::host_it(&unit).context("raise the lifeline")
}
