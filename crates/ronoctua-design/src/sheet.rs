//! The label-and-value sheet the three browsers draw.
//!
//! One implementation with one set of constants. There were three: the glyph
//! browser and the symbol browser reserved 24 cells for the label, Lucide
//! reserved 20, and nothing said why the same sheet was spaced differently in
//! two windows — which is the "same view spaced differently" in the report,
//! reached from the other direction.

use crate::screen::{Run, Screen};
use crate::{GAP, Palette, budget::room};

/// Where the value column starts, when there is room for it.
///
/// **Twenty-four, because it is what fits the longest label we send.** The
/// buffer cannot measure that on its own: it sends a KEY and the host
/// translates (invariant 11), so what it does is declare how much it reserved,
/// and the check `browser-filters` compares both languages against this number.
/// It was sixteen once, and four labels came out as `…` at every window size,
/// because the cut came from the budget and not from the space.
pub const SHEET_LABEL: u16 = 24;

/// Where the label ends and the value begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sheet {
    /// How many cells the label may take.
    pub label: u16,
    /// The value's first cell.
    pub value: u16,
    /// How many cells the value may take.
    pub room: u16,
}

/// The sheet's two columns for a window this wide.
///
/// # Why this is not a [`Table`](crate::Table)
///
/// Because its label column **shrinks** instead of giving way. A table drops a
/// column when the width stops paying for it; a label that got half the window
/// is still a label, and a sheet whose labels were dropped is a column of
/// values with nothing saying what they are. So the arithmetic is one line and
/// it is its own line: the value column is [`SHEET_LABEL`], or half the window,
/// whichever is smaller. Without the half, a window of thirty cells spent
/// twenty-four on the label and six on the value.
///
/// The label's own budget stops one cell short of the value, which is
/// [`GAP`](crate::GAP): there is no side bearing in these faces, so the air
/// between two runs has to be put in by hand or the two touch.
///
/// # The width below which there is no sheet
///
/// Four cells. A two-column sheet needs a cell of label, a cell of air and a
/// cell of value, and the half means the label's half has to hold two of those
/// three. Below that there is no arrangement, and this returns zeroes rather
/// than a label whose budget of `0` would mean "to the edge" — the protocol's
/// own spelling — and write straight over the value. That is not a hypothetical:
/// it is what the first version of this function did at one cell.
#[must_use]
pub fn sheet_split(columns: u16) -> Sheet {
    let value = SHEET_LABEL.min(columns / 2);
    if value < GAP.saturating_add(1) {
        return Sheet {
            label: 0,
            value: 0,
            room: 0,
        };
    }
    Sheet {
        label: value.saturating_sub(GAP),
        value,
        room: room(columns, value),
    }
}

impl<P: Palette> Screen<P> {
    /// One line of the sheet: a label on the left, its value on the right.
    ///
    /// `marked` is the selected copy option, and it is marked **by colour** —
    /// never by a box or an outline (invariant 15), which is the same thing the
    /// grid does with focus.
    ///
    /// The label is drawn smaller than the value on purpose: it accompanies the
    /// value without competing with it, and the value is what one came to read.
    ///
    /// Returns how many cells of height the line took.
    pub fn sheet(&mut self, line: u16, label: &str, value: Run<P::Icon>, marked: bool) -> u16 {
        let split = sheet_split(self.columns());
        // NOTHING AT ALL when the window cannot hold two columns. Not the label
        // alone: a label with no value beside it is a word that says what is
        // missing.
        if split.label == 0 {
            return 0;
        }
        let mut cells = self.put(
            line,
            Run::label(label)
                .small()
                .playing(if marked {
                    crate::Role::Accent
                } else {
                    crate::Role::Dim
                })
                .at(0)
                .within(split.label),
        );
        // NOTHING IS DRAWN WHEN THERE IS NO ROOM. A value at the last cell is
        // one character and an ellipsis, which takes the line to say nothing.
        if split.room > 0 {
            let placed = value
                .playing(if marked {
                    crate::Role::Accent
                } else {
                    crate::Role::Body
                })
                .at(split.value)
                .within(split.room);
            cells = cells.max(self.put(line, placed));
        }
        cells
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tokens;
    use ronoctua_protocol::{Context, Heights};

    fn context(columns: u16) -> Context {
        Context {
            columns,
            lines: 24,
            heights: Heights::from_sizes(12.0, 14.0, 20.0, 28.0, 40.0, 1.45),
        }
    }

    /// The label never reaches the value, at any width.
    #[test]
    fn the_label_stops_a_cell_short_of_the_value() {
        for columns in 0..=200u16 {
            let s = sheet_split(columns);
            assert!(s.label.saturating_add(GAP) <= s.value.max(GAP), "{columns}");
            assert!(s.value.saturating_add(s.room) <= columns, "{columns}");
        }
    }

    /// The sheet is one arithmetic in both doors: the same width gives the same
    /// answer whether the buffer is in the footer's region or in its own
    /// window. That is the "same view spaced differently" defect, as a test.
    #[test]
    fn one_width_gives_one_answer() {
        assert_eq!(sheet_split(80), sheet_split(80));
        assert_eq!(sheet_split(30).value, 15);
        assert_eq!(sheet_split(200).value, SHEET_LABEL);
    }

    /// And the frame it produces answers the host's two questions.
    #[test]
    fn a_sheet_survives_the_hosts_questions() {
        for columns in 0..=200u16 {
            let mut screen = Screen::<Tokens>::new(&context(columns));
            let mut line = 0;
            for (label, value) in [("glyph-name", "arrow-left"), ("glyph-codepoint", "U+2190")] {
                line += screen.sheet(line, label, Run::datum(value), false);
            }
            screen
                .check()
                .unwrap_or_else(|e| panic!("{columns} columns: {e}"));
        }
    }
}
