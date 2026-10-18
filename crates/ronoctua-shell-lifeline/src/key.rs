//! The text keys this crate says (`i18n/<language>.ftl`) and those of every
//! crate it stands on that speaks: one constant per key, written from the key
//! NAMES by `nix/text-keys.nix` (`nix/scripts/gen-keys.py` in a checkout), and
//! the keys a program's start guard checks its texts against (`sets`).

// A crate reached through two of the crates it stands on is re-exported
// twice, and the second is redundant.
#![allow(missing_docs, unused_imports)]

include!(concat!(
    env!("RONOCTUA_KEYS_DIR"),
    "/",
    env!("CARGO_PKG_NAME"),
    ".rs"
));
