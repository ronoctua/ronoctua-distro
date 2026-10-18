//! `ronoctua-shell-manual` — the manuals, `LEADER h`.
//!
//! Their list and each manual open from it (`manual`): the shortcuts', made
//! from the keys the system binds at the moment of opening, and the
//! gestures', drawn. Two doors drive the one view — the footer's region
//! (`manual_region`, a `region::Region` the footer hosts by its id) and a
//! window of its own (`manual_window`) — and what either is asked is carried
//! out by one handler (`manual::carry_out`).
//!
//! A feature crate: it stands on the kit and the door, and names neither the
//! footer nor another feature.

#![deny(unsafe_code)]

// THE FOUNDATION, by the names these modules already use (`crate::chrome::…`).
use ronoctua_shell_door::{door, region};
use ronoctua_shell_kit::{actions, bindings, chrome, echo, fields, line, minibuffer};
// What only the tests name: the doorway they build.
#[cfg(test)]
use ronoctua_shell_door::sudo;
#[cfg(test)]
use ronoctua_shell_kit::wake;

pub mod key;
pub mod manual;
pub mod manual_region;
pub mod manual_window;

/// THE MANUALS' WINDOW the binary runs by name: `ronoctua-shell manual`
/// opens on their list, as `LEADER h` does, and `manual <word>` on one of
/// them — the word the footer's `C-w` hands.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "manual",
    run: manuals_door,
}];

/// `ronoctua-shell manual [<word>]` — a word that names no manual is
/// refused, never taken for the shortcuts'.
fn manuals_door(words: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let which = match words.first() {
        None => None,
        Some(word) => match manual::Which::of_word(&word) {
            Some(which) => Some(which),
            None => anyhow::bail!(
                "no manual is called `{word}`; the words are {}",
                manual::Which::ALL.map(manual::Which::word).join(", ")
            ),
        },
    };
    manual_window::host_it(which).context("host the manuals")
}
