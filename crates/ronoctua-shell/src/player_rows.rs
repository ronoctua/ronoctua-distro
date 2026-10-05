//! The rows of the player's view — what a page is made of, and the ONE
//! builder every page of it writes with (`crate::player_page`).
//!
//! A row is placed and cut here, once, for every door: the footer's region, a
//! window and the column draw the same `Row`s and ask the same answers of
//! them — which bar is under a press ([`slider_at`]), what turning it to a
//! point asks the player ([`slid`]).

use crate::files::{Field, Hit as FilesHit, Ink};
use crate::player::{Errand, Target, VOLUME_MOST};
use ronoctua_player::wire::{Ask, State};
use ronoctua_tokens as tokens;

/// One drawn line.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Placed and cut.
    pub fields: Vec<Field>,
    /// The cursor is on it.
    pub focused: bool,
    /// The cursor skips it.
    pub heading: bool,
    /// What a key does here.
    pub target: Target,
    /// Its identity across a rebuild.
    pub spot: String,
    /// When the line is a row of a GRID — the files, laid out in columns —
    /// or carries a control beside its title: where each one is, and what it
    /// does. `target` is then the one the cursor is on.
    pub cells: Vec<Cell>,
    /// The bar the value is drawn as, when it is one — a control a press
    /// sets and a drag turns ([`slider_at`]).
    pub slider: Option<Slider>,
    /// A line that CONTINUES the row above it — the rest of a label too long
    /// for the room before its section's values (`chrome::aligned_pair`),
    /// never cut. The cursor never stops on it, and a click on it is a click
    /// on its row ([`Row::owner`]).
    pub continues: bool,
    /// The colour CHIPS on its line — a subtitle lane's colour on the black
    /// edge a subtitle wears over a film ([`Chip`]): each door paints the
    /// ground under the glyph with `chrome::chip`, one arithmetic for all.
    pub chips: Vec<Chip>,
    /// A LINE OF THE DECK — the ruler, or a lane — when the row is one
    /// ([`DeckRow`]): each door paints the deck's shapes under its words and
    /// takes a press on it where the deck says, as it does on the block of
    /// its own (`crate::player_timeline`).
    pub deck: Option<DeckRow>,
    /// A LINE OF THE LAYOUT'S SCHEMATIC ([`SchematicRow`]): each door paints
    /// the pictures' places over its lines, the one whose row the cursor is on
    /// lit (`crate::player_layout`). A figure, and no band lights behind it.
    pub schematic: Option<SchematicRow>,
    /// A LINE OF A COVER'S PICTURE ([`CoverRow`]) — of two, side by side,
    /// where a suggested cover stands beside the file's own
    /// ([`Builder::pictures`]): each door draws each picture the view holds
    /// over its lines (`crate::player_metadata`), cut where the rows are cut.
    pub covers: Vec<CoverRow>,
    /// A LINE OF THE GLOBE's picture on a directory's page ([`GlobeRow`]):
    /// each door draws the view's globe over its lines and takes the pointer
    /// there (`crate::player_globe`). A figure, and no band lights behind it.
    pub globe: Option<GlobeRow>,
    /// The cell a TEXT CARET stands before, on a line of a text being edited
    /// (`crate::text_area`): each door draws it as a thin bar in the
    /// cursor's colour (`chrome::caret`) — the cursor is a colour,
    /// and a text's is a place between two letters.
    pub caret: Option<u16>,
    /// How the pointer resting on the line LIGHTS it ([`Lit`]): the row
    /// whole, the words beside a figure at its head, or only its controls —
    /// what both doors paint, through [`lighting`].
    pub lit: Lit,
}

/// How the pointer resting on a line lights it — the answer both doors paint
/// with ([`lighting`]), the cells a click takes being the ones lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lit {
    /// The row whole — or, on a grid's row, the one cell under the pointer
    /// ([`grid_cells`]).
    #[default]
    Row,
    /// From this column to the row's end, wherever on the row the pointer
    /// rests: a FIGURE AT THE ROW'S HEAD — a strip's picture, or the box
    /// that waits for it (`player_posters::strip`) — is never lit, and the
    /// words beside it are, on every line of the row, the same with its
    /// picture or without.
    From(u16),
    /// Only the control under the pointer: a line of a grid's tiles whose
    /// cells are all controls — nothing else on it is taken by a click, so
    /// nothing else is lit (`player_posters::tiled`, the `⋯`).
    Controls,
}

/// What the pointer resting on line `n` lights ([`Row::lit`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lighting {
    /// The row whole.
    Whole,
    /// The one of these cells under the pointer — `(first column, columns)`
    /// — and nothing where none is.
    Cell(Vec<(u16, u16)>),
    /// From this column to the row's end, wherever the pointer rests on it.
    From(u16),
}

/// What line `n` of `rows` lights under the pointer — the ONE answer the
/// window's bands and the footer's read.
#[must_use]
pub fn lighting(rows: &[Row], n: usize) -> Lighting {
    match rows.get(n).map(|r| r.lit) {
        Some(Lit::From(column)) => Lighting::From(column),
        Some(Lit::Controls) => Lighting::Cell(
            rows.get(n)
                .map(|r| {
                    r.cells
                        .iter()
                        .filter(|c| c.control)
                        .map(|c| (c.from, c.width))
                        .collect()
                })
                .unwrap_or_default(),
        ),
        Some(Lit::Row) | None => {
            let cells = grid_cells(rows, n);
            if cells.is_empty() {
                Lighting::Whole
            } else {
                Lighting::Cell(cells)
            }
        }
    }
}

/// A row that is a line of the GLOBE: which line of how many, where its box
/// starts among the rows' cells and how wide it is — placed ONCE, when the
/// rows were built, so every door draws it and takes the pointer in the same
/// box (`player_globe::box_of`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobeRow {
    /// Its line of the picture: `0` is the top.
    pub line: u16,
    /// How many lines the picture takes.
    pub lines: u16,
    /// The box's first cell, as a field's `at`.
    pub left: u16,
    /// How many cells wide the box is.
    pub cells: u16,
}

/// A picture drawn side by side with others ([`Builder::pictures`]): the
/// key the view holds it under and its size in pixels — `None` while it is
/// not here — and what the line under it says it is.
pub(crate) type Pictured = (Option<(String, (u32, u32))>, String);

/// One box of the pictures' lines: its first cell, its cells, and the
/// picture drawn in it, by its key and size.
type PictureBox<'k> = (u16, u16, Option<(&'k str, (u32, u32))>);

/// A picture's key, shared by its lines, and its size.
type Keyed = (std::rc::Rc<str>, (u32, u32));

/// A row that is a line of a COVER's picture: which line of how many, where
/// the picture's box starts among the rows' cells and how wide it is, the
/// texture's key and the picture's own size — placed ONCE, when the rows
/// were built, so every door draws it in the same box
/// (`player_metadata::cover_place`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverRow {
    /// Its line of the picture: `0` is the top.
    pub line: u16,
    /// How many lines the picture's box takes.
    pub lines: u16,
    /// The box's first cell, as a field's `at`.
    pub left: u16,
    /// How many cells wide the box is.
    pub cells: u16,
    /// The key the view holds the picture under.
    pub key: std::rc::Rc<str>,
    /// The picture's own size, in pixels: its proportion.
    pub size: (u32, u32),
}

/// A row that is a line of the Layout page's SCHEMATIC: which line, where the
/// schematic starts among the rows' cells, and the schematic itself — placed
/// ONCE, when the rows were built (`player_layout::Schematic`), so every door
/// paints the same places.
#[derive(Debug, Clone, PartialEq)]
pub struct SchematicRow {
    /// Its line of the schematic: `0` is the top.
    pub line: u16,
    /// The schematic's first cell, as a field's `at`.
    pub left: u16,
    /// The schematic, shared by its lines.
    pub figure: std::rc::Rc<crate::player_layout::Schematic>,
}

/// A row that is a line of a DECK: which line, where the deck starts among
/// the rows' cells, and the deck itself — laid out ONCE, when the rows were
/// built, so the painter and the pointer read one geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct DeckRow {
    /// Its line of the deck: `0` is the ruler.
    pub line: usize,
    /// The deck's first cell, as a field's `at`.
    pub left: u16,
    /// The deck, shared by its rows.
    pub deck: std::rc::Rc<crate::player_timeline::Deck>,
}

impl Row {
    /// Whether the cursor stops on it: a row of its own, not a title and not
    /// the rest of the row above.
    #[must_use]
    pub const fn stops(&self) -> bool {
        !self.heading && !self.continues
    }

    /// How many lines the row that begins at line `owner` of `rows` takes:
    /// itself and every line that continues it.
    #[must_use]
    pub fn lines_of(rows: &[Self], owner: usize) -> usize {
        1 + rows
            .iter()
            .skip(owner + 1)
            .take_while(|r| r.continues)
            .count()
    }

    /// The row line `n` of `rows` belongs to — itself, or for the rest of a
    /// wrapped label the row it continues. A line PAST THE LAST ROW is no
    /// row's, and stays where it is: a click in the empty room under a list
    /// finds nothing there (`rows.get` is `None`), and never the last row.
    #[must_use]
    pub fn owner(rows: &[Self], n: usize) -> usize {
        if n >= rows.len() {
            return n;
        }
        let mut n = n;
        while n > 0 && rows.get(n).is_some_and(|r| r.continues) {
            n -= 1;
        }
        n
    }
}

/// Every line of the row line `n` belongs to — the lines a hover lights, as
/// a click takes them all ([`Row::owner`], [`Row::lines_of`]); none for none.
pub fn whole_row(rows: &[Row], n: Option<usize>) -> std::ops::Range<usize> {
    let Some(n) = n.filter(|n| *n < rows.len()) else {
        return 0..0;
    };
    let owner = Row::owner(rows, n);
    owner..owner + Row::lines_of(rows, owner)
}

/// A subtitle lane's colour CHIP at the head of a row: its glyph, drawn in
/// the lane's colour, on a ground of `subtitle.edge` — the black rim every
/// subtitle wears over a film, the one ground the lane colours are measured
/// on (`gen-theme.py`'s subtitle gates). On a page's own ground a lane colour
/// is measured against nothing, and on a light theme the yellow one is gone.
///
/// ONE GEOMETRY for the ground and the control: the chip takes the row's
/// first `space.player.chipCells` cells — the control a click turns
/// (`Builder::pair_with_chip`) — its glyph is in the middle one, and the
/// ground is `chrome::chip`'s, half a cell past the glyph on each side, so
/// every pixel of it is inside what the pointer takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chip {
    /// Its first cell, as a field's `at`.
    pub at: u16,
}

/// Which value a bar of the view turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slid {
    /// Where in the item: a press seeks there.
    Position,
    /// The volume, up to [`VOLUME_MOST`].
    Volume,
    /// An ambient sound's own volume, up to [`VOLUME_MOST`] as the player
    /// clamps it — the sound by its place in `State::ambience`, which is the
    /// order the machine declares them in and never moves.
    Ambience(usize),
    /// A lane's own volume, up to `own::VOLUME_MOST_PERCENT` — the lane by
    /// its id, which survives the others coming and going.
    Lane(ronoctua_player::own::LaneId),
}

/// A bar on a row: where it is drawn, in the row's own cells (a field's
/// `at`), and which value it turns.
///
/// ONE ANSWER for the painter and the pointer: the bar is the value's first
/// cells, placed by `chrome::field_pair`, and this is read back from the field
/// that was placed — so a press is taken where the bar was drawn, in both
/// doors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slider {
    /// The value it turns.
    pub of: Slid,
    /// Its first cell.
    pub from: u16,
    /// How many cells it takes.
    pub cells: u16,
}

impl Slider {
    /// Whether `column` — fractional cells from the rows' origin — is on it.
    #[must_use]
    pub fn under(self, column: f64) -> bool {
        column >= f64::from(self.from) && column < f64::from(self.from + self.cells)
    }

    /// How far along it `column` is, clamped to its two ends: a drag that
    /// leaves the bar keeps the end it left by.
    #[must_use]
    pub fn along(self, column: f64) -> f64 {
        ((column - f64::from(self.from)) / f64::from(self.cells.max(1))).clamp(0.0, 1.0)
    }
}

/// The bar on row `n` under `column` (fractional cells from the rows'
/// origin), when there is one there — what a press in either door asks.
#[must_use]
pub fn slider_at(rows: &[Row], n: usize, column: f64) -> Option<Slider> {
    rows.get(n)?.slider.filter(|b| b.under(column))
}

/// The line of a DECK on row `n` under `column` (fractional cells from the
/// rows' origin), when the row is one: the row's deck and the column from the
/// deck's own first cell — what a press, a drag and a hover on a deck's row
/// ask of it, in every door ([`slider_at`]'s twin).
#[must_use]
pub fn deck_at(rows: &[Row], n: usize, column: f64) -> Option<(&DeckRow, f64)> {
    let d = rows.get(n)?.deck.as_ref()?;
    Some((d, column - f64::from(d.left)))
}

/// What turning a bar to `fraction` of its length asks the player — the same
/// arithmetic as the window's time bar and volume track.
#[must_use]
pub fn slid(of: Slid, fraction: f64, state: Option<&State>) -> Errand {
    let fraction = fraction.clamp(0.0, 1.0);
    match of {
        Slid::Position => {
            state
                .and_then(|s| s.duration)
                .filter(|d| *d > 0.0)
                .map_or(Errand::Nothing, |d| {
                    Errand::Tell(Ask::Seek {
                        seconds: d * fraction,
                        relative: false,
                    })
                })
        }
        Slid::Volume => Errand::Tell(Ask::Volume {
            percent: percent_of(fraction),
        }),
        Slid::Lane(lane) => Errand::Tell(Ask::Lane {
            lane,
            change: ronoctua_player::wire::LaneChange::Volume {
                percent: percent_of(fraction),
            },
        }),
        Slid::Ambience(n) => {
            state
                .and_then(|s| s.ambience.sounds.get(n))
                .map_or(Errand::Nothing, |sound| {
                    Errand::Tell(Ask::AmbientVolume {
                        id: sound.id.clone(),
                        percent: percent_of(fraction),
                    })
                })
        }
    }
}

/// `fraction` of a volume bar, in per cent of [`VOLUME_MOST`].
fn percent_of(fraction: f64) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let percent = (fraction.clamp(0.0, 1.0) * f64::from(VOLUME_MOST)).round() as u8;
    percent
}

/// One entry of a row laid out in columns, or a CONTROL on a row's line — a
/// glyph at the head or the end of it, a title's.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// Its first column, as a field's `at`.
    pub from: u16,
    /// How many columns it takes.
    pub width: u16,
    /// What a key or a click does on it.
    pub target: Target,
    /// Its identity across a rebuild.
    pub spot: String,
    /// A control: a click on it ACTS at once — a group's triangle, a title's
    /// glyph, the glyph that plays a medium too — instead of choosing it as
    /// the entry of a grid the next `RET` acts on.
    pub control: bool,
    /// What it does, as the COMMAND ID whose sentence the pointer resting on
    /// it says (`chrome::hint_sentence`: `cmd-<id>`, else `about-<id>`) —
    /// every title's control has one ([`Builder::heading_controls`]), read
    /// by both doors through [`control_under`].
    pub about: Option<&'static str>,
}

/// Every control a click on line `n` takes that says what it does: its
/// cell, its width and its sentence's id. What a door that PLACES the row
/// writes down for its pointer (the footer's `player_controls_placed`), the
/// cells [`control_under`] reads.
///
/// A CONTROL IS TAKEN ON THE LINE IT IS DRAWN ON — its row's first — and
/// nowhere else of a row of several lines: a strip's `⋯`, on the first line
/// of a tile six lines tall, was taken on all six (`View::clicked_at` reads
/// the same).
#[must_use]
pub fn controls_under(rows: &[Row], n: usize) -> Vec<(u16, u16, &'static str)> {
    rows.get(n).map_or_else(Vec::new, |row| {
        row.cells
            .iter()
            .filter(|c| c.control)
            .filter_map(|c| Some((c.from, c.width, c.about?)))
            .collect()
    })
}

/// The sentence id of the control at `column` of line `n` — what the
/// pointer resting there says, in every door. The same cells a click takes
/// (`View::clicked_at`): the line's own, the line a control is drawn on
/// ([`controls_under`]), and a cell by `from` and `width`.
#[must_use]
pub fn control_under(rows: &[Row], n: usize, column: u16) -> Option<&'static str> {
    let row = rows.get(n)?;
    row.cells
        .iter()
        .find(|c| c.control && crate::chrome::in_cell(column, c.from, c.width))
        .and_then(|c| c.about)
}

/// The cells of row `n` that are entries of a GRID — `(first column,
/// columns)` each — and none for a row that is one thing, or whose cells are
/// only its controls: what the hover lights one of (`chrome::hover_cells`),
/// the same cells a click chooses among (`View::clicked_at`).
#[must_use]
pub fn grid_cells(rows: &[Row], n: usize) -> Vec<(u16, u16)> {
    rows.get(n).map_or_else(Vec::new, |r| {
        r.cells
            .iter()
            .filter(|c| !c.control)
            .map(|c| (c.from, c.width))
            .collect()
    })
}

/// The GRID cell the cursor is on — its row, and its `(first column,
/// columns)` — when the cursor is on a grid: what wears the selection
/// (`chrome::cell_selection`).
#[must_use]
pub fn grid_cursor(rows: &[Row]) -> Option<(usize, (u16, u16))> {
    rows.iter().enumerate().find_map(|(n, r)| {
        r.cells
            .iter()
            .filter(|c| !c.control)
            .find(|c| r.focused && c.target == r.target)
            .map(|c| (n, (c.from, c.width)))
    })
}

/// A row of the FILE PICKER (`crate::file_picker`), as a row of this view:
/// its fields as they were placed, and its cells as the cells of a grid —
/// each one a click chooses and the pointer lights alone
/// (`chrome::hover_cells`).
pub(crate) fn of_the_picker(r: crate::file_picker::Row) -> Row {
    let of = |t: crate::file_picker::Target| match t {
        crate::file_picker::Target::Nothing => Target::Nothing,
        t => Target::Picker(t),
    };
    let cells = r
        .cells
        .into_iter()
        .map(|c| Cell {
            from: c.from,
            width: c.width,
            spot: c.target.spot(),
            target: of(c.target),
            control: false,
            about: None,
        })
        .collect();
    Row {
        fields: r.fields,
        focused: r.focused,
        heading: r.heading,
        target: of(r.target),
        spot: r.spot,
        cells,
        slider: None,
        continues: r.continues,
        chips: Vec::new(),
        deck: None,
        schematic: None,
        covers: Vec::new(),
        globe: None,
        caret: r.caret,
        lit: Lit::Row,
    }
}

/// A row of the queue's tree (`crate::player_queue`), before it is placed.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    /// How deep in the tree: each level is set in by `view.nestCells`.
    pub depth: u16,
    /// Its number among its siblings, `3.`, and the cells the widest number
    /// of them takes.
    pub number: (String, u16),
    /// A group's triangle — open or folded, and what a click on it does;
    /// `None` for an entry that holds nothing.
    pub fold: Option<(bool, Target)>,
    /// A glyph in the triangle's place that is no control — what kind a
    /// set's lane is.
    pub mark: Option<char>,
    /// Whether the row keeps the triangle's room: its siblings have one, and
    /// the names start in one column.
    pub glyph_room: bool,
    /// What it is called, whole: cut to the row here.
    pub name: String,
    /// What plays.
    pub current: bool,
    /// What a key does on it.
    pub target: Target,
    /// Its identity.
    pub spot: String,
    /// The glyphs that move it one place earlier and later, drawn at the end
    /// of the row while the CURSOR is on it — `None` where it cannot go.
    pub moves: [Option<(char, Target)>; 2],
}

/// `part` of `whole`, in per cent, for a bar.
pub(crate) fn share(part: f64, whole: f64) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let percent = (part / whole.max(f64::MIN_POSITIVE) * 100.0)
        .clamp(0.0, 100.0)
        .round() as u8;
    percent
}

/// A speed as a person writes it: `1.25`, `2`, never `2.00`.
pub(crate) fn trim_float(x: f64) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A switch's answer, in words.
pub(crate) fn on_off(on: bool, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(if on {
        ronoctua_i18n::key::POINTERS_ON
    } else {
        ronoctua_i18n::key::POINTERS_OFF
    })
}

/// What every page of the view writes its rows with: where the text starts,
/// how much room it has, and which row and cell the cursor is on.
///
/// # Two passes, and one column a section
///
/// A page is written TWICE ([`Builder::build`]): the first pass only
/// measures — each section's labels and values, a section being what one
/// title heads — and the second places every `label: value` row of a section
/// with its values in one column, after the section's widest label
/// (`chrome::section_column`), the labels too long for the room before it
/// WRAPPED onto the lines under their row (`chrome::aligned_pair`). A row
/// placed on its own could only push its value off the column or cut its
/// label, and in a narrow door it did both.
pub struct Builder<'a> {
    /// The rows so far.
    pub(crate) out: Vec<Row>,
    /// The first cell the text starts at.
    pub(crate) left: u16,
    /// How many cells the text has.
    pub(crate) room: u16,
    /// Which row the cursor is on.
    pub(crate) cursor: usize,
    /// Which entry of a grid row the cursor is on.
    pub(crate) cell: usize,
    /// The words.
    pub(crate) texts: &'a ronoctua_i18n::Texts,
    /// The page's own first cell and room, before any section is nested.
    origin: (u16, u16),
    /// Which section the rows being written are in: one more at each title.
    section: usize,
    /// What the first pass measured of each section's pairs — `(label's
    /// first cell, label cells, value cells)` — while it is the first pass.
    measured: Option<Vec<Vec<(u16, u16, u16)>>>,
    /// Where each section's values begin, in the second pass.
    columns: Vec<u16>,
    /// A MARK for the next title — words of their own tone after it
    /// ([`Builder::title_mark`]) — taken by that title.
    mark: Option<(String, tokens::Color)>,
}

impl<'a> Builder<'a> {
    /// The rows `fill` writes on a page `left`/`room`, the cursor on row
    /// `cursor` and entry `cell` — written twice: measured, then placed with
    /// each section's values in one column.
    pub(crate) fn build(
        left: u16,
        room: u16,
        cursor: usize,
        cell: usize,
        texts: &'a ronoctua_i18n::Texts,
        fill: impl Fn(&mut Self),
    ) -> Vec<Row> {
        let blank = |measured, columns| Self {
            out: Vec::new(),
            left,
            room,
            cursor,
            cell,
            texts,
            origin: (left, room),
            section: 0,
            measured,
            columns,
            mark: None,
        };
        let mut first = blank(Some(vec![Vec::new()]), Vec::new());
        fill(&mut first);
        let columns = first
            .measured
            .unwrap_or_default()
            .iter()
            .map(|pairs| crate::chrome::section_column(left, room, pairs))
            .collect();
        let mut placed = blank(None, columns);
        fill(&mut placed);
        placed.out
    }

    /// Where the values of the section being written begin.
    fn column_here(&self) -> u16 {
        self.columns
            .get(self.section)
            .copied()
            .unwrap_or_else(|| crate::chrome::value_column(self.origin.0, self.origin.1))
    }
}

impl Builder<'_> {
    /// A text, by its key.
    pub(crate) fn t(&self, key: &str) -> String {
        self.texts.t(key)
    }

    /// A field of text in one colour, where nothing is under a click.
    pub(crate) const fn piece(at: u16, text: String, color: tokens::Color) -> Field {
        Field {
            at,
            text,
            color,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        }
    }

    /// How many lines are written so far — where the next row starts.
    pub(crate) fn len(&self) -> usize {
        self.out.len()
    }

    /// Every line written from `first` on, DIMMED — a station that did not
    /// answer: its words in the quiet register, the cursor's colour kept on
    /// the row it is on (the cursor is a colour, and a dimmed row it stands
    /// on still says so).
    pub(crate) fn dim_from(&mut self, first: usize) {
        for row in self.out.iter_mut().skip(first).filter(|r| !r.focused) {
            for f in &mut row.fields {
                f.color = tokens::color::TEXT_DIM;
            }
        }
    }

    /// A line the cursor skips.
    pub(crate) fn inert(&mut self, fields: Vec<Field>) {
        self.out.push(Row {
            fields,
            focused: false,
            heading: true,
            target: Target::Nothing,
            spot: String::new(),
            cells: Vec::new(),
            slider: None,
            continues: false,
            chips: Vec::new(),
            deck: None,
            schematic: None,
            covers: Vec::new(),
            globe: None,
            caret: None,
            lit: Lit::Row,
        });
    }

    /// A section's title, with air above it unless it is the first line —
    /// and a new section: its values have a column of their own.
    pub(crate) fn heading(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.heading_said(text);
    }

    /// A section's title already in words — a name that is DATA, as an
    /// ambient sound's on its credits: [`Builder::heading`] for a text no key
    /// holds.
    pub(crate) fn heading_said(&mut self, text: String) {
        self.section += 1;
        if let Some(m) = self.measured.as_mut() {
            m.push(Vec::new());
        }
        if !self.out.is_empty() {
            self.inert(crate::chrome::AIR);
        }
        self.title_lines(text);
    }

    /// A title's words, wrapped in the room and never cut: a long one in a
    /// narrow door ("Etiquetas Matroska do arquivo inteiro") ran past the
    /// edge and lost its last word.
    fn title_lines(&mut self, text: String) {
        for line in crate::chrome::note_lines(&text, self.room) {
            self.inert(vec![Self::piece(
                self.left,
                line,
                tokens::color::TEXT_HEADING,
            )]);
        }
    }

    /// A heading INSIDE a section: its title, with the air before it, and the
    /// section's values still in one column with those above it — the ambient
    /// sounds' groups, whose bars are one length only while they start in one
    /// column.
    pub(crate) fn heading_within(&mut self, text: String) {
        if !self.out.is_empty() {
            self.inert(crate::chrome::AIR);
        }
        self.title_lines(text);
    }

    /// A sentence, wrapped — never cut.
    pub(crate) fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        for line in crate::chrome::note_lines(&text, self.room) {
            self.inert(vec![Self::piece(self.left, line, tokens::color::TEXT_DIM)]);
        }
    }

    /// A sentence already in words that WARNS — a trouble, a refusal —
    /// wrapped, and never cut.
    pub(crate) fn warning(&mut self, said: &str) {
        for line in crate::chrome::note_lines(said, self.room) {
            self.inert(vec![Self::piece(
                self.left,
                line,
                tokens::color::STATE_WARNING,
            )]);
        }
    }

    /// A `label: value` row: its tones say whether it acts, and where the
    /// cursor is. Its value in its section's column, and its label whole —
    /// wrapped onto the lines under it when the room before the column is
    /// too short (`chrome::aligned_pair`), each such line continuing it.
    pub(crate) fn pair(&mut self, label: String, value: String, target: Target, spot: &str) {
        let wide = |t: &str| ronoctua_render::layout::width(t);
        let at = self.left;
        let section = self.section;
        // A PAIR WITH NO VALUE takes the whole row for its label, and needs no
        // column: measured, a long one — "Ask api.opensubtitles.com…" —
        // pushed every value of its section far to the right.
        if !value.is_empty()
            && let Some(pairs) = self.measured.as_mut().and_then(|m| m.get_mut(section))
        {
            pairs.push((at, wide(&label), wide(&value)));
        }
        let focused = self.cursor == self.out.len();
        let acts = target.acts();
        let kind = crate::chrome::Kind::of(acts);
        let lines = crate::chrome::aligned_pair(
            self.left,
            self.column_here(),
            self.left + self.room,
            &label,
            &value,
            kind,
            focused,
        );
        for (i, fields) in lines.into_iter().enumerate() {
            self.out.push(Row {
                fields,
                focused,
                heading: false,
                target: target.clone(),
                spot: spot.to_owned(),
                cells: Vec::new(),
                slider: None,
                continues: i > 0,
                chips: Vec::new(),
                deck: None,
                schematic: None,
                covers: Vec::new(),
                globe: None,
                caret: None,
                lit: Lit::Row,
            });
        }
    }

    /// A pair whose value is a BAR, `filled` per cent of it, and `text` after
    /// it — the bar is a control, and the row says where it was drawn.
    pub(crate) fn slider_pair(
        &mut self,
        label: String,
        filled: u8,
        text: &str,
        target: Target,
        spot: &str,
        of: Slid,
    ) {
        self.slider_pair_of(
            (tokens::space::PLAYER_VOLUMEBAR, 0),
            label,
            filled,
            text,
            (target, spot),
            of,
        );
    }

    /// [`Builder::slider_pair`] with a bar of `bar_cells` at the most — a
    /// page of several bars side by side gives them less, so their names are
    /// not squeezed to a stub in a narrow door — and room kept after it for
    /// `text_cells` at the least: the bars of one page are one length, sized
    /// for the longest word any of their rows can say, and never a cell
    /// shorter on the row whose word is longer.
    pub(crate) fn slider_pair_of(
        &mut self,
        (bar_cells, text_cells): (u16, u16),
        label: String,
        filled: u8,
        text: &str,
        (target, spot): (Target, &str),
        of: Slid,
    ) {
        // THE TEXT IS READ WHOLE, and the bar takes what is left of the
        // value's room, down to none: a time cut to `12:…` reads as a wrong
        // time, and a bar a cell shorter reads as the same bar. The first
        // pass measures the whole bar.
        let text_cells = ronoctua_render::layout::width(text).max(text_cells);
        let room = (self.left + self.room).saturating_sub(self.column_here());
        let cells = if self.measured.is_some() {
            bar_cells
        } else {
            bar_cells.min(room.saturating_sub(text_cells + 1))
        };
        let value = if cells == 0 {
            text.to_owned()
        } else {
            format!("{} {text}", crate::chrome::meter(filled, cells))
        };
        let first = self.out.len();
        self.pair(label, value, target, spot);
        // WHERE THE VALUE WAS PLACED is where the bar begins — read back from
        // the field, and never worked out a second time — and a value cut
        // short keeps only the cells it drew.
        if cells > 0
            && let Some(row) = self.out.get_mut(first)
            && let Some(value) = row.fields.get(1)
        {
            let drawn = ronoctua_render::layout::width(&value.text).min(cells);
            row.slider = (drawn > 0).then_some(Slider {
                of,
                from: value.at,
                cells: drawn,
            });
        }
    }

    /// The files' entries in up to three columns, filled DOWN each column
    /// — the order a listing is read in, so `↓` is the next name — and
    /// `←`/`→` across. Three where the room holds three of the longest name,
    /// two where names are long, one where a column would cut every name to
    /// a stub (`space.player.browseColumn`).
    ///
    /// NO PAGE OF THIS VIEW DRAWS ONE NOW — the files are the file picker's
    /// (`crate::file_picker`, whose grid fills the same way) — and it stays
    /// for the catalogue's compact columns (`player_posters::columns`), which
    /// another lane of this wave writes on it.
    #[allow(dead_code)]
    pub(crate) fn grid(&mut self, entries: &[(char, String, Target, String)]) {
        if entries.is_empty() {
            return;
        }
        let glyph_cells: u16 = 2;
        let gap: u16 = 2;
        let widest = entries
            .iter()
            .map(|(_, name, _, _)| ronoctua_protocol::cells(name))
            .max()
            .unwrap_or(0)
            .saturating_add(glyph_cells + gap);
        let floor = tokens::space::PLAYER_BROWSECOLUMN;
        let columns: u16 = if self.room / 3 >= widest.max(floor) {
            3
        } else if self.room / 2 >= floor {
            2
        } else {
            1
        };
        let width = self.room / columns;
        let count = entries.len();
        let per = count.div_ceil(usize::from(columns));
        for r in 0..per {
            let focused_row = self.cursor == self.out.len();
            let mut fields = Vec::new();
            let mut cells = Vec::new();
            for c in 0..columns {
                let Some((glyph, name, target, spot)) = entries.get(usize::from(c) * per + r)
                else {
                    continue;
                };
                let from = self.left + c * width;
                let here = focused_row && usize::from(c) == self.cell.min(usize::from(columns) - 1);
                let colour = if here {
                    tokens::color::TEXT_HERE_ACTS
                } else {
                    tokens::color::TEXT_PRIMARY
                };
                let name = ronoctua_render::layout::elide_end(
                    name,
                    width.saturating_sub(glyph_cells + gap),
                );
                fields.push(Self::piece(
                    from,
                    glyph.to_string(),
                    tokens::color::TEXT_DIM,
                ));
                fields.push(Self::piece(from + glyph_cells, name, colour));
                cells.push(Cell {
                    from,
                    width,
                    target: target.clone(),
                    spot: spot.clone(),
                    control: false,
                    about: None,
                });
            }
            let on = self.cell.min(cells.len().saturating_sub(1));
            let target = if focused_row {
                cells.get(on).map_or(Target::Nothing, |c| c.target.clone())
            } else {
                cells.first().map_or(Target::Nothing, |c| c.target.clone())
            };
            let spot = cells.first().map(|c| c.spot.clone()).unwrap_or_default();
            self.out.push(Row {
                fields,
                focused: focused_row,
                heading: false,
                target,
                spot,
                cells,
                slider: None,
                continues: false,
                chips: Vec::new(),
                deck: None,
                schematic: None,
                covers: Vec::new(),
                globe: None,
                caret: None,
                lit: Lit::Row,
            });
        }
    }

    /// A title with a control at the end of its line — a glyph that acts on a
    /// click (`View::clicked_at`) and says what it does under the pointer:
    /// `about` is the command id of its sentence, never optional. The key
    /// that does the same is taught in the hint line.
    pub(crate) fn heading_with(
        &mut self,
        key: &str,
        glyph: char,
        target: Target,
        about: &'static str,
    ) {
        self.heading_controls(key, &[(glyph, target, about)]);
    }

    /// A title with CONTROLS at the end of its first line, side by side —
    /// each a glyph in its own `space.player.queueGlyph` cells, COUNTED FROM
    /// THE RIGHT EDGE of the room, so a control keeps its place whatever the
    /// title says and in either language; the last one given is the one at
    /// the edge. Each acts on a click and says its `about` under the pointer
    /// ([`control_under`]).
    ///
    /// THE TITLE IS WRAPPED IN WHAT IS LEFT, with a cell of air before the
    /// first control (`space.player.queueAir`): a long title in a narrow
    /// door ran under the glyphs.
    pub(crate) fn heading_controls(
        &mut self,
        key: &str,
        controls: &[(char, Target, &'static str)],
    ) {
        let mark = self.mark.take();
        self.title_marked(key, mark, controls);
    }

    /// A MARK after the next title ([`Builder::heading_with`],
    /// [`Builder::heading_controls`]) — words of its own tone, after the
    /// title and the `player-meta-joiner`: the Queue's file, "not saved" in
    /// the danger tone (`player_queue::kept_mark`). Set apart from the title
    /// so its controls stay literals at the call the checks read.
    pub(crate) fn title_mark(&mut self, mark: Option<(String, tokens::Color)>) {
        self.mark = mark;
    }

    /// A title, its mark and its controls. Title and mark share the line
    /// when they fit before the controls; else the mark wraps onto the lines
    /// under the title, whole and in its tone.
    fn title_marked(
        &mut self,
        key: &str,
        mark: Option<(String, tokens::Color)>,
        controls: &[(char, Target, &'static str)],
    ) {
        let text = self.texts.t(key);
        self.title_said_marked(text, mark, controls);
    }

    /// [`Builder::heading_controls`] for a title already in words — what a
    /// listing of the catalogue is titled by: a theme's name, a category's,
    /// a search's words. A mark set before it ([`Builder::title_mark`]) is
    /// taken by it, as by any title.
    pub(crate) fn heading_controls_said(
        &mut self,
        text: String,
        controls: &[(char, Target, &'static str)],
    ) {
        let mark = self.mark.take();
        self.title_said_marked(text, mark, controls);
    }

    /// [`Builder::title_marked`], for a title already in words.
    fn title_said_marked(
        &mut self,
        text: String,
        mark: Option<(String, tokens::Color)>,
        controls: &[(char, Target, &'static str)],
    ) {
        self.section += 1;
        if let Some(m) = self.measured.as_mut() {
            m.push(Vec::new());
        }
        if !self.out.is_empty() {
            self.inert(crate::chrome::AIR);
        }
        let step = tokens::space::PLAYER_QUEUEGLYPH;
        let n = u16::try_from(controls.len()).unwrap_or(u16::MAX);
        let cells = n.saturating_mul(step).saturating_add(if n > 0 {
            tokens::space::PLAYER_QUEUEAIR
        } else {
            0
        });
        let first = self.out.len();
        let room = self.room;
        self.room = self.room.saturating_sub(cells).max(1);
        match mark {
            None => self.title_lines(text),
            Some((mark, tone)) => {
                let joiner = self.texts.t(ronoctua_i18n::key::PLAYER_META_JOINER);
                let wide = |t: &str| ronoctua_render::layout::width(t);
                let title = wide(&text);
                // ON THE TITLE'S LINE when the whole of it fits there.
                if title + wide(&joiner) + wide(&mark) <= self.room {
                    // THE JOINER IS THE TITLE'S, and only the mark wears
                    // its own tone.
                    let at = self.left + title + wide(&joiner);
                    self.inert(vec![
                        Self::piece(
                            self.left,
                            format!("{text}{joiner}"),
                            tokens::color::TEXT_HEADING,
                        ),
                        Self::piece(at, mark, tone),
                    ]);
                } else {
                    self.title_lines(text);
                    for line in crate::chrome::note_lines(&mark, self.room) {
                        self.inert(vec![Self::piece(self.left, line, tone)]);
                    }
                }
            }
        }
        self.room = room;
        let edge = self.left + self.room;
        let Some(row) = self.out.get_mut(first) else {
            return;
        };
        for (k, (glyph, target, about)) in controls.iter().enumerate() {
            let from_edge = (n - u16::try_from(k).unwrap_or(0)).saturating_mul(step);
            let at = edge.saturating_sub(from_edge);
            // THE GLYPH WEARS ITS TITLE'S TONE, as the drawers' chevrons do
            // (`network::drawer`): a second, quieter text tone on a title
            // line is the title read as body text.
            row.fields.push(Self::piece(
                at,
                glyph.to_string(),
                tokens::color::TEXT_HEADING,
            ));
            row.cells.push(Cell {
                from: at,
                width: step,
                target: target.clone(),
                spot: format!("control-{about}"),
                control: true,
                about: Some(about),
            });
        }
    }

    /// A `label: value` row with a CONTROL at the end of its line — a glyph
    /// a click acts on at once ([`Cell::control`]), as a title's; the key
    /// that does the same is taught in the hint line.
    pub(crate) fn pair_with_control(
        &mut self,
        label: String,
        value: String,
        target: Target,
        spot: &str,
        control: Option<(char, Target)>,
    ) {
        let first = self.out.len();
        self.pair(label, value, target, spot);
        let Some((glyph, acts)) = control else {
            return;
        };
        if let Some(row) = self.out.get_mut(first) {
            let at = self.left + self.room.saturating_sub(tokens::space::PLAYER_QUEUEGLYPH);
            let colour = if row.focused {
                tokens::color::TEXT_HERE_ACTS
            } else {
                tokens::color::TEXT_SECONDARY
            };
            row.fields.push(Self::piece(at, glyph.to_string(), colour));
            row.cells.push(Cell {
                from: at,
                width: tokens::space::PLAYER_QUEUEGLYPH,
                target: acts,
                spot: format!("{spot}-control"),
                control: true,
                about: None,
            });
        }
    }

    /// A `label: value` row with CONTROLS at the end of its line, side by
    /// side — glyphs a click acts on at once ([`Cell::control`]), each in
    /// its own `space.player.queueGlyph` cells, counted from the row's right
    /// edge so a control keeps its place whatever the label says. A `None`
    /// keeps its place EMPTY: an arrow that cannot move a picture further is
    /// not drawn, and the one beside it does not slide into its cells.
    pub(crate) fn pair_with_controls(
        &mut self,
        label: String,
        value: String,
        target: Target,
        spot: &str,
        controls: &[Option<(char, Target)>],
    ) {
        let step = tokens::space::PLAYER_QUEUEGLYPH;
        let n = u16::try_from(controls.len()).unwrap_or(u16::MAX);
        // THE CONTROLS' CELLS ARE THEIRS: the label and the value are laid
        // out in what is left, wrapped there — a value run under an arrow
        // was an arrow drawn over a word, in a narrow column.
        let first = self.out.len();
        let room = self.room;
        // AND A CELL OF AIR between the value and the first control
        // (`space.player.queueAir`): a glyph against the last letter reads as
        // part of the word.
        let controls_cells = n
            .saturating_mul(step)
            .saturating_add(tokens::space::PLAYER_QUEUEAIR);
        self.room = self.room.saturating_sub(controls_cells);
        self.pair(label, value, target, spot);
        self.room = room;
        // AND THE SECTION'S VALUES' COLUMN KNOWS THEM: measured, the value is
        // as wide as it is and its controls, so the column moves left enough
        // for both where the row allows it (`chrome::section_column`) — and
        // the value is not cut to make room for the arrows.
        let section = self.section;
        if let Some(measured) = self
            .measured
            .as_mut()
            .and_then(|m| m.get_mut(section))
            .and_then(|pairs| pairs.last_mut())
        {
            measured.2 = measured.2.saturating_add(controls_cells);
        }
        let Some(row) = self.out.get_mut(first) else {
            return;
        };
        let colour = if row.focused {
            tokens::color::TEXT_HERE_ACTS
        } else {
            tokens::color::TEXT_SECONDARY
        };
        for (k, control) in (0u16..).zip(controls) {
            let Some((glyph, acts)) = control else {
                continue;
            };
            let at = (self.left + self.room).saturating_sub((n - k) * step);
            row.fields.push(Self::piece(at, glyph.to_string(), colour));
            row.cells.push(Cell {
                from: at,
                width: step,
                target: acts.clone(),
                spot: format!("{spot}-control-{k}"),
                control: true,
                about: None,
            });
        }
    }

    /// The Layout page's SCHEMATIC ([`crate::player_layout::Schematic`]): as
    /// many lines as it is tall, each one a figure the cursor skips — the
    /// doors paint the pictures' places over them ([`SchematicRow`]).
    pub(crate) fn schematic(&mut self, figure: crate::player_layout::Schematic) {
        let figure = std::rc::Rc::new(figure);
        for line in 0..figure.lines {
            self.out.push(Row {
                fields: Vec::new(),
                focused: false,
                heading: true,
                target: Target::Nothing,
                spot: format!("schematic-{line}"),
                cells: Vec::new(),
                slider: None,
                continues: false,
                chips: Vec::new(),
                deck: None,
                schematic: Some(SchematicRow {
                    line,
                    left: self.left,
                    figure: figure.clone(),
                }),
                covers: Vec::new(),
                globe: None,
                caret: None,
                lit: Lit::Row,
            });
        }
    }

    /// A COVER'S PICTURE ([`CoverRow`]): `space.player.coverLines` lines
    /// the cursor skips, as wide as the rows, and the doors draw the picture
    /// the view holds under `key` over them, in its own proportion `size`.
    pub(crate) fn cover(&mut self, key: &str, size: (u32, u32)) {
        self.cover_lines(
            &[(self.left, self.room, Some((key, size)))],
            tokens::space::PLAYER_COVERLINES,
        );
    }

    /// PICTURES SIDE BY SIDE, in ONE GEOMETRY: the rows' room shared in
    /// equal boxes, `space.player.coverGap` cells apart, each drawn in its
    /// box as a cover is (`player_metadata::cover_place`) — a suggested
    /// cover beside the file's own — and under them a line saying which is
    /// which, each caption at its box's first cell. A box whose picture is
    /// not here yet keeps its place: the other does not slide into it.
    pub(crate) fn pictures(&mut self, pictures: &[Pictured]) {
        // NO MORE THAN THE RENDERER COUNTS: a picture past its ceiling would
        // be dropped silently (`ronoctua_render::image::PLAYER_COVERS`).
        let pictures = &pictures[..pictures.len().min(ronoctua_render::image::PLAYER_COVERS)];
        let n = u16::try_from(pictures.len()).unwrap_or(u16::MAX).max(1);
        let gap = tokens::space::PLAYER_COVERGAP;
        let cells = self
            .room
            .saturating_sub(gap.saturating_mul(n - 1))
            .checked_div(n)
            .unwrap_or(0);
        let boxes: Vec<PictureBox<'_>> = (0u16..)
            .zip(pictures)
            .map(|(i, (p, _))| {
                (
                    self.left + i * (cells + gap),
                    cells,
                    p.as_ref().map(|(k, size)| (k.as_str(), *size)),
                )
            })
            .collect();
        // AS TALL AS THE TALLEST PICTURE IS IN ITS BOX, at the most
        // `space.player.coverLines`: two boxes side by side are narrow, a
        // square fills its width before its height, and the lines under it
        // were air between the pictures and what they are. A picture not
        // here yet is taken for a square.
        let size = tokens::text::size();
        let (cell_px, line_px) = (
            tokens::metrics::cell_width(size),
            tokens::metrics::line_height(size),
        );
        let air = crate::files::picture_air();
        let most = tokens::space::PLAYER_COVERLINES;
        let lines = pictures
            .iter()
            .map(|(p, _)| {
                let (w, h) = p.as_ref().map_or((1, 1), |(_, s)| *s);
                #[allow(clippy::cast_precision_loss)]
                let tall = f32::from(cells) * cell_px * h as f32 / w.max(1) as f32;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let lines = ((tall + air + air) / line_px.max(1.0)).ceil() as u16;
                lines
            })
            .max()
            .unwrap_or(most)
            .clamp(1, most);
        self.cover_lines(&boxes, lines);
        // EACH CAPTION WRAPPED IN ITS BOX, never cut: "Fetching the cover…"
        // in a narrow door's box was "Fetching the…".
        let captions: Vec<Vec<String>> = pictures
            .iter()
            .map(|(_, caption)| crate::chrome::note_lines(caption, cells.max(1)))
            .collect();
        let tall = captions.iter().map(Vec::len).max().unwrap_or(0);
        for line in 0..tall {
            let fields = (0u16..)
                .zip(&captions)
                .filter_map(|(i, lines)| {
                    let text = lines.get(line).filter(|t| !t.is_empty())?;
                    Some(Self::piece(
                        self.left + i * (cells + gap),
                        text.clone(),
                        tokens::color::TEXT_DIM,
                    ))
                })
                .collect();
            self.inert(fields);
        }
    }

    /// A `label: value` row whose VALUE IS READ WHOLE — wrapped in its
    /// column, beside the rest of the label while it wraps and under it
    /// after — never cut: a ring's answer that is a sentence ("a .lrc beside
    /// the song") was cut to "a .lrc bes…" in a narrow door.
    pub(crate) fn pair_whole(&mut self, label: String, value: String, target: Target, spot: &str) {
        self.pair_wrapping(label, value, None, target, spot);
    }

    /// `lines` lines the cursor skips, each carrying a line of every picture
    /// of `boxes` — its first cell, its cells, and the picture's key and
    /// size, when there is one to draw.
    fn cover_lines(&mut self, boxes: &[PictureBox<'_>], lines: u16) {
        let keys: Vec<Option<Keyed>> = boxes
            .iter()
            .map(|(_, _, p)| p.map(|(k, size)| (std::rc::Rc::from(k), size)))
            .collect();
        for line in 0..lines {
            let covers = boxes
                .iter()
                .zip(&keys)
                .filter_map(|((left, cells, _), key)| {
                    let (key, size) = key.clone()?;
                    Some(CoverRow {
                        line,
                        lines,
                        left: *left,
                        cells: *cells,
                        key,
                        size,
                    })
                })
                .collect();
            self.out.push(Row {
                fields: Vec::new(),
                focused: false,
                heading: true,
                target: Target::Nothing,
                spot: format!("cover-{line}"),
                cells: Vec::new(),
                slider: None,
                continues: false,
                chips: Vec::new(),
                deck: None,
                schematic: None,
                covers,
                globe: None,
                caret: None,
                lit: Lit::Row,
            });
        }
    }

    /// A `label: value` row and, UNDER it — set in by `view.nestCells`, in
    /// the quiet register, wrapped to the row's whole width and never cut —
    /// what is said of it: the hosts a switch lets be asked, where a key is
    /// sent, what a result of a search is. With no value the label takes the
    /// whole row too: a result's name is data as long as it is, and a column
    /// of values squeezed it to a stub cut mid-word in a narrow door. The
    /// lines under it continue the row: a click on them is a click on it.
    pub(crate) fn pair_under(
        &mut self,
        label: String,
        value: String,
        under: &str,
        target: Target,
        spot: &str,
    ) {
        self.pair(label, value, target.clone(), spot);
        let nest = tokens::space::VIEW_NESTCELLS.min(self.room);
        for line in crate::chrome::note_lines(under, self.room - nest) {
            if line.is_empty() {
                continue;
            }
            self.out.push(Row {
                fields: vec![Self::piece(self.left + nest, line, tokens::color::TEXT_DIM)],
                focused: false,
                heading: false,
                target: target.clone(),
                spot: spot.to_owned(),
                cells: Vec::new(),
                slider: None,
                continues: true,
                chips: Vec::new(),
                deck: None,
                schematic: None,
                covers: Vec::new(),
                globe: None,
                caret: None,
                lit: Lit::Row,
            });
        }
    }

    /// A ROW THAT PLAYS — a station, a channel, a film, a live — with its
    /// STAR at its head: kept among the favourites or not, a glyph a click
    /// turns at once ([`Cell::control`]) as `b` on the row does; then its name
    /// and what it is, and under it, in the quiet register, what more is said
    /// of it (`crate::player_directory`). The label keeps its column whether
    /// the star is on or off.
    pub(crate) fn pair_with_star(
        &mut self,
        (on, star): (bool, Target),
        label: String,
        value: String,
        under: &str,
        target: Target,
        spot: &str,
    ) {
        let (left, room) = (self.left, self.room);
        let step = tokens::space::PLAYER_QUEUEGLYPH.min(room);
        self.left += step;
        self.room -= step;
        let first = self.out.len();
        self.pair_under(label, value, under, target, spot);
        (self.left, self.room) = (left, room);
        if let Some(row) = self.out.get_mut(first) {
            let colour = row
                .fields
                .first()
                .map_or(tokens::color::TEXT_SECONDARY, |f| f.color);
            let glyph = if on {
                tokens::icon::PLAYER_FAVOURITE
            } else {
                tokens::icon::PLAYER_FAVOURITE_OFF
            };
            row.fields
                .insert(0, Self::piece(left, glyph.to_string(), colour));
            row.cells.push(Cell {
                from: left,
                width: step,
                target: star,
                spot: format!("{spot}-star"),
                control: true,
                about: None,
            });
        }
    }

    /// THE GLOBE's picture on a directory's page ([`GlobeRow`]): `lines`
    /// lines the cursor skips, as wide as the rows, and the doors draw the
    /// view's globe over them (`crate::player_globe`).
    pub(crate) fn globe(&mut self, lines: u16) {
        for line in 0..lines {
            self.out.push(Row {
                fields: Vec::new(),
                focused: false,
                heading: true,
                target: Target::Nothing,
                spot: format!("globe-{line}"),
                cells: Vec::new(),
                slider: None,
                continues: false,
                chips: Vec::new(),
                deck: None,
                schematic: None,
                covers: Vec::new(),
                globe: Some(GlobeRow {
                    line,
                    lines,
                    left: self.left,
                    cells: self.room,
                }),
                caret: None,
                lit: Lit::Row,
            });
        }
    }

    /// A `label: value` row with a TOGGLE at its head — a glyph, on or off, a
    /// click on it turns at once ([`Cell::control`]) as `RET` and the arrows
    /// on the row do — and, under its value in the quiet register, `now`:
    /// what the thing says now, beside what it would become. `None` keeps the
    /// toggle's cells empty: a row that has nothing to turn keeps its label in
    /// the same column as the ones that have.
    pub(crate) fn pair_with_toggle(
        &mut self,
        toggle: Option<bool>,
        label: String,
        value: String,
        now: Option<String>,
        target: Target,
        spot: &str,
    ) {
        let (left, room) = (self.left, self.room);
        let step = tokens::space::PLAYER_QUEUEGLYPH.min(room);
        self.left += step;
        self.room -= step;
        let first = self.out.len();
        self.pair_wrapping(label, value, now, target.clone(), spot);
        (self.left, self.room) = (left, room);
        let Some(on) = toggle else {
            return;
        };
        if let Some(row) = self.out.get_mut(first) {
            let colour = row
                .fields
                .first()
                .map_or(tokens::color::TEXT_SECONDARY, |f| f.color);
            let glyph = if on {
                tokens::icon::PLAYER_IMPORT_ON
            } else {
                tokens::icon::PLAYER_IMPORT_OFF
            };
            row.fields
                .insert(0, Self::piece(left, glyph.to_string(), colour));
            row.cells.push(Cell {
                from: left,
                width: step,
                target,
                spot: format!("{spot}-toggle"),
                control: true,
                about: None,
            });
        }
    }

    /// A `label: value` row, its value wrapped whole in its column and,
    /// after it in the quiet register, `now` — the one arithmetic of
    /// [`Builder::pair_with_toggle`] and [`Builder::pair_whole`].
    fn pair_wrapping(
        &mut self,
        label: String,
        value: String,
        now: Option<String>,
        target: Target,
        spot: &str,
    ) {
        let first = self.out.len();
        // THE VALUE WHOLE, wrapped in its column: what would be written is
        // read to its end, never cut — its first line on the row, the rest
        // beside the rest of the label, or under it. Measured whole.
        let column = self.column_here();
        let end = self.left + self.room;
        let values: Vec<String> = if self.measured.is_some() || value.is_empty() {
            vec![value]
        } else {
            crate::chrome::note_lines(&value, end.saturating_sub(column))
                .into_iter()
                .filter(|l| !l.is_empty())
                .collect()
        };
        let focused = self.cursor == first;
        self.pair(
            label,
            values.first().cloned().unwrap_or_default(),
            target.clone(),
            spot,
        );
        // IN THE VALUE'S COLUMN, beside the rest of the label while it wraps
        // and under it after: the rest of the value in its own tone, then
        // what the thing says now in the quiet register.
        let column = self
            .out
            .get(first)
            .and_then(|row| row.fields.get(1))
            .map_or(column, |f| f.at);
        let tone = crate::chrome::Kind::of(target.acts()).tones(focused).1;
        let now_lines = now.map_or_else(Vec::new, |now| {
            crate::chrome::note_lines(&now, end.saturating_sub(column))
        });
        let beside = values.into_iter().skip(1).map(|line| (line, tone)).chain(
            now_lines
                .into_iter()
                .filter(|l| !l.is_empty())
                .map(|line| (line, tokens::color::TEXT_DIM)),
        );
        for (i, (line, colour)) in beside.enumerate() {
            let piece = Self::piece(column, line, colour);
            match self.out.get_mut(first + 1 + i) {
                Some(row) if row.continues && row.target == target => row.fields.push(piece),
                _ => self.out.push(Row {
                    fields: vec![piece],
                    focused: false,
                    heading: false,
                    target: target.clone(),
                    spot: spot.to_owned(),
                    cells: Vec::new(),
                    slider: None,
                    continues: true,
                    chips: Vec::new(),
                    deck: None,
                    schematic: None,
                    covers: Vec::new(),
                    globe: None,
                    caret: None,
                    lit: Lit::Row,
                }),
            }
        }
    }

    /// Lights, on the schematic written so far, the picture whose row the
    /// cursor is on — asked once the page is written, since the pictures'
    /// rows come after the figure they light.
    pub(crate) fn light_schematic(&mut self) {
        let lit = match self.out.get(self.cursor).map(|r| &r.target) {
            Some(Target::LayoutPicture(id)) => Some(*id),
            _ => None,
        };
        let mut shared: Option<std::rc::Rc<crate::player_layout::Schematic>> = None;
        for row in &mut self.out {
            let Some(s) = row.schematic.as_mut() else {
                continue;
            };
            if s.figure.lit == lit {
                continue;
            }
            let figure = shared
                .get_or_insert_with(|| {
                    let mut f = (*s.figure).clone();
                    f.lit = lit;
                    std::rc::Rc::new(f)
                })
                .clone();
            s.figure = figure;
        }
    }

    /// A `label: value` row with a MARK in front of its label — what kind of
    /// thing it is, a glyph — the label and the lines it wraps onto set in
    /// after it, so a name too long for the room never leaves the mark alone
    /// on its line.
    pub(crate) fn pair_with_mark(
        &mut self,
        mark: char,
        label: String,
        value: String,
        target: Target,
        spot: &str,
    ) {
        self.marked(mark, |b| b.pair(label, value, target, spot));
    }

    /// The row `write` writes — a pair, a bar — with a MARK in front of its
    /// label: set in by `space.player.queueGlyph`, and the glyph in the
    /// label's own tone in the room it left.
    pub(crate) fn marked(&mut self, mark: char, write: impl FnOnce(&mut Self)) {
        let (left, room) = (self.left, self.room);
        let step = tokens::space::PLAYER_QUEUEGLYPH.min(room);
        self.left += step;
        self.room -= step;
        let first = self.out.len();
        write(self);
        (self.left, self.room) = (left, room);
        if let Some(row) = self.out.get_mut(first) {
            // IN THE LABEL'S OWN TONE, the cursor's included: the mark is
            // part of what the label says.
            let colour = row
                .fields
                .first()
                .map_or(tokens::color::TEXT_SECONDARY, |f| f.color);
            row.fields
                .insert(0, Self::piece(left, mark.to_string(), colour));
        }
    }

    /// A `label: value` row with a subtitle lane's colour CHIP in front of
    /// its label ([`Chip`]): `glyph` drawn in `colour` in the middle of the
    /// chip's cells, the label and the lines it wraps onto set in after
    /// them, and the chip a CONTROL — a click on it does `toggle` at once, as
    /// a group's triangle does.
    ///
    /// `colour` is a lane's, and it is drawn ONLY here, on the chip's ground:
    /// the row's own tones say whether it acts and where the cursor is.
    pub(crate) fn pair_with_chip(
        &mut self,
        (glyph, colour): (char, tokens::Color),
        label: String,
        value: String,
        (target, toggle): (Target, Target),
        spot: &str,
    ) {
        let (left, room) = (self.left, self.room);
        let step = tokens::space::PLAYER_CHIPCELLS.min(room);
        self.left += step;
        self.room -= step;
        let first = self.out.len();
        self.pair(label, value, target, spot);
        (self.left, self.room) = (left, room);
        if let Some(row) = self.out.get_mut(first) {
            row.fields
                .insert(0, Self::piece(left + 1, glyph.to_string(), colour));
            row.chips.push(Chip { at: left });
            row.cells.push(Cell {
                from: left,
                width: step,
                target: toggle,
                spot: format!("{spot}-chip"),
                control: true,
                about: None,
            });
        }
    }

    /// A row of the queue's tree: set in by its depth, its number, a group's
    /// triangle — a control — and its name cut to what is left.
    pub(crate) fn entry(&mut self, e: Entry) {
        let focused = self.cursor == self.out.len();
        let colour = match (focused, e.current) {
            (true, _) => tokens::color::TEXT_HERE_ACTS,
            (false, true) => tokens::color::STATE_ACCENT,
            (false, false) => tokens::color::TEXT_PRIMARY,
        };
        let indent = e.depth.saturating_mul(tokens::space::VIEW_NESTCELLS);
        let at = self.left.saturating_add(indent);
        let (number, number_cells) = e.number;
        let glyph_at = at.saturating_add(number_cells);
        let name_at = if e.glyph_room || e.fold.is_some() || e.mark.is_some() {
            glyph_at.saturating_add(tokens::space::PLAYER_QUEUEGLYPH)
        } else {
            glyph_at
        };
        // THE MOVES' CELLS ARE THEIRS, on the cursor's row: the name is cut
        // before them, with a cell of air, as `pair_with_controls` keeps.
        let step = tokens::space::PLAYER_QUEUEGLYPH;
        let moves = focused && e.moves.iter().any(Option::is_some);
        let moves_cells = if moves {
            step.saturating_mul(2)
                .saturating_add(tokens::space::PLAYER_QUEUEAIR)
        } else {
            0
        };
        let room = (self.left + self.room)
            .saturating_sub(name_at)
            .saturating_sub(moves_cells);
        let name = ronoctua_render::layout::elide_end(&e.name, room);
        let mut fields = vec![Self::piece(at, number, tokens::color::TEXT_DIM)];
        let mut cells = Vec::new();
        if let Some((open, fold)) = e.fold {
            let glyph = if open {
                tokens::icon::PLAYER_UNFOLDED
            } else {
                tokens::icon::PLAYER_FOLDED
            };
            fields.push(Self::piece(
                glyph_at,
                glyph.to_string(),
                tokens::color::TEXT_SECONDARY,
            ));
            cells.push(Cell {
                from: glyph_at,
                width: tokens::space::PLAYER_QUEUEGLYPH,
                target: fold,
                spot: format!("{}-fold", e.spot),
                control: true,
                about: None,
            });
        }
        if let Some(mark) = e.mark {
            fields.push(Self::piece(
                glyph_at,
                mark.to_string(),
                tokens::color::TEXT_SECONDARY,
            ));
        }
        fields.push(Self::piece(name_at, name, colour));
        if moves {
            let end = self.left + self.room;
            for (k, m) in (0u16..).zip(&e.moves) {
                let Some((glyph, acts)) = m else {
                    continue;
                };
                let at = end.saturating_sub((2 - k) * step);
                fields.push(Self::piece(
                    at,
                    glyph.to_string(),
                    tokens::color::TEXT_HERE_ACTS,
                ));
                cells.push(Cell {
                    from: at,
                    width: step,
                    target: acts.clone(),
                    spot: format!("{}-move-{k}", e.spot),
                    control: true,
                    about: None,
                });
            }
        }
        self.out.push(Row {
            fields,
            focused,
            heading: false,
            target: e.target,
            spot: e.spot,
            cells,
            slider: None,
            continues: false,
            chips: Vec::new(),
            deck: None,
            schematic: None,
            covers: Vec::new(),
            globe: None,
            caret: None,
            lit: Lit::Row,
        });
    }

    /// THE DECK AS ROWS — `lanes` on one clock, as `state` plays them: the
    /// ruler, then a row for each lane, every lane shown (the Timeline page,
    /// the footer's "Now"). Each row carries its line of the deck, laid out
    /// ONCE here on the page's own cells ([`DeckRow`]), and its words from
    /// that line — the keys walk them: the ruler's row is the position (the
    /// arrows seek), a lane's is the lane (its page, its start, taking it
    /// out) — and each door paints the deck's shapes under the words and
    /// takes a press where the deck says.
    pub(crate) fn deck(&mut self, lanes: &[ronoctua_player::own::Lane], state: &State) {
        use crate::player_timeline::{DeckLine, Room, of_state};
        let deck = std::rc::Rc::new(of_state(
            lanes,
            Some(state),
            Room::wide(self.room).every_lane(),
            0,
        ));
        for line in 0..deck.lines.len() {
            let focused = self.cursor == self.out.len();
            let fields = deck
                .words(line, state, self.texts, None, focused)
                .into_iter()
                .map(|w| Self::piece(self.left + w.at, w.text, w.color))
                .collect();
            let (target, spot) = match &deck.lines[line] {
                DeckLine::Ruler(_) => (Target::Position, "deck-ruler".to_owned()),
                DeckLine::Lane(l) => (Target::Lane(l.lane), format!("deck-lane-{}", l.lane.0)),
            };
            self.out.push(Row {
                fields,
                focused,
                heading: false,
                target,
                spot,
                cells: Vec::new(),
                slider: None,
                continues: false,
                chips: Vec::new(),
                deck: Some(DeckRow {
                    line,
                    left: self.left,
                    deck: deck.clone(),
                }),
                schematic: None,
                covers: Vec::new(),
                globe: None,
                caret: None,
                lit: Lit::Row,
            });
        }
    }

    /// The rows of one page set in by `space.view.nestCells` — what `rows`
    /// writes while it runs is under what came before it. Hierarchy is air
    /// and colour here, never a rule.
    pub(crate) fn nested(&mut self, rows: impl FnOnce(&mut Self)) {
        let (left, room) = (self.left, self.room);
        self.left += tokens::space::VIEW_NESTCELLS;
        self.room = self.room.saturating_sub(tokens::space::VIEW_NESTCELLS);
        rows(self);
        (self.left, self.room) = (left, room);
    }
}

#[cfg(test)]
mod whole_row_tests {
    use super::*;

    /// FAILS IF: a row of several lines — a result and what is said under
    /// it — lights less than all its lines under the pointer, from any of
    /// them, while a click on any takes it whole; or a hover lights the row
    /// after it.
    #[test]
    fn a_row_of_several_lines_lights_whole_from_any_of_them() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("the base language");
        let rows = Builder::build(0, 24, 99, 0, &t, |b| {
            b.pair("First".into(), String::new(), Target::Rename, "first");
            b.pair_under(
                "Harbour.2024.1080p.BluRay".into(),
                String::new(),
                "OpenSubtitles · 12 downloads · timed for this file",
                Target::Invite,
                "result",
            );
            b.pair("After".into(), String::new(), Target::Silence, "after");
        });
        let first = rows
            .iter()
            .position(|r| r.target == Target::Invite)
            .expect("the result");
        let last = rows
            .iter()
            .rposition(|r| r.target == Target::Invite)
            .expect("the result");
        assert!(last > first, "the fixture has one line: {rows:?}");
        for n in first..=last {
            assert_eq!(whole_row(&rows, Some(n)), first..last + 1, "from line {n}");
        }
        assert_eq!(whole_row(&rows, Some(0)), 0..1);
        assert_eq!(whole_row(&rows, None), 0..0);
    }

    /// FAILS IF: a row with no value — whose label takes the whole row — is
    /// measured for its section's column: a long one ("Ask
    /// api.opensubtitles.com…") pushed every value of the section to the
    /// right edge.
    #[test]
    fn a_row_with_no_value_moves_no_column() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("the base language");
        let at_of = |with_long: bool| {
            let rows = Builder::build(0, 60, 99, 0, &t, |b| {
                if with_long {
                    b.pair(
                        "Ask api.opensubtitles.com and api.subdl.com now".into(),
                        String::new(),
                        Target::Rename,
                        "ask",
                    );
                }
                b.pair("Left today".into(), "18".into(), Target::Invite, "left");
            });
            rows.iter()
                .find(|r| r.target == Target::Invite)
                .and_then(|r| r.fields.get(1))
                .map(|f| f.at)
                .expect("the value")
        };
        assert_eq!(
            at_of(true),
            at_of(false),
            "a label with no value moved the column"
        );
    }

    /// FAILS IF: a title longer than the room runs past the edge — a narrow
    /// door lost "inteiro" of "Etiquetas Matroska do arquivo inteiro".
    #[test]
    fn a_long_title_wraps_and_loses_no_word() {
        let t = ronoctua_i18n::Texts::fresh("pt-BR").expect("a language");
        let title = "Etiquetas Matroska do arquivo inteiro";
        let rows = Builder::build(0, 20, 99, 0, &t, |b| {
            b.heading_within(title.into());
        });
        let lines: Vec<String> = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(lines.len() > 1, "{lines:?}");
        assert!(
            lines
                .iter()
                .all(|l| ronoctua_render::layout::width(l) <= 20),
            "{lines:?}"
        );
        assert_eq!(lines.join(" "), title);
    }
}
