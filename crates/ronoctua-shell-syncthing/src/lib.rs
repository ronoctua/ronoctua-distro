//! `ronoctua-shell-syncthing` — Syncthing (`LEADER s s`).
//!
//! Everything the view is: what it knows and what a key does on each row
//! ([`syncthing`]), its window ([`syncthing_window`]) and its region of the
//! footer ([`syncthing_region`]) — the two doors, each carrying out the
//! view's errands through the one handler ([`syncthing_region::carry_out`]).
//! A crate of its own so a line of it recompiles it and the binary, and
//! nothing of the footer or of any other view.

#![forbid(unsafe_code)]

// THE KIT AND THE DOOR, by the names the modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, file_picker, region};
use ronoctua_shell_kit::{bindings, chrome, echo, fields, line, sizes, wake};
// AND SYNCTHING'S REFUSALS, which the footer says too: a crate under both.
use ronoctua_shell_syncthing_words::syncthing_words;

pub mod key;
pub mod syncthing;
pub mod syncthing_region;
pub mod syncthing_window;

/// SYNCTHING'S WINDOW the binary runs by name (`ronoctua-shell syncthing`).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "syncthing",
    run: syncthing_door,
}];

/// `ronoctua-shell syncthing` — the Syncthing view in a window of its own.
fn syncthing_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    syncthing_window::host_it().context("host the syncthing view")
}
