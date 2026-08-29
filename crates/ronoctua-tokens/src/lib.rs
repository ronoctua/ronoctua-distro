//! ronoctua-distro's design system tokens.
//!
//! **Invariant 9: zero hard-coded visual values.** Node colour, font,
//! size, spacing or icon codepoint lives outside here.
//!
//! The way to make that hold is not discipline — it is there being no way to write
//! a loose value. [`Color`] has no public constructor; the only instances
//! that exist are the constants generated in [`color`].
//!
//! # Where this comes from
//!
//! The tokens are defined **once**, in `assets/tokens/*.nix` (section 7.6).
//! The build generates this crate's Rust table. Editing the generated code is pointless:
//! it disappears at the next build.
//!
//! # Why zero dependencies
//!
//! This crate is a dependency of everything else, including of `ronoctua-design` and
//! `ronoctua-glyphdb`, which compile to `wasm32-wasip2` (invariant 12).
//! Any dependency here becomes a dependency of everything, and would have to work in
//! wasm too.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// An RGBA colour, 8 bits per channel.
///
/// **There is no public constructor.** The only colours that exist are the
/// constants in [`color`], generated from `assets/tokens/colors.nix`. That is
/// invariant 9 being structural instead of agreed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Color {
    /// A **crate-private** constructor: only the generated code calls it.
    const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Components in `[0.0, 1.0]` in RGBA order, in **LINEAR** space.
    ///
    /// The tokens are written in sRGB, which is how one writes colour. A wgpu
    /// `Rgba8UnormSrgb` target expects LINEAR values and does the sRGB
    /// encoding on write — so passing `r/255` straight through leaves the curve out and
    /// lightens everything: the footer background's `0x14` reaches the screen as `0x4D`. The
    /// mistake does not show up as a failure, only as a colour nobody can
    /// trace back to the token.
    ///
    /// The alpha does NOT go through the curve: sRGB is a luminance transfer, and
    /// opacity is not luminance.
    #[must_use]
    pub fn to_linear(self) -> [f32; 4] {
        fn channel(v: u8) -> f32 {
            let c = f32::from(v) / 255.0;
            if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        [
            channel(self.r),
            channel(self.g),
            channel(self.b),
            f32::from(self.a) / 255.0,
        ]
    }

    /// Raw components, in RGBA order.
    #[must_use]
    pub const fn to_rgba8(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

include!(concat!(env!("OUT_DIR"), "/tokens.rs"));

// An empty or truncated generation takes COMPILATION down, not a test.
//
// The counts are compile-time constants, so an `assert!` in a `#[test]`
// would be folded by the compiler and would verify nothing at run time — clippy
// warns about exactly that. As a const assertion, the error shows up at build time, which
// is where a broken generator ought to show up.
const _: () = assert!(
    counts::COLORS >= 15,
    "the colour generation came out truncated"
);
const _: () = assert!(
    counts::ICONS >= 40,
    "the icon generation came out truncated"
);
const _: () = assert!(
    counts::SPACES >= 5,
    "the spacing generation came out truncated"
);

/// Geometry derived from the tokens.
///
/// Everything here is a pure function of the font size. Node text measurement,
/// no query to the renderer — and that is what makes the footer's layout cheap
/// enough to fit in one frame (section 5.2).
pub mod metrics {
    use super::font;

    /// The width of one cell, in logical px.
    ///
    /// Exact, not approximate: the font has a single advance per width.
    #[must_use]
    pub fn cell_width(font_size: f32) -> f32 {
        font_size * font::CELL_WIDTH_EM
    }

    /// The height of one line, in logical px.
    #[must_use]
    pub fn line_height(font_size: f32) -> f32 {
        font_size * font::LINE_HEIGHT
    }

    /// The footer's height: **exactly two lines** (section 4.1).
    ///
    /// This is the layer-shell surface's `exclusive zone` value, and it
    /// **does not change when the minibuffer expands** (section 4.3): the expansion
    /// grows upwards over the content, without reflowing what is behind.
    #[must_use]
    pub fn footer_height(font_size: f32) -> f32 {
        line_height(font_size) * 2.0
    }

    /// How many whole cells fit in a given width.
    #[must_use]
    pub fn columns(width_px: f32, font_size: f32) -> u16 {
        let w = cell_width(font_size);
        if w <= 0.0 {
            return 0;
        }
        // `floor`: half a cell is no use to anyone, and rounding up
        // would make the last segment spill off the screen.
        (width_px / w).floor().max(0.0) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_linear_undoes_the_srgb_curve() {
        // Without this the footer's background (0x14171C) reaches the screen as
        // ~0x4D grey. The right linear value for 0x14 is ~0.006995, not 0.0784.
        let c = Color {
            r: 0x14,
            g: 0x17,
            b: 0x1C,
            a: 0xFF,
        };
        let [r, _, _, a] = c.to_linear();
        assert!((r - 0.006_995).abs() < 1e-5, "r linear = {r}");
        // Alpha does NOT go through the curve.
        assert!((a - 1.0).abs() < 1e-6);

        // Black and white are fixed points in both spaces; if these
        // fail, the formula is inverted.
        let black = Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0xFF,
        }
        .to_linear();
        let blank = Color {
            r: 255,
            g: 255,
            b: 255,
            a: 0xFF,
        }
        .to_linear();
        assert_eq!(black[0], 0.0);
        assert!((blank[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_cell_is_six_tenths_of_an_em() {
        // Measured in the inventory of the 135 faces. If this changes, the whole
        // footer layout has to be reviewed — it is not a constant of taste.
        assert!((font::CELL_WIDTH_EM - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn the_footer_has_exactly_two_lines() {
        let s = font::SIZE_BASE;
        assert!((metrics::footer_height(s) - metrics::line_height(s) * 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn columns_never_overflow_the_width() {
        let s = font::SIZE_BASE;
        for width in [1.0_f32, 13.0, 100.0, 800.0, 3840.0] {
            let n = metrics::columns(width, s);
            let used = f32::from(n) * metrics::cell_width(s);
            assert!(
                used <= width + f32::EPSILON,
                "{n} columns take {used} px in a width of {width}"
            );
        }
    }

    #[test]
    fn the_systems_font_is_the_patched_one() {
        // Section 6.2 says TTF-nerdfonts is the default everywhere. The
        // name has to be that of the CORRECTED family (the 27 SemiExtended faces
        // fell into phantom families before the correction).
        assert!(font::FAMILY_MONO.contains("Nerd Font Mono"));
        assert_eq!(font::FAMILY_MONO, font::FAMILY_UI);
    }
}
