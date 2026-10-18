//! `ronoctua-shell-keep` — the belongings, as the rest of the shell touches
//! and says them.
//!
//! What the file manager keeps in the belongings (`keep`, through
//! `belongings.json` and the belongings crate's own types), and the word of
//! every closed set of that crate (`belongings_words`), which the belongings'
//! view, the file manager and the seal all say — and whether a change the
//! software centre makes to the person's signed declarations asks the
//! belongings' passphrase first (`keep::signing_asks`). Under all four, and
//! on no other shell crate.

#![deny(unsafe_code)]

pub mod belongings_words;
pub mod keep;
pub mod key;
