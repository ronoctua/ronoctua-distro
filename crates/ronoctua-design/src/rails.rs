//! The two rails, and the conversion between them.
//!
//! # Why this is behind `host`
//!
//! Because one of the rails is PIXELS, and a buffer has none. A component
//! counts in cells and lines and hands back a display list; who turns that
//! into a surface is the host (invariants 4 and 11). Compiling this into a
//! component would put a unit there that nothing on that side can mean.
//!
//! It is still here rather than in the shell because the arithmetic is the
//! design system's, not one door's: the same conversion answers for the
//! footer, for each window, and for anything drawn later. It had four
//! spellings in the shell when it was counted.
//!
//! # The residue, which is the whole point
//!
//! A surface is `floor(width / cell)` whole cells and `floor(height / line)`
//! whole lines, PLUS what is left over — less than one of each. The two grids
//! coincide only when the surface divides exactly, and it never does: a window
//! is resized by dragging.
//!
//! Every geometry defect this project has been asked to fix lived in that
//! residue: a rule that stopped short of the edge, a scrollbar with a strip of
//! background under its thumb, a picture centred in a box a line shorter than
//! the panel it was in. So the conversion happens HERE, once, and the leftover
//! has an owner instead of being a convention each site has to remember.

/// A surface, and the grid over it.
///
/// `cell` and `line` are given rather than looked up: this crate has no tokens
/// (invariant 11 forbids a buffer receiving their values, and a `no_std` crate
/// that reached for them would have to depend on them for both sides). The
/// host brings the two numbers; the arithmetic is the same either way.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// How wide a cell is, in pixels.
    pub cell: f32,
    /// How tall a line is, in pixels.
    pub line: f32,
    /// The surface's width, in pixels. Never a count of anything.
    pub width: f32,
    /// The surface's height, in pixels. This is the number the compositor gave
    /// and the shell may not argue with it (invariant 4).
    pub height: f32,
    /// The region's first line, in the grid.
    pub first: u16,
}

impl Frame {
    /// How much of a cell a point may be short of a boundary and still count
    /// as being on it.
    ///
    /// # Why an epsilon is not sloppiness here
    ///
    /// Because [`Frame::x_of`] and [`Frame::column_at`] have to be each other's
    /// inverse, and in `f32` they are not: `7 * 10.8` comes back as
    /// `75.599998`, and dividing that by `10.8` gives `6.9999998`, which floors
    /// to **six**. A press on the first pixel of a cell resolved to the cell
    /// before it — the exact shape of "the click landed on one thing while the
    /// screen showed another".
    ///
    /// A ten-thousandth of a cell is a thousand times smaller than any pointer
    /// can resolve and a thousand times larger than the error.
    pub const SNAP: f32 = 1e-4;

    /// A frame over a surface.
    #[must_use]
    pub const fn of(cell: f32, line: f32, width: f32, height: f32, first: u16) -> Self {
        Self {
            cell,
            line,
            width,
            height,
            first,
        }
    }

    /// How many whole cells the surface holds.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn columns(self) -> u16 {
        if self.cell <= 0.0 {
            return 0;
        }
        // `as` on a non-negative float IS floor, and it saturates rather than
        // wrapping — which is what a width in pixels needs.
        (self.width / self.cell) as u16
    }

    /// How many whole lines the surface holds.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn lines(self) -> u16 {
        if self.line <= 0.0 {
            return 0;
        }
        (self.height / self.line) as u16
    }

    /// The pixel a column begins at, counted from the LEFT.
    #[must_use]
    pub fn x_of(self, column: u16) -> f32 {
        f32::from(column) * self.cell
    }

    /// The pixel a line begins at.
    #[must_use]
    pub fn y_of(self, line: u16) -> f32 {
        f32::from(line) * self.line
    }

    /// The surface's bottom edge, in pixels. The TRUE one.
    #[must_use]
    pub const fn bottom(self) -> f32 {
        self.height
    }

    /// Which column a pixel is in.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn column_at(self, x: f32) -> u16 {
        if self.cell <= 0.0 || x < 0.0 {
            return 0;
        }
        (x / self.cell + Self::SNAP) as u16
    }

    /// Which line a pixel is on, or `None` above the surface.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn line_at(self, y: f32) -> Option<u16> {
        if self.line <= 0.0 || y < 0.0 {
            return None;
        }
        Some((y / self.line + Self::SNAP) as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::Frame;

    /// THE LAST LINE REACHES THE SURFACE — the law the plan names, checked at
    /// every height and both line heights rather than at the one that was
    /// convenient.
    #[test]
    fn the_last_line_reaches_the_surface() {
        // The two the shell uses: the footer's 18 px and a window's 20 px,
        // through the 1.45 the tokens set.
        for line in [26.1f32, 29.0] {
            for h in 200..=1200 {
                #[allow(clippy::cast_precision_loss)]
                let height = h as f32;
                let f = Frame::of(10.8, line, 900.0, height, 1);
                let last = f.y_of(f.lines());
                assert!(
                    last <= f.bottom() + 0.001,
                    "line={line} h={height}: the grid's end {last} passed the surface"
                );
                assert!(
                    f.bottom() - last < line,
                    "line={line} h={height}: {} px are left under the last line",
                    f.bottom() - last
                );
            }
        }
    }

    /// A POINT LANDS ON THE CELL IT WAS DRAWN IN, which `f32` does not give
    /// for free — see [`Frame::SNAP`].
    #[test]
    fn a_point_lands_on_the_cell_it_was_drawn_in() {
        for cell in [10.8f32, 12.0, 7.5] {
            let f = Frame::of(cell, 26.0, 1920.0, 1080.0, 1);
            for column in 0..f.columns() {
                assert_eq!(
                    f.column_at(f.x_of(column)),
                    column,
                    "cell={cell}: column {column} came back as another"
                );
            }
            for line in 0..f.lines() {
                assert_eq!(
                    f.line_at(f.y_of(line)),
                    Some(line),
                    "cell={cell}: line {line} came back as another"
                );
            }
        }
    }

    /// AND A SURFACE OF NOTHING ANSWERS NOTHING, rather than dividing by zero.
    #[test]
    fn a_surface_with_no_grid_answers_nothing() {
        let f = Frame::of(0.0, 0.0, 900.0, 800.0, 0);
        assert_eq!(f.columns(), 0);
        assert_eq!(f.lines(), 0);
        assert_eq!(f.column_at(50.0), 0);
        assert_eq!(f.line_at(50.0), None);
    }
}
