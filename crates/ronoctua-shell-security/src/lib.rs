//! `ronoctua-shell-security` — the security centre (`LEADER s e`): the
//! firewall, the passwordless window, the fingers and the keys, the
//! `/private` vault, encrypt anything, the online checks.
//!
//! One view and its two doors — the footer's region and a window — over one
//! errand handler both carry out ([`security_region::carry_out`]), the
//! window's camera and the keymap's contribution ([`security::KEYS`]). A
//! crate of its own, so a line of the centre recompiles the centre and what
//! stands on it — the binary and the file manager — and nothing else. The
//! passwordless window's mark on the modeline, which the footer draws, is
//! said in the centre's words under both (the kit's `security_words`).

#![deny(unsafe_code)]

// THE KIT, THE DOOR, THE REPORT AND THE WORK, by the names these modules
// already use (`crate::chrome::…`, `crate::seal::…`).
use ronoctua_shell_door::{door, file_picker, region, sudo};
use ronoctua_shell_kit::{bindings, chrome, echo, fields, line, security_words, sizes, wake, when};
use ronoctua_shell_report::report;
use ronoctua_shell_work::{jobs, seal, vault};

pub mod key;
pub mod security;
pub mod security_region;
pub mod security_window;

/// THE SECURITY CENTRE'S WINDOW the binary runs by name (`ronoctua-shell
/// security [section]`, `LEADER s e w`): the SAME `security::View` the
/// footer's region draws.
pub const DOORS: &[ronoctua_shell_kit::doors::Door] = &[ronoctua_shell_kit::doors::Door {
    name: "security",
    run: centre_door,
}];

/// THE SECURITY CENTRE'S WINDOW'S CAMERA (`photograph --door security`),
/// over a scene handed in whole.
pub const CAMERAS: &[ronoctua_shell_door::photo::Camera] = &[ronoctua_shell_door::photo::Camera {
    door: "security",
    take: security_window::photographed,
}];

/// `ronoctua-shell security [section]` — on the section named.
fn centre_door(words: &ronoctua_shell_kit::doors::Words) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let words = words.all();
    let (section, check) = security_window::words_of(&words)?;
    security_window::host_it(section, check).context("host the security centre")
}
