//! `ronoctua-shell-display-words` — the screens as the shell says them.
//!
//! What the footer, which keeps the screens' reader for the session, and the
//! screens' view (`ronoctua-shell-display`) both say (`screens_words`): a
//! refusal of the reader in a sentence, and the brightness keys' step. Under
//! both, and on no other shell crate.

#![deny(unsafe_code)]

pub mod key;
pub mod screens_words;
