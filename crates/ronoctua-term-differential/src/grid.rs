//! The canonical grid: the one form two engines can be compared in.
//!
//! Each engine reads its own cells into a [`Grid`] of [`CellBox`]es, in the
//! terms BOTH keep — the text, the style bits they share, the colour as the
//! program named it, the width — and the differential compares the two grids
//! and writes each one's goldens from them. Whatever only one engine keeps
//! has no field here, on purpose: a field the other cannot fill would be a
//! divergence in every case that touches it.

use std::fmt::Write as _;

/// A colour as a cell carries it — before any palette resolves it.
///
/// Both engines keep the distinction a program made: `SGR 31` is palette
/// index 1, `SGR 38;2;…` is an RGB triple, and no SGR at all is the default.
/// Comparing RESOLVED colours instead would make two engines with different
/// palettes disagree about a cell they interpreted identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Colour {
    /// No colour was asked for: the terminal's own foreground or background.
    #[default]
    Default,
    /// An entry of the 256-colour palette.
    Palette(u8),
    /// A direct colour.
    Rgb(u8, u8, u8),
}

impl Colour {
    /// The canonical spelling: `p1`, `#ff8000`, or nothing for the default.
    fn token(self) -> Option<String> {
        match self {
            Self::Default => None,
            Self::Palette(i) => Some(format!("p{i}")),
            Self::Rgb(r, g, b) => Some(format!("#{r:02x}{g:02x}{b:02x}")),
        }
    }
}

/// The attributes of a cell that BOTH engines keep.
///
/// Blink, overline, the underline's kind and colour exist in Ghostty and not
/// in alacritty; a field one engine cannot fill would be a divergence in
/// every case that touches it, and would hide the real ones. Those are pinned
/// by the Ghostty engine's own tests instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[allow(clippy::struct_excessive_bools)]
pub struct Style {
    /// SGR 1.
    pub bold: bool,
    /// SGR 3.
    pub italic: bool,
    /// SGR 2 (alacritty calls it dim).
    pub faint: bool,
    /// SGR 7.
    pub inverse: bool,
    /// SGR 8 (alacritty calls it hidden).
    pub invisible: bool,
    /// Any underline, whatever its kind (SGR 4, 4:2…4:5, 21).
    pub underline: bool,
    /// SGR 9.
    pub strikethrough: bool,
    /// The foreground.
    pub fg: Colour,
    /// The background.
    pub bg: Colour,
}

/// How many columns a cell's text takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Width {
    /// One column.
    #[default]
    Narrow,
    /// The first of two columns.
    Wide,
    /// A column that belongs to a wide character drawn elsewhere: the second
    /// half of one, or the end of a line a wide character did not fit on.
    /// Ghostty names the two (tail and head) and alacritty names them too;
    /// both are "draw nothing here", which is all the grid compares.
    Spacer,
}

/// A cell of the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellBox {
    /// The grapheme cluster: a base character and whatever combines with it.
    /// A space when empty.
    pub text: String,
    /// What the cell looks like, in the terms both engines share.
    pub style: Style,
    /// How many columns it takes.
    pub width: Width,
}

impl CellBox {
    /// An unstyled, narrow cell holding one character.
    #[must_use]
    pub fn of(c: char) -> Self {
        Self {
            text: c.to_string(),
            ..Self::default()
        }
    }

    /// The first character, or a space for an empty cell.
    #[must_use]
    pub fn first(&self) -> char {
        self.text.chars().next().unwrap_or(' ')
    }
}

impl Default for CellBox {
    fn default() -> Self {
        Self {
            text: " ".to_owned(),
            style: Style::default(),
            width: Width::Narrow,
        }
    }
}

/// The character as the canonical form sees it.
///
/// **A tab becomes a space.** In a terminal, TAB is MOVEMENT, never content:
/// alacritty stores a `\t` in the cell the jump started from and fills the rest
/// with spaces; another engine may fill it all with spaces. Both are in the
/// same visual state, and without this normalisation the differential would flag
/// divergence in every case with a tab — a false positive that would hide
/// the actual ones.
///
/// Control characters in general fall back to a space for the same reason: there is no
/// control character "on screen", only its effect.
fn canonical(c: char) -> char {
    if c.is_control() { ' ' } else { c }
}

/// A cell's text as the canonical form sees it: [`canonical`] on every
/// character, and an empty cell is a space.
fn canonical_text(cell: &CellBox) -> String {
    let text: String = cell.text.chars().map(canonical).collect();
    if text.is_empty() {
        " ".to_owned()
    } else {
        text
    }
}

/// A terminal's grid, in canonical form.
///
/// **Canonical** is the word that matters: two engines can only be compared
/// if they produce the same representation for the same state. Trailing whitespace
/// at end of line is removed during serialisation precisely for that — an engine
/// that pads the line and one that leaves it short are in the SAME state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    /// Columns.
    pub columns: u16,
    /// Rows.
    pub lines: u16,
    cells: Vec<CellBox>,
    /// Where the cursor is: `(row, column)`.
    pub cursor: (u16, u16),
}

impl Grid {
    /// An empty grid of the given size.
    #[must_use]
    pub fn fresh_one(columns: u16, lines: u16) -> Self {
        Self {
            columns,
            lines,
            cells: vec![CellBox::default(); usize::from(columns) * usize::from(lines)],
            cursor: (0, 0),
        }
    }

    /// Reads a cell. Outside the grid it returns an empty one — it does not panic.
    #[must_use]
    pub fn cell(&self, line: u16, column: u16) -> CellBox {
        if line >= self.lines || column >= self.columns {
            return CellBox::default();
        }
        self.cells[usize::from(line) * usize::from(self.columns) + usize::from(column)].clone()
    }

    /// Writes a cell. Outside the grid it is ignored, on purpose: an engine
    /// emitting an invalid coordinate must not take the comparator down.
    pub fn put_at(&mut self, line: u16, column: u16, c: CellBox) {
        if line >= self.lines || column >= self.columns {
            return;
        }
        let i = usize::from(line) * usize::from(self.columns) + usize::from(column);
        self.cells[i] = c;
    }

    /// The visible lines, already canonicalised. **Without the cursor's line.**
    ///
    /// It is what gets DRAWN. [`Grid::canonical_f`] serves to compare two
    /// engines and for that reason carries the cursor's position at the end; drawing
    /// that would put `cursor: 3,0` on screen as if it were terminal content.
    #[must_use]
    pub fn visible_rows(&self) -> Vec<String> {
        (0..self.lines)
            .map(|l| {
                let mut line = String::with_capacity(usize::from(self.columns));
                for c in 0..self.columns {
                    line.push_str(&canonical_text(&self.cell(l, c)));
                }
                line.trim_end().to_owned()
            })
            .collect()
    }

    /// Canonical form as text, one line per line.
    ///
    /// Trailing space REMOVED: it is what makes two engines comparable. The
    /// CURSOR goes at the end because two grids with the same text and the cursor
    /// in different places are not in the same state — whoever only wants what
    /// appears on screen uses [`Grid::visible_rows`].
    ///
    /// Characters and cursor only, on purpose: the goldens of
    /// `tests/golden-vt/` were recorded in this form before cells carried a
    /// style, and the style has its own form ([`Grid::canonical_styled`])
    /// with its own goldens, so neither moves when the other does.
    #[must_use]
    pub fn canonical_f(&self) -> String {
        let mut s = String::new();
        for line in self.visible_rows() {
            let _ = writeln!(s, "{line}");
        }
        let _ = write!(s, "cursor: {},{}", self.cursor.0, self.cursor.1);
        s
    }

    /// The style of the grid as text: one line per RUN of equally styled
    /// cells, `row:first-last attributes`, and unstyled narrow cells left out.
    ///
    /// Runs and not cells, so a golden says "this word is bold" in one line
    /// rather than in five; and unstyled cells left out, so a golden shows
    /// what the case is about instead of every blank of the screen.
    #[must_use]
    pub fn canonical_styled(&self) -> String {
        let mut s = String::new();
        for l in 0..self.lines {
            let mut run: Option<(u16, u16, String)> = None;
            for c in 0..self.columns {
                let key = attributes(&self.cell(l, c));
                match &mut run {
                    Some((_, last, k)) if *k == key => *last = c,
                    _ => {
                        if let Some((first, last, k)) = run.take()
                            && !k.is_empty()
                        {
                            let _ = writeln!(s, "{l}:{first}-{last} {k}");
                        }
                        run = Some((c, c, key));
                    }
                }
            }
            if let Some((first, last, k)) = run
                && !k.is_empty()
            {
                let _ = writeln!(s, "{l}:{first}-{last} {k}");
            }
        }
        if s.is_empty() {
            s.push_str("no styled cell\n");
        }
        s
    }

    /// Differences between two grids, readable.
    ///
    /// Empty when they are equal. The message says **where** and **what**, because
    /// "the grids differ" helps nobody find the interpretation bug.
    #[must_use]
    pub fn diff(&self, otherwise_one: &Self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(size) = self.size_differs(otherwise_one) {
            out.push(size);
            return out;
        }
        for l in 0..self.lines {
            for c in 0..self.columns {
                // It compares the CANONICAL form, not the raw character: otherwise the
                // diff would flag `\t` against ` ` as a divergence.
                let (a, b) = (
                    canonical_text(&self.cell(l, c)),
                    canonical_text(&otherwise_one.cell(l, c)),
                );
                if a != b {
                    out.push(format!("cell {l},{c}: {a:?} against {b:?}"));
                }
            }
        }
        if self.cursor != otherwise_one.cursor {
            out.push(format!(
                "cursor: {:?} against {:?}",
                self.cursor, otherwise_one.cursor
            ));
        }
        out
    }

    /// Differences of style and width between two grids, cell by cell.
    ///
    /// A separate question from [`Grid::diff`] so a report says which kind of
    /// disagreement it is: a wrong character and a wrong colour are bugs in
    /// different halves of an engine.
    #[must_use]
    pub fn style_diff(&self, otherwise_one: &Self) -> Vec<String> {
        if let Some(size) = self.size_differs(otherwise_one) {
            return vec![size];
        }
        let mut out = Vec::new();
        for l in 0..self.lines {
            for c in 0..self.columns {
                let (a, b) = (
                    attributes(&self.cell(l, c)),
                    attributes(&otherwise_one.cell(l, c)),
                );
                if a != b {
                    out.push(format!("style {l},{c}: [{a}] against [{b}]"));
                }
            }
        }
        out
    }

    /// Comparing grids of different sizes cell by cell would produce hundreds
    /// of lines of noise, hiding the single cause.
    fn size_differs(&self, other: &Self) -> Option<String> {
        ((self.columns, self.lines) != (other.columns, other.lines)).then(|| {
            format!(
                "size: {}x{} against {}x{}",
                self.columns, self.lines, other.columns, other.lines
            )
        })
    }
}

/// A cell's attributes as one canonical string, empty for an unstyled
/// narrow cell. The order is fixed, so two equal cells give equal strings.
fn attributes(cell: &CellBox) -> String {
    let st = cell.style;
    let mut parts: Vec<String> = [
        (st.bold, "bold"),
        (st.italic, "italic"),
        (st.faint, "faint"),
        (st.inverse, "inverse"),
        (st.invisible, "invisible"),
        (st.underline, "underline"),
        (st.strikethrough, "strike"),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, name)| name.to_owned())
    .collect();
    if let Some(fg) = st.fg.token() {
        parts.push(format!("fg={fg}"));
    }
    if let Some(bg) = st.bg.token() {
        parts.push(format!("bg={bg}"));
    }
    match cell.width {
        Width::Narrow => {}
        Width::Wide => parts.push("wide".to_owned()),
        Width::Spacer => parts.push("spacer".to_owned()),
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_canonical_form_ignores_trailing_space() {
        // The canonical form's reason to exist: an engine that pads the line
        // with spaces and one that leaves it short are in the SAME state, and without
        // this the differential would flag divergence on every line.
        let mut a = Grid::fresh_one(10, 1);
        a.put_at(0, 0, CellBox::of('o'));
        a.put_at(0, 1, CellBox::of('i'));

        let mut b = Grid::fresh_one(10, 1);
        b.put_at(0, 0, CellBox::of('o'));
        b.put_at(0, 1, CellBox::of('i'));
        for c in 2..10 {
            b.put_at(0, c, CellBox::of(' '));
        }
        assert_eq!(a.canonical_f(), b.canonical_f());
    }

    #[test]
    fn tab_and_space_are_the_same_state() {
        // Without this, every case with a tab would give a false positive in the
        // differential — and a false positive hides the actual one.
        let mut a = Grid::fresh_one(4, 1);
        a.put_at(0, 0, CellBox::of('\t'));
        a.put_at(0, 1, CellBox::of('x'));
        let mut b = Grid::fresh_one(4, 1);
        b.put_at(0, 0, CellBox::of(' '));
        b.put_at(0, 1, CellBox::of('x'));
        assert_eq!(a.canonical_f(), b.canonical_f());
        assert!(a.diff(&b).is_empty());
    }

    #[test]
    fn an_empty_cell_and_a_space_are_the_same_state() {
        // FAILS IF an engine that leaves a cell empty (Ghostty's spacer has no
        // text at all) and one that stores a space (alacritty's) are told apart
        // — every wide character would be a divergence.
        let mut a = Grid::fresh_one(4, 1);
        a.put_at(
            0,
            1,
            CellBox {
                text: String::new(),
                ..CellBox::default()
            },
        );
        let b = Grid::fresh_one(4, 1);
        assert!(a.diff(&b).is_empty(), "{:?}", a.diff(&b));
    }

    #[test]
    fn the_diff_says_where_and_what() {
        let mut a = Grid::fresh_one(4, 1);
        a.put_at(0, 2, CellBox::of('x'));
        let b = Grid::fresh_one(4, 1);
        let d = a.diff(&b);
        assert_eq!(d.len(), 1);
        assert!(d[0].contains("0,2"), "{d:?}");
    }

    #[test]
    fn different_sizes_stop_the_diff_early() {
        // Comparing grids of different sizes cell by cell would produce
        // hundreds of lines of noise, hiding the single cause.
        let d = Grid::fresh_one(4, 1).diff(&Grid::fresh_one(8, 1));
        assert_eq!(d.len(), 1);
        assert!(d[0].starts_with("size"));
        let d = Grid::fresh_one(4, 1).style_diff(&Grid::fresh_one(8, 1));
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn outside_the_grid_does_not_bring_it_down() {
        // An engine with a coordinate bug must not take the comparator down: the
        // comparator's whole purpose is to report the bug.
        let mut g = Grid::fresh_one(2, 2);
        g.put_at(99, 99, CellBox::of('x'));
        assert_eq!(g.cell(99, 99).first(), ' ');
    }

    #[test]
    fn styled_cells_are_written_as_runs() {
        // FAILS IF the styled form writes one line per cell (a golden nobody
        // can read) or merges cells whose styles differ (a golden that hides
        // the very difference it records).
        let mut g = Grid::fresh_one(8, 2);
        let bold = Style {
            bold: true,
            fg: Colour::Palette(1),
            ..Style::default()
        };
        for c in 1..4 {
            g.put_at(
                0,
                c,
                CellBox {
                    style: bold,
                    ..CellBox::of('x')
                },
            );
        }
        g.put_at(
            1,
            0,
            CellBox {
                width: Width::Wide,
                ..CellBox::of('日')
            },
        );
        g.put_at(
            1,
            1,
            CellBox {
                width: Width::Spacer,
                ..CellBox::default()
            },
        );
        g.put_at(
            1,
            2,
            CellBox {
                style: Style {
                    bg: Colour::Rgb(0xff, 0x80, 0),
                    ..Style::default()
                },
                ..CellBox::default()
            },
        );
        assert_eq!(
            g.canonical_styled(),
            "0:1-3 bold fg=p1\n1:0-0 wide\n1:1-1 spacer\n1:2-2 bg=#ff8000\n"
        );
    }

    #[test]
    fn an_unstyled_grid_says_so() {
        // FAILS IF an unstyled grid writes an empty golden: an empty file and
        // a file that was never written look alike in a review.
        assert_eq!(Grid::fresh_one(4, 2).canonical_styled(), "no styled cell\n");
    }

    #[test]
    fn the_style_diff_names_the_cell_and_both_styles() {
        // FAILS IF a style disagreement is reported without where it is, or
        // is not reported at all when only the colour differs.
        let mut a = Grid::fresh_one(4, 1);
        a.put_at(
            0,
            3,
            CellBox {
                style: Style {
                    fg: Colour::Palette(2),
                    ..Style::default()
                },
                ..CellBox::of('x')
            },
        );
        let mut b = Grid::fresh_one(4, 1);
        b.put_at(0, 3, CellBox::of('x'));
        let d = a.style_diff(&b);
        assert_eq!(d, ["style 0,3: [fg=p2] against []"]);
        assert!(a.diff(&b).is_empty(), "the characters agree");
    }
}
