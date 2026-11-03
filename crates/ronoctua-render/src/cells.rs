//! A GRID OF CELLS, drawn the way a terminal draws one — a glyph per cell,
//! each in its own colours and style, at the GPU's speed.
//!
//! # Why the text path is not enough
//!
//! Because [`crate::Span`] is one colour, no style and a run shaped again on
//! every frame. A grid is the opposite on every count: each cell has its own
//! foreground, background, weight, slant and lines; most of it does not change
//! from one frame to the next; and it is redrawn as fast as a program writes.
//! Splitting a screen of `ls --color` into one run per colour, reshaped per
//! frame, was estimated at 10–30 ms for eight thousand cells — a frame
//! budget gone before the first pixel.
//!
//! So a grid is a primitive of its own, and generic: it is named for what it
//! draws and never for who uses it — the base never names a feature
//! (`the-shell-foundation-names-no-feature`).
//!
//! # The data, and the two halves that draw it
//!
//! - [`Sheet`] is the data: the geometry in whole physical pixels, rows of
//!   [`Cell`] each with a generation, the default colours, the cursor, the
//!   selection and the pictures. Whoever owns the grid writes it.
//! - [`Assembly`] turns a sheet into instances — rebuilding ONLY the rows
//!   whose generation moved — and rasterises what it has not seen into an
//!   atlas of its own. It needs no device, so it can be benched alone.
//! - [`Cells`] is the GPU side: it uploads only the ranges that changed and
//!   draws, in this order — backgrounds, glyphs, the lines under, through and
//!   over them, the cursor, the selection.
//!
//! # Colours are data here
//!
//! [`Rgba`] is a colour a PROGRAM chose: SGR's 256 colours and 24-bit ones,
//! what OSC 4/10/11 changed. That is content, like an emoji's colours or a
//! picture's (the owner's exception to invariant 9); the theme's roles stay
//! roles, and the CALLER resolves them into an [`Rgba`] on the page it wears
//! ([`Rgba::of`]) — this module never names one.
//!
//! # The glyphs
//!
//! Each grapheme is SHAPED once per style through the door's own
//! `FontSystem`, so the declared fallback chain (`crate::chain`) answers here
//! exactly as it answers for the interface's text, and rasterised by swash
//! into an atlas keyed by face, glyph, size, scale and the synthetic styles —
//! size and scale in the key, because a raster at another one is another
//! drawing. A wide glyph is centred on its two cells and drawn smaller when it
//! is wider than them; a spacer draws nothing; box drawing, blocks, braille
//! and the rest of [`crate::sprite`] are drawn at the cell's exact size.

use crate::sprite::{self, Frame, Stroke};
use cosmic_text::{
    Attrs, AttrsList, CacheKey, CacheKeyFlags, Family, FontSystem, ShapeLine, Shaping, SwashCache,
    SwashContent, Weight, fontdb,
};
use ronoctua_tokens as tokens;
use std::collections::HashMap;
use std::ops::Range;

/// The atlas's side never grows past this, in texels.
///
/// A BOUND, because every cache has one (`assets/cache.nix`): an atlas that
/// only grew would be a screen of CJK at three sizes and two scales held in
/// video memory for ever. At this side a full atlas is 16 MiB of masks and
/// 64 MiB of colour; when it fills, it starts over — every raster dropped and
/// the rows that drew from it rebuilt — and only what the screen shows comes
/// back.
///
/// TODO(cache.nix): the number belongs in `assets/cache.nix` beside the other
/// caches' bounds, reaching Rust through `nix/cache.nix`'s `cache.rs` — which
/// this crate does not read today (`RONOCTUA_CACHE_RS` is not in its `env`),
/// and adding it rebuilds every unit above the renderer.
pub const ATLAS_SIDE_MOST: u32 = 4096;

/// The side an atlas is born at: a screen of Latin text at one size fits in
/// it several times over, so most windows never grow one.
const ATLAS_SIDE_FIRST: u32 = 512;

/// How many graphemes, each in its four styles, stay shaped.
///
/// The same bound's family as [`ATLAS_SIDE_MOST`] (TODO(cache.nix)): past it
/// the shapes are dropped and shaped again as the screen asks for them, which
/// costs a few frames of shaping and never a wrong glyph.
const SHAPES_MOST: usize = 8192;

/// How many times one assembly may start an atlas over. Twice: once because
/// it was full, and once more for a screen whose glyphs did not fit even then
/// — after that the glyphs that do not fit are not drawn, and the log says so,
/// rather than a frame that never ends.
const STARTS_OVER_MOST: u8 = 2;

/// The tab width a grapheme is shaped with. A cell never holds a tab — the
/// program's own tab stops move the cursor — so the number only has to be
/// one cosmic-text accepts.
const TAB_WIDTH: u16 = 8;

/// A COLOUR A PROGRAM CHOSE, in sRGB bytes — data, as an emoji's colours or a
/// picture's are, and not a role (see the module's head for whose exception
/// to invariant 9 this is).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha.
    pub a: u8,
}

impl Rgba {
    /// A colour, with its alpha.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// An opaque colour.
    #[must_use]
    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, u8::MAX)
    }

    /// A ROLE, resolved on the page the process wears NOW — how the caller
    /// turns the theme's colours (the sixteen ANSI ones, the defaults, the
    /// cursor, the selection) into the data a sheet carries. Resolved again
    /// at every change of theme, by whoever resolved it the first time.
    #[must_use]
    pub fn of(role: tokens::Color) -> Self {
        let [r, g, b, a] = role.to_rgba8();
        Self::new(r, g, b, a)
    }

    /// Components in `[0, 1]`, in LINEAR space, the alpha left as it is —
    /// what an `Srgb` target wants (see `tokens::Color::to_linear` for the
    /// lightening that passing sRGB straight through causes).
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

    /// The same colour with its alpha multiplied by `by / 255` — a faint
    /// cell's ink.
    const fn faded(self, by: u8) -> Self {
        let a = (self.a as u16 * by as u16 / 255) as u8;
        Self::new(self.r, self.g, self.b, a)
    }
}

/// How many cells a cell's glyph takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Width {
    /// One.
    #[default]
    Narrow,
    /// Two: this cell and the spacer after it.
    Wide,
    /// The second half of a wide cell. It draws nothing of its own but its
    /// background.
    Spacer,
}

/// The kinds of line under a cell — SGR 4 and its colon forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Underline {
    /// None.
    #[default]
    None,
    /// One straight line.
    Single,
    /// Two.
    Double,
    /// A wave.
    Curly,
    /// Dots.
    Dotted,
    /// Dashes.
    Dashed,
}

/// How a cell's glyph is drawn. INVERSE is not here: the caller swaps the two
/// colours before they reach the cell, because which colours to swap
/// (a default, a role, a program's) is the caller's knowledge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Style {
    /// Bold: the bold face when it is loaded, the regular one thickened when
    /// it is not.
    pub bold: bool,
    /// Italic: the italic face when it is loaded, the regular one slanted
    /// when it is not.
    pub italic: bool,
    /// Faint: the ink at [`Defaults::faint`].
    pub faint: bool,
    /// Blinking: the ink comes and goes with [`Sheet::set_blink_off`].
    pub blink: bool,
    /// Invisible: the background is drawn, and nothing over it.
    pub invisible: bool,
    /// A line through it.
    pub strike: bool,
    /// A line over it.
    pub overline: bool,
    /// The line under it.
    pub underline: Underline,
}

/// What a cell holds: one grapheme cluster — a character, or a character and
/// its marks, or an emoji's whole sequence.
///
/// One `char` is the common case and costs no allocation; anything longer is
/// boxed. A blank cell is a space.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Grapheme {
    /// A cluster of one character.
    One(char),
    /// A longer cluster.
    Many(Box<str>),
}

impl Default for Grapheme {
    fn default() -> Self {
        Self::One(' ')
    }
}

impl Grapheme {
    /// The cluster `text` is, one character costing nothing. Empty is a
    /// blank cell.
    #[must_use]
    pub fn of(text: &str) -> Self {
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (None, _) => Self::One(' '),
            (Some(c), None) => Self::One(c),
            _ => Self::Many(text.into()),
        }
    }

    /// The one character of a cluster of one.
    const fn sole(&self) -> Option<char> {
        match self {
            Self::One(c) => Some(*c),
            Self::Many(_) => None,
        }
    }

    /// Whether it draws nothing: a space, or a character no shaper should be
    /// handed — a control, or a line or paragraph separator, which would make
    /// a cell of its own a second paragraph.
    fn is_blank(&self) -> bool {
        let unshaped = |c: char| c == ' ' || c.is_control() || matches!(c, '\u{2028}' | '\u{2029}');
        match self {
            Self::One(c) => unshaped(*c),
            Self::Many(s) => s.chars().all(unshaped) || s.chars().any(|c| c.is_control()),
        }
    }
}

/// One cell of a [`Sheet`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cell {
    /// What it shows.
    pub grapheme: Grapheme,
    /// Its ink.
    pub fg: Rgba,
    /// Its ground — `None` for the sheet's own ([`Defaults::background`]),
    /// which is the window's clear colour and is not drawn again.
    pub bg: Option<Rgba>,
    /// The colour of the line under it (SGR 58) — `None` for the ink's.
    pub underline_colour: Option<Rgba>,
    /// How it is drawn.
    pub style: Style,
    /// How many cells it takes.
    pub width: Width,
}

/// THE CELL'S SHAPE IN PHYSICAL PIXELS, measured from the face — every number
/// whole, so a grid is integer arithmetic and two neighbours never share a
/// fractional pixel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    /// A cell's width.
    pub cell_width: u32,
    /// A cell's height.
    pub cell_height: u32,
    /// Where the baseline is, from the cell's top.
    pub baseline: u32,
    /// A light line's thickness — the underline's, which is what a
    /// box-drawing line and a bar cursor are drawn with.
    pub stroke: u32,
    /// The underline.
    pub underline: Stroke,
    /// The strike-through.
    pub strike: Stroke,
    /// The overline.
    pub overline: Stroke,
    /// The size the face is drawn at, in LOGICAL pixels.
    pub size: f32,
    /// The screen's scale — physical pixels per logical one.
    pub scale: u16,
}

impl Geometry {
    /// The geometry of the primary face at `size` logical pixels on a screen
    /// of `scale`, or `None` when the face is not in `fonts` or the size is
    /// not one.
    ///
    /// # The arithmetic, and why it is Ghostty's
    ///
    /// The cell is the face's advance and its ascent, descent and gap, each
    /// rounded to the nearest whole pixel — not the interface's 1.45 line:
    /// the terminal is a font's grid, and the same rounding Ghostty 1.4 makes
    /// is what lets the two draw the same program at the same size and look
    /// the same. Half the gap goes above and half below, and the baseline is
    /// placed so the rounding falls evenly on both sides. A face that names no
    /// underline or strike-through gets Ghostty's estimates from its x-height,
    /// and a stroke is never thinner than a pixel.
    #[must_use]
    pub fn measure(fonts: &mut FontSystem, size: f32, scale: u16) -> Option<Self> {
        if size.is_nan() || size <= 0.0 || scale == 0 {
            return None;
        }
        let ink = size * f32::from(scale);
        let attrs = Attrs::new().family(Family::Name(tokens::font::FAMILY_MONO));
        let line = ShapeLine::new(
            fonts,
            "M",
            &AttrsList::new(&attrs),
            Shaping::Advanced,
            TAB_WIDTH,
        );
        let glyph = line
            .spans
            .iter()
            .flat_map(|s| &s.words)
            .flat_map(|w| &w.glyphs)
            .next()?;
        let (id, advance) = (glyph.font_id, glyph.x_advance);
        let font = fonts.get_font(id, Weight::NORMAL)?;
        let m = font.metrics();
        let upem = f32::from(m.units_per_em);
        if upem <= 0.0 {
            return None;
        }
        let px = |v: f32| v / upem * ink;
        let (ascent, descent, gap) = (px(m.ascent), px(m.descent), px(m.leading).max(0.0));
        let face_height = ascent - descent + gap;
        let width = (advance * ink).round().max(1.0);
        let height = face_height.round().max(1.0);
        // THE BASELINE, counted from the bottom and then turned round: half
        // the gap under the descent, and half of the rounding's difference.
        let from_bottom = (gap / 2.0 - descent - (height - face_height) / 2.0).round();
        let baseline = (height - from_bottom).clamp(0.0, height);
        // THE ESTIMATES, for a face that does not say: a cap is three
        // quarters of the ascent, an x three quarters of a cap.
        let cap = m
            .cap_height
            .map(px)
            .filter(|v| *v > 0.0)
            .unwrap_or(0.75 * ascent);
        let ex = m
            .x_height
            .map(px)
            .filter(|v| *v > 0.0)
            .unwrap_or(0.75 * cap);
        let thickness = |d: Option<f32>, otherwise: f32| {
            d.filter(|v| *v > 0.0).unwrap_or(otherwise).ceil().max(1.0)
        };
        let under_thickness = thickness(m.underline.as_ref().map(|d| px(d.thickness)), 0.15 * ex);
        let under_offset = m
            .underline
            .as_ref()
            .map_or(-under_thickness, |d| px(d.offset));
        let strike_thickness = thickness(
            m.strikeout.as_ref().map(|d| px(d.thickness)),
            under_thickness,
        );
        let strike_offset = m
            .strikeout
            .as_ref()
            .map_or(f32::midpoint(ex, strike_thickness), |d| px(d.offset));
        // A LINE THAT WOULD LEAVE THE CELL is kept inside it: what the cell
        // draws is clipped to the cell, and an underline nobody sees is not one.
        let inside = |top: f32, t: f32| top.round().clamp(0.0, (height - t).max(0.0));
        let whole = |v: f32| v as u32;
        Some(Self {
            cell_width: whole(width),
            cell_height: whole(height),
            baseline: whole(baseline),
            stroke: whole(under_thickness),
            underline: Stroke {
                top: inside(baseline - under_offset, under_thickness) as i32,
                thickness: whole(under_thickness),
            },
            strike: Stroke {
                top: inside(baseline - strike_offset, strike_thickness) as i32,
                thickness: whole(strike_thickness),
            },
            overline: Stroke {
                top: 0,
                thickness: whole(under_thickness),
            },
            size,
            scale,
        })
    }

    /// The size the glyphs are rasterised at, in physical pixels.
    #[must_use]
    pub fn ink(&self) -> f32 {
        self.size * f32::from(self.scale)
    }

    /// How many whole cells fit across `width` physical pixels.
    #[must_use]
    pub fn columns_in(&self, width: u32) -> u16 {
        u16::try_from(width / self.cell_width.max(1)).unwrap_or(u16::MAX)
    }

    /// How many whole lines fit down `height` physical pixels.
    #[must_use]
    pub fn lines_in(&self, height: u32) -> u16 {
        u16::try_from(height / self.cell_height.max(1)).unwrap_or(u16::MAX)
    }

    /// The sprite frame of a span of `cells` cells.
    const fn frame(&self, cells: u32) -> Frame {
        Frame {
            cell: self.cell_width,
            width: self.cell_width * cells,
            height: self.cell_height,
            stroke: self.stroke,
            underline: self.underline,
            strike: self.strike,
            overline: self.overline,
        }
    }
}

/// The colours a sheet assumes where its cells say none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Defaults {
    /// The ink of a blank cell ([`Sheet::blank`]).
    pub foreground: Rgba,
    /// The ground: the window's clear colour, and so never drawn as a cell's.
    pub background: Rgba,
    /// The alpha a faint cell's ink is drawn at, out of 255 — the caller's,
    /// read from a token, because a renderer that picked one would be a
    /// visual value written by hand.
    pub faint: u8,
}

/// The cursor's shape — DECSCUSR's three, and the hollow block a cursor wears
/// when its window does not have the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CursorShape {
    /// The whole cell, its glyph drawn over it in [`Cursor::text`].
    #[default]
    Block,
    /// A stroke down the cell's left edge.
    Bar,
    /// A stroke where the underline goes.
    Underline,
    /// The block's outline.
    Hollow,
}

/// Where the cursor is and how it looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// Its line.
    pub line: u16,
    /// Its column. On a spacer it is drawn on the wide cell the spacer ends.
    pub column: u16,
    /// Its shape.
    pub shape: CursorShape,
    /// Whether it is drawn at all — a program hides it (DECTCEM), and a blink
    /// takes it away half the time.
    pub visible: bool,
    /// Its colour.
    pub colour: Rgba,
    /// The colour of the glyph under a block.
    pub text: Rgba,
}

/// A run of one line's cells laid over the ink — a selection, a match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Band {
    /// The line.
    pub line: u16,
    /// The columns it covers.
    pub columns: Range<u16>,
    /// Its colour, translucent for the ink under it to show.
    pub colour: Rgba,
}

/// Where a picture goes in the stack of passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layer {
    /// Under the backgrounds.
    UnderPaper,
    /// Over the backgrounds and under the glyphs.
    UnderInk,
    /// Over everything.
    OverInk,
}

/// A picture placed on the grid (the kitty graphics' placements).
///
/// TODO(phase 6, kitty graphics): a sheet carries its pictures and
/// [`Cells`] draws NONE of them yet. Drawing them is the door's
/// `image::Images` between the passes — under the paper, under the ink, over
/// it — which wants [`Cells::draw`] split at those three points and the
/// pictures' textures keyed and capped beside the frame's other pictures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// The texture's key in `image::Images`.
    pub key: String,
    /// Its top-left corner, in physical pixels from the grid's.
    pub at: (i32, i32),
    /// Its size, in physical pixels.
    pub size: (u32, u32),
    /// Where it goes in the stack.
    pub layer: Layer,
}

/// One line of cells, and the generation it was last written at.
#[derive(Debug, Clone)]
struct Row {
    cells: Vec<Cell>,
    generation: u64,
}

/// THE CPU DATA OF A GRID: what each cell shows, and the rest of a frame.
///
/// # Generations, so a frame redraws what changed
///
/// Every row carries the generation it was last written at, and every way of
/// writing a row ([`Sheet::row_mut`]) moves it — there is no way to change a
/// cell and forget to say so. A change that touches every row (the geometry,
/// the defaults, the size) moves the sheet's own `front` instead. The
/// [`Assembly`] keeps what it built of each row and rebuilds a row only when
/// its generation is not the one it built.
///
/// ONE CLOCK FOR EVERY SHEET of the process, so no generation is ever minted
/// twice: a CLONE keeps its original's id and the generations it had, and the
/// two then write on with numbers neither shares — an assembly handed one and
/// then the other rebuilds exactly the rows that differ, where a clock of each
/// sheet's own would give the same number to two different writes and leave
/// one of them drawn as the other.
#[derive(Debug, Clone)]
pub struct Sheet {
    /// Which sheet this is, so an assembly handed another one rebuilds it
    /// whole instead of taking its generations for its own.
    id: u64,
    geometry: Geometry,
    columns: u16,
    rows: Vec<Row>,
    defaults: Defaults,
    /// Where the grid's top-left corner sits on the surface, in physical
    /// pixels. Moving it moves the whole grid without rebuilding a row.
    pub origin: (i32, i32),
    /// The cursor, when there is one.
    pub cursor: Option<Cursor>,
    /// What is laid over the ink — the selection, a search's matches.
    pub selection: Vec<Band>,
    /// The pictures on the grid. See [`Picture`]: not drawn yet.
    pub pictures: Vec<Picture>,
    blink_off: bool,
    front: u64,
}

/// The generations' clock — see [`Sheet`].
static CLOCK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Sheet {
    /// A sheet of `columns × lines` blank cells.
    #[must_use]
    pub fn new(geometry: Geometry, columns: u16, lines: u16, defaults: Defaults) -> Self {
        static SHEETS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let mut sheet = Self {
            id: SHEETS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            geometry,
            columns: 0,
            rows: Vec::new(),
            defaults,
            origin: (0, 0),
            cursor: None,
            selection: Vec::new(),
            pictures: Vec::new(),
            blink_off: false,
            front: 0,
        };
        sheet.resize(columns, lines);
        sheet
    }

    /// The next generation, never one any sheet had.
    fn tick() -> u64 {
        CLOCK.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// The cell's shape.
    #[must_use]
    pub const fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    /// A new shape — a size, a scale — and every row drawn again.
    pub fn set_geometry(&mut self, geometry: Geometry) {
        if geometry != self.geometry {
            self.geometry = geometry;
            self.front = Self::tick();
        }
    }

    /// The default colours.
    #[must_use]
    pub const fn defaults(&self) -> &Defaults {
        &self.defaults
    }

    /// New default colours — a theme changed — and every row drawn again.
    pub fn set_defaults(&mut self, defaults: Defaults) {
        if defaults != self.defaults {
            self.defaults = defaults;
            self.front = Self::tick();
        }
    }

    /// A blank cell in the default colours.
    #[must_use]
    pub fn blank(&self) -> Cell {
        Cell {
            fg: self.defaults.foreground,
            ..Cell::default()
        }
    }

    /// How many cells across.
    #[must_use]
    pub const fn columns(&self) -> u16 {
        self.columns
    }

    /// How many lines down.
    #[must_use]
    pub fn lines(&self) -> u16 {
        u16::try_from(self.rows.len()).unwrap_or(u16::MAX)
    }

    /// The grid's size in physical pixels.
    #[must_use]
    pub fn pixels(&self) -> (u32, u32) {
        (
            u32::from(self.columns) * self.geometry.cell_width,
            u32::from(self.lines()) * self.geometry.cell_height,
        )
    }

    /// A new size: what fits is kept, the rest is blank, and every row is
    /// drawn again.
    pub fn resize(&mut self, columns: u16, lines: u16) {
        let blank = self.blank();
        self.rows.truncate(usize::from(lines));
        while self.rows.len() < usize::from(lines) {
            self.rows.push(Row {
                cells: Vec::new(),
                generation: 0,
            });
        }
        for row in &mut self.rows {
            row.cells.resize(usize::from(columns), blank.clone());
        }
        self.columns = columns;
        self.front = Self::tick();
    }

    /// Line `line`'s cells — none past the last line.
    #[must_use]
    pub fn row(&self, line: u16) -> &[Cell] {
        self.rows.get(usize::from(line)).map_or(&[], |r| &r.cells)
    }

    /// Line `line`'s cells, to write — and the line is drawn again on the
    /// next frame. None past the last line.
    pub fn row_mut(&mut self, line: u16) -> &mut [Cell] {
        let generation = Self::tick();
        match self.rows.get_mut(usize::from(line)) {
            Some(row) => {
                row.generation = generation;
                &mut row.cells
            }
            None => &mut [],
        }
    }

    /// The generation line `line` was last written at.
    #[must_use]
    pub fn generation_of(&self, line: u16) -> u64 {
        self.rows.get(usize::from(line)).map_or(0, |r| r.generation)
    }

    /// Whether blinking cells are in their dark half: their ink is not
    /// drawn. Only the rows that blink are drawn again.
    pub const fn set_blink_off(&mut self, off: bool) {
        self.blink_off = off;
    }

    /// Whether blinking cells are in their dark half.
    #[must_use]
    pub const fn blink_off(&self) -> bool {
        self.blink_off
    }
}

/// What a raster is drawn from: one byte of coverage per texel, or four of
/// colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    /// Coverage, the cell's colour applied where it is drawn.
    Mask,
    /// An emoji's own colours.
    Colour,
}

/// How an instance is drawn — the shader's `kind`.
const SOLID: u32 = 0;
/// See [`SOLID`].
const MASK: u32 = 1;
/// See [`SOLID`].
const COLOUR: u32 = 2;

/// One instance as the GPU sees it: a rectangle in physical pixels from the
/// grid's origin, the colour, where its raster is, and how to draw it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    rect: [f32; 4],
    colour: [f32; 4],
    texel: [f32; 2],
    kind: u32,
    padding: u32,
}

impl Instance {
    /// A flat rectangle.
    fn solid(x: i32, y: i32, width: u32, height: u32, colour: Rgba) -> Self {
        Self {
            rect: [x as f32, y as f32, width as f32, height as f32],
            colour: colour.to_linear(),
            texel: [0.0, 0.0],
            kind: SOLID,
            padding: 0,
        }
    }

    /// A raster of the atlas, its top-left at `(x, y)`.
    fn sampled(x: i32, y: i32, placed: Placed, colour: Rgba) -> Self {
        let (kind, colour) = match placed.kind {
            Kind::Mask => (MASK, colour.to_linear()),
            // AN EMOJI KEEPS ITS COLOURS: only the alpha is the cell's, so a
            // faint cell's emoji is faint too.
            Kind::Colour => (COLOUR, [1.0, 1.0, 1.0, f32::from(colour.a) / 255.0]),
        };
        Self {
            rect: [
                x as f32,
                y as f32,
                placed.width as f32,
                placed.height as f32,
            ],
            colour,
            texel: [placed.x as f32, placed.y as f32],
            kind,
            padding: 0,
        }
    }
}

/// Where a raster sits in its atlas, and where it sits against its pen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Placed {
    kind: Kind,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    /// From the pen to the raster's left edge — a sprite's is zero.
    left: i32,
    /// From the baseline UP to the raster's top — a sprite is placed from the
    /// cell's top instead, and its is zero.
    top: i32,
}

/// What a raster is keyed by in the atlas.
///
/// EVERYTHING THAT CHANGES THE DRAWING is in it (AGENTS: "a chave leva tudo o
/// que muda o desenho"): a face glyph's size in logical pixels AND the scale,
/// the shrink a wide glyph took, the weight and the synthetic styles; a sprite's whole
/// frame. No palette: a mask is coverage and an emoji's colours are its own,
/// so no raster here is in any theme's colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    /// A glyph of a face.
    Face {
        font: fontdb::ID,
        glyph: u16,
        /// The logical size's `f32` bits.
        size: u32,
        scale: u16,
        /// The factor a glyph wider than its cells was drawn at, as `f32` bits.
        shrink: u32,
        /// The weight asked for: a variable face is drawn at it (its `wght`
        /// axis), so its bold and its regular are one glyph id and two
        /// drawings.
        weight: u16,
        /// Slanted by the rasteriser: no italic face answered.
        italic: bool,
        /// Thickened here: no bold face answered.
        bold: bool,
    },
    /// A glyph [`crate::sprite`] draws.
    Sprite { c: char, frame: Frame },
    /// A line under, through or over a cell.
    Line { line: sprite::Line, frame: Frame },
}

/// A raster before it has a place.
struct Raster {
    kind: Kind,
    width: u32,
    height: u32,
    left: i32,
    top: i32,
    data: Vec<u8>,
}

/// A raster waiting to go up to its atlas's texture.
#[derive(Debug, Clone)]
struct Upload {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    data: Vec<u8>,
}

/// A row of rasters in an atlas: its top, its height, and how far along it
/// is full.
#[derive(Debug, Clone, Copy)]
struct Shelf {
    y: u32,
    height: u32,
    right: u32,
}

/// THE CPU HALF OF AN ATLAS: where rasters go, and the ones still to upload.
///
/// SHELVES, and not a general packer: a grid's rasters are all about a cell
/// tall, so a shelf per height wastes little, and it is twenty lines a test
/// can hold — where `etagere` is a dependency this crate has not got.
#[derive(Debug)]
struct Atlas {
    side: u32,
    shelves: Vec<Shelf>,
    bottom: u32,
    uploads: Vec<Upload>,
}

impl Atlas {
    fn new(side: u32) -> Self {
        Self {
            side,
            shelves: Vec::new(),
            bottom: 0,
            uploads: Vec::new(),
        }
    }

    /// Where a `width × height` raster goes, or `None` when it is full: the
    /// lowest shelf it fits on with little to spare, or a new one under them.
    fn place(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width > self.side || height > self.side {
            return None;
        }
        let side = self.side;
        let fits = |s: &Shelf| {
            s.height >= height && s.height <= height + height / 4 + 2 && side - s.right >= width
        };
        let best = self
            .shelves
            .iter_mut()
            .filter(|s| fits(s))
            .min_by_key(|s| s.height);
        if let Some(s) = best {
            let at = (s.right, s.y);
            s.right += width;
            return Some(at);
        }
        if side - self.bottom < height {
            return None;
        }
        let at = (0, self.bottom);
        self.shelves.push(Shelf {
            y: self.bottom,
            height,
            right: width,
        });
        self.bottom += height;
        Some(at)
    }

    /// Empty, at `side` — what was placed and what was waiting to go up.
    fn start_over(&mut self, side: u32) {
        self.side = side;
        self.shelves.clear();
        self.bottom = 0;
        self.uploads.clear();
    }
}

/// A grapheme shaped once in one style: its glyphs, placed from the pen in
/// multiples of the font size, and how far it advances.
#[derive(Debug, Clone)]
struct Shaped {
    glyphs: Vec<Shape>,
    advance: f32,
}

/// One shaped glyph.
#[derive(Debug, Clone, Copy)]
struct Shape {
    font: fontdb::ID,
    glyph: u16,
    /// The weight asked for — what the rasteriser selects the face's
    /// instance by.
    weight: Weight,
    /// From the pen, in em.
    x: f32,
    /// Up from the baseline, in em.
    y: f32,
    /// The face that answered is not italic, and the rasteriser slants it.
    italic: bool,
    /// The face that answered is not bold, and this module thickens it.
    bold: bool,
}

/// What was built of one row: its three layers, the generation it was built
/// at, and whether anything on it blinks.
#[derive(Debug, Clone, Default)]
struct Built {
    generation: u64,
    blinks: bool,
    paper: Vec<Instance>,
    ink: Vec<Instance>,
    lines: Vec<Instance>,
}

/// One layer's instances, every row's end to end, and the ranges of it that
/// changed since they were last uploaded.
#[derive(Debug, Default)]
struct Strip {
    all: Vec<Instance>,
    lens: Vec<usize>,
    changed: Vec<Range<usize>>,
}

/// How many separate ranges a strip keeps before it uploads their span as
/// one: a handful of rows touched here and there is a handful of writes, a
/// scattered hundred is one.
const RANGES_MOST: usize = 32;

impl Strip {
    /// Says `range` changed, merged with the last one when they touch.
    fn mark(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        if let Some(last) = self.changed.last_mut()
            && range.start <= last.end
            && range.end >= last.start
        {
            *last = last.start.min(range.start)..last.end.max(range.end);
            return;
        }
        self.changed.push(range);
        if self.changed.len() > RANGES_MOST {
            let start = self.changed.iter().map(|r| r.start).min().unwrap_or(0);
            let end = self.changed.iter().map(|r| r.end).max().unwrap_or(0);
            self.changed.clear();
            self.changed.push(start..end);
        }
    }

    /// The rows' instances end to end, rewriting only what moved: a row
    /// rebuilt at the same length is written in place; from the first row
    /// whose length changed, everything after it is.
    fn stitch<'a>(&mut self, rows: impl Iterator<Item = (&'a [Instance], bool)>) {
        let rows: Vec<(&[Instance], bool)> = rows.collect();
        let shift = rows
            .iter()
            .enumerate()
            .find(|(i, (row, _))| self.lens.get(*i) != Some(&row.len()))
            .map_or(rows.len(), |(i, _)| i);
        let mut at = 0;
        for (row, rebuilt) in &rows[..shift] {
            if *rebuilt {
                self.all[at..at + row.len()].copy_from_slice(row);
                self.mark(at..at + row.len());
            }
            at += row.len();
        }
        if shift < rows.len() || self.all.len() != at {
            self.all.truncate(at);
            for (row, _) in &rows[shift..] {
                self.all.extend_from_slice(row);
            }
            let end = self.all.len();
            self.mark(at..end);
        }
        self.lens = rows.iter().map(|(row, _)| row.len()).collect();
    }
}

/// What one assembly did — for a test, and for a bench.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Assembled {
    /// Rows rebuilt.
    pub rows: usize,
    /// Rasters drawn and placed for the first time.
    pub rasters: usize,
    /// Times an atlas started over.
    pub starts_over: usize,
    /// Rasters NOT drawn, because the atlas was full again after starting
    /// over as often as one assembly may.
    pub refused: usize,
}

/// What an assembly last built against: which sheet, its front, and the
/// atlases' epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    sheet: u64,
    front: u64,
    epoch: u64,
}

/// THE CPU HALF of [`Cells`]: a sheet into instances, and the rasters they
/// draw from — with no device, so a bench can time it alone.
pub struct Assembly {
    swash: SwashCache,
    /// Every shape, by index — the maps below name them.
    shapes: Vec<Shaped>,
    /// One character's index in `shapes`, in each of the four styles.
    by_char: HashMap<char, [Option<usize>; 4]>,
    /// A longer cluster's.
    by_cluster: HashMap<Box<str>, [Option<usize>; 4]>,
    /// Every raster asked for, `None` for one that has no ink.
    glyphs: HashMap<Key, Option<Placed>>,
    masks: Atlas,
    colours: Atlas,
    side_most: u32,
    /// Moves every time an atlas starts over: every texel coordinate built
    /// before it points at nothing.
    epoch: u64,
    starts_over_left: u8,
    built: Vec<Built>,
    stamp: Option<Stamp>,
    blink_off: bool,
    paper: Strip,
    ink: Strip,
    lines: Strip,
    /// The cursor's instances, then the selection's — rebuilt every frame,
    /// because they are few and move without a row being written.
    over: Vec<Instance>,
    origin: (i32, i32),
    done: Assembled,
}

impl Default for Assembly {
    fn default() -> Self {
        Self::new()
    }
}

impl Assembly {
    /// An assembly whose atlases are born at [`ATLAS_SIDE_FIRST`] and grow to
    /// [`ATLAS_SIDE_MOST`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_sides(ATLAS_SIDE_FIRST, ATLAS_SIDE_MOST)
    }

    /// An assembly whose atlases are born at `first` texels a side and grow
    /// to `most` — a device's own ceiling, or a test's small one.
    #[must_use]
    pub fn with_sides(first: u32, most: u32) -> Self {
        let most = most.max(1);
        let first = first.clamp(1, most);
        Self {
            swash: SwashCache::new(),
            shapes: Vec::new(),
            by_char: HashMap::new(),
            by_cluster: HashMap::new(),
            glyphs: HashMap::new(),
            masks: Atlas::new(first),
            colours: Atlas::new(first),
            side_most: most,
            epoch: 0,
            starts_over_left: STARTS_OVER_MOST,
            built: Vec::new(),
            stamp: None,
            blink_off: false,
            paper: Strip::default(),
            ink: Strip::default(),
            lines: Strip::default(),
            over: Vec::new(),
            origin: (0, 0),
            done: Assembled::default(),
        }
    }

    /// The sheet as instances, rebuilding only the rows whose generation moved
    /// — every row when the sheet, its geometry, its defaults or an atlas
    /// changed — and rasterising what the atlases have not got.
    pub fn assemble(&mut self, sheet: &Sheet, fonts: &mut FontSystem) -> Assembled {
        self.done = Assembled::default();
        self.starts_over_left = STARTS_OVER_MOST;
        self.origin = sheet.origin;
        let lines = usize::from(sheet.lines());
        loop {
            let epoch = self.epoch;
            let stamp = Stamp {
                sheet: sheet.id,
                front: sheet.front,
                epoch,
            };
            let whole = self.stamp != Some(stamp) || self.built.len() != lines;
            self.built.resize_with(lines, Built::default);
            let blink_moved = self.blink_off != sheet.blink_off;
            let mut rebuilt = vec![false; lines];
            for (line, fresh) in rebuilt.iter_mut().enumerate() {
                let b = &self.built[line];
                let row = &sheet.rows[line];
                if whole || b.generation != row.generation || (b.blinks && blink_moved) {
                    let index = u16::try_from(line).unwrap_or(u16::MAX);
                    self.built[line] = self.build_row(sheet, index, fonts);
                    self.built[line].generation = row.generation;
                    *fresh = true;
                    self.done.rows += 1;
                }
                // AN ATLAS STARTED OVER under this row: every row built so far
                // draws from texels that are gone, so all of them again.
                if self.epoch != epoch {
                    break;
                }
            }
            if self.epoch != epoch {
                continue;
            }
            self.over.clear();
            self.cursor_of(sheet, fonts);
            if self.epoch != epoch {
                continue;
            }
            // A BAND PAST THE LAST LINE is not drawn, as a cursor there is
            // not: it would lie on whatever the window draws under the grid.
            for band in sheet.selection.iter().filter(|b| b.line < sheet.lines()) {
                self.over.push(band_of(sheet, band));
            }
            self.stamp = Some(stamp);
            self.blink_off = sheet.blink_off;
            let built = &self.built;
            self.paper
                .stitch(built.iter().zip(&rebuilt).map(|(b, f)| (&b.paper[..], *f)));
            self.ink
                .stitch(built.iter().zip(&rebuilt).map(|(b, f)| (&b.ink[..], *f)));
            self.lines
                .stitch(built.iter().zip(&rebuilt).map(|(b, f)| (&b.lines[..], *f)));
            // NOT SILENT, and said once a frame rather than once a glyph.
            if self.done.refused > 0 {
                tracing::warn!(
                    refused = self.done.refused,
                    "the cells' atlas filled again after starting over; some glyphs are not drawn"
                );
            }
            return self.done;
        }
    }

    /// One row's three layers.
    fn build_row(&mut self, sheet: &Sheet, line: u16, fonts: &mut FontSystem) -> Built {
        let g = *sheet.geometry();
        let defaults = *sheet.defaults();
        let cells = sheet.row(line);
        let columns = cells.len();
        let y = to_px(u32::from(line) * g.cell_height);
        let x_of = |column: usize| to_px(column as u32 * g.cell_width);
        let mut built = Built::default();

        // THE BACKGROUNDS, a rectangle per run of one colour — and none for
        // the sheet's own ground, which is the window's clear colour.
        let mut run: Option<(usize, Rgba)> = None;
        for column in 0..=columns {
            let bg = cells
                .get(column)
                .and_then(|c| c.bg)
                .filter(|bg| *bg != defaults.background);
            match (run, bg) {
                (Some((_, now)), Some(next)) if now == next => {}
                _ => {
                    if let Some((from, colour)) = run.take() {
                        let width = (column - from) as u32 * g.cell_width;
                        built.paper.push(Instance::solid(
                            x_of(from),
                            y,
                            width,
                            g.cell_height,
                            colour,
                        ));
                    }
                    run = bg.map(|bg| (column, bg));
                }
            }
        }

        // THE GLYPHS AND THEIR LINES.
        for (column, cell) in cells.iter().enumerate() {
            built.blinks |= cell.style.blink;
            if cell.width == Width::Spacer
                || cell.style.invisible
                || (cell.style.blink && sheet.blink_off)
            {
                continue;
            }
            let span = span_of(cell, column, columns);
            let ink = if cell.style.faint {
                cell.fg.faded(defaults.faint)
            } else {
                cell.fg
            };
            let x = x_of(column);
            self.glyph_into(&mut built.ink, &g, cell, (x, y), span, ink, fonts);
            let frame = g.frame(span);
            let under = cell.underline_colour.map_or(ink, |c| {
                if cell.style.faint {
                    c.faded(defaults.faint)
                } else {
                    c
                }
            });
            let mut lined = |line: sprite::Line, colour: Rgba, out: &mut Vec<Instance>| {
                let key = Key::Line { line, frame };
                let placed = match self.glyphs.get(&key) {
                    Some(p) => *p,
                    None => {
                        let raster = mask_of(sprite::line(line, &frame));
                        self.store(key, raster)
                    }
                };
                if let Some(p) = placed {
                    out.push(Instance::sampled(x, y, p, colour));
                }
            };
            if cell.style.underline != Underline::None {
                lined(
                    sprite::Line::Under(cell.style.underline),
                    under,
                    &mut built.lines,
                );
            }
            if cell.style.strike {
                lined(sprite::Line::Strike, ink, &mut built.lines);
            }
            if cell.style.overline {
                lined(sprite::Line::Over, ink, &mut built.lines);
            }
        }
        built
    }

    /// One cell's glyph — a sprite, or the shaped cluster's glyphs — into
    /// `out`, its span `span` cells from `(x, y)`.
    #[allow(clippy::too_many_arguments)]
    fn glyph_into(
        &mut self,
        out: &mut Vec<Instance>,
        g: &Geometry,
        cell: &Cell,
        (x, y): (i32, i32),
        span: u32,
        ink: Rgba,
        fonts: &mut FontSystem,
    ) {
        // DRAWN, NOT READ, when it is one of the sprites: at the cell's exact
        // size, from its top-left corner.
        if let Some(c) = cell.grapheme.sole()
            && sprite::covers(c)
        {
            let frame = g.frame(span);
            let key = Key::Sprite { c, frame };
            let placed = match self.glyphs.get(&key) {
                Some(p) => *p,
                None => {
                    let raster = sprite::glyph(c, &frame).and_then(mask_of);
                    self.store(key, raster)
                }
            };
            if let Some(p) = placed {
                out.push(Instance::sampled(x + p.left, y, p, ink));
            }
            return;
        }
        if cell.grapheme.is_blank() {
            return;
        }
        let Some(shaped) = self.shaped(fonts, &cell.grapheme, cell.style.bold, cell.style.italic)
        else {
            return;
        };
        // CENTRED ON ITS SPAN, and smaller when wider than it: a wide emoji
        // is a little wider than two cells, and a fallback face's glyph may be
        // wider than its one. The primary face's own advance rounds to the
        // cell, so it is never shrunk — the pixel of tolerance is that
        // rounding's.
        let ink_px = g.ink();
        let span_px = (span * g.cell_width) as f32;
        let advance = self.shapes[shaped].advance * ink_px;
        let shrink = if advance > span_px + 1.0 {
            span_px / advance
        } else {
            1.0
        };
        let size = ink_px * shrink;
        let pen = x + ((span_px - advance * shrink) / 2.0).round() as i32;
        let baseline = y + to_px(g.baseline);
        for at in 0..self.shapes[shaped].glyphs.len() {
            let s = self.shapes[shaped].glyphs[at];
            let key = Key::Face {
                font: s.font,
                glyph: s.glyph,
                size: g.size.to_bits(),
                scale: g.scale,
                shrink: shrink.to_bits(),
                weight: s.weight.0,
                italic: s.italic,
                bold: s.bold,
            };
            let placed = match self.glyphs.get(&key) {
                Some(p) => *p,
                None => {
                    let raster = raster_of(fonts, &mut self.swash, &s, size);
                    self.store(key, raster)
                }
            };
            if let Some(p) = placed {
                let gx = pen + (s.x * size).round() as i32 + p.left;
                let gy = baseline - (s.y * size).round() as i32 - p.top;
                out.push(Instance::sampled(gx, gy, p, ink));
            }
        }
    }

    /// The index of `grapheme` shaped in this style, shaping it the first
    /// time.
    fn shaped(
        &mut self,
        fonts: &mut FontSystem,
        grapheme: &Grapheme,
        bold: bool,
        italic: bool,
    ) -> Option<usize> {
        let style = usize::from(bold) | (usize::from(italic) << 1);
        let known = match grapheme {
            Grapheme::One(c) => self.by_char.get(c).and_then(|s| s[style]),
            Grapheme::Many(s) => self.by_cluster.get(&**s).and_then(|s| s[style]),
        };
        if known.is_some() {
            return known;
        }
        // THE BOUND: past it every shape goes, and comes back as asked.
        if self.shapes.len() >= SHAPES_MOST {
            self.shapes.clear();
            self.by_char.clear();
            self.by_cluster.clear();
        }
        let mut one = [0; 4];
        let text = match grapheme {
            Grapheme::One(c) => &*c.encode_utf8(&mut one),
            Grapheme::Many(s) => &**s,
        };
        let shaped = shape(fonts, text, bold, italic);
        let index = self.shapes.len();
        self.shapes.push(shaped);
        let slots = match grapheme {
            Grapheme::One(c) => self.by_char.entry(*c).or_default(),
            Grapheme::Many(s) => self.by_cluster.entry(s.clone()).or_default(),
        };
        slots[style] = Some(index);
        Some(index)
    }

    /// The cursor, into `over`.
    fn cursor_of(&mut self, sheet: &Sheet, fonts: &mut FontSystem) {
        let Some(c) = sheet.cursor.filter(|c| c.visible) else {
            return;
        };
        let g = *sheet.geometry();
        let row = sheet.row(c.line);
        let mut column = usize::from(c.column);
        // ON A SPACER, the cursor is the wide cell's.
        if row
            .get(column)
            .is_some_and(|cell| cell.width == Width::Spacer)
            && column > 0
            && row[column - 1].width == Width::Wide
        {
            column -= 1;
        }
        let Some(cell) = row.get(column) else {
            return;
        };
        let span = span_of(cell, column, row.len());
        let (x, y) = (
            to_px(column as u32 * g.cell_width),
            to_px(u32::from(c.line) * g.cell_height),
        );
        let (width, height, stroke) = (span * g.cell_width, g.cell_height, g.stroke.max(1));
        match c.shape {
            CursorShape::Block => {
                self.over
                    .push(Instance::solid(x, y, width, height, c.colour));
                // AND THE GLYPH AGAIN, over the block, in the cursor's text
                // colour — what makes the letter under a block readable.
                if !cell.style.invisible {
                    let mut glyph = Vec::new();
                    self.glyph_into(&mut glyph, &g, cell, (x, y), span, c.text, fonts);
                    self.over.extend(glyph);
                }
            }
            CursorShape::Bar => {
                self.over
                    .push(Instance::solid(x, y, stroke, height, c.colour));
            }
            CursorShape::Underline => {
                let s = g.underline;
                self.over.push(Instance::solid(
                    x,
                    y + s.top,
                    width,
                    s.thickness.max(1),
                    c.colour,
                ));
            }
            CursorShape::Hollow => {
                let (w, h, t) = (to_px(width), to_px(height), to_px(stroke));
                self.over
                    .push(Instance::solid(x, y, width, stroke, c.colour));
                self.over
                    .push(Instance::solid(x, y + h - t, width, stroke, c.colour));
                self.over
                    .push(Instance::solid(x, y, stroke, height, c.colour));
                self.over
                    .push(Instance::solid(x + w - t, y, stroke, height, c.colour));
            }
        }
    }

    /// A raster stored in its atlas — starting the atlas over when it is full
    /// — and its place; `None` for a raster with no ink, or one that does not
    /// fit even an empty atlas.
    fn store(&mut self, key: Key, raster: Option<Raster>) -> Option<Placed> {
        // A RASTER NO ATLAS COULD HOLD — wider or taller than an atlas may
        // ever grow — is not drawn, and is remembered as one with no ink:
        // starting over for it would wipe every raster, on every frame, to
        // make room that never comes. Said once, when it is first asked for.
        let most = self.side_most;
        let raster = raster.filter(|r| {
            let fits = r.width <= most && r.height <= most;
            if !fits {
                tracing::warn!(
                    width = r.width,
                    height = r.height,
                    most,
                    "a glyph is larger than the cells' atlas can ever be; it is not drawn"
                );
            }
            fits
        });
        let Some(r) = raster else {
            // A RASTER WITH NO INK takes no texel, so no atlas bounds how many
            // are remembered: past [`SHAPES_MOST`] of everything, they go.
            if self.glyphs.len() >= SHAPES_MOST {
                self.glyphs.retain(|_, p| p.is_some());
            }
            self.glyphs.insert(key, None);
            return None;
        };
        let at = match self.atlas(r.kind).place(r.width, r.height) {
            Some(at) => at,
            None => {
                if !self.start_over(r.kind) {
                    return None;
                }
                self.atlas(r.kind).place(r.width, r.height)?
            }
        };
        let placed = Placed {
            kind: r.kind,
            x: at.0,
            y: at.1,
            width: r.width,
            height: r.height,
            left: r.left,
            top: r.top,
        };
        self.atlas(r.kind).uploads.push(Upload {
            x: at.0,
            y: at.1,
            width: r.width,
            height: r.height,
            data: r.data,
        });
        self.glyphs.insert(key, Some(placed));
        self.done.rasters += 1;
        Some(placed)
    }

    /// The atlas of a kind.
    const fn atlas(&mut self, kind: Kind) -> &mut Atlas {
        match kind {
            Kind::Mask => &mut self.masks,
            Kind::Colour => &mut self.colours,
        }
    }

    /// The atlas of `kind`, full, starts over — twice as big while it may
    /// grow — and every raster in it is forgotten. `false` when this assembly
    /// may not start over again.
    fn start_over(&mut self, kind: Kind) -> bool {
        if self.starts_over_left == 0 {
            self.done.refused += 1;
            return false;
        }
        self.starts_over_left -= 1;
        let most = self.side_most;
        let atlas = self.atlas(kind);
        let side = atlas.side.saturating_mul(2).min(most);
        atlas.start_over(side);
        self.glyphs.retain(|_, p| p.is_none_or(|p| p.kind != kind));
        self.epoch += 1;
        self.done.starts_over += 1;
        tracing::debug!(?kind, side, "the cells' atlas starts over");
        true
    }
}

/// How many cells a cell's glyph spans, at `column` of a row `columns` wide:
/// two for a wide one, unless it is the row's last.
fn span_of(cell: &Cell, column: usize, columns: usize) -> u32 {
    if cell.width == Width::Wide && column + 1 < columns {
        2
    } else {
        1
    }
}

/// A count of pixels as a coordinate.
fn to_px(v: u32) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

/// A band, as an instance over the ink.
fn band_of(sheet: &Sheet, band: &Band) -> Instance {
    let g = sheet.geometry();
    let end = band.columns.end.min(sheet.columns());
    let from = band.columns.start.min(end);
    Instance::solid(
        to_px(u32::from(from) * g.cell_width),
        to_px(u32::from(band.line) * g.cell_height),
        u32::from(end - from) * g.cell_width,
        g.cell_height,
        band.colour,
    )
}

/// A sprite's coverage as a raster — `None` when it has no ink.
fn mask_of(c: sprite::Coverage) -> Option<Raster> {
    (!c.is_blank()).then_some(Raster {
        kind: Kind::Mask,
        width: c.width,
        height: c.height,
        left: 0,
        top: 0,
        data: c.data,
    })
}

/// `text` shaped as one cluster in this style, its glyphs placed from the pen
/// in multiples of the font size — through the door's own `FontSystem`, so
/// the fallback chain answers.
fn shape(fonts: &mut FontSystem, text: &str, bold: bool, italic: bool) -> Shaped {
    // BOLD IS ASKED OF THE SHAPER only where the primary family HAS a bold
    // face. cosmic-text keeps, for a weight, only the faces of that weight or
    // variable ones spanning it, so a bold asked of a family holding just its
    // Regular is answered by ANOTHER family's variable face — Noto Sans Mono,
    // of the coverage faces: a bold `x` in another typeface. Asked at the
    // regular weight, the cluster is the Regular's, and its bold is drawn
    // below, face by face.
    let shaped_bold = bold && family_has_bold(fonts);
    let attrs = Attrs::new()
        .family(Family::Name(tokens::font::FAMILY_MONO))
        .weight(if shaped_bold {
            Weight::BOLD
        } else {
            Weight::NORMAL
        })
        .style(if italic {
            cosmic_text::Style::Italic
        } else {
            cosmic_text::Style::Normal
        });
    let line = ShapeLine::new(
        fonts,
        text,
        &AttrsList::new(&attrs),
        Shaping::Advanced,
        TAB_WIDTH,
    );
    let mut pen = 0.0;
    let mut glyphs = Vec::new();
    for g in line
        .spans
        .iter()
        .flat_map(|s| &s.words)
        .flat_map(|w| &w.glyphs)
    {
        // HOW A BOLD IS DRAWN, by the face that answered: a bold face as it
        // is; a variable one at the bold weight, by its own axis (the
        // rasteriser sets `wght` from the weight in the key); any other
        // thickened here.
        let (weight, thickened) = if !bold || heavy(fonts, g.font_id) {
            (g.font_weight, false)
        } else if weighs(fonts, g.font_id, g.font_weight) {
            (Weight::BOLD, false)
        } else {
            (g.font_weight, true)
        };
        glyphs.push(Shape {
            font: g.font_id,
            glyph: g.glyph_id,
            weight,
            x: pen + g.x_offset,
            y: g.y_offset,
            italic: g.cache_key_flags.contains(CacheKeyFlags::FAKE_ITALIC),
            bold: thickened,
        });
        pen += g.x_advance;
    }
    Shaped {
        glyphs,
        advance: pen,
    }
}

/// Whether the primary family has a bold face loaded — its Bold or its
/// BoldItalic, which come with `RONOCTUA_TERMINAL_FACES` (`crate::text::STYLES`).
fn family_has_bold(fonts: &FontSystem) -> bool {
    fonts.db().faces().any(|f| {
        f.weight.0 >= Weight::SEMIBOLD.0
            && f.families
                .iter()
                .any(|(name, _)| name == tokens::font::FAMILY_MONO)
    })
}

/// Whether the face `id` is a bold one.
fn heavy(fonts: &FontSystem, id: fontdb::ID) -> bool {
    fonts
        .db()
        .face(id)
        .is_some_and(|f| f.weight.0 >= Weight::SEMIBOLD.0)
}

/// Whether the face `id` is VARIABLE in weight up to a bold — drawn bold by
/// the rasteriser when the key asks for it (cosmic-text's `swash_image`
/// reads `wght` off the cache key), so thickening it here as well would draw
/// it bold twice. Most of the fallback chain's scripts (Arabic, Devanagari,
/// the CJK) are such faces. `weight` is the one it was shaped at, whose font
/// the shaping already loaded.
fn weighs(fonts: &mut FontSystem, id: fontdb::ID, weight: Weight) -> bool {
    // `swash::Tag` is a `u32`, the four letters big-endian.
    let wght = u32::from_be_bytes(*b"wght");
    fonts.get_font(id, weight).is_some_and(|font| {
        font.as_swash()
            .variations()
            .find_by_tag(wght)
            .is_some_and(|axis| axis.max_value() >= f32::from(Weight::SEMIBOLD.0))
    })
}

/// The strength a missing bold is synthesised at: an em over 24, FreeType's
/// own `FT_GlyphSlot_Embolden` — a rasteriser's convention, not a choice of
/// look.
const EMBOLDEN_PER_EM: f32 = 1.0 / 24.0;

/// A face's glyph rasterised at `size` physical pixels — slanted by swash when
/// no italic face answered, thickened here when no bold one did.
fn raster_of(
    fonts: &mut FontSystem,
    swash: &mut SwashCache,
    s: &Shape,
    size: f32,
) -> Option<Raster> {
    let flags = if s.italic {
        CacheKeyFlags::FAKE_ITALIC
    } else {
        CacheKeyFlags::empty()
    };
    let (key, _, _) = CacheKey::new(s.font, s.glyph, size, (0.0, 0.0), s.weight, flags);
    // UNCACHED: the atlas is the cache, and swash's own would keep a second
    // copy of every raster with no bound at all.
    let image = swash.get_image_uncached(fonts, key)?;
    let (width, height) = (image.placement.width, image.placement.height);
    if width == 0 || height == 0 {
        return None;
    }
    let mut raster = Raster {
        kind: match image.content {
            SwashContent::Mask => Kind::Mask,
            SwashContent::Color => Kind::Colour,
            // NEVER ASKED FOR: swash renders `Format::Alpha` in cosmic-text.
            SwashContent::SubpixelMask => return None,
        },
        width,
        height,
        left: image.placement.left,
        top: image.placement.top,
        data: image.data,
    };
    if s.bold && raster.kind == Kind::Mask {
        let by = ((size * EMBOLDEN_PER_EM).round() as u32).max(1);
        embolden(&mut raster, by);
    }
    Some(raster)
}

/// A mask thickened `by` pixels to the right: each pixel the most of itself
/// and the `by` pixels left of it — the overstrike a terminal has always
/// drawn a missing bold with, done once in the atlas rather than per frame.
fn embolden(raster: &mut Raster, by: u32) {
    let (w, h) = (raster.width as usize, raster.height as usize);
    let grown = w + by as usize;
    let mut data = vec![0; grown * h];
    for y in 0..h {
        let row = &raster.data[y * w..(y + 1) * w];
        for x in 0..grown {
            let from = x.saturating_sub(by as usize);
            let to = x.min(w - 1);
            if from > to {
                continue;
            }
            data[y * grown + x] = row[from..=to].iter().copied().max().unwrap_or(0);
        }
    }
    raster.width = u32::try_from(grown).unwrap_or(u32::MAX);
    raster.data = data;
}

/// The uniform the shader reads: the surface's size and the grid's origin,
/// both physical — what lets the grid move without a row being rebuilt.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct View {
    size: [f32; 2],
    origin: [f32; 2],
}

/// One strip's instances on the GPU.
struct Buffer {
    buffer: wgpu::Buffer,
    capacity: usize,
    amount: u32,
}

impl Buffer {
    /// The first capacity, in instances: a row's worth of an ordinary window.
    const FIRST: usize = 256;

    fn new(device: &wgpu::Device, capacity: usize) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("cells-instances"),
                size: (capacity * std::mem::size_of::<Instance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
            amount: 0,
        }
    }

    /// `all` up to the GPU: only the `changed` ranges, unless the buffer had
    /// to grow — then all of it.
    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        all: &[Instance],
        changed: &mut Vec<Range<usize>>,
    ) {
        if all.len() > self.capacity {
            *self = Self::new(device, all.len().next_power_of_two());
            changed.clear();
            changed.push(0..all.len());
        }
        for range in changed.drain(..) {
            let end = range.end.min(all.len());
            if range.start >= end {
                continue;
            }
            let offset = (range.start * std::mem::size_of::<Instance>()) as u64;
            queue.write_buffer(
                &self.buffer,
                offset,
                bytemuck::cast_slice(&all[range.start..end]),
            );
        }
        self.amount = u32::try_from(all.len()).unwrap_or(u32::MAX);
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.amount == 0 {
            return;
        }
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        // Four vertices in a `TriangleStrip` = one rectangle, with no index.
        pass.draw(0..4, 0..self.amount);
    }
}

/// THE GPU SIDE: the pipeline, the two atlases' textures and a buffer per
/// layer — and the [`Assembly`] that feeds them.
pub struct Cells {
    assembly: Assembly,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    view: wgpu::Buffer,
    group: wgpu::BindGroup,
    masks: (wgpu::Texture, u32),
    colours: (wgpu::Texture, u32),
    paper: Buffer,
    ink: Buffer,
    lines: Buffer,
    over: Buffer,
}

impl Cells {
    /// Builds the pipeline.
    ///
    /// # Errors
    ///
    /// It never fails today; the signature follows [`crate::quad::Quads::new`].
    pub fn new(device: &wgpu::Device, format_of: wgpu::TextureFormat) -> anyhow::Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cells"),
            source: wgpu::ShaderSource::Wgsl(include_str!("cells.wgsl").into()),
        });
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            // READ BY `textureLoad`, texel for texel: a raster is drawn at
            // the size it was rasterised, so there is nothing to filter.
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cells"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cells"),
            // An `Option` in wgpu 30, like the pipeline's `layout`. See the
            // graphics-API trap in AGENTS.md.
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cells"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4,
                        1 => Float32x4,
                        2 => Float32x2,
                        3 => Uint32,
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: format_of,
                    // STRAIGHT ALPHA, as glyphon draws the interface's text:
                    // coverage is the alpha, and a translucent band lets the
                    // ink under it through.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let view = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cells-view"),
            size: std::mem::size_of::<View>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let most = ATLAS_SIDE_MOST.min(device.limits().max_texture_dimension_2d);
        let assembly = Assembly::with_sides(ATLAS_SIDE_FIRST, most);
        let (mask_side, colour_side) = (assembly.masks.side, assembly.colours.side);
        let masks = texture(device, Kind::Mask, mask_side);
        let colours = texture(device, Kind::Colour, colour_side);
        let group = bind(device, &layout, &view, &masks, &colours);
        Ok(Self {
            assembly,
            pipeline,
            layout,
            view,
            group,
            masks: (masks, mask_side),
            colours: (colours, colour_side),
            paper: Buffer::new(device, Buffer::FIRST),
            ink: Buffer::new(device, Buffer::FIRST),
            lines: Buffer::new(device, Buffer::FIRST),
            over: Buffer::new(device, Buffer::FIRST),
        })
    }

    /// The CPU half, for whoever wants to see what it built.
    #[must_use]
    pub const fn assembly(&self) -> &Assembly {
        &self.assembly
    }

    /// Assembles `sheet` and uploads what changed, for a surface of
    /// `viewport` physical pixels.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sheet: &Sheet,
        fonts: &mut FontSystem,
        viewport: (u32, u32),
    ) -> Assembled {
        let done = self.assembly.assemble(sheet, fonts);
        self.upload(device, queue, viewport);
        done
    }

    /// What the assembly left to go up: the atlases' new rasters — onto a new
    /// texture when one grew — the view, and the changed ranges of every
    /// layer.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, viewport: (u32, u32)) {
        let a = &mut self.assembly;
        let mut regroup = false;
        for (kind, held, atlas) in [
            (Kind::Mask, &mut self.masks, &mut a.masks),
            (Kind::Colour, &mut self.colours, &mut a.colours),
        ] {
            if held.1 != atlas.side {
                *held = (texture(device, kind, atlas.side), atlas.side);
                regroup = true;
            }
            let texel = match kind {
                Kind::Mask => 1,
                Kind::Colour => 4,
            };
            for up in atlas.uploads.drain(..) {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &held.0,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: up.x,
                            y: up.y,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &up.data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(up.width * texel),
                        rows_per_image: Some(up.height),
                    },
                    wgpu::Extent3d {
                        width: up.width,
                        height: up.height,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        if regroup {
            self.group = bind(
                device,
                &self.layout,
                &self.view,
                &self.masks.0,
                &self.colours.0,
            );
        }
        let view = View {
            size: [viewport.0.max(1) as f32, viewport.1.max(1) as f32],
            origin: [a.origin.0 as f32, a.origin.1 as f32],
        };
        queue.write_buffer(&self.view, 0, bytemuck::bytes_of(&view));
        self.paper
            .upload(device, queue, &a.paper.all, &mut a.paper.changed);
        self.ink
            .upload(device, queue, &a.ink.all, &mut a.ink.changed);
        self.lines
            .upload(device, queue, &a.lines.all, &mut a.lines.changed);
        // THE CURSOR AND THE BANDS, always whole: they are a handful.
        let mut whole = Vec::with_capacity(1);
        whole.push(0..a.over.len());
        self.over.upload(device, queue, &a.over, &mut whole);
    }

    /// Draws what was uploaded, in the passes' order: the backgrounds, the
    /// glyphs, the lines, then the cursor and the selection over them.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        self.paper.draw(pass);
        self.ink.draw(pass);
        self.lines.draw(pass);
        self.over.draw(pass);
    }
}

/// An atlas's texture, `side` texels a side.
fn texture(device: &wgpu::Device, kind: Kind, side: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("cells-atlas"),
        size: wgpu::Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: match kind {
            Kind::Mask => wgpu::TextureFormat::R8Unorm,
            // `Srgb`: an emoji's bytes are sRGB, and the load undoes the
            // curve — the same reasoning as `image.rs`'s pictures.
            Kind::Colour => wgpu::TextureFormat::Rgba8UnormSrgb,
        },
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// The bind group of the view and the two atlases.
fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::Buffer,
    masks: &wgpu::Texture,
    colours: &wgpu::Texture,
) -> wgpu::BindGroup {
    let masks = masks.create_view(&wgpu::TextureViewDescriptor::default());
    let colours = colours.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cells"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: view.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&masks),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&colours),
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The faces, as a door holds them — the embedded four.
    fn fonts() -> FontSystem {
        crate::Text::fonts_alone(crate::chain::DEFAULT_LOCALE).expect("the faces")
    }

    fn defaults() -> Defaults {
        Defaults {
            foreground: Rgba::opaque(200, 200, 200),
            background: Rgba::opaque(10, 10, 10),
            faint: 128,
        }
    }

    fn sheet(fonts: &mut FontSystem, columns: u16, lines: u16) -> Sheet {
        let g = Geometry::measure(fonts, tokens::font::SIZE_BASE, 1).expect("a geometry");
        Sheet::new(g, columns, lines, defaults())
    }

    fn write(sheet: &mut Sheet, line: u16, text: &str) {
        let fg = sheet.defaults().foreground;
        for (cell, c) in sheet.row_mut(line).iter_mut().zip(text.chars()) {
            *cell = Cell {
                grapheme: Grapheme::One(c),
                fg,
                ..Cell::default()
            };
        }
    }

    /// FAILS IF: the instance stops being 48 bytes — the vertex layout's
    /// stride and offsets are computed from it, and one field added without
    /// them would misplace every instance with no error at all.
    #[test]
    fn the_instance_is_48_bytes() {
        assert_eq!(std::mem::size_of::<Instance>(), 48);
        assert_eq!(std::mem::size_of::<View>(), 16);
    }

    /// FAILS IF: the cell is not measured from the face — its width is not
    /// the primary face's advance rounded, its baseline or its lines fall
    /// outside it, or a screen of twice the scale does not give a cell of
    /// twice the size to the pixel the rounding allows.
    #[test]
    fn a_cell_is_measured_from_the_face() {
        let mut fonts = fonts();
        let size = tokens::font::SIZE_BASE;
        let one = Geometry::measure(&mut fonts, size, 1).expect("scale 1");
        let advance = tokens::font::CELL_WIDTH_EM * size;
        assert!(
            (one.cell_width as f32 - advance).abs() <= 0.5,
            "the cell is {} px wide and the face advances {advance}",
            one.cell_width
        );
        assert!(one.cell_height > one.cell_width);
        assert!(one.baseline > 0 && one.baseline < one.cell_height);
        for s in [one.underline, one.strike] {
            assert!(s.thickness >= 1);
            assert!(s.top >= 0 && s.top + s.thickness as i32 <= one.cell_height as i32);
        }
        assert!(
            one.strike.top < one.underline.top,
            "the strike is under the underline"
        );
        let two = Geometry::measure(&mut fonts, size, 2).expect("scale 2");
        assert!(two.cell_width.abs_diff(one.cell_width * 2) <= 1);
        assert!(two.cell_height.abs_diff(one.cell_height * 2) <= 1);
        assert_eq!(Geometry::measure(&mut fonts, 0.0, 1), None);
        assert_eq!(Geometry::measure(&mut fonts, size, 0), None);
    }

    /// FAILS IF: a wide cell is not drawn across its two cells — a sprite
    /// spans exactly both, an emoji is centred inside them, shrunk when wider
    /// — or the spacer after it draws anything of its own.
    #[test]
    fn a_wide_cell_spans_two_cells_and_a_spacer_draws_nothing() {
        let mut fonts = fonts();
        let mut sheet = sheet(&mut fonts, 8, 2);
        let g = *sheet.geometry();
        let fg = sheet.defaults().foreground;
        let wide = |grapheme: Grapheme| Cell {
            grapheme,
            fg,
            width: Width::Wide,
            ..Cell::default()
        };
        // A SPACER THAT CARRIES A GLYPH, as a careless engine might hand one:
        // it must still draw nothing.
        let spacer = Cell {
            grapheme: Grapheme::One('█'),
            fg,
            width: Width::Spacer,
            ..Cell::default()
        };
        {
            let row = sheet.row_mut(0);
            row[1] = wide(Grapheme::One('█'));
            row[2] = spacer.clone();
        }
        {
            let row = sheet.row_mut(1);
            row[3] = wide(Grapheme::of("😀"));
            row[4] = spacer;
        }
        let mut assembly = Assembly::new();
        assembly.assemble(&sheet, &mut fonts);
        let ink = &assembly.ink.all;
        assert_eq!(
            ink.len(),
            2,
            "one glyph per wide cell, none per spacer: {ink:?}"
        );
        let (block, emoji) = (ink[0], ink[1]);
        let cw = g.cell_width as f32;
        assert_eq!(block.rect[0], cw, "the block starts on its cell");
        assert_eq!(block.rect[2], 2.0 * cw, "the block spans both cells");
        assert_eq!(block.rect[3], g.cell_height as f32);
        let (left, right) = (3.0 * cw, 5.0 * cw);
        assert!(
            emoji.rect[0] >= left - 1.0 && emoji.rect[0] + emoji.rect[2] <= right + 1.0,
            "the emoji {:?} leaves its two cells {left}..{right}",
            emoji.rect
        );
        let middle = emoji.rect[0] + emoji.rect[2] / 2.0;
        assert!(
            (middle - (left + right) / 2.0).abs() <= 1.5,
            "the emoji is not centred"
        );
        assert_eq!(emoji.kind, COLOUR, "the emoji lost its colours");
    }

    /// FAILS IF: an assembly rebuilds a row nobody wrote — a frame of a
    /// screen with one line changing would cost the whole screen — or misses
    /// one somebody did, or a change of geometry, of defaults or of sheet
    /// leaves a row as it was built.
    #[test]
    fn only_the_rows_written_are_rebuilt() {
        let mut fonts = fonts();
        let mut sheet = sheet(&mut fonts, 20, 6);
        for line in 0..6 {
            write(&mut sheet, line, "ronoctua 0123456789");
        }
        let mut assembly = Assembly::new();
        assert_eq!(
            assembly.assemble(&sheet, &mut fonts).rows,
            6,
            "the first is whole"
        );
        assert_eq!(
            assembly.assemble(&sheet, &mut fonts).rows,
            0,
            "nothing was written"
        );
        // AS AN UPLOAD WOULD: what changed so far went up.
        assembly.ink.changed.clear();
        // LETTERS THE ROW ALREADY HAD, none of them a blank: the row keeps its
        // length and every glyph is in the atlas.
        write(&mut sheet, 3, "otua");
        let done = assembly.assemble(&sheet, &mut fonts);
        assert_eq!(done.rows, 1, "one row was written");
        assert_eq!(done.rasters, 0, "every glyph was already in the atlas");
        // THE ROW WAS REWRITTEN IN PLACE, being as long as it was: one range,
        // that row's.
        let before: usize = assembly.ink.lens[..3].iter().sum();
        let after = before + assembly.ink.lens[3];
        assert_eq!(assembly.ink.changed, vec![before..after]);
        // A NEW CURSOR rebuilds no row: it is drawn over them.
        sheet.cursor = Some(Cursor {
            line: 2,
            column: 4,
            shape: CursorShape::Block,
            visible: true,
            colour: Rgba::opaque(255, 255, 255),
            text: Rgba::opaque(0, 0, 0),
        });
        assert_eq!(assembly.assemble(&sheet, &mut fonts).rows, 0);
        assert!(!assembly.over.is_empty(), "the cursor was not drawn");
        // A BAND rebuilds no row either — and one past the last line is not
        // drawn at all.
        let over = assembly.over.len();
        let band = |line| Band {
            line,
            columns: 0..4,
            colour: Rgba::new(0, 0, 255, 96),
        };
        sheet.selection = vec![band(1), band(6)];
        assert_eq!(assembly.assemble(&sheet, &mut fonts).rows, 0);
        assert_eq!(
            assembly.over.len(),
            over + 1,
            "the band past the grid was drawn"
        );
        sheet.selection.clear();
        let mut defaults = *sheet.defaults();
        defaults.faint = 100;
        sheet.set_defaults(defaults);
        assert_eq!(
            assembly.assemble(&sheet, &mut fonts).rows,
            6,
            "new defaults redraw all"
        );
        let other = sheet.clone();
        let mut other = Sheet::new(*other.geometry(), 20, 6, *other.defaults());
        write(&mut other, 0, "x");
        assert_eq!(
            assembly.assemble(&other, &mut fonts).rows,
            6,
            "another sheet is whole"
        );
    }

    /// FAILS IF: two clones of one sheet, each written once, are taken for
    /// each other — the same generation minted for two different writes, and
    /// the second sheet's row drawn as the first's — or the rows they still
    /// share are rebuilt.
    #[test]
    fn a_clone_written_apart_is_drawn_apart() {
        let mut fonts = fonts();
        let mut a = sheet(&mut fonts, 4, 2);
        write(&mut a, 0, "same");
        let mut b = a.clone();
        write(&mut a, 1, "aaaa");
        write(&mut b, 1, "bbbb");
        let mut assembly = Assembly::new();
        assembly.assemble(&a, &mut fonts);
        let done = assembly.assemble(&b, &mut fonts);
        assert_eq!(done.rows, 1, "the shared row was rebuilt, or b's was not");
        assert_eq!(done.rasters, 1, "b's row was drawn from a's glyphs");
    }

    /// FAILS IF: a blink rebuilds a row that does not blink, or leaves one
    /// that does drawing its ink in the dark half.
    #[test]
    fn a_blink_rebuilds_only_the_rows_that_blink() {
        let mut fonts = fonts();
        let mut sheet = sheet(&mut fonts, 10, 3);
        write(&mut sheet, 0, "steady");
        write(&mut sheet, 1, "blink");
        for cell in sheet.row_mut(1) {
            cell.style.blink = true;
        }
        let mut assembly = Assembly::new();
        assembly.assemble(&sheet, &mut fonts);
        let lit = assembly.ink.all.len();
        sheet.set_blink_off(true);
        assert_eq!(assembly.assemble(&sheet, &mut fonts).rows, 1);
        assert_eq!(
            assembly.ink.all.len(),
            lit - "blink".len(),
            "the blinking ink stayed"
        );
    }

    /// FAILS IF: the atlas key forgets the size or the scale — a raster drawn
    /// at one would be drawn at the other, a glyph of the wrong size placed in
    /// a cell of the right one.
    #[test]
    fn the_atlas_key_carries_size_and_scale() {
        let mut fonts = fonts();
        let mut assembly = Assembly::new();
        let mut sizes = Vec::new();
        for (size, scale) in [(18.0, 1), (18.0, 2), (24.0, 1)] {
            let g = Geometry::measure(&mut fonts, size, scale).expect("a geometry");
            let mut sheet = Sheet::new(g, 4, 1, defaults());
            write(&mut sheet, 0, "a");
            let done = assembly.assemble(&sheet, &mut fonts);
            assert_eq!(
                done.rasters, 1,
                "{size} px at {scale}: the raster was reused"
            );
            sizes.push(assembly.ink.all[0].rect[3]);
        }
        let keys: std::collections::HashSet<_> = assembly
            .glyphs
            .keys()
            .filter_map(|k| match k {
                Key::Face { size, scale, .. } => Some((*size, *scale)),
                _ => None,
            })
            .collect();
        assert_eq!(keys.len(), 3, "{keys:?}");
        assert!(sizes[1] > sizes[0] && sizes[2] > sizes[0], "{sizes:?}");
    }

    /// FAILS IF: a full atlas is not started over — a screen of new glyphs
    /// would stop drawing — or starting over leaves an instance pointing at a
    /// texel of the atlas that is gone.
    #[test]
    fn a_full_atlas_starts_over_and_redraws_from_the_new_one() {
        let mut fonts = fonts();
        let mut sheet = sheet(&mut fonts, 40, 4);
        let g = *sheet.geometry();
        // AN ATLAS THAT HOLDS A FEW CELLS AND NO MORE, so a screen overflows it.
        let side = g.cell_height * 2;
        let mut assembly = Assembly::with_sides(side, side);
        let alphabet: String = ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
        write(&mut sheet, 0, &alphabet[..40]);
        let done = assembly.assemble(&sheet, &mut fonts);
        assert!(done.starts_over >= 1, "the atlas never filled: {done:?}");
        // EVERY INSTANCE IS INSIDE THE ATLAS AS IT IS NOW.
        for i in &assembly.ink.all {
            assert!(i.texel[0] + i.rect[2] <= side as f32 && i.texel[1] + i.rect[3] <= side as f32);
        }
        // AND THE STARTS OVER ARE BOUNDED: the frame ended.
        assert!(done.starts_over <= usize::from(STARTS_OVER_MOST));
    }

    /// FAILS IF: a raster larger than the atlas can ever grow starts it over
    /// — every raster wiped to make room that never comes, and, the epoch
    /// moved, every row rebuilt on the next frame and the one after, for
    /// ever — instead of being left undrawn once.
    #[test]
    fn a_raster_no_atlas_can_hold_never_starts_one_over() {
        let mut fonts = fonts();
        let mut sheet = sheet(&mut fonts, 4, 1);
        write(&mut sheet, 0, "a─");
        // SMALLER THAN ONE CELL: no glyph and no sprite of it fits.
        let side = sheet.geometry().cell_width / 2;
        let mut assembly = Assembly::with_sides(side, side);
        let done = assembly.assemble(&sheet, &mut fonts);
        assert_eq!(done.starts_over, 0, "{done:?}");
        assert!(assembly.ink.all.is_empty());
        assert_eq!(
            assembly.assemble(&sheet, &mut fonts).rows,
            0,
            "an unwritten row was rebuilt"
        );
    }

    /// FAILS IF: a background is drawn for the sheet's own ground — the clear
    /// colour drawn again over itself, a rectangle per cell of every blank
    /// screen — or a run of one colour is cut into a rectangle per cell.
    #[test]
    fn backgrounds_are_runs_and_the_ground_is_not_drawn() {
        let mut fonts = fonts();
        let mut sheet = sheet(&mut fonts, 10, 1);
        let red = Rgba::opaque(200, 0, 0);
        let ground = sheet.defaults().background;
        {
            let row = sheet.row_mut(0);
            for cell in &mut row[2..6] {
                cell.bg = Some(red);
            }
            row[7].bg = Some(ground);
        }
        let mut assembly = Assembly::new();
        assembly.assemble(&sheet, &mut fonts);
        let paper = &assembly.paper.all;
        assert_eq!(paper.len(), 1, "{paper:?}");
        let cw = sheet.geometry().cell_width as f32;
        assert_eq!(paper[0].rect[0], 2.0 * cw);
        assert_eq!(paper[0].rect[2], 4.0 * cw);
    }

    /// FAILS IF: a style is not drawn — a line under, through or over a
    /// cell missing, a faint cell at full ink, or an invisible one drawn.
    #[test]
    fn a_style_is_drawn_as_it_says() {
        let mut fonts = fonts();
        let mut sheet = sheet(&mut fonts, 6, 1);
        let fg = sheet.defaults().foreground;
        let styled = |style: Style| Cell {
            grapheme: Grapheme::One('x'),
            fg,
            style,
            ..Cell::default()
        };
        {
            let row = sheet.row_mut(0);
            row[0] = styled(Style {
                underline: Underline::Curly,
                strike: true,
                overline: true,
                ..Style::default()
            });
            row[1] = styled(Style {
                faint: true,
                ..Style::default()
            });
            row[2] = styled(Style {
                invisible: true,
                underline: Underline::Single,
                ..Style::default()
            });
            row[3] = styled(Style {
                bold: true,
                italic: true,
                ..Style::default()
            });
        }
        let mut assembly = Assembly::new();
        assembly.assemble(&sheet, &mut fonts);
        assert_eq!(
            assembly.lines.all.len(),
            3,
            "under, through and over the first"
        );
        assert_eq!(assembly.ink.all.len(), 3, "the invisible cell drew a glyph");
        let faint = assembly.ink.all[1].colour[3];
        assert!(
            (faint - 128.0 / 255.0).abs() < 0.01,
            "the faint ink is at {faint}"
        );
        // THE SYNTHETIC STYLES, exactly when no styled face answers: with
        // only the embedded Regular (no `RONOCTUA_TERMINAL_FACES`), the
        // bold-italic `x` is slanted by the rasteriser and thickened here;
        // with the BoldItalic face loaded it is drawn from it as it is.
        let styled = fonts.db().faces().any(|f| {
            f.weight.0 >= Weight::SEMIBOLD.0
                && f.style == cosmic_text::Style::Italic
                && f.families
                    .iter()
                    .any(|(name, _)| name == tokens::font::FAMILY_MONO)
        });
        let synthetic = assembly.glyphs.keys().any(|k| {
            matches!(
                k,
                Key::Face {
                    italic: true,
                    bold: true,
                    ..
                }
            )
        });
        assert_eq!(synthetic, !styled, "styled faces loaded: {styled}");
    }

    /// FAILS IF: a strip rewrites more than moved — a row rebuilt at its own
    /// length is not written in place — or less: a longer row does not move
    /// every row after it.
    #[test]
    fn a_strip_uploads_only_what_moved() {
        let instance = |n: f32| Instance::solid(n as i32, 0, 1, 1, Rgba::default());
        let rows = [
            vec![instance(0.0)],
            vec![instance(1.0)],
            vec![instance(2.0)],
        ];
        let mut strip = Strip::default();
        strip.stitch(rows.iter().map(|r| (&r[..], true)));
        assert_eq!(strip.changed, vec![0..3]);
        strip.changed.clear();
        let same = [
            vec![instance(0.0)],
            vec![instance(9.0)],
            vec![instance(2.0)],
        ];
        strip.stitch(
            same.iter()
                .zip([false, true, false])
                .map(|(r, f)| (&r[..], f)),
        );
        assert_eq!(strip.changed, vec![1..2]);
        assert_eq!(strip.all[1], instance(9.0));
        strip.changed.clear();
        let longer = [
            vec![instance(0.0)],
            vec![instance(1.0), instance(1.5)],
            vec![instance(2.0)],
        ];
        strip.stitch(
            longer
                .iter()
                .zip([false, true, false])
                .map(|(r, f)| (&r[..], f)),
        );
        assert_eq!(strip.changed, vec![1..4]);
        assert_eq!(strip.all.len(), 4);
    }

    /// FAILS IF: a bold cell leaves the primary family where it has no bold
    /// face — cosmic-text answering a bold asked of a family with only its
    /// Regular from another family's variable face, a bold `x` drawn in Noto
    /// Sans Mono — or is not thickened there, or is thickened where the Bold
    /// face is loaded (`RONOCTUA_TERMINAL_FACES`). The same verdict with the
    /// variable and without it: each branch asserts its own answer.
    #[test]
    fn a_bold_cell_stays_in_the_primary_family() {
        let mut fonts = fonts();
        let loaded = family_has_bold(&fonts);
        let bold = shape(&mut fonts, "x", true, false).glyphs[0];
        let face = fonts.db().face(bold.font).expect("the face");
        assert!(
            face.families
                .iter()
                .any(|(name, _)| name == tokens::font::FAMILY_MONO),
            "a bold x is drawn in {:?}",
            face.families
        );
        assert_eq!(heavy(&fonts, bold.font), loaded);
        assert_eq!(bold.bold, !loaded, "the bold face is loaded: {loaded}");
    }

    /// FAILS IF: a face that draws a bold of its own is thickened again — a
    /// bold cell of CJK or Arabic, which the chain answers from VARIABLE
    /// faces the rasteriser sets to the weight in the key, drawn bold twice —
    /// or is not drawn at the bold weight at all, or the bold and the regular
    /// of one variable glyph are one atlas key.
    ///
    /// It needs the coverage faces (`RONOCTUA_COVERAGE_FONTS`); where they
    /// are, it must find a variable one, so it is never vacuous there.
    #[test]
    fn a_variable_face_is_drawn_bold_by_its_own_axis() {
        let mut fonts = fonts();
        let mut variable = 0;
        for text in ["日", "ب", "क"] {
            let bold = shape(&mut fonts, text, true, false);
            let plain = shape(&mut fonts, text, false, false);
            for (b, p) in bold.glyphs.iter().zip(&plain.glyphs) {
                if !weighs(&mut fonts, b.font, b.weight) || heavy(&fonts, b.font) {
                    continue;
                }
                variable += 1;
                assert!(!b.bold, "{text}: a variable face was thickened");
                assert_eq!(b.weight, Weight::BOLD, "{text}: drawn at {:?}", b.weight);
                assert_ne!(b.weight, p.weight, "{text}: the bold is the regular's key");
            }
        }
        if std::env::var_os("RONOCTUA_COVERAGE_FONTS").is_some() {
            assert!(
                variable > 0,
                "the coverage faces answered with no variable face"
            );
        }
    }

    /// FAILS IF: a synthetic bold loses the glyph's ink or does not widen it.
    #[test]
    fn a_synthetic_bold_thickens_the_ink() {
        let mut r = Raster {
            kind: Kind::Mask,
            width: 3,
            height: 1,
            left: 0,
            top: 0,
            data: vec![0, 255, 0],
        };
        embolden(&mut r, 1);
        assert_eq!(r.width, 4);
        assert_eq!(r.data, vec![0, 255, 255, 0]);
    }
}
