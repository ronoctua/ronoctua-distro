//! The workspaces that hold something, as numbers on the modeline.
//!
//! What is open where is live compositor state, and it arrives the way the
//! key mode does: on the footer's event thread, which costs nothing while
//! nothing moves (`ronoctua_compositor::Event::Workspaces`). It is not a
//! brain task — the brain may not speak to the compositor, and a minute's
//! cadence is not a modeline.
//!
//! **Fewer than two is NOTHING.** One occupied workspace is where the person
//! already is, and zero is a desk with nothing on it: a number there would say
//! nothing a glance does not. A modeline at rest says nothing.
//!
//! **The current one is BRIGHTER, not blue** (`colour_of`): on this line the
//! accent means only "the footer holds the keyboard", and a workspace number
//! in it would read as that.

use ronoctua_compositor::Workspace;
use ronoctua_tokens as tokens;

/// The ten marks the modeline can carry at once, one per numbered workspace.
///
/// TEN literals and not a `format!`: `layout::Segment::id` is a `&'static
/// str`. Ten is what the compositor's own keys reach — `1` to `9`, and `0` is
/// the tenth.
pub const POOL: [&str; 10] = [
    "ws-1", "ws-2", "ws-3", "ws-4", "ws-5", "ws-6", "ws-7", "ws-8", "ws-9", "ws-10",
];

/// One number to draw: which workspace, and whether it is the one in front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown {
    /// The workspace's number, which is also its place in [`POOL`].
    pub number: u32,
    /// Whether it is the focused workspace.
    pub current: bool,
}

/// What the modeline draws: the numbered workspaces that hold something, in
/// order — and nothing at all when fewer than two do.
///
/// A named workspace is left out: its name is not a number to press, and the
/// pool has no slot for it.
#[must_use]
pub fn shown(all: &[Workspace]) -> Vec<Shown> {
    let mut out: Vec<Shown> = all
        .iter()
        .filter(|w| w.occupied)
        .filter_map(|w| {
            let number = w.number?;
            id_of(number)?;
            Some(Shown {
                number,
                current: w.focused,
            })
        })
        .collect();
    if out.len() < 2 {
        return Vec::new();
    }
    out.sort_by_key(|s| s.number);
    out.dedup_by_key(|s| s.number);
    out
}

/// The segment id of workspace `number`, when the pool has one.
#[must_use]
pub fn id_of(number: u32) -> Option<&'static str> {
    POOL.get(usize::try_from(number).ok()?.checked_sub(1)?)
        .copied()
}

/// The workspace a segment id names, when it is one of ours.
#[must_use]
pub fn number_of(id: &str) -> Option<u32> {
    let at = POOL.iter().position(|p| *p == id)?;
    u32::try_from(at + 1).ok()
}

/// The colour of a number: the current one a step brighter on the same
/// neutral ramp, every other one quiet.
#[must_use]
pub fn colour_of(current: bool) -> tokens::Color {
    if current {
        tokens::color::TEXT_PRIMARY
    } else {
        tokens::color::TEXT_DIM
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(number: Option<u32>, focused: bool, occupied: bool) -> Workspace {
        Workspace {
            number,
            focused,
            occupied,
        }
    }

    /// FAILS IF: one occupied workspace draws a number — the owner's rule is
    /// that one or none draws nothing.
    #[test]
    fn one_or_none_draws_nothing() {
        assert!(shown(&[]).is_empty());
        assert!(shown(&[at(Some(1), true, true)]).is_empty());
        // The focused one EMPTY does not count: it is where the person is,
        // not something open.
        assert!(shown(&[at(Some(1), true, false), at(Some(3), false, true)]).is_empty());
        // Nor does a named one, which has no number to draw.
        assert!(shown(&[at(None, false, true), at(Some(3), true, true)]).is_empty());
    }

    /// FAILS IF: the numbers come in the compositor's order rather than
    /// their own, or the current one is not the focused one.
    #[test]
    fn two_or_more_draw_in_order_with_the_current_one_marked() {
        let got = shown(&[
            at(Some(4), false, true),
            at(Some(2), true, false),
            at(Some(1), false, true),
            at(Some(3), true, true),
        ]);
        assert_eq!(
            got,
            [
                Shown {
                    number: 1,
                    current: false
                },
                Shown {
                    number: 3,
                    current: true
                },
                Shown {
                    number: 4,
                    current: false
                },
            ]
        );
    }

    /// FAILS IF: a segment id and a workspace number stop being each other's
    /// inverse — the click would go to a workspace other than the one drawn.
    #[test]
    fn the_pool_and_the_numbers_are_each_others_inverse() {
        for n in 1..=10 {
            assert_eq!(number_of(id_of(n).unwrap()), Some(n));
        }
        assert_eq!(id_of(0), None);
        assert_eq!(id_of(11), None);
        assert_eq!(number_of("bt-1"), None);
        // Beyond the pool is left out rather than drawn with no way to click.
        assert!(shown(&[at(Some(12), false, true), at(Some(1), true, true)]).is_empty());
    }

    /// FAILS IF: the current number is drawn in the accent, which on the
    /// modeline means only that the footer holds the keyboard.
    #[test]
    fn the_current_one_is_brighter_and_never_the_accent() {
        assert_ne!(colour_of(true), colour_of(false));
        assert_ne!(colour_of(true), tokens::color::STATE_ACCENT);
    }
}
