//! `ronoctua-shell-bluetooth` — the radio's view (`LEADER s b`).
//!
//! Everything the view is: what it knows ([`bluetooth`]), how it reads a key
//! ([`bluetooth_words`]), its window ([`bluetooth_window`]) and its region of
//! the footer ([`bluetooth_region`]) — the two doors, each carrying out the
//! view's errands through the one handler ([`bluetooth_region::carry_out`]).
//! A crate of its own so a line of it recompiles it and the binary, and
//! nothing of the footer or of any other view. What the footer says of the
//! radio too — its marks, its question, its refusals — is the leaf under
//! both (`ronoctua-shell-bluetooth-words`).

#![forbid(unsafe_code)]

// THE KIT AND THE DOOR, by the names the modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, photo, region};
use ronoctua_shell_kit::{bindings, chrome, echo, fields, line, network_refusal};
// THE RADIO AS BOTH DOORS AND THE FOOTER SAY IT — the marks, the pairing
// question, the refusals, the adapter's three orders: a leaf under this view
// and the footer, which keeps the radio and names no feature.
use ronoctua_shell_bluetooth_words::bluez_words;

pub mod bluetooth;
pub mod bluetooth_region;
pub mod bluetooth_window;
pub mod bluetooth_words;
pub mod key;

/// THE RADIO'S SECOND DOOR the binary runs by name (`ronoctua-shell
/// bluetooth`), one device to a line and a fact to a row — the footer's
/// capped region is a glance, not a reading.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "bluetooth",
    run: radio_door,
}];

/// THE RADIO'S WINDOW'S CAMERA (`photograph --door bluetooth`), over a
/// planted machine.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "bluetooth",
    take: bluetooth_window::camera,
}];

/// `ronoctua-shell bluetooth` — the radio's view in a window of its own.
fn radio_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    bluetooth_window::host_it().context("host the bluetooth view")
}
