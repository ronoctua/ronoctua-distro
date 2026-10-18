//! `ronoctua-shell-audio` — sound (`LEADER s a`).
//!
//! Everything the view is: what it knows and what a key does on each row
//! ([`audio`]), its window ([`audio_window`]) and its region of the footer
//! ([`audio_region`]) — the two doors, each carrying out the view's errands
//! through the one handler ([`audio_region::carry_out`]) and opening the
//! microphone the cursor is on through the one meter
//! ([`audio_region::Meter`]). A crate of its own so a line of it recompiles
//! it and the binary, and nothing of the footer or of any other view.

#![forbid(unsafe_code)]

// THE KIT AND THE DOOR, by the names the modules already use
// (`crate::chrome::…`, `crate::door::…`).
use ronoctua_shell_door::{door, region};
use ronoctua_shell_kit::{bindings, chrome, echo, fields, sizes};
// AND PIPEWIRE'S REFUSALS, which the footer says too: a crate under both.
use ronoctua_shell_audio_words::audio_words;

pub mod audio;
pub mod audio_region;
pub mod audio_window;
pub mod key;

/// THE SOUND VIEW'S WINDOW the binary runs by name (`ronoctua-shell audio`).
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "audio",
    run: sound_door,
}];

/// `ronoctua-shell audio` — the sound view in a window of its own.
fn sound_door(_: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    audio_window::host_it().context("host the sound view")
}
