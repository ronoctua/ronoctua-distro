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
//! The tokens are defined **once**, in `assets/tokens/*.nix` (section 7.6) —
//! the colours as RECIPES that `nix/themes.nix` solves into one palette per
//! theme in `assets/themes/themes.nix`. The build generates this crate's Rust
//! table: every theme's values, and one constant per role. Editing the
//! generated code is pointless: it disappears at the next build.
//!
//! # A colour is a role; the theme is the process's
//!
//! `color::TEXT_DIM` is not bytes, it is "the dim text", and which bytes that
//! is depends on the theme the process is in ([`theme`]). The 280 places that
//! name a colour never changed for this: they name the role, and the role
//! resolves when it is drawn. Switching theme is one atomic store, and the
//! next frame is in the new palette.
//!
//! # Why zero dependencies
//!
//! This crate sits under the renderer, the shell, the file manager's helper,
//! the installer and the rescue console. Any dependency here becomes a
//! dependency of all of them.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// A design-system colour: a ROLE, resolved through the current theme the
/// moment it is asked for.
///
/// **There is no public constructor.** The only colours that exist are the
/// constants in [`color`], generated from `assets/tokens/colors.nix` — and each
/// is a role, not a value: `SURFACE_BASE` is "the content's ground", and which
/// bytes that is depends on the theme the process is in ([`theme::set`]).
/// Equality and hashing are BY ROLE: two roles that happen to share bytes in
/// one theme are still two roles, which is what every comparison in the shell
/// means by `==` anyway ("is this field drawn as dim text?").
///
/// That is invariant 9 being structural instead of agreed, twice over: no way
/// to write a loose value, and no way to hold a value that a theme change
/// would leave stale.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color(u16);

impl Color {
    /// A **crate-private** constructor: only the generated table calls it.
    const fn at(role: u16) -> Self {
        Self(role)
    }

    /// The token's name, `text.dim`, for a log line or a test's message.
    #[must_use]
    pub const fn name(self) -> &'static str {
        color::NAMES[self.0 as usize]
    }

    /// Raw components, in RGBA order, in the CURRENT theme.
    ///
    /// Not `const`: it reads which theme the process is in, and that is the
    /// whole point.
    #[must_use]
    pub fn to_rgba8(self) -> [u8; 4] {
        themes::TABLE[theme::index()][self.0 as usize]
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
        linear_of(self.to_rgba8())
    }
}

impl core::fmt::Debug for Color {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "color.{}", self.name())
    }
}

/// The sRGB curve undone, on bytes. A pure function, so the test that proves
/// it needs no theme.
fn linear_of([r, g, b, a]: [u8; 4]) -> [f32; 4] {
    fn channel(v: u8) -> f32 {
        let c = f32::from(v) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    [channel(r), channel(g), channel(b), f32::from(a) / 255.0]
}

/// Which theme the process resolves its colours through.
///
/// One index, process-wide, behind an atomic: a theme is a property of the
/// process the way the language is, and threading it through the 280 places
/// that name a colour would carry a value nothing but the draw call reads.
/// [`set`] is called at start (`theme.json` > `RONOCTUA_THEME` > the first
/// theme) and again when the person chooses; every `Color` resolves through
/// it at the moment it is drawn, so the next frame is in the new palette.
pub mod theme {
    use core::sync::atomic::{AtomicUsize, Ordering};

    static CURRENT: AtomicUsize = AtomicUsize::new(0);

    /// One of the declared themes. Only this module makes them, which is what
    /// keeps an index inside the table.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Theme {
        index: usize,
    }

    impl Theme {
        /// The id, as `assets/themes/themes.nix` spells it: `owl`.
        #[must_use]
        pub const fn id(self) -> &'static str {
            super::themes::IDS[self.index]
        }

        /// Where it sits in the declared order; the default is `0`.
        #[must_use]
        pub const fn position(self) -> usize {
            self.index
        }
    }

    /// Every theme, in `assets/themes/themes.nix` order. The first is the default.
    pub fn all() -> impl ExactSizeIterator<Item = Theme> {
        (0..super::counts::THEMES).map(|index| Theme { index })
    }

    /// The first declared theme: what a process starts in before anything
    /// says otherwise.
    #[must_use]
    pub const fn default_one() -> Theme {
        Theme { index: 0 }
    }

    /// The theme with this id, if it is declared.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Theme> {
        all().find(|t| t.id() == id)
    }

    /// The theme every `Color` resolves through right now.
    #[must_use]
    pub fn current() -> Theme {
        Theme {
            index: CURRENT.load(Ordering::Relaxed),
        }
    }

    /// Makes `t` the theme every `Color` resolves through, process-wide.
    ///
    /// `true` if it changed. `Relaxed` is enough: one self-contained word,
    /// nothing published alongside it; a frame that started under the old
    /// theme finishes under it, and the next one is in the new.
    pub fn set(t: Theme) -> bool {
        CURRENT.swap(t.index, Ordering::Relaxed) != t.index
    }

    /// The variable a NixOS option exports with the machine's default theme
    /// (`ronoctua.theme`). What the person chose lives in a file the shell
    /// reads first; this is the floor under it.
    pub const ENV: &str = "RONOCTUA_THEME";

    /// The theme [`ENV`] names, if it is set and declared. The caller logs a
    /// name that is set and unknown.
    #[must_use]
    pub fn from_env() -> Option<Theme> {
        std::env::var(ENV).ok().and_then(|id| by_id(&id))
    }

    pub(crate) fn index() -> usize {
        CURRENT.load(Ordering::Relaxed)
    }
}

include!(concat!(env!("OUT_DIR"), "/tokens.rs"));

// The table has to hold every theme, and every theme the same number of roles;
// the generator refuses anything else, and this is the wall behind it.
const _: () = assert!(counts::THEMES >= 1, "no theme was generated");
const _: () = assert!(
    themes::IDS.len() == counts::THEMES && themes::TABLE.len() == counts::THEMES,
    "the theme table and its ids disagree"
);
const _: () = assert!(
    color::NAMES.len() == counts::COLORS,
    "the role names and the table disagree"
);

// An empty or truncated generation takes COMPILATION down, not a test.
//
// The counts are compile-time constants, so an `assert!` in a `#[test]`
// would be folded by the compiler and would verify nothing at run time — clippy
// warns about exactly that. As a const assertion, the error shows up at build time, which
// is where a broken generator ought to show up.
const _: () = assert!(
    counts::COLORS >= 40,
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
    fn linear_of_undoes_the_srgb_curve() {
        // Without this the footer's background (0x14171C) reaches the screen as
        // ~0x4D grey. The right linear value for 0x14 is ~0.006995, not 0.0784.
        let [r, _, _, a] = linear_of([0x14, 0x17, 0x1C, 0xFF]);
        assert!((r - 0.006_995).abs() < 1e-5, "r linear = {r}");
        // Alpha does NOT go through the curve.
        assert!((a - 1.0).abs() < 1e-6);

        // Black and white are fixed points in both spaces; if these
        // fail, the formula is inverted.
        let black = linear_of([0, 0, 0, 0xFF]);
        let blank = linear_of([255, 255, 255, 0xFF]);
        assert_eq!(black[0], 0.0);
        assert!((blank[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_first_theme_is_the_default_and_every_id_round_trips() {
        assert_eq!(theme::default_one(), theme::all().next().unwrap());
        assert_eq!(theme::default_one().position(), 0);
        for t in theme::all() {
            assert_eq!(theme::by_id(t.id()), Some(t), "{}", t.id());
        }
        assert_eq!(theme::by_id("no-such-theme"), None);
    }

    #[test]
    fn a_colour_is_a_role_and_the_theme_says_which_bytes() {
        // THE ONE TEST THAT CALLS `set`: the theme is process-wide, and the
        // workspace's tests run in parallel. It restores what it found.
        let before = theme::current();
        for t in theme::all() {
            theme::set(t);
            assert_eq!(theme::current(), t);
            // Every role, in every theme, resolves to that theme's row.
            for (role, name) in color::NAMES.iter().enumerate() {
                let handle = Color::at(u16::try_from(role).unwrap());
                assert_eq!(handle.name(), *name);
                assert_eq!(
                    handle.to_rgba8(),
                    themes::TABLE[t.position()][role],
                    "{name} in {}",
                    t.id()
                );
            }
        }
        assert!(
            !theme::set(theme::current()),
            "setting the same theme is not a change"
        );
        theme::set(before);
        // Role equality does not care which theme is in.
        assert_eq!(color::TEXT_DIM, color::TEXT_DIM);
        assert_ne!(color::TEXT_DIM, color::TEXT_PRIMARY);
        assert_eq!(format!("{:?}", color::TEXT_DIM), "color.text.dim");
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
