//! `TAB` FROM SECTION TO SECTION — the first row of the next section's, and
//! `Shift+TAB` the previous one's, in a ring.
//!
//! The network's view walked its sections so (`LEADER s n`), and the owner
//! asked for it in every window and view where `TAB` means nothing else
//! (2026-11-09) — and in every one written after. So it is answered ONCE, for
//! every list the door or the footer draws, from what both already have: the
//! rows, which ones a click takes and where the cursor is. A section begins at
//! a TITLE — a row in the title ink (`text.heading`), or a drawer's line,
//! see [`is_title`] —
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
        .filter(|(i, fields)| is_title(fields, takes_at(*i)))
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

/// Whether a row is a section's TITLE: a row in the title ink, or a
/// drawer's line.
///
/// It was only a row that takes no click, and a title that grew controls —
/// a list's page arrows at the end of its line, a layout's glyphs, a
/// drawer's chevron — stopped being one: its section was folded into the
/// one above, and a page left with one title turned `TAB` off altogether
/// (owner, 2026-11-10: a title with glyphs is still a section). So a title
/// is told by its ink, whether it takes a click or not:
///
/// - a row that takes NONE is one with any field in the title ink, as it
///   always was;
/// - a row that takes a click is one when its FIRST field is — its words,
///   before what it offers — so a row that only carries a heading-ink thing
///   somewhere along it (a chosen unit's name, a column's lit heading after
///   a mark) is not mistaken for one;
/// - and a DRAWER's line is one whatever its ink: the row that begins with
///   the chevron that opens and closes it. The ink alone was not enough
///   there, because the cursor changes it — a drawer takes the accent whole
///   under the cursor, chevron and word, so it was a title at rest and none
///   while the cursor stood on it, and `Shift+TAB` from the drawer's own line
///   skipped the section above (its rows were counted in that one). The
///   chevron is the drawer's and nobody else's (the owner refused one on a
///   row that acts), so it tells a drawer by what it is, not by a colour.
#[must_use]
pub fn is_title(fields: &[crate::fields::Field], takes: bool) -> bool {
    let heading = |f: &crate::fields::Field| f.color == tokens::color::TEXT_HEADING;
    let drawer = |f: &crate::fields::Field| {
        f.text
            .starts_with([tokens::icon::EXPAND, tokens::icon::COLLAPSE])
    };
    if takes {
        fields.first().is_some_and(|f| heading(f) || drawer(f))
    } else {
        fields.iter().any(heading)
    }
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

    /// FAILS IF: a title that takes a click — its words first, then the
    /// glyphs at the end of its line, a page's arrows — stops beginning a
    /// section, so its rows fold into the section above and a page with two
    /// titles loses `TAB`; or a row that takes a click and carries the
    /// title ink only after something else (a chosen unit's name behind its
    /// mark) is taken for a title.
    #[test]
    fn a_title_with_glyphs_still_begins_a_section() {
        let h = tokens::color::TEXT_HEADING;
        let r = tokens::color::TEXT_PRIMARY;
        let titled = |glyphs: usize| {
            let mut fields = row(h);
            for n in 0..glyphs {
                fields.push(crate::fields::Field {
                    at: 10 + u16::try_from(n).unwrap_or(0) * 2,
                    ..row(h).remove(0)
                });
            }
            fields
        };
        let after_a_mark = {
            let mut fields = row(r);
            fields.push(crate::fields::Field {
                at: 2,
                ..row(h).remove(0)
            });
            fields
        };
        // 0 title, 1 2 rows; 3 air; 4 a title with two glyphs a click takes,
        // 5 a row, 6 a row with the title ink behind a mark, 7 a row.
        let rows = vec![
            row(h),
            row(r),
            row(r),
            Vec::new(),
            titled(2),
            row(r),
            after_a_mark,
            row(r),
        ];
        let takes = [false, true, true, false, true, true, true, true];
        assert!(is_title(&rows[4], true), "a title with glyphs");
        assert!(!is_title(&rows[6], true), "a row after a mark");
        assert_eq!(
            landing(&rows, &takes, Some(1), false),
            Some(5),
            "the title with glyphs does not begin a section"
        );
        assert_eq!(
            landing(&rows, &takes, Some(5), false),
            Some(1),
            "a row with the title ink after a mark was taken for a title"
        );
        assert_eq!(landing(&rows, &takes, Some(7), true), Some(1));
        // AND ON THE TITLE ITSELF the cursor is in its section: on to the
        // next one, and back to the one before.
        assert_eq!(landing(&rows, &takes, Some(4), false), Some(1));
        assert_eq!(landing(&rows, &takes, Some(4), true), Some(1));
        // AND A TITLE THAT TAKES NO CLICK is still told by any field.
        let mut quiet = row(r);
        quiet.push(row(h).remove(0));
        assert!(is_title(&quiet, false));
    }

    /// FAILS IF: a drawer's line stops beginning a section while the cursor
    /// is on it — a drawer takes the accent whole there, chevron and word, so
    /// a title told by its ink alone is one at rest and none under the
    /// cursor, and `Shift+TAB` from the drawer's own line skips the section
    /// above it — or an ordinary row in the cursor's accent is taken for a
    /// title.
    #[test]
    fn a_drawer_begins_its_section_under_the_cursor_too() {
        let h = tokens::color::TEXT_HEADING;
        let r = tokens::color::TEXT_PRIMARY;
        let a = tokens::color::STATE_ACCENT;
        let drawer = |colour: ronoctua_tokens::Color| {
            vec![
                crate::fields::Field {
                    text: tokens::icon::COLLAPSE.to_string(),
                    ..row(colour).remove(0)
                },
                crate::fields::Field {
                    at: 2,
                    ..row(colour).remove(0)
                },
            ]
        };
        // 0 title, 1 a row; 2 air; 3 title, 4 a row; 5 air; 6 the drawer,
        // open, under the cursor, 7 8 its rows.
        let rows = vec![
            row(h),
            row(r),
            Vec::new(),
            row(h),
            row(r),
            Vec::new(),
            drawer(a),
            row(r),
            row(r),
        ];
        let takes = [false, true, false, false, true, false, true, true, true];
        assert!(is_title(&drawer(h), true), "the drawer at rest");
        assert!(is_title(&rows[6], true), "the drawer under the cursor");
        assert!(
            !is_title(&row(a), true),
            "an ordinary row in the cursor's accent was taken for a title"
        );
        assert_eq!(
            landing(&rows, &takes, Some(6), true),
            Some(4),
            "Shift+TAB from the drawer's line skipped the section above it"
        );
        assert_eq!(
            landing(&rows, &takes, Some(7), true),
            Some(4),
            "from a row inside the drawer"
        );
        assert_eq!(
            landing(&rows, &takes, Some(4), false),
            Some(7),
            "TAB does not reach the open drawer's first row"
        );
        assert_eq!(
            landing(&rows, &takes, Some(6), false),
            Some(1),
            "TAB from the drawer's line does not go round the ring"
        );
    }
}
