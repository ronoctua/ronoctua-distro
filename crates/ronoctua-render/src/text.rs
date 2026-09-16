//! Text drawing — section 7.7.
//!
//! # Two atlases, not one
//!
//! Section 5.2 wants the predictable repertoire in the atlas from startup; 6.5
//! wants a browser with 10,473 glyphs. That is not solved with a single atlas:
//!
//! - [`Atlas::Pinned`] — ASCII, Latin-1, box-drawing, icons in use as tokens.
//!   Filled lazily, on the first frame that asks for a glyph, and trimmed
//!   only when the SIZE or the SCALE moves — the two things that make every
//!   raster in it undrawable. It said "populated at startup and never
//!   subjected to `trim()`", and neither half was true.
//! - [`Atlas::Scratch`] — only the glyph browser uses it. `trim()` per frame.
//!
//! The browser rasterises only the visible rows (~50 glyphs), so it never
//! touches the pinned atlas, and 5.2's performance guarantee stays intact.

use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;

/// The face embedded in the binary.
///
/// **Not a convenience, a requirement**: the installer runs on a TTY before
/// any GPU is configured (section 7.1), and a minimal environment may not have
/// `share/fonts` mounted. Without the embedded face, there is nothing to draw.
///
/// It is `noctua-iosevka-core`'s Regular — the 4 normal-width RIBBI
/// faces, the same subset that goes on the ISO.
pub const EMBEDDED_FONT: &[u8] = include_bytes!(env!("RONOCTUA_FONT_REGULAR"));

/// The EMOJI face, embedded alongside the Iosevka.
///
/// # Why it exists, and why it does not contradict §6.4
///
/// The Iosevka draws 75 of the 1,184 single-codepoint emoji and none of the 2,760
/// sequences — the rest would come out as `.notdef`, which in this font is a visible
/// little box. Tofu is silent fallback in different clothes.
///
/// §6.4 forbids the renderer from CONSULTING fontconfig, and that still holds:
/// the chain here is DECLARED in two lines of code, not discovered at run
/// time. Which face draws what is still a question with a fixed
/// answer, and the `renderer-loads-four-faces` check fixes it —
/// written when the third face came in, because until then it was only a
/// promise in a comment, which is the same thing as an absent guard.
///
/// CBDT/CBLC bitmaps — the format `swash` decodes and for which
/// `glyphon` already has a separate colour atlas.
pub const EMOJI_FONT: &[u8] = include_bytes!(env!("RONOCTUA_FONT_EMOJI"));

/// The LUCIDE face, with its `cmap` rewritten by `nix/lucidegen.nix`.
///
/// # Why it is remapped, and why that is not fussiness
///
/// Lucide publishes its 1,668 icons at `U+E038..U+E6BB`. The Iosevka here is
/// patched with Nerd Fonts, which occupies **634 of those same codepoints** — and
/// the primary face (the one asked for by name, see `new`) always wins the
/// fallback chain. Without remapping, 38% of the icons would appear drawn as ANOTHER
/// symbol: not as tofu, which one sees, but as a pretty, wrong icon.
///
/// The new range is plane 15 (`U+F2000`), and what checks that it is free
/// in the other two faces is the generator, before writing anything at all.
///
/// The SOURCE codepoint still exists, in `lcp` in the database: it is what serves
/// whoever pastes into a page with the actual Lucide font. They are two facts, from
/// two different sources — not two copies of one.
pub const LUCIDE_FONT: &[u8] = include_bytes!(env!("RONOCTUA_FONT_LUCIDE"));

/// The MARKS face, and it is the only one this repository DRAWS.
///
/// # Why a fourth face exists
///
/// Because a mark the interface has to draw existed in none of the other
/// three, and the two invariants that meet there leave one way out. Invariant
/// 10: a UI icon is a glyph of a font, and no design system component consults
/// a pixmap. Invariant 9: a codepoint is a token, resolved at build time
/// against a real cmap. So the mark is drawn INTO a face of ours, declared as
/// dots on a grid in `assets/icons/marks.nix` and built by
/// `nix/marksgen.nix` — which is a drawing somebody can read and tune, and not
/// a path nobody can.
///
/// It sits at `U+F3000`: past Nerd Fonts, which stops at `U+F1AF0`, and past
/// Lucide, which was moved to `U+F2000` for the same reason. The face asked
/// for by name wins the fallback chain, so a mark sharing a codepoint with a
/// Nerd Fonts glyph would come out drawn as ANOTHER symbol — and the generator
/// checks the range is free in all three others before writing anything.
///
/// Its glyphs carry the ADVANCE of the icons they sit beside — 600 units on a
/// 1000-unit em-square, which is what every one of the font's 6896 `md-`
/// glyphs advances — so a mark counts one cell and draws one cell. Their ink
/// has to fit inside a reference icon's box; it need not equal it, and no two
/// icons agree on one anyway.
pub const MARKS_FONT: &[u8] = include_bytes!(env!("RONOCTUA_FONT_MARKS"));

/// How many faces the bank has to have once assembled.
///
/// An EXACT number, and not `> 0`. The old guard could only say "some face
/// loaded": with it, the emoji face could disappear from the build and the only symptom
/// would be tofu coming back on the screen of whoever looks for an emoji.
pub const FACES: usize = 4;

/// Which atlas to use for a run of text.
///
/// Choosing wrong does not break the drawing, but it destroys the performance
/// guarantee — and silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Atlas {
    /// A predictable repertoire. Populated at startup, never pruned.
    Pinned,
    /// Transient, LRU. Only the glyph browser (6.5) and tray pixmaps.
    Scratch,
}

/// What size a run is drawn at, and how it sits on the cell grid.
///
/// # Why per run
///
/// The glyph browser needs the icon large and the surrounding text at the
/// normal size, on the same screen. A second renderer would give a second
/// atlas and a second copy of the font just to draw two sizes; here each run
/// already has its own glyphon `Buffer`, and the metric is its own.
///
/// # Why the last two are not one
///
/// Because a run drawn at another size has an ADVANCE of another size, and the
/// grid positions by cell. Whoever draws a whole line at 20 px wants it to
/// start where a 20 px line starts — from the same left edge. Whoever drops ONE
/// larger glyph among text at the base size wants it centred on the cell it
/// occupies, or it drifts right and reads as a glyph that slipped.
///
/// A single variant with a boolean beside it would let the two be asked for at
/// once, which means nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    /// The size the renderer was built with — the common case, which is why it
    /// forces nobody to say anything.
    Grid,
    /// Another size, from the same left edge a grid run would use.
    Px(f32),
    /// Another size, CENTRED on the cell it occupies.
    ///
    /// For a chrome glyph sitting inline with text at the base size — the bell,
    /// the transfer arrows, the countdown. See `font.size.icon`.
    OnTheCell(f32),
    /// Another size, and the `n`th line of a BLOCK leaded by that size.
    ///
    /// # Why a block is not `n` runs of [`Size::Px`]
    ///
    /// Because the grid's leading belongs to the grid's size. A name that wraps
    /// onto a second line is one thing read downwards, and at three quarters of
    /// the body size two of its lines a whole grid line apart have a quarter of
    /// a line of nothing between them — the owner read it off the file
    /// manager's smallest thumbnails: *"a distância entre as duas linhas dos
    /// nomes … está muito grande"*.
    ///
    /// The run still NAMES the grid line it would have occupied, so the door
    /// keeps one row per line and every hit test, band and elision is
    /// untouched; `n` says how many lines back the block began, and the
    /// renderer draws it at the block's own leading instead of the grid's.
    /// `Stacked(px, 0)` is [`Size::Px`] exactly.
    ///
    /// It works in both directions: a block LARGER than the grid is pushed
    /// down by the same expression, which is the only honest way to write it.
    Stacked(f32, u16),
}

impl Size {
    /// The size in px, when it is not the renderer's.
    #[must_use]
    pub const fn px(self) -> Option<f32> {
        match self {
            Self::Grid => None,
            Self::Px(v) | Self::OnTheCell(v) | Self::Stacked(v, _) => Some(v),
        }
    }

    /// The same size in PHYSICAL pixels.
    ///
    /// A `Size::Px` and a `Size::OnTheCell` carry a number the DOOR worked
    /// out — an icon's size, a title's — and that number is logical like
    /// everything else the door counts in. `Size::Grid` is not touched: it
    /// means "the renderer's own", and the renderer's own is already inked.
    #[must_use]
    pub fn times(self, scale: f32) -> Self {
        match self {
            Self::Grid => Self::Grid,
            Self::Px(v) => Self::Px(v * scale),
            Self::OnTheCell(v) => Self::OnTheCell(v * scale),
            Self::Stacked(v, n) => Self::Stacked(v * scale, n),
        }
    }

    /// How far DOWN from the line it names a stacked run is drawn, in px.
    ///
    /// Zero for everything else, and zero for the first line of a block. For
    /// the rest it is the whole difference between the two leadings — the
    /// block's own and the grid's — taken `n` times, which is what moves the
    /// run from where the GRID would have put it to where the BLOCK does. It
    /// comes out negative whenever the block is smaller than the grid, which
    /// is every case there is today.
    #[must_use]
    pub fn stacked(self, grid: f32) -> f32 {
        match self {
            Self::Stacked(v, n) => f32::from(n) * v.mul_add(tokens::font::LINE_HEIGHT, -grid),
            _ => 0.0,
        }
    }

    /// How far LEFT of its cell the run starts, in px.
    ///
    /// Zero for everything but [`Size::OnTheCell`], where it is half the
    /// difference between the two advances — which is what "centred on the
    /// cell" means once both advances are `size x cell_width_em`.
    #[must_use]
    pub fn nudge(self, grid: f32) -> f32 {
        match self {
            Self::OnTheCell(v) => {
                (tokens::metrics::cell_width(grid) - tokens::metrics::cell_width(v)) / 2.0
            }
            _ => 0.0,
        }
    }
}

/// How many whole cells fit across a viewport of `physical` pixels.
///
/// THE DOOR'S OWN ANSWER, FROM THE DOOR'S OWN FUNCTION. `Span::edge` is placed
/// by counting cells back from the far border, so this has to be the number
/// the layout was resolved against, to the unit — and `Text::prepare` used to
/// work it out as `physical width / physical cell`. That is the same value in
/// arithmetic and not in `f32`: `(size·s)·0.600` is not `(size·0.600)·s` for
/// every `s`. Swept over the whole size range and every width to 8192, scales
/// 1, 2 and 4 agree — a power of two is exact — and **scale 3 disagrees on 280
/// widths**: at 80% on 1080 logical pixels the door resolves 124 columns and
/// the renderer resolved 125, so every right-anchored run on the modeline sat
/// one cell out.
///
/// The viewport arrives PHYSICAL and is divided back rather than the cell
/// being multiplied, because dividing is what makes the two expressions
/// identical instead of merely equal.
#[must_use]
pub fn across(physical: u32, scale: f32, size: f32) -> u16 {
    if scale <= 0.0 {
        return 0;
    }
    tokens::metrics::columns(physical as f32 / scale, size)
}

/// The same, downwards: how many whole lines fit in `physical` pixels.
///
/// For `Span::foot`, which is `Span::edge` on the other axis. See `across`.
#[must_use]
pub fn down(physical: u32, scale: f32, size: f32) -> u16 {
    let line = tokens::metrics::line_height(size);
    if line <= 0.0 || scale <= 0.0 {
        return 0;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        ((physical as f32 / scale) / line).floor().max(0.0) as u16
    }
}

/// The largest buffer scale anything in this system follows.
///
/// # Why the bound lives here and not in the shell
///
/// Because it was written in two places — the shell's `hidpi::followed`, which
/// refuses a scale the compositor sends, and `render-png --scale`, which
/// refuses one a person types — and each said in its own comment that it was
/// "the same bound the other takes", with nothing making that true. This crate
/// is what both of them already depend on, and [`rasterised_at`] right below
/// is the part of the rule that has to draw at the number, so the bound is at
/// home beside it.
///
/// EIGHT: three more than any compositor drives a screen at, and the point at
/// which an ordinary window's buffer is past what an ordinary GPU takes — at
/// 9, a 1920-logical window wants 17280 px against a 16384 ceiling.
pub const MOST_SCALE: i32 = 8;

/// The compositor's integer scale as this rasteriser wants it.
///
/// ONE CONVERSION, and it lives here because the shell had one too and the two
/// had different policies: this end used to fall back to `1.0` on both bad
/// values without a word, which is the branch the other end's doc warns about
/// at length — the buffer sized by a scale the glyphs were not drawn at.
///
/// The type is `i32` because that is what the compositor sends and what every
/// door holds; `u16` covers every real scale and converts to `f32` exactly.
#[must_use]
pub fn rasterised_at(scale: i32) -> f32 {
    if scale <= 0 {
        tracing::warn!(
            scale,
            "a buffer scale that is not positive; rasterising one to one"
        );
        return 1.0;
    }
    match u16::try_from(scale) {
        Ok(fits) => f32::from(fits),
        Err(_) => {
            tracing::warn!(
                scale,
                "a buffer scale no screen has; rasterising one to one"
            );
            1.0
        }
    }
}

/// A run positioned in CELLS.
///
/// Position in cells, not in pixels: with the exact cell (0.600 at), the
/// layout is integer arithmetic and there is no text measurement (section 4.1).
#[derive(Debug, Clone)]
pub struct Span {
    /// The already-elided text, ready to draw.
    pub text: String,
    /// The column, from the left.
    pub column: u16,
    /// The row: 0 = modeline, 1 = minibuffer (section 4.1).
    pub line: u16,
    /// Colour, from [`ronoctua_tokens::color`].
    pub color: tokens::Color,
    /// The size, and how it sits on the grid. See [`Size`].
    pub size: Size,
    /// Whether this run is anchored to the surface's RIGHT EDGE.
    ///
    /// # Why a run ever needs to leave the grid
    ///
    /// Because the grid does not reach the edge. `columns` is
    /// `floor(width / cell)`, so between the last cell and the surface's border
    /// there is a remainder of nought to one cell — and it changes with the
    /// window's width. Anything counted from the left is unaffected; anything
    /// that should sit AGAINST the border inherits that remainder as a gap that
    /// grows and shrinks as the window is resized.
    ///
    /// The owner found it by making a window fullscreen: *"the position of the
    /// icons and numbers on the modeline's right shifts a little... it can never
    /// be variable"*. With this set, the run is placed counting cells from the
    /// border instead of from the origin, and the remainder falls where nobody
    /// is looking — on the inside, between the segments and the text to their
    /// left.
    pub edge: bool,
    /// Whether this run is anchored to the surface's BOTTOM EDGE.
    ///
    /// # The same remainder, on the other axis
    ///
    /// [`Span::edge`] exists because the grid does not reach the RIGHT border:
    /// `columns` is `floor(width / cell)` and the leftover shows up as a gap
    /// that changes with the window. The vertical is identical — `lines` is
    /// `floor(height / line)` — and the owner found it the same way, on the
    /// file manager's transport: *"os controles continuam elevados (com espaço
    /// abaixo deles) quando há apenas um espaço menor que uma linha"*.
    ///
    /// With this set the run is placed counting lines UP from the bottom, so
    /// the remainder falls where nobody is looking — above the run, inside the
    /// panel — instead of below the one thing that was supposed to sit at the
    /// foot of it.
    pub foot: bool,
}

/// The line box a run occupies, which is what decides whether it moves the
/// line it is on.
///
/// # Why a cell-centred glyph keeps the GRID's box
///
/// [`aligned_tops`] centres every run of a line against the LARGEST box on it.
/// While an icon carried a box of its own size, one 22 px bell made the
/// modeline's box 31,9 px instead of 26,1 — and every letter beside it moved
/// down 2,9 px. Nothing failed: the line simply sat lower inside its own
/// background, and the `modeline-vm` OCR went from reading `foot` to reading
/// `fant`. The gate found it; nobody would have.
///
/// "Centred on the cell" has to mean the cell in BOTH axes. The font's box at
/// 22 px is 27,5 px against the line's 26,1, so such a run is clipped by 0,7 px
/// at each end — and the ink of a chrome glyph is 0,60 em, nowhere near it.
/// `chrome-icons-match-the-text` measures exactly that ink.
#[must_use]
pub fn line_box(size: Size, grid: f32) -> f32 {
    match size {
        Size::Grid | Size::OnTheCell(_) => grid,
        Size::Px(px) | Size::Stacked(px, _) => px * tokens::font::LINE_HEIGHT,
    }
}

/// Where the top of each run goes, so the LINE shares a MIDDLE.
///
/// # Why the middle, and not the baseline nor the top
///
/// Because it is the middle the eye reads as "they are on the same line" when the sizes
/// differ. While everything had a single size this did not show — top,
/// middle and baseline moved together. One larger icon beside the search field is
/// enough for the text to rise to the icon's top and the line to fall apart.
///
/// # Why centring the BOX gets the ink right
///
/// Because the line height is symmetrical: the line box is `1.45 at` and the font's
/// box is `1.25 at` (ascender 0.965 + descender 0.285), so `0.10 at` is left
/// above and `0.10 at` below. The middle of the line box is the middle of the
/// font box — and the middle of the font box sits `0.34 at` above the baseline,
/// which is exactly the middle of a Nerd icon's ink (most go from `0.04`
/// to `0.64 at`). Centring the box, measuring the ink and centring the ink give
/// the same result, and the box is the only one of the three the renderer knows without
/// opening the font.
///
/// On a single-size line this returns exactly `line × line height`, which
/// is what was there before: it is what keeps the system's footer and the modeline
/// from moving a pixel, since they never mix sizes.
///
/// It takes `(line in cells, height of the run's box)` and returns the top in
/// pixels, in the same order.
#[must_use]
pub fn aligned_tops(spans: &[(u16, f32)], line_height: f32) -> Vec<f32> {
    // The largest box on each line. There are few lines per frame, so the linear
    // search costs less than the table that would avoid it.
    let mut largest: Vec<(u16, f32)> = Vec::new();
    for (line, bounds) in spans {
        match largest.iter_mut().find(|(l, _)| l == line) {
            Some((_, c)) => *c = c.max(*bounds),
            None => largest.push((*line, *bounds)),
        }
    }
    spans
        .iter()
        .map(|(line, bounds)| {
            let of_the_line = largest
                .iter()
                .find(|(l, _)| l == line)
                .map_or(*bounds, |(_, c)| *c);
            f32::from(*line).mul_add(line_height, (of_the_line - bounds) / 2.0)
        })
        .collect()
}

/// Draws text onto a wgpu surface.
pub struct Text {
    face: glyphon::FontSystem,
    swash: glyphon::SwashCache,
    /// How many `prepare` calls have run. Only for timing the first few.
    shapings: u8,
    /// glyphon's `Cache` holds pipelines and bind group layouts. Creating one
    /// per frame — as an earlier version did — would recreate the pipeline on
    /// every draw, which on its own would blow section 5.2's budget.
    viewport: glyphon::Viewport,
    atlas_pinned: glyphon::TextAtlas,
    atlas_scratch: glyphon::TextAtlas,
    renderer_pinned: glyphon::TextRenderer,
    renderer_scratch: glyphon::TextRenderer,
    /// The size in LOGICAL pixels — the one a person chose, the one the layout
    /// counted cells with, the one a pointer's coordinates are in.
    size: f32,
    /// How many physical pixels one logical pixel is, on the screen this
    /// surface is currently on.
    ///
    /// # Why the renderer is where this lives
    ///
    /// Because everything else in the shell is LOGICAL and has to stay that
    /// way: a `Span` is a column and a line, a `Quad` is a rectangle over a
    /// logical viewport, a pointer arrives in logical coordinates, and the
    /// number of columns is `logical width / logical cell`. Only the last
    /// step — glyph rasters on a texture — is in physical pixels, and doing
    /// the multiplication anywhere else would mean every door multiplying at
    /// every site and one of them forgetting.
    ///
    /// So: the doors keep handing over logical everything and say what the
    /// scale is; this is the one place it is applied.
    scale: f32,
    /// How many of the chain's coverage faces actually loaded.
    ///
    /// Not a boolean, because the two ends of the range mean different things:
    /// zero is an installer or a rescue console, which is expected; fewer than
    /// declared is a build that lost a file, which is not. See
    /// [`Text::coverage`].
    coverage: usize,
}

impl Text {
    /// Draws at a new size, from the next frame on. `true` if it moved.
    ///
    /// # Why this is the WHOLE of the change
    ///
    /// The atlases, the renderers and the viewport do not depend on the size:
    /// glyphon keys a glyph's raster by its own metrics inside its cache, and
    /// a `Buffer` is given `Metrics::new` at shaping time. So the size is one
    /// field, and every measurement taken from it — the cell, the line, the
    /// nudge a run of another scale gets — is read through `self.size` when
    /// the frame is built.
    ///
    /// # And why it has to exist at all
    ///
    /// The size is a person's choice now, not a property of the build. Without
    /// this the shaper keeps the size the process started at while everything
    /// AROUND it moves: the columns, the cell width, the line height, every
    /// hit test. The result is worse than not following at all — a grid at one
    /// size with letters at another, which reads as a broken font rather than
    /// as a setting that did not take.
    ///
    /// # What is proved, and by what
    ///
    /// That the shaper DRAWS at the size it holds is measured, not argued:
    /// `ronoctua-render-png --text 70/100/200` gives the same thirteen runs
    /// on the same 60x12 grid, on surfaces of exactly 0,70x and 2,00x the
    /// hundred-percent one. That the DOORS reach this is measured too — a
    /// footer under Xvfb, told through its file, moved from 53 px and 118
    /// columns to 79/79, then 105/59, then 37/169, and back to 53/118.
    ///
    /// This function itself has no test of its own: it needs a device, and a
    /// device in a unit test would be a test of wgpu.
    ///
    /// # THE ATLASES ARE RELEASED, and `trim` is not what that word suggests
    ///
    /// A raster IS keyed by its metrics, so a new size is a whole new set of
    /// entries and the old ones can never be drawn from again. The obvious
    /// move is to trim the atlases — and the obvious reading of that word is
    /// wrong, so it is written down here rather than assumed.
    ///
    /// `glyphon::TextAtlas::trim` (0.12.0, `text_atlas.rs:334`) forwards to
    /// each inner atlas, whose whole body is `self.glyphs_in_use.clear()`
    /// (`:219`). It frees NOTHING at the moment it runs: `glyph_cache` is an
    /// unbounded LRU (`:57`) and the only evictions are the two `pop_lru`
    /// calls inside `try_allocate` (`:72-105`), reached only when the atlas
    /// texture is already full.
    ///
    /// What the call really buys is LAZY RECLAMATION: it takes the old size's
    /// glyphs out of `glyphs_in_use`, so the next allocation that runs out of
    /// room evicts them instead of growing the texture — a doubling, capped
    /// at `max_texture_dimension_2d`. Without it the old size's rasters would
    /// be pinned as in use for ever and the growth would be the only way out.
    ///
    /// SO THE RESIDENCY IS NOT RECLAIMED, and cannot be: glyphon 0.12 offers
    /// no `clear` and no `evict`, and the only route would be building a new
    /// `TextAtlas`. The repertoire is a few megabytes across the whole range,
    /// so that is not worth doing — but it is worth saying, because the first
    /// version of this comment claimed the memory came back.
    ///
    /// The first frame after a change pays a full re-raster either way: the
    /// old rasters are the wrong size.
    pub fn draw_at(&mut self, size: f32) -> bool {
        let moved = (self.size - size).abs() > f32::EPSILON;
        if moved {
            self.atlas_pinned.trim();
            self.trim_scratch();
        }
        self.size = size;
        moved
    }

    /// Draws on a screen of a different scale, from the next frame on.
    ///
    /// `true` if it moved, and the caller has to redraw — the same contract
    /// `draw_at` has, and for the same reason: a raster is keyed by its
    /// metrics, so twice the scale is a whole new set of glyphs and the old
    /// ones can never be drawn from again. Read `draw_at` for what `trim`
    /// does and does not free.
    ///
    /// A scale of zero or less is REFUSED rather than clamped: the
    /// compositor's own protocol says a buffer scale is a positive integer,
    /// and a zero would put every glyph at zero pixels — a window that draws
    /// nothing, which reads as a crash and is not one.
    pub fn draw_scaled(&mut self, scale: f32) -> bool {
        if scale <= 0.0 {
            tracing::warn!(
                scale,
                "a scale that is not positive; keeping the one we had"
            );
            return false;
        }
        let moved = (self.scale - scale).abs() > f32::EPSILON;
        if moved {
            self.atlas_pinned.trim();
            self.trim_scratch();
        }
        self.scale = scale;
        moved
    }

    /// The size the GPU actually rasterises at.
    fn ink(&self) -> f32 {
        self.size * self.scale
    }

    /// Builds the drawer on an already-created device.
    ///
    /// The font comes embedded ([`EMBEDDED_FONT`]) and **not** from fontconfig
    /// (section 6.4).
    /// The scale is a PARAMETER and not a default, so that no door can build
    /// a shaper without saying what screen it is on. Seven of the nine did:
    /// `Text::new` started at one, `hidpi::followed` only reaches the shaper
    /// through `text.as_mut()`, and a scale that arrived BEFORE the `Text`
    /// existed was then never applied — `followed` short-circuits on the same
    /// factor for ever after. The result is worse than the blur it replaces:
    /// glyphs rasterised at logical size into a physical buffer, half-size
    /// text in the top-left quarter of the window.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format_of: wgpu::TextureFormat,
        // THE COMPOSITOR'S OWN TYPE, and not a second `f32` beside `size`.
        // Two adjacent floats can be swapped and it compiles; at 100% and
        // scale 1 the swap is even invisible, because `ink()` is 18.0 either
        // way and the column count comes out the same — it only shows on an
        // icon run, where the bell would be eighteen times too big. An `i32`
        // is what `scale_factor_changed` hands over and what every door
        // holds, so this is also one conversion fewer at eleven call sites.
        scale: i32,
        size: f32,
    ) -> Result<Self> {
        // THE BANK IS BUILT HERE, and not by `new_with_fonts`.
        //
        // cosmic-text 0.19's `new_with_fonts` does
        // `db.set_monospace_family("Noto Sans Mono")` — a font that is NOT
        // in our bank, because we only load our own face. Asking for
        // `Family::Monospace` resolved to a non-existent name and fell into
        // the fallback machinery.
        //
        // Measured cost of the difference, on the FIRST shaping:
        //
        //     Family::Monospace           3,187 ms
        //     Family::Name(FAMILY_MONO)     122 ms     <- 26x
        //
        // It is not only performance: with the generic, which face drew the footer
        // depended on the fallback, which contradicts section 6.4 ("the renderer does not
        // use fontconfig") just as much as consulting fontconfig would.
        // See `ronoctua-bench-shaping`.
        let mut db = glyphon::fontdb::Database::new();
        db.load_font_source(glyphon::fontdb::Source::Binary(std::sync::Arc::new(
            EMBEDDED_FONT,
        )));
        // THE SECOND FACE. cosmic-text's `Shaping::Advanced` already searches by
        // name when the first does not have the codepoint; with a bank of one face
        // only, that search had nowhere to land and the result was `.notdef`.
        db.load_font_source(glyphon::fontdb::Source::Binary(std::sync::Arc::new(
            EMOJI_FONT,
        )));
        // THE THIRD. It is reached by the same path as the emoji one — by
        // CODEPOINT, and not by family name — and that is why the
        // remapping had to be done: with the source codepoints, the
        // Iosevka would answer first for 634 of them. Measured with a probe that
        // asks `cosmic-text` which face drew the glyph; the `renderer-loads-four-faces`
        // check repeats it on every build.
        db.load_font_source(glyphon::fontdb::Source::Binary(std::sync::Arc::new(
            LUCIDE_FONT,
        )));
        // THE FOURTH, and the only one with no upstream font behind it: the
        // marks this repository draws, at `U+F3000`. Reached by codepoint like
        // the two above, and free of them by construction — the generator
        // refuses to write into a range any other face already answers for.
        db.load_font_source(glyphon::fontdb::Source::Binary(std::sync::Arc::new(
            MARKS_FONT,
        )));
        // All three generics point at our face: if some day a
        // run asks for `Serif`, let it land here and not on the system.
        db.set_monospace_family(tokens::font::FAMILY_MONO);
        db.set_sans_serif_family(tokens::font::FAMILY_UI);
        db.set_serif_family(tokens::font::FAMILY_UI);

        // IT FAILS LOUDLY IF AN EMBEDDED FACE DID NOT LOAD, and it is counted
        // BEFORE the coverage faces join — otherwise the number would depend on
        // what a directory happened to hold, and an exact number is the whole
        // point of this guard (see `FACES`).
        let n = db.faces().count();
        anyhow::ensure!(
            n == FACES,
            "the database has {n} faces and should have {FACES} (Iosevka, emoji, Lucide \
             and the marks). One face fewer draws tofu again in place of the emoji, \
             the icons or a mark, and carrying on from here would only produce that \
             on screen."
        );
        tracing::debug!(faces = n, "the embedded faces are loaded");

        // THE COVERAGE FACES — every script the chain promises.
        //
        // By PATH and not embedded, and the difference is not a detail: they are
        // 44 files and about 54 MB. `include_bytes!` would put them in the
        // binary AND — the expensive half — put a store path in the environment
        // of the derivation that compiles the workspace, where Nix addresses by
        // input and any change to it rebuilds all thirty-three crates. That is
        // the fourth incident in the architecture document, and the answer it
        // arrived at is this one: the path travels on the wrapper.
        //
        // Absence is NOT an error. The installer and the rescue console run
        // without it and draw the two Latin languages this system speaks with
        // the four faces above. What is refused instead is silence: `coverage`
        // reports how many arrived, so a screen can say the interface is
        // reduced rather than showing tofu and letting somebody guess.
        let coverage = Self::load_the_coverage(&mut db);

        let face = glyphon::FontSystem::new_with_locale_and_db_and_fallback(
            // THE LOCALE IS A DRAWING, not a formality: it is what decides,
            // among the four unified Han faces, whether 直 comes out Japanese or
            // Chinese. It starts at the default and `set_locale` moves it when
            // the interface's language changes.
            crate::chain::DEFAULT_LOCALE.to_owned(),
            db,
            crate::chain::Chain,
        );

        let cache = glyphon::Cache::new(device);
        let viewport = glyphon::Viewport::new(device, &cache);
        let mut atlas_pinned = glyphon::TextAtlas::new(device, queue, &cache, format_of);
        let mut atlas_scratch = glyphon::TextAtlas::new(device, queue, &cache, format_of);
        let renderer_pinned = glyphon::TextRenderer::new(
            &mut atlas_pinned,
            device,
            wgpu::MultisampleState::default(),
            None,
        );
        let renderer_scratch = glyphon::TextRenderer::new(
            &mut atlas_scratch,
            device,
            wgpu::MultisampleState::default(),
            None,
        );

        Ok(Self {
            face,
            swash: glyphon::SwashCache::new(),
            shapings: 0,
            viewport,
            atlas_pinned,
            atlas_scratch,
            renderer_pinned,
            renderer_scratch,
            size,
            // REFUSED, AND LOUDLY, the way `draw_scaled` refuses it — see
            // `rasterised_at`, which is the one conversion.
            scale: rasterised_at(scale),
            coverage,
        })
    }

    /// Draws the unified Han the way this language expects, from the next
    /// frame on. `true` if it moved.
    ///
    /// # Why a renderer has a language at all
    ///
    /// It has exactly one, and only for this: 直 is a single codepoint with
    /// four accepted drawings, and Unicode left the choice to whoever renders.
    /// Nothing else here knows what a language is — the texts, the date order
    /// and the decimal mark all live in `ronoctua-i18n`, and must not move.
    ///
    /// # Why it rebuilds the font system
    ///
    /// Because cosmic-text resolves the fallback ONCE per `FontSystem` and
    /// caches it, so a locale changed underneath would take effect for text
    /// shaped after it and not for text already in the cache — a screen with
    /// two Han faces on it, which is worse than either. Rebuilding is the same
    /// answer the theme takes for its textures. It costs the database's faces
    /// being re-registered, not re-read: fontdb keeps `Source::File` lazy.
    pub fn set_locale(&mut self, locale: &str) -> bool {
        if self.face.locale() == locale {
            return false;
        }
        let mut db = glyphon::fontdb::Database::new();
        for face in self.face.db().faces() {
            db.push_face_info(face.clone());
        }
        self.face = glyphon::FontSystem::new_with_locale_and_db_and_fallback(
            locale.to_owned(),
            db,
            crate::chain::Chain,
        );
        true
    }

    /// How many faces of the declared chain are loaded, and how many it names.
    ///
    /// A screen that wants to say "this machine cannot draw Japanese" asks
    /// here. The alternative is tofu, and tofu is a silent fallback wearing
    /// different clothes — the sentence this whole module is organised around.
    #[must_use]
    pub const fn coverage(&self) -> (usize, usize) {
        (self.coverage, crate::chain::COVERAGE.len())
    }

    /// Which face draws a codepoint, by family name. `None` when nothing does.
    ///
    /// It exists for `ronoctua-test-faces`, which is what turns the declared
    /// chain from a comment into a promise: it asks the assembled database who
    /// answers for each of the chain's probes.
    #[must_use]
    pub fn who_draws(&mut self, c: char) -> Option<String> {
        let mut buffer = glyphon::Buffer::new(&mut self.face, glyphon::Metrics::new(16.0, 16.0));
        buffer.set_text(
            &c.to_string(),
            &glyphon::Attrs::new().family(glyphon::Family::Name(tokens::font::FAMILY_MONO)),
            glyphon::Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.face, false);
        let run = buffer.layout_runs().next()?;
        let glyph = run.glyphs.first()?;
        // GLYPH ZERO IS `.notdef`, and reporting the face that owns it would
        // name a face that drew nothing. Tofu has to answer `None`.
        if glyph.glyph_id == 0 {
            return None;
        }
        let info = self.face.db().face(glyph.font_id)?;
        info.families.first().map(|(name, _)| name.clone())
    }

    /// Loads every coverage face the wrapper points at, and says how many.
    ///
    /// # Why it does not read the manifest
    ///
    /// Because the manifest maps FAMILY to file, and fontdb files a face under
    /// the family its own name table declares — `nameID 16` when present, which
    /// is the difference between `Noto Sans Hebrew` and `Noto Sans Hebrew
    /// Light`. Loading the directory and letting fontdb name what it finds
    /// keeps one authority for that. The manifest is what the DERIVATION uses
    /// to refuse a missing family, which is the other half of the promise.
    fn load_the_coverage(db: &mut glyphon::fontdb::Database) -> usize {
        let Some(dir) = std::env::var_os("RONOCTUA_COVERAGE_FONTS") else {
            tracing::debug!(
                "RONOCTUA_COVERAGE_FONTS is not set; only the embedded faces are available"
            );
            return 0;
        };
        let before = db.len();
        db.load_fonts_dir(&dir);
        let loaded = db.len() - before;
        tracing::debug!(
            faces = loaded,
            directory = ?dir,
            "the coverage faces are loaded"
        );
        loaded
    }

    /// The width of one cell, in PHYSICAL px — the scale is already in it.
    ///
    /// It is the width the rasteriser places a glyph at, which is what the
    /// only caller (`prepare`) wants. What a door counts columns with is the
    /// LOGICAL cell, and that one comes from `tokens::metrics::cell_width` —
    /// see the `scale` field for why the two live apart.
    #[must_use]
    pub fn cell_width(&self) -> f32 {
        tokens::metrics::cell_width(self.ink())
    }

    /// The height of one line, in PHYSICAL px. See [`Text::cell_width`].
    #[must_use]
    pub fn line_height(&self) -> f32 {
        tokens::metrics::line_height(self.ink())
    }

    /// Prunes **only** the transient atlas.
    ///
    /// The pinned one is not touched HERE — it is what preserves section
    /// 5.2's guarantee even with the glyph browser open. Two places release
    /// it, [`Text::draw_at`] and [`Text::draw_scaled`], and only because a
    /// change of SIZE or of SCALE makes every raster in it undrawable. Read
    /// `draw_at` before believing the word "trim": it marks glyphs as no
    /// longer in use, which lets a later allocation reclaim them; it frees
    /// nothing when it runs.
    pub fn trim_scratch(&mut self) {
        self.atlas_scratch.trim();
    }

    /// Prepares the runs for the next frame.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        atlas: Atlas,
        spans: &[Span],
    ) -> Result<()> {
        self.viewport
            .update(queue, glyphon::Resolution { width, height });

        let cw = self.cell_width();
        let lh = self.line_height();
        // TAKEN AS NUMBERS, not read through `self`: the closures below are
        // alive while the shaper and the atlases are borrowed mutably.
        let (ink, scale) = (self.ink(), self.scale);
        // THE SAME COUNT THE DOOR RESOLVED THE LAYOUT AGAINST, from the same
        // function with the same inputs.
        //
        // `Span::edge` and `Span::foot` are placed by counting cells back from
        // the far border, so this number has to be the door's to the unit —
        // and it used to be recomputed here as `physical / physical`. That is
        // the same value in arithmetic and not in `f32`: `(size·s)·0.600` is
        // not `(size·0.600)·s` for every `s`. Swept over the whole size range
        // and every width up to 8192, scale 1, 2 and 4 agree (a power of two
        // is exact) and **scale 3 disagrees on 280 widths** — at 80% and 1080
        // logical px the door resolves 124 columns and this resolved 125, so
        // every right-anchored run on the modeline moved one cell.
        //
        // The viewport arrives physical, so it is divided back rather than the
        // cell being multiplied: dividing is what makes the two expressions
        // identical instead of merely equal.
        let across = f32::from(across(width, scale, self.size));
        let down = f32::from(down(height, scale, self.size));
        let metric = glyphon::Metrics::new(ink, lh);
        // THE METRIC OF A RUN THAT ASKS FOR ANOTHER SIZE.
        //
        // The line height follows the size: with the renderer's, a large
        // run would be clipped top and bottom by the line box.
        let metric_of = |t: &Span| match t.size.times(scale) {
            Size::Grid => metric,
            // A WHOLE LINE AT ANOTHER SIZE takes that size's line height too:
            // with the renderer's, a large run would be clipped top and bottom
            // by the line box.
            Size::Px(px) | Size::Stacked(px, _) => {
                glyphon::Metrics::new(px, px * tokens::font::LINE_HEIGHT)
            }
            // A GLYPH CENTRED ON ITS CELL KEEPS THE GRID'S LINE HEIGHT — see
            // [`line_box`], which is the same rule and the only place it is
            // written. Two expressions of it are how the box and the metric
            // come to disagree, and the disagreement moves a whole line.
            Size::OnTheCell(px) => glyphon::Metrics::new(px, line_box(t.size.times(scale), lh)),
        };

        // In cosmic-text 0.19, `set_size` and `set_text` no longer take the
        // FontSystem, and `set_text` gained an `Option<Align>` at the end. Only
        // `Buffer::new` and `shape_until_scroll` still ask for it.
        // Shaping and atlas upload are costs of different natures: one is
        // rustybuzz over the font's tables, the other is swash rasterising and
        // uploading a texture. Only separated can one tell which to optimise.
        let began_shaping = std::time::Instant::now();
        let mut buffers = Vec::with_capacity(spans.len());
        for t in spans {
            let mut b = glyphon::Buffer::new(&mut self.face, metric_of(t));
            b.set_size(Some(width as f32), Some(lh));
            b.set_text(
                &t.text,
                // By NAME, never by the generic. See the comment in `new`.
                &glyphon::Attrs::new().family(glyphon::Family::Name(tokens::font::FAMILY_MONO)),
                glyphon::Shaping::Advanced,
                // Alignment comes from the LAYOUT, not from the shaper: the positions
                // already come out in cells from `layout::resolve`.
                None,
            );
            b.shape_until_scroll(&mut self.face, false);
            buffers.push(b);
        }

        let shaping = began_shaping.elapsed();

        // EACH RUN'S BOX: it is the line height of its own metric, the same one
        // `metric_of` built. See `aligned_tops`.
        let boxes: Vec<(u16, f32)> = spans
            .iter()
            .map(|t| (t.line, line_box(t.size.times(scale), lh)))
            .collect();
        let tops = aligned_tops(&boxes, lh);

        let areas = spans
            .iter()
            .zip(&buffers)
            .zip(&tops)
            .map(|((t, buf), top)| {
                let [r, g, b, a] = t.color.to_rgba8();
                glyphon::TextArea {
                    buffer: buf,
                    // THE NUDGE IS THE ONLY PLACE A RUN LEAVES THE GRID, and
                    // it is zero for everything but `Size::OnTheCell` — see
                    // `Size::nudge`.
                    // COUNTED FROM THE BORDER when the run is anchored there —
                    // see `Span::edge`. `columns` is what the layout was
                    // resolved against, so `columns - column` is how many cells
                    // the run sits from the right, and that distance is exact
                    // in pixels however the remainder falls.
                    left: if t.edge {
                        (across - f32::from(t.column)).mul_add(-cw, width as f32)
                            + t.size.times(scale).nudge(ink)
                    } else {
                        f32::from(t.column).mul_add(cw, t.size.times(scale).nudge(ink))
                    },
                    // COUNTED UP FROM THE BOTTOM when the run is anchored
                    // there — see `Span::foot`, and `Span::edge` for the same
                    // decision on the other axis.
                    // AND A STACKED RUN LEAVES ITS LINE — see `Size::stacked`,
                    // which is zero for every other size and for the first line
                    // of a block. It is added to whichever anchor applies,
                    // because "which line" and "how far from it" are different
                    // questions.
                    top: t.size.times(scale).stacked(lh)
                        + if t.foot {
                            (down - f32::from(t.line)).mul_add(-lh, height as f32)
                                + (*top - f32::from(t.line) * lh)
                        } else {
                            *top
                        },
                    scale: 1.0,
                    bounds: glyphon::TextBounds {
                        left: 0,
                        top: 0,
                        right: width as i32,
                        bottom: height as i32,
                    },
                    default_color: glyphon::Color::rgba(r, g, b, a),
                    custom_glyphs: &[],
                }
            });

        let (renderer, atlas_ref) = match atlas {
            Atlas::Pinned => (&mut self.renderer_pinned, &mut self.atlas_pinned),
            Atlas::Scratch => (&mut self.renderer_scratch, &mut self.atlas_scratch),
        };

        let t_atlas = std::time::Instant::now();
        renderer
            .prepare(
                device,
                queue,
                &mut self.face,
                atlas_ref,
                &self.viewport,
                areas,
                &mut self.swash,
            )
            .context("glyphon could not prepare the text")?;

        // Only the first few: after that they would be one line per 1 s tick.
        if self.shapings < 3 {
            self.shapings += 1;
            tracing::info!(
                n = self.shapings,
                spans = spans.len(),
                shaping_ms = shaping.as_secs_f64() * 1000.0,
                atlas_ms = t_atlas.elapsed().as_secs_f64() * 1000.0,
                "start-up: prepare"
            );
        }

        Ok(())
    }

    /// Emits the draw commands into the given `render pass`.
    pub fn draw(&self, atlas: Atlas, pass: &mut wgpu::RenderPass<'_>) -> Result<()> {
        let (renderer, atlas_ref) = match atlas {
            Atlas::Pinned => (&self.renderer_pinned, &self.atlas_pinned),
            Atlas::Scratch => (&self.renderer_scratch, &self.atlas_scratch),
        };
        renderer
            .render(atlas_ref, &self.viewport, pass)
            .context("glyphon could not draw")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_font_is_not_empty() {
        // It catches the case of `include_bytes!` having pointed at the wrong or
        // truncated file — which would only show up as "nothing draws".
        assert!(
            EMBEDDED_FONT.len() > 1_000_000,
            "the embedded face is {} bytes, which is far too small for Iosevka",
            EMBEDDED_FONT.len()
        );
        // The TrueType signature.
        assert_eq!(&EMBEDDED_FONT[0..4], &[0x00, 0x01, 0x00, 0x00]);
    }

    /// The cell's height, as the renderer builds it: `20 × 1.45`.
    const LH: f32 = 29.0;
    /// A run's box at icon scale: `40 × 1.45`.
    const ICON: f32 = 58.0;
    /// The search field's box: `18 × 1.45`.
    const TEXT: f32 = 26.1;

    #[test]
    fn a_grid_run_does_not_leave_its_cell() {
        // The common case, and the one that must not move a pixel: everything
        // the footer draws is `Grid`, and a nudge there would shift the whole
        // system half a cell.
        assert_eq!(Size::Grid.nudge(18.0), 0.0);
        // And a whole line at ANOTHER size still starts from the same left
        // edge: the file manager's window draws every run at 20 px, and
        // centring each of them would jitter the columns against each other.
        assert_eq!(Size::Px(20.0).nudge(18.0), 0.0);
    }

    #[test]
    fn a_glyph_on_the_cell_is_centred_on_it() {
        // The bell: 22 px of glyph on a grid of 18. The advance grows from
        // 10,8 px to 13,2, so the run starts 1,2 px to the LEFT and the ink
        // ends up centred on the cell the layout gave it.
        let nudge = Size::OnTheCell(22.0).nudge(18.0);
        assert!((nudge - -1.2).abs() < 0.001, "the nudge is {nudge}");
        // A glyph SMALLER than the grid goes the other way, and that is the
        // same rule and not a second one.
        assert!(Size::OnTheCell(15.0).nudge(18.0) > 0.0);
    }

    #[test]
    fn the_renderer_counts_the_cells_the_door_counted() {
        // `Span::edge` is placed by counting cells back from the far border,
        // so this number has to be the door's to the unit. It used to be
        // `physical width / physical cell` here and `logical / logical` there
        // — the same value in arithmetic and not in `f32`.
        //
        // THE SWEEP IS WHAT FOUND IT: scales 1, 2 and 4 agree because
        // multiplying by a power of two is exact, so a test at scale 2 alone
        // would have passed over the defect. Scale 3 is where it lives.
        let mut apart = 0;
        // THE RANGE COMES FROM THE TOKENS, not from three numbers typed here.
        // They became `font.scale.{floor,ceiling,step}` in the same round this
        // sweep was written, and `one-text-size` only reads the shell — so a
        // copy here is a copy nothing keeps honest: move the ceiling and this
        // sweep silently stops covering what it exists to cover.
        for pct in (tokens::font::SCALE_FLOOR..=tokens::font::SCALE_CEILING)
            .step_by(tokens::font::SCALE_STEP as usize)
        {
            let size = tokens::font::SIZE_BASE * pct as f32 / 100.0;
            for scale in [1.0_f32, 2.0, 3.0, 4.0] {
                for logical in (1_u32..=4096).step_by(7) {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let physical = (logical as f32 * scale) as u32;
                    let theirs = tokens::metrics::columns(logical as f32, size);
                    if across(physical, scale, size) != theirs {
                        apart += 1;
                    }
                }
            }
        }
        assert_eq!(apart, 0, "the two counts disagree on {apart} widths");

        // AND THE ONE THAT WAS THERE IS STILL WRONG, which is why this is a
        // function and not an expression: the old arithmetic, written out, on
        // a case the sweep above found. 70% of the base size, scale 3, 3969
        // logical pixels: the door resolves 525 columns and this resolved 524,
        // so a run anchored to the right edge sat one cell PAST it and its
        // last cell was clipped off the surface.
        let size = tokens::font::SIZE_BASE * 70.0 / 100.0;
        let (scale, logical) = (3.0_f32, 3969.0_f32);
        let cw = tokens::metrics::cell_width(size * scale);
        let was = ((logical * scale) / cw).floor();
        assert_eq!(tokens::metrics::columns(logical, size), 525);
        assert_ne!(
            was, 525.0,
            "the defect this guards is not reproducible; the guard may be idle",
        );
    }

    #[test]
    fn a_scale_no_screen_has_rasterises_one_to_one() {
        assert!((rasterised_at(1) - 1.0).abs() < f32::EPSILON);
        assert!((rasterised_at(2) - 2.0).abs() < f32::EPSILON);
        assert!((rasterised_at(3) - 3.0).abs() < f32::EPSILON);
        // THE TWO THAT ARE NOT SCALES. This answers an `f32` and so cannot
        // refuse; what it can do is not invent one, and say so. Past `u16` it
        // used to fall back silently, which is the branch that would leave the
        // buffer sized by the scale and the glyphs drawn at one.
        assert!((rasterised_at(0) - 1.0).abs() < f32::EPSILON);
        assert!((rasterised_at(-4) - 1.0).abs() < f32::EPSILON);
        assert!((rasterised_at(i32::MAX) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_block_of_small_runs_is_leaded_by_itself_and_not_by_the_grid() {
        // THE DEFECT THIS CATCHES: a name that wraps onto a second line drawn a
        // whole GRID line below the first while its letters are three quarters
        // of the grid's size. The owner read it off the file manager's smallest
        // thumbnails: *"a distância entre as duas linhas dos nomes de arquivos
        // e pastas está muito grande"*.
        let grid = tokens::font::SIZE_BASE * tokens::font::LINE_HEIGHT;
        let small = tokens::font::SIZE_SMALL;
        let own = small * tokens::font::LINE_HEIGHT;

        // THE FIRST LINE OF A BLOCK DOES NOT MOVE: it is `Size::Px` exactly, so
        // a name that fits on one line draws where it always did.
        assert!(Size::Stacked(small, 0).stacked(grid).abs() < f32::EPSILON);
        assert!(Size::Px(small).stacked(grid).abs() < f32::EPSILON);
        assert!(Size::Grid.stacked(grid).abs() < f32::EPSILON);

        // AND THE SECOND IS PULLED UP BY THE WHOLE DIFFERENCE, which is what
        // makes the pair sit at the small size's own leading.
        let shift = Size::Stacked(small, 1).stacked(grid);
        assert!(shift < 0.0, "the second line was not pulled up: {shift}");
        assert!(
            ((grid + shift) - own).abs() < 0.001,
            "the block is leaded at {} and its own leading is {own}",
            grid + shift
        );

        // AND THE BOX IS THE BLOCK'S, not the grid's — a stacked run answers
        // `line_box` exactly as the `Size::Px` it continues, so `aligned_tops`
        // cannot move a line it shares with anything else.
        let boxed = line_box(Size::Stacked(small, 1), grid) - line_box(Size::Px(small), grid);
        assert!(boxed.abs() < f32::EPSILON);

        // AND IT SCALES. On a screen of scale 2 the block and the grid are both
        // twice as big, so the lift doubles rather than staying put — the same
        // promise `Size::nudge` makes on the other axis.
        let twice = Size::Stacked(small, 1).times(2.0).stacked(grid * 2.0);
        assert!(
            (twice - shift * 2.0).abs() < 0.001,
            "the lift did not scale: {twice}"
        );
    }

    #[test]
    fn a_viewport_of_nothing_counts_nothing() {
        // A COMPOSITOR CAN CONFIGURE A WINDOW AT ZERO while it is mapping, and
        // a scale that is not positive is refused rather than divided by.
        let size = tokens::font::SIZE_BASE;
        assert_eq!(across(0, 1.0, size), 0);
        assert_eq!(down(0, 1.0, size), 0);
        assert_eq!(across(1920, 0.0, size), 0);
        assert_eq!(down(1080, -1.0, size), 0);
        assert_eq!(across(1920, 1.0, 0.0), 0);
    }

    #[test]
    fn a_screen_of_another_scale_keeps_everything_in_proportion() {
        // THE WHOLE POINT OF `Size::times`: on a screen of scale 2 the run and
        // the grid are BOTH twice as big, so a glyph centred on its cell is
        // still centred on it — the nudge doubles rather than staying put.
        //
        // A nudge that did not scale would be the defect this guards: the bell
        // would sit 1,2 px left of where it belongs on a HiDPI screen, which
        // at that size is half a stem's width and reads as a wobble.
        let one = Size::OnTheCell(22.0).nudge(18.0);
        let two = Size::OnTheCell(22.0).times(2.0).nudge(18.0 * 2.0);
        assert!(
            (two - one * 2.0).abs() < 0.001,
            "the nudge did not scale: {two}"
        );

        // AND A GRID RUN IS UNTOUCHED, because `Size::Grid` means "whatever
        // the renderer's own size is" and the renderer's own is already inked.
        assert_eq!(Size::Grid.times(3.0), Size::Grid);
        assert_eq!(Size::Px(9.0).times(1.5), Size::Px(13.5));
    }

    #[test]
    fn an_icon_on_the_cell_does_not_move_the_line() {
        // THE DEFECT THIS EXISTS FOR, and the gate is what found it. See
        // `line_box` for the whole story.
        //
        // The modeline is text at the grid's size plus ONE bell at 22 px. With
        // the bell carrying a box of its own, every letter beside it moved down
        // 2,9 px — silently, inside its own background.
        let icon = Size::OnTheCell(22.0);
        assert_eq!(line_box(icon, LH), LH, "the icon brought a box of its own");

        let boxes: Vec<(u16, f32)> = vec![
            (0, line_box(Size::Grid, LH)),
            (0, line_box(icon, LH)),
            (0, line_box(Size::Grid, LH)),
        ];
        let tops = aligned_tops(&boxes, LH);
        assert_eq!(
            tops,
            vec![0.0, 0.0, 0.0],
            "the line moved down because of the icon"
        );

        // And the guard on the other side: a run that DOES carry its own box
        // still moves the line, which is what the glyph browser wants.
        let browser = aligned_tops(&[(0, line_box(Size::Px(40.0), LH)), (0, TEXT)], LH);
        assert!(
            browser[1] > 0.0,
            "the browser lost its alignment down the middle"
        );
    }

    #[test]
    fn a_line_of_a_single_size_does_not_move() {
        // The footer and the modeline never mix sizes. If this test
        // fails, the alignment change touched something that had nothing to
        // gain from it — and touching the footer is touching the whole system.
        let tops = aligned_tops(&[(0, TEXT), (0, TEXT), (1, TEXT)], LH);
        assert_eq!(tops, vec![0.0, 0.0, LH]);
    }

    #[test]
    fn the_smaller_sits_in_the_middle_of_the_larger() {
        // The browser header's case: a 40 px magnifying glass and the 18 px search
        // field on the SAME line. What the owner asked for, literally: "the middle
        // of the line of icons and of texts aligned".
        let tops = aligned_tops(&[(0, ICON), (0, TEXT)], LH);
        assert_eq!(tops[0], 0.0, "the largest decides, and it does not move");
        // The proof is the MIDDLE, and not the top: the centre of each box has to fall
        // in the same place, which is what "being on the same line" means
        // when the sizes differ.
        let medium = |top: f32, bounds: f32| top + bounds / 2.0;
        assert!((medium(tops[0], ICON) - medium(tops[1], TEXT)).abs() < 0.001);
    }

    #[test]
    fn each_line_has_its_own_middle() {
        // A large icon on line 0 must not push line 1's text: aligning
        // happens within the line, otherwise the whole screen slides.
        let tops = aligned_tops(&[(0, ICON), (0, TEXT), (1, TEXT)], LH);
        assert_eq!(tops[2], LH);
    }
}
