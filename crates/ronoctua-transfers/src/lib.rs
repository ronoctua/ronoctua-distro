//! The queue of everything that takes a while — and the switch that stops it.
//!
//! # Why a crate, and a daemon, of its own
//!
//! Because the same list has to be visible from THREE processes: the footer,
//! which draws the modeline's percentage and the panel in its expanded region;
//! the file manager's window, which is a re-exec of the shell's binary and
//! therefore somebody else entirely; and — once it exists — the wasm buffer's
//! host window. A queue living inside the footer would be invisible to the
//! other two, and each of them would grow its own.
//!
//! And because a copy of forty gibibytes must not die when the footer does. In
//! the nested development loop the footer is restarted on **every file edit**
//! (`nix/dev.nix`), and on the installed machine a shell that crashes should
//! cost a repaint and not an hour of copying.
//!
//! # What this crate does NOT do
//!
//! It does not draw, does not translate, and does not touch the filesystem
//! itself: every local operation is [`ronoctua_files::watch`]'s, watched. What
//! it owns is *which jobs exist, how far along each is, and who may stop them*.
//!
//! A row's label comes out as a text KEY plus data — [`job::Work::word`],
//! [`job::Stage::word`] — and whoever has the bundle turns it into a sentence
//! (invariant 8). Nothing here knows what language anybody reads.
//!
//! # The shape of it
//!
//! ```text
//!   ronoctua-shell ──┐
//!   files window   ──┼─→  socket  →  Shared (Mutex + Condvar)
//!   buffer host    ──┘                 │
//!                                      ├─→ thread per job → ronoctua_files::watch
//!                                      └─→ thread per watcher → the whole list
//! ```

#![forbid(unsafe_code)]

pub mod aria2;
pub mod atlas;
pub mod auth;
pub mod belongings;
pub mod browser;
pub mod client;
pub mod container;
pub mod engine;
pub mod incus;
pub mod job;
pub mod ledger;
pub mod local;
pub mod media;
pub mod scan;
pub mod seal;
pub mod secrets;
pub mod serve;
pub mod software;
pub mod stick;
pub mod torrent;
pub mod where_to;
pub mod wire;
// ── anchor: L13 ──
// ── anchor: L17 ──
// ── anchor: L22 ──

pub use job::{Engine, Identity, Job, Measure, Stage, Want, Work};
pub use ledger::Shared;
pub use wire::{Ask, Say};

/// An executable file written by a child `/bin/sh` and never by this
/// process, for the tests' stand-in programs. The tests run in parallel: a
/// file this process holds open for writing while another test's thread
/// forks is inherited by that fork until it execs, and running the file in
/// that window fails with `ETXTBSY` ("Text file busy"). It made the incus and
/// seal engines' tests fail at random in a whole-workspace run.
#[cfg(test)]
pub(crate) fn write_executable(path: &std::path::Path, body: &str) {
    let status = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            "printf '%s' \"$2\" > \"$1\" && chmod 700 \"$1\"",
            "sh",
        ])
        .arg(path)
        .arg(body)
        .status()
        .expect("a shell writes the stand-in");
    assert!(status.success(), "{}", path.display());
}
