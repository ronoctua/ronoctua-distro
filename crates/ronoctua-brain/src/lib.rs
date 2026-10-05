//! The brain: what the machine does by itself, when it may do it, and what
//! happened.
//!
//! # Why a crate, and a daemon, of its own
//!
//! Because the questions it answers outlive every process that asks them. "Has
//! this machine been updated in the last week" is not a question the footer can
//! answer, because the footer is restarted on every file edit in the nested
//! loop and on every crash on the installed one. It is the same argument
//! `ronoctua-transfers` makes for a copy of forty gibibytes, one level up: a
//! copy must survive the screen, and so must a decision about when the machine
//! rebuilds itself.
//!
//! And because three processes want the same answer — the footer's modeline,
//! the brain's own screen in the expanded region, and the window that screen
//! opens in. A schedule living inside the footer would be invisible to the
//! other two, and each would grow its own.
//!
//! # What it does NOT do
//!
//! - **It does not read `/proc`.** [`ronoctua_memory`] is the only crate that
//!   does, exactly as `ronoctua-scroll` is the only one that speaks to the
//!   compositor. The pulse arrives through [`pulse::Meter`], already read.
//! - **It reimplements no Nix logic** (invariant 5). It does not know what an
//!   update IS; it names a unit and reads what the unit reported.
//! - **It runs no privileged command.** It asks systemd to start a unit, and
//!   the unit — and the narrow polkit rule that lets it be started — belong to
//!   the flake. `systemctl` is found by STORE PATH and never by name: under a
//!   unit the `PATH` is minimal and a name fails far from its cause.
//! - **It holds no keyboard and creates no surface** (invariant 14). What is on
//!   screen is a `wasm32-wasip2` component in `buffers/brain/`, drawn by the
//!   footer like every other one.
//! - **It does not translate.** What crosses is a WORD — a task's id, a hold's
//!   name — and whoever has the bundle makes a sentence out of it (invariant 8).
//!
//! # THE DOOR — how a future feature is added
//!
//! This is the point of the crate. The owner asked for a place where "anything
//! in that space" becomes a feature rather than a new daemon, and a place is
//! only a place if the steps to use it are written down.
//!
//! A feature is a MODULE: a folder under `crates/ronoctua-brain/modules/<id>/`
//! whose `module.rs` answers the contract in [`module`] — a manifest (what it
//! is, its schedule, its marks, notices, actions and events), and whichever of
//! three faces it has: a run on a schedule ([`module::Scheduled`]), a watch on
//! the journal, a path, a child or a clock ([`module::Watcher`]), and what it
//! shows ([`module::Module::face`], [`module::Module::screen`]). The folder
//! carries its texts, its NixOS module, its checks and its `why.md`; the
//! generator reads the folder, and nothing outside it names the module.
//!
//! What a module does **not** have to write, because the core already owns it:
//! the queue (at most one run at a time, and what comes due meanwhile waits),
//! the metric gate (busy machine, battery), the announcement and its
//! countdown, the journal, the one follower of the system journal, the socket,
//! dismissing and snoozing its notices, and every screen that watches.
//!
//! Every feature lives in its own folder under `modules/`; the features that
//! came before modules moved there too, so nothing is written in place any
//! more.
//!
//! # The shape of it
//!
//! ```text
//!   footer      ──┐
//!   brain buffer ─┼─→  socket  →  Core: Shared (Mutex + Condvar + version),
//!   `ronoctua-brain look` ─┘        │     ONE registry, ONE journal
//!                                   ├─→ ONE runner thread → systemctl start
//!                                   │        ↑ pulse (ronoctua-memory)
//!                                   ├─→ ONE watch thread ← journalctl --follow,
//!                                   │        inotify, children, one clock
//!                                   └─→ thread per client → the whole mind
//! ```
//!
//! # Where the database would go
//!
//! Not here, and not yet. What is written today is two small JSON files, for
//! the reason [`journal`] gives: a few dozen records, rewritten when something
//! happens. The day the brain keeps a MEASUREMENT per minute — which is the
//! first feature that needs one — the seam is [`journal::Journal`], whose whole
//! surface is "give me the last run of this task" and "write this one down".
//! A store under `$XDG_STATE_HOME/ronoctua/brain/` replaces the file behind
//! those two calls, and nothing above them changes.

#![forbid(unsafe_code)]

pub mod client;
pub mod compiled;
pub mod hub;
pub mod journal;
pub mod mind;
pub mod module;
pub mod moment;
pub mod prefs;
pub mod pulse;
pub mod registry;
pub mod runner;
pub mod schedule;
pub mod serve;
pub mod systemd;
pub mod tail;
pub mod task;
pub mod watch;
pub mod wire;

#[cfg(test)]
mod fake;

/// Every module this build knows, made for one birth: the ones compiled from
/// their folders under `modules/` ([`compiled`]).
#[must_use]
pub fn every_module(birth: &Birth) -> Vec<Box<dyn Module>> {
    compiled::all(birth)
}

pub use journal::Journal;
pub use mind::{Announcement, Mind, Shared, Standing, StepState, TaskState};
// THE CONTRACT'S DOORS at the root; its vocabulary (`Face`, `Mark`, the
// module's `Notice`…) is named through `module::`, because the root's `Notice`
// is still the old one-line notice the footer reads.
pub use hub::Core;
pub use module::{Birth, Manifest, Module, Scheduled, Watcher};
pub use moment::{Clock, Moment, Offset};
pub use prefs::{Gate, PartOfDay, Prefs, TaskPrefs, Where};
pub use pulse::{Meter, Pulse};
pub use registry::Registry;
pub use runner::Runner;
pub use task::{
    Due, Hold, Kind, Outcome, OutcomeKind, Report, RunRecord, Sheet, Step, StepOutcome, Verdict,
    Watched,
};
pub use wire::{Ask, Knob, Say};

/// THE `#[path]` PILOT (brain.md §3, "Step 1"): the shim `build.rs` writes for
/// `tests/modules/pilot-sample`, loading it the way every module is loaded.
/// Tests only — the sample lives outside `modules/` and is never registered.
#[cfg(test)]
mod pilot {
    include!(concat!(env!("OUT_DIR"), "/pilot.rs"));
}
