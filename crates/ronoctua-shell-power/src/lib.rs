//! `ronoctua-shell-power` — power (`LEADER s p`): the battery, the charger,
//! the profile, sleep and the machine's four ways out.
//!
//! Everything the view is: what it knows and what a key does on each row
//! ([`power`]), its window ([`power_window`]) and its region of the footer
//! ([`power_region`]) — the two doors, each carrying out the view's errands
//! through the one handler ([`power_region::carry_out`]). A crate of its own
//! so a line of it recompiles it and the binary, and nothing of the footer or
//! of any other view.

#![forbid(unsafe_code)]

// THE KIT AND THE DOOR, by the names the modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, region};
use ronoctua_shell_kit::{bindings, chrome, echo, fields};
// AND THE POWER DAEMONS' WORDS AND MARKS, which the footer says too: a crate
// under both.
use ronoctua_shell_power_words::power_words;

pub mod key;
pub mod power;
pub mod power_region;
pub mod power_window;

/// THE POWER VIEW'S WINDOW the binary runs by name (`ronoctua-shell
/// power`): the SAME `power::View` the footer draws from.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "power",
    run: power_door,
}];

/// `ronoctua-shell power` — the power view in a window of its own.
fn power_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    power_window::host_it().context("host the power view")
}
