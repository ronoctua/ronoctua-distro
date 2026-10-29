//! `LEADER s t` — the themes, as a list in the footer, and beside them the
//! choices ABOUT a theme.
//!
//! # What it does, and what it deliberately does not
//!
//! It is the network list's shape: a cursor over a few rows, `RET` on the row,
//! `Esc` out. What differs is that MOVING THE CURSOR PREVIEWS: the choice under
//! the cursors becomes the process's the moment a cursor lands, so the footer —
//! the surface this list is drawn on — is the swatch. There is no swatch drawn
//! in a theme's own colours beside its id, and there cannot be one: a `Color`
//! is a role that resolves through the CURRENT theme, and a `Color` bound to
//! another theme is exactly the thing the tokens crate has no way to express.
//! The row is the id alone, an identifier as the `M-x` list shows command ids;
//! a name in the person's language was a second spelling of the same thing.
//!
//! # Six columns, each a list of its own
//!
//! Beside the themes, the five choices about a theme (`crate::theme`): the
//! desk the compositor paints where no window is ([`Backdrop`]), what the
//! footer stands on and what the windows stand on ([`Ground`], asked twice),
//! how loud the windows' outlines are ([`Borders`]) and whether they have a
//! gap round them ([`Gaps`]). One column each,
//! side by side, a rule between every two (`chrome::divider`).
//!
//! EVERY COLUMN IS ITS OWN LIST, and the owner asked for exactly that: its own
//! title, its own cursor — kept per column, so crossing away and back finds
//! it where it was — and its own HOVER. The pointer's band covers the cells of
//! the column it is over on the row it is on, bounded by the rules on either
//! side ([`Layout::band_px`]); it never runs across the row, which is what it
//! did while the chooser was two lists drawn as one key/text pair. `<left>` and
//! `<right>` cross between the columns, `<up>` and `<down>` walk the one that
//! has the keyboard, and a title is not a row a cursor lands on.
//!
//! EVERY CURSOR PREVIEWS THE WHOLE CHOICE. A preview writes the FILE
//! (`theme::show_everywhere`), which is the channel every other door already
//! watches, so the windows, the desk and the terminals move with the footer;
//! `Esc` puts the file back — or takes it away, when there was none — and
//! `RET` applies the six answers under the six cursors. A file the person
//! wrote that does not read whole is written over by none of them, and never
//! taken away (`theme::Before`, `theme::NotWritten`): the line says why.
//!
//! # Why the widths are the longest translation's
//!
//! A column is as wide as the widest thing it may ever hold, in EVERY
//! declared language — the repository's rule that the width reserved for a
//! column is its longest translation's. Measured in the language on screen, the rules would
//! move when the person changes language, and the desk's column would move
//! when the preview crosses from a dark theme to a light one ("Black" and
//! "White" are not the same width in every language). When the footer is too
//! narrow for all six, the widest give first and their labels are elided at
//! the end ([`Layout::of`]).
//!
//! # Why the state is six cursors and a choice
//!
//! The cursors and which column has the keyboard are where the person is;
//! `committed` is what the file says. They differ while a preview is on, and
//! `Esc` is "back to committed". An external write to the file during a
//! preview moves `committed` (the door's `theme_changed` says so): what is on
//! disk wins over what is being tried.

use std::sync::OnceLock;

use ronoctua_render::layout;
use ronoctua_tokens as tokens;
use ronoctua_tokens::Color;
use ronoctua_tokens::theme::{self, Theme};

use crate::fields::{Field, Hit, Ink};
use crate::theme::{Backdrop, Before, Borders, Choice, Gaps, Ground};

/// The cell a check mark would have taken, when there is no check.
///
/// Every column carries its mark inside its TEXT, so every row reserves the
/// cell whether it is ticked or not, and so does the column's TITLE — which
/// once sat one cell left of the answers it named, and the owner read that as
/// two columns with no rule between them.
pub const BLANK_MARK: &str = " ";

/// The cell the rule between two columns sits in: a one-pixel line and one
/// cell of grid, drawn in the MIDDLE of that cell (`fields::rule_x`) — the file
/// manager's own arithmetic for the rule between its sidebar and its listing.
const RULE: u16 = 1;

/// The narrowest a column is squeezed to when the footer is too narrow for
/// all five: its mark, the air after it, and the one cell an elision leaves.
const FLOOR: u16 = 3;

/// How many columns there are.
pub const COLUMNS: usize = Column::ALL.len();

/// One of the five lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    /// The themes, on the left.
    Themes,
    /// The desk under the windows ([`Backdrop`]).
    Desk,
    /// What the footer stands on ([`Ground`]).
    Footer,
    /// What the windows stand on ([`Ground`]).
    Windows,
    /// How loud the windows' outlines are ([`Borders`]).
    Borders,
    /// Whether the windows have a gap round them ([`Gaps`]).
    Gaps,
}

impl Column {
    /// Every column, left to right.
    pub const ALL: [Self; 6] = [
        Self::Themes,
        Self::Desk,
        Self::Footer,
        Self::Windows,
        Self::Borders,
        Self::Gaps,
    ];

    /// Where it sits in [`Self::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Themes => 0,
            Self::Desk => 1,
            Self::Footer => 2,
            Self::Windows => 3,
            Self::Borders => 4,
            Self::Gaps => 5,
        }
    }

    /// The text key of its title (invariant 8).
    #[must_use]
    pub const fn title_key(self) -> &'static str {
        match self {
            Self::Themes => crate::key::THEME_OPTION_THEME,
            Self::Desk => crate::key::THEME_OPTION_BACKDROP,
            Self::Footer => crate::key::THEME_OPTION_FOOTER,
            Self::Windows => crate::key::THEME_OPTION_WINDOWS,
            Self::Borders => crate::key::THEME_OPTION_BORDERS,
            Self::Gaps => crate::key::THEME_OPTION_GAPS,
        }
    }

    /// How many answers it lists under a theme that is `light` or not: the
    /// desks a light theme offers are three (white, near-white and its own
    /// colour), a dark one's two (`Backdrop::offered`).
    #[must_use]
    pub fn answers(self, light: bool) -> usize {
        match self {
            Self::Themes => theme::all().len(),
            Self::Desk => Backdrop::offered(light).len(),
            Self::Footer | Self::Windows => Ground::ALL.len(),
            Self::Borders => Borders::ALL.len(),
            Self::Gaps => Gaps::ALL.len(),
        }
    }

    /// Which of its answers `choice` holds, in the list a theme that is
    /// `light` or not offers — the mode of the theme SHOWN, which is the list
    /// on screen: a near-white under a dark theme is on the absolute desk's
    /// row, the desk it is worn as, and the theme's own desk is the second row
    /// of a dark list and the third of a light one.
    #[must_use]
    pub fn position_in(self, choice: Choice, light: bool) -> usize {
        match self {
            Self::Themes => choice.theme.position(),
            Self::Desk => desk_row(choice.backdrop, light),
            Self::Footer => choice.footer.position(),
            Self::Windows => choice.windows.position(),
            Self::Borders => choice.borders.position(),
            Self::Gaps => choice.gaps.position(),
        }
    }

    /// The name of its answer `n`, under a theme that is `light` or not: the
    /// theme's id (an identifier, the same in every language), or the
    /// answer's text in `texts`. The absolute desk is "Black" under a dark
    /// theme and "White" under a light one (`Backdrop::text_key`).
    #[must_use]
    pub fn answer(self, n: usize, light: bool, texts: &ronoctua_i18n::Texts) -> Option<String> {
        match self {
            Self::Themes => theme::all().nth(n).map(|t| t.id().to_owned()),
            Self::Desk => Backdrop::offered(light)
                .get(n)
                .map(|b| texts.t(b.text_key(light))),
            Self::Footer | Self::Windows => Ground::ALL.get(n).map(|g| texts.t(g.text_key())),
            Self::Borders => Borders::ALL.get(n).map(|b| texts.t(b.text_key())),
            Self::Gaps => Gaps::ALL.get(n).map(|g| texts.t(g.text_key())),
        }
    }
}

/// What row `row` of a column is.
///
/// The row on screen and the answer's index in its list are not the same
/// number — every column opens with a title — and the two must not be
/// confused: a click that did not know it would choose the answer one above
/// the one under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// The column's name, over its answers. Not a place for the cursor.
    Title,
    /// One answer, by its index in the column's list.
    Answer(usize),
    /// Nothing: another column is longer than this one here.
    Empty,
}

/// What a click did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clicked {
    /// A cursor moved: the choice under the five cursors, to preview.
    Previewed(Choice),
    /// Only the keyboard moved to the column: its cursor was already there,
    /// so nothing new is previewed.
    Focused,
    /// The row the focused cursor is already on — the SECOND click, which is
    /// `RET` (invariant 19).
    Again,
    /// A title, or nothing.
    Nothing,
}

/// Where each column is drawn, in the cells of a row of FIELDS — counted
/// from `chrome::FIELDS_LEAD`, which is where a field's `at` counts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Place {
    /// The column's first cell: its mark's.
    pub from: u16,
    /// One past its last cell of text.
    pub to: u16,
    /// Its right-hand boundary: the cell the rule after it sits in. For the
    /// LAST column no rule is drawn there, but the cell still bounds its band
    /// and its clicks, with the same air every other column has before its
    /// rule.
    pub edge: u16,
}

/// The five columns laid out for a width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// One per column, in [`Column::ALL`] order.
    pub places: [Place; COLUMNS],
}

impl Layout {
    /// The layout for a footer `columns` cells wide.
    #[must_use]
    pub fn of(columns: u16) -> Self {
        Self::with(natural_widths(), columns)
    }

    /// The layout for these natural widths: each column as wide as its
    /// widest label, unless the footer is too narrow for all five — then
    /// the widest give first, down to [`FLOOR`].
    fn with(natural: [u16; COLUMNS], columns: u16) -> Self {
        // THE FILE MANAGER'S TABLE AIR, because this is a table: a column
        // begins `files.gutter` after its left boundary — the surface's lead
        // for the first, whose mark then sits where text starts in every row
        // of fields (`chrome::INSET`), and a rule for the others — and ends
        // `files.edgeRight` before its right one. So every rule has air on
        // both sides and no label or mark touches it (a glyph flush against a
        // rule reads as a glyph that was cut).
        let gutter = tokens::space::FILES_GUTTER;
        let air = tokens::space::FILES_EDGERIGHT;
        let between = air + RULE + gutter;
        // EVERYTHING THAT IS NOT TEXT: the first gutter, the gaps between
        // the columns, and the last column's air and boundary cell.
        let gaps = u16::try_from(COLUMNS - 1).unwrap_or(u16::MAX);
        let fixed = gutter + between * gaps + air + RULE;
        // THE CELLS A ROW OF FIELDS HAS, less the region's bar: a bar never
        // draws over what it scrolls.
        let room = columns
            .saturating_sub(crate::chrome::FIELDS_LEAD + crate::chrome::BAR_CELLS)
            .saturating_sub(fixed);
        let widths = fitted(natural, room);
        let mut places = [Place::default(); COLUMNS];
        let mut at = gutter;
        for (place, wide) in places.iter_mut().zip(widths) {
            let to = at + wide;
            let edge = to + air;
            *place = Place { from: at, to, edge };
            at = edge + RULE + gutter;
        }
        Self { places }
    }

    /// The rules to draw: the edge of every column but the last.
    pub fn rules(&self) -> impl Iterator<Item = u16> + '_ {
        self.places[..COLUMNS - 1].iter().map(|p| p.edge)
    }

    /// Which column owns the pixel `x` of the row — its band's: from the
    /// rule on its left (the surface's edge, for the first) up to the rule on
    /// its right, each rule's own pixel ([`Self::band_px`]). Past the last
    /// column's band there is no column.
    ///
    /// # Why pixels, and the band's own arithmetic
    ///
    /// Because a control is grabbed where it is drawn. The rule sits in the
    /// MIDDLE of its cell (`fields::rule_x`), so a decision made by cells put
    /// half of that cell on the wrong side of the line the eye sees — and it
    /// was made one cell off besides: the door hands over a cell of the ROW,
    /// already past `chrome::FIELDS_LEAD`, and the chooser took the lead off a
    /// second time. The pointer half a cell right of a rule lit and clicked
    /// the column on its left, and a cell and a half of empty footer past the
    /// last band took a click. One answer now: the band IS the hit.
    #[must_use]
    pub fn column_at_px(&self, x: f32, lead: u16, cell_px: f32) -> Option<Column> {
        Column::ALL.into_iter().find(|c| {
            let (left, right) = self.band_edges_px(*c, lead, cell_px);
            x >= left && x < right
        })
    }

    /// A column's band as its two edges, in pixels: the rule on its left (the
    /// surface's edge, for the first) and the rule on its right, each the
    /// rule's own pixel (`fields::rule_x`). The one arithmetic the band, the
    /// click and the rules share — and edges, not a width, so the pixel of a
    /// rule belongs to exactly one side of it (a width added back to the
    /// left edge lands a rounding off the rule it came from).
    fn band_edges_px(&self, column: Column, lead: u16, cell_px: f32) -> (f32, f32) {
        let i = column.index();
        let left = match i.checked_sub(1) {
            Some(before) => crate::fields::rule_x(self.places[before].edge, lead, cell_px),
            None => 0.0,
        };
        let right = crate::fields::rule_x(self.places[i].edge, lead, cell_px);
        (left, right)
    }

    /// A column's band, in pixels (`x`, `width`): from the rule on its left —
    /// or the surface's own edge, for the first — to the rule on its right,
    /// each rule's own pixel (`fields::rule_x`), so a lit row has no strip of
    /// ground between it and the line beside it. `lead` is what the door owes
    /// before a field's first cell, and `cell_px` how wide a cell is there.
    #[must_use]
    pub fn band_px(&self, column: Column, lead: u16, cell_px: f32) -> (f32, f32) {
        let (left, right) = self.band_edges_px(column, lead, cell_px);
        (left, (right - left).max(0.0))
    }
}

/// The widths the columns are drawn at: their natural ones when `room` holds
/// them all, and otherwise a common ceiling lowered until they fit — so the
/// WIDEST give first, and a narrow column keeps every cell it has — never
/// under [`FLOOR`]. Only a footer narrower than five floors and their air
/// (about thirty cells, far under any screen this system drives) would push
/// the last column past the edge.
fn fitted(natural: [u16; COLUMNS], room: u16) -> [u16; COLUMNS] {
    let needed = |cap: u16| -> u32 { natural.iter().map(|w| u32::from((*w).min(cap))).sum() };
    let mut cap = natural.iter().copied().max().unwrap_or(0);
    while cap > FLOOR && needed(cap) > u32::from(room) {
        cap -= 1;
    }
    natural.map(|w| w.min(cap))
}

/// The row of the desk column `backdrop` is shown on under a theme that is
/// `light` or not: its own, or — for a near-white under a dark theme — the
/// absolute desk's, which is what it is worn as.
fn desk_row(backdrop: Backdrop, light: bool) -> usize {
    let worn = backdrop.worn_in(light);
    Backdrop::offered(light)
        .iter()
        .position(|b| *b == worn)
        .unwrap_or(0)
}

/// `mark` and `label` as one run: the mark, a cell of air, the label.
fn marked(mark: &str, label: &str) -> String {
    format!("{mark} {label}")
}

/// Every label a column may draw in `texts`: its title and every answer, the
/// desk's in both modes.
fn labels_of(column: Column, texts: &ronoctua_i18n::Texts) -> Vec<String> {
    let mut out = vec![texts.t(column.title_key())];
    for light in [false, true] {
        for n in 0..column.answers(light) {
            out.extend(column.answer(n, light, texts));
        }
    }
    out
}

/// Each column's natural width in `every` language: the widest of its
/// labels, marked or not.
fn widths_in(every: &[ronoctua_i18n::Texts]) -> [u16; COLUMNS] {
    let tick = tokens::icon::SUCCESS.to_string();
    Column::ALL.map(|column| {
        every
            .iter()
            .flat_map(|texts| labels_of(column, texts))
            .map(|label| {
                layout::width(&marked(BLANK_MARK, &label))
                    .max(layout::width(&marked(&tick, &label)))
            })
            .max()
            .unwrap_or(0)
    })
}

/// [`widths_in`] over every declared language, measured once per process:
/// the labels are the build's, and no language or theme changes them.
fn natural_widths() -> [u16; COLUMNS] {
    static WIDTHS: OnceLock<[u16; COLUMNS]> = OnceLock::new();
    *WIDTHS.get_or_init(|| {
        let every: Vec<ronoctua_i18n::Texts> = ronoctua_i18n::LANGUAGES
            .iter()
            .filter_map(|language| ronoctua_i18n::Texts::fresh(language))
            .collect();
        widths_in(&every)
    })
}

/// The chooser's state while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    /// Which list has the keyboard.
    column: Column,
    /// Each column's cursor: an index into its list, in [`Column::ALL`]
    /// order. Kept per column, so crossing away and back finds it where it
    /// was. The DESK's is not read from here: it is [`Self::desk`], whose row
    /// depends on the theme shown.
    cursors: [usize; COLUMNS],
    /// The desk under its cursor, as an ANSWER and not as a row: a light theme
    /// lists three desks and a dark one two, so a row number would change
    /// meaning under a moving theme cursor — the near-white asked for under a
    /// light theme is kept when the cursor crosses a dark one (it is shown on
    /// the absolute desk's row there, the desk it is worn as) and comes back
    /// with the next light theme.
    desk: Backdrop,
    /// What the file says — what `Esc` returns to.
    committed: Choice,
    /// What the file was when this opened: none, a choice, or a file that
    /// does not read whole (`theme::Before`).
    ///
    /// A preview WRITES the file — that is how every other door hears about
    /// it — so by the time `Esc` runs, "does the file exist" can no longer
    /// answer "did the person ever choose". It is answered here, once, before
    /// the first preview: a machine that never chose must not be given a file
    /// by having looked at the list, and a file the person wrote and this
    /// system could not read must be neither written over nor taken away.
    before: Before,
    /// The choice this door last SHOWED, while a preview is on.
    ///
    /// IT EXISTS BECAUSE A PREVIEW WRITES THE FILE, and this door watches that
    /// file like every other. Its own write came back to it as a change and
    /// `theme_changed` moved `committed` to it — so after one press of an
    /// arrow there was nothing left to go back to, and `Esc` became a key that
    /// did nothing. Measured in `.#shots`: the file said
    /// `{"backdrop":"black","theme":"midnight"}` before `Esc` and the same
    /// after it, with the key arriving in the journal.
    ///
    /// A door cannot tell its own write from anybody else's by looking at the
    /// file, so it remembers. Anything the file says that is NOT this is
    /// somebody else's choice and still wins, which is the promise the module
    /// header makes.
    previewed: Option<Choice>,
}

impl View {
    /// Opens on what the file says (`theme::committed`), every cursor on the
    /// committed answer of its column. The keyboard starts on the themes.
    #[must_use]
    pub fn fresh_one(committed: Choice, before: Before) -> Self {
        Self {
            column: Column::Themes,
            cursors: Column::ALL.map(|c| c.position_in(committed, committed.theme.is_light())),
            desk: committed.backdrop,
            committed,
            before,
            previewed: None,
        }
    }

    /// What the file was when this opened. See the field.
    #[must_use]
    pub const fn before(&self) -> Before {
        self.before
    }

    /// How many rows the region shows: a title and the longest list in
    /// either mode — the region does not change height under a moving theme
    /// cursor.
    #[must_use]
    pub fn rows() -> usize {
        1 + Column::ALL
            .iter()
            .flat_map(|c| [c.answers(false), c.answers(true)])
            .max()
            .unwrap_or(0)
    }

    /// What row `row` of `column` is, under a theme that is `light` or not.
    #[must_use]
    pub fn slot(column: Column, row: usize, light: bool) -> Slot {
        match row {
            0 => Slot::Title,
            n if n <= column.answers(light) => Slot::Answer(n - 1),
            _ => Slot::Empty,
        }
    }

    /// Whether the theme SHOWN — the one under the theme cursor, which every
    /// cursor previews — is a light one: what the desk column lists.
    #[must_use]
    pub fn shows_light(&self) -> bool {
        self.focused().is_light()
    }

    /// Which list has the keyboard — asked by the tests; the door asks what
    /// it needs through [`Self::lines`] and [`Self::row`].
    #[cfg(test)]
    #[must_use]
    pub const fn column(&self) -> Column {
        self.column
    }

    /// The cursor of `column`: an index into its list — the desk's, into
    /// what the theme shown offers.
    #[must_use]
    pub fn cursor(&self, column: Column) -> usize {
        match column {
            Column::Desk => desk_row(self.desk, self.shows_light()),
            _ => self.cursors[column.index()],
        }
    }

    /// The row on screen the focused cursor is on — what the region scrolls
    /// to. Every column's answers start under its title.
    #[must_use]
    pub fn row(&self) -> usize {
        1 + self.cursor(self.column)
    }

    /// Puts `column`'s cursor on answer `n` of its list.
    fn put(&mut self, column: Column, n: usize) {
        match column {
            Column::Desk => {
                if let Some(b) = Backdrop::offered(self.shows_light()).get(n) {
                    self.desk = *b;
                }
            }
            _ => self.cursors[column.index()] = n,
        }
    }

    /// The theme under its cursor.
    #[must_use]
    pub fn focused(&self) -> Theme {
        theme::all()
            .nth(self.cursor(Column::Themes))
            .unwrap_or_else(theme::default_one)
    }

    /// The five answers under the five cursors — what a preview shows and
    /// `RET` writes.
    #[must_use]
    pub fn focused_choice(&self) -> Choice {
        Choice {
            theme: self.focused(),
            backdrop: self.desk,
            footer: Ground::ALL
                .get(self.cursor(Column::Footer))
                .copied()
                .unwrap_or_default(),
            windows: Ground::ALL
                .get(self.cursor(Column::Windows))
                .copied()
                .unwrap_or_default(),
            borders: Borders::ALL
                .get(self.cursor(Column::Borders))
                .copied()
                .unwrap_or_default(),
            gaps: Gaps::ALL
                .get(self.cursor(Column::Gaps))
                .copied()
                .unwrap_or_default(),
        }
    }

    /// The choice the file says.
    #[must_use]
    pub const fn committed(&self) -> Choice {
        self.committed
    }

    /// Moves the focused column's cursor without leaving the ends, and says
    /// the choice under the cursors when it moved — `None` at an end, where
    /// there is nothing new to preview. It does not wrap, for the reason the
    /// network list gives: going back to the start after the end makes the
    /// eye lose its place.
    ///
    /// AN ARROW THAT DID NOT MOVE THE CURSOR CHANGES NO ANSWER. The desk is
    /// kept as an answer (the `desk` field): a near-white asked for under a
    /// light theme sits on the absolute desk's row under a dark one, and
    /// writing that row back at the top of the list would turn it into the
    /// absolute desk with nothing on screen moving — the next light theme
    /// would then show white where the person had chosen near-white.
    pub fn move_by(&mut self, step: i32) -> Option<Choice> {
        let column = self.column;
        let was = self.cursor(column);
        let to = clamped(was, step, column.answers(self.shows_light()));
        if to == was {
            return None;
        }
        self.put(column, to);
        Some(self.focused_choice())
    }

    /// Crosses to the next column — `<left>` and `<right>` — and says which
    /// has the keyboard now. At an end the arrow that points outward does
    /// nothing, as in the file manager's sections.
    pub fn cross(&mut self, step: i32) -> Column {
        self.column = Column::ALL[clamped(self.column.index(), step.signum(), COLUMNS)];
        self.column
    }

    /// The pointer pressed row `row` of `column`. See [`Clicked`].
    pub fn clicked(&mut self, column: Column, row: usize) -> Clicked {
        let Slot::Answer(n) = Self::slot(column, row, self.shows_light()) else {
            return Clicked::Nothing;
        };
        if self.column == column && self.cursor(column) == n {
            return Clicked::Again;
        }
        self.column = column;
        // THE CURSOR'S OWN ROW CHANGES NO ANSWER, for the reason `move_by`
        // gives: a click that only brings the keyboard to a column must not
        // turn a kept near-white into the absolute desk its row shows — or
        // the pointer would commit one answer where `<right>`, `RET` commits
        // another (invariant 19).
        if self.cursor(column) == n {
            return Clicked::Focused;
        }
        self.put(column, n);
        Clicked::Previewed(self.focused_choice())
    }

    /// The rows of the region, one per line, each the fields of the columns
    /// that have something on it, laid out for a footer `columns` wide.
    ///
    /// The desk's absolute answer is named by the mode of the theme under the
    /// THEME cursor — the one being shown, since every cursor previews.
    #[must_use]
    pub fn lines(&self, texts: &ronoctua_i18n::Texts, columns: u16) -> Vec<Vec<Field>> {
        let layout = Layout::of(columns);
        let light = self.focused().is_light();
        (0..Self::rows())
            .map(|row| {
                Column::ALL
                    .iter()
                    .zip(layout.places)
                    .filter_map(|(column, place)| {
                        let (text, color) = match Self::slot(*column, row, light) {
                            // A TITLE IN THE COLOUR EVERY OTHER LIST'S TITLE
                            // HAS, over the NAMES of what its list holds: it
                            // pays the mark's cell too.
                            Slot::Title => (
                                marked(BLANK_MARK, &texts.t(column.title_key())),
                                tokens::color::TEXT_HEADING,
                            ),
                            Slot::Answer(n) => {
                                // THE CHECK ON WHAT THE FILE SAYS, in every
                                // column: `RET` applies what is under the
                                // cursors, and the check says what that
                                // would replace.
                                // THE ROW OF THE LIST ON SCREEN: the list is
                                // the SHOWN theme's, so the committed answer
                                // is placed in it by that theme's mode, never
                                // by the committed theme's.
                                let mark = if column.position_in(self.committed, light) == n {
                                    tokens::icon::SUCCESS.to_string()
                                } else {
                                    BLANK_MARK.to_owned()
                                };
                                (
                                    marked(&mark, &column.answer(n, light, texts)?),
                                    self.colour_of(*column, n),
                                )
                            }
                            Slot::Empty => return None,
                        };
                        Some(Field {
                            ink: Ink::Grid,
                            at: place.from,
                            text: layout::elide_end(&text, place.to.saturating_sub(place.from)),
                            color,
                            hit: Hit::Nothing,
                            explain: None,
                        })
                    })
                    .collect()
            })
            .collect()
    }

    /// The colour of answer `n` of `column`: THE CURSOR IS A COLOUR. The
    /// accent on the cursor of the column that has the keyboard; the primary
    /// text on every other column's cursor — `RET` applies the answers under
    /// ALL five, so all five have to be readable, the manager's precedent
    /// where a section that lost the focus keeps a quiet mark — and the
    /// secondary text on the rest.
    fn colour_of(&self, column: Column, n: usize) -> Color {
        if self.cursor(column) != n {
            tokens::color::TEXT_SECONDARY
        } else if column == self.column {
            tokens::color::STATE_ACCENT
        } else {
            tokens::color::TEXT_PRIMARY
        }
    }

    /// The hover band of the pointer at pixel `x` of row `row`, in pixels
    /// (`x`, `width`): its column's, and `None` where that column has no
    /// answer on this row — a band over a title or over nothing would promise
    /// a click that does nothing. Under a theme that is `light` or not: the
    /// desk column lists three answers in one and two in the other.
    #[must_use]
    pub fn band_px(
        columns: u16,
        row: usize,
        x: f32,
        cell_px: f32,
        light: bool,
    ) -> Option<(f32, f32)> {
        let lead = crate::chrome::FIELDS_LEAD;
        let layout = Layout::of(columns);
        let column = layout.column_at_px(x, lead, cell_px)?;
        matches!(Self::slot(column, row, light), Slot::Answer(_))
            .then(|| layout.band_px(column, lead, cell_px))
    }

    /// The column under pixel `x` of a row — what a click asks, and what
    /// tells two pointers on one row apart: the same
    /// [`Layout::column_at_px`] the band asks.
    #[must_use]
    pub fn column_under(columns: u16, x: f32, cell_px: f32) -> Option<Column> {
        Layout::of(columns).column_at_px(x, crate::chrome::FIELDS_LEAD, cell_px)
    }

    /// This door has just SHOWN a choice everywhere, which wrote the file.
    pub const fn previewing(&mut self, c: Choice) {
        self.previewed = Some(c);
    }

    /// What this door last showed, if a preview is on.
    #[must_use]
    pub const fn previewed(&self) -> Option<Choice> {
        self.previewed
    }

    /// The choice under the cursors becomes the committed one: `theme-apply`
    /// ran and the file is written.
    pub fn commit(&mut self) {
        self.committed = self.focused_choice();
        // THE PREVIEW IS OVER: what the file says is now a choice, and the
        // event this write raises has to be allowed to move `committed` —
        // it already holds the same choice, so it changes nothing, and
        // leaving the mark set would make the NEXT external write be ignored.
        self.previewed = None;
    }

    /// The file changed under the chooser: what is on disk is now the thing
    /// `Esc` returns to.
    pub fn committed_became(&mut self, c: Choice) {
        self.committed = c;
    }

    /// The hint line: the key that applies and the arrows that cross between
    /// the columns, in the localiser's sentence, with the keys marked so the
    /// chrome colours them.
    ///
    /// NOT THE WAY BACK. Every view's line starts with `Esc`, clickable, put
    /// there by the footer (`chrome::with_the_way_back`); a second, inert
    /// `Esc` at the end of this sentence drew the key twice on one line.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        use ronoctua_keymap::{Key, Sym};
        let apply = key_that_runs("theme-apply").map_or_else(String::new, |k| k.notation());
        let left = Key::simple(Sym::Left).notation();
        let right = Key::simple(Sym::Right).notation();
        // ONE ARGUMENT PER KEY: the chrome marks a piece as a key only when the
        // whole piece is a notation, so `<left>/<right>` as one argument came
        // out in the prose register. The slash is the sentence's.
        let marked = texts.t_marked(
            crate::key::THEME_HINT,
            &[
                ("apply", apply.as_str()),
                ("left", left.as_str()),
                ("right", right.as_str()),
            ],
        );
        let mut out = Vec::new();
        crate::chrome::Piece::extend(
            &mut out,
            &marked,
            &[apply.as_str(), left.as_str(), right.as_str()],
        );
        out
    }
}

/// `at + step`, held inside `0..len`.
fn clamped(at: usize, step: i32, len: usize) -> usize {
    let last = i64::try_from(len.saturating_sub(1)).unwrap_or(0);
    let fresh = i64::try_from(at).unwrap_or(0) + i64::from(step);
    usize::try_from(fresh.clamp(0, last)).unwrap_or(0)
}

/// The KEY that runs a given id, read out of the view's own table — the
/// reason `files::key_that_runs` gives: the hint and the mouse read the table
/// rather than writing a key out by hand.
#[must_use]
pub fn key_that_runs(id: &str) -> Option<ronoctua_keymap::Key> {
    crate::leader::OF_THEME_VIEW
        .iter()
        .find(|(_, command)| *command == id)
        .and_then(|(notation, _)| ronoctua_keymap::Key::parse(notation).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed choice a chooser opens on: the theme the process is in,
    /// on `backdrop`, every other answer at its default.
    fn on(backdrop: Backdrop) -> Choice {
        Choice {
            backdrop,
            ..Choice::with_theme(theme::current())
        }
    }

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    /// A footer wide enough for every column at its natural width.
    const WIDE: u16 = 240;

    #[test]
    fn it_opens_on_the_committed_choice_with_every_cursor_on_its_answer() {
        let committed = Choice {
            backdrop: Backdrop::Absolute,
            footer: Ground::Desk,
            windows: Ground::Theme,
            borders: Borders::Discreet,
            ..Choice::with_theme(theme::current())
        };
        let v = View::fresh_one(committed, Before::Chosen);
        assert_eq!(v.column(), Column::Themes);
        assert_eq!(v.focused_choice(), committed);
        for c in Column::ALL {
            assert_eq!(
                v.cursor(c),
                c.position_in(committed, committed.theme.is_light()),
                "{c:?}"
            );
        }
        assert_eq!(v.row(), 1 + v.cursor(Column::Themes));
    }

    #[test]
    fn every_column_opens_with_a_title_and_no_title_takes_a_cursor() {
        assert_eq!(
            View::rows(),
            1 + Column::ALL.iter().map(|c| c.answers(false)).max().unwrap()
        );
        for c in Column::ALL {
            assert_eq!(View::slot(c, 0, false), Slot::Title, "{c:?}");
            for n in 0..c.answers(false) {
                assert_eq!(View::slot(c, n + 1, false), Slot::Answer(n), "{c:?}");
            }
            assert_eq!(
                View::slot(c, c.answers(false) + 1, false),
                Slot::Empty,
                "{c:?}"
            );
            let mut v = View::fresh_one(on(Backdrop::Absolute), Before::Chosen);
            let before = v.clone();
            assert_eq!(v.clicked(c, 0), Clicked::Nothing, "{c:?}: the title");
            assert_eq!(
                v.clicked(c, c.answers(false) + 1),
                Clicked::Nothing,
                "{c:?}: nothing"
            );
            assert_eq!(v, before, "{c:?}: a dead click moved something");
        }
    }

    #[test]
    fn the_arrows_cross_between_the_six_columns_and_walk_the_focused_one() {
        let mut v = View::fresh_one(on(Backdrop::Absolute), Before::Chosen);
        // ACROSS, and never past either end.
        assert_eq!(v.cross(-1), Column::Themes);
        for c in &Column::ALL[1..] {
            assert_eq!(v.cross(1), *c);
        }
        assert_eq!(v.cross(1), Column::Gaps, "no seventh column");
        for c in Column::ALL[..COLUMNS - 1].iter().rev() {
            assert_eq!(v.cross(-1), *c);
        }
        // WITHIN: only the focused column's cursor moves, and every other
        // column keeps its own.
        for c in Column::ALL {
            let mut v = View::fresh_one(on(Backdrop::Absolute), Before::Chosen);
            while v.column() != c {
                v.cross(1);
            }
            let others: Vec<usize> = Column::ALL
                .iter()
                .filter(|o| **o != c)
                .map(|o| v.cursor(*o))
                .collect();
            for _ in 0..20 {
                v.move_by(1);
            }
            assert_eq!(v.cursor(c), c.answers(false) - 1, "{c:?}: the last answer");
            assert_eq!(v.move_by(1), None, "{c:?}: the end previews nothing new");
            for _ in 0..20 {
                v.move_by(-1);
            }
            assert_eq!(v.cursor(c), 0, "{c:?}: the first answer");
            assert_eq!(v.move_by(-1), None);
            let still: Vec<usize> = Column::ALL
                .iter()
                .filter(|o| **o != c)
                .map(|o| v.cursor(*o))
                .collect();
            assert_eq!(still, others, "{c:?} moved another column's cursor");
            assert_eq!(v.row(), 1, "{c:?}: the region scrolls to its cursor");
        }
        // AND A CURSOR IS KEPT PER COLUMN: away and back finds it.
        let mut v = View::fresh_one(on(Backdrop::Absolute), Before::Chosen);
        v.cross(1);
        v.move_by(1);
        let desk = v.cursor(Column::Desk);
        v.cross(1);
        v.cross(1);
        v.cross(-1);
        v.cross(-1);
        assert_eq!(v.column(), Column::Desk);
        assert_eq!(v.cursor(Column::Desk), desk);
    }

    #[test]
    fn every_cursor_previews_the_whole_choice() {
        // THE DEFECT THE OWNER READ OFF THE SCREEN, still guarded: a preview
        // that carried only the answer that moved would show a theme worn by
        // one strip of the screen. Every move answers all five.
        let mut v = View::fresh_one(on(Backdrop::Absolute), Before::Chosen);
        let mut expected = v.focused_choice();
        v.cross(1);
        expected.backdrop = Backdrop::Theme;
        assert_eq!(v.move_by(1), Some(expected));
        v.cross(1);
        expected.footer = Ground::Desk;
        assert_eq!(v.move_by(1), Some(expected));
        v.cross(1);
        expected.windows = Ground::Desk;
        assert_eq!(v.move_by(1), Some(expected));
        v.cross(1);
        expected.borders = Borders::Discreet;
        assert_eq!(v.move_by(1), Some(expected));
        v.cross(1);
        expected.gaps = Gaps::None;
        assert_eq!(v.move_by(1), Some(expected));
        // THE THEME COLUMN CARRIES THE OTHER FIVE.
        while v.column() != Column::Themes {
            v.cross(-1);
        }
        let moved = v
            .move_by(1)
            .or_else(|| v.move_by(-1))
            .expect("more than one theme");
        assert_eq!(
            (
                moved.backdrop,
                moved.footer,
                moved.windows,
                moved.borders,
                moved.gaps
            ),
            (
                Backdrop::Theme,
                Ground::Desk,
                Ground::Desk,
                Borders::Discreet,
                Gaps::None
            )
        );
        assert_eq!(moved, v.focused_choice());
        // AND IT IS STILL A PREVIEW: nothing moved what `Esc` goes back to.
        assert_eq!(v.committed(), on(Backdrop::Absolute));
    }

    #[test]
    fn a_click_selects_in_its_own_column_and_the_second_is_return() {
        for c in Column::ALL {
            let mut v = View::fresh_one(on(Backdrop::Absolute), Before::Chosen);
            // AN ANSWER THAT IS NOT THE CURSOR: the cursor goes there, the
            // keyboard goes to that column, and the choice is previewed.
            let other = (v.cursor(c) + 1) % c.answers(false);
            let Clicked::Previewed(shown) = v.clicked(c, 1 + other) else {
                panic!("{c:?}: a click on another answer previews it");
            };
            assert_eq!(v.column(), c);
            assert_eq!(v.cursor(c), other);
            assert_eq!(shown, v.focused_choice());
            // THE SAME ROW AGAIN is `RET`.
            assert_eq!(v.clicked(c, 1 + other), Clicked::Again, "{c:?}");
            assert_eq!(v.cursor(c), other);
        }
        // THE CURSOR'S OWN ROW in a column without the keyboard only brings
        // the keyboard: nothing new to preview, and not yet `RET`.
        let mut v = View::fresh_one(on(Backdrop::Absolute), Before::Chosen);
        let at = v.cursor(Column::Borders);
        assert_eq!(v.clicked(Column::Borders, 1 + at), Clicked::Focused);
        assert_eq!(v.column(), Column::Borders);
        assert_eq!(v.clicked(Column::Borders, 1 + at), Clicked::Again);
    }

    #[test]
    fn a_light_theme_lists_three_desks_a_dark_one_two_and_the_near_white_is_kept() {
        let dark = theme::all().find(|t| !t.is_light()).expect("a dark theme");
        let light = theme::all().find(|t| t.is_light()).expect("a light theme");
        let t = texts("pt-BR");
        let desk_labels = |v: &View| -> Vec<String> {
            let desk = Layout::of(WIDE).places[Column::Desk.index()];
            v.lines(&t, WIDE)
                .iter()
                .skip(1)
                .flatten()
                .filter(|f| f.at == desk.from)
                .map(|f| f.text.clone())
                .collect()
        };
        // UNDER A LIGHT THEME: white, near-white and the theme's own colour.
        let mut v = View::fresh_one(Choice::with_theme(light), Before::Chosen);
        assert_eq!(desk_labels(&v).len(), 3, "{:?}", desk_labels(&v));
        assert!(desk_labels(&v)[1].ends_with("Quase branco"));
        v.cross(1);
        assert_eq!(v.column(), Column::Desk);
        // Walk to the near-white.
        while v.focused_choice().backdrop != Backdrop::Near {
            assert!(v.move_by(-1).is_some(), "the near-white is not reachable");
        }
        // UNDER A DARK THEME: two, and the near-white is worn as the black —
        // shown on its row — but KEPT, so a light theme brings it back.
        v.cross(-1);
        v.clicked(Column::Themes, 1 + dark.position());
        assert_eq!(v.focused(), dark);
        assert_eq!(desk_labels(&v).len(), 2, "{:?}", desk_labels(&v));
        assert_eq!(v.focused_choice().backdrop, Backdrop::Near);
        assert_eq!(
            v.cursor(Column::Desk),
            0,
            "the near-white is shown on the absolute row"
        );
        v.clicked(Column::Themes, 1 + light.position());
        assert_eq!(v.focused_choice().backdrop, Backdrop::Near);
        assert_eq!(v.cursor(Column::Desk), 1);
        // AND A CLICK ON A ROW A DARK THEME DOES NOT HAVE is nothing.
        v.clicked(Column::Themes, 1 + dark.position());
        assert_eq!(v.clicked(Column::Desk, 3), Clicked::Nothing);
    }

    #[test]
    fn the_desk_is_black_under_a_dark_theme_and_white_under_a_light_one() {
        let dark = theme::all().find(|t| !t.is_light()).expect("a dark theme");
        let light = theme::all().find(|t| t.is_light()).expect("a light theme");
        for (language, black, white) in [("en-GB", "Black", "White"), ("pt-BR", "Preto", "Branco")]
        {
            let t = texts(language);
            for (shown, word, not) in [(dark, black, white), (light, white, black)] {
                // THE THEME UNDER THE CURSOR names it — the one being shown.
                let mut v =
                    View::fresh_one(Choice::with_theme(theme::default_one()), Before::Chosen);
                v.clicked(Column::Themes, 1 + shown.position());
                assert_eq!(v.focused(), shown);
                let lines = v.lines(&t, WIDE);
                let desk = Layout::of(WIDE).places[Column::Desk.index()];
                let said: Vec<&str> = lines
                    .iter()
                    .flatten()
                    .filter(|f| f.at == desk.from)
                    .map(|f| f.text.as_str())
                    .collect();
                assert!(
                    said.iter().any(|s| s.ends_with(word)),
                    "{language}, {}: {said:?}",
                    shown.id()
                );
                assert!(
                    !said.iter().any(|s| s.ends_with(not)),
                    "{language}, {}: {said:?}",
                    shown.id()
                );
            }
        }
    }

    #[test]
    fn every_row_draws_each_column_in_its_own_cells_with_its_title_first() {
        let t = texts("pt-BR");
        let v = View::fresh_one(on(Backdrop::Absolute), Before::Chosen);
        let layout = Layout::of(WIDE);
        let lines = v.lines(&t, WIDE);
        assert_eq!(lines.len(), View::rows());
        for (row, fields) in lines.iter().enumerate() {
            for c in Column::ALL {
                let p = layout.places[c.index()];
                let here: Vec<&Field> = fields.iter().filter(|f| f.at == p.from).collect();
                match View::slot(c, row, false) {
                    Slot::Empty => assert!(here.is_empty(), "{c:?} row {row}"),
                    slot => {
                        assert_eq!(here.len(), 1, "{c:?} row {row}");
                        let f = here[0];
                        assert!(
                            f.at + layout::width(&f.text) <= p.to,
                            "{c:?} row {row}: {:?} runs past its column",
                            f.text
                        );
                        if slot == Slot::Title {
                            assert_eq!(f.color, tokens::color::TEXT_HEADING);
                            assert_eq!(f.text, marked(BLANK_MARK, &t.t(c.title_key())));
                        }
                    }
                }
            }
        }
        // THE CURSOR IS A COLOUR: the accent on the focused column's cursor,
        // the primary text on every other column's, and a check on what the
        // file says.
        let colour_at = |c: Column, row: usize| {
            let p = layout.places[c.index()];
            lines[row].iter().find(|f| f.at == p.from).map(|f| f.color)
        };
        assert_eq!(
            colour_at(Column::Themes, v.row()),
            Some(tokens::color::STATE_ACCENT)
        );
        assert_eq!(
            colour_at(Column::Desk, 1 + v.cursor(Column::Desk)),
            Some(tokens::color::TEXT_PRIMARY)
        );
        let desk = layout.places[Column::Desk.index()];
        let ticked = lines[1 + Backdrop::Absolute.position()]
            .iter()
            .find(|f| f.at == desk.from)
            .expect("the absolute desk's row");
        assert!(
            ticked.text.starts_with(tokens::icon::SUCCESS),
            "{:?}",
            ticked.text
        );
    }

    #[test]
    fn each_column_is_as_wide_as_its_widest_label_in_the_longest_translation() {
        let every: Vec<ronoctua_i18n::Texts> =
            ronoctua_i18n::LANGUAGES.iter().map(|l| texts(l)).collect();
        let widths = widths_in(&every);
        for c in Column::ALL {
            let widest = every
                .iter()
                .flat_map(|t| labels_of(c, t))
                .map(|l| layout::width(&marked(BLANK_MARK, &l)))
                .max()
                .unwrap();
            assert_eq!(widths[c.index()], widest, "{c:?}");
        }
        // AND THE LAYOUT GIVES EACH EXACTLY THAT, when there is room.
        let layout = Layout::of(WIDE);
        for c in Column::ALL {
            let p = layout.places[c.index()];
            assert_eq!(p.to - p.from, widths[c.index()], "{c:?}");
        }
        // THE COLUMNS ARE IN ORDER and never touch: a rule's cell between
        // every two, with air on both sides of it.
        for pair in layout.places.windows(2) {
            assert!(pair[0].to < pair[0].edge, "{layout:?}");
            assert!(pair[0].edge < pair[1].from, "{layout:?}");
        }
        assert_eq!(layout.rules().count(), COLUMNS - 1);
        // THE FIRST COLUMN STARTS WHERE TEXT STARTS in every row of fields.
        assert_eq!(
            crate::chrome::FIELDS_LEAD + layout.places[0].from,
            crate::chrome::INSET
        );
    }

    #[test]
    fn a_narrow_footer_elides_the_widest_first_and_nothing_leaks() {
        let wide = Layout::of(WIDE);
        let natural: Vec<u16> = wide.places.iter().map(|p| p.to - p.from).collect();
        let needed = wide.places[COLUMNS - 1].edge + RULE;
        let narrow = crate::chrome::FIELDS_LEAD + crate::chrome::BAR_CELLS + needed - 8;
        let layout = Layout::of(narrow);
        let widths: Vec<u16> = layout.places.iter().map(|p| p.to - p.from).collect();
        assert!(
            crate::chrome::FIELDS_LEAD + layout.places[COLUMNS - 1].edge + RULE
                <= narrow - crate::chrome::BAR_CELLS,
            "{layout:?} leaks past {narrow}"
        );
        assert!(widths.iter().zip(&natural).any(|(w, n)| w < n));
        // THE WIDEST GAVE: a column that kept its natural width is no wider
        // than one that was cut.
        let cap = widths.iter().copied().max().unwrap();
        for (w, n) in widths.iter().zip(&natural) {
            assert_eq!(*w, (*n).min(cap));
        }
        // AND WHAT IS DRAWN IS ELIDED INTO ITS CELLS.
        let t = texts("en-GB");
        let v = View::fresh_one(on(Backdrop::Absolute), Before::Chosen);
        for fields in v.lines(&t, narrow) {
            for f in fields {
                let p = layout
                    .places
                    .iter()
                    .find(|p| p.from == f.at)
                    .expect("a field starts at a column");
                assert!(f.at + layout::width(&f.text) <= p.to, "{:?}", f.text);
            }
        }
    }

    #[test]
    fn the_hover_band_is_its_columns_own_and_never_the_row() {
        let lead = crate::chrome::FIELDS_LEAD;
        let cell = 10.0_f32;
        let layout = Layout::of(WIDE);
        let row_px = f32::from(WIDE) * cell;
        let mut previous_end = 0.0_f32;
        for c in Column::ALL {
            let p = layout.places[c.index()];
            let (x, width) = layout.band_px(c, lead, cell);
            // BOUNDED BY THE RULES' OWN PIXELS on either side.
            if c.index() == 0 {
                assert!(x.abs() < f32::EPSILON, "the first band starts at the edge");
            } else {
                let rule = layout.places[c.index() - 1].edge;
                assert!((x - crate::fields::rule_x(rule, lead, cell)).abs() < f32::EPSILON);
            }
            assert!(
                (x + width - crate::fields::rule_x(p.edge, lead, cell)).abs() < 0.01,
                "{c:?}"
            );
            // IT COVERS ITS TEXT and never the row.
            assert!(x <= f32::from(lead + p.from) * cell, "{c:?}");
            assert!(x + width >= f32::from(lead + p.to) * cell, "{c:?}");
            assert!(width < row_px / 2.0, "{c:?}: a band across the row");
            // AND THE BANDS DO NOT OVERLAP.
            assert!(x >= previous_end - 0.01, "{c:?}");
            previous_end = x + width;
            // THROUGH THE DOOR'S QUESTION: a row with an answer bands the
            // column, a title or an empty cell bands nothing.
            let middle = x + width / 2.0;
            assert_eq!(
                View::band_px(WIDE, 1, middle, cell, false),
                Some((x, width))
            );
            assert_eq!(View::column_under(WIDE, middle, cell), Some(c));
            assert_eq!(
                View::band_px(WIDE, 0, middle, cell, false),
                None,
                "{c:?} title"
            );
            assert_eq!(
                View::band_px(WIDE, c.answers(false) + 1, middle, cell, false),
                None,
                "{c:?} past its list"
            );
        }
        // AND THE LEAD IS THE FIRST COLUMN'S, as the file manager's sidebar
        // band runs from the screen's edge.
        assert_eq!(View::column_under(WIDE, 0.0, cell), Some(Column::Themes));
    }

    #[test]
    fn the_click_and_the_band_take_exactly_the_column_drawn_between_its_rules() {
        // THE REPORT: the pointer half a cell to the RIGHT of a drawn rule
        // lit — and a click there chose in — the column on its LEFT, and a
        // cell and a half of empty footer past the last column took a click
        // for the borders. The door hands over pixels; the lead came off
        // twice and the rule's half cell fell on the wrong side.
        //
        // So: walk the row a quarter of a pixel at a time, as the pointer
        // does, and ask the door's two questions — which column a click
        // takes, and which band lights. Each column must answer on EXACTLY
        // the span between the rules the footer draws (`Layout::rules`, at
        // `fields::rule_x`), from the surface's edge for the first, and to
        // the rule's pixel after the last. Nothing past it.
        let lead = crate::chrome::FIELDS_LEAD;
        for cell in [10.0_f32, 9.6, 11.4] {
            let layout = Layout::of(WIDE);
            let rules: Vec<f32> = layout
                .rules()
                .map(|at| crate::fields::rule_x(at, lead, cell))
                .collect();
            let last_edge = crate::fields::rule_x(layout.places[COLUMNS - 1].edge, lead, cell);
            let drawn = |c: Column| -> (f32, f32) {
                let i = c.index();
                let from = if i == 0 { 0.0 } else { rules[i - 1] };
                let to = if i == COLUMNS - 1 {
                    last_edge
                } else {
                    rules[i]
                };
                (from, to)
            };
            let row_px = f32::from(WIDE) * cell;
            let step = 0.25_f32;
            let mut x = 0.0_f32;
            while x < row_px {
                let clicked = View::column_under(WIDE, x, cell);
                let expected = Column::ALL.into_iter().find(|c| {
                    let (from, to) = drawn(*c);
                    x >= from && x < to
                });
                assert_eq!(clicked, expected, "cell {cell}: the click at x {x}");
                // AND THE BAND THAT LIGHTS IS THE ONE THE CLICK TAKES, on a
                // row every column has an answer on.
                let lit = View::band_px(WIDE, 1, x, cell, false);
                match expected {
                    Some(c) => {
                        let (from, to) = drawn(c);
                        let (bx, bw) = lit.unwrap_or_else(|| panic!("nothing lit at x {x}"));
                        assert!(
                            (bx - from).abs() < 0.01,
                            "cell {cell}, {c:?}: band from {bx}, drawn {from}"
                        );
                        assert!(
                            (bx + bw - to).abs() < 0.01,
                            "cell {cell}, {c:?}: band to {}, drawn {to}",
                            bx + bw
                        );
                    }
                    None => assert_eq!(
                        lit, None,
                        "cell {cell}: a band past the last column at x {x}"
                    ),
                }
                x += step;
            }
            // THE EXACT PIXELS EITHER SIDE OF EVERY RULE.
            for (i, rule) in rules.iter().enumerate() {
                assert_eq!(
                    View::column_under(WIDE, *rule - 0.01, cell),
                    Some(Column::ALL[i])
                );
                assert_eq!(
                    View::column_under(WIDE, *rule, cell),
                    Some(Column::ALL[i + 1])
                );
            }
            assert_eq!(View::column_under(WIDE, last_edge, cell), None);
        }
    }

    #[test]
    fn two_pointers_on_one_row_in_two_columns_are_two_bands() {
        // EACH LIST ITS OWN HOVER, for every hand: two seats, one row, two
        // columns — the second band was dropped because the footer kept one
        // band per ROW. The pure half: the two pointers name two columns, and
        // the footer keeps one band per (row, column) — `seats::first_of_each`.
        let cell = 10.0_f32;
        let layout = Layout::of(WIDE);
        let x_of = |c: Column| {
            let (x, w) = layout.band_px(c, crate::chrome::FIELDS_LEAD, cell);
            x + w / 2.0
        };
        let a = View::column_under(WIDE, x_of(Column::Themes), cell);
        let b = View::column_under(WIDE, x_of(Column::Borders), cell);
        assert_ne!(a, b, "two columns of one row share a band");
        assert_eq!(
            a,
            View::column_under(WIDE, x_of(Column::Themes) + 1.0, cell)
        );
        let kept = crate::seats::first_of_each([((3, a), "A"), ((3, b), "B"), ((3, a), "C")]);
        assert_eq!(
            kept.iter().map(|(_, who)| *who).collect::<Vec<_>>(),
            ["A", "B"],
            "a pointer on another column's cell of the same row lit nothing"
        );
    }

    #[test]
    fn the_check_is_on_the_committed_answer_in_the_list_of_the_theme_shown() {
        // THE REPORT: committed dark + the theme's desk, the cursor on a light
        // theme — the check sat on "Quase branco" while the cursor sat on "Cor
        // do tema"; and committed light + the theme's desk showed under a dark
        // theme with no check at all. The list is the SHOWN theme's, so the
        // committed answer is found in it by that theme's mode.
        let dark = theme::all().find(|t| !t.is_light()).expect("a dark theme");
        let light = theme::all().find(|t| t.is_light()).expect("a light theme");
        let t = texts("en-GB");
        let desk = Layout::of(WIDE).places[Column::Desk.index()];
        let ticked = |v: &View| -> Vec<String> {
            v.lines(&t, WIDE)
                .iter()
                .flatten()
                .filter(|f| f.at == desk.from && f.text.starts_with(tokens::icon::SUCCESS))
                .map(|f| f.text.clone())
                .collect()
        };
        for (committed, shown) in [(dark, light), (light, dark), (dark, dark), (light, light)] {
            for backdrop in Backdrop::ALL {
                let c = Choice {
                    backdrop,
                    ..Choice::with_theme(committed)
                };
                let mut v = View::fresh_one(c, Before::Chosen);
                v.clicked(Column::Themes, 1 + shown.position());
                assert_eq!(v.focused(), shown);
                let marks = ticked(&v);
                let name = t.t(backdrop.text_key(shown.is_light()));
                assert_eq!(
                    marks.len(),
                    1,
                    "{} committed on {backdrop:?}, {} shown: {marks:?}",
                    committed.id(),
                    shown.id()
                );
                assert!(
                    marks[0].ends_with(&name),
                    "{} committed on {backdrop:?}, {} shown: the check is on {:?}, not {name:?}",
                    committed.id(),
                    shown.id(),
                    marks[0]
                );
                // AND IT IS ON THE ROW THE CURSOR IS ON, since nothing moved
                // the desk's cursor.
                assert_eq!(
                    Column::Desk.position_in(c, shown.is_light()),
                    v.cursor(Column::Desk)
                );
            }
        }
    }

    #[test]
    fn a_kept_near_white_survives_the_top_of_the_list_and_a_click_that_only_focuses() {
        // THE NEAR-WHITE IS AN ANSWER, KEPT under a dark theme on the absolute
        // desk's row. `<up>` at the top of the desk column and a click on that
        // row from another column each changed it to the absolute desk with
        // nothing on screen moving — and the keyboard (`<right>`, `RET`) then
        // committed "near" where the pointer (click, click) committed
        // "absolute".
        let dark = theme::all().find(|t| !t.is_light()).expect("a dark theme");
        let near_under_dark = Choice {
            backdrop: Backdrop::Near,
            ..Choice::with_theme(dark)
        };
        // THE KEYBOARD: across to the desk, `<up>` at the top.
        let mut keys = View::fresh_one(near_under_dark, Before::Chosen);
        keys.cross(1);
        assert_eq!(keys.cursor(Column::Desk), 0, "shown on the absolute row");
        assert_eq!(keys.move_by(-1), None, "the top previews nothing");
        assert_eq!(keys.focused_choice().backdrop, Backdrop::Near);
        // THE POINTER: a click on the desk's cursor row from the themes
        // column only brings the keyboard there.
        let mut pointer = View::fresh_one(near_under_dark, Before::Chosen);
        assert_eq!(pointer.clicked(Column::Desk, 1), Clicked::Focused);
        assert_eq!(pointer.focused_choice().backdrop, Backdrop::Near);
        // AND THE SECOND CLICK IS `RET`, with the same answer the keys have.
        assert_eq!(pointer.clicked(Column::Desk, 1), Clicked::Again);
        assert_eq!(pointer.focused_choice(), keys.focused_choice());
        // ONLY ANOTHER ROW CHANGES IT.
        assert!(matches!(
            pointer.clicked(Column::Desk, 2),
            Clicked::Previewed(c) if c.backdrop == Backdrop::Theme
        ));
    }

    #[test]
    fn a_legacy_black_opens_on_the_absolute_desk_and_the_choice_round_trips() {
        // A FILE FROM BEFORE 2026-09-28 says `"black"`. The chooser opens
        // with the desk's cursor on the absolute answer, and what `RET` would
        // write is what the file meant.
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-theme-chooser-{}-legacy",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let owl = theme::default_one();
        std::fs::write(
            dir.join(crate::theme::FILE),
            format!(
                "{{\"theme\":\"{}\",\"backdrop\":\"black\",\"footer\":\"desk\",\"borders\":\"discreet\"}}",
                owl.id()
            ),
        )
        .unwrap();
        let (committed, _, complaints) = crate::theme::resolve_in(Some(&dir), None);
        assert!(complaints.is_empty(), "{complaints:?}");
        let v = View::fresh_one(committed, Before::Chosen);
        assert_eq!(v.cursor(Column::Desk), Backdrop::Absolute.position());
        assert_eq!(v.cursor(Column::Footer), Ground::Desk.position());
        assert_eq!(v.cursor(Column::Windows), Ground::Theme.position());
        assert_eq!(v.cursor(Column::Borders), Borders::Discreet.position());
        assert_eq!(v.focused_choice(), committed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn commit_moves_the_committed_choice_and_nothing_else_does() {
        let mut v = View::fresh_one(on(Backdrop::Absolute), Before::Chosen);
        let before = v.committed();
        v.cross(1);
        let shown = v.move_by(1).expect("the desk moved");
        assert_eq!(shown.backdrop, Backdrop::Theme);
        assert_eq!(v.committed(), before, "moving is a preview, not a choice");
        v.commit();
        assert_eq!(v.committed().backdrop, Backdrop::Theme);
        let other = Choice {
            backdrop: Backdrop::Absolute,
            ..Choice::with_theme(theme::default_one())
        };
        v.committed_became(other);
        assert_eq!(v.committed(), other);
    }

    #[test]
    fn the_hint_names_the_keys_that_apply_and_cross_and_the_way_back_once() {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let hint = View::fresh_one(on(Backdrop::Absolute), Before::Chosen).hint(&texts);
        let joined: String = hint.iter().map(|p| p.text.as_str()).collect();
        assert!(joined.contains("RET"), "{joined}");
        assert!(joined.contains("<left>/<right>"), "{joined}");
        // AND NOT `ESC`: the way back is the footer's, clickable and first
        // (`chrome::with_the_way_back`), and a second one of the view's own
        // drew the key twice on one line.
        assert!(!joined.contains("ESC"), "{joined}");
        let line: String = crate::chrome::with_the_way_back(hint.clone(), &texts)
            .iter()
            .map(|p| p.text.as_str())
            .collect();
        assert_eq!(line.matches("ESC").count(), 1, "{line}");
        // AND THE ARROWS ARE KEYS, not prose: each in the register `RET` is
        // in, and the words between them in another.
        let register_of = |text: &str| {
            hint.iter()
                .find(|p| p.text == text)
                .unwrap_or_else(|| panic!("no piece `{text}` in {joined}"))
                .register
        };
        assert_eq!(register_of("<left>"), register_of("RET"));
        assert_eq!(register_of("<right>"), register_of("RET"));
        assert_ne!(
            register_of("/"),
            register_of("RET"),
            "the slash is the sentence's"
        );
        assert_eq!(
            key_that_runs("theme-apply")
                .map(|k| k.notation())
                .as_deref(),
            Some("RET")
        );
        assert!(key_that_runs("theme-open").is_none());
    }

    #[test]
    fn every_title_and_answer_has_its_name_in_the_texts() {
        let every: Vec<ronoctua_i18n::Texts> = ronoctua_i18n::LANGUAGES
            .iter()
            .map(|l| ronoctua_i18n::Texts::fresh(l).expect("a declared language"))
            .collect();
        let has = |key: &str| every.iter().all(|t| t.has(key));
        for c in Column::ALL {
            assert!(has(c.title_key()), "{c:?} has no title");
        }
        for b in Backdrop::ALL {
            for light in [false, true] {
                assert!(
                    has(b.text_key(light)),
                    "{} has no text key",
                    b.text_key(light)
                );
            }
        }
    }

    #[test]
    fn a_preview_this_door_wrote_does_not_become_the_committed_choice() {
        // `Esc` HAD NOTHING TO GO BACK TO. A preview writes the file, this
        // door watches that file, and `theme_changed` moved `committed` to
        // whatever the file said — including this door's own preview, a moment
        // after it wrote it. One arrow was enough to make `Esc` a key that did
        // nothing; measured in `.#shots`, where the file read the previewed
        // choice both before and after the key.
        let mut v = View::fresh_one(on(Backdrop::Theme), Before::Chosen);
        let was = v.committed();
        let previewed = Choice {
            backdrop: Backdrop::Absolute,
            ..was
        };
        v.previewing(previewed);
        // The watcher hands this door back what it just wrote.
        if v.previewed() != Some(previewed) {
            v.committed_became(previewed);
        }
        assert_eq!(
            v.committed(),
            was,
            "this door learnt its own preview back as a choice"
        );

        // AND SOMEBODY ELSE'S WRITE STILL WINS, which is the promise the
        // module header makes: what is on disk beats what is being tried.
        let elsewhere = Choice {
            backdrop: Backdrop::Absolute,
            ..was
        };
        let mut other = View::fresh_one(on(Backdrop::Theme), Before::Chosen);
        if other.previewed() != Some(elsewhere) {
            other.committed_became(elsewhere);
        }
        assert_eq!(
            other.committed(),
            elsewhere,
            "an external write was ignored"
        );
    }
}
