//! Selecting text with the pointer, in every door: the footer, every window,
//! and every buffer (whose host is a window).
//!
//! # Why the selection is over what was DRAWN
//!
//! Because every host already knows the runs it painted — a `Span` is a text
//! at a column of a line — and nothing else knows the same thing in the same
//! way. A selection over a view's DATA would need every view to learn to
//! answer "which characters are between these two points", in thirty shapes;
//! a selection over the painted runs needs nobody to learn anything, and what
//! is copied is exactly what the eye swept over. An elided line copies with
//! its `…`, which is the price: what is copied is what was on screen.
//!
//! # Why it has an AREA
//!
//! Because a screen of two columns side by side is two texts, and a sweep down
//! the left one that took the right one's lines with it would copy a sentence
//! interleaved with facts. The area is the rectangle the drag STARTED in —
//! the whole width, or the half a door says the cell is in — and the sweep
//! never leaves it.
//!
//! # Why a press is not a selection
//!
//! A press is a click, and every click this system has must go on working. A
//! selection starts when a held button has moved a whole cell away from where
//! it went down (`Selecting::moved_to`); until then it is nobody's but the
//! click's.

use ronoctua_protocol::{cells_of, cells_of_one};

/// A run of text as it was painted: which line, which cell it starts at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Painted {
    /// The surface's line.
    pub line: u16,
    /// The cell it starts at.
    pub column: u16,
    /// What was drawn, already elided.
    pub text: String,
}

/// The rectangle a sweep may not leave, in cells: `[left, right)` by
/// `[top, bottom)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    /// First cell.
    pub left: u16,
    /// One past the last cell.
    pub right: u16,
    /// First line.
    pub top: u16,
    /// One past the last line.
    pub bottom: u16,
}

impl Area {
    /// Does it hold the cell?
    #[must_use]
    pub const fn holds(&self, line: u16, column: u16) -> bool {
        line >= self.top && line < self.bottom && column >= self.left && column < self.right
    }

    /// The nearest cell inside it: a sweep that runs off the edge keeps
    /// going along the edge.
    #[must_use]
    pub fn clamp(&self, line: u16, column: u16) -> (u16, u16) {
        (
            line.clamp(self.top, self.bottom.saturating_sub(1).max(self.top)),
            column.clamp(self.left, self.right.saturating_sub(1).max(self.left)),
        )
    }
}

/// A sweep of text: where it went down and where the pointer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Where it may go.
    pub area: Area,
    /// Where the button went down, `(line, cell)`.
    pub anchor: (u16, u16),
    /// Where the pointer is now.
    pub head: (u16, u16),
}

/// What one line of a selection covers: `[from, to)` cells of `line`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stretch {
    /// The line.
    pub line: u16,
    /// First cell.
    pub from: u16,
    /// One past the last.
    pub to: u16,
}

impl Selection {
    /// The two ends in reading order.
    fn ordered(&self) -> ((u16, u16), (u16, u16)) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// What each line covers, as text is read: from the first end to the
    /// area's right edge, whole lines between, and from the area's left edge
    /// to the last end — the last cell INCLUDED, because the pointer is on it.
    #[must_use]
    pub fn stretches(&self) -> Vec<Stretch> {
        let ((l0, c0), (l1, c1)) = self.ordered();
        (l0..=l1)
            .map(|line| {
                let from = if line == l0 { c0 } else { self.area.left };
                let to = if line == l1 {
                    c1.saturating_add(1)
                } else {
                    self.area.right
                };
                Stretch { line, from, to }
            })
            .filter(|s| s.to > s.from)
            .collect()
    }

    /// The text it covers, line by line, out of what was painted.
    ///
    /// A cluster belongs to the selection when the cell it STARTS on does —
    /// a wide character is taken whole or not at all. Trailing blanks go,
    /// and a line with nothing on it is an empty line of the copy: it was a
    /// blank line on screen.
    #[must_use]
    pub fn extract(&self, painted: &[Painted]) -> String {
        let mut out: Vec<String> = Vec::new();
        for s in self.stretches() {
            let mut runs: Vec<&Painted> = painted.iter().filter(|p| p.line == s.line).collect();
            runs.sort_by_key(|p| p.column);
            let mut line = String::new();
            // WHERE THE LAST CLUSTER TAKEN ENDED — `None` before the first,
            // so the margin before a line's text is not copied as spaces.
            let mut at_cell: Option<u16> = None;
            for run in runs {
                let mut cell = run.column;
                for cluster in cells_of(&run.text) {
                    let wide = cells_of_one(cluster).max(1);
                    if crate::chrome::in_cell(cell, s.from, s.to.saturating_sub(s.from))
                        && at_cell.is_none_or(|a| cell >= a)
                    {
                        // THE GAP BETWEEN TWO RUNS IS SPACES: two runs a cell
                        // apart read as two words.
                        if let Some(a) = at_cell {
                            for _ in a..cell {
                                line.push(' ');
                            }
                        }
                        line.push_str(cluster);
                        at_cell = Some(cell + wide);
                    }
                    cell += wide;
                }
            }
            out.push(line.trim_end().to_owned());
        }
        out.join("\n")
    }
}

/// A button held down that may become a sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Selecting {
    /// Nothing held.
    #[default]
    Idle,
    /// Held, not yet a cell away: still a click.
    Pressed {
        /// Where it went down.
        at: (u16, u16),
        /// The area the sweep would stay in.
        area: Area,
    },
    /// A sweep in progress.
    Sweeping(Selection),
}

impl Selecting {
    /// The held button moved to `(line, cell)`. `Some` is the sweep as it
    /// stands, when there is one.
    pub fn moved_to(&mut self, line: u16, column: u16) -> Option<Selection> {
        match *self {
            Self::Idle => None,
            Self::Pressed { at, area } => {
                if (line, column) == at {
                    return None;
                }
                let sel = Selection {
                    area,
                    anchor: at,
                    head: area.clamp(line, column),
                };
                *self = Self::Sweeping(sel);
                Some(sel)
            }
            Self::Sweeping(mut sel) => {
                sel.head = sel.area.clamp(line, column);
                *self = Self::Sweeping(sel);
                Some(sel)
            }
        }
    }

    /// The cell of the surface under a pointer at `(x, y)`, `(line, cell)`.
    #[must_use]
    pub fn cell_of(x: f64, y: f64) -> Option<(u16, u16)> {
        let size = ronoctua_tokens::text::size();
        let cell = f64::from(ronoctua_tokens::metrics::cell_width(size));
        let line = f64::from(ronoctua_tokens::metrics::line_height(size));
        if cell <= 0.0 || line <= 0.0 || x < 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Some(((y / line) as u16, (x / cell) as u16))
    }

    /// Is a sweep under way (and so the pointer's events are its own)?
    #[must_use]
    pub const fn sweeping(&self) -> bool {
        matches!(self, Self::Sweeping(_))
    }
}

/// The highlight of a selection, as rectangles in pixels.
///
/// `cell` and `line_px` are the grid and `y_of` where a line begins, in the
/// surface's own pixels: the runs' columns are the surface's cells already.
/// Over the letters and translucent (`surface.text-selection`), so what is
/// chosen stays readable.
#[must_use]
pub fn highlight(
    sel: &Selection,
    cell: f32,
    line_px: f32,
    y_of: impl Fn(u16) -> f32,
) -> Vec<ronoctua_render::quad::Quad> {
    sel.stretches()
        .into_iter()
        .map(|s| ronoctua_render::quad::Quad {
            x: f32::from(s.from) * cell,
            y: y_of(s.line),
            width: f32::from(s.to - s.from) * cell,
            height: line_px,
            color: ronoctua_tokens::color::SURFACE_TEXT_SELECTION,
        })
        .collect()
}

/// The painted runs of a list of spans, in cells.
///
/// Only the runs laid on the GRID: a run placed in pixels (a small
/// annotation, a centred label) has no cell to be swept from.
#[must_use]
pub fn painted_of(spans: &[ronoctua_render::Span]) -> Vec<Painted> {
    spans
        .iter()
        .filter(|s| matches!(s.size, ronoctua_render::Size::Grid) && !s.edge)
        .map(|s| Painted {
            line: s.line,
            column: s.column,
            text: s.text.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(line: u16, column: u16, text: &str) -> Painted {
        Painted {
            line,
            column,
            text: text.to_owned(),
        }
    }

    const WIDE: Area = Area {
        left: 0,
        right: 80,
        top: 0,
        bottom: 20,
    };

    #[test]
    fn a_press_that_does_not_move_is_a_click() {
        let mut s = Selecting::Pressed {
            at: (2, 5),
            area: WIDE,
        };
        assert_eq!(s.moved_to(2, 5), None);
        assert!(!s.sweeping());
        assert!(s.moved_to(2, 6).is_some());
        assert!(s.sweeping());
    }

    #[test]
    fn a_sweep_reads_as_text_is_read() {
        let painted = [
            run(1, 2, "the first line"),
            run(2, 2, "the second"),
            run(2, 20, "beside it"),
            run(3, 2, "and the last"),
        ];
        let sel = Selection {
            area: WIDE,
            anchor: (1, 6),
            head: (3, 4),
        };
        assert_eq!(
            sel.extract(&painted),
            "first line\nthe second        beside it\nand"
        );
        // BACKWARDS IS THE SAME SWEEP.
        let back = Selection {
            anchor: (3, 4),
            head: (1, 6),
            ..sel
        };
        assert_eq!(back.extract(&painted), sel.extract(&painted));
    }

    #[test]
    fn a_sweep_never_leaves_the_half_it_started_in() {
        // TWO COLUMNS SIDE BY SIDE are two texts: the message on the left
        // and its facts on the right.
        let painted = [
            run(1, 2, "the message goes on"),
            run(1, 40, "Arrived"),
            run(2, 2, "and on"),
            run(2, 40, "Filter"),
        ];
        let half = Area { right: 38, ..WIDE };
        let mut s = Selecting::Pressed {
            at: (1, 2),
            area: half,
        };
        let sel = s.moved_to(2, 60).unwrap();
        assert_eq!(sel.head, (2, 37), "the pointer ran into the other half");
        let text = sel.extract(&painted);
        assert!(
            !text.contains("Arrived") && !text.contains("Filter"),
            "{text}"
        );
        assert_eq!(text, "the message goes on\nand on");
    }

    #[test]
    fn a_wide_character_is_taken_whole() {
        // `日本語` is three clusters and SIX cells: a cut by cluster count
        // would take the wrong half.
        let painted = [run(0, 0, "ab日本語cd")];
        let sel = Selection {
            area: WIDE,
            anchor: (0, 2),
            head: (0, 5),
        };
        assert_eq!(sel.extract(&painted), "日本");
    }

    #[test]
    fn the_highlight_covers_the_stretches() {
        let sel = Selection {
            area: Area {
                left: 2,
                right: 10,
                ..WIDE
            },
            anchor: (1, 4),
            head: (2, 3),
        };
        let q = highlight(&sel, 10.0, 20.0, |l| f32::from(l) * 20.0);
        assert_eq!(q.len(), 2);
        assert_eq!(
            (q[0].x, q[0].width),
            (40.0, 60.0),
            "to the area's right edge"
        );
        assert_eq!(
            (q[1].x, q[1].width),
            (20.0, 20.0),
            "from its left edge, the last cell in"
        );
    }
}
