//! `ronoctua-shell-lock` — the lock screen.
//!
//! The session locked through the compositor's own lock
//! (`ronoctua_shell_door::session_lock`, invariant 3's exception of
//! 2026-10-31): every screen covered, every input the lock's, and the session
//! opened again by the account's password — or by a finger on the reader,
//! where there is one with a finger enrolled. `ronoctua-shell lock` is its own
//! process, started by the footer (`LEADER s q l`, or the idle watcher when
//! the person turned the automatic lock on), so a footer that dies leaves the
//! session locked; a lock that dies leaves it locked too, and its unit starts
//! another, which takes the session over.

#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names the module already uses
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, photo, session_lock};
use ronoctua_shell_kit::{bindings, chrome, daemon, line, text_select, theme};

pub mod key;
pub mod lock;

/// THE LOCK'S DOOR, by name (`ronoctua-shell lock`): run by the session's
/// unit (`ronoctua-lock.service`), or by the footer itself where there is no
/// unit (the nested loop, a test's machine).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: session_lock::LOCK_WORD,
    run: lock_door,
}];

/// THE LOCK SCREEN'S CAMERA (`photograph --door lock`), over a scene given in
/// words: no compositor is locked, and nothing is asked of PAM.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: session_lock::LOCK_WORD,
    take: lock::camera,
}];

/// `ronoctua-shell lock` — the session locked until the person opens it.
///
/// A lock that REFUSED to lock (no password's stack on this machine, no
/// account name, no lock in the compositor) ends with [`lock::REFUSED`], the
/// status its unit does not start again: the refusal would only repeat.
fn lock_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    match lock::host_it() {
        Ok(()) => Ok(()),
        Err(lock::Ended::Refused(why)) => {
            tracing::error!(why = %why, "the screen was NOT locked");
            eprintln!("the screen was not locked: {why}");
            std::process::exit(lock::REFUSED);
        }
        Err(lock::Ended::Failed(e)) => Err(e.context("the lock screen")),
    }
}
