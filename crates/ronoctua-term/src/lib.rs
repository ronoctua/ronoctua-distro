//! Terminal engine — section 7.10, risk R1 (closed).
//!
//! # One engine, and it is the product
//!
//! `libghostty-vt` ([`ghostty`]) is the engine the terminal runs on: the
//! authoritative [`ghostty::Vt`] a holder keeps beside the PTY, the
//! [`ghostty::Mirror`] a window builds from its snapshot, the render state,
//! the key, mouse, focus and paste encoders. Around it, the [`holder`] that
//! keeps a terminal alive without a window, the [`pty`], the [`inbound`]
//! filter (sixel and kitty media) and the program [`status`] records.
//!
//! # The second opinion lives elsewhere, and that is the point
//!
//! `alacritty_terminal` reads the same bytes in the differential test, which
//! asks two independently written engines to agree on a VT corpus — and the
//! canonical grid they are compared in, the trait both answer through and the
//! reference colours both are handed are the differential's, in
//! `ronoctua-term-differential`, a driver that never reaches a machine. They
//! used to sit here behind a feature, and a colour table and a second engine
//! inside the crate every terminal links are one forgotten `cfg` away from
//! shipping. What this crate offers the comparison is a question about its
//! own engine, colour-free: [`ghostty::Vt::written`], a cell in the program's
//! own terms.

// `deny` AND NOT `forbid`, and the difference is this crate's whole design.
//
// `docs/ARCHITECTURE.md` says this crate isolates all the `unsafe` and all the
// FFI. That was easy to promise while there was no FFI, and `forbid` was the
// promise: it cannot be relaxed anywhere, not even by a module that means it.
//
// The Ghostty engine needs one. With `deny`, unsafe is refused everywhere EXCEPT
// where a module says otherwise in writing — and today that is exactly one file,
// `ghostty.rs`, which is the FFI. The isolation stops being a hope and becomes a
// grep for `allow(unsafe_code)`.
#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod ghostty;
pub mod holder;
pub mod inbound;
pub mod pty;
pub mod sixel;
pub mod status;
