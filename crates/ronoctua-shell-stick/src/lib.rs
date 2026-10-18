//! `ronoctua-shell-stick` — the installer on a USB stick (`LEADER s z`): the
//! latest version, the drives it can go on, and the write.
//!
//! One feature, one crate: the view (`stick`), its window (`stick_window`,
//! with its camera), its region in the footer (`stick_region`) and its keys
//! (`stick::KEYS`). Both doors hold the view open (`stick::Open`) and carry
//! out what it asks through one function (`stick::carry_out`). It stands on
//! the kit and the door alone: it names no other feature and not the footer,
//! which knows its region by its id.

#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names these modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, photo, region, sudo};
use ronoctua_shell_kit::{
    bindings, chrome, consent_rows, echo, fields, security_words, sizes, wake, when,
};

pub mod key;
pub mod stick;
// THE VIEW IN THE FOOTER'S REGION: the region the binary registers.
pub mod stick_region;
pub mod stick_window;

/// THE INSTALLER STICK'S WINDOW the binary runs by name (`ronoctua-shell
/// stick [--drive <id>]`, `LEADER s z w`), opened on a drive by the file
/// manager's `W`: the SAME `stick::View` the footer's region draws.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "stick",
    run: stick_door,
}];

/// THE INSTALLER STICK'S WINDOW'S CAMERA (`photograph --door stick`), over
/// readings handed in whole.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "stick",
    take: stick_window::photographed,
}];

/// `ronoctua-shell stick [--drive <id>]` — on the drive given.
fn stick_door(words: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let words = words.all();
    let drive = match words.as_slice() {
        [] => None,
        [flag, id] if flag == "--drive" => Some(id.clone()),
        _ => anyhow::bail!("usage: ronoctua-shell stick [--drive <id>]"),
    };
    stick_window::host_it(drive).context("host the installer stick")
}
