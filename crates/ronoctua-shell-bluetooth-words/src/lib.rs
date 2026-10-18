//! `ronoctua-shell-bluetooth-words` — the radio as the shell says it.
//!
//! What the footer, which keeps the radio for the session (the pairing agent,
//! the modeline's device marks, the alarms), and the radio's view
//! (`ronoctua-shell-bluetooth`) both say and read (`bluez_words`). Under
//! both, and on no other shell crate: the footer names no feature, and a line
//! of the view recompiles the view and the binary alone.

#![deny(unsafe_code)]

pub mod bluez_words;
pub mod key;
