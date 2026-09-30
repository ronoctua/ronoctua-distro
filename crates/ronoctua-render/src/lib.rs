//! ronoctua-distro's renderer — section 7.7.
//!
//! `wgpu` + `cosmic-text` + `glyphon`. **Node vello**: the workload is glyphs,
//! rectangles and one-line rules, and the UI's icons are glyphs from the font
//! (section 6.3), so there is nothing to justify one.
//!
//! # The renderer does not use fontconfig
//!
//! Section 6.4 is explicit: if it depended on fontconfig, it would break in the
//! installer and in a minimal environment. The faces are loaded by explicit
//! path or embedded in the binary, and the fallback chain is the same one that
//! feeds the `.conf` — generated together, from `nix/fontconfig.nix`.
//!
//! # Two atlases, not one
//!
//! Section 5.2 wants the predictable repertoire in the atlas from startup; 6.5
//! wants a browser with 10,473 glyphs. That is not solved with a single atlas:
//!
//! - [`Atlas::Pinned`] — ASCII, Latin-1, box-drawing, icons in use as
//!   tokens. Populated at startup and **never subjected to `trim()`**.
//! - [`Atlas::Scratch`] — only the glyph browser uses it. `trim()` per frame.
//!
//! The browser rasterises only the visible rows (~50 glyphs), so it never
//! touches the pinned atlas, and 5.2's performance guarantee stays intact.

// `deny`, not `forbid`: the `gpu` module needs ONE unsafe block to create
// the wgpu::Surface from raw Wayland handles, and `forbid` accepts no
// exception even with a written contract. `deny` keeps the rest of the crate clean and
// requires an explicit, visible `#[allow]` where the exception holds.
#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod chain;
pub mod gpu;

/// The instant from which the "first frame" is counted.
///
/// Anchored at the FIRST call — which is why `main` calls it before anything. What
/// is left out is the `exec` and the dynamic linking, which happen before
/// any line of ours runs; it is a floor, not the complete number.
///
/// It exists as a function and not as a parameter because section 5.2's target belongs to the
/// PROCESS, and threading it through five layers of constructor would dirty each
/// of them with a concern that is not theirs.
#[must_use]
pub fn start() -> std::time::Instant {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *START.get_or_init(std::time::Instant::now)
}
pub mod image;
pub mod layout;
pub mod offscreen;
pub mod quad;
pub mod text;

pub use text::{
    Atlas, EMBEDDED_FONT, MOST_SCALE, Size, Span, Text, from_the_border, rasterised_at,
};
