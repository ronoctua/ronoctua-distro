//! `TAB` FROM SECTION TO SECTION — the first row of the next section's, and
//! `Shift+TAB` the previous one's, in a ring.
//!
//! The network's view walked its sections so (`LEADER s n`), and the owner
//! asked for it in every window and view where `TAB` means nothing else
//! (2026-11-09) — and in every one written after. So it is answered ONCE, for
//! every list the door or the footer draws, from what both already have: the
//! rows, which ones a click takes and where the cursor is. A section begins at
//! a TITLE — a row in the title ink (`text.heading`) that takes no click —
//! and the cursor goes to the first row of it a click takes: what the first
//! click there would do, which is only to move the cursor (invariant 19).
//!
//! A list with fewer than two titles has no sections to walk, and `TAB` is
//! the window's own then, as it always was.

use ronoctua_tokens as tokens;

/// The row `TAB` lands on — or `Shift+TAB`, `back` — from the cursor at
/// `cursor`, in rows `rows` of which `takes` says which a click takes; `None`
/// where there are no sections to walk, or no row in any of them to stand on.
#[must_use]
pub fn landing(
    rows: &[Vec<crate::fields::Field>],
    takes: &[bool],
    cursor: Option<usize>,
    back: bool,
) -> Option<usize> {
    let takes_at = |i: usize| takes.get(i).copied().unwrap_or(false);
    let titles: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(i, fields)| {
            !takes_at(*i)
                && fields
                    .iter()
                    .any(|f| f.color == tokens::color::TEXT_HEADING)
        })
        .map(|(i, _)| i)
        .collect();
    if titles.len() < 2 {
        return None;
    }
    // THE FIRST ROW A CLICK TAKES under each title, before the next one.
    let firsts: Vec<Option<usize>> = titles
        .iter()
        .enumerate()
        .map(|(n, &title)| {
            let end = titles.get(n + 1).copied().unwrap_or(rows.len());
            (title + 1..end).find(|&i| takes_at(i))
        })
        .collect();
    let count = titles.len();
    // THE SECTION THE CURSOR IS IN: the last title above it — none above it
    // is before the first, and the ring starts at the end it walks towards.
    let here = cursor.and_then(|c| titles.iter().rposition(|&t| t <= c));
    let mut at = match (here, back) {
        (Some(h), _) => h,
        (None, false) => count - 1,
        (None, true) => 0,
    };
    // A SECTION WITH NOTHING TO STAND ON is passed over, once round the ring.
    for _ in 0..count {
        at = if back {
            (at + count - 1) % count
        } else {
            (at + 1) % count
        };
        if let Some(row) = firsts[at] {
            return Some(row);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(color: ronoctua_tokens::Color) -> Vec<crate::fields::Field> {
        vec![crate::fields::Field {
            at: 0,
            text: "x".to_owned(),
            color,
            hit: crate::fields::Hit::Nothing,
            explain: None,
            ink: crate::fields::Ink::Grid,
        }]
    }

    /// FAILS IF: `TAB` does not walk the sections' first rows in a ring, both
    /// ways, passing over one with nothing to stand on — or walks a list with
    /// one title, or lands on a title.
    #[test]
    fn tab_walks_the_first_rows_of_the_sections_in_a_ring() {
        let h = tokens::color::TEXT_HEADING;
        let r = tokens::color::TEXT_PRIMARY;
        // 0 title, 1 2 rows; 3 air; 4 title, 5 a note; 6 air; 7 title, 8 9 rows.
        let rows = vec![
            row(h),
            row(r),
            row(r),
            Vec::new(),
            row(h),
            row(r),
            Vec::new(),
            row(h),
            row(r),
            row(r),
        ];
        let takes = [
            false, true, true, false, false, false, false, false, true, true,
        ];
        assert_eq!(
            landing(&rows, &takes, Some(1), false),
            Some(8),
            "the empty section was not passed over"
        );
        assert_eq!(
            landing(&rows, &takes, Some(9), false),
            Some(1),
            "the ring does not close"
        );
        assert_eq!(landing(&rows, &takes, Some(8), true), Some(1));
        assert_eq!(
            landing(&rows, &takes, Some(2), true),
            Some(8),
            "back from the first is the last"
        );
        assert_eq!(landing(&rows, &takes, None, false), Some(1));
        assert_eq!(landing(&rows, &takes, None, true), Some(8));
        // ONE TITLE IS NO SECTIONS: `TAB` is the window's.
        assert_eq!(landing(&rows[..3], &takes[..3], Some(1), false), None);
    }
}
