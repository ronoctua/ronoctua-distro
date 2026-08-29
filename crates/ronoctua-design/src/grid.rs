//! A grid of items — the icon browsers' geometry, in one place.
//!
//! # What it replaces
//!
//! Three near-identical blocks (`grid_columns`, `height`, `panel_first_row`,
//! `rule_row`, `grid_first_row`, `footer_row`, `row_step`) in
//! `buffers/glyphs`, `buffers/lucide` and `buffers/unicode`, each with its own
//! constants and each with a bare literal `2` for the air on the right. The
//! constants that genuinely differ — how wide one drawing is — stay with the
//! caller; the arithmetic is here.
//!
//! # The `2` that is not here
//!
//! All three wrote `ctx.columns.saturating_sub(2) / step`. That was a second
//! right-hand margin: the host already takes `space.buffer.marginX` off BOTH
//! sides before it reports `columns`. What the three were really working around
//! is that the last item in a row needs its own width and not a whole step —
//! `(columns + GAP) / step` says exactly that, with no number to justify.

use crate::GAP;

/// How many items fit, and where each one goes.
///
/// The pair [`Grid::at`] and [`Grid::index_at`] is the same arithmetic in both
/// directions on purpose: the drawing and the click have already diverged once
/// in these buffers, and the symptom was the click landing on one item while
/// the screen showed another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    /// How many items fit side by side. At least one.
    pub across: u16,
    /// How many cells one item takes across, its air included.
    pub step: u16,
    /// How many cells one item takes down.
    ///
    /// It comes from the HOST: how many cells a scale is worth depends on the
    /// font size and the line height, which a buffer does not have (invariant
    /// 11). It is also the whole of the vertical spacing — the gap between one
    /// row and the next is what is left of the line box (`1.45 em` of box
    /// against `0.60 em` of ink), and it is already wider than the horizontal
    /// gap. The empty cell that used to be added here tripled the vertical
    /// distance and took a third of the icons off the screen.
    pub down: u16,
    /// The first row.
    pub first: u16,
    /// How many rows fit. At least one.
    pub rows: u16,
}

impl Grid {
    /// The grid that fits between `first` and `until`, for items `item` cells
    /// wide and `down` cells tall.
    ///
    /// # The arithmetic, so there is no loose number
    ///
    /// A row of `n` items ends at `(n − 1) × step + item`, where
    /// `step = item + GAP` — the air goes BETWEEN items and never after the
    /// last one. Solving `(n − 1) × step + item ≤ columns` gives
    /// `n = ⌊(columns + GAP) / step⌋`, which is what is written below. At that
    /// `n` the last item ends at `n × step − GAP ≤ columns` exactly.
    ///
    /// # The two `max(1)`
    ///
    /// Not fussiness: in a window of three lines the row arithmetic gives zero,
    /// and a grid of no rows shows nothing. One item in a tiny window is ugly;
    /// none looks broken.
    #[must_use]
    pub const fn fitted(columns: u16, item: u16, first: u16, until: u16, down: u16) -> Self {
        let step = if item == 0 { GAP } else { item + GAP };
        let across = columns.saturating_add(GAP) / step;
        let down = if down == 0 { 1 } else { down };
        let rows = until.saturating_sub(first) / down;
        Self {
            across: if across == 0 { 1 } else { across },
            step,
            down,
            first,
            rows: if rows == 0 { 1 } else { rows },
        }
    }

    /// How many items are on the screen at once.
    #[must_use]
    pub const fn fit(&self) -> usize {
        self.across as usize * self.rows as usize
    }

    /// Where the `index`-th item of the page goes: `(row, first cell)`.
    ///
    /// `None` past the last one that fits, so a caller walks the page without
    /// counting it out first.
    #[must_use]
    pub fn at(&self, index: usize) -> Option<(u16, u16)> {
        if index >= self.fit() {
            return None;
        }
        let across = usize::from(self.across);
        let row = u16::try_from(index / across).ok()?;
        let column = u16::try_from(index % across).ok()?;
        Some((
            self.first.saturating_add(row.saturating_mul(self.down)),
            column.saturating_mul(self.step),
        ))
    }

    /// Which item a cell belongs to. The other direction of [`Self::at`].
    ///
    /// The whole step belongs to the item, air included: a target with slack
    /// beside it is a target one does not miss, and the air between two icons
    /// has no other owner.
    #[must_use]
    pub fn index_at(&self, line: u16, column: u16) -> Option<usize> {
        let row = line.checked_sub(self.first)? / self.down;
        if row >= self.rows {
            return None;
        }
        let at = column / self.step;
        if at >= self.across {
            return None;
        }
        Some(usize::from(row) * usize::from(self.across) + usize::from(at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NOTHING LEAKS: the last item of a row ends inside the window, at every
    /// width and every item size the browsers use (2 for a glyph, 4 for a
    /// Lucide drawing, 3 for a marker plus a character).
    #[test]
    fn the_last_item_of_a_row_ends_inside_the_window() {
        for columns in 1..=240u16 {
            for item in [2u16, 3, 4] {
                let g = Grid::fitted(columns, item, 0, 40, 2);
                let ends = (g.across - 1) * g.step + item;
                assert!(ends <= columns || g.across == 1, "{columns}/{item}: {ends}");
            }
        }
    }

    /// One more item would NOT have fitted — the grid is as full as it can be.
    #[test]
    fn one_more_item_would_not_have_fitted() {
        for columns in 8..=240u16 {
            for item in [2u16, 3, 4] {
                let g = Grid::fitted(columns, item, 0, 40, 2);
                assert!(g.across * g.step + item > columns, "{columns}/{item}");
            }
        }
    }

    /// THE DRAWING AND THE CLICK ARE THE SAME ARITHMETIC. Every item the grid
    /// places is found again at the cell it was placed on.
    #[test]
    fn where_it_draws_is_where_it_is_found() {
        let g = Grid::fitted(80, 2, 3, 30, 2);
        for i in 0..g.fit() {
            let (line, column) = g.at(i).expect("inside the page");
            assert_eq!(g.index_at(line, column), Some(i), "item {i}");
            // And the air beside it belongs to it too.
            assert_eq!(g.index_at(line, column + g.step - 1), Some(i), "air of {i}");
        }
        assert_eq!(g.at(g.fit()), None);
    }

    /// Above the first row there is no item, and below the last there is none
    /// either.
    #[test]
    fn outside_the_page_there_is_nothing() {
        let g = Grid::fitted(80, 2, 3, 30, 2);
        assert_eq!(g.index_at(2, 0), None);
        assert_eq!(g.index_at(3 + g.rows * g.down, 0), None);
        assert_eq!(g.index_at(3, g.across * g.step), None);
    }
}
