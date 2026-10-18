//! `ronoctua-shell-transfers` — the transfer queue's panel, `LEADER t t`.
//!
//! The view (`transfers`): how a job is drawn and which key does what to it,
//! a function of the jobs it is handed. The footer's half of it
//! (`transfers_region`): the panel as a `region::Region` the footer hosts by
//! its id, its one errand handler, and the eight commands that put what was
//! copied into the queue. Its other door is a WASM buffer (`LEADER t w`),
//! which shares the queue's own table of keys.
//!
//! A feature crate: it stands on the kit, the door and the work, and names
//! neither the footer nor another feature.

#![deny(unsafe_code)]

// THE FOUNDATION, by the names these modules already use (`crate::chrome::…`).
use ronoctua_shell_door::{door, launch, queue, region};
use ronoctua_shell_kit::{bindings, chrome, echo, fields, sizes};
use ronoctua_shell_work::{jobs, software_words};
// What only the tests name: the doorway they build, and what the jobs they
// plant report and are about.
#[cfg(test)]
use ronoctua_shell_door::sudo;
#[cfg(test)]
use ronoctua_shell_keep::belongings_words;
#[cfg(test)]
use ronoctua_shell_kit::wake;
#[cfg(test)]
use ronoctua_shell_report::report;

pub mod key;
pub mod transfers;
pub mod transfers_region;
