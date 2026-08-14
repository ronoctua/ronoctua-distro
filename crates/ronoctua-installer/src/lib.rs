//! The installer — section 7.1, milestone 8.
//!
//! Two parts, separated on purpose:
//!
//! - [`model`] and [`build_plan`] are **pure**: the questions, the validation and the
//!   command sequence. Testable without a TUI, without a disk and without root, and it is where
//!   the milestone's criteria live.
//! - [`tui`] reads and draws, and [`executor`] runs. Neither of them decides
//!   anything: the TUI's state is testable without a terminal, and the executor's pure
//!   functions are testable without a disk.
//!
//! Node Nix logic is reimplemented here (invariant 5): the installer
//! writes a `host.nix` and calls `disko` and `nixos-install`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod executor;
pub mod model;
pub mod plan;
pub mod tui;
