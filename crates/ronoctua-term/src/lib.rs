//! Terminal engine — section 7.10, risk R1.
//!
//! # The trait is what makes the risk non-blocking
//!
//! Section 7.10 wants `libghostty-vt`. It is pre-1.0, fetched from a pinned
//! commit, and needs an FOD with the Zig package cache — milestone 7's item of
//! highest variance, recorded as risk R1.
//!
//! [`Engine`] exists so that **nothing is blocked by that**. The rest of the system
//! talks to the trait; swapping the implementation underneath touches nothing else.
//! The milestone closes with `alacritty_terminal`, and Ghostty comes later — which
//! is exactly the mitigation R1 foresaw.
//!
//! # The differential test
//!
//! Two engines interpreting the SAME VT corpus have to produce the SAME
//! grid. [`Grid`] exists for that: it is the canonical form both
//! produce, and the diff between them is the verdict.
//!
//! When there is only one engine, the comparison degrades to a golden file — the
//! recorded grid plays the part of the second. It is weaker and it is said so: what it catches
//! is regression, not divergence of interpretation.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::fmt::Write as _;

/// A cell of the grid.
///
/// Only what the differential diff has to compare. Colour, bold and underline
/// come in once there is a second engine to disagree about them — today they would be
/// fields nobody compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellBox {
    /// The character. A space when empty.
    pub c: char,
}

impl Default for CellBox {
    fn default() -> Self {
        Self { c: ' ' }
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
        self.cells[usize::from(line) * usize::from(self.columns) + usize::from(column)]
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
                    line.push(canonical(self.cell(l, c).c));
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
    #[must_use]
    pub fn canonical_f(&self) -> String {
        let mut s = String::new();
        for line in self.visible_rows() {
            let _ = writeln!(s, "{line}");
        }
        let _ = write!(s, "cursor: {},{}", self.cursor.0, self.cursor.1);
        s
    }

    /// Differences between two grids, readable.
    ///
    /// Empty when they are equal. The message says **where** and **what**, because
    /// "the grids differ" helps nobody find the interpretation bug.
    #[must_use]
    pub fn diff(&self, otherwise_one: &Self) -> Vec<String> {
        let mut out = Vec::new();
        if (self.columns, self.lines) != (otherwise_one.columns, otherwise_one.lines) {
            out.push(format!(
                "tamanho: {}x{} contra {}x{}",
                self.columns, self.lines, otherwise_one.columns, otherwise_one.lines
            ));
            return out;
        }
        for l in 0..self.lines {
            for c in 0..self.columns {
                // It compares the CANONICAL form, not the raw character: otherwise the
                // diff would flag `\t` against ` ` as a divergence.
                let (a, b) = (
                    canonical(self.cell(l, c).c),
                    canonical(otherwise_one.cell(l, c).c),
                );
                if a != b {
                    out.push(format!("celula {l},{c}: {a:?} contra {b:?}"));
                }
            }
        }
        if self.cursor != otherwise_one.cursor {
            out.push(format!(
                "cursor: {:?} contra {:?}",
                self.cursor, otherwise_one.cursor
            ));
        }
        out
    }
}

/// A terminal engine.
///
/// Deliberately small. What the differential test needs is: feed
/// bytes, read the grid. Resizing is in because it changes the interpretation of
/// line wrapping, which is where engines usually diverge.
pub trait Engine {
    /// A name, so the diff message can say WHO disagreed.
    fn name(&self) -> &'static str;

    /// Feeds in bytes coming from the PTY.
    fn feed(&mut self, bytes: &[u8]);

    /// The grid right now.
    fn grid(&self) -> Grid;

    /// Resizes.
    fn resize(&mut self, columns: u16, lines: u16);
}

pub mod alacritty;
pub mod pty;
pub mod terminal;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_canonical_form_ignores_trailing_space() {
        // The canonical form's reason to exist: an engine that pads the line
        // with spaces and one that leaves it short are in the SAME state, and without
        // this the differential would flag divergence on every line.
        let mut a = Grid::fresh_one(10, 1);
        a.put_at(0, 0, CellBox { c: 'o' });
        a.put_at(0, 1, CellBox { c: 'i' });

        let mut b = Grid::fresh_one(10, 1);
        b.put_at(0, 0, CellBox { c: 'o' });
        b.put_at(0, 1, CellBox { c: 'i' });
        for c in 2..10 {
            b.put_at(0, c, CellBox { c: ' ' });
        }
        assert_eq!(a.canonical_f(), b.canonical_f());
    }

    #[test]
    fn tab_and_space_are_the_same_state() {
        // Without this, every case with a tab would give a false positive in the
        // differential — and a false positive hides the actual one.
        let mut a = Grid::fresh_one(4, 1);
        a.put_at(0, 0, CellBox { c: '\t' });
        a.put_at(0, 1, CellBox { c: 'x' });
        let mut b = Grid::fresh_one(4, 1);
        b.put_at(0, 0, CellBox { c: ' ' });
        b.put_at(0, 1, CellBox { c: 'x' });
        assert_eq!(a.canonical_f(), b.canonical_f());
        assert!(a.diff(&b).is_empty());
    }

    #[test]
    fn the_diff_says_where_and_what() {
        let mut a = Grid::fresh_one(4, 1);
        a.put_at(0, 2, CellBox { c: 'x' });
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
        assert!(d[0].starts_with("tamanho"));
    }

    #[test]
    fn outside_the_grid_does_not_bring_it_down() {
        // An engine with a coordinate bug must not take the comparator down: the
        // comparator's whole purpose is to report the bug.
        let mut g = Grid::fresh_one(2, 2);
        g.put_at(99, 99, CellBox { c: 'x' });
        assert_eq!(g.cell(99, 99).c, ' ');
    }
}
