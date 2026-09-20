//! The part of the window a block of the screen OWNS.
//!
//! # Why this exists
//!
//! Because it did not, and the owner photographed what that costs: a footer
//! painted over the last rows of a history list, and a strip of the page's own
//! ground between that footer and the bottom edge. Neither was a slip in the
//! arithmetic — there was no arithmetic to slip. A buffer addressed the window
//! by bare `u16`, `Screen::go_to` had no bound, the lowering was a one-to-one
//! map over the parts, and the host clipped nothing: a line past the bottom was
//! laid out, submitted, and dropped by the rasteriser without a word in any
//! log.
//!
//! So the blocks of a screen were an idea held in the author's head and
//! threaded through six functions as two loose numbers called `from` and `to`.
//! Every defect the owner reported was one of those numbers reaching into a
//! neighbour.
//!
//! A region is that idea made a value. It says where a block starts and how
//! tall it is, it converts between its OWN rows and the window's lines in both
//! directions, and — through [`crate::screen::Pen`] — it refuses a draw that
//! would land outside it. Refuses, and counts: clamping would hide the defect,
//! which is exactly what the host was doing by accident.
//!
//! # It is not a sixth thing on the wire
//!
//! Nothing here crosses. A region is a guest-side grammar for deciding WHERE a
//! part goes, and the parts that come out of it are the same seven primitives
//! that always crossed. The proof that the blocks tile the window is
//! [`crate::screen::Screen::check`]'s, on this side, in the buffer's own unit
//! tests — which is where a drawing mistake can still be cheap.

use alloc::vec::Vec;

/// A block of the window, in the window's own lines.
///
/// `top` is the first line it owns and `lines` is how many — so the last line
/// it owns is `top + lines - 1`, and [`Self::bottom`] is one past it, the way
/// every half-open range in this crate reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Region {
    /// The first line of the window this region owns.
    pub top: u16,
    /// How many lines it owns. Zero is a region that is not drawn at all —
    /// the warning line when nothing is wrong, the activity log on a window
    /// too short for one.
    pub lines: u16,
}

impl Region {
    /// A region of `lines` lines starting at `top`.
    #[must_use]
    pub const fn new(top: u16, lines: u16) -> Self {
        Self { top, lines }
    }

    /// One line past the last it owns.
    #[must_use]
    pub const fn bottom(&self) -> u16 {
        self.top.saturating_add(self.lines)
    }

    /// Whether this line of the WINDOW belongs to this region.
    #[must_use]
    pub const fn holds(&self, line: u16) -> bool {
        line >= self.top && line < self.bottom()
    }

    /// Whether it owns no lines at all.
    #[must_use]
    pub const fn empty(&self) -> bool {
        self.lines == 0
    }

    /// Which line of the WINDOW carries row `row` of this region.
    ///
    /// `None` past its last row. The pair with [`Self::row_at`] is the point:
    /// drawing asks this one and the pointer asks the other, and a screen whose
    /// two answers disagree is a click that lands on a different thing from the
    /// one under the hand.
    #[must_use]
    pub const fn line_of(&self, row: u16) -> Option<u16> {
        if row >= self.lines {
            return None;
        }
        Some(self.top.saturating_add(row))
    }

    /// Which row of this region a line of the WINDOW is, or `None` when the
    /// line belongs to somebody else.
    #[must_use]
    pub const fn row_at(&self, line: u16) -> Option<u16> {
        if !self.holds(line) {
            return None;
        }
        Some(line - self.top)
    }

    /// Whether two regions share a line.
    #[must_use]
    pub const fn meets(&self, other: &Self) -> bool {
        if self.empty() || other.empty() {
            return false;
        }
        self.top < other.bottom() && other.top < self.bottom()
    }

    /// The scrollbar arithmetic for a list drawn in this region.
    ///
    /// The same [`crate::Scroll`] every other bar in the system is built from —
    /// a region hands it out rather than growing a second one.
    #[must_use]
    pub const fn scroll(&self, total: usize, offset: usize) -> crate::Scroll {
        crate::Scroll {
            total,
            lines: self.lines as usize,
            offset,
            top: 0,
        }
    }
}

/// What a caller asks for when it divides the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slice {
    /// Exactly this many lines.
    Fixed(u16),
    /// Whatever is left over.
    ///
    /// Exactly one slice should be this. It is what stops a window's last line
    /// from belonging to nobody: integer division leaves a remainder, and a
    /// remainder nobody owns is the blank strip the file manager's `Panes::boxes`
    /// already had to name once.
    Rest,
}

/// Divide a window of `lines` into regions that TILE it.
///
/// The guarantee, and it is the whole reason this function exists rather than
/// five subtractions at the call site:
///
/// * the regions come back in the order they were asked for;
/// * no two of them share a line;
/// * together they cover `0..lines` exactly, with nothing left over.
///
/// # When the asks do not fit
///
/// The fixed slices are honoured in order while there is room, `Rest` takes
/// what remains where it stands, and anything after that gets zero lines. A
/// region of zero lines draws nothing — [`crate::screen::Pen`] refuses every
/// row of it — so a window too short comes out with its last blocks missing
/// rather than with two blocks on one line. That is a deliberate order of
/// preference and not a fallback: a caller that wants its footer kept puts the
/// footer's slice before the ones it would rather lose.
#[must_use]
pub fn split(lines: u16, spec: &[Slice]) -> Vec<Region> {
    let fixed: u16 = spec
        .iter()
        .map(|s| match s {
            Slice::Fixed(n) => *n,
            Slice::Rest => 0,
        })
        .fold(0, u16::saturating_add);
    let mut rest = lines.saturating_sub(fixed);
    let mut top = 0;
    let mut out = Vec::with_capacity(spec.len());
    for slice in spec {
        let want = match slice {
            Slice::Fixed(n) => *n,
            // ONE `Rest` TAKES IT ALL. A second would find nothing left, which
            // is the honest answer to asking for the remainder twice.
            Slice::Rest => core::mem::take(&mut rest),
        };
        let got = want.min(lines.saturating_sub(top));
        out.push(Region::new(top, got));
        top = top.saturating_add(got);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn a_split_tiles_the_window_exactly() {
        for lines in 1..60u16 {
            let regions = split(lines, &[Slice::Fixed(1), Slice::Rest, Slice::Fixed(3)]);
            let covered: u16 = regions.iter().map(|r| r.lines).fold(0, u16::saturating_add);
            assert_eq!(covered, lines, "at {lines} lines the split lost a line");
            for (n, a) in regions.iter().enumerate() {
                for b in regions.iter().skip(n + 1) {
                    assert!(!a.meets(b), "at {lines} lines {a:?} and {b:?} share a line");
                }
            }
            // AND EVERY LINE HAS AN OWNER, which is the half that the sum
            // alone does not prove: three regions of the right total could
            // still leave a hole and overlap somewhere else.
            for line in 0..lines {
                assert_eq!(
                    regions.iter().filter(|r| r.holds(line)).count(),
                    1,
                    "at {lines} lines, line {line} has no single owner"
                );
            }
        }
    }

    #[test]
    fn a_window_too_short_loses_the_last_blocks_and_never_overlaps() {
        let regions = split(2, &[Slice::Fixed(1), Slice::Fixed(1), Slice::Fixed(4)]);
        assert_eq!(regions[0], Region::new(0, 1));
        assert_eq!(regions[1], Region::new(1, 1));
        assert_eq!(
            regions[2],
            Region::new(2, 0),
            "the block that did not fit drew"
        );
        assert!(regions[2].empty());
    }

    #[test]
    fn the_two_conversions_are_each_other() {
        let r = Region::new(7, 5);
        for row in 0..5 {
            let line = r.line_of(row).expect("the row is in the region");
            assert_eq!(
                r.row_at(line),
                Some(row),
                "row {row} went and did not come back"
            );
        }
        assert_eq!(r.line_of(5), None, "a row past the region answered");
        assert_eq!(r.row_at(6), None, "the line above the region answered");
        assert_eq!(r.row_at(12), None, "the line below the region answered");
    }

    #[test]
    fn an_empty_region_holds_nothing_and_meets_nothing() {
        let empty = Region::new(4, 0);
        assert!(!empty.holds(4));
        assert_eq!(empty.line_of(0), None);
        assert!(!empty.meets(&Region::new(0, 10)));
        assert!(!Region::new(0, 10).meets(&empty));
    }

    #[test]
    fn a_split_with_no_rest_still_tiles_what_it_was_given() {
        let regions = split(10, &[Slice::Fixed(3), Slice::Fixed(3)]);
        assert_eq!(regions, vec![Region::new(0, 3), Region::new(3, 3)]);
    }
}
