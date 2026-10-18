//! `ronoctua-shell-notifications` — the notification manager, `LEADER n n`.
//!
//! The manager (`notifications`): what happened, what a filter suppressed and
//! the catalogue of every notification and filter, with a sheet to adjust one
//! and a draft to write one's own — a cursor over lists it is handed, and no
//! record of its own. Two doors drive it: the footer's region
//! (`notifications_region`, a `region::Region` the footer hosts by its id,
//! which asks the footer's notification desk) and a window of its own
//! (`notifications_window`, which keeps the two files the footer watches);
//! what either is asked is carried out by one handler
//! (`notifications::carry_out`).
//!
//! A feature crate: it stands on the kit and the door, and names neither the
//! footer nor another feature.

#![deny(unsafe_code)]

// THE FOUNDATION, by the names these modules already use (`crate::chrome::…`).
use ronoctua_shell_door::{door, region};
use ronoctua_shell_kit::{
    actions, band, bars, bindings, chrome, desk, echo, fields, foreign, line, text_select, wheel,
};
// What only the tests name: the doorway they build.
#[cfg(test)]
use ronoctua_shell_door::sudo;
#[cfg(test)]
use ronoctua_shell_kit::{clipboard_copy, wake};

pub mod key;
pub mod notifications;
pub mod notifications_region;
pub mod notifications_window;

/// THE MANAGER'S WINDOW the binary runs by name (`ronoctua-shell
/// notifications`): the SAME `notifications::View` the footer draws from —
/// what differs is the window and its events.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "notifications",
    run: manager_door,
}];

/// `ronoctua-shell notifications` — the manager in a window of its own.
fn manager_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    notifications_window::host_it().context("host the manager")
}
