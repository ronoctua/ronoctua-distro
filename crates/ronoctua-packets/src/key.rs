//! The keys of the book `packets` (`i18n/<language>.ftl`) — each a
//! `ronoctua_i18n::BookKey`, read from the book's own file and never from the
//! texts every program reads — and those of every crate this one stands on
//! that speaks: written from the key NAMES by `nix/text-keys.nix`
//! (`nix/scripts/gen-keys.py` in a checkout), with the book and its keys
//! (`BOOK`), which `ronoctua_i18n::Book::open` checks its file against.

// A crate reached through two of the crates it stands on is re-exported
// twice, and the second is redundant.
#![allow(missing_docs, unused_imports)]

include!(concat!(
    env!("RONOCTUA_KEYS_DIR"),
    "/",
    env!("CARGO_PKG_NAME"),
    ".rs"
));
