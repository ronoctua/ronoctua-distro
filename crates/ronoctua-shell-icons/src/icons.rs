//! THE ICONS APP — Nerd Fonts, Lucide and Unicode in one view, and its two
//! doors (`LEADER m i`: the footer's region, `m i w` the window).
//!
//! # What it replaced
//!
//! Three WASM buffers, one per collection, each with its own search, its own
//! grid arithmetic, its own sheet and its own line of shortcuts. Now the
//! tables, the search and the copy formats are ONE library
//! (`ronoctua-iconset`), and this is the one view that shows them: three
//! screens behind three tabs, a grid, and the icon's page
//! (`crate::icons_page`). The view does not know which door it is in — the
//! window (`crate::icons_window`) and the footer's region
//! (`crate::icons_region`) hand it an
//! [`Area`] and paint the [`Drawing`] it gives back, and a key or a click
//! reaches it the same way from both.
//!
//! # The shape of a screen
//!
//! The tabs first — on the window's header line, or the region's first line
//! in the footer, each a click that is its key (`C-1`…`C-3`, `C-TAB` walks
//! them). Under them ONE line, the search's: what is typed (or what typing
//! does), how much it found, the filter mark, and what the icon under the
//! pointer — or under the cursor — is: its name and its tags. Then the grid,
//! from the very next line to the last: no rule and no air on top, no pinned
//! line at the bottom. The keys are the system's shortcut line (the window's
//! header, the footer's minibuffer), never a line of the view's own.
//!
//! The name is on the view's own line and not on the door's explanation
//! row: that row exists only while the header's line is OPEN and a key is
//! under the pointer (the owner's rule, `chrome::Hinted::rows`), so an icon's
//! name put there would be hidden whenever the line is shut. What the door's
//! row says, the search line says for the view's own controls — the filter
//! mark, and in the footer the tabs — in the explanation's register.
//!
//! # The filter panel: the categories one finds without knowing their names
//!
//! Every screen filters by a prefix typed in the search (`md:`,
//! `category:arrows`, `family:letter`) — fast, and invisible: nobody types
//! `md:` who does not already know the set is called `md`. The panel is the
//! discovery (`TAB`, or the filter mark beside the count): the Nerd Fonts
//! sets, Lucide's categories, Unicode's families and — in a section of their
//! own — the emoji groups and skin tones, each a choice that WRITES its
//! prefix into the search, which stays editable. It is not a second filter:
//! choosing keeps the words typed (`folder` becomes `md:folder`), and the
//! mark beside the count is lit whenever a filter is in use, so a narrowed
//! grid never looks like the whole table. The list comes from the TABLES
//! (`Set::sets`, `Set::categories`) and not from code, so a set the next
//! Nerd Fonts adds is offered the day it arrives.
//!
//! # A grid lights a CELL
//!
//! The cell under the pointer is lit on its own cell, never a row's band,
//! and the cursor's cell wears `surface.selection` — the file manager's
//! exception, extended to every grid (AGENTS.md). An emoji's colour is inside
//! its glyph, so the cursor could never be a colour of the ink here.
//!
//! # The tables are held while a door shows them
//!
//! `ronoctua_iconset::shared` is read on a thread when a door opens the view
//! (~9 MB, a few hundred milliseconds), handed back with [`View::took`], and
//! dropped with the view: the footer lives for months.

use std::sync::Arc;

use ronoctua_iconset::{Collection, FilterKind, Grammar, Query, Refusal, Set};
use ronoctua_tokens as tokens;

use crate::chrome::Piece;
pub use crate::icons_page::{Area, Drawing};
use crate::icons_page::{Icon, Page, Settings};

/// The grid's keys, as the shortcut line teaches them — `(notation, id)`,
/// the id a text key (its words) with an `about-<id>` sentence.
pub const OF_THE_GRID: &[(&str, &str)] = &[
    ("RET", "icons-page"),
    ("C-c", "icons-copy"),
    ("TAB", "icons-filters"),
];

/// The open filter panel's keys.
pub const OF_THE_PANEL: &[(&str, &str)] = &[("RET", "icons-choose"), ("TAB", "icons-filters")];

/// The page's keys. `RET` says `icons-change` where the cursor is on a
/// setting (`chrome::hint_of_with`).
pub const OF_THE_PAGE: &[(&str, &str)] = &[
    ("<left>", "icons-less"),
    ("<right>", "icons-more"),
    ("RET", "icons-enter"),
    ("TAB", "icons-reset"),
];

/// The key that opens and shuts the filter panel, and its id — the filter
/// mark beside the count is the same control.
pub const FILTERS: &str = "TAB";
/// See [`FILTERS`].
pub const FILTERS_ID: &str = "icons-filters";

/// The three tabs: the key that shows each, its screen and its id (its
/// sentence is `about-<id>`).
pub const TABS: [(&str, Collection, &str); 3] = [
    ("C-1", Collection::NerdFonts, "icons-tab-nerd-fonts"),
    ("C-2", Collection::Lucide, "icons-tab-lucide"),
    ("C-3", Collection::Unicode, "icons-tab-unicode"),
];

/// The notation of the key that walks the tabs.
pub const NEXT_SCREEN: &str = "C-TAB";

/// The command that asks for the Lucide colour — the question's id, and what
/// the answer comes back under in both doors.
pub const COLOUR_COMMAND: &str = "icons-colour";

/// The commands the leader reaches this view by, and the screen each opens
/// on.
pub const SCREEN_COMMANDS: [(&str, Collection); 3] = [
    ("icons-nerd-fonts", Collection::NerdFonts),
    ("icons-lucide", Collection::Lucide),
    ("icons-unicode", Collection::Unicode),
];

/// The screen a command, or a word of the command line, names.
#[must_use]
pub fn screen_named(word: &str) -> Option<Collection> {
    SCREEN_COMMANDS
        .iter()
        .find(|(id, _)| *id == word)
        .map(|(_, c)| *c)
        .or_else(|| Collection::ALL.into_iter().find(|c| c.id() == word))
}

/// Every command this view reaches, for `bindings::reachable`.
pub fn reached() -> impl Iterator<Item = &'static str> {
    ["icons-open", "icons-window", COLOUR_COMMAND]
        .into_iter()
        .chain(SCREEN_COMMANDS.iter().map(|(id, _)| *id))
}

/// A key, read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Show this screen.
    Tab(Collection),
    /// The next screen.
    NextScreen,
    /// A step of the grid's cursor — or the panel's, while it is open:
    /// across and down, in items and rows.
    Move(i32, i32),
    /// A page of rows, down or up.
    Pages(i32),
    /// The first item, or the last.
    End(bool),
    /// Open the page of the item under the cursor — or, with the panel open,
    /// choose the filter under its cursor.
    Open,
    /// Copy the icon under the cursor, as it is drawn.
    Copy,
    /// Open or shut the filter panel.
    Filters,
    /// `Esc`: the page, the panel, the query, then the view.
    Escape,
    /// `DEL`: the query's last letter.
    Erase,
    /// A letter of the query.
    Letter(char),
    /// `C-v`: the clipboard into the query.
    Paste,
    /// A key of the open page.
    Page(crate::icons_page::Press),
}

/// A key by its WHOLE notation, as this view's press — the one reader both
/// doors call. `in_page` says which table applies.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key, in_page: bool) -> Option<Press> {
    use crate::icons_page::Press as P;
    use ronoctua_keymap::Sym;
    if key.sym == Sym::Esc {
        return Some(Press::Escape);
    }
    let notation = key.notation();
    if let Some((_, c, _)) = TABS.iter().find(|(n, _, _)| *n == notation) {
        return Some(Press::Tab(*c));
    }
    if notation == NEXT_SCREEN {
        return Some(Press::NextScreen);
    }
    if crate::line::pastes(key) {
        return Some(Press::Paste);
    }
    if in_page {
        return Some(match notation.as_str() {
            "<up>" | "C-p" => Press::Page(P::Up),
            "<down>" | "C-n" => Press::Page(P::Down),
            "<left>" => Press::Page(P::Less),
            "<right>" => Press::Page(P::More),
            "RET" => Press::Page(P::Enter),
            "TAB" => Press::Page(P::Reset),
            "C-c" => Press::Copy,
            "C-g" => Press::Escape,
            _ => return None,
        });
    }
    Some(match notation.as_str() {
        "<left>" => Press::Move(-1, 0),
        "<right>" => Press::Move(1, 0),
        "<up>" | "C-p" => Press::Move(0, -1),
        "<down>" | "C-n" => Press::Move(0, 1),
        "<prior>" => Press::Pages(-1),
        "<next>" => Press::Pages(1),
        "<home>" => Press::End(false),
        "<end>" => Press::End(true),
        "RET" => Press::Open,
        "C-c" => Press::Copy,
        n if n == FILTERS => Press::Filters,
        _ => match crate::minibuffer::gesture_of(key) {
            crate::minibuffer::Gesture::Cancel => Press::Escape,
            crate::minibuffer::Gesture::Delete => Press::Erase,
            crate::minibuffer::Gesture::Insert(c) => Press::Letter(c),
            _ => return None,
        },
    })
}

/// What the view asks of its door.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// A frame, and no more.
    Nothing,
    /// Close the view.
    Close,
    /// Put this on the clipboard, and say so.
    Copy(String),
    /// Ask for the Lucide colour, the line starting from this.
    AskColour(String),
    /// Read the clipboard, and hand its text to [`View::pasted`].
    Paste,
}

/// What the pointer is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// An item of the grid, by its place among the hits.
    Cell(usize),
    /// The grid's bar.
    GridBar,
    /// The filter mark beside the count.
    Funnel,
    /// A choice of the open filter panel.
    Choice(usize),
    /// The open panel's bar, when its lines do not fit.
    PanelBar,
    /// A part of the open page.
    Page(crate::icons_page::Target),
}

/// Where the codes of [`Target::code`] begin, per kind.
const CHOICE: usize = 1 << 47;
/// See [`CHOICE`].
const PAGE: usize = 1 << 48;

impl Target {
    /// The target as ONE number — what a door's hand keeps as its "row", so
    /// each pointer's hover reaches the paint in its own colour.
    #[must_use]
    pub const fn code(self) -> usize {
        use crate::icons_page::Target as P;
        match self {
            Self::Cell(n) => n,
            Self::Choice(n) => CHOICE + n,
            Self::PanelBar => PAGE - 3,
            Self::Funnel => PAGE - 2,
            Self::GridBar => PAGE - 1,
            Self::Page(P::Row(n)) => PAGE + n,
            Self::Page(P::Bar) => usize::MAX - 1,
            Self::Page(P::Stage) => usize::MAX,
        }
    }

    /// The target a [`Target::code`] stands for.
    #[must_use]
    pub const fn of_code(code: usize) -> Self {
        use crate::icons_page::Target as P;
        match code {
            usize::MAX => Self::Page(P::Stage),
            c if c == usize::MAX - 1 => Self::Page(P::Bar),
            c if c == PAGE - 1 => Self::GridBar,
            c if c == PAGE - 2 => Self::Funnel,
            c if c == PAGE - 3 => Self::PanelBar,
            c if c >= PAGE => Self::Page(P::Row(c - PAGE)),
            c if c >= CHOICE => Self::Choice(c - CHOICE),
            c => Self::Cell(c),
        }
    }
}

/// A bar being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grab {
    /// The grid's.
    Grid,
    /// The page's list's.
    List,
    /// The filter panel's.
    Panel,
}

/// One screen: what is typed, what it finds, where the cursor and the grid
/// are.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Screen {
    query: String,
    /// The records found, best first, by their index in the table.
    hits: Vec<usize>,
    /// The record under the cursor — by IDENTITY, never by its place: a new
    /// query reorders the hits.
    cursor: Option<usize>,
    /// The first item shown, a multiple of the grid's row.
    top: usize,
    /// Whether `hits` still answers `query`.
    fresh: bool,
}

/// One choice of the filter panel: its words, and the filter it writes —
/// `None` for "all", which writes none.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Choice {
    label: String,
    filter: Option<(FilterKind, String)>,
}

/// The filter panel of the screen showing, while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Panel {
    /// Every choice, in the order drawn.
    choices: Vec<Choice>,
    /// Where the second section begins — Unicode's emoji groups and tones,
    /// filters only the emoji have; `choices.len()` when there is none.
    split: usize,
    /// The second section's heading.
    heading: String,
    /// The choice under the panel's cursor.
    cursor: usize,
    /// The first line of the panel shown.
    offset: usize,
}

/// A line of the panel as it is laid out.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PanelLine {
    /// Choices, by index, left to right.
    Choices(Vec<usize>),
    /// The blank line before a section's title (`chrome::AIR`).
    Air,
    /// The second section's title.
    Heading,
}

/// Where the open panel is: its lines, which of them show and where, and the
/// width of a choice's cell — the ONE layout its drawing and the pointer read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PanelLay {
    lines: Vec<PanelLine>,
    /// The first line of the surface it takes.
    top: u16,
    /// How many lines it takes.
    shown: u16,
    /// Its first line shown, among `lines`.
    offset: usize,
    /// The first cell of its first column.
    left: u16,
    /// A choice's cell, its air included.
    step: u16,
}

impl PanelLay {
    /// Where choice `n` is drawn, when it is shown: its line of the surface,
    /// its place on that line, and how many choices share the line.
    #[cfg(test)]
    fn place(&self, n: usize) -> Option<(u16, usize, usize)> {
        self.lines
            .iter()
            .enumerate()
            .skip(self.offset)
            .take(usize::from(self.shown))
            .find_map(|(i, l)| match l {
                PanelLine::Choices(v) => v.iter().position(|c| *c == n).map(|k| {
                    (
                        self.top + u16::try_from(i - self.offset).unwrap_or(0),
                        k,
                        v.len(),
                    )
                }),
                _ => None,
            })
    }

    /// The `count` choices of the surface's line `line`, as a grid of one
    /// row (`chrome::Shelves`): the cell a hover lights and a click takes is
    /// a choice's words and half the air on each side, from one arithmetic
    /// in both directions.
    fn shelf(&self, line: u16, count: usize) -> crate::chrome::Shelves {
        crate::chrome::Shelves {
            grid: ronoctua_design::Grid {
                across: u16::try_from(count).unwrap_or(u16::MAX).max(1),
                step: self.step,
                down: 1,
                first: line,
                rows: 1,
            },
            left: self.left,
            air: tokens::space::ICONS_PANELGAP,
        }
    }

    /// The pixel box of choice `n`, when it is shown.
    #[cfg(test)]
    fn cell_box(&self, n: usize) -> Option<crate::chrome::Rect> {
        let (line, k, count) = self.place(n)?;
        self.shelf(line, count).cell_box(k)
    }

    /// The choice under `(x, y)`, from the arithmetic the drawing uses.
    fn choice_at(&self, x: f32, y: f32) -> Option<usize> {
        if y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = u16::try_from((y / Area::line()).floor() as u32).ok()?;
        let shown = line.checked_sub(self.top)?;
        if shown >= self.shown {
            return None;
        }
        let PanelLine::Choices(v) = self.lines.get(self.offset + usize::from(shown))? else {
            return None;
        };
        self.shelf(line, v.len())
            .index_at(x, y)
            .and_then(|k| v.get(k).copied())
    }
}

/// The view.
#[derive(Debug, Clone)]
pub struct View {
    screen: Collection,
    screens: [Screen; 3],
    /// The icon's page, while one is open.
    page: Option<Page>,
    /// The filter panel, while it is open.
    panel: Option<Panel>,
    /// What a person adjusted, across the icons of this view.
    settings: Settings,
    /// The tables, once a door handed them in.
    set: Option<Arc<Set>>,
    /// The prefixes a query understands, built once with the set.
    grammar: Grammar,
    /// Why there are no tables.
    refused: Option<Refusal>,
    /// The reader's language, for the tags a search ranks first.
    subtag: String,
    /// What the pointer that moved last rests on — what the search line
    /// names, and the footer's one hover.
    hover: Option<Target>,
    /// Whether a KEY came after the pointer last moved: the search line then
    /// names the cursor's icon, which is what the keys are moving.
    keyed: bool,
    /// A bar being dragged.
    pub grabbed: Option<Grab>,
    /// Lines the wheel asked for and the grid has not moved yet.
    pub lines_owed: f64,
    /// The footer's tab under its pointer, lit as every shortcut is.
    tab_lit: Option<&'static str>,
}

/// The index of a screen in [`Collection::ALL`].
fn slot(c: Collection) -> usize {
    Collection::ALL.iter().position(|x| *x == c).unwrap_or(0)
}

/// The filter kinds a screen's panel writes — and takes off the search when
/// another of its choices is made. Unicode's `symbols:` is among them: the
/// screen opens on it, and choosing a family replaces it.
const fn panel_kinds(screen: Collection) -> &'static [FilterKind] {
    match screen {
        Collection::NerdFonts => &[FilterKind::Set],
        Collection::Lucide => &[FilterKind::Category],
        Collection::Unicode => &[
            FilterKind::Symbols,
            FilterKind::Family,
            FilterKind::Group,
            FilterKind::Tone,
        ],
    }
}

impl View {
    /// A view on `screen`, with no tables yet — the door reads them on a
    /// thread and hands them in with [`View::took`].
    #[must_use]
    pub fn new(screen: Collection, texts: &ronoctua_i18n::Texts) -> Self {
        let mut screens: [Screen; 3] = Default::default();
        // UNICODE OPENS FILTERED, WITH THE FILTER WRITTEN IN THE FIELD: the
        // symbols and the numbers, and the words `symbols:` where the person
        // can see why the letters are not there — a filter nobody can see is
        // hidden state. `Esc` takes it off (`View::escape`).
        screens[slot(Collection::Unicode)].query = opening(Collection::Unicode, texts);
        Self {
            screen,
            screens,
            page: None,
            panel: None,
            settings: Settings::default(),
            set: None,
            grammar: Grammar::default(),
            refused: None,
            subtag: crate::icons_page::subtag_of(texts),
            hover: None,
            keyed: false,
            grabbed: None,
            lines_owed: 0.0,
            tab_lit: None,
        }
    }

    /// The tables arrived — or why they did not.
    pub fn took(&mut self, answer: Result<Arc<Set>, Refusal>) {
        match answer {
            Ok(set) => {
                self.grammar = Grammar::new(&set);
                self.set = Some(set);
                self.refused = None;
                for s in &mut self.screens {
                    s.fresh = false;
                }
            }
            Err(why) => {
                self.set = None;
                self.refused = Some(why);
            }
        }
        self.search();
    }

    /// The screen showing.
    #[must_use]
    pub const fn screen(&self) -> Collection {
        self.screen
    }

    /// Whether the icon's page is open.
    #[must_use]
    pub const fn in_page(&self) -> bool {
        self.page.is_some()
    }

    /// Whether the filter panel is open.
    #[cfg(test)]
    const fn in_panel(&self) -> bool {
        self.panel.is_some()
    }

    /// What is typed on the screen showing.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.screens[slot(self.screen)].query
    }

    /// What the screen showing found, by index in its table.
    #[must_use]
    pub fn hits(&self) -> &[usize] {
        &self.screens[slot(self.screen)].hits
    }

    /// The record under the cursor.
    #[must_use]
    pub fn cursor(&self) -> Option<usize> {
        self.screens[slot(self.screen)].cursor
    }

    /// The pointer moved, and now rests on `target` — what the search line
    /// names from now until a key moves the cursor. Answers whether that
    /// changed anything.
    pub fn hovered(&mut self, target: Option<Target>) -> bool {
        if target == self.hover {
            return false;
        }
        self.hover = target;
        self.keyed = false;
        true
    }

    /// The footer's pointer is on its tab line, over tab `id` (or none):
    /// lit, and said on the search line under it — until a key.
    pub fn tab_hovered(&mut self, id: Option<&'static str>) -> bool {
        if id == self.tab_lit {
            return false;
        }
        self.tab_lit = id;
        self.keyed = false;
        true
    }

    /// Shows `screen`, the page and the panel closed.
    pub fn show(&mut self, screen: Collection) {
        self.screen = screen;
        self.page = None;
        self.panel = None;
        self.hover = None;
        self.search();
    }

    /// The hits of the screen showing, answered again when the query moved.
    fn search(&mut self) {
        let Some(set) = self.set.clone() else {
            return;
        };
        let at = slot(self.screen);
        if self.screens[at].fresh {
            return;
        }
        let query = Query::parse(&self.screens[at].query, &self.grammar);
        let most = set.len(self.screen);
        let hits: Vec<usize> =
            ronoctua_iconset::search(&set, self.screen, &query, &self.subtag, most)
                .into_iter()
                .map(|h| h.index)
                .collect();
        let s = &mut self.screens[at];
        // THE CURSOR KEEPS ITS ICON while the icon is still found; a new
        // query otherwise starts on its best match, at the top.
        if !s.cursor.is_some_and(|c| hits.contains(&c)) {
            s.cursor = hits.first().copied();
            s.top = 0;
        }
        s.hits = hits;
        s.fresh = true;
    }

    /// The query changed: the hits follow, and the cursor goes to the best.
    fn requery(&mut self, query: String) {
        let s = &mut self.screens[slot(self.screen)];
        if s.query == query {
            return;
        }
        s.query = query;
        s.fresh = false;
        s.cursor = None;
        s.top = 0;
        self.search();
    }

    /// The clipboard's text, into the query — ONE line of it.
    pub fn pasted(&mut self, text: &str) -> Errand {
        if self.page.is_some() {
            return Errand::Nothing;
        }
        let mut q = self.query().to_owned();
        q.push_str(&crate::line::one_line(text));
        self.requery(q);
        Errand::Nothing
    }

    /// The Lucide colour, as it was typed on the question's line.
    pub fn colour_answered(&mut self, text: &str) {
        self.settings.lucide.colour = crate::line::one_line(text);
    }

    /// The Lucide colour as it is — where the question's line starts when it
    /// is asked by the command's name rather than from the page.
    #[must_use]
    pub fn colour(&self) -> String {
        self.settings.lucide.colour.clone()
    }

    /// The icon of the record under the cursor.
    fn icon(&self) -> Option<Icon> {
        self.cursor().map(|index| Icon {
            collection: self.screen,
            index,
        })
    }

    /// One key. `area` is where the view is drawn — its search line first; a
    /// step down is a ROW of the grid, and a row is as wide as the door is.
    pub fn key(&mut self, press: Press, area: Area, texts: &ronoctua_i18n::Texts) -> Errand {
        self.keyed = true;
        match press {
            Press::Tab(c) => {
                self.show(c);
                Errand::Nothing
            }
            Press::NextScreen => {
                let next = Collection::ALL[(slot(self.screen) + 1) % Collection::ALL.len()];
                self.show(next);
                Errand::Nothing
            }
            Press::Escape => self.escape(texts),
            Press::Paste => Errand::Paste,
            Press::Copy => self.copy_drawn(),
            Press::Page(p) => self.page_key(p, area, texts),
            _ if self.page.is_some() => Errand::Nothing,
            Press::Filters => {
                self.toggle_panel(area, texts);
                Errand::Nothing
            }
            Press::Erase => {
                let mut q = self.query().to_owned();
                crate::line::pop_cluster(&mut q);
                self.requery(q);
                Errand::Nothing
            }
            Press::Letter(c) => {
                let mut q = self.query().to_owned();
                q.push(c);
                self.requery(q);
                Errand::Nothing
            }
            _ if self.panel.is_some() => {
                self.panel_key(press, area, texts);
                Errand::Nothing
            }
            Press::Open => {
                self.open_page(area, texts);
                Errand::Nothing
            }
            Press::Move(across, down) => {
                let g = self.grid(area);
                let step = i64::from(across) + i64::from(down) * i64::from(g.across.max(1));
                self.step(step, area);
                Errand::Nothing
            }
            Press::Pages(n) => {
                let g = self.grid(area);
                let step = i64::from(n) * i64::try_from(g.fit()).unwrap_or(i64::MAX);
                self.step(step, area);
                Errand::Nothing
            }
            Press::End(last) => {
                let n = self.hits().len();
                let to = if last { n.saturating_sub(1) } else { 0 };
                self.cursor_to(to, area);
                Errand::Nothing
            }
        }
    }

    /// `Esc`, in layers: the page, the panel, the words typed after the
    /// opening filter, the filter, and then the view — each press changes
    /// what is on screen, so there is no press that seems to do nothing.
    fn escape(&mut self, texts: &ronoctua_i18n::Texts) -> Errand {
        if self.page.take().is_some() || self.panel.take().is_some() {
            return Errand::Nothing;
        }
        let query = self.query().to_owned();
        if query.is_empty() {
            return Errand::Close;
        }
        let opening = opening(self.screen, texts);
        let back = if !opening.is_empty() && query.trim() != opening && query.starts_with(&opening)
        {
            opening
        } else {
            String::new()
        };
        self.requery(back);
        Errand::Nothing
    }

    /// Copies the icon under the cursor as it is DRAWN — the glyph, the
    /// character, the Lucide SVG — whether the page is open or not.
    fn copy_drawn(&self) -> Errand {
        use crate::icons_page::Format;
        use ronoctua_iconset::copy::{LucideFormat, NerdFormat, UnicodeFormat};
        let (Some(set), Some(icon)) = (
            self.set.as_ref(),
            self.page.as_ref().map(|p| p.icon).or_else(|| self.icon()),
        ) else {
            return Errand::Nothing;
        };
        let format = match icon.collection {
            Collection::NerdFonts => Format::Nerd(NerdFormat::Glyph),
            Collection::Lucide => Format::Lucide(LucideFormat::Svg),
            Collection::Unicode => Format::Unicode(UnicodeFormat::Character),
        };
        let page = Page {
            icon,
            cursor: 0,
            offset: 0,
        };
        page.copied(set, &self.settings, format)
            .map_or(Errand::Nothing, Errand::Copy)
    }

    /// The area the page is drawn in: under the search line.
    const fn page_area(area: Area) -> Area {
        area.below(1)
    }

    /// Opens the page of the icon under the cursor — its list scrolled to its
    /// cursor, which starts on the first row that acts and may be further down
    /// than a short door shows.
    fn open_page(&mut self, area: Area, texts: &ronoctua_i18n::Texts) {
        let (Some(set), Some(icon)) = (self.set.clone(), self.icon()) else {
            return;
        };
        let mut page = Page::of(icon, &set, &self.settings, texts);
        page.follow(Self::page_area(area), &set, &self.settings, texts);
        self.page = Some(page);
        self.panel = None;
        self.hover = None;
    }

    /// A key of the page, and what it asks.
    fn page_key(
        &mut self,
        press: crate::icons_page::Press,
        area: Area,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(set) = self.set.clone() else {
            return Errand::Nothing;
        };
        let Some(page) = self.page.as_mut() else {
            return Errand::Nothing;
        };
        let errand = page.key(press, &set, &mut self.settings, texts);
        self.page_errand(errand, &set, area, texts)
    }

    /// What the page asked, done.
    fn page_errand(
        &mut self,
        errand: crate::icons_page::Errand,
        set: &Set,
        area: Area,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use crate::icons_page::Errand as E;
        let out = match errand {
            E::Nothing => Errand::Nothing,
            E::Copy(text) => Errand::Copy(text),
            E::AskColour(initial) => Errand::AskColour(initial),
            E::Neighbour(by) => {
                // THE ICON BEFORE OR AFTER, among what the search found: the
                // grid's cursor walks with it, so `Esc` lands where the page
                // ended.
                self.step(i64::from(by), area);
                if let (Some(icon), Some(page)) = (self.icon(), self.page.as_mut()) {
                    page.show(icon, set, &self.settings, texts);
                }
                Errand::Nothing
            }
        };
        if let Some(page) = self.page.as_mut() {
            page.follow(Self::page_area(area), set, &self.settings, texts);
        }
        out
    }

    /// Moves the cursor `by` items among the hits, and the grid with it.
    fn step(&mut self, by: i64, area: Area) {
        let s = &self.screens[slot(self.screen)];
        let n = s.hits.len();
        if n == 0 {
            return;
        }
        let at = s
            .cursor
            .and_then(|c| s.hits.iter().position(|h| *h == c))
            .unwrap_or(0);
        let to = i64::try_from(at)
            .unwrap_or(0)
            .saturating_add(by)
            .clamp(0, i64::try_from(n - 1).unwrap_or(0));
        self.cursor_to(usize::try_from(to).unwrap_or(0), area);
    }

    /// Puts the cursor on the `n`th hit, and the grid's rows around it.
    fn cursor_to(&mut self, n: usize, area: Area) {
        let g = self.grid(area);
        let s = &mut self.screens[slot(self.screen)];
        let Some(record) = s.hits.get(n).copied() else {
            return;
        };
        s.cursor = Some(record);
        let across = usize::from(g.across.max(1));
        let row = n / across;
        let top_row = s.top / across;
        let rows = usize::from(g.rows.max(1));
        let top_row = if row < top_row {
            row
        } else if row >= top_row + rows {
            row + 1 - rows
        } else {
            top_row
        };
        s.top = g.top_at_row(top_row, s.hits.len());
    }

    /// How many cells one item takes across, on the screen showing.
    const fn item_cells(&self) -> u16 {
        match self.screen {
            Collection::NerdFonts => tokens::space::ICONS_NERDCELLS,
            Collection::Lucide => tokens::space::ICONS_LUCIDECELLS,
            Collection::Unicode => tokens::space::ICONS_UNICODECELLS,
        }
    }

    // ── THE FILTER PANEL ──────────────────────────────────────────────────

    /// The panel of the screen showing, its cursor on the filter in use.
    fn panel_of(&self, texts: &ronoctua_i18n::Texts) -> Option<Panel> {
        use crate::icons_page::named;
        let set = self.set.as_ref()?;
        let choice = |label: String, filter: Option<(FilterKind, String)>| Choice { label, filter };
        let mut choices = Vec::new();
        let mut split = None;
        match self.screen {
            Collection::NerdFonts => {
                choices.push(choice(texts.t(crate::key::ICONS_SET_ALL), None));
                for s in set.sets() {
                    choices.push(choice(
                        named(texts, "icons-set-", &s),
                        Some((FilterKind::Set, s)),
                    ));
                }
            }
            Collection::Lucide => {
                choices.push(choice(texts.t(crate::key::ICONS_CATEGORY_ALL), None));
                for c in set.categories() {
                    choices.push(choice(
                        named(texts, "icons-category-", &c),
                        Some((FilterKind::Category, c)),
                    ));
                }
            }
            Collection::Unicode => {
                choices.push(choice(texts.t(crate::key::ICONS_FAMILY_ALL), None));
                for f in ronoctua_iconset::FAMILIES {
                    choices.push(choice(
                        named(texts, "icons-family-", f),
                        Some((FilterKind::Family, f.to_owned())),
                    ));
                }
                // THE EMOJI'S OWN FILTERS, in a section of their own and
                // ALWAYS there: hidden until `emoji` was chosen, they were a
                // panel that changed size under the hand, and nobody who did
                // not know them could find them.
                split = Some(choices.len());
                for g in ronoctua_iconset::GROUPS {
                    choices.push(choice(
                        named(texts, "icons-group-", g),
                        Some((FilterKind::Group, g.to_owned())),
                    ));
                }
                for t in ronoctua_iconset::TONES {
                    choices.push(choice(
                        named(texts, "icons-tone-", t),
                        Some((FilterKind::Tone, t.to_owned())),
                    ));
                }
            }
        }
        let mut panel = Panel {
            split: split.unwrap_or(choices.len()),
            choices,
            heading: texts.t(crate::key::ICONS_PANEL_EMOJI),
            cursor: 0,
            offset: 0,
        };
        // IT OPENS WHERE THE FILTER ALREADY IS: always opening at "all"
        // would make the person find, every time, the one already in use.
        panel.cursor = self.active_choice(&panel).unwrap_or(0);
        Some(panel)
    }

    /// The filters the search has now.
    fn filters(&self) -> Vec<ronoctua_iconset::Filter> {
        Query::parse(self.query(), &self.grammar).filters
    }

    /// Which choice of `panel` the search is filtered by — "all" when it is
    /// filtered by nothing, and none when by something no choice writes (a
    /// tag, a block): saying "all" there would be a lie.
    fn active_choice(&self, panel: &Panel) -> Option<usize> {
        let filters = self.filters();
        if filters.is_empty() {
            return Some(0);
        }
        panel.choices.iter().position(|c| {
            c.filter.as_ref().is_some_and(|(kind, id)| {
                filters
                    .iter()
                    .any(|f| f.kind == *kind && f.value.as_deref() == Some(id.as_str()))
            })
        })
    }

    /// Opens or shuts the panel.
    fn toggle_panel(&mut self, area: Area, texts: &ronoctua_i18n::Texts) {
        if self.panel.take().is_some() {
            return;
        }
        self.panel = self.panel_of(texts);
        self.panel_follow(area);
    }

    /// The search with `choice` in place of the screen's panel filters — the
    /// words typed kept: `folder` and Material Design make `md:folder`.
    fn with_choice(&self, choice: &Choice, texts: &ronoctua_i18n::Texts) -> String {
        let kinds = panel_kinds(self.screen);
        let mut free: Vec<String> = Vec::new();
        let mut pieces = self.query().split_whitespace();
        while let Some(piece) = pieces.next() {
            let folded = ronoctua_iconset::fold(piece);
            let prefix = self
                .grammar
                .prefixes
                .iter()
                .filter(|p| folded.starts_with(&p.word))
                .max_by_key(|p| p.word.len());
            match prefix {
                Some(p) if kinds.contains(&p.kind) => {
                    let glued = &folded[p.word.len()..];
                    match p.kind.takes() {
                        // A PREFIX THAT TAKES NOTHING leaves what is glued to
                        // it as a word of the person's (`md:home`).
                        ronoctua_iconset::Takes::Nothing if !glued.is_empty() => {
                            free.push(glued.to_owned());
                        }
                        ronoctua_iconset::Takes::Word if glued.is_empty() => {
                            pieces.next();
                        }
                        _ => {}
                    }
                }
                _ => free.push(piece.to_owned()),
            }
        }
        let free = free.join(" ");
        let written = match &choice.filter {
            None => return free,
            Some((FilterKind::Set, s)) => return format!("{s}:{free}"),
            Some((kind, id)) => {
                let (prefix, stem) = match kind {
                    FilterKind::Family => (crate::key::ICONS_PREFIX_FAMILY, "icons-value-family-"),
                    FilterKind::Group => (crate::key::ICONS_PREFIX_GROUP, "icons-value-group-"),
                    FilterKind::Tone => (crate::key::ICONS_PREFIX_TONE, "icons-value-tone-"),
                    _ => (crate::key::ICONS_PREFIX_CATEGORY, ""),
                };
                // THE VALUE IN THE READER'S WORD where the language has one
                // (`familia:letra`): what is written is what they would type.
                let value = if stem.is_empty() {
                    id.clone()
                } else {
                    crate::icons_page::named(texts, stem, id)
                };
                format!("{}:{value}", texts.t(prefix))
            }
        };
        if free.is_empty() {
            written
        } else {
            format!("{written} {free}")
        }
    }

    /// Choice `n` into the search, and the panel shut: the choice is made,
    /// and what one wants to see is the grid it filtered.
    fn choose(&mut self, n: usize, texts: &ronoctua_i18n::Texts) {
        let Some(choice) = self.panel.as_ref().and_then(|p| p.choices.get(n)).cloned() else {
            return;
        };
        let query = self.with_choice(&choice, texts);
        self.panel = None;
        self.requery(query);
    }

    /// A key of the open panel: the arrows walk its cells, `RET` chooses.
    fn panel_key(&mut self, press: Press, area: Area, texts: &ronoctua_i18n::Texts) {
        let Some(lay) = self.panel_lay(area) else {
            return;
        };
        let Some(panel) = self.panel.as_mut() else {
            return;
        };
        let last = panel.choices.len().saturating_sub(1);
        match press {
            Press::Open => {
                let n = panel.cursor;
                self.choose(n, texts);
                return;
            }
            Press::Move(across, 0) => {
                let to = i64::try_from(panel.cursor).unwrap_or(0) + i64::from(across);
                panel.cursor = usize::try_from(to.max(0)).unwrap_or(0).min(last);
            }
            Press::Move(_, down) => {
                // A WHOLE LINE, to the same column where the line below has
                // one — the drawing is a grid, and the arrow goes where the
                // eye does.
                let rows: Vec<&Vec<usize>> = lay
                    .lines
                    .iter()
                    .filter_map(|l| match l {
                        PanelLine::Choices(v) => Some(v),
                        _ => None,
                    })
                    .collect();
                let Some((row, k)) = rows
                    .iter()
                    .enumerate()
                    .find_map(|(r, v)| v.iter().position(|c| *c == panel.cursor).map(|k| (r, k)))
                else {
                    return;
                };
                let to = i64::try_from(row).unwrap_or(0) + i64::from(down);
                let to = usize::try_from(to.max(0))
                    .unwrap_or(0)
                    .min(rows.len().saturating_sub(1));
                if let Some(v) = rows.get(to) {
                    panel.cursor = v[k.min(v.len().saturating_sub(1))];
                }
            }
            Press::End(end) => panel.cursor = if end { last } else { 0 },
            _ => return,
        }
        self.panel_follow(area);
    }

    /// Keeps the panel's cursor on its shown lines.
    fn panel_follow(&mut self, area: Area) {
        let Some(lay) = self.panel_lay(area) else {
            return;
        };
        let Some(panel) = self.panel.as_mut() else {
            return;
        };
        let Some(line) = lay.lines.iter().position(|l| match l {
            PanelLine::Choices(v) => v.contains(&panel.cursor),
            _ => false,
        }) else {
            return;
        };
        let shown = usize::from(lay.shown).max(1);
        if line < panel.offset {
            // A SECTION'S TITLE COMES WITH ITS FIRST LINE.
            let titled = line >= 2 && lay.lines.get(line - 1) == Some(&PanelLine::Heading);
            panel.offset = if titled && shown > 2 { line - 2 } else { line };
        } else if line >= panel.offset + shown {
            panel.offset = line + 1 - shown;
        }
        panel.offset = panel.offset.min(lay.lines.len().saturating_sub(shown));
    }

    /// The open panel laid out under the search line of `area`: at most
    /// `space.icons.panelSharePct` of what is left, the rest the grid's.
    fn panel_lay(&self, area: Area) -> Option<PanelLay> {
        let panel = self.panel.as_ref()?;
        let a = area.below(1);
        let gap = tokens::space::ICONS_PANELGAP;
        let widest = panel
            .choices
            .iter()
            .map(|c| ronoctua_render::layout::width(&c.label))
            .max()
            .unwrap_or(1)
            .clamp(1, a.columns.max(1));
        let step = widest + gap;
        let laid = |columns: u16| {
            let across = usize::from(((columns + gap) / step).max(1));
            let mut lines: Vec<PanelLine> = Vec::new();
            let chunk = |lines: &mut Vec<PanelLine>, from: usize, to: usize| {
                let all: Vec<usize> = (from..to).collect();
                for c in all.chunks(across) {
                    lines.push(PanelLine::Choices(c.to_vec()));
                }
            };
            chunk(&mut lines, 0, panel.split);
            if panel.split < panel.choices.len() {
                lines.push(PanelLine::Air);
                lines.push(PanelLine::Heading);
                chunk(&mut lines, panel.split, panel.choices.len());
            }
            lines
        };
        let room =
            u16::try_from(u32::from(a.lines) * u32::from(tokens::space::ICONS_PANELSHAREPCT) / 100)
                .unwrap_or(u16::MAX)
                .max(1);
        // A PANEL THAT DOES NOT FIT HAS A BAR, and the bar's column is its
        // own: the choices are laid out again without it (`a-bar-owns-its-
        // column`), as the grid is.
        let mut lines = laid(a.columns);
        if lines.len() > usize::from(room) {
            lines = laid(
                a.columns
                    .saturating_sub(crate::chrome::BAR_CELLS + ronoctua_design::GAP),
            );
        }
        let shown = u16::try_from(lines.len()).unwrap_or(u16::MAX).min(room);
        let offset = panel
            .offset
            .min(lines.len().saturating_sub(usize::from(shown)));
        Some(PanelLay {
            lines,
            top: a.top,
            shown,
            offset,
            left: a.left,
            step,
        })
    }

    /// Where the open panel's bar is — its track and thumb — when its lines
    /// do not fit: the one answer its drawing and the pointer read.
    fn choices_bar(
        &self,
        area: Area,
    ) -> Option<(crate::chrome::Rect, Option<crate::chrome::Rect>)> {
        let lay = self.panel_lay(area)?;
        let scroll =
            crate::chrome::list_scroll(lay.lines.len(), usize::from(lay.shown), lay.offset);
        scroll.cut().then(|| {
            crate::chrome::band_bar(
                scroll,
                area.right_px,
                Area::y_of(lay.top),
                Area::y_of(lay.top + lay.shown),
                Area::cell(),
                Area::line(),
            )
        })
    }

    /// Scrolls the open panel to `offset` lines — the wheel and its bar; its
    /// cursor stays where it is, as a list's does under the wheel.
    fn panel_to(&mut self, area: Area, offset: usize) {
        let Some(lay) = self.panel_lay(area) else {
            return;
        };
        if let Some(p) = self.panel.as_mut() {
            p.offset = offset.min(lay.lines.len().saturating_sub(usize::from(lay.shown)));
        }
    }

    /// How many lines the open panel takes — none when it is shut.
    fn panel_lines(&self, area: Area) -> u16 {
        self.panel_lay(area).map_or(0, |l| l.shown)
    }

    /// The choice under `(x, y)`, from the layout the drawing uses.
    fn choice_at(&self, area: Area, x: f32, y: f32) -> Option<usize> {
        let lay = self.panel_lay(area)?;
        self.panel.as_ref()?;
        lay.choice_at(x, y)
    }

    /// The panel, drawn: the choice under the cursor in the accent, the one
    /// in use in the title's ink, the rest in the ink of what acts — three
    /// questions, three answers — and a pointer's choice lit on its cell.
    fn draw_panel(&self, area: Area, lit: &[(tokens::Color, Target)], out: &mut Drawing) {
        let (Some(lay), Some(panel)) = (self.panel_lay(area), self.panel.as_ref()) else {
            return;
        };
        let active = self.active_choice(panel);
        let size = ronoctua_render::Size::Px(tokens::text::size());
        let room = lay.step.saturating_sub(tokens::space::ICONS_PANELGAP);
        for (i, l) in lay
            .lines
            .iter()
            .enumerate()
            .skip(lay.offset)
            .take(usize::from(lay.shown))
        {
            let line = lay.top + u16::try_from(i - lay.offset).unwrap_or(0);
            match l {
                PanelLine::Air => {}
                PanelLine::Heading => out.spans.push(ronoctua_render::Span {
                    size,
                    text: ronoctua_render::layout::elide_end(&panel.heading, area.columns),
                    column: lay.left,
                    line,
                    color: tokens::color::TEXT_HEADING,
                    edge: false,
                    foot: false,
                }),
                PanelLine::Choices(v) => {
                    for (k, n) in v.iter().enumerate() {
                        let column = lay.left + u16::try_from(k).unwrap_or(0) * lay.step;
                        if let Some((colour, _)) =
                            lit.iter().find(|(_, t)| *t == Target::Choice(*n))
                            && let Some((x, y, w, h)) = lay.shelf(line, v.len()).cell_box(k)
                        {
                            out.under.push(ronoctua_render::quad::Quad {
                                x,
                                y,
                                width: w,
                                height: h,
                                color: *colour,
                            });
                        }
                        let color = if *n == panel.cursor {
                            crate::chrome::Kind::Setting.tones(true).0
                        } else if Some(*n) == active {
                            crate::chrome::Register::Title.color()
                        } else {
                            crate::chrome::Kind::Setting.tones(false).0
                        };
                        out.spans.push(ronoctua_render::Span {
                            size,
                            text: ronoctua_render::layout::elide_end(
                                &panel.choices[*n].label,
                                room,
                            ),
                            column,
                            line,
                            color,
                            edge: false,
                            foot: false,
                        });
                    }
                }
            }
        }
        if let Some((track, thumb)) = self.choices_bar(area) {
            out.tracks.push(crate::bars::Track::of(track));
            if let Some(t) = thumb {
                out.thumbs.push(crate::chrome::thumb(t));
            }
        }
    }

    // ── THE GRID ──────────────────────────────────────────────────────────

    /// The area under the search line and the open panel, where the grid or
    /// the page is.
    #[must_use]
    pub fn below_the_explanation(&self, area: Area) -> Area {
        area.below(1 + self.panel_lines(area))
    }

    /// The grid in `area` (the view's whole area), leaving its bar a cell
    /// when the hits do not fit.
    fn grid(&self, area: Area) -> ronoctua_design::Grid {
        let a = self.below_the_explanation(area);
        let down = tokens::space::ICONS_GRIDSCALE;
        let fit = |columns: u16| {
            ronoctua_design::Grid::fitted(columns, self.item_cells(), a.top, a.top + a.lines, down)
        };
        let g = fit(a.columns);
        if g.fit() < self.hits().len() {
            fit(a
                .columns
                .saturating_sub(crate::chrome::BAR_CELLS + ronoctua_design::GAP))
        } else {
            g
        }
    }

    /// Where the grid's bar is: its track and its thumb, when the hits do not
    /// fit.
    fn grid_bar(&self, area: Area) -> Option<(crate::chrome::Rect, Option<crate::chrome::Rect>)> {
        let g = self.grid(area);
        let s = &self.screens[slot(self.screen)];
        let scroll = g.scroll(s.hits.len(), s.top);
        let a = self.below_the_explanation(area);
        scroll.cut().then(|| {
            crate::chrome::band_bar(
                scroll,
                area.right_px,
                Area::y_of(a.top),
                area.bottom_px,
                Area::cell(),
                Area::line() * f32::from(g.down),
            )
        })
    }

    /// The grid in the area, as a hover, the cursor and a click read it
    /// (`chrome::Shelves`): each item's cell is its ink and half the air on
    /// each side, and the item under a point is the grid's own arithmetic in
    /// the other direction.
    fn shelves(g: ronoctua_design::Grid, area: Area) -> crate::chrome::Shelves {
        crate::chrome::Shelves {
            grid: g,
            left: area.left,
            air: ronoctua_design::GAP,
        }
    }

    /// What is under `(x, y)` — the one hit test both doors ask, from the
    /// layout the drawing uses.
    #[must_use]
    pub fn target_at(
        &self,
        area: Area,
        texts: &ronoctua_i18n::Texts,
        x: f32,
        y: f32,
    ) -> Option<Target> {
        if y < Area::y_of(area.top) || y >= area.bottom_px {
            return None;
        }
        // THE SEARCH LINE: its one control is the filter mark.
        if y < Area::y_of(area.top + 1) {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let column = (x / Area::cell()).max(0.0) as u16;
            return self
                .search_marks(area, texts)
                .into_iter()
                .find(|m| m.covers(area.top, column))
                .and_then(|m| (m.id == FILTERS_ID).then_some(Target::Funnel));
        }
        if let Some(page) = &self.page {
            let set = self.set.as_ref()?;
            let (_, _, lines) = page.list(Self::page_area(area), set, &self.settings, texts);
            return page
                .hit(Self::page_area(area), &lines, x, y)
                .map(Target::Page);
        }
        if let Some((track, _)) = self.choices_bar(area)
            && crate::bars::Track::of(track).holds(f64::from(x), f64::from(y))
        {
            return Some(Target::PanelBar);
        }
        if let Some(n) = self.choice_at(area, x, y) {
            return Some(Target::Choice(n));
        }
        if y < Area::y_of(self.below_the_explanation(area).top) {
            return None;
        }
        if let Some((track, _)) = self.grid_bar(area)
            && crate::bars::Track::of(track).holds(f64::from(x), f64::from(y))
        {
            return Some(Target::GridBar);
        }
        let g = self.grid(area);
        let s = &self.screens[slot(self.screen)];
        let shown = s.hits.len().saturating_sub(s.top).min(g.fit());
        Self::shelves(g, area)
            .index_at(x, y)
            .filter(|k| *k < shown)
            .map(|k| Target::Cell(s.top + k))
    }

    /// A press of the first button on `target`: the first click selects, the
    /// one on the cursor acts (invariant 19); a bar is grabbed.
    pub fn pressed(
        &mut self,
        target: Target,
        area: Area,
        y: f32,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        match target {
            Target::Cell(n) => {
                let hit = self.hits().get(n).copied();
                if hit.is_some() && hit == self.cursor() {
                    self.open_page(area, texts);
                } else {
                    self.cursor_to(n, area);
                }
                Errand::Nothing
            }
            Target::Funnel => {
                self.toggle_panel(area, texts);
                Errand::Nothing
            }
            Target::Choice(n) => {
                let on_it = self.panel.as_ref().is_some_and(|p| p.cursor == n);
                if on_it {
                    self.choose(n, texts);
                } else if let Some(p) = self.panel.as_mut() {
                    p.cursor = n;
                }
                Errand::Nothing
            }
            Target::GridBar => {
                self.grabbed = Some(Grab::Grid);
                self.dragged(area, y, texts);
                Errand::Nothing
            }
            Target::PanelBar => {
                self.grabbed = Some(Grab::Panel);
                self.dragged(area, y, texts);
                Errand::Nothing
            }
            Target::Page(crate::icons_page::Target::Bar) => {
                self.grabbed = Some(Grab::List);
                self.dragged(area, y, texts);
                Errand::Nothing
            }
            Target::Page(crate::icons_page::Target::Stage) => Errand::Nothing,
            Target::Page(crate::icons_page::Target::Row(n)) => {
                let Some(set) = self.set.clone() else {
                    return Errand::Nothing;
                };
                let Some(page) = self.page.as_mut() else {
                    return Errand::Nothing;
                };
                let errand = page.clicked(n, &set, &mut self.settings, texts);
                self.page_errand(errand, &set, area, texts)
            }
        }
    }

    /// A held bar followed the pointer to `y`.
    pub fn dragged(&mut self, area: Area, y: f32, texts: &ronoctua_i18n::Texts) {
        match self.grabbed {
            Some(Grab::Grid) => {
                let g = self.grid(area);
                let a = self.below_the_explanation(area);
                let top = Area::y_of(a.top);
                let span = (area.bottom_px - top).max(1.0);
                let s = &mut self.screens[slot(self.screen)];
                let rows = g.rows_of(s.hits.len());
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    clippy::cast_precision_loss
                )]
                let row = (((y - top) / span).clamp(0.0, 1.0) * rows as f32) as usize;
                s.top = g.top_at_row(row, s.hits.len());
            }
            Some(Grab::Panel) => {
                let Some(lay) = self.panel_lay(area) else {
                    return;
                };
                let top = Area::y_of(lay.top);
                let span = (Area::y_of(lay.top + lay.shown) - top).max(1.0);
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    clippy::cast_precision_loss
                )]
                let at = (((y - top) / span).clamp(0.0, 1.0) * lay.lines.len() as f32) as usize;
                self.panel_to(area, at);
            }
            Some(Grab::List) => {
                let Some(set) = self.set.clone() else {
                    return;
                };
                if let Some(page) = self.page.as_mut() {
                    let a = Self::page_area(area);
                    let total = page.lines_of(a, &set, &self.settings, texts);
                    page.drag_to(a, total, y);
                }
            }
            None => {}
        }
    }

    /// The wheel turned `by` lines at height `y` — the page's list, the
    /// panel's lines when it is over them, or the grid's rows.
    pub fn wheeled(&mut self, by: i64, area: Area, y: f32, texts: &ronoctua_i18n::Texts) {
        if let Some(page) = self.page.as_mut() {
            let Some(set) = self.set.clone() else {
                return;
            };
            let a = Self::page_area(area);
            let total = page.lines_of(a, &set, &self.settings, texts);
            let lay = crate::icons_page::Layout::of(a);
            page.wheeled(by, total, lay.list_lines);
            return;
        }
        if let Some(lay) = self.panel_lay(area)
            && y >= Area::y_of(lay.top)
            && y < Area::y_of(lay.top + lay.shown)
        {
            let to = i64::try_from(lay.offset)
                .unwrap_or(0)
                .saturating_add(by)
                .max(0);
            self.panel_to(area, usize::try_from(to).unwrap_or(0));
            return;
        }
        let g = self.grid(area);
        let s = &mut self.screens[slot(self.screen)];
        s.top = g.wheeled(s.top, by, s.hits.len());
    }

    /// The pieces of the footer's tab line: the three tabs, each its key and
    /// its name — the one showing in the title's ink. In a window they lead
    /// the header's line.
    #[must_use]
    pub fn tab_pieces(&self, texts: &ronoctua_i18n::Texts) -> Vec<Piece> {
        let mut out: Vec<Piece> = Vec::new();
        for (notation, c, id) in TABS {
            let Ok(key) = ronoctua_keymap::Key::parse(notation) else {
                continue;
            };
            if !out.is_empty() {
                out.push(Piece::separator());
            }
            out.push(
                Piece::key_in(crate::chrome::key_text(&key), crate::chrome::Register::Key)
                    .about(id)
                    .pressing(key),
            );
            let name = format!(" {}", texts.t(&format!("icons-screen-{}", c.id())));
            let register = if c == self.screen {
                crate::chrome::Register::Title
            } else {
                crate::chrome::Register::Prose
            };
            out.push(Piece::key_in(name, register).about(id).pressing(key));
        }
        out
    }

    /// The search line: what is typed — or what typing does —, how much it
    /// found, the filter mark (lit while a filter is in use, a click that
    /// opens the panel), and then what the icon is.
    #[must_use]
    pub fn search_pieces(&self, texts: &ronoctua_i18n::Texts) -> Vec<Piece> {
        let mut out: Vec<Piece> = Vec::new();
        let query = self.query();
        if query.is_empty() {
            out.push(Piece::prose(
                texts.t(&format!("icons-search-{}", self.screen.id())),
            ));
        } else {
            out.push(Piece::said(query.to_owned()));
        }
        if let Some(set) = &self.set {
            out.push(Piece::prose(format!(
                "  {}",
                texts.t_args(
                    crate::key::ICONS_COUNT,
                    &[
                        (
                            "shown",
                            ronoctua_i18n::Arg::Number(
                                i64::try_from(self.hits().len()).unwrap_or(0)
                            ),
                        ),
                        (
                            "total",
                            ronoctua_i18n::Arg::Number(
                                i64::try_from(set.len(self.screen)).unwrap_or(0),
                            ),
                        ),
                    ],
                )
            )));
            if let Ok(key) = ronoctua_keymap::Key::parse(FILTERS) {
                out.push(Piece::prose(" "));
                let register = if self.filters().is_empty() {
                    crate::chrome::Register::Key
                } else {
                    crate::chrome::Register::Glyph
                };
                out.push(
                    Piece::key_in(tokens::icon::FILTER.to_string(), register)
                        .about(FILTERS_ID)
                        .pressing(key),
                );
            }
        }
        let said = self.explanation(texts);
        if !said.is_empty() {
            out.push(Piece::separator());
            out.extend(said);
        }
        out
    }

    /// Where the search line's pieces land, in `area` — the SAME placement
    /// its drawing makes, asked again by the pointer.
    fn search_marks(&self, area: Area, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Mark> {
        crate::chrome::hint_marks(
            &self.search_pieces(texts),
            area.left,
            area.top,
            area.columns,
        )
    }

    /// The system's shortcut line for this view: the page's keys, the
    /// panel's or the grid's, and `Esc` first.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<Piece> {
        let pieces = match (self.page.as_ref(), self.set.as_ref()) {
            (Some(page), Some(set)) => {
                let changes = page.enter_changes(set, &self.settings, texts);
                crate::chrome::hint_of_with(
                    OF_THE_PAGE,
                    |id| (changes && id == "icons-enter").then_some("icons-change"),
                    texts,
                )
            }
            (Some(_), None) => crate::chrome::hint_of(OF_THE_PAGE, texts),
            (None, _) if self.panel.is_some() => crate::chrome::hint_of(OF_THE_PANEL, texts),
            (None, _) => crate::chrome::hint_of(OF_THE_GRID, texts),
        };
        crate::chrome::with_the_way_back(pieces, texts)
    }

    /// What the search line says after the count: the sentence about the
    /// control under the pointer (the filter mark; the footer's tabs), or the
    /// icon the pointer rests on — or, after a key, the cursor's — its name
    /// and its tags, or why there is none.
    #[must_use]
    pub fn explanation(&self, texts: &ronoctua_i18n::Texts) -> Vec<Piece> {
        // A SENTENCE ONLY WHILE THE POINTER IS WHAT MOVED LAST: after a key
        // the line names what the key moved, and a control that slid from
        // under a resting pointer (the search grew) is not explained there.
        let about = match (self.hover, self.tab_lit) {
            _ if self.keyed => None,
            (Some(Target::Funnel), _) => Some(FILTERS_ID),
            (_, Some(id)) => Some(id),
            _ => None,
        };
        if let Some(said) = about.and_then(|id| crate::chrome::hint_sentence(id, texts)) {
            return vec![Piece::key_in(said, crate::chrome::Register::Explain)];
        }
        let Some(set) = self.set.as_ref() else {
            let said = self.refused.map_or_else(
                || texts.t(crate::key::ICONS_LOADING),
                |r| {
                    texts.t_with(
                        &format!("icons-why-{}", r.why.id()),
                        &[("file", r.file.unwrap_or_default())],
                    )
                },
            );
            return vec![Piece::said(said)];
        };
        if self.hits().is_empty() {
            return vec![Piece::said(
                texts.t_with(crate::key::ICONS_NONE, &[("query", self.query())]),
            )];
        }
        let shown = match (self.page.as_ref(), self.hover) {
            (Some(p), _) => Some(p.icon.index),
            (None, Some(Target::Cell(n))) if !self.keyed => self.hits().get(n).copied(),
            _ => self.cursor(),
        };
        let Some(index) = shown else {
            return Vec::new();
        };
        let subtag = &self.subtag;
        let (name, tags) = match self.screen {
            Collection::NerdFonts => set.nerd.get(index).map(|g| {
                (
                    // A GLYPH NO TABLE NAMES is said by its codepoint.
                    g.name
                        .clone()
                        .unwrap_or_else(|| crate::icons_page::hex(g.cp)),
                    crate::icons_page::tags_of(&g.tags, subtag),
                )
            }),
            Collection::Lucide => set
                .lucide
                .get(index)
                .map(|f| (f.name.clone(), crate::icons_page::tags_of(&f.tags, subtag))),
            Collection::Unicode => set
                .unicode
                .get(index)
                .map(|c| (c.name.clone(), crate::icons_page::tags_of(&c.tags, subtag))),
        }
        .unwrap_or_default();
        let mut out = vec![Piece::said(name)];
        if !tags.is_empty() {
            out.push(Piece::separator());
            out.push(Piece::key_in(tags, crate::chrome::Register::Explain));
        }
        out
    }

    /// The view, drawn in `area`: the tab line first when `tabs` (the
    /// footer's region — the window's is its header), the search line, the
    /// open panel, then the grid or the page. `lit` is every pointer's
    /// target, in its colour.
    pub fn draw(
        &self,
        area: Area,
        tabs: bool,
        lit: &[(tokens::Color, Target)],
        texts: &ronoctua_i18n::Texts,
        out: &mut Drawing,
    ) {
        let size = ronoctua_render::Size::Px(tokens::text::size());
        let mut area = area;
        if tabs {
            let pieces = crate::chrome::lit(&self.tab_pieces(texts), self.tab_lit);
            out.spans.extend(crate::chrome::hint_spans(
                &pieces,
                size,
                area.left,
                area.top,
                area.columns,
            ));
            area = area.below(1);
        }
        let funnel_lit = lit.iter().any(|(_, t)| *t == Target::Funnel);
        let pieces =
            crate::chrome::lit(&self.search_pieces(texts), funnel_lit.then_some(FILTERS_ID));
        out.spans.extend(crate::chrome::hint_spans(
            &pieces,
            size,
            area.left,
            area.top,
            area.columns,
        ));
        if let (Some(set), Some(page)) = (self.set.as_ref(), self.page.as_ref()) {
            let rows: Vec<(tokens::Color, usize)> = lit
                .iter()
                .filter_map(|(c, t)| match t {
                    Target::Page(crate::icons_page::Target::Row(n)) => Some((*c, *n)),
                    _ => None,
                })
                .collect();
            page.draw(
                Self::page_area(area),
                set,
                &self.settings,
                texts,
                &rows,
                out,
            );
            return;
        }
        self.draw_panel(area, lit, out);
        self.draw_grid(area, lit, out);
    }

    /// Where the footer's tab line puts each tab, in `area` (the view's
    /// whole area, the tab line its first) — the SAME placement its drawing
    /// makes (`chrome::hint_marks` over the pieces `chrome::hint_spans`
    /// draws), asked again by the pointer.
    fn tab_marks(&self, area: Area, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Mark> {
        crate::chrome::hint_marks(&self.tab_pieces(texts), area.left, area.top, area.columns)
    }

    /// The tab under `(column, line)` of the footer's tab line, as the key a
    /// click presses there.
    #[must_use]
    pub fn tab_press_at(
        &self,
        area: Area,
        texts: &ronoctua_i18n::Texts,
        column: u16,
        line: u16,
    ) -> Option<crate::chrome::Act> {
        self.tab_marks(area, texts)
            .into_iter()
            .find(|m| m.covers(line, column))
            .and_then(|m| m.press)
    }

    /// The id of the tab under `(column, line)` of the footer's tab line.
    #[must_use]
    pub fn tab_at(
        &self,
        area: Area,
        texts: &ronoctua_i18n::Texts,
        column: u16,
        line: u16,
    ) -> Option<&'static str> {
        self.tab_marks(area, texts)
            .into_iter()
            .find(|m| m.covers(line, column))
            .map(|m| m.id)
    }

    /// The grid: every item shown, the cursor's cell, the cells under the
    /// pointers and the bar.
    fn draw_grid(&self, area: Area, lit: &[(tokens::Color, Target)], out: &mut Drawing) {
        let Some(set) = self.set.as_ref() else {
            return;
        };
        let g = self.grid(area);
        let s = &self.screens[slot(self.screen)];
        let scale = tokens::space::ICONS_GRIDSCALE;
        let px = tokens::text::size() * f32::from(scale);
        let item = self.item_cells();
        let bar = self.grid_bar(area);
        for (k, record) in s.hits.iter().skip(s.top).take(g.fit()).enumerate() {
            let Some((line, column)) = g.at(k) else {
                break;
            };
            let n = s.top + k;
            let cell = Self::shelves(g, area).cell_box(k);
            // THE CURSOR'S CELL, then a pointer's — never over the cursor's:
            // the two are different marks, and the cursor is the keyboard's.
            if let Some((x, y, w, h)) = cell {
                if s.cursor == Some(*record) {
                    out.under.push(ronoctua_render::quad::Quad {
                        x,
                        y,
                        width: w,
                        height: h,
                        color: tokens::color::SURFACE_SELECTION,
                    });
                } else if let Some((colour, _)) = lit.iter().find(|(_, t)| *t == Target::Cell(n)) {
                    out.under.push(ronoctua_render::quad::Quad {
                        x,
                        y,
                        width: w,
                        height: h,
                        color: *colour,
                    });
                }
            }
            let (text, ink) = match self.screen {
                Collection::NerdFonts => (
                    set.nerd
                        .get(*record)
                        .and_then(|g| char::from_u32(g.cp))
                        .map(String::from),
                    scale,
                ),
                Collection::Lucide => (
                    set.lucide
                        .get(*record)
                        .and_then(|f| char::from_u32(f.cp))
                        .map(String::from),
                    scale,
                ),
                Collection::Unicode => set
                    .unicode
                    .get(*record)
                    .map_or((None, scale), |c| (Some(c.drawn()), c.ink(scale))),
            };
            let Some(text) = text else { continue };
            // A NARROWER INK IS CENTRED IN ITS ITEM: a one-cell character in
            // a column budgeted for two.
            let inset = item.saturating_sub(ink) / 2;
            out.spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Px(px),
                text,
                column: area.left + column + inset,
                line,
                color: tokens::color::TEXT_PRIMARY,
                edge: false,
                foot: false,
            });
        }
        if let Some((track, thumb)) = bar {
            out.tracks.push(crate::bars::Track::of(track));
            if let Some(t) = thumb {
                out.thumbs.push(crate::chrome::thumb(t));
            }
        }
    }

    /// The tables it holds.
    #[cfg(test)]
    fn set(&self) -> Option<&Arc<Set>> {
        self.set.as_ref()
    }
}

/// What a screen's field holds when it opens: Unicode's `symbols:`, in the
/// reader's word; the others' nothing.
fn opening(screen: Collection, texts: &ronoctua_i18n::Texts) -> String {
    match screen {
        Collection::Unicode => format!("{}:", texts.t(crate::key::ICONS_PREFIX_SYMBOLS)),
        Collection::NerdFonts | Collection::Lucide => String::new(),
    }
}

/// Reads the tables on a thread and hands the answer to `back` — the one
/// way a door gets them: ~9 MB never read on an event loop.
pub fn read_the_tables(back: impl FnOnce(Result<Arc<Set>, Refusal>) + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("icons-tables".to_owned())
        .spawn(move || back(ronoctua_iconset::shared()));
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "could not start the thread that reads the icon tables");
    }
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "icons",
    commands: &[
        // THE ICONS APP (`crate::icons`): the footer's region and the window,
        // a screen each, and the Lucide colour's question.
        ronoctua_cmd::Command {
            id: "icons-open",
            label: crate::key::CMD_ICONS_OPEN,
        },
        ronoctua_cmd::Command {
            id: "icons-window",
            label: crate::key::CMD_ICONS_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "icons-nerd-fonts",
            label: crate::key::CMD_ICONS_NERD_FONTS,
        },
        ronoctua_cmd::Command {
            id: "icons-lucide",
            label: crate::key::CMD_ICONS_LUCIDE,
        },
        ronoctua_cmd::Command {
            id: "icons-unicode",
            label: crate::key::CMD_ICONS_UNICODE,
        },
        ronoctua_cmd::Command {
            id: "icons-colour",
            label: crate::key::CMD_ICONS_COLOUR,
        },
    ],
    bindings: &[
        // `i` for ICONS, and not `g` for glyphs: the group holds the three
        // collections. `m i i` and `m i w` are the two doors, so the group
        // folds into `LEADER m i`; a screen of its own is `m i n|l|u`, in the
        // whole tree.
        ("m i i", "icons-open", None),
        ("m i w", "icons-window", None),
        ("m i n", "icons-nerd-fonts", None),
        ("m i l", "icons-lucide", None),
        ("m i u", "icons-unicode", None),
    ],
    groups: &[("m i", crate::key::GROUP_ICONS)],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::icons::reached().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the English words")
    }

    fn a_set() -> Arc<Set> {
        let nerd = (0..40u32)
            .map(|i| ronoctua_iconset::Nerd {
                cp: 0xF0000 + i,
                name: Some(format!("md-thing_{i:02}")),
                names: vec![format!("md-thing_{i:02}")],
                sets: vec!["md".to_owned()],
                ..Default::default()
            })
            .collect();
        let lucide = vec![ronoctua_iconset::Lucide {
            name: "house".to_owned(),
            cp: 0xF2000,
            lcp: 0xE0F5,
            ..Default::default()
        }];
        let unicode = vec![ronoctua_iconset::Char {
            cp: 0x2605,
            seq: vec![0x2605],
            shows: vec![0x2605],
            name: "BLACK STAR".to_owned(),
            family: "symbol".to_owned(),
            width: 1,
            ..Default::default()
        }];
        Arc::new(Set::new(nerd, lucide, unicode))
    }

    fn area(columns: u16, lines: u16) -> Area {
        Area {
            left: 1,
            top: 1,
            columns,
            lines,
            right_px: Area::x_of(columns + 2),
            bottom_px: Area::y_of(lines + 1),
        }
    }

    fn a_view() -> View {
        let t = texts();
        let mut v = View::new(Collection::NerdFonts, &t);
        v.took(Ok(a_set()));
        v
    }

    /// FAILS IF: a tab's key does not show its screen, or `C-TAB` does not
    /// walk the three and come back.
    #[test]
    fn the_tabs_are_their_keys() {
        let t = texts();
        let mut v = a_view();
        let a = area(40, 20);
        for (notation, c, _) in TABS {
            let key = ronoctua_keymap::Key::parse(notation).expect("a key");
            let p = press_of_key(key, false).expect("a press");
            v.key(p, a, &t);
            assert_eq!(v.screen(), c);
        }
        let next = press_of_key(
            ronoctua_keymap::Key::parse(NEXT_SCREEN).expect("C-TAB"),
            false,
        )
        .expect("a press");
        v.key(next, a, &t);
        assert_eq!(v.screen(), Collection::NerdFonts);
    }

    /// FAILS IF: what is drawn at a cell is not what a click there takes —
    /// the grid's hover and click read the layout the drawing used.
    #[test]
    fn a_cell_is_taken_where_it_is_drawn() {
        let t = texts();
        let v = a_view();
        let a = area(30, 12);
        let mut out = Drawing::default();
        v.draw(a, false, &[], &t, &mut out);
        let set = v.set().expect("the set").clone();
        // EVERY ICON DRAWN — taken from the drawing, not from the layout the
        // hit test also reads — is found under the middle of its ink.
        let px = tokens::text::size() * f32::from(tokens::space::ICONS_GRIDSCALE);
        let icons: Vec<&ronoctua_render::Span> = out
            .spans
            .iter()
            .filter(|s| matches!(s.size, ronoctua_render::Size::Px(p) if (p - px).abs() < 0.01))
            .collect();
        assert!(icons.len() > 10, "{} icons drawn", icons.len());
        for s in icons {
            let x = Area::x_of(s.column) + Area::cell() / 2.0;
            let y = Area::y_of(s.line) + Area::line() / 2.0;
            let Some(Target::Cell(n)) = v.target_at(a, &t, x, y) else {
                panic!("nothing is taken under {:?} at {x},{y}", s.text);
            };
            let record = v.hits()[n];
            let drawn = char::from_u32(set.nerd[record].cp).map(String::from);
            assert_eq!(drawn.as_deref(), Some(s.text.as_str()), "cell {n}");
        }
    }

    /// FAILS IF: a point where two cells meet is in neither — the middle of
    /// a surface's cell is where two items' boxes meet, and a pointer resting
    /// there lit nothing.
    #[test]
    fn where_two_cells_meet_one_takes_the_pointer() {
        let t = texts();
        let v = a_view();
        let a = area(60, 12);
        let g = v.grid(a);
        for k in 0..usize::from(g.across.saturating_sub(1)) {
            let (bx, by, bw, bh) = View::shelves(g, a).cell_box(k).expect("a cell");
            let seam = bx + bw;
            // THE POINTER'S OWN ARITHMETIC: the middle of a surface cell, in
            // f64 and then f32, as the photograph and a door hand it in.
            let x = seam;
            assert!(
                v.target_at(a, &t, x, by + bh / 2.0).is_some(),
                "the seam after item {k}, at {x}"
            );
        }
    }

    /// FAILS IF: the page opens with its cursor off the lines a short door
    /// shows — the first row that acts is below every fact.
    #[test]
    fn the_page_opens_on_its_cursor() {
        let t = texts();
        for lines in [6u16, 8, 12] {
            let mut v = a_view();
            let a = area(80, lines);
            v.key(Press::Open, a, &t);
            let set = v.set().expect("the set").clone();
            let page = v.page.clone().expect("the page opened");
            let pa = View::page_area(a);
            let (lay, _, shown) = page.list(pa, &set, &v.settings, &t);
            let at = shown
                .iter()
                .position(|s| s.row == page.cursor)
                .expect("the cursor's row is laid out");
            assert!(
                at >= page.offset && at < page.offset + usize::from(lay.list_lines),
                "{lines} lines: the cursor is line {at}, the list shows {}..{}",
                page.offset,
                page.offset + usize::from(lay.list_lines)
            );
            // AND IT IS DRAWN in the cursor's colour.
            let mut out = Drawing::default();
            v.draw(a, false, &[], &t, &mut out);
            assert!(
                out.spans
                    .iter()
                    .any(|s| s.color == crate::chrome::Kind::Setting.tones(true).0),
                "{lines} lines: no run in the cursor's colour"
            );
        }
    }

    /// FAILS IF: a choice of the panel does not write its prefix into the
    /// search, loses the words typed, or leaves the panel open.
    #[test]
    fn a_choice_writes_its_filter_and_keeps_the_words() {
        let t = texts();
        let mut v = a_view();
        let a = area(60, 20);
        for c in "thing".chars() {
            v.key(Press::Letter(c), a, &t);
        }
        v.key(Press::Filters, a, &t);
        assert!(v.in_panel());
        assert_eq!(
            v.panel.as_ref().map(|p| p.cursor),
            Some(0),
            "no filter: the panel opens on all"
        );
        v.key(Press::Move(1, 0), a, &t);
        v.key(Press::Open, a, &t);
        assert!(!v.in_panel(), "choosing shuts the panel");
        assert_eq!(v.query(), "md:thing");
        assert_eq!(v.hits().len(), 40, "every glyph is md and a thing");
        // THE MARK IS LIT while a filter is in use.
        let lit = |v: &View| {
            v.search_pieces(&t)
                .iter()
                .any(|p| p.id == Some(FILTERS_ID) && p.register == crate::chrome::Register::Glyph)
        };
        assert!(lit(&v));
        // AND "ALL" TAKES THE FILTER OFF, the words staying.
        v.key(Press::Filters, a, &t);
        assert_eq!(
            v.panel.as_ref().map(|p| p.cursor),
            Some(1),
            "it opens on md"
        );
        v.key(Press::Move(-1, 0), a, &t);
        v.key(Press::Open, a, &t);
        assert_eq!(v.query(), "thing");
        assert!(!lit(&v));
    }

    /// FAILS IF: a Unicode family replaces the opening `symbols:` wrongly, or
    /// the emoji's section is not there.
    #[test]
    fn a_family_replaces_the_opening_filter() {
        let t = texts();
        let mut v = a_view();
        let a = area(60, 30);
        v.show(Collection::Unicode);
        v.key(Press::Filters, a, &t);
        let panel = v.panel.clone().expect("the panel");
        assert!(panel.split < panel.choices.len(), "the emoji's section");
        let letter = panel
            .choices
            .iter()
            .position(|c| c.filter == Some((FilterKind::Family, "letter".to_owned())))
            .expect("the letters");
        let choice = panel.choices[letter].clone();
        assert_eq!(v.with_choice(&choice, &t), "family:letter");
        v.key(Press::Letter('x'), a, &t);
        assert_eq!(v.with_choice(&choice, &t), "family:letter x");
    }

    /// FAILS IF: the panel's cell is not taken where it is drawn, or the
    /// first click on a choice chooses instead of selecting (invariant 19).
    #[test]
    fn a_choice_is_taken_where_it_is_drawn() {
        let t = texts();
        let mut v = a_view();
        let a = area(60, 20);
        v.key(Press::Filters, a, &t);
        let lay = v.panel_lay(a).expect("the panel is laid out");
        let (x, y, w, h) = lay.cell_box(1).expect("md is shown");
        let target = v.target_at(a, &t, x + w / 2.0, y + h / 2.0);
        assert_eq!(target, Some(Target::Choice(1)));
        v.pressed(Target::Choice(1), a, y, &t);
        assert!(v.in_panel(), "the first click selects");
        v.pressed(Target::Choice(1), a, y, &t);
        assert!(!v.in_panel());
        assert_eq!(v.query(), "md:");
    }

    /// FAILS IF: a panel cut by a short door has no bar, its bar is not taken
    /// where it is drawn, or the wheel over it and a drag of its bar do not
    /// scroll it — what the arrows reach, the pointer reaches (invariant 19).
    #[test]
    fn a_cut_panel_scrolls_by_the_pointer() {
        let t = texts();
        let mut v = a_view();
        let a = area(40, 8);
        v.show(Collection::Unicode);
        v.key(Press::Filters, a, &t);
        let lay = v.panel_lay(a).expect("the panel");
        assert!(lay.lines.len() > usize::from(lay.shown), "the panel is cut");
        let mut out = Drawing::default();
        v.draw(a, false, &[], &t, &mut out);
        let track = out.tracks.first().expect("the panel draws a bar");
        let (x, y) = (track.x + track.width / 2.0, track.y + Area::line() / 2.0);
        assert_eq!(v.target_at(a, &t, x, y), Some(Target::PanelBar));
        // NO CHOICE UNDER THE BAR, at any width: its column is its own.
        for columns in 30..=90 {
            let mut w = a_view();
            let wa = area(columns, 8);
            w.show(Collection::Unicode);
            w.key(Press::Filters, wa, &t);
            let lay = w.panel_lay(wa).expect("the panel");
            let (track, _) = w.choices_bar(wa).expect("a cut panel has a bar");
            for n in 0..w.panel.as_ref().map_or(0, |p| p.choices.len()) {
                if let Some((bx, _, bw, _)) = lay.cell_box(n) {
                    assert!(
                        bx + bw <= track.0 + 1e-3,
                        "{columns} columns: choice {n} ends at {} under the bar at {}",
                        bx + bw,
                        track.0
                    );
                }
            }
        }
        v.wheeled(2, a, Area::y_of(lay.top), &t);
        assert_eq!(v.panel_lay(a).map(|l| l.offset), Some(2), "the wheel");
        v.pressed(Target::PanelBar, a, track.y, &t);
        assert_eq!(v.panel_lay(a).map(|l| l.offset), Some(0), "the bar's top");
        v.grabbed = None;
        // AND THE WHEEL BELOW THE PANEL STILL MOVES THE GRID.
        let below = Area::y_of(lay.top + lay.shown);
        v.wheeled(1, a, below, &t);
        assert_eq!(v.panel_lay(a).map(|l| l.offset), Some(0));
    }

    /// FAILS IF: a tab or the filter mark under the footer's pointer does not
    /// say what it does on the line under the tabs — the window's header says
    /// it — or the sentence outlives a key.
    #[test]
    fn a_control_under_the_pointer_explains_itself() {
        let t = texts();
        let mut v = a_view();
        let a = area(30, 12);
        let said = |v: &View| {
            v.explanation(&t)
                .first()
                .filter(|p| p.register == crate::chrome::Register::Explain)
                .map(|p| p.text.clone())
        };
        v.tab_hovered(Some("icons-tab-lucide"));
        assert_eq!(said(&v), Some(t.t("about-icons-tab-lucide")));
        v.key(Press::Move(1, 0), a, &t);
        assert_eq!(said(&v), None, "after a key the line names the cursor");
        v.tab_hovered(None);
        v.hovered(Some(Target::Funnel));
        assert_eq!(said(&v), Some(t.t("about-icons-filters")));
    }

    /// FAILS IF: after a key the search line still names the icon under a
    /// resting pointer rather than the one the keys moved to.
    #[test]
    fn a_key_names_the_cursor() {
        let t = texts();
        let mut v = a_view();
        let a = area(30, 12);
        let name = |v: &View| v.explanation(&t).first().map(|p| p.text.clone());
        v.hovered(Some(Target::Cell(5)));
        assert_eq!(name(&v).as_deref(), Some("md-thing_05"));
        v.key(Press::Move(1, 0), a, &t);
        assert_eq!(name(&v).as_deref(), Some("md-thing_01"));
        v.hovered(Some(Target::Cell(7)));
        assert_eq!(name(&v).as_deref(), Some("md-thing_07"));
    }

    /// FAILS IF: the grid does not begin on the line right under the
    /// explanation's — no rule, no air on top.
    #[test]
    fn the_grid_starts_right_under_the_explanation() {
        let t = texts();
        let v = a_view();
        let a = area(30, 12);
        let mut out = Drawing::default();
        v.draw(a, false, &[], &t, &mut out);
        let first = out
            .spans
            .iter()
            .filter(|s| matches!(s.size, ronoctua_render::Size::Px(p) if p > tokens::text::size()))
            .map(|s| s.line)
            .min()
            .expect("an icon was drawn");
        assert_eq!(first, a.top + 1);
    }

    /// FAILS IF: the first click on a cell does not select it, or the second
    /// does not open its page (invariant 19).
    #[test]
    fn the_second_click_opens_the_page() {
        let t = texts();
        let mut v = a_view();
        let a = area(30, 12);
        v.pressed(Target::Cell(3), a, 0.0, &t);
        assert_eq!(v.cursor(), Some(v.hits()[3]));
        assert!(!v.in_page());
        v.pressed(Target::Cell(3), a, 0.0, &t);
        assert!(v.in_page());
        // `Esc` closes the page before anything else.
        v.key(Press::Escape, a, &t);
        assert!(!v.in_page());
    }

    /// FAILS IF: `Esc` does not peel the query in layers — the words, the
    /// opening filter, then the view.
    #[test]
    fn escape_peels_the_unicode_filter() {
        let t = texts();
        let mut v = a_view();
        let a = area(30, 12);
        v.show(Collection::Unicode);
        let opening = v.query().to_owned();
        assert!(opening.ends_with(':'), "Unicode opens filtered: {opening}");
        v.key(Press::Letter('s'), a, &t);
        assert_eq!(v.key(Press::Escape, a, &t), Errand::Nothing);
        assert_eq!(v.query(), opening);
        assert_eq!(v.key(Press::Escape, a, &t), Errand::Nothing);
        assert_eq!(v.query(), "");
        assert_eq!(v.key(Press::Escape, a, &t), Errand::Close);
    }

    /// FAILS IF: moving down a row does not land a whole row further, or the
    /// grid does not follow the cursor off its last row.
    #[test]
    fn the_cursor_moves_by_rows_and_the_grid_follows() {
        let t = texts();
        let mut v = a_view();
        let a = area(30, 5);
        let g = v.grid(a);
        let across = usize::from(g.across);
        v.key(Press::Move(0, 1), a, &t);
        assert_eq!(v.cursor(), Some(v.hits()[across]));
        v.key(Press::End(true), a, &t);
        let s = &v.screens[slot(v.screen)];
        let last = s.hits.len() - 1;
        assert!(
            s.top <= last && last < s.top + g.fit(),
            "the last item is shown"
        );
        assert_eq!(s.top % across, 0, "the grid moves by whole rows");
    }

    /// FAILS IF: a typed word does not narrow the hits, and the cursor does
    /// not start on the best of them.
    #[test]
    fn typing_searches() {
        let t = texts();
        let mut v = a_view();
        let a = area(30, 5);
        for c in "thing_07".chars() {
            v.key(Press::Letter(c), a, &t);
        }
        assert!(!v.hits().is_empty());
        assert_eq!(
            v.set().expect("the set").nerd[v.cursor().expect("a cursor")]
                .name
                .as_deref(),
            Some("md-thing_07")
        );
        v.key(Press::Erase, a, &t);
        assert_eq!(v.query(), "thing_0");
    }

    /// FAILS IF: a target's code does not come back as the target — the
    /// window's hover travels as one.
    #[test]
    fn a_target_survives_its_code() {
        use crate::icons_page::Target as P;
        for t in [
            Target::Cell(0),
            Target::Cell(9_999),
            Target::GridBar,
            Target::Funnel,
            Target::Choice(0),
            Target::Choice(42),
            Target::PanelBar,
            Target::Page(P::Row(0)),
            Target::Page(P::Row(28)),
            Target::Page(P::Bar),
            Target::Page(P::Stage),
        ] {
            assert_eq!(Target::of_code(t.code()), t);
        }
    }

    /// FAILS IF: every key a line teaches does not have its words and its
    /// sentence in both languages.
    #[test]
    fn every_taught_key_has_words_and_a_sentence() {
        for lang in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(lang).expect("a language");
            for (_, id) in OF_THE_GRID
                .iter()
                .chain(OF_THE_PAGE)
                .chain(OF_THE_PANEL)
                .chain(&[("RET", "icons-change")])
            {
                assert!(t.has(id), "{lang}: {id}");
                assert!(t.has(&format!("about-{id}")), "{lang}: about-{id}");
            }
            for (_, c, id) in TABS {
                assert!(t.has(&format!("about-{id}")), "{lang}: about-{id}");
                assert!(t.has(&format!("icons-screen-{}", c.id())), "{lang}");
                assert!(t.has(&format!("icons-search-{}", c.id())), "{lang}");
            }
        }
    }
}
