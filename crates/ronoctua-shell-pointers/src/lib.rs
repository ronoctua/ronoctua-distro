//! `ronoctua-shell-pointers` — mice, touchpads and every cursor (`LEADER s i`).
//!
//! Everything the view is: what it knows and what a key does on each row
//! ([`pointers`]), its window ([`pointers_window`]) and its region of the
//! footer ([`pointers_region`]) — the two doors, each carrying out the view's
//! errands through the one handler ([`pointers_region::carry_out`]). A crate
//! of its own so a line of it recompiles it and the binary, and nothing of
//! the footer or of any other view.

#![forbid(unsafe_code)]

// THE KIT AND THE DOOR, by the names the modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, region};
use ronoctua_shell_kit::{bindings, chrome, echo, fields};

pub mod key;
pub mod pointers;
pub mod pointers_region;
pub mod pointers_window;

/// MICE, TOUCHPADS AND EVERY CURSOR, in a window of their own the binary
/// runs by name (`ronoctua-shell pointers`).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "pointers",
    run: pointers_door,
}];

/// `ronoctua-shell pointers` — the pointers' view in a window of its own.
fn pointers_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    pointers_window::host_it().context("host the pointers view")
}
