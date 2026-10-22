//! The installer — section 7.1, milestone 8.
//!
//! Two parts, separated on purpose:
//!
//! - [`model`], [`plan`], [`gate`], [`release`], [`swap`] and [`access`] are
//!   **pure**: the questions, the validation, the command sequence, and what
//!   the network, the release and the person's repository answered.
//!   Testable without a TUI, without a disk, without the network and without
//!   root, and it is where the milestone's criteria live.
//! - [`tui`] reads and draws, [`reach`] asks the world what the questions
//!   check ([`job`]), and [`executor`] runs. None of them decides anything:
//!   the TUI's state is testable without a terminal, the jobs without a
//!   network, and the executor's pure functions without a disk.
//!
//! No Nix logic is reimplemented here (invariant 5): the installer writes
//! the machine's flake from the distro's template, a `host.nix`, and calls
//! `nix`, `disko`, `nixos-install` and the person's own programs.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod access;
pub mod executor;
pub mod gate;
pub mod job;
pub mod key;
pub mod model;
pub mod plan;
pub mod platform;
pub mod reach;
pub mod release;
pub mod swap;
pub mod tui;
