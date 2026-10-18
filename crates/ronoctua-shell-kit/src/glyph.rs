//! HOW BIG A GLYPH IS DRAWN beside the text — the measured size of an icon,
//! scaled as the text is.
//!
//! # Why a module of its own
//!
//! The footer, the doors' headers, the columns and every view that leads a
//! run with a glyph ask the same question, and it used to be answered inside
//! the footer: one function every door shares belongs under every door.

use ronoctua_tokens as tokens;

/// The measured size of a run's leading glyph — [`tokens::icon::chrome_size`]
/// over the first character; the icon size when there is none.
///
/// SCALED, like every other size the host draws at. A glyph's measured size
/// is measured AGAINST THE DIGIT beside it (`chrome-icons-match-the-text`
/// re-measures it on every build), so a glyph that stayed put while the text
/// moved would be the very mismatch that check exists to refuse.
pub fn chrome_size(glyph: &str) -> f32 {
    tokens::text::scaled(
        glyph
            .chars()
            .next()
            .map_or(tokens::font::SIZE_ICON, measured),
    )
}

/// The same, for a caller that already holds the character.
///
/// The two chevrons do, and making them build a `String` per frame to have it
/// taken apart again was a cost this introduced and nothing asked for.
pub fn chrome_size_of(glyph: char) -> f32 {
    tokens::text::scaled(measured(glyph))
}

/// A glyph's measured size, unscaled: a brain module's by the catalogue the
/// process read, every other by the chrome's table.
///
/// # Why two tables
///
/// A module's mark is drawn beside the modeline's own glyphs and answers the
/// same band, so it is measured by the same code — but into the brain's
/// catalogue (`nix/brain-catalogue.nix`), data read at run time, and not into
/// the chrome's table every crate that draws compiles: a module the person
/// writes would rebuild them all. A module's codepoint reaches a painter only
/// out of that file — a mark through `brainlink::glyph_of`, a notification
/// through the book — and the binary's start reads it before anything draws,
/// so asking only a catalogue already read (`loaded`) misses none, and a
/// program that never draws a mark (the PNG camera) never reads it. A
/// codepoint both tables hold is measured the same in both.
fn measured(glyph: char) -> f32 {
    ronoctua_brain::catalogue::loaded()
        .and_then(|c| c.measured(glyph))
        .map_or_else(|| tokens::icon::chrome_size(glyph), |g| g.size)
}
