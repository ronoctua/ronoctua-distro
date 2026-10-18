//! `ronoctua-shell-history` — the shell's history, and whether it leaves this
//! machine (`LEADER s h`, Atuin).
//!
//! Everything the view is: what it knows and what a key does on each row
//! ([`history`]), its window ([`history_window`]) and its region of the
//! footer ([`history_region`]) — the two doors, each carrying out the view's
//! errands through the one handler ([`history_region::carry_out`]). A crate
//! of its own so a line of it recompiles it and the binary, and nothing of
//! the footer or of any other view.

#![forbid(unsafe_code)]

// THE KIT AND THE DOOR, by the names the modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, region};
use ronoctua_shell_kit::{bindings, chrome, echo, fields, line};
// AND WHAT A FINISHED ORDER OF ATUIN SAYS, which the footer says too: a crate
// under both.
use ronoctua_shell_history_words::history_words;

pub mod history;
pub mod history_region;
pub mod history_window;
pub mod key;

/// THE SHELL HISTORY'S WINDOW the binary runs by name (`ronoctua-shell
/// history`).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "history",
    run: history_door,
}];

/// `ronoctua-shell history` — the shell history in a window of its own.
fn history_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    history_window::host_it()
}
