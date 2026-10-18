//! `ronoctua-shell-agents` — the person's LLM accounts (`LEADER m a`):
//! Claude Code and Codex, their plans, limits, usage and tokens, a session
//! and a login of each.
//!
//! One feature, one crate: the view (`agents`, with its words in
//! `agents_words`), its window (`agents_window`) and its camera
//! (`agents_photograph`), its region in the footer (`agents_region`) and its
//! keys (`agents::KEYS`). Both doors hold the view open (`agents::Open`) and
//! carry out what it asks through one function (`agents::carry_out`). It
//! stands on the kit and the door alone: it names no other feature and not
//! the footer, which knows its region by its id.

#![deny(unsafe_code)]

// THE KIT AND THE DOOR, by the names these modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{brainlink, door, photo, region};
use ronoctua_shell_kit::{bindings, chrome, consent_rows, echo, fields, line, sizes, wake, when};
// The tests reach the sudo bar's gate.
#[cfg(test)]
use ronoctua_shell_door::sudo;

pub mod agents;
pub mod agents_photograph;
// THE VIEW IN THE FOOTER'S REGION: the region the binary registers.
pub mod agents_region;
pub mod agents_window;
pub mod agents_words;
pub mod key;

/// THE LLM ACCOUNTS' WINDOW the binary runs by name (`ronoctua-shell agents
/// [n]`, `LEADER m a w`), on the account at place `n` when one is given: the
/// SAME `agents::View` the footer's region draws.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "agents",
    run: accounts_door,
}];

/// THE LLM ACCOUNTS' WINDOW'S CAMERA (`photograph --door agents`), over a
/// machine laid out under one folder.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "agents",
    take: agents_photograph::taken,
}];

/// `ronoctua-shell agents [n]` — on the account at place `n`, when it is one.
fn accounts_door(words: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let place = words.first().and_then(|n| n.parse::<usize>().ok());
    agents_window::host_it(place).context("host the LLM accounts")
}
