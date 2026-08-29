//! How much room a run may take, and how much one takes.
//!
//! # There is no elision here, and that is the point
//!
//! A buffer does not cut text. It cannot cut a label — the sentence is the
//! host's, and its length changes with the language (invariant 8); and cutting
//! its own data by codepoint invents drawings, which is why the host elides by
//! CLUSTER and does it in one place (`render::layout::elide_end`). What a
//! buffer knows is how much SPACE there is, and that is exactly what
//! `max-width` carries across the WIT: the buffer declares the budget, the host
//! holds the string and does the cutting.
//!
//! So what is here is arithmetic over cells: how much room is left, what a run
//! may claim, and what one does claim. Every one of them used to be written
//! out at the call site, seven times, as `ctx.columns.saturating_sub(3)`.

use ronoctua_protocol::{Heights, Scale};

use crate::screen::{Ink, Run};

/// How many cells there are from `column` to the right edge.
///
/// Zero when the column is already past it — `saturating_sub`, because a
/// negative width is a width the caller would then have to check.
#[must_use]
pub const fn room(columns: u16, column: u16) -> u16 {
    columns.saturating_sub(column)
}

/// The budget a run at `column` may declare, given what it asked for.
///
/// `0` in and `0` out: the protocol's own spelling for "to the edge", and the
/// host applies the edge anyway. A number in comes out capped by what is
/// actually left, so a buffer that asked for forty cells in a window of
/// thirty gets thirty and not a run the compositor refuses to clip.
#[must_use]
pub const fn budget(columns: u16, column: u16, wanted: u16) -> u16 {
    if wanted == 0 {
        return 0;
    }
    let left = room(columns, column);
    if wanted < left { wanted } else { left }
}

/// How many cells of HEIGHT a run of this scale occupies.
///
/// `max(1)`: a host that reported zero would put every row of the frame on one
/// line, and a run always occupies at least the cell it names. The same
/// `max(1)` `DisplayList::rows_of` applies, and for the same reason.
#[must_use]
pub const fn tall(heights: &Heights, scale: Scale) -> u16 {
    let n = heights.of(scale);
    if n == 0 { 1 } else { n }
}

/// How many cells WIDE a piece of literal data is, at the body's size.
///
/// Counted in `char`s, which is what `DisplayList::columns_of` charges — and
/// the two have to agree exactly. A buffer that positioned the next field from
/// a different count would be laying out against a number the host does not
/// use, which is a class of defect that only shows on somebody else's screen.
///
/// (This is a count, not a cut. The cluster rule is about cutting, and cutting
/// is the host's — see this module's header.)
#[must_use]
pub fn wide(text: &str) -> u16 {
    u16::try_from(text.chars().count()).unwrap_or(u16::MAX)
}

/// How many cells a run actually claims, budget and scale included.
///
/// The same arithmetic `DisplayList::columns_of` runs on the other side, and
/// deliberately so: it is what lets a buffer put the next field immediately
/// after this one without guessing.
///
/// - a **label** claims its whole budget, because nothing is known about it
///   until the host looks the key up;
/// - **data** claims its own length, widened by the scale — the cell is
///   `0.600 em`, so a run at twice the body's size has twice the advance — and
///   then capped by the budget;
/// - an **icon** claims what its scale is worth in cells, capped the same way.
#[must_use]
pub fn claimed<I>(run: &Run<I>, heights: &Heights, columns: u16) -> u16 {
    let ceiling = if run.budget == 0 {
        room(columns, run.column)
    } else {
        run.budget
    };
    let step = tall(heights, run.scale);
    match &run.ink {
        Ink::Label(_) => ceiling,
        Ink::Datum(t) => wide(t).saturating_mul(step).min(ceiling),
        Ink::Glyph(_) => step.min(ceiling),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heights() -> Heights {
        Heights::from_sizes(12.0, 14.0, 20.0, 28.0, 40.0, 1.45)
    }

    #[test]
    fn a_budget_never_reaches_past_the_edge() {
        for columns in 0..=64u16 {
            for column in 0..=64u16 {
                for wanted in [0u16, 1, 12, 40, 400] {
                    let b = budget(columns, column, wanted);
                    assert!(
                        b == 0 || column.saturating_add(b) <= columns,
                        "{columns}/{column}/{wanted} gave {b}"
                    );
                }
            }
        }
    }

    /// `claimed` has to answer what the host charges, or a buffer positions
    /// against a number nobody else uses.
    #[test]
    fn a_label_claims_its_whole_budget_and_data_claims_its_length() {
        let h = heights();
        let label: Run<&'static str> = Run::label("cmd-mx-open").at(0).within(30);
        assert_eq!(claimed(&label, &h, 80), 30);

        let datum: Run<&'static str> = Run::datum("sda1").at(0).within(30);
        assert_eq!(claimed(&datum, &h, 80), 4);

        // The scale widens as much as it raises: `Large` is two cells tall and
        // each character is two cells wide.
        let big: Run<&'static str> = Run::datum("sda1").at(0).within(30).large();
        assert_eq!(claimed(&big, &h, 80), 4 * tall(&h, Scale::Large));

        // And the budget still wins.
        let cut: Run<&'static str> = Run::datum("sda1").at(0).within(3).large();
        assert_eq!(claimed(&cut, &h, 80), 3);
    }
}
