//! `ronoctua-shell-clipboard` — the clipboard's history, as its two doors
//! show it (`LEADER c`).
//!
//! The view both doors draw (`clipboard_view`: its rows, its keys, the
//! sync's rows), the window (`clipboard_window`) and the region the footer
//! hosts (`clipboard_region`) — and between the two, the open history both
//! keep (`clipboard_region::Clip`) and the ONE errand handler they share
//! (`clipboard_region::carry_out`).
//!
//! THE HISTORY ITSELF IS THE FOOTER'S: one watcher of the clipboard, one
//! history, which both doors follow and act on over the same wire
//! (`clipboard_wire`) — the window through the control socket, the region
//! through its doorway.
//!
//! On the kit and the door only.

#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names the modules already use.
use ronoctua_shell_door::{clipboard_wire, door, photo, region};
use ronoctua_shell_kit::{bindings, chrome, echo, fields, ipc, line, sizes};

pub mod clipboard_region;
pub mod clipboard_view;
pub mod clipboard_window;
pub mod key;

/// THE CLIPBOARD'S HISTORY IN A WINDOW the binary runs by name
/// (`ronoctua-shell clipboard`): the footer's, followed over the control
/// socket.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "clipboard",
    run: history_door,
}];

/// THE CLIPBOARD WINDOW'S CAMERA (`photograph --door clipboard`), over a
/// scene handed in whole.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "clipboard",
    take: clipboard_window::camera,
}];

/// `ronoctua-shell clipboard` — the clipboard's history in a window.
fn history_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    clipboard_window::host_it().context("host the clipboard's window")
}
