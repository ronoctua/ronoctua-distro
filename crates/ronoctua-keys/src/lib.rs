//! Independent keyboards — the remapper of Independent mode.
//!
//! # What this is for
//!
//! Together mode is keyd: it grabs every keyboard and emits ONE virtual
//! keyboard, which is the right answer for one person and the reason two
//! people cannot type at once — the compositor sees one keyboard, on one
//! seat. Independent mode swaps keyd for this: every keyboard UNIT is grabbed
//! and re-emitted by a virtual twin of its own ([`device`]), so the compositor
//! sees one keyboard per unit and can put each on the seat of its cursor.
//!
//! # The same vocabulary, the same meaning
//!
//! A unit's remaps are keyd's words from `assets/keyboards.nix` — the ones the
//! screen already offers — and [`engine`] gives them keyd's meaning, so a key
//! does the same thing in both modes.
//!
//! # And the leader never goes through the compositor here
//!
//! See [`wire`]: the tap of a unit's leader key is told to the footer over a
//! socket, with the unit's name, and the footer takes that ONE keyboard's keys
//! through it — never the Wayland keyboard, which the compositor would take
//! from every seat at once.

pub mod client;
pub mod codes;
pub mod config;
pub mod daemon;
pub mod device;
pub mod engine;
pub mod wire;
