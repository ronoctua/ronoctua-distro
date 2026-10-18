//! WHERE FAST-ACCESS DRAWS WHAT IT FOUND — the typed line, the results and
//! the detail beside them (or below), as ONE layout the drawing and the
//! pointer both read ([`Lay::of`]).
//!
//! # The area
//!
//! A door hands an `icons_page::Area` — the window's surface under its
//! header, the footer's region, the column under its control line — and the
//! view takes its first line for what is typed. Under it the results take
//! the left and the detail the right, apart by a divider (`chrome::divider`);
//! a press on the divider's cell swaps the detail between
//! `space.fastAccess.detailCells` and `detailWideCells`. When the results
//! would have less than `resultsMinCells` beside it, the detail goes BELOW
//! them, `belowLines` tall, apart by a rule, with a bar of its own — the file
//! manager's strip (`files::strip_fits`). A scope that is a grid, a card or a
//! list of prefixes takes the whole area (`Scope::whole`), and `C-w` gives
//! the detail the whole area.
//!
//! # Rows have shapes
//!
//! - A PAIR: a name, and a quieter line beside it (a folder, a channel, the
//!   keys that run a command — against the right edge).
//! - A STRIP: a page of icons of one collection, EXACTLY three strips (Nerd
//!   Fonts, Lucide, Unicode), icons only; `‹`/`›` at its ends only while
//!   icons are hidden past them — each a click that turns the page — and
//!   `←`/`→` walk it, turning the page at its edge.
//! - A GRID: videos as pictures with their title and channel under them,
//!   YouTube's way; the cell under the pointer lights alone and the cursor's
//!   cell wears `surface.selection` (a grid's rule).
//! - A CARD: a word's meanings, its synonyms, a translation — wrapped, never
//!   cut.
//!
//! Every control is a [`Target`], placed by the layout and taken by the
//! pointer where it was drawn (`a-bar-is-grabbed-where-it-is-drawn`,
//! `the-pointer-lights-what-the-click-takes`).

use ronoctua_iconset::Collection;
use ronoctua_tokens as tokens;

use crate::fast_access::{Card, Filled, Hit, Press, SectionId, View, What};
use crate::icons_page::Area;

/// A bar being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grab {
    /// The results'.
    Results,
    /// The detail's.
    Detail,
    /// One of a file's panels' bars (`fast_access_detail::FileBar`).
    File(crate::fields::Pane, crate::fields::Axis),
}

/// What is under a point, or what a click takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A hit, by its place among all hits.
    Hit(usize),
    /// The `‹` of a collection's strip.
    Earlier(usize),
    /// The `›` of a collection's strip.
    Later(usize),
    /// The divider between the results and the detail.
    Divider,
    /// The results' bar.
    ResultsBar,
    /// A choice of the web video's question.
    Choice(usize),
    /// Something of the detail's.
    Detail(crate::fast_access_detail::Target),
}

/// The tag bits of a target's code (`Hand::row_under_pointer` holds a
/// number).
const TAG: usize = 1 << 40;

impl Target {
    /// The target as one number, for a hand to keep.
    #[must_use]
    pub fn code(self) -> usize {
        match self {
            Self::Hit(n) => n,
            Self::Earlier(c) => TAG + c,
            Self::Later(c) => 2 * TAG + c,
            Self::Divider => 3 * TAG,
            Self::ResultsBar => 4 * TAG,
            Self::Choice(n) => 5 * TAG + n,
            Self::Detail(t) => 6 * TAG + t.code(),
        }
    }

    /// The target a number is.
    #[must_use]
    pub fn of_code(code: usize) -> Self {
        let (tag, n) = (code / TAG, code % TAG);
        match tag {
            0 => Self::Hit(n),
            1 => Self::Earlier(n),
            2 => Self::Later(n),
            3 => Self::Divider,
            4 => Self::ResultsBar,
            5 => Self::Choice(n),
            _ => Self::Detail(crate::fast_access_detail::Target::of_code(n)),
        }
    }
}

/// One control drawn on a line: its first cell (in the surface's columns),
/// how many cells, and what pressing it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// Its first column.
    pub from: u16,
    /// How many columns.
    pub width: u16,
    /// What it takes.
    pub target: Target,
}

/// What a line of the results is.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// The blank line before a section's title.
    Air,
    /// A section's title.
    Heading(SectionId),
    /// A sentence: why a section has nothing, or that it is being asked.
    Note(String, tokens::Color),
    /// A hit on one line: a name and a quieter one.
    Pair(usize),
    /// One line of a hit drawn on several: a card's, the consent row's.
    Part(usize, String, tokens::Color),
    /// A page of one collection's strip.
    Strip {
        /// Which collection (its place in `Collection::ALL`).
        collection: usize,
        /// The hits on the page.
        hits: Vec<usize>,
        /// Icons hidden before the page.
        earlier: bool,
        /// Icons hidden after it.
        later: bool,
    },
    /// A line of a row of the grid: `part` 0 is where its pictures begin.
    Grid {
        /// The row's videos.
        hits: Vec<usize>,
        /// Which line of the row.
        part: u16,
    },
    /// A line of the web video's question: its title (`None`) or a choice.
    Question(Option<usize>),
}

/// One line of the results.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// What it is.
    pub kind: Kind,
    /// The controls on it.
    pub cells: Vec<Cell>,
}

/// Where the divider is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Divider {
    /// A column between the results and the detail: its cell and its x.
    Beside {
        /// Its cell, in the surface's columns.
        column: u16,
        /// Its rule's x, in pixels.
        x: f32,
    },
    /// A rule above the detail below: its line and its y.
    Below {
        /// Its line, in the surface's lines.
        line: u16,
        /// Its rule's y, in pixels.
        y: f32,
    },
}

/// The ONE layout of a frame: what the drawing draws and the pointer reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Lay {
    /// The view's whole area; its first line is what is typed.
    pub area: Area,
    /// The results' box.
    pub results: Area,
    /// The detail's box, when there is one.
    pub detail: Option<Area>,
    /// The divider, when the detail is beside or below.
    pub divider: Option<Divider>,
    /// Whether a press on the divider changes anything — the detail's two
    /// widths beside, its two heights below, when the area has room for
    /// both. A divider that would change nothing is drawn, and is no
    /// control (it neither lights nor takes a press).
    pub toggles: bool,
    /// Every line of the results, from the first.
    pub lines: Vec<Line>,
    /// The rows the cursor walks, in order: each a list of hits side by
    /// side, and the line it is on.
    pub rows: Vec<(usize, Vec<usize>)>,
    /// Every hit, by its place.
    pub hits: Vec<Hit>,
}

/// The cells a strip's icon takes.
const fn strip_cells() -> u16 {
    tokens::space::FASTACCESS_STRIPCELLS
}

/// The cells a page mark takes, with its air.
const MARK_CELLS: u16 = 2;

/// The cell of air before the `›` mark, so a wide glyph at the page's end
/// never touches it.
const MARK_AIR: u16 = 1;

/// How many icons a page of a strip holds in `columns` of results: what is
/// left after both marks, the air before `›` and the results' bar.
fn strip_per(columns: u16) -> usize {
    let room = columns.saturating_sub(2 * MARK_CELLS + MARK_AIR + crate::chrome::BAR_CELLS);
    usize::from((room / strip_cells()).max(1))
}

/// The first cell of a strip's `›`, from the results' `left`.
fn later_at(left: u16, per: usize) -> u16 {
    left + MARK_CELLS + u16::try_from(per).unwrap_or(0) * strip_cells() + MARK_AIR
}

/// The videos across a row of the grid in `columns` of results, and the
/// cells each takes: as many as `gridCells` lets, the leftover spread over
/// them — never a strip of empty cells at the right.
fn grid_step(columns: u16) -> (usize, u16) {
    let room = columns.saturating_sub(crate::chrome::BAR_CELLS);
    let least = tokens::space::FASTACCESS_GRIDCELLS;
    let across = (room / least).max(1);
    (
        usize::from(across),
        (room / across).max(least.min(room).max(1)),
    )
}

/// The index of a collection.
fn slot(c: Collection) -> usize {
    Collection::ALL.iter().position(|x| *x == c).unwrap_or(0)
}

/// A collection's name's key (`fast-access-strip-<id>`).
#[must_use]
pub fn strip_key(c: Collection) -> String {
    format!("fast-access-strip-{}", c.id())
}

impl Lay {
    /// The layout of `view` in `area`.
    #[must_use]
    pub fn of(view: &View, area: Area, texts: &ronoctua_i18n::Texts) -> Self {
        let hits: Vec<Hit> = view.hits().cloned().collect();
        let below_area = area.below(1);
        let whole_scope = view.query().scope.whole();
        let has_hit = view.current().is_some();
        let (results, detail, divider, toggles) =
            if view.play_in.is_some() || whole_scope || !has_hit {
                (below_area, None, None, false)
            } else if view.whole {
                (
                    Area {
                        lines: 0,
                        ..below_area
                    },
                    Some(below_area),
                    None,
                    false,
                )
            } else {
                split(below_area, view.wide)
            };
        let mut lay = Self {
            area,
            results,
            detail,
            divider,
            toggles,
            lines: Vec::new(),
            rows: Vec::new(),
            hits,
        };
        lay.lines = lay.lines_of(view, texts);
        lay.rows = rows_of(&lay.lines);
        lay
    }

    /// The flat place of the hit with `id`.
    #[must_use]
    pub fn place_of(&self, id: &str) -> Option<usize> {
        self.hits.iter().position(|h| h.id == id)
    }

    /// The lines of the results.
    fn lines_of(&self, view: &View, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        let room = self.results.columns;
        let left = self.results.left;
        let mut out: Vec<Line> = Vec::new();
        let plain = |kind| Line {
            kind,
            cells: Vec::new(),
        };
        if let Some(q) = &view.play_in {
            out.push(plain(Kind::Question(None)));
            for n in 0..3 {
                out.push(Line {
                    kind: Kind::Question(Some(n)),
                    cells: vec![Cell {
                        from: left,
                        width: room,
                        target: Target::Choice(n),
                    }],
                });
            }
            let _ = q;
            return out;
        }
        let typed = view.text().trim();
        if typed.is_empty() {
            // AN EMPTY BOX SAYS WHAT A SEARCH IS FOR, above the recent.
            for l in crate::chrome::note_lines(&texts.t(crate::key::FAST_ACCESS_EMPTY), room) {
                out.push(plain(Kind::Note(l, tokens::color::TEXT_SECONDARY)));
            }
        } else if view.sections().iter().all(|s| {
            s.filled != Filled::Pending
                && (s.hits().is_empty()
                    || (s.id == SectionId::Web
                        && view.query().scope == crate::fast_access::Scope::All))
        }) && view
            .sections()
            .iter()
            .all(|s| !matches!(s.filled, Filled::Refused(_)))
        {
            // NOTHING FOUND IS SAID, never a blank page: every section of this
            // machine answered and none holds a hit — the web's row may follow.
            let said = texts.t_with(crate::key::FAST_ACCESS_NOTHING, &[("words", typed)]);
            for l in crate::chrome::note_lines(&said, room) {
                out.push(plain(Kind::Note(l, tokens::color::TEXT_SECONDARY)));
            }
        }
        let mut place = 0usize;
        for (n, section) in view.sections().iter().enumerate() {
            let hits = section.hits();
            if matches!(section.filled, Filled::Hits(_)) && hits.is_empty() {
                continue;
            }
            if n > 0 || !out.is_empty() {
                out.push(plain(Kind::Air));
            }
            out.push(plain(Kind::Heading(section.id)));
            match &section.filled {
                // THE ICONS WAIT FOR THEIR TABLES, and say so — the one
                // in-process source that can be pending.
                Filled::Pending if section.id == SectionId::Icons => out.push(plain(Kind::Note(
                    texts.t(crate::key::FAST_ACCESS_READING_ICONS),
                    tokens::color::TEXT_DIM,
                ))),
                Filled::Pending => out.push(plain(Kind::Note(
                    texts.t(crate::key::FAST_ACCESS_SEARCHING),
                    tokens::color::TEXT_DIM,
                ))),
                Filled::Refused(said) => {
                    for l in crate::chrome::note_lines(&said.said(texts), room) {
                        out.push(plain(Kind::Note(l, tokens::color::TEXT_SECONDARY)));
                    }
                }
                Filled::Hits(_) => {}
            }
            match section.id {
                SectionId::Icons => self.strips(view, hits, place, &mut out),
                SectionId::YouTube => self.grid(view, hits, place, texts, &mut out),
                _ => {
                    for (k, hit) in hits.iter().enumerate() {
                        self.hit_lines(view, hit, place + k, texts, &mut out);
                    }
                }
            }
            place += hits.len();
        }
        out
    }

    /// One hit's lines: a pair, or a card's or the consent row's parts.
    fn hit_lines(
        &self,
        view: &View,
        hit: &Hit,
        place: usize,
        texts: &ronoctua_i18n::Texts,
        out: &mut Vec<Line>,
    ) {
        let room = self
            .results
            .columns
            .saturating_sub(crate::chrome::BAR_CELLS);
        let whole = Cell {
            from: self.results.left,
            width: self.results.columns,
            target: Target::Hit(place),
        };
        let focused = view.current().is_some_and(|c| c.id == hit.id);
        let parts: Option<Vec<(String, tokens::Color)>> = match &hit.what {
            What::Consent { hosts, what } => {
                let hosts: Vec<&str> = hosts.iter().map(String::as_str).collect();
                Some(crate::consent_rows::lines(
                    crate::consent_rows::Ask {
                        hosts: &hosts,
                        what,
                    },
                    room,
                    focused,
                    texts,
                ))
            }
            What::Words(card) => Some(card_lines(card, &hit.title, room, focused, texts)),
            _ => None,
        };
        match parts {
            Some(parts) => {
                for (text, colour) in parts {
                    out.push(Line {
                        kind: Kind::Part(place, text, colour),
                        cells: vec![whole],
                    });
                }
            }
            None => out.push(Line {
                kind: Kind::Pair(place),
                cells: vec![whole],
            }),
        }
    }

    /// The three strips — EXACTLY three, one per collection, in their order:
    /// a page each, as wide as the results allow; a collection with nothing
    /// that matches says so on its own line instead of leaving its place.
    fn strips(&self, view: &View, hits: &[Hit], first: usize, out: &mut Vec<Line>) {
        let per = strip_per(self.results.columns);
        let left = self.results.left;
        for c in Collection::ALL {
            let mine: Vec<usize> = hits
                .iter()
                .enumerate()
                .filter(|(_, h)| matches!(h.what, What::Icon(i) if i.collection == c))
                .map(|(k, _)| first + k)
                .collect();
            let start = view.strip_pages[slot(c)].min(mine.len().saturating_sub(1));
            let start = start - start % per;
            let page: Vec<usize> = mine.iter().skip(start).take(per).copied().collect();
            let earlier = start > 0;
            let later = start + page.len() < mine.len();
            let mut cells = Vec::new();
            if earlier {
                cells.push(Cell {
                    from: left,
                    width: MARK_CELLS,
                    target: Target::Earlier(slot(c)),
                });
            }
            for (k, h) in page.iter().enumerate() {
                cells.push(Cell {
                    from: left + MARK_CELLS + u16::try_from(k).unwrap_or(0) * strip_cells(),
                    width: strip_cells(),
                    target: Target::Hit(*h),
                });
            }
            if later {
                cells.push(Cell {
                    from: later_at(left, per),
                    width: MARK_CELLS,
                    target: Target::Later(slot(c)),
                });
            }
            out.push(Line {
                kind: Kind::Strip {
                    collection: slot(c),
                    hits: page,
                    earlier,
                    later,
                },
                cells,
            });
        }
    }

    /// YouTube's grid: the switch, the consent rows, then the videos in rows
    /// of pictures with their title and channel under them.
    fn grid(
        &self,
        view: &View,
        hits: &[Hit],
        first: usize,
        texts: &ronoctua_i18n::Texts,
        out: &mut Vec<Line>,
    ) {
        let mut videos = Vec::new();
        for (k, h) in hits.iter().enumerate() {
            match &h.what {
                What::Video(_) => videos.push(first + k),
                _ => self.hit_lines(view, h, first + k, texts, out),
            }
        }
        let (across, step) = grid_step(self.results.columns);
        let tall = tokens::space::FASTACCESS_GRIDLINES;
        for row in videos.chunks(across) {
            let cells: Vec<Cell> = row
                .iter()
                .enumerate()
                .map(|(k, h)| Cell {
                    from: self.results.left + u16::try_from(k).unwrap_or(0) * step,
                    width: step,
                    target: Target::Hit(*h),
                })
                .collect();
            for part in 0..tall {
                out.push(Line {
                    kind: Kind::Grid {
                        hits: row.to_vec(),
                        part,
                    },
                    cells: cells.clone(),
                });
            }
        }
    }

    /// How many lines of the results show.
    #[must_use]
    pub fn room(&self) -> usize {
        usize::from(self.results.lines)
    }

    /// `at` moved onto the first line of the row of the grid it falls in —
    /// or, `forward`, of the next row (when there is one): a row of pictures
    /// is shown from its pictures, never from its titles with a blank above
    /// them. A line that is no grid's stays.
    #[must_use]
    pub fn snap(&self, at: usize, forward: bool) -> usize {
        let part = match self.lines.get(at).map(|l| &l.kind) {
            Some(Kind::Grid { part, .. }) if *part > 0 => usize::from(*part),
            _ => return at,
        };
        let start = at - part;
        let next = start + usize::from(tokens::space::FASTACCESS_GRIDLINES);
        if forward && next < self.lines.len() {
            next
        } else {
            start
        }
    }

    /// The last first line: the results' end at the bottom — or, in a grid,
    /// the next row's start, the space under the last row left empty.
    #[must_use]
    pub fn last(&self) -> usize {
        self.snap(self.lines.len().saturating_sub(self.room()), true)
    }

    /// The first line shown when the view's offset is `offset`: held to the
    /// end, and never in the middle of a row of the grid.
    #[must_use]
    pub fn first(&self, offset: usize) -> usize {
        self.snap(offset.min(self.last()), false)
    }

    /// The results' scroll, at `offset` — a grid's empty space under its last
    /// row counted, so the thumb reaches the end with the offset.
    #[must_use]
    pub fn scroll(&self, offset: usize) -> ronoctua_design::Scroll {
        crate::chrome::list_scroll(
            self.lines.len().max(self.last() + self.room()),
            self.room(),
            self.first(offset),
        )
    }

    /// The results' bar — its track and thumb — when they do not fit.
    #[must_use]
    pub fn bar(&self, offset: usize) -> Option<(crate::chrome::Rect, Option<crate::chrome::Rect>)> {
        let scroll = self.scroll(offset);
        scroll.cut().then(|| {
            let right = match self.divider {
                Some(Divider::Beside { column, .. }) => Area::x_of(column),
                _ => self.results.right_px,
            };
            let bottom = match self.divider {
                Some(Divider::Below { y, .. }) => y,
                _ => self.results.bottom_px,
            };
            crate::chrome::band_bar(
                scroll,
                right,
                Area::y_of(self.results.top),
                bottom,
                Area::cell(),
                Area::line(),
            )
        })
    }

    /// The line of the results the cursor's hit begins on.
    #[must_use]
    pub fn line_of(&self, place: usize) -> Option<usize> {
        self.lines
            .iter()
            .position(|l| l.cells.iter().any(|c| c.target == Target::Hit(place)))
    }

    /// What is at `(x, y)` of the surface, by this layout.
    #[must_use]
    pub fn target_at(
        &self,
        view: &View,
        x: f32,
        y: f32,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Target> {
        if let Some((track, _)) = self.bar(view.offset)
            && crate::bars::Track::of(track).holds(f64::from(x), f64::from(y))
        {
            return Some(Target::ResultsBar);
        }
        let on_the_divider = match self.divider {
            Some(Divider::Beside { column, .. }) => {
                y >= Area::y_of(self.results.top)
                    && x >= Area::x_of(column)
                    && x < Area::x_of(column + 1)
            }
            Some(Divider::Below { line, .. }) => y >= Area::y_of(line) && y < Area::y_of(line + 1),
            None => false,
        };
        if on_the_divider {
            return self.toggles.then_some(Target::Divider);
        }
        if let Some(d) = self.detail
            && x >= Area::x_of(d.left.saturating_sub(crate::chrome::FIELDS_LEAD))
            && y >= Area::y_of(d.top)
            && y < d.bottom_px
        {
            return crate::fast_access_detail::target_at(view, d, x, y, texts).map(Target::Detail);
        }
        if y < Area::y_of(self.results.top) || self.results.lines == 0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let row = ((y - Area::y_of(self.results.top)) / Area::line()) as usize;
        if row >= self.room() {
            return None;
        }
        let line = self.lines.get(self.first(view.offset) + row)?;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let column = (x / Area::cell()).floor().max(0.0) as u16;
        line.cells
            .iter()
            .find(|c| crate::chrome::in_cell(column, c.from, c.width))
            .map(|c| c.target)
    }
}

/// The results and the detail beside them, or the detail below when the
/// results would be too narrow beside it — decided by the detail's NORMAL
/// width, so `wide` only ever changes how wide (beside) or how tall (below)
/// the detail is. The fourth answer: whether `wide` changes anything here.
fn split(area: Area, wide: bool) -> (Area, Option<Area>, Option<Divider>, bool) {
    let min = tokens::space::FASTACCESS_RESULTSMINCELLS;
    let normal = tokens::space::FASTACCESS_DETAILCELLS.min(area.columns / 2);
    if area.columns > min + normal {
        // THE WIDER WIDTH, as far as the results keep their least.
        let widest = tokens::space::FASTACCESS_DETAILWIDECELLS
            .min(area.columns / 2)
            .min(area.columns.saturating_sub(min + 1))
            .max(normal);
        let detail_cells = if wide { widest } else { normal };
        let results_cols = area.columns - detail_cells - 1;
        let column = area.left + results_cols;
        let results = Area {
            columns: results_cols,
            right_px: Area::x_of(column),
            ..area
        };
        let detail = Area {
            left: column + 1 + crate::chrome::FIELDS_LEAD,
            columns: detail_cells.saturating_sub(crate::chrome::FIELDS_LEAD),
            ..area
        };
        let x = Area::x_of(column) + Area::cell() / 2.0;
        return (
            results,
            Some(detail),
            Some(Divider::Beside { column, x }),
            widest > normal,
        );
    }
    let below = tokens::space::FASTACCESS_BELOWLINES;
    // THE RESULTS KEEP FOUR LINES above the rule, whichever height.
    let keep = 4;
    if area.lines < below + keep {
        // NO ROOM BELOW EITHER: the results alone; `C-w` still gives the
        // detail the whole area.
        return (area, None, None, false);
    }
    let tallest = tokens::space::FASTACCESS_BELOWWIDELINES
        .min(area.lines - keep)
        .max(below);
    let lines = if wide { tallest } else { below };
    let results_lines = area.lines - lines - 1;
    let line = area.top + results_lines;
    let results = Area {
        lines: results_lines,
        bottom_px: Area::y_of(line),
        ..area
    };
    let detail = Area {
        top: line + 1,
        lines,
        ..area
    };
    let y = Area::y_of(line) + Area::line() / 2.0;
    (
        results,
        Some(detail),
        Some(Divider::Below { line, y }),
        tallest > below,
    )
}

/// The rows the cursor walks: each line that starts a hit's row, with the
/// hits side by side on it.
fn rows_of(lines: &[Line]) -> Vec<(usize, Vec<usize>)> {
    let mut out: Vec<(usize, Vec<usize>)> = Vec::new();
    for (n, l) in lines.iter().enumerate() {
        let hits: Vec<usize> = match &l.kind {
            Kind::Pair(h) => vec![*h],
            Kind::Part(h, ..) => vec![*h],
            Kind::Strip { hits, .. } if !hits.is_empty() => hits.clone(),
            Kind::Grid { hits, part: 0 } => hits.clone(),
            _ => continue,
        };
        if out.last().is_some_and(|(_, last)| *last == hits) {
            continue;
        }
        out.push((n, hits));
    }
    out
}

/// A card's lines, wrapped: a word's meanings, its synonyms, a translation.
fn card_lines(
    card: &Card,
    word: &str,
    room: u16,
    focused: bool,
    texts: &ronoctua_i18n::Texts,
) -> Vec<(String, tokens::Color)> {
    let (label, value) = crate::chrome::Kind::ListedFact.tones(focused);
    let mut out: Vec<(String, tokens::Color)> = vec![(word.to_owned(), label)];
    let wrap = |text: &str, colour, out: &mut Vec<(String, tokens::Color)>| {
        for l in crate::chrome::note_lines(text, room) {
            out.push((l, colour));
        }
    };
    match card {
        Card::Senses(senses) => {
            for s in senses {
                let gloss = match s.part {
                    Some(p) => format!(
                        "{} — {}",
                        texts.t(&format!("words-part-{}", p.id())),
                        s.gloss
                    ),
                    None => s.gloss.clone(),
                };
                wrap(&gloss, value, &mut out);
                for e in &s.examples {
                    wrap(&format!("“{e}”"), tokens::color::TEXT_SECONDARY, &mut out);
                }
                if !s.synonyms.is_empty() {
                    wrap(
                        &s.synonyms.join(", "),
                        tokens::color::TEXT_SECONDARY,
                        &mut out,
                    );
                }
            }
            if let Some(source) = senses.first().map(|s| s.source) {
                let id = match source {
                    ronoctua_words::Source::WordNet => "wordnet",
                    ronoctua_words::Source::Wiktionary => "wiktionary",
                };
                wrap(
                    &texts.t(&format!("words-source-{id}")),
                    tokens::color::TEXT_DIM,
                    &mut out,
                );
            }
        }
        Card::Synonyms(words) => {
            if words.is_empty() {
                wrap(
                    &texts.t(crate::key::FAST_ACCESS_SYNONYMS_NONE),
                    tokens::color::TEXT_SECONDARY,
                    &mut out,
                );
            } else {
                wrap(&words.join(", "), value, &mut out);
            }
        }
        Card::Translation(t) => {
            wrap(
                &texts.t_with(
                    crate::key::FAST_ACCESS_TRANSLATE_INTO,
                    &[("tongue", t.to.code())],
                ),
                tokens::color::TEXT_SECONDARY,
                &mut out,
            );
            for line in t.text.lines() {
                wrap(line, value, &mut out);
            }
            wrap(
                &texts.t_with(
                    crate::key::WORDS_TRANSLATED_BY,
                    &[(
                        "provider",
                        &texts.t(&format!("words-provider-{}", t.by.id())),
                    )],
                ),
                tokens::color::TEXT_DIM,
                &mut out,
            );
        }
    }
    out
}

/// What a door draws for the view: runs, grounds, pictures and bars.
#[derive(Debug, Clone, Default)]
pub struct Drawing {
    /// The runs of text.
    pub spans: Vec<ronoctua_render::Span>,
    /// The grounds and the bands, under everything else.
    pub under: Vec<ronoctua_render::quad::Quad>,
    /// What goes over the pictures: the bars' thumbs.
    pub over: Vec<ronoctua_render::quad::Quad>,
    /// The pictures, by key.
    pub pictures: Vec<Pic>,
    /// Every bar's track (`crate::bars`).
    pub tracks: Vec<crate::bars::Track>,
}

/// A picture of a frame: its key, where it goes, and how its pixels come —
/// the door's own picture (`region::Painted`), which every door of the search
/// puts up by one rule: a still one when its texture is missing, a moving one
/// every frame, an SVG rasterised when missing (a Lucide figure's page).
pub type Pic = crate::region::Painted;

/// Where a picture's pixels come from (`region::Source`).
pub use crate::region::Source;

/// A Lucide figure's page as a picture, rasterised only when the door's GPU
/// is missing its key.
#[must_use]
pub fn svg_picture(p: crate::icons_page::Picture) -> Pic {
    let key = p.key.clone();
    let area = p.area;
    Pic {
        key,
        area,
        source: Source::Drawn(std::sync::Arc::new(move || p.rgba())),
    }
}

/// A run of text at the grid's size.
fn run(text: String, column: u16, line: u16, color: tokens::Color) -> ronoctua_render::Span {
    ronoctua_render::Span {
        size: ronoctua_render::Size::Px(tokens::text::size()),
        text,
        column,
        line,
        color,
        edge: false,
        foot: false,
    }
}

/// Draws the view in `area`: what is typed, the results, the divider and
/// the detail. `lit` is every hand's target, in its colour.
pub fn draw(
    view: &View,
    area: Area,
    focused: bool,
    lit: &[(tokens::Color, Target)],
    texts: &ronoctua_i18n::Texts,
    out: &mut Drawing,
) {
    let lay = Lay::of(view, area, texts);
    draw_typed(view, area, focused, texts, out);
    let offset = lay.first(view.offset);
    let cursor = view.current().and_then(|c| lay.place_of(&c.id));
    let hovered: Vec<(tokens::Color, Target)> = lit.to_vec();
    for (k, line) in lay.lines.iter().skip(offset).take(lay.room()).enumerate() {
        let at = lay.results.top + u16::try_from(k).unwrap_or(u16::MAX);
        draw_line(view, &lay, line, at, cursor, &hovered, texts, out);
    }
    if let Some((track, thumb)) = lay.bar(offset) {
        out.tracks.push(crate::bars::Track::of(track));
        if let Some(t) = thumb {
            out.over.push(crate::chrome::thumb(t));
        }
    }
    // THE DIVIDER, a control when a press would change something: it swaps
    // the detail's width (beside) or its height (below).
    let lit_here = lay.toggles && lit.iter().any(|(_, t)| *t == Target::Divider);
    match lay.divider {
        Some(Divider::Beside { x, .. }) => {
            let q = if lit_here || view.focus == crate::fast_access::Focus::Detail {
                crate::chrome::divider_lit(x, Area::y_of(lay.results.top), area.bottom_px)
            } else {
                crate::chrome::divider(x, Area::y_of(lay.results.top), area.bottom_px)
            };
            out.under.push(q);
        }
        Some(Divider::Below { y, .. }) => {
            // LIT AS THE ONE BESIDE IS: under a hand, or with the keys below.
            let rule = crate::chrome::rule(area.right_px, y);
            out.under.push(
                if lit_here || view.focus == crate::fast_access::Focus::Detail {
                    ronoctua_render::quad::Quad {
                        color: tokens::color::STATE_ACCENT,
                        ..rule
                    }
                } else {
                    rule
                },
            );
        }
        None => {}
    }
    if let (Some(d), Some(hit)) = (lay.detail, view.current()) {
        let lit_detail: Vec<(tokens::Color, crate::fast_access_detail::Target)> = lit
            .iter()
            .filter_map(|(c, t)| match t {
                Target::Detail(d) => Some((*c, *d)),
                _ => None,
            })
            .collect();
        crate::fast_access_detail::draw(view, hit, d, &lit_detail, texts, out);
    }
}

/// The typed line: the prefix's scope in the accent, the words, the caret —
/// or, with nothing typed, what typing does.
fn draw_typed(
    view: &View,
    area: Area,
    focused: bool,
    texts: &ronoctua_i18n::Texts,
    out: &mut Drawing,
) {
    let text = view.text();
    if text.is_empty() {
        out.spans.push(run(
            ronoctua_render::layout::elide_end(
                &texts.t_with(
                    crate::key::FAST_ACCESS_PLACEHOLDER,
                    &[("slash", crate::fast_access::SLASH)],
                ),
                area.columns,
            ),
            area.left,
            area.top,
            tokens::color::TEXT_DIM,
        ));
        if focused {
            out.under.push(crate::chrome::caret(
                0.0,
                Area::y_of(area.top),
                Area::cell(),
                Area::line(),
                area.left,
            ));
        }
        return;
    }
    let shown = ronoctua_render::layout::elide_end(text, area.columns.saturating_sub(1));
    let width = ronoctua_render::layout::width(&shown);
    let q = view.query();
    let scope_word = q
        .scope
        .letter()
        .map(|_| texts.t(&format!("fast-access-scope-{}", q.scope.id())));
    // WHAT IS TYPED is a value the person sets, in a setting's tone.
    let (_, typed_tone) = crate::chrome::Kind::Setting.tones(false);
    out.spans.push(run(shown, area.left, area.top, typed_tone));
    if focused {
        out.under.push(crate::chrome::caret(
            0.0,
            Area::y_of(area.top),
            Area::cell(),
            Area::line(),
            area.left + width,
        ));
    }
    // THE SCOPE'S NAME against the right edge, quiet: what the prefix
    // narrowed to.
    if let Some(word) = scope_word {
        let w = ronoctua_render::layout::width(&word);
        if width + w + 3 <= area.columns {
            out.spans.push(run(
                word,
                area.left + area.columns - w,
                area.top,
                tokens::color::STATE_ACCENT,
            ));
        }
    }
}

/// One line of the results, drawn on surface line `at`.
#[allow(clippy::too_many_arguments)]
fn draw_line(
    view: &View,
    lay: &Lay,
    line: &Line,
    at: u16,
    cursor: Option<usize>,
    lit: &[(tokens::Color, Target)],
    texts: &ronoctua_i18n::Texts,
    out: &mut Drawing,
) {
    let left = lay.results.left;
    let room = lay.results.columns.saturating_sub(crate::chrome::BAR_CELLS);
    let line_px = Area::line();
    let band = |from: u16, width: u16, colour| ronoctua_render::quad::Quad {
        x: Area::x_of(from),
        y: Area::y_of(at),
        width: f32::from(width) * Area::cell(),
        height: line_px,
        color: colour,
    };
    // A LIST'S ROW LIGHTS ITS BAND under a hand; a strip's or a grid's CELL
    // lights alone.
    let one_per_line = matches!(
        line.kind,
        Kind::Pair(_) | Kind::Part(..) | Kind::Question(Some(_))
    );
    for (colour, t) in lit {
        if let Some(c) = line.cells.iter().find(|c| c.target == *t) {
            let width = if one_per_line { room } else { c.width };
            out.under.push(band(c.from, width, *colour));
        }
    }
    match &line.kind {
        Kind::Air => {}
        Kind::Heading(s) => out.spans.push(run(
            texts.t(&format!("fast-access-section-{}", s.id())),
            left,
            at,
            tokens::color::TEXT_HEADING,
        )),
        Kind::Note(text, colour) => out.spans.push(run(text.clone(), left, at, *colour)),
        // A CARD'S AND THE CONSENT ROW'S LINES carry their tones already —
        // the cursor's among them (`consent_rows::lines`, `card_lines`).
        Kind::Part(_, text, colour) => out.spans.push(run(text.clone(), left, at, *colour)),
        Kind::Pair(h) => {
            let Some(hit) = lay.hits.get(*h) else {
                return;
            };
            draw_pair(view, hit, left, room, at, cursor == Some(*h), texts, out);
        }
        // A COLLECTION WITH NOTHING THAT MATCHES keeps its strip, and says so.
        Kind::Strip {
            collection, hits, ..
        } if hits.is_empty() => {
            let name = Collection::ALL
                .get(*collection)
                .map_or_else(String::new, |c| texts.t(&strip_key(*c)));
            out.spans.push(run(
                ronoctua_render::layout::elide_end(
                    &texts.t_with(crate::key::FAST_ACCESS_STRIP_NONE, &[("collection", &name)]),
                    room,
                ),
                left + MARK_CELLS,
                at,
                tokens::color::TEXT_DIM,
            ));
        }
        Kind::Strip {
            hits,
            earlier,
            later,
            ..
        } => {
            // THE PAGE MARKS ACT: a click turns the strip's page.
            let (mark_tone, _) = crate::chrome::Kind::Setting.tones(false);
            if *earlier {
                out.spans.push(run(
                    tokens::icon::FAST_ACCESS_EARLIER.to_string(),
                    left,
                    at,
                    mark_tone,
                ));
            }
            for (k, h) in hits.iter().enumerate() {
                let Some(hit) = lay.hits.get(*h) else {
                    continue;
                };
                let column = left + MARK_CELLS + u16::try_from(k).unwrap_or(0) * strip_cells();
                if cursor == Some(*h) {
                    out.under.push(band(
                        column,
                        strip_cells(),
                        tokens::color::SURFACE_SELECTION,
                    ));
                }
                // AT THE SIZE THE GLYPHS ARE MEASURED FOR (`chrome_size`):
                // a glyph centred on its cell is never sized loose.
                out.spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::OnTheCell(crate::glyph::chrome_size(&hit.title)),
                    ..run(
                        hit.title.clone(),
                        column + strip_cells() / 2,
                        at,
                        tokens::color::TEXT_PRIMARY,
                    )
                });
            }
            if *later {
                out.spans.push(run(
                    tokens::icon::FAST_ACCESS_LATER.to_string(),
                    later_at(left, strip_per(lay.results.columns)),
                    at,
                    mark_tone,
                ));
            }
        }
        Kind::Grid { hits, part } => draw_grid_line(view, lay, hits, *part, at, cursor, texts, out),
        Kind::Question(None) => {
            if let Some(q) = &view.play_in {
                out.spans.push(run(
                    ronoctua_render::layout::elide_end(
                        &texts.t_with(crate::key::FAST_ACCESS_PLAY_IN, &[("title", &q.title)]),
                        room,
                    ),
                    left,
                    at,
                    tokens::color::TEXT_HEADING,
                ));
            }
        }
        Kind::Question(Some(n)) => {
            if let Some(q) = &view.play_in {
                let focused = q.cursor == *n;
                let (label, value) = crate::chrome::Kind::ListedSetting.tones(focused);
                let text = match n {
                    0 => texts.t(crate::key::FAST_ACCESS_PLAY_PLAYER),
                    1 => texts.t(crate::key::FAST_ACCESS_PLAY_KIOSK),
                    _ => format!(
                        "{} {}",
                        if q.remember {
                            tokens::icon::FAST_ACCESS_TICKED
                        } else {
                            tokens::icon::FAST_ACCESS_UNTICKED
                        },
                        texts.t(crate::key::FAST_ACCESS_PLAY_REMEMBER)
                    ),
                };
                out.spans
                    .push(run(text, left, at, if *n == 2 { value } else { label }));
            }
        }
    }
}

/// A pair: a glyph when the hit has one, its name, and the quieter line —
/// against the right edge for a command's keys, after the name otherwise.
#[allow(clippy::too_many_arguments)]
fn draw_pair(
    view: &View,
    hit: &Hit,
    left: u16,
    room: u16,
    at: u16,
    focused: bool,
    texts: &ronoctua_i18n::Texts,
    out: &mut Drawing,
) {
    let (label, value) = crate::chrome::Kind::ListedSetting.tones(focused);
    let glyph: Option<char> = match &hit.what {
        What::App { glyph, .. } => Some(*glyph),
        What::File { folder: true, .. } => Some(ronoctua_files::mime::icon_of("inode/directory")),
        What::File { mime, .. } => Some(ronoctua_files::mime::icon_of(
            mime.as_deref().unwrap_or("application/octet-stream"),
        )),
        _ => None,
    };
    let mut column = left;
    if let Some(g) = glyph {
        out.spans.push(ronoctua_render::Span {
            size: ronoctua_render::Size::OnTheCell(crate::glyph::chrome_size_of(g)),
            ..run(g.to_string(), column, at, value)
        });
        column += 2;
    }
    let title = match &hit.what {
        What::Shorts(shown) => texts.t(if *shown {
            "fast-access-shorts-shown"
        } else {
            "fast-access-shorts-hidden"
        }),
        What::Zone { offset_s, .. } => format!(
            "{}  {}",
            hit.title,
            zone_time(*offset_s, ronoctua_index::now_s())
        ),
        What::Math(_) => hit.title.clone(),
        _ => hit.title.clone(),
    };
    let rest = (left + room).saturating_sub(column);
    let note = hit.note.clone().filter(|n| !n.is_empty());
    let keys_at_edge = matches!(hit.what, What::Command { .. } | What::Setting(_));
    let note_w = note.as_deref().map_or(0, ronoctua_render::layout::width);
    let title_room = if keys_at_edge && note_w > 0 {
        rest.saturating_sub(note_w + 2)
    } else {
        rest
    };
    let title = ronoctua_render::layout::elide_end(&title, title_room);
    let tw = ronoctua_render::layout::width(&title);
    out.spans.push(run(title, column, at, label));
    if let Some(note) = note {
        if keys_at_edge {
            let n = ronoctua_render::layout::elide_end(&note, rest.saturating_sub(tw + 2));
            let w = ronoctua_render::layout::width(&n);
            out.spans.push(run(n, left + room - w, at, value));
        } else if tw + 2 < rest {
            let n = ronoctua_render::layout::elide_end(&note, rest - tw - 2);
            out.spans.push(run(n, column + tw + 2, at, value));
        }
    }
    let _ = view;
}

/// One line of a row of the grid: the pictures on part 0, then each video's
/// title and channel under its picture.
#[allow(clippy::too_many_arguments)]
fn draw_grid_line(
    view: &View,
    lay: &Lay,
    hits: &[usize],
    part: u16,
    at: u16,
    cursor: Option<usize>,
    texts: &ronoctua_i18n::Texts,
    out: &mut Drawing,
) {
    let (_, step) = grid_step(lay.results.columns);
    let picture_lines = tokens::space::FASTACCESS_GRIDPICTURELINES;
    // THE RESULTS' LAST LINE IS A WALL: a row begun near the bottom shows its
    // picture smaller, in the lines left, never past the box.
    let left_below = (lay.results.top + lay.results.lines).saturating_sub(at);
    let air = 1u16;
    for (k, h) in hits.iter().enumerate() {
        let Some(hit) = lay.hits.get(*h) else {
            continue;
        };
        let What::Video(v) = &hit.what else {
            continue;
        };
        let column = lay.results.left + u16::try_from(k).unwrap_or(0) * step;
        let room = step.saturating_sub(air);
        // THE CURSOR'S CELL wears `surface.selection`, on every line of it.
        if cursor == Some(*h) {
            out.under.push(ronoctua_render::quad::Quad {
                x: Area::x_of(column),
                y: Area::y_of(at),
                width: f32::from(room) * Area::cell(),
                height: Area::line(),
                color: tokens::color::SURFACE_SELECTION,
            });
        }
        if part == 0 {
            if let Some(key) = &v.thumb
                && let Some(rgba) = view.pictures.get(key)
            {
                let tall = picture_lines.min(left_below);
                if tall == 0 {
                    continue;
                }
                let space = ronoctua_render::image::Area {
                    x: Area::x_of(column),
                    y: Area::y_of(at),
                    width: f32::from(room) * Area::cell(),
                    height: f32::from(tall) * Area::line(),
                };
                out.pictures.push(Pic {
                    key: key.clone(),
                    area: crate::sizes::picture_in(space, rgba.width, rgba.height),
                    source: Source::Still(std::sync::Arc::clone(rgba)),
                });
            }
        } else if part == picture_lines {
            let (label, _) = crate::chrome::Kind::ListedSetting.tones(cursor == Some(*h));
            out.spans.push(run(
                ronoctua_render::layout::elide_end(&hit.title, room),
                column,
                at,
                label,
            ));
        } else if part == picture_lines + 1 {
            let mut said: Vec<String> = Vec::new();
            if let Some(c) = &v.channel {
                said.push(c.clone());
            }
            if let Some(d) = v.duration_s {
                said.push(crate::fast_access_videos::length(d));
            }
            said.push(texts.t(if v.local {
                "fast-access-youtube-local"
            } else {
                "fast-access-youtube-online"
            }));
            out.spans.push(run(
                ronoctua_render::layout::elide_end(&said.join(" · "), room),
                column,
                at,
                tokens::color::TEXT_SECONDARY,
            ));
        }
    }
}

/// The cursor moved by a key, along the rows drawn: up and down across rows
/// keeping its place in a row of a grid or a strip; left and right along a
/// strip — turning its page at the edge — or a grid's row.
pub fn moved(view: &mut View, press: &Press, area: Area, texts: &ronoctua_i18n::Texts) {
    let mut lay = Lay::of(view, area, texts);
    let cursor = view.current().and_then(|c| lay.place_of(&c.id));
    let Some(cursor) = cursor else {
        view.cursor_to_end(false);
        return;
    };
    let mut at = lay.rows.iter().position(|(_, hits)| hits.contains(&cursor));
    // AN ICON ON NO PAGE SHOWN — its strip's page turned by a click — brings
    // its page back, and the key goes on from there: a key never dies on a
    // cursor nobody can see.
    if at.is_none()
        && let What::Icon(i) = lay.hits[cursor].what
    {
        let c = slot(i.collection);
        let mine = (0..lay.hits.len())
            .filter(|h| matches!(lay.hits[*h].what, What::Icon(j) if j.collection == i.collection))
            .position(|h| h == cursor);
        if let Some(p) = mine {
            view.strip_pages[c] = p;
            lay = Lay::of(view, area, texts);
            at = lay.rows.iter().position(|(_, hits)| hits.contains(&cursor));
        }
    }
    let Some(at) = at else {
        return;
    };
    let (_, row) = &lay.rows[at];
    let k = row.iter().position(|h| *h == cursor).unwrap_or(0);
    let strip = lay.lines.get(lay.rows[at].0).and_then(|l| match &l.kind {
        Kind::Strip { collection, .. } => Some(*collection),
        _ => None,
    });
    match press {
        Press::Up | Press::Down => {
            let to = if *press == Press::Up {
                at.checked_sub(1)
            } else {
                (at + 1 < lay.rows.len()).then_some(at + 1)
            };
            if let Some(to) = to {
                let row = &lay.rows[to].1;
                if let Some(h) = row.get(k.min(row.len().saturating_sub(1))) {
                    view.point_at(&lay.hits[*h].id);
                }
            }
        }
        Press::Left | Press::Right => {
            let forward = *press == Press::Right;
            if let Some(c) = strip {
                // A STRIP: one icon along, the page turning at its edge.
                let all: Vec<usize> = (0..lay.hits.len())
                    .filter(
                        |h| matches!(lay.hits[*h].what, What::Icon(i) if slot(i.collection) == c),
                    )
                    .collect();
                let p = all.iter().position(|h| *h == cursor).unwrap_or(0);
                let to = if forward {
                    (p + 1).min(all.len().saturating_sub(1))
                } else {
                    p.saturating_sub(1)
                };
                view.strip_pages[c] = to;
                view.point_at(&lay.hits[all[to]].id);
            } else {
                let to = if forward {
                    (k + 1).min(row.len().saturating_sub(1))
                } else {
                    k.saturating_sub(1)
                };
                view.point_at(&lay.hits[row[to]].id);
            }
        }
        _ => {}
    }
    follow(view, area, texts);
}

/// A page of the results up or down, the cursor onto the first row shown.
pub fn paged(view: &mut View, down: bool, area: Area, texts: &ronoctua_i18n::Texts) {
    let lay = Lay::of(view, area, texts);
    let room = lay.room().max(1);
    let shown = lay.first(view.offset);
    let to = if down {
        shown + room
    } else {
        shown.saturating_sub(room)
    };
    view.offset = lay.snap(to.min(lay.last()), false);
    if let Some((_, row)) = lay.rows.iter().find(|(line, _)| *line >= view.offset)
        && let Some(h) = row.first()
    {
        view.point_at(&lay.hits[*h].id);
    }
}

/// Keeps the cursor's row on screen — after a key or a click, never for
/// data.
pub fn follow(view: &mut View, area: Area, texts: &ronoctua_i18n::Texts) {
    let lay = Lay::of(view, area, texts);
    let Some(place) = view.current().and_then(|c| lay.place_of(&c.id)) else {
        return;
    };
    let Some(first) = lay.line_of(place) else {
        return;
    };
    let last = lay.lines[first..]
        .iter()
        .position(|l| !l.cells.iter().any(|c| c.target == Target::Hit(place)))
        .map_or(lay.lines.len() - 1, |n| first + n - 1);
    let room = lay.room().max(1);
    let shown = lay.first(view.offset);
    // A ROW OF THE GRID IS SHOWN FROM ITS PICTURES: the offset lands on a
    // row's first line (`Lay::snap`), never past the cursor's own.
    view.offset = if first < shown {
        // A SECTION'S TITLE COMES WITH ITS FIRST ROW.
        lay.snap(first.saturating_sub(2.min(room.saturating_sub(1))), true)
            .min(first)
    } else if last >= shown + room {
        lay.snap(last + 1 - room, true).min(first)
    } else {
        shown
    };
    view.offset = view.offset.min(lay.last());
}

/// The wheel turned `by` lines over the results.
///
/// A grid moves by its rows: a turn that lands inside one goes on to the
/// next row's start (down) or back to its own (up) — `Lay::snap`.
pub fn wheeled(view: &mut View, by: i64, area: Area, texts: &ronoctua_i18n::Texts) {
    let lay = Lay::of(view, area, texts);
    let now = i64::try_from(lay.first(view.offset))
        .unwrap_or(0)
        .saturating_add(by)
        .max(0);
    let to = usize::try_from(now).unwrap_or(0).min(lay.last());
    view.offset = lay.snap(to, by > 0).min(lay.last());
}

/// The results' bar dragged to `y`.
pub fn dragged(view: &mut View, area: Area, y: f32, texts: &ronoctua_i18n::Texts) {
    let lay = Lay::of(view, area, texts);
    let scroll = lay.scroll(view.offset);
    let top = Area::y_of(lay.results.top);
    let span = (lay.results.bottom_px - top).max(1.0);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let at = (((y - top) / span).clamp(0.0, 1.0) * scroll.total as f32) as usize;
    view.offset = lay.snap(at.min(lay.last()), false);
}

/// A held bar followed to `(x, y)`: the results', the detail's or one of a
/// file's panels', whichever the press grabbed (`View::grabbed`). Answers
/// whether one was held.
pub fn held(view: &mut View, area: Area, (x, y): (f32, f32), texts: &ronoctua_i18n::Texts) -> bool {
    match view.grabbed {
        Some(Grab::Results) => dragged(view, area, y, texts),
        Some(Grab::Detail) => {
            let lay = Lay::of(view, area, texts);
            let hit = view.current().cloned();
            let _ = crate::fast_access_detail::pressed(
                view,
                hit.as_ref(),
                &lay,
                crate::fast_access_detail::Target::Bar,
                (x, y),
                texts,
            );
        }
        Some(Grab::File(pane, axis)) => {
            let lay = Lay::of(view, area, texts);
            if let Some(d) = lay.detail {
                crate::fast_access_detail::file_dragged(view, d, (pane, axis), (x, y), texts);
            }
        }
        None => return false,
    }
    true
}

/// The pointer rests on `target` at `y` (or left the view): what the line
/// under the shortcuts and every band read, and the file's panel told which
/// of its panes the pointer is over — what brings a film's controls up.
pub fn hovered(
    view: &mut View,
    area: Area,
    target: Option<Target>,
    y: f32,
    texts: &ronoctua_i18n::Texts,
) {
    view.hover = target;
    let lay = Lay::of(view, area, texts);
    let on_detail = match target {
        Some(Target::Detail(t)) => Some((t, y)),
        _ => None,
    };
    if let Some(d) = lay.detail {
        crate::fast_access_detail::pointed(view, d, on_detail);
    }
}

/// The wheel over the view: the detail's when the point is in its box, the
/// results' otherwise.
pub fn wheel_at(
    view: &mut View,
    area: Area,
    x: f32,
    y: f32,
    by: i64,
    texts: &ronoctua_i18n::Texts,
) {
    let lay = Lay::of(view, area, texts);
    match lay.detail.filter(|d| {
        x >= Area::x_of(d.left) && y >= Area::y_of(d.top) && y < Area::y_of(d.top + d.lines)
    }) {
        Some(d) => crate::fast_access_detail::wheeled(view, by, d, y, texts),
        None => wheeled(view, by, area, texts),
    }
}

/// A press on `target`: the first click selects, the second does what `RET`
/// does (invariant 19). The errands for the door.
pub fn pressed(
    view: &mut View,
    target: Target,
    area: Area,
    (x, y): (f32, f32),
    ground: &crate::fast_access::Ground,
    texts: &ronoctua_i18n::Texts,
) -> Vec<crate::fast_access::Errand> {
    let lay = Lay::of(view, area, texts);
    match target {
        Target::Hit(h) => {
            let Some(hit) = lay.hits.get(h) else {
                return Vec::new();
            };
            if view.current().is_some_and(|c| c.id == hit.id) {
                return view.key(Press::Enter, area, ground, texts);
            }
            view.point_at(&hit.id.clone());
            view.focus = crate::fast_access::Focus::Results;
            Vec::new()
        }
        Target::Earlier(c) | Target::Later(c) => {
            let per = strip_per(lay.results.columns);
            let start = view.strip_pages[c] - view.strip_pages[c] % per;
            view.strip_pages[c] = if matches!(target, Target::Later(_)) {
                start + per
            } else {
                start.saturating_sub(per)
            };
            // THE CURSOR'S ICON TURNED OUT OF SIGHT goes with its page: onto
            // the first icon of the page shown, so the keys and the detail
            // stay on something drawn.
            let hidden = view
                .current()
                .is_some_and(|h| matches!(h.what, What::Icon(i) if slot(i.collection) == c));
            if hidden {
                let turned = Lay::of(view, area, texts);
                let first = turned.lines.iter().find_map(|l| match &l.kind {
                    Kind::Strip {
                        collection, hits, ..
                    } if *collection == c => hits.first().copied(),
                    _ => None,
                });
                let cursor = view.current().map(|h| h.id.clone());
                let shown = cursor
                    .as_deref()
                    .and_then(|id| turned.place_of(id))
                    .is_some_and(|p| turned.rows.iter().any(|(_, r)| r.contains(&p)));
                if !shown && let Some(h) = first {
                    let id = turned.hits[h].id.clone();
                    view.point_at(&id);
                }
            }
            Vec::new()
        }
        Target::Divider => {
            view.wide = !view.wide;
            Vec::new()
        }
        Target::ResultsBar => {
            view.grabbed = Some(Grab::Results);
            dragged(view, area, y, texts);
            Vec::new()
        }
        Target::Choice(n) => {
            if view.play_in.as_ref().is_some_and(|q| q.cursor == n) {
                return view.play_in_choose(Some(n));
            }
            if let Some(q) = view.play_in.as_mut() {
                q.cursor = n;
            }
            Vec::new()
        }
        Target::Detail(t) => {
            view.focus = crate::fast_access::Focus::Detail;
            let hit = view.current().cloned();
            crate::fast_access_detail::pressed(view, hit.as_ref(), &lay, t, (x, y), texts)
        }
    }
}

/// A path as the person reads it: their home as `~`.
#[must_use]
pub fn tilde(path: &std::path::Path) -> String {
    let shown = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && shown.starts_with(&home) => {
            format!("~{}", &shown[home.len()..])
        }
        _ => shown,
    }
}

/// The host of an address, for a row's quieter line.
#[must_use]
pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#'])
        .next()
        .unwrap_or(rest)
        .to_owned()
}

/// The time in a zone `offset_s` ahead of UTC, at `now_s`: `14:05`. Pure.
#[must_use]
pub fn zone_time(offset_s: i32, now_s: i64) -> String {
    let c = ronoctua_files::when::civil(
        now_s + i64::from(offset_s),
        &ronoctua_files::when::Zone::utc(),
    );
    format!("{:02}:{:02}", c.hour, c.minute)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fast_access::{Ground, Section, Settings};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the language")
    }

    fn ground() -> Ground {
        Ground {
            catalogue: std::sync::Arc::default(),
            commands: Vec::new(),
            zone: ronoctua_files::when::Zone::utc(),
            zoneinfo: None,
            queue_keys: String::new(),
        }
    }

    fn area(columns: u16, lines: u16) -> Area {
        Area {
            left: 1,
            top: 2,
            columns,
            lines,
            right_px: Area::x_of(columns + 2),
            bottom_px: Area::y_of(lines + 2),
        }
    }

    fn icon(c: Collection, index: usize) -> Hit {
        Hit {
            id: format!("icon:{}:{index}", c.id()),
            title: "x".to_owned(),
            note: Some("x".to_owned()),
            what: What::Icon(crate::icons_page::Icon {
                collection: c,
                index,
            }),
        }
    }

    fn file(n: usize) -> Hit {
        Hit {
            id: format!("file:/f/{n}"),
            title: format!("{n}"),
            note: None,
            what: What::File {
                path: std::path::PathBuf::from(format!("/f/{n}")),
                folder: false,
                content: false,
                mime: None,
                size_bytes: None,
                mtime_s: None,
            },
        }
    }

    fn view_of(text: &str, sections: Vec<Section>) -> View {
        let t = texts();
        let mut v = View::new(Settings::default(), &t);
        v.set_text(text);
        v.with_sections(sections);
        v
    }

    fn strips(lay: &Lay) -> Vec<(usize, Vec<usize>, bool, bool)> {
        lay.lines
            .iter()
            .filter_map(|l| match &l.kind {
                Kind::Strip {
                    collection,
                    hits,
                    earlier,
                    later,
                } => Some((*collection, hits.clone(), *earlier, *later)),
                _ => None,
            })
            .collect()
    }

    /// FAILS IF: the icons are not EXACTLY three strips — one per
    /// collection — or a page mark is drawn where nothing is hidden, or a
    /// click on `›` does not turn the page.
    #[test]
    fn three_strips_paged_with_marks_only_when_icons_hide() {
        let t = texts();
        let mut hits = Vec::new();
        for c in Collection::ALL {
            for n in 0..40 {
                hits.push(icon(c, n));
            }
        }
        let mut view = view_of(
            "/g arrow",
            vec![Section {
                id: SectionId::Icons,
                filled: Filled::Hits(hits),
            }],
        );
        let a = area(60, 30);
        let lay = Lay::of(&view, a, &t);
        let s = strips(&lay);
        assert_eq!(s.len(), 3, "{s:?}");
        let per = s[0].1.len();
        assert!(per > 1 && per < 40);
        assert!(s.iter().all(|(_, _, earlier, later)| !earlier && *later));
        let errands = pressed(&mut view, Target::Later(0), a, (0.0, 0.0), &ground(), &t);
        assert!(errands.is_empty());
        assert_eq!(view.strip_pages[0], per);
        let lay = Lay::of(&view, a, &t);
        let s = strips(&lay);
        assert!(s[0].2, "the first strip hides icons before its page");
        assert!(!s[1].2, "the second strip turned with the first");
        // A STRIP THAT HOLDS ALL ITS ICONS draws no mark — and a collection
        // with nothing that matches keeps its strip, saying so, so there are
        // still EXACTLY three, in their order.
        let few = view_of(
            "/g arrow",
            vec![Section {
                id: SectionId::Icons,
                filled: Filled::Hits(vec![
                    icon(Collection::Lucide, 0),
                    icon(Collection::Lucide, 1),
                ]),
            }],
        );
        let lay = Lay::of(&few, a, &t);
        let s = strips(&lay);
        assert_eq!(s.len(), 3, "{s:?}");
        assert_eq!(
            s.iter().map(|x| x.0).collect::<Vec<_>>(),
            [0, 1, 2],
            "the strips are not in the collections' order"
        );
        assert!(s[0].1.is_empty() && s[2].1.is_empty());
        assert!(!s[1].2 && !s[1].3);
        // AN EMPTY STRIP IS NO ROW THE CURSOR WALKS INTO.
        assert!(lay.rows.iter().all(|(_, hits)| !hits.is_empty()));
        let mut drawing = Drawing::default();
        draw(&few, a, true, &[], &t, &mut drawing);
        let none = t.t_with(
            "fast-access-strip-none",
            &[("collection", &t.t("fast-access-strip-nerd-fonts"))],
        );
        assert!(
            drawing.spans.iter().any(|sp| sp.text == none),
            "an empty strip does not say so"
        );
    }

    /// FAILS IF: a click on `›` turns the page under the cursor's icon and
    /// leaves the cursor on an icon nobody sees — every arrow key then dead —
    /// or a key on such a cursor does nothing; or the `›` touches the last
    /// icon's cell.
    #[test]
    fn a_page_turned_by_a_click_takes_the_cursor_with_it() {
        let t = texts();
        let hits: Vec<Hit> = (0..40).map(|n| icon(Collection::NerdFonts, n)).collect();
        let mut view = view_of(
            "/g a",
            vec![Section {
                id: SectionId::Icons,
                filled: Filled::Hits(hits),
            }],
        );
        let a = area(60, 30);
        view.point_at(&format!("icon:{}:0", Collection::NerdFonts.id()));
        let per = strips(&Lay::of(&view, a, &t))[0].1.len();
        let _ = pressed(&mut view, Target::Later(0), a, (0.0, 0.0), &ground(), &t);
        let lay = Lay::of(&view, a, &t);
        let place = view.current().and_then(|c| lay.place_of(&c.id));
        assert!(
            place.is_some_and(|p| lay.rows.iter().any(|(_, r)| r.contains(&p))),
            "the cursor stayed on an icon turned out of sight"
        );
        assert_eq!(
            view.current().map(|h| h.id.clone()),
            Some(format!("icon:{}:{per}", Collection::NerdFonts.id()))
        );
        // AND THE KEYS GO ON: right walks the page shown.
        moved(&mut view, &Press::Right, a, &t);
        assert_eq!(
            view.current().map(|h| h.id.clone()),
            Some(format!("icon:{}:{}", Collection::NerdFonts.id(), per + 1))
        );
        // A CURSOR ON A HIDDEN ICON (a page turned another way) brings its
        // page back on the next key, and the key acts.
        view.point_at(&format!("icon:{}:0", Collection::NerdFonts.id()));
        moved(&mut view, &Press::Right, a, &t);
        assert_eq!(
            view.current().map(|h| h.id.clone()),
            Some(format!("icon:{}:1", Collection::NerdFonts.id()))
        );
        // THE `›` HAS A CELL OF AIR after the last icon's.
        let lay = Lay::of(&view, a, &t);
        let line = lay
            .lines
            .iter()
            .find(|l| matches!(l.kind, Kind::Strip { collection: 0, .. }))
            .expect("a strip");
        let last_icon = line
            .cells
            .iter()
            .filter(|c| matches!(c.target, Target::Hit(_)))
            .map(|c| c.from + c.width)
            .max()
            .expect("icons");
        let mark = line
            .cells
            .iter()
            .find(|c| matches!(c.target, Target::Later(_)))
            .expect("the `›`");
        assert!(mark.from > last_icon, "{} > {last_icon}", mark.from);
        assert!(mark.from + mark.width <= a.left + a.columns - crate::chrome::BAR_CELLS);
    }

    fn video(n: usize) -> Hit {
        Hit {
            id: format!("yt:{n}"),
            title: format!("{n}"),
            note: None,
            what: What::Video(Box::new(crate::fast_access::Video {
                id: format!("{n}"),
                url: String::new(),
                channel: None,
                duration_s: None,
                views: None,
                short: false,
                local: true,
                thumb_file: None,
                thumb: None,
            })),
        }
    }

    /// FAILS IF: the grid scrolls into the middle of a row — its pictures
    /// gone and its titles under blank lines — by the wheel, a key or the
    /// bar; or a picture is drawn past the results' last line; or a row of
    /// the grid leaves cells empty at the right.
    #[test]
    fn the_grid_moves_by_whole_rows_and_its_pictures_stay_in_the_box() {
        let t = texts();
        let hits: Vec<Hit> = (0..24).map(video).collect();
        let mut view = view_of(
            "/y lo",
            vec![Section {
                id: SectionId::YouTube,
                filled: Filled::Hits(hits),
            }],
        );
        let a = area(100, 26);
        let starts = |lay: &Lay| -> Vec<usize> {
            lay.lines
                .iter()
                .enumerate()
                .filter_map(|(n, l)| matches!(l.kind, Kind::Grid { part: 0, .. }).then_some(n))
                .collect()
        };
        let lay = Lay::of(&view, a, &t);
        let rows = starts(&lay);
        assert!(rows.len() > 2, "{rows:?}");
        let shown = |v: &View| {
            let lay = Lay::of(v, a, &t);
            let first = lay.first(v.offset);
            match lay.lines.get(first).map(|l| &l.kind) {
                Some(Kind::Grid { part, .. }) => *part,
                _ => 0,
            }
        };
        // THE WHEEL, a few lines at a time.
        for _ in 0..12 {
            wheeled(&mut view, 3, a, &t);
            assert_eq!(shown(&view), 0, "the wheel stopped inside a row");
        }
        for _ in 0..12 {
            wheeled(&mut view, -3, a, &t);
            assert_eq!(shown(&view), 0, "the wheel stopped inside a row");
        }
        // THE KEYS: down to the last row, each step keeping the cursor's row
        // on screen from its pictures.
        view.cursor_to_end(false);
        for _ in 0..rows.len() {
            moved(&mut view, &Press::Down, a, &t);
            assert_eq!(shown(&view), 0, "a key scrolled into a row");
        }
        // THE BAR, held anywhere.
        for y in [0.3_f32, 0.55, 0.8] {
            dragged(
                &mut view,
                a,
                Area::y_of(a.top) + y * f32::from(a.lines) * Area::line(),
                &t,
            );
            assert_eq!(shown(&view), 0, "the bar stopped inside a row");
        }
        // NO PICTURE PAST THE RESULTS' LAST LINE.
        let mut drawing = Drawing::default();
        view.pictures.insert(
            "k".to_owned(),
            std::sync::Arc::new(ronoctua_render::image::Rgba {
                width: 16,
                height: 9,
                pixels: vec![0; 16 * 9 * 4],
            }),
        );
        for h in 0..24 {
            let _ = view.thumbed(&format!("{h}"), "k");
        }
        view.offset = 0;
        draw(&view, a, true, &[], &t, &mut drawing);
        let lay = Lay::of(&view, a, &t);
        let floor = Area::y_of(lay.results.top + lay.results.lines);
        assert!(!drawing.pictures.is_empty());
        for p in &drawing.pictures {
            assert!(
                p.area.y + p.area.height <= floor + 0.5,
                "{:?} past {floor}",
                p.area
            );
        }
        // THE ROW SPREADS ITS LEFTOVER: no strip of empty cells at the right.
        let line = lay
            .lines
            .iter()
            .find(|l| matches!(l.kind, Kind::Grid { .. }))
            .expect("a row");
        let end = line
            .cells
            .iter()
            .map(|c| c.from + c.width)
            .max()
            .expect("cells");
        let room = lay.results.left + lay.results.columns - crate::chrome::BAR_CELLS;
        assert!(room - end < line.cells.len() as u16, "{end} of {room}");
    }

    /// FAILS IF: `→` at a strip's last shown icon does not turn its page.
    #[test]
    fn the_arrow_turns_a_strip_at_its_edge() {
        let t = texts();
        let hits: Vec<Hit> = (0..40).map(|n| icon(Collection::NerdFonts, n)).collect();
        let mut view = view_of(
            "/g a",
            vec![Section {
                id: SectionId::Icons,
                filled: Filled::Hits(hits),
            }],
        );
        let a = area(60, 30);
        let per = strips(&Lay::of(&view, a, &t))[0].1.len();
        view.point_at(&format!("icon:{}:{}", Collection::NerdFonts.id(), per - 1));
        moved(&mut view, &Press::Right, a, &t);
        assert_eq!(
            view.cursor_id(),
            Some(format!("icon:{}:{per}", Collection::NerdFonts.id()).as_str())
        );
        let s = strips(&Lay::of(&view, a, &t));
        assert!(s[0].2, "the page did not turn");
        assert_eq!(s[0].1.first().copied(), Some(per));
    }

    /// FAILS IF: a wide area does not put the detail beside the results, a
    /// narrow one does not put it below in its own block, or a press on the
    /// divider does not widen the detail.
    #[test]
    fn the_detail_is_beside_or_below_and_the_divider_widens_it() {
        let t = texts();
        let hits: Vec<Hit> = (0..5).map(file).collect();
        let mut view = view_of(
            "f",
            vec![Section {
                id: SectionId::Files,
                filled: Filled::Hits(hits),
            }],
        );
        let wide = area(160, 40);
        let lay = Lay::of(&view, wide, &t);
        assert!(
            matches!(lay.divider, Some(Divider::Beside { .. })),
            "{:?}",
            lay.divider
        );
        let narrow_detail = lay.detail.expect("a detail").columns;
        let errands = pressed(&mut view, Target::Divider, wide, (0.0, 0.0), &ground(), &t);
        assert!(errands.is_empty());
        assert!(view.wide);
        let wider = Lay::of(&view, wide, &t).detail.expect("a detail").columns;
        assert!(wider > narrow_detail, "{wider} > {narrow_detail}");
        view.wide = false;
        let narrow = area(70, 40);
        let lay = Lay::of(&view, narrow, &t);
        let Some(Divider::Below { line, .. }) = lay.divider else {
            panic!("not below: {:?}", lay.divider);
        };
        let detail = lay.detail.expect("a detail");
        assert_eq!(detail.lines, tokens::space::FASTACCESS_BELOWLINES);
        assert_eq!(detail.top, line + 1);
        assert!(lay.results.lines < narrow.lines);
        // BELOW, A PRESS ON THE RULE MAKES THE DETAIL TALLER — it stays below.
        assert!(lay.toggles);
        let _ = pressed(
            &mut view,
            Target::Divider,
            narrow,
            (0.0, 0.0),
            &ground(),
            &t,
        );
        let lay = Lay::of(&view, narrow, &t);
        assert!(matches!(lay.divider, Some(Divider::Below { .. })));
        assert!(lay.detail.expect("a detail").lines > tokens::space::FASTACCESS_BELOWLINES);
        // EVERY WIDTH: the press changes the width or the height, never
        // beside into below; and where it would change nothing it is no
        // control.
        for columns in 60..200 {
            let at = area(columns, 40);
            view.wide = false;
            let normal = Lay::of(&view, at, &t);
            view.wide = true;
            let widened = Lay::of(&view, at, &t);
            assert_eq!(
                std::mem::discriminant(&normal.divider),
                std::mem::discriminant(&widened.divider),
                "{columns} columns"
            );
            if let (Some(Divider::Beside { .. }), Some(Divider::Beside { .. })) =
                (normal.divider, widened.divider)
            {
                let (n, w) = (
                    normal.detail.expect("d").columns,
                    widened.detail.expect("d").columns,
                );
                assert_eq!(normal.toggles, w > n, "{columns} columns");
                let on = Area::x_of(match normal.divider {
                    Some(Divider::Beside { column, .. }) => column,
                    _ => 0,
                }) + 1.0;
                let hit = normal.target_at(&view, on, Area::y_of(at.top + 3), &t);
                assert_eq!(
                    hit == Some(Target::Divider),
                    normal.toggles,
                    "{columns} columns"
                );
            }
        }
        view.wide = false;
        // AND A SCOPE THAT TAKES THE WHOLE AREA has no detail at all.
        let card = view_of("/d word", Vec::new());
        assert!(Lay::of(&card, wide, &t).detail.is_none());
    }

    /// FAILS IF: a search that found nothing draws a blank page instead of
    /// saying so, or says so while a section is still searching.
    #[test]
    fn nothing_found_is_said() {
        let t = texts();
        let notes = |v: &View| -> Vec<String> {
            Lay::of(v, area(100, 30), &t)
                .lines
                .iter()
                .filter_map(|l| match &l.kind {
                    Kind::Note(text, _) => Some(text.clone()),
                    _ => None,
                })
                .collect()
        };
        let empty = view_of(
            "zzqx",
            vec![Section {
                id: SectionId::Files,
                filled: Filled::Hits(Vec::new()),
            }],
        );
        assert!(
            notes(&empty).iter().any(|n| n.contains("zzqx")),
            "{:?}",
            notes(&empty)
        );
        let waiting = view_of(
            "zzqx",
            vec![Section {
                id: SectionId::Files,
                filled: Filled::Pending,
            }],
        );
        assert!(notes(&waiting).iter().all(|n| !n.contains("zzqx")));
    }

    /// FAILS IF: a target's code does not come back as the same target — the
    /// hover of one hand would light another's.
    #[test]
    fn every_target_survives_its_code() {
        for t in [
            Target::Hit(0),
            Target::Hit(4096),
            Target::Earlier(2),
            Target::Later(1),
            Target::Divider,
            Target::ResultsBar,
            Target::Choice(2),
            Target::Detail(crate::fast_access_detail::Target::Row(3)),
            Target::Detail(crate::fast_access_detail::Target::Bar),
            Target::Detail(crate::fast_access_detail::Target::Cell(4, 17)),
            Target::Detail(crate::fast_access_detail::Target::Answer(1)),
            Target::Detail(crate::fast_access_detail::Target::FileBar(
                crate::fields::Pane::Info,
                crate::fields::Axis::Down,
            )),
            Target::Detail(crate::fast_access_detail::Target::FileBar(
                crate::fields::Pane::Preview,
                crate::fields::Axis::Time,
            )),
            Target::Detail(crate::fast_access_detail::Target::FileBar(
                crate::fields::Pane::Preview,
                crate::fields::Axis::Across,
            )),
        ] {
            assert_eq!(Target::of_code(t.code()), t, "{t:?}");
        }
    }
}
