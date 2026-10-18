//! `ronoctua-shell-calc` — the calculator (`LEADER =`).
//!
//! The view both doors draw (`calc`: its modes, its keys, its lines, what a
//! click on any of them does; `calc_convert`, the converter; `calc_keypad`,
//! the window's cards), the window (`calc_window`), and the region the
//! footer hosts (`calc_region`) — with the ONE errand handler the window and
//! the region share (`calc_region::carry_out`), so what a key asks is done
//! alike in both doors.
//!
//! On the kit and the door only: a line of the calculator recompiles this
//! crate, the search that reads its answers and the binary — never the
//! footer nor another view.

#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names the modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, floating, region};
use ronoctua_shell_kit::{
    bars, bindings, chrome, consent_rows, daemon, echo, fields, line, text_select, wake, wheel,
    when,
};

pub mod calc;
pub mod calc_convert;
pub mod calc_keypad;
pub mod calc_region;
pub mod calc_window;
pub mod key;

/// THE CALCULATOR'S WINDOW the binary runs by name (`ronoctua-shell calc`,
/// `LEADER = w`): the SAME `calc::View` the footer's region draws.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "calc",
    run: calc_door,
}];

/// `ronoctua-shell calc` — the calculator in a window.
fn calc_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    calc_window::host_it().context("host the calculator")
}
