//! THE PAGE OF ONE ICON — its stage, its facts, its adjustments and every way
//! to copy it. The icons app opens it on `RET` (`crate::icons`), and
//! fast-access's detail pane draws the same page for an icon it found.
//!
//! # What a door hands in and what comes back
//!
//! A door hands in an [`Area`] — where the page is on its surface, in cells
//! and in pixels — and gets back a [`Drawing`]: the runs, the grounds, the
//! pictures and the bars' tracks, all in the surface's own coordinates. The
//! window paints it on its canvas and the footer on its region; neither
//! computes a place of its own. The pointer asks [`Page::hit`], which lays the
//! page out with the SAME [`Layout::of`] the drawing used — a control is taken
//! where it was drawn.
//!
//! # The stage, and why the icon grows from its centre
//!
//! A square on `surface.container` (a measured ground), `space.icons.
//! stageSharePct` of the page's width or its height, whichever is less. On it,
//! the icon's BOUNDS — the em box of a glyph, the 24 viewBox of a Lucide
//! figure — drawn in the rule a raised ground takes (`chrome::outline`), and
//! the icon placed at `centre − size / 2` ([`placed`]). A size that changes
//! under `←`/`→` changes the box around ONE point, so the icon grows in every
//! direction at once and never walks towards a corner — which is what the
//! owner asked to see, and what `the-icon-grows-from-its-centre` holds.
//!
//! The centre is a pixel the renderer can reach exactly: a glyph is a text
//! run placed at any pixel across and down (`ronoctua_render::Size::Placed`:
//! its left edge right of its cell, its box lowered from its line). So the
//! run's middle is the stage's centre at every size, for a glyph one cell
//! wide and for a character two cells wide alike.
//!
//! # A glyph is text, a Lucide figure is its SVG
//!
//! A glyph (Nerd Fonts, Unicode) is drawn by the font, as a run of the size
//! chosen. A Lucide figure is the SVG the copy puts on the clipboard,
//! rasterised by resvg at its NATURAL size (`Fit::Natural`): the same text as
//! [`ronoctua_iconset::lucide::svg`], byte for byte, so what is seen is what is
//! copied, stroke, colour and caps included. Its texture's key is the document
//! and the palette, so a theme changed since is a new picture.
//!
//! # The list beside it
//!
//! Facts, then the adjustments, then the copy formats, in one list that
//! scrolls with its own bar (the Lucide page has 29 rows). Where a row's
//! arrows adjust, a mark says so in the two cells before its value — the same
//! two cells on every row, so the values stay in one column; where they do
//! not, `←`/`→` show the icon before or after this one ([`Affords`] is asked
//! by the drawing and by the key alike).

use ronoctua_iconset::copy::{self, LucideFormat, NerdFormat, UnicodeFormat};
use ronoctua_iconset::lucide::{self, Adjust, Linecap};
use ronoctua_iconset::{Collection, Set};
use ronoctua_tokens as tokens;

/// Where a page, or the grid, is drawn on a door's surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    /// Its first cell, in the surface's columns.
    pub left: u16,
    /// Its first line, in the surface's lines.
    pub top: u16,
    /// How many cells across.
    pub columns: u16,
    /// How many lines down.
    pub lines: u16,
    /// The pixel its bar's column ENDS at — the surface's right edge.
    pub right_px: f32,
    /// The pixel it ends at, down — the surface's bottom, or the region's.
    pub bottom_px: f32,
}

impl Area {
    /// A cell's width, in logical pixels.
    #[must_use]
    pub fn cell() -> f32 {
        crate::chrome::cell_width()
    }

    /// A line's height, in logical pixels.
    #[must_use]
    pub fn line() -> f32 {
        tokens::metrics::line_height(tokens::text::size())
    }

    /// The pixel column `c` of the surface starts at.
    #[must_use]
    pub fn x_of(c: u16) -> f32 {
        f32::from(c) * Self::cell()
    }

    /// The pixel line `l` of the surface starts at.
    #[must_use]
    pub fn y_of(l: u16) -> f32 {
        f32::from(l) * Self::line()
    }

    /// The same area, from line `down` of it on.
    #[must_use]
    pub const fn below(self, down: u16) -> Self {
        Self {
            top: self.top + down,
            lines: self.lines.saturating_sub(down),
            ..self
        }
    }
}

/// A picture of the page: its key, where it goes and the document it is made
/// of — rasterised only when the door's GPU is missing that key.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    /// The texture's key: the document, its side and the palette.
    pub key: String,
    /// Where it goes, in the surface's logical pixels.
    pub area: ronoctua_render::image::Area,
    /// The SVG, its `currentColor` already the token's value.
    pub svg: String,
    /// The most pixels it may take, across and down.
    pub side: u32,
}

impl Picture {
    /// The picture's pixels: the document at its natural size, never larger
    /// than [`Picture::side`].
    #[must_use]
    pub fn rgba(&self) -> Option<ronoctua_render::image::Rgba> {
        let r = ronoctua_notify::icon::from_svg(
            self.svg.as_bytes(),
            self.side,
            ronoctua_notify::icon::Fit::Natural,
        )?;
        Some(ronoctua_render::image::Rgba {
            width: r.width,
            height: r.height,
            pixels: r.pixels,
        })
    }
}

/// What a door paints for the page or the grid, in its surface's coordinates.
#[derive(Debug, Clone, Default)]
pub struct Drawing {
    /// The runs of text.
    pub spans: Vec<ronoctua_render::Span>,
    /// The grounds and the bands, under everything else.
    pub under: Vec<ronoctua_render::quad::Quad>,
    /// The pictures.
    pub pictures: Vec<Picture>,
    /// Every bar's thumb, over the grounds.
    pub thumbs: Vec<ronoctua_render::quad::Quad>,
    /// Every bar's track, for the door's record (`crate::bars`).
    pub tracks: Vec<crate::bars::Track>,
}

/// Which icon a page is about: a record of the set, by collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Icon {
    /// The collection.
    pub collection: Collection,
    /// Its index in that collection's table of the [`Set`].
    pub index: usize,
}

/// What a person adjusted, kept across the icons of one view: a size chosen
/// for one glyph is the size the next one is looked at in, as lucide.dev
/// keeps its adjustments while one browses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// A glyph's and a character's size on the stage, in logical pixels.
    pub glyph_px: u16,
    /// A Lucide figure's adjustments — what its SVG is written with.
    pub lucide: Adjust,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            glyph_px: tokens::space::ICONS_GLYPHSTARTPX,
            lucide: Adjust::lucide(),
        }
    }
}

/// A setting of the page — a row whose arrows change it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Setting {
    /// The size on the stage (a Lucide figure's: its SVG's).
    Size,
    /// A Lucide figure's stroke width.
    Stroke,
    /// A Lucide figure's colour — typed, on the door's question line.
    Colour,
    /// A Lucide figure's line caps, a ring of three.
    Cap,
    /// A Lucide figure's absolute stroke width, a switch.
    Absolute,
}

/// A copy format, of whichever collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// A Nerd Fonts glyph's.
    Nerd(NerdFormat),
    /// A Lucide figure's.
    Lucide(LucideFormat),
    /// A Unicode character's.
    Unicode(UnicodeFormat),
}

impl Format {
    /// Its words' key: `icons-copy-<collection>-<id>`.
    #[must_use]
    pub fn key(self) -> String {
        let (collection, id) = match self {
            Self::Nerd(f) => (Collection::NerdFonts, f.id()),
            Self::Lucide(f) => (Collection::Lucide, f.id()),
            Self::Unicode(f) => (Collection::Unicode, f.id()),
        };
        format!("icons-copy-{}-{id}", collection.id())
    }

    /// Every format of a collection, in the page's order.
    #[must_use]
    pub fn all(collection: Collection) -> Vec<Self> {
        match collection {
            Collection::NerdFonts => NerdFormat::ALL.into_iter().map(Self::Nerd).collect(),
            Collection::Lucide => LucideFormat::ALL.into_iter().map(Self::Lucide).collect(),
            Collection::Unicode => UnicodeFormat::ALL.into_iter().map(Self::Unicode).collect(),
        }
    }
}

/// What one row of the page is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// The blank line before a section's title (`chrome::AIR`).
    Air,
    /// A section's title.
    Heading,
    /// A fact, read and not changed.
    Fact,
    /// A setting.
    Setting(Setting),
    /// A copy format.
    Copy(Format),
}

/// One row of the page: what it is, its label and its value, in the reader's
/// words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What it is.
    pub kind: Kind,
    /// The label (or the heading's words).
    pub label: String,
    /// The value — empty for a heading and a copy format.
    pub value: String,
}

impl Row {
    /// Whether the cursor may rest on it: the rows that act.
    #[must_use]
    pub const fn acts(&self) -> bool {
        matches!(self.kind, Kind::Setting(_) | Kind::Copy(_))
    }
}

/// What the side arrows do on a row — asked by the drawing, for the mark, and
/// by the key, before it acts: one answer, so a mark never promises a key
/// that does something else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Affords {
    /// They change this row's value (and a mark says so).
    Adjusts,
    /// They show the icon before or after this one.
    Neighbours,
}

/// The side arrows' meaning on `row`.
#[must_use]
pub const fn affordance(row: &Row) -> Affords {
    match row.kind {
        Kind::Setting(Setting::Size | Setting::Stroke | Setting::Cap | Setting::Absolute) => {
            Affords::Adjusts
        }
        _ => Affords::Neighbours,
    }
}

/// A key of the page, read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// The row above that acts.
    Up,
    /// The row below that acts.
    Down,
    /// `←`: the setting down, or the icon before.
    Less,
    /// `→`: the setting up, or the icon after.
    More,
    /// `RET`: copy in the format, or change the setting.
    Enter,
    /// `TAB`: every adjustment back as it was.
    Reset,
}

/// What a key or a click on the page asks of the view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// A frame, and no more.
    Nothing,
    /// Put this text on the clipboard.
    Copy(String),
    /// Ask for the Lucide colour, the line starting from this one.
    AskColour(String),
    /// Show the icon this many places after this one (before, when negative).
    Neighbour(i32),
}

/// The page of one icon: which, and where its cursor and its list are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// The icon.
    pub icon: Icon,
    /// The row of [`Page::rows`] the cursor is on — always one that acts.
    pub cursor: usize,
    /// The first row of the list shown.
    pub offset: usize,
}

/// Where the page's parts are, from its [`Area`]: the ONE layout the drawing
/// and the pointer both read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// The stage: `(x, y, side, side)`.
    pub stage: crate::chrome::Rect,
    /// Its centre — where every size of the icon is centred.
    pub centre: (f32, f32),
    /// The column the stage's centre is the middle of.
    pub centre_column: u16,
    /// The list's first cell.
    pub list_left: u16,
    /// How many cells of text the list has, its bar's excluded.
    pub list_room: u16,
    /// The list's first line and how many it shows.
    pub list_top: u16,
    /// How many lines it shows.
    pub list_lines: u16,
}

impl Layout {
    /// The page laid out in `area`.
    #[must_use]
    pub fn of(area: Area) -> Self {
        let (cell, line) = (Area::cell(), Area::line());
        let x0 = Area::x_of(area.left);
        let y0 = Area::y_of(area.top);
        let width = f32::from(area.columns) * cell;
        let height = (area.bottom_px - y0).max(0.0);
        // THE SIDE: a share of the width, the height at most, and never so
        // wide that the list is left under its floor — but never under a line,
        // or the stage is a slit.
        let share = width * f32::from(tokens::space::ICONS_STAGESHAREPCT) / 100.0;
        let list_floor =
            f32::from(tokens::space::ICONS_LISTCELLS + tokens::space::ICONS_STAGEGAP) * cell;
        let side = share
            .min(height)
            .min((width - list_floor).max(0.0))
            .max(line.min(height));
        // THE CENTRE ON THE MIDDLE OF A CELL, across — the one pixel a run's
        // box is centred on (see the module's head) — and the stage around it.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let centre_column = ((x0 + side / 2.0) / cell).floor().max(0.0) as u16;
        let centre_x = (f32::from(centre_column) + 0.5) * cell;
        let centre = (centre_x, y0 + side / 2.0);
        let stage = (centre.0 - side / 2.0, y0, side, side);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let stage_ends = ((stage.0 + side) / cell).ceil().max(0.0) as u16;
        let list_left = stage_ends + tokens::space::ICONS_STAGEGAP;
        let ends = area.left + area.columns;
        Self {
            stage,
            centre,
            centre_column,
            list_left,
            list_room: ends
                .saturating_sub(list_left)
                .saturating_sub(crate::chrome::BAR_CELLS),
            list_top: area.top,
            list_lines: area.lines,
        }
    }

    /// The cell a value starts at: the section's column, then the two cells
    /// the adjust mark takes on a row whose arrows adjust — on EVERY row, so
    /// the values keep one column.
    #[must_use]
    pub fn value_at(&self) -> u16 {
        crate::chrome::value_column(self.list_left, self.list_room) + 2
    }

    /// How many cells a value has, from [`Layout::value_at`] to the list's end.
    #[must_use]
    pub fn value_room(&self) -> u16 {
        (self.list_left + self.list_room).saturating_sub(self.value_at())
    }
}

/// One line of the list AS IT IS DRAWN: which row it belongs to, and which
/// piece of the row's value it shows.
///
/// A fact's value is WRAPPED onto the lines under it, never cut: the second
/// language's tags and a figure's categories are read nowhere else, and a
/// value elided with `…` is a fact the page has and does not say
/// (`chrome::note_lines`). So a row may be several lines, and the offset, the
/// bar and the pointer count LINES; the cursor stays a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The row of [`Page::rows`] it belongs to.
    pub row: usize,
    /// Whether it is the row's first line — the one its label is on.
    pub first: bool,
    /// The piece of the value on this line.
    pub value: String,
}

/// The lines `rows` take in `lay`: one per row, and as many as a fact's value
/// wraps into.
#[must_use]
pub fn shown(rows: &[Row], lay: &Layout) -> Vec<Shown> {
    let room = lay.value_room().max(1);
    let mut out = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        if row.kind == Kind::Fact {
            for (n, piece) in crate::chrome::note_lines(&row.value, room)
                .into_iter()
                .enumerate()
            {
                out.push(Shown {
                    row: i,
                    first: n == 0,
                    value: piece,
                });
            }
        } else {
            out.push(Shown {
                row: i,
                first: true,
                value: row.value.clone(),
            });
        }
    }
    out
}

/// THE ICON'S BOX AT THIS SIZE: `side` pixels square, centred on `centre`.
///
/// The one expression the stage places anything with — the bounds, a glyph,
/// a picture — so a size that changes moves the box's four edges by the same
/// amount, outwards, and its centre not at all.
#[must_use]
pub fn placed(centre: (f32, f32), side: f32) -> crate::chrome::Rect {
    placed_box(centre, side, side)
}

/// The same, `width` by `height` — a character two cells wide is wider than
/// the em box, and its bounds are the box its run takes.
#[must_use]
pub fn placed_box(centre: (f32, f32), width: f32, height: f32) -> crate::chrome::Rect {
    (
        centre.0 - width / 2.0,
        centre.1 - height / 2.0,
        width,
        height,
    )
}

/// How much wider than the em box a character of `cells` cells is drawn: one
/// for a glyph, `cells` times a cell's share of the em for a character wider
/// than one cell — a RATIO of the font's (`font::CELL_WIDTH_EM`), never a size.
#[must_use]
pub fn widening(cells: u8) -> f32 {
    (f32::from(cells) * tokens::font::CELL_WIDTH_EM).max(1.0)
}

/// What the pointer is on, on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A row of the list.
    Row(usize),
    /// The list's bar.
    Bar,
    /// The stage.
    Stage,
}

impl Page {
    /// The page of `icon`, its cursor on the first row that acts.
    #[must_use]
    pub fn of(icon: Icon, set: &Set, settings: &Settings, texts: &ronoctua_i18n::Texts) -> Self {
        let mut page = Self {
            icon,
            cursor: 0,
            offset: 0,
        };
        let rows = page.rows(set, settings, texts);
        page.cursor = rows.iter().position(Row::acts).unwrap_or(0);
        page
    }

    /// The same page, about another icon: the cursor stays on the same KIND of
    /// row where the new icon has one, so walking through icons with the
    /// cursor on "SVG" keeps copying SVGs.
    pub fn show(
        &mut self,
        icon: Icon,
        set: &Set,
        settings: &Settings,
        texts: &ronoctua_i18n::Texts,
    ) {
        let was = self
            .rows(set, settings, texts)
            .get(self.cursor)
            .map(|r| r.kind.clone());
        self.icon = icon;
        let rows = self.rows(set, settings, texts);
        self.cursor = was
            .and_then(|k| rows.iter().position(|r| r.kind == k))
            .or_else(|| rows.iter().position(Row::acts))
            .unwrap_or(0);
    }

    /// The rows, in the reader's words: the facts, the adjustments, the
    /// copies.
    #[must_use]
    pub fn rows(&self, set: &Set, settings: &Settings, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let mut out = Vec::new();
        let heading = |out: &mut Vec<Row>, key: &str| {
            if !out.is_empty() {
                out.push(Row {
                    kind: Kind::Air,
                    label: String::new(),
                    value: String::new(),
                });
            }
            out.push(Row {
                kind: Kind::Heading,
                label: texts.t(key),
                value: String::new(),
            });
        };
        let fact = |out: &mut Vec<Row>, key: &str, value: String| {
            if !value.is_empty() {
                out.push(Row {
                    kind: Kind::Fact,
                    label: texts.t(key),
                    value,
                });
            }
        };
        let setting = |out: &mut Vec<Row>, s: Setting, key: &str, value: String| {
            out.push(Row {
                kind: Kind::Setting(s),
                label: texts.t(key),
                value,
            });
        };
        let px = |n: u16| {
            texts.t_args(
                ronoctua_i18n::key::ICONS_SIZE_PX,
                &[("px", ronoctua_i18n::Arg::Number(i64::from(n)))],
            )
        };
        heading(&mut out, ronoctua_i18n::key::ICONS_HEADING_FACTS);
        match self.icon.collection {
            Collection::NerdFonts => {
                if let Some(g) = set.nerd.get(self.icon.index) {
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_NAME,
                        g.names.join(", "),
                    );
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_CODEPOINT,
                        hex(g.cp),
                    );
                    let sets: Vec<String> = g
                        .sets
                        .iter()
                        .map(|s| named(texts, "icons-set-", s))
                        .collect();
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_SET,
                        sets.join(", "),
                    );
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_TOKEN,
                        g.token.clone().unwrap_or_default(),
                    );
                    if g.in_tray {
                        fact(
                            &mut out,
                            ronoctua_i18n::key::ICONS_FACT_TRAY,
                            texts.t(ronoctua_i18n::key::ICONS_YES),
                        );
                    }
                    for (key, value) in tag_rows(&g.tags) {
                        fact(&mut out, &key, value);
                    }
                }
                heading(&mut out, ronoctua_i18n::key::ICONS_HEADING_ADJUST);
                setting(
                    &mut out,
                    Setting::Size,
                    ronoctua_i18n::key::ICONS_ADJUST_SIZE,
                    px(settings.glyph_px),
                );
            }
            Collection::Lucide => {
                if let Some(f) = set.lucide.get(self.icon.index) {
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_NAME,
                        f.name.clone(),
                    );
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_COMPONENT,
                        copy::lucide(f, &settings.lucide, LucideFormat::Component),
                    );
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_CODEPOINT,
                        hex(f.lcp),
                    );
                    let categories: Vec<String> = f
                        .categories
                        .iter()
                        .map(|c| named(texts, "icons-category-", c))
                        .collect();
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_CATEGORIES,
                        categories.join(", "),
                    );
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_ALIASES,
                        f.aliases.join(", "),
                    );
                    for (key, value) in tag_rows(&f.tags) {
                        fact(&mut out, &key, value);
                    }
                }
                let a = &settings.lucide;
                heading(&mut out, ronoctua_i18n::key::ICONS_HEADING_ADJUST);
                setting(
                    &mut out,
                    Setting::Size,
                    ronoctua_i18n::key::ICONS_ADJUST_SIZE,
                    px(a.size()),
                );
                setting(
                    &mut out,
                    Setting::Stroke,
                    ronoctua_i18n::key::ICONS_ADJUST_STROKE,
                    a.stroke(),
                );
                setting(
                    &mut out,
                    Setting::Colour,
                    ronoctua_i18n::key::ICONS_ADJUST_COLOUR,
                    a.colour(),
                );
                setting(
                    &mut out,
                    Setting::Cap,
                    ronoctua_i18n::key::ICONS_ADJUST_CAP,
                    texts.t(&format!("icons-cap-{}", a.linecap.id())),
                );
                setting(
                    &mut out,
                    Setting::Absolute,
                    ronoctua_i18n::key::ICONS_ADJUST_ABSOLUTE,
                    texts.t(if a.absolute {
                        ronoctua_i18n::key::ICONS_YES
                    } else {
                        ronoctua_i18n::key::ICONS_NO
                    }),
                );
            }
            Collection::Unicode => {
                if let Some(c) = set.unicode.get(self.icon.index) {
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_NAME,
                        c.name.clone(),
                    );
                    let points: Vec<String> = c.seq.iter().map(|cp| hex(*cp)).collect();
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_CODEPOINT,
                        points.join(" "),
                    );
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_FAMILY,
                        named(texts, "icons-family-", &c.family),
                    );
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_CATEGORY,
                        c.category.clone(),
                    );
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_BLOCK,
                        c.block.clone(),
                    );
                    if let Some(g) = &c.group {
                        fact(
                            &mut out,
                            ronoctua_i18n::key::ICONS_FACT_GROUP,
                            named(texts, "icons-group-", g),
                        );
                    }
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_SUBGROUP,
                        c.subgroup.clone().unwrap_or_default(),
                    );
                    if let Some(t) = &c.tone {
                        fact(
                            &mut out,
                            ronoctua_i18n::key::ICONS_FACT_TONE,
                            named(texts, "icons-tone-", t),
                        );
                    }
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_DECOMPOSITION,
                        c.decomposition.clone(),
                    );
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_ALIASES,
                        c.aliases.join(", "),
                    );
                    fact(
                        &mut out,
                        ronoctua_i18n::key::ICONS_FACT_WIDTH,
                        texts.t(match c.width {
                            0 => ronoctua_i18n::key::ICONS_WIDTH_ZERO,
                            1 => ronoctua_i18n::key::ICONS_WIDTH_ONE,
                            _ => ronoctua_i18n::key::ICONS_WIDTH_DOUBLE,
                        }),
                    );
                    for (key, value) in tag_rows(&c.tags) {
                        fact(&mut out, &key, value);
                    }
                }
                heading(&mut out, ronoctua_i18n::key::ICONS_HEADING_ADJUST);
                setting(
                    &mut out,
                    Setting::Size,
                    ronoctua_i18n::key::ICONS_ADJUST_SIZE,
                    px(settings.glyph_px),
                );
            }
        }
        heading(&mut out, ronoctua_i18n::key::ICONS_HEADING_COPY);
        for f in Format::all(self.icon.collection) {
            out.push(Row {
                kind: Kind::Copy(f),
                label: texts.t(&f.key()),
                value: String::new(),
            });
        }
        out
    }

    /// Whether `RET` on the cursor's row CHANGES a setting rather than copies
    /// — what the shortcut line calls it.
    #[must_use]
    pub fn enter_changes(
        &self,
        set: &Set,
        settings: &Settings,
        texts: &ronoctua_i18n::Texts,
    ) -> bool {
        self.rows(set, settings, texts)
            .get(self.cursor)
            .is_some_and(|r| matches!(r.kind, Kind::Setting(_)))
    }

    /// What `format` puts on the clipboard for this icon.
    #[must_use]
    pub fn copied(&self, set: &Set, settings: &Settings, format: Format) -> Option<String> {
        Some(match format {
            Format::Nerd(f) => copy::nerd(set.nerd.get(self.icon.index)?, f),
            Format::Lucide(f) => {
                copy::lucide(set.lucide.get(self.icon.index)?, &settings.lucide, f)
            }
            Format::Unicode(f) => copy::unicode(set.unicode.get(self.icon.index)?, f),
        })
    }

    /// One key on the page.
    pub fn key(
        &mut self,
        press: Press,
        set: &Set,
        settings: &mut Settings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let rows = self.rows(set, settings, texts);
        let Some(row) = rows.get(self.cursor).cloned() else {
            return Errand::Nothing;
        };
        match press {
            Press::Up | Press::Down => {
                let acting: Vec<usize> = (0..rows.len()).filter(|i| rows[*i].acts()).collect();
                let at = acting.iter().position(|i| *i == self.cursor).unwrap_or(0);
                let to = if press == Press::Up {
                    at.saturating_sub(1)
                } else {
                    (at + 1).min(acting.len().saturating_sub(1))
                };
                if let Some(i) = acting.get(to) {
                    self.cursor = *i;
                }
                Errand::Nothing
            }
            Press::Less | Press::More => {
                let up = press == Press::More;
                match (affordance(&row), &row.kind) {
                    (Affords::Adjusts, Kind::Setting(s)) => {
                        turn(*s, up, self.icon.collection, settings);
                        Errand::Nothing
                    }
                    _ => Errand::Neighbour(if up { 1 } else { -1 }),
                }
            }
            Press::Enter => match row.kind {
                Kind::Copy(f) => self
                    .copied(set, settings, f)
                    .map_or(Errand::Nothing, Errand::Copy),
                Kind::Setting(Setting::Colour) => Errand::AskColour(settings.lucide.colour.clone()),
                Kind::Setting(s @ (Setting::Cap | Setting::Absolute)) => {
                    turn(s, true, self.icon.collection, settings);
                    Errand::Nothing
                }
                // `RET` ON A NUMBER STEPS IT UP, and from its top back to its
                // bottom: the second click is `RET` (invariant 19), and a
                // press that changes nothing on the row the page opens on is
                // a key that seems broken.
                Kind::Setting(s @ (Setting::Size | Setting::Stroke)) => {
                    let was = settings.clone();
                    turn(s, true, self.icon.collection, settings);
                    if *settings == was {
                        to_the_bottom(s, self.icon.collection, settings);
                    }
                    Errand::Nothing
                }
                Kind::Air | Kind::Heading | Kind::Fact => Errand::Nothing,
            },
            Press::Reset => {
                *settings = Settings::default();
                Errand::Nothing
            }
        }
    }

    /// A click on row `n`: the cursor goes there, and a click on the row it is
    /// already on is `RET` (invariant 19). Answers the errand.
    pub fn clicked(
        &mut self,
        n: usize,
        set: &Set,
        settings: &mut Settings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let rows = self.rows(set, settings, texts);
        if !rows.get(n).is_some_and(Row::acts) {
            return Errand::Nothing;
        }
        if self.cursor == n {
            return self.key(Press::Enter, set, settings, texts);
        }
        self.cursor = n;
        Errand::Nothing
    }

    /// The page's list as it is laid out in `area`: the layout, the rows and
    /// the lines they take.
    #[must_use]
    pub fn list(
        &self,
        area: Area,
        set: &Set,
        settings: &Settings,
        texts: &ronoctua_i18n::Texts,
    ) -> (Layout, Vec<Row>, Vec<Shown>) {
        let lay = Layout::of(area);
        let rows = self.rows(set, settings, texts);
        let lines = shown(&rows, &lay);
        (lay, rows, lines)
    }

    /// How many lines the list takes in `area`.
    #[must_use]
    pub fn lines_of(
        &self,
        area: Area,
        set: &Set,
        settings: &Settings,
        texts: &ronoctua_i18n::Texts,
    ) -> usize {
        self.list(area, set, settings, texts).2.len()
    }

    /// Keeps the cursor's row on screen, in `area` — after a key, a click or
    /// the page's opening, never from a paint.
    pub fn follow(
        &mut self,
        area: Area,
        set: &Set,
        settings: &Settings,
        texts: &ronoctua_i18n::Texts,
    ) {
        let (lay, _, lines) = self.list(area, set, settings, texts);
        let room = usize::from(lay.list_lines).max(1);
        let first = lines.iter().position(|s| s.row == self.cursor).unwrap_or(0);
        let last = lines
            .iter()
            .rposition(|s| s.row == self.cursor)
            .unwrap_or(first);
        if first < self.offset {
            // A SECTION'S TITLE COMES WITH ITS FIRST ROW: the two lines above
            // it, when there are two and they fit with it.
            self.offset = first.saturating_sub(2.min(room.saturating_sub(1)));
        } else if last >= self.offset + room {
            self.offset = last + 1 - room;
        }
        self.offset = self.offset.min(lines.len().saturating_sub(room));
    }

    /// The list's scroll: `total` lines in `lines`.
    #[must_use]
    pub fn scroll(&self, total: usize, lines: u16) -> ronoctua_design::Scroll {
        crate::chrome::list_scroll(
            total,
            usize::from(lines),
            self.offset.min(total.saturating_sub(usize::from(lines))),
        )
    }

    /// The list's bar's track and thumb, when its `total` lines do not fit.
    #[must_use]
    pub fn bar(
        &self,
        area: Area,
        total: usize,
    ) -> Option<(crate::chrome::Rect, Option<crate::chrome::Rect>)> {
        let lay = Layout::of(area);
        let scroll = self.scroll(total, lay.list_lines);
        scroll.cut().then(|| {
            crate::chrome::band_bar(
                scroll,
                area.right_px,
                Area::y_of(lay.list_top),
                area.bottom_px,
                Area::cell(),
                Area::line(),
            )
        })
    }

    /// Scrolls the list of `total` lines to where a drag of its bar is at `y`.
    pub fn drag_to(&mut self, area: Area, total: usize, y: f32) {
        let lay = Layout::of(area);
        let scroll = self.scroll(total, lay.list_lines);
        let top = Area::y_of(lay.list_top);
        let span = (area.bottom_px - top).max(1.0);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let at = (((y - top) / span).clamp(0.0, 1.0) * scroll.total as f32) as usize;
        self.offset = at.min(total.saturating_sub(usize::from(lay.list_lines)));
    }

    /// The wheel turned `by` lines over a list of `total` lines, `lines` shown.
    pub fn wheeled(&mut self, by: i64, total: usize, lines: u16) {
        let last = total.saturating_sub(usize::from(lines));
        let now = i64::try_from(self.offset)
            .unwrap_or(0)
            .saturating_add(by)
            .max(0);
        self.offset = usize::try_from(now).unwrap_or(0).min(last);
    }

    /// What is under `(x, y)` of the surface, by the layout the drawing uses:
    /// a line of the list is its ROW.
    #[must_use]
    pub fn hit(&self, area: Area, lines: &[Shown], x: f32, y: f32) -> Option<Target> {
        if let Some((track, _)) = self.bar(area, lines.len())
            && crate::bars::Track::of(track).holds(f64::from(x), f64::from(y))
        {
            return Some(Target::Bar);
        }
        let lay = Layout::of(area);
        let (sx, sy, sw, sh) = lay.stage;
        if x >= sx && x < sx + sw && y >= sy && y < sy + sh {
            return Some(Target::Stage);
        }
        let list_x = Area::x_of(lay.list_left);
        if x < list_x || x >= area.right_px || y < Area::y_of(lay.list_top) {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = ((y - Area::y_of(lay.list_top)) / Area::line()) as usize;
        if line >= usize::from(lay.list_lines) {
            return None;
        }
        let offset = self
            .offset
            .min(lines.len().saturating_sub(usize::from(lay.list_lines)));
        lines.get(offset + line).map(|s| Target::Row(s.row))
    }

    /// The page, drawn in `area`: the stage, the icon on it, and the list —
    /// its row under the pointer lit in `lit`'s colour.
    pub fn draw(
        &self,
        area: Area,
        set: &Set,
        settings: &Settings,
        texts: &ronoctua_i18n::Texts,
        lit: &[(tokens::Color, usize)],
        out: &mut Drawing,
    ) {
        let (lay, rows, lines) = self.list(area, set, settings, texts);
        // A PAINT MOVES NOTHING: it draws the offset kept inside a list that
        // changed size, and the keys and the wheel are what keep it.
        let offset = self
            .offset
            .min(lines.len().saturating_sub(usize::from(lay.list_lines)));
        self.draw_stage(&lay, set, settings, out);
        let size = tokens::text::size();
        let (cell, line) = (Area::cell(), Area::line());
        let bar = self.bar(area, lines.len());
        let band_right = bar.map_or(area.right_px, |(t, _)| t.0);
        let value_mark = crate::chrome::value_column(lay.list_left, lay.list_room);
        let value_at = lay.value_at();
        let room_to = lay.list_left + lay.list_room;
        for (i, s) in lines
            .iter()
            .enumerate()
            .skip(offset)
            .take(usize::from(lay.list_lines))
        {
            let Some(row) = rows.get(s.row) else {
                continue;
            };
            let y_line = lay.list_top + u16::try_from(i - offset).unwrap_or(0);
            if let Some((colour, _)) = lit.iter().find(|(_, n)| *n == s.row)
                && row.acts()
            {
                out.under.push(ronoctua_render::quad::Quad {
                    x: Area::x_of(lay.list_left) - cell,
                    y: Area::y_of(y_line),
                    width: (band_right - Area::x_of(lay.list_left) + cell).max(0.0),
                    height: line,
                    color: *colour,
                });
            }
            let focused = s.row == self.cursor;
            let mut put = |column: u16, text: String, color: tokens::Color| {
                if !text.is_empty() {
                    out.spans.push(ronoctua_render::Span {
                        size: ronoctua_render::Size::Px(size),
                        text,
                        column,
                        line: y_line,
                        color,
                        edge: false,
                        foot: false,
                    });
                }
            };
            match &row.kind {
                Kind::Air => {}
                Kind::Heading => put(
                    lay.list_left,
                    ronoctua_render::layout::elide_end(&row.label, lay.list_room),
                    tokens::color::TEXT_HEADING,
                ),
                Kind::Copy(_) => {
                    let (label, _) = crate::chrome::Kind::Setting.tones(focused);
                    put(
                        lay.list_left,
                        ronoctua_render::layout::elide_end(&row.label, lay.list_room),
                        label,
                    );
                }
                Kind::Fact | Kind::Setting(_) => {
                    let kind = crate::chrome::Kind::of(row.acts());
                    let (label, value) = kind.tones(focused);
                    if s.first {
                        put(
                            lay.list_left,
                            ronoctua_render::layout::elide_end(
                                &row.label,
                                value_mark.saturating_sub(lay.list_left + 1),
                            ),
                            label,
                        );
                    }
                    // A FACT'S LINE IS A PIECE OF ITS WRAPPED VALUE, whole; a
                    // setting's value is a number or a word, and the room is
                    // what is left.
                    put(
                        value_at,
                        ronoctua_render::layout::elide_end(
                            &s.value,
                            room_to.saturating_sub(value_at),
                        ),
                        value,
                    );
                    // THE TWO CELLS BEFORE THE VALUE, ON EVERY ROW: the mark
                    // where the arrows adjust, nothing where they do not — so
                    // the values keep one column.
                    if s.first && affordance(row) == Affords::Adjusts {
                        out.spans.push(ronoctua_render::Span {
                            size: ronoctua_render::Size::OnTheCell(crate::surface::chrome_size_of(
                                tokens::icon::ADJUST,
                            )),
                            text: tokens::icon::ADJUST.to_string(),
                            column: value_mark,
                            line: y_line,
                            color: value,
                            edge: false,
                            foot: false,
                        });
                    }
                }
            }
        }
        if let Some((track, thumb)) = bar {
            out.tracks.push(crate::bars::Track::of(track));
            if let Some(t) = thumb {
                out.thumbs.push(crate::chrome::thumb(t));
            }
        }
    }

    /// The stage: its ground, the icon's bounds and the icon, all centred on
    /// ONE point.
    fn draw_stage(&self, lay: &Layout, set: &Set, settings: &Settings, out: &mut Drawing) {
        let (sx, sy, side, _) = lay.stage;
        out.under.push(ronoctua_render::quad::Quad {
            x: sx,
            y: sy,
            width: side,
            height: side,
            color: tokens::color::SURFACE_CONTAINER,
        });
        let drawn_px = match self.icon.collection {
            Collection::Lucide => f32::from(settings.lucide.size()),
            Collection::NerdFonts | Collection::Unicode => f32::from(settings.glyph_px),
        };
        // WHAT IS DRAWN, and how many cells wide at its size: a Lucide
        // figure is its 24 viewBox, square; a glyph one cell; a character
        // two cells wide (every emoji, the long arrows) is WIDER than the em
        // box and its ink TALLER (a colour emoji's bitmap is ~1.1 em), and
        // its bounds are the box its run takes — two cells across and a line
        // of its own size down. Drawn around the em box alone, the ink crossed
        // all four sides.
        let (text, cells) = match self.icon.collection {
            Collection::Lucide => (None, 1),
            Collection::NerdFonts => (
                set.nerd
                    .get(self.icon.index)
                    .and_then(|g| char::from_u32(g.cp))
                    .map(String::from),
                1,
            ),
            Collection::Unicode => set
                .unicode
                .get(self.icon.index)
                .map_or((None, 1), |c| (Some(c.drawn()), c.width.max(1))),
        };
        let wide = widening(cells);
        // A RATIO of the font's, never a size: a line's height per em.
        let tall = if cells > 1 {
            tokens::font::LINE_HEIGHT
        } else {
            1.0
        };
        // NEVER LARGER THAN THE STAGE: the number on the row still says the
        // size chosen, and the stage shows as much of it as it holds.
        let shown = drawn_px.min(side / wide).min(side / tall);
        let bounds = if cells > 1 {
            placed_box(lay.centre, shown * wide, shown * tall)
        } else {
            placed(lay.centre, shown)
        };
        out.under.extend(crate::chrome::outline(bounds));
        if self.icon.collection == Collection::Lucide {
            let Some(f) = set.lucide.get(self.icon.index) else {
                return;
            };
            let document = lucide::svg(f, &settings.lucide, false);
            let ink = resolved(&document, tokens::color::TEXT_PRIMARY);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let most = shown.round().max(1.0) as u32;
            out.pictures.push(Picture {
                key: format!("icons:{}:{most}:{document}", tokens::theme::palette()),
                area: ronoctua_render::image::Area {
                    x: bounds.0,
                    y: bounds.1,
                    width: bounds.2,
                    height: bounds.3,
                },
                svg: ink,
                side: most,
            });
        } else if let Some(text) = text {
            out.spans.push(centred_run(text, lay.centre, shown, cells));
        }
    }
}

/// A run of `text` at `px`, whose box is centred on `centre` — `cells` wide at
/// that size (two for an emoji).
///
/// The run's LEFT EDGE is the centre less half of its width at its size, put
/// at that pixel (`Size::Placed`: the cell it falls in, and the pixels right
/// of that cell's edge), and its box is lowered from the line it names to the
/// centre down — so the middle of a run two cells wide lands on the centre
/// exactly, as a one-cell glyph's does.
#[must_use]
pub fn centred_run(text: String, centre: (f32, f32), px: f32, cells: u8) -> ronoctua_render::Span {
    let (cell, line) = (Area::cell(), Area::line());
    let left = centre.0 - tokens::metrics::cell_width(px) * f32::from(cells.max(1)) / 2.0;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let column = (left / cell).floor().max(0.0) as u16;
    let across = left - f32::from(column) * cell;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let row = (centre.1 / line).floor().max(0.0) as u16;
    let by = centre.1 - f32::from(row) * line - line / 2.0;
    ronoctua_render::Span {
        size: ronoctua_render::Size::Placed(across, by, px),
        text,
        column,
        line: row,
        color: tokens::color::TEXT_PRIMARY,
        edge: false,
        foot: false,
    }
}

/// The SVG with its `currentColor` as `colour`'s value in the palette now —
/// the page's ink. A colour the person typed has no `currentColor` left, and
/// the drawing is theirs.
fn resolved(svg: &str, colour: tokens::Color) -> String {
    let [r, g, b, _] = colour.to_rgba8();
    svg.replace("currentColor", &format!("#{r:02X}{g:02X}{b:02X}"))
}

/// A number setting at its smallest.
fn to_the_bottom(setting: Setting, collection: Collection, settings: &mut Settings) {
    match setting {
        Setting::Size if collection == Collection::Lucide => {
            settings.lucide.size_px = lucide::SIZE_MIN_PX;
        }
        Setting::Size => settings.glyph_px = tokens::space::ICONS_GLYPHMINPX,
        Setting::Stroke => settings.lucide.stroke_tenths = lucide::STROKE_MIN_TENTHS,
        Setting::Colour | Setting::Cap | Setting::Absolute => {}
    }
}

/// One step of a setting, up or down.
fn turn(setting: Setting, up: bool, collection: Collection, settings: &mut Settings) {
    let step = |v: u16, by: u16, lo: u16, hi: u16| {
        if up {
            v.saturating_add(by).min(hi)
        } else {
            v.saturating_sub(by).max(lo)
        }
    };
    match setting {
        Setting::Size if collection == Collection::Lucide => {
            let a = &mut settings.lucide;
            a.size_px = step(
                a.size(),
                tokens::space::ICONS_LUCIDESTEPPX,
                lucide::SIZE_MIN_PX,
                lucide::SIZE_MAX_PX,
            );
        }
        Setting::Size => {
            settings.glyph_px = step(
                settings.glyph_px,
                tokens::space::ICONS_GLYPHSTEPPX,
                tokens::space::ICONS_GLYPHMINPX,
                tokens::space::ICONS_GLYPHMAXPX,
            );
        }
        Setting::Stroke => {
            let a = &mut settings.lucide;
            a.stroke_tenths = step(
                a.stroke_tenths,
                tokens::space::ICONS_STROKESTEPTENTHS,
                lucide::STROKE_MIN_TENTHS,
                lucide::STROKE_MAX_TENTHS,
            );
        }
        Setting::Cap => {
            let a = &mut settings.lucide;
            let all = Linecap::ALL;
            let at = all.iter().position(|c| *c == a.linecap).unwrap_or(0);
            let n = all.len();
            a.linecap = all[if up { (at + 1) % n } else { (at + n - 1) % n }];
        }
        Setting::Absolute => settings.lucide.absolute = !settings.lucide.absolute,
        Setting::Colour => {}
    }
}

/// `U+F02DC`.
#[must_use]
pub fn hex(cp: u32) -> String {
    format!("U+{cp:04X}")
}

/// The reader's language's subtag (`pt`, `en`) — what the tables key tags by.
#[must_use]
pub fn subtag_of(texts: &ronoctua_i18n::Texts) -> String {
    texts
        .language()
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_lowercase()
}

/// A table's word in the reader's words, when a key names it (`icons-set-md`
/// is "Material Design"); the word itself, when none does.
pub(crate) fn named(texts: &ronoctua_i18n::Texts, stem: &str, id: &str) -> String {
    let key = format!("{stem}{id}");
    if texts.has(&key) {
        texts.t(&key)
    } else {
        id.to_owned()
    }
}

/// One row of tags per declared language: its label's key
/// (`icons-sheet-tags-<subtag>`) and the tags — a person who reads one
/// language still knows the other's words, and the search finds both.
fn tag_rows(tags: &std::collections::BTreeMap<String, Vec<String>>) -> Vec<(String, String)> {
    ronoctua_i18n::LANGUAGES
        .iter()
        .map(|l| {
            let subtag = l
                .split(['-', '_'])
                .next()
                .unwrap_or_default()
                .to_lowercase();
            let value = tags.get(&subtag).map(|t| t.join(", ")).unwrap_or_default();
            (format!("icons-sheet-tags-{subtag}"), value)
        })
        .collect()
}

/// The tags in the reader's language, or every language's when it has none.
#[must_use]
pub fn tags_of(tags: &std::collections::BTreeMap<String, Vec<String>>, subtag: &str) -> String {
    tags.get(subtag)
        .filter(|t| !t.is_empty())
        .or_else(|| tags.values().find(|t| !t.is_empty()))
        .map(|t| t.join(", "))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the English words")
    }

    fn set() -> Set {
        Set::new(
            vec![ronoctua_iconset::Nerd {
                cp: 0xF02DC,
                name: Some("md-home".to_owned()),
                names: vec!["md-home".to_owned()],
                sets: vec!["md".to_owned()],
                ..Default::default()
            }],
            vec![ronoctua_iconset::Lucide {
                name: "house".to_owned(),
                cp: 0xF2000,
                lcp: 0xE0F5,
                nodes: String::new(),
                ..Default::default()
            }],
            vec![ronoctua_iconset::Char {
                cp: 0x2605,
                seq: vec![0x2605],
                shows: vec![0x2605],
                name: "BLACK STAR".to_owned(),
                width: 1,
                ..Default::default()
            }],
        )
    }

    fn area(columns: u16, lines: u16) -> Area {
        let (cell, line) = (Area::cell(), Area::line());
        Area {
            left: 1,
            top: 2,
            columns,
            lines,
            right_px: f32::from(columns + 2) * cell,
            bottom_px: f32::from(lines + 2) * line,
        }
    }

    /// FAILS IF: a size change moves the icon's centre — the box grows from
    /// ONE point, in every direction by the same amount.
    #[test]
    fn the_icon_grows_from_its_centre() {
        let centre = (123.5, 211.25);
        for (a, b) in [(24.0_f32, 96.0_f32), (8.0, 512.0), (100.0, 101.0)] {
            let (small, large) = (placed(centre, a), placed(centre, b));
            let middle = |r: crate::chrome::Rect| (r.0 + r.2 / 2.0, r.1 + r.3 / 2.0);
            assert_eq!(middle(small), centre);
            assert_eq!(middle(large), centre);
            let grew = (b - a) / 2.0;
            assert!((small.0 - large.0 - grew).abs() < 1e-3, "the left edge");
            assert!((small.1 - large.1 - grew).abs() < 1e-3, "the top edge");
        }
    }

    /// FAILS IF: the glyph's run is not centred on the stage's centre — the
    /// cell's middle across, and the lowering down — at any size.
    #[test]
    fn a_glyph_is_centred_at_every_size() {
        let lay = Layout::of(area(100, 30));
        let (cell, line) = (Area::cell(), Area::line());
        for px in [8.0_f32, 24.0, 96.0, 300.0] {
            let run = centred_run("x".to_owned(), lay.centre, px, 1);
            let ronoctua_render::Size::Placed(left, by, size) = run.size else {
                panic!("the run is not placed at a size of its own");
            };
            assert!((size - px).abs() < f32::EPSILON);
            let across =
                f32::from(run.column).mul_add(cell, left) + tokens::metrics::cell_width(px) / 2.0;
            let down = f32::from(run.line).mul_add(line, line / 2.0) + by;
            assert!(
                (across - lay.centre.0).abs() < 1e-3,
                "{px}: across {across}"
            );
            assert!((down - lay.centre.1).abs() < 1e-3, "{px}: down {down}");
        }
    }

    /// FAILS IF: the stage is not square, leaves its area, or starves the list
    /// of its floor.
    #[test]
    fn the_stage_is_a_square_that_leaves_the_list_its_room() {
        for (columns, lines) in [(60u16, 12u16), (120, 40), (40, 60), (200, 8)] {
            let a = area(columns, lines);
            let lay = Layout::of(a);
            let (x, y, w, h) = lay.stage;
            assert!((w - h).abs() < f32::EPSILON);
            assert!(y >= Area::y_of(a.top) && y + h <= a.bottom_px + 1e-3);
            assert!(
                x + Area::cell() / 2.0 >= Area::x_of(a.left) - 1e-3,
                "{columns}x{lines}"
            );
            assert!(lay.list_left > lay.centre_column);
        }
    }

    /// FAILS IF: the copied SVG is not the one the stage draws — the promise
    /// the preview makes.
    #[test]
    fn the_copy_is_the_svg_the_stage_draws() {
        let s = set();
        let t = texts();
        let mut settings = Settings::default();
        let icon = Icon {
            collection: Collection::Lucide,
            index: 0,
        };
        let page = Page::of(icon, &s, &settings, &t);
        for stroke in [20u16, 35] {
            settings.lucide.stroke_tenths = stroke;
            let mut out = Drawing::default();
            let p = page.clone();
            p.draw(area(100, 30), &s, &settings, &t, &[], &mut out);
            let copied = p
                .copied(&s, &settings, Format::Lucide(LucideFormat::Svg))
                .expect("a copy");
            let pic = out.pictures.first().expect("the stage draws the figure");
            // WHAT IS RASTERISED is the copy, its `currentColor` the ink —
            // the document itself, and not only the texture's key.
            assert_eq!(pic.svg, resolved(&copied, tokens::color::TEXT_PRIMARY));
            assert!(pic.key.ends_with(&copied), "the key is the document copied");
        }
    }

    /// FAILS IF: an arrow does something other than what the row's mark says.
    #[test]
    fn the_arrows_do_what_the_mark_says() {
        let s = set();
        let t = texts();
        let mut settings = Settings::default();
        let mut page = Page::of(
            Icon {
                collection: Collection::NerdFonts,
                index: 0,
            },
            &s,
            &settings,
            &t,
        );
        let rows = page.rows(&s, &settings, &t);
        assert_eq!(rows[page.cursor].kind, Kind::Setting(Setting::Size));
        let was = settings.glyph_px;
        assert_eq!(
            page.key(Press::More, &s, &mut settings, &t),
            Errand::Nothing
        );
        assert_eq!(settings.glyph_px, was + tokens::space::ICONS_GLYPHSTEPPX);
        // A COPY ROW HAS NO MARK, and its arrows walk the icons.
        page.key(Press::Down, &s, &mut settings, &t);
        let rows = page.rows(&s, &settings, &t);
        assert!(matches!(rows[page.cursor].kind, Kind::Copy(_)));
        assert_eq!(affordance(&rows[page.cursor]), Affords::Neighbours);
        assert_eq!(
            page.key(Press::Less, &s, &mut settings, &t),
            Errand::Neighbour(-1)
        );
        // `TAB` puts every adjustment back.
        page.key(Press::Reset, &s, &mut settings, &t);
        assert_eq!(settings, Settings::default());
    }

    /// FAILS IF: a copy of a Unicode character is not the character — the
    /// dotted circle a mark is drawn on must never reach the clipboard.
    #[test]
    fn a_combining_mark_copies_alone() {
        let s = Set::new(
            vec![],
            vec![],
            vec![ronoctua_iconset::Char {
                cp: 0x0301,
                seq: vec![0x0301],
                shows: vec![0x25CC, 0x0301],
                name: "COMBINING ACUTE ACCENT".to_owned(),
                width: 0,
                ..Default::default()
            }],
        );
        let page = Page::of(
            Icon {
                collection: Collection::Unicode,
                index: 0,
            },
            &s,
            &Settings::default(),
            &texts(),
        );
        let copied = page
            .copied(
                &s,
                &Settings::default(),
                Format::Unicode(UnicodeFormat::Character),
            )
            .expect("a copy");
        assert_eq!(copied, "\u{0301}");
    }

    /// FAILS IF: the pointer finds a row other than the one drawn there, or the
    /// bar is not taken on its own column.
    #[test]
    fn the_pointer_takes_the_row_drawn_under_it() {
        let s = set();
        let t = texts();
        let settings = Settings::default();
        let page = Page::of(
            Icon {
                collection: Collection::Lucide,
                index: 0,
            },
            &s,
            &settings,
            &t,
        );
        let a = area(100, 8);
        let (_, _, lines) = page.list(a, &s, &settings, &t);
        let mut out = Drawing::default();
        page.draw(a, &s, &settings, &t, &[], &mut out);
        let lay = Layout::of(a);
        let x = Area::x_of(lay.list_left + 1);
        let y = Area::y_of(lay.list_top + 3) + 1.0;
        assert_eq!(
            page.hit(a, &lines, x, y),
            Some(Target::Row(lines[page.offset + 3].row))
        );
        let track = out.tracks.first().expect("29 rows in 8 lines have a bar");
        assert_eq!(
            page.hit(a, &lines, track.x + 1.0, track.y + 1.0),
            Some(Target::Bar)
        );
    }

    /// FAILS IF: a fact's value is cut with `…` where the lines under it
    /// could say it whole — the second language's tags are read nowhere else.
    #[test]
    fn a_long_fact_is_wrapped_and_not_cut() {
        let mut tags = std::collections::BTreeMap::new();
        let words: Vec<String> = (0..30).map(|i| format!("word{i:02}")).collect();
        tags.insert("pt".to_owned(), words.clone());
        let s = Set::new(
            vec![ronoctua_iconset::Nerd {
                cp: 0xF02DC,
                name: Some("md-home".to_owned()),
                names: vec!["md-home".to_owned()],
                sets: vec!["md".to_owned()],
                tags,
                ..Default::default()
            }],
            vec![],
            vec![],
        );
        let t = texts();
        let settings = Settings::default();
        let page = Page::of(
            Icon {
                collection: Collection::NerdFonts,
                index: 0,
            },
            &s,
            &settings,
            &t,
        );
        let a = area(90, 60);
        let mut out = Drawing::default();
        page.draw(a, &s, &settings, &t, &[], &mut out);
        let drawn: String = out
            .spans
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!drawn.contains('…'), "a value was cut: {drawn}");
        for w in &words {
            assert!(drawn.contains(w.as_str()), "{w} is not on the page");
        }
        let (_, rows, lines) = page.list(a, &s, &settings, &t);
        assert!(lines.len() > rows.len(), "the tags took more than one line");
    }

    /// FAILS IF: a character two cells wide is drawn wider than the bounds
    /// drawn around it — the outline must hold the ink.
    #[test]
    fn a_wide_character_is_inside_its_bounds() {
        let s = Set::new(
            vec![],
            vec![],
            vec![ronoctua_iconset::Char {
                cp: 0x1F408,
                seq: vec![0x1F408],
                shows: vec![0x1F408],
                name: "CAT".to_owned(),
                width: 2,
                ..Default::default()
            }],
        );
        let t = texts();
        let settings = Settings::default();
        let page = Page::of(
            Icon {
                collection: Collection::Unicode,
                index: 0,
            },
            &s,
            &settings,
            &t,
        );
        let mut out = Drawing::default();
        page.draw(area(100, 30), &s, &settings, &t, &[], &mut out);
        let rules: Vec<_> = out
            .under
            .iter()
            .filter(|q| q.color == tokens::color::RULE_RAISED)
            .collect();
        assert_eq!(rules.len(), 4);
        let left = rules.iter().map(|q| q.x).fold(f32::MAX, f32::min);
        let right = rules.iter().map(|q| q.x + q.width).fold(f32::MIN, f32::max);
        let top = rules.iter().map(|q| q.y).fold(f32::MAX, f32::min);
        let bottom = rules
            .iter()
            .map(|q| q.y + q.height)
            .fold(f32::MIN, f32::max);
        let run = out
            .spans
            .iter()
            .find(|r| r.text == "\u{1F408}")
            .expect("the cat is drawn");
        let ronoctua_render::Size::Placed(left_px, _, px) = run.size else {
            panic!("the run has no size of its own");
        };
        let ink = 2.0 * tokens::metrics::cell_width(px);
        let cell = Area::cell();
        // THE RUN'S BOX: from its cell's edge and the pixels right of it.
        let ink_left = f32::from(run.column).mul_add(cell, left_px);
        // AND ITS MIDDLE IS THE BOUNDS' MIDDLE, exactly: a character two
        // cells wide is no longer half a glyph right of the centre.
        assert!(
            ((ink_left + ink / 2.0) - (left + right) / 2.0).abs() < 1e-2,
            "the run's middle {} is not the bounds' {}",
            ink_left + ink / 2.0,
            (left + right) / 2.0
        );
        assert!(
            right - left + cell >= ink,
            "the bounds are {} px for {ink} px of ink",
            right - left
        );
        assert!(
            ink_left >= left - cell && ink_left + ink <= right + cell,
            "the ink {ink_left}..{} leaves {left}..{right}",
            ink_left + ink
        );
        // AND DOWN: the run's box is a line of its own size, centred on its
        // line and lowered (`centred_run`) — a colour emoji's bitmap is taller
        // than one em, so bounds one em tall were crossed at the top and the
        // bottom.
        let ronoctua_render::Size::Placed(_, by, _) = run.size else {
            unreachable!("matched above");
        };
        let middle = f32::from(run.line).mul_add(Area::line(), Area::line() / 2.0) + by;
        let half = tokens::metrics::line_height(px) / 2.0;
        assert!(
            middle - half >= top - 0.01 && middle + half <= bottom + 0.01,
            "the run's box {}..{} leaves {top}..{bottom}",
            middle - half,
            middle + half
        );
    }

    /// FAILS IF: `RET` on the size — the row the page opens on — changes
    /// nothing, or does not come back from the top.
    #[test]
    fn enter_on_a_number_steps_it() {
        let s = set();
        let t = texts();
        let mut settings = Settings::default();
        let mut page = Page::of(
            Icon {
                collection: Collection::NerdFonts,
                index: 0,
            },
            &s,
            &settings,
            &t,
        );
        assert!(page.enter_changes(&s, &settings, &t));
        let was = settings.glyph_px;
        page.key(Press::Enter, &s, &mut settings, &t);
        assert_eq!(settings.glyph_px, was + tokens::space::ICONS_GLYPHSTEPPX);
        settings.glyph_px = tokens::space::ICONS_GLYPHMAXPX;
        page.key(Press::Enter, &s, &mut settings, &t);
        assert_eq!(settings.glyph_px, tokens::space::ICONS_GLYPHMINPX);
    }
}
