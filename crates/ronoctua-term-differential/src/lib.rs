//! The differential test of the terminal engine — section 7.10, milestone 7's
//! criterion.
//!
//! # Two engines over one corpus
//!
//! `libghostty-vt` (`ronoctua_term::ghostty`) is the engine the terminal runs
//! on. `alacritty_terminal` ([`alacritty`]) is no engine anything runs on: it
//! is the SECOND OPINION, an independently written reading of the same bytes,
//! so a wrong reading of the VT specification shows up as two engines
//! disagreeing instead of as "the terminal goes odd sometimes".
//!
//! # Why this is a crate of its own
//!
//! Everything here exists only to ask that question — the second engine, the
//! canonical grid both answer in, and the reference colours both are handed,
//! a table written by hand. Inside `ronoctua-term` they sat behind a feature,
//! one `--features` away from every program that links the engine; here the
//! one program is a driver (`ronoctua-term-diff`), and nothing a machine runs
//! depends on this crate. The product engine is asked only through its own
//! safe API ([`ghostty::Ghostty`] over `Vt::written`), so `ronoctua-term`
//! keeps its single file of `unsafe` and this crate has none.
//!
//! # The trait is the differential's question, and nothing else
//!
//! [`Engine`] used to be the seam the whole terminal talked through, so that
//! swapping engines touched nothing else. That job is over, and the trait was
//! narrowed to what two engines can BOTH be compared on: feed bytes, resize,
//! read the grid with its styled cells, and say what would have been written
//! back to the PTY. Render state, encoders and snapshots exist only on the
//! Ghostty side, and widening the trait to them would make the second
//! implementation a set of stubs that compare nothing.
//!
//! # What is compared, and what is pinned instead
//!
//! Two engines interpreting the SAME VT corpus have to produce the SAME grid.
//! [`Grid`] is the canonical form both produce, and the diff between them is
//! the verdict. What they cannot agree on BY DESIGN — Ghostty answers DA1 with
//! sixel and XTVERSION with our name, alacritty with its own — is left out of
//! the cross-engine comparison ([`comparable_replies`]) and pinned instead by
//! each engine's own golden file, which catches a regression where the
//! comparison cannot.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod alacritty;
pub mod colours;
pub mod ghostty;
pub mod grid;
pub mod replies;

pub use colours::{ReferenceColours, Rgb8, reference_colours};
pub use grid::{CellBox, Colour, Grid, Style, Width};
pub use replies::{Reply, comparable_replies};

/// A terminal engine, as the differential sees it.
///
/// Deliberately small: what the differential needs is to feed bytes, resize,
/// read the grid and read what the engine would have answered. Resizing is in
/// because it changes the interpretation of line wrapping, which is where
/// engines usually diverge.
pub trait Engine {
    /// A name, so the diff message can say WHO disagreed.
    fn name(&self) -> &'static str;

    /// Feeds in bytes coming from the PTY.
    fn feed(&mut self, bytes: &[u8]);

    /// Resizes.
    fn resize(&mut self, columns: u16, lines: u16);

    /// The grid right now.
    fn grid(&self) -> Grid;

    /// Everything the engine would have written back to the PTY since the
    /// last call — the answers to the program's questions, in order.
    fn replies(&mut self) -> Vec<u8>;
}
