//! THE PAGES OF A LIST OF VIDEOS — the two lists this crate pages, the
//! catalogue's Videos page (`video_metrics`) and the player's ranking
//! (`player_ranking`), alike (the owner, 2026-11-10): how many videos a page
//! holds, a ring the person turns ([`PerPage`]); how many pages a list
//! takes; the mark beside the list's title — "1 of 2", in a quiet ink — and
//! the arrows at its edge and at the list's end, each one only where it can
//! move ([`arrows`]); and, when the size changes, the page that still holds
//! the first video the person was looking at ([`resized`]).
//!
//! # One arithmetic, two lists
//!
//! The two lists are drawn by two builders — the catalogue's own rows, the
//! player's — and each places its title and its end where its rows go. What
//! they share is what a page IS: here, once, so "1 of 2" and the arrows
//! never disagree between the footer's region of one and the window of the
//! other.

use crate::key;
use ronoctua_i18n::Arg;

/// HOW MANY VIDEOS A PAGE MAY HOLD, a ring the person turns on the row under
/// the page's (the owner, 2026-11-10): 25 at first, then 50 and 100 —
/// THE SESSION'S, as the list's other filters are, and never written down.
///
/// THE LARGEST IS DRAWN WHOLE: a page of the grid is a page of the list, and
/// every one of its thumbnails must fit what the renderer's frame keeps
/// (`video_metrics::PICTURES_MOST`, which the const assert there holds the
/// last of these to).
pub const PER_PAGES: [u32; 3] = [25, 50, 100];

/// HOW MANY VIDEOS A PAGE HOLDS, a member of [`PER_PAGES`] — the first until
/// the person turns it: a derived `0` would be a page of nothing, and a list
/// that never ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PerPage(u32);

impl Default for PerPage {
    fn default() -> Self {
        Self(PER_PAGES[0])
    }
}

impl PerPage {
    /// How many videos, a page.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// The next size of the ring — or the one before — round its ends.
    #[must_use]
    pub fn turned(self, on: bool) -> Self {
        Self(ring(self.0, on))
    }
}

/// How many pages `total` videos make at `per` a page — none for none.
#[must_use]
pub fn pages_of(total: u64, per: u32) -> u32 {
    u32::try_from(total.div_ceil(u64::from(per.max(1)))).unwrap_or(u32::MAX)
}

/// The size of [`PER_PAGES`] after `now` — or before it — round the ends;
/// the first for a size the ring does not hold.
#[must_use]
pub fn ring(now: u32, on: bool) -> u32 {
    let n = PER_PAGES.len();
    let to = match (PER_PAGES.iter().position(|&p| p == now), on) {
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
        (None, _) => 0,
    };
    PER_PAGES[to]
}

/// THE PAGE THAT HOLDS THE FIRST VIDEO OF PAGE `page`, its size turned from
/// `from` a page to `to`: the list stays where the person was looking — the
/// owner's choice (2026-11-10), against going back to the first page as a
/// filter changed does.
#[must_use]
pub fn resized(page: u32, from: u32, to: u32) -> u32 {
    let first = u64::from(page) * u64::from(from);
    u32::try_from(first / u64::from(to.max(1))).unwrap_or(u32::MAX)
}

/// WHICH ARROWS A LIST'S TITLE AND ITS END DRAW on page `page` (from 0) of
/// `pages`: `[earlier, later]`, each `true` where it can move — an arrow that
/// cannot leaves ITS PLACE EMPTY, and the other keeps its own. `None` for a
/// list of one page or none: no arrow at all, and no mark (the owner's
/// choice, 2026-11-10 — the Page row still says "1 of 1").
#[must_use]
pub const fn arrows(page: u32, pages: u32) -> Option<[bool; 2]> {
    if pages <= 1 {
        return None;
    }
    Some([page > 0, page + 1 < pages])
}

/// THE MARK BESIDE A LIST'S TITLE — "1 of 2" — in the player's words for a
/// page of a listing (`player-catalog-page-of`, the numbers as NUMBERS:
/// `numeric-selectors-get-numbers`); `None` where [`arrows`] draws none.
#[must_use]
pub fn mark(page: u32, pages: u32, texts: &ronoctua_i18n::Texts) -> Option<String> {
    arrows(page, pages)?;
    Some(texts.t_args(
        key::PLAYER_CATALOG_PAGE_OF,
        &[
            ("page", Arg::Number(i64::from(page) + 1)),
            ("pages", Arg::Number(i64::from(pages))),
        ],
    ))
}

/// The spot of a list's title — the row a turned page brings to the top of
/// the door, so what the person turned to is what they see.
pub const LIST_TITLE: &str = "list-title";

/// The spot of the arrows at a list's end.
pub const LIST_PAGER: &str = "list-pager";

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the ring skips a size or does not go round, a size it does
    /// not hold is not taken back to its first, or a page's size starts at
    /// anything but 25.
    #[test]
    fn the_sizes_turn_in_a_ring() {
        assert_eq!(PerPage::default().get(), 25);
        assert_eq!(ring(25, true), 50);
        assert_eq!(ring(50, true), 100);
        assert_eq!(ring(100, true), 25);
        assert_eq!(ring(25, false), 100);
        assert_eq!(ring(24, true), 25);
        assert_eq!(PerPage::default().turned(true).turned(true).get(), 100);
    }

    /// FAILS IF: a page of a new size does not hold the first video of the
    /// page that was on screen — the person loses their place in the list.
    #[test]
    fn a_new_size_keeps_the_first_video_in_sight() {
        for (page, from, to) in [
            (0, 25, 50),
            (3, 25, 50),
            (3, 25, 100),
            (1, 100, 25),
            (7, 50, 25),
        ] {
            let first = u64::from(page) * u64::from(from);
            let at = resized(page, from, to);
            let held = u64::from(at) * u64::from(to)..u64::from(at + 1) * u64::from(to);
            assert!(held.contains(&first), "{page}·{from} → {to}: {at}");
        }
    }

    /// FAILS IF: an arrow is drawn where it cannot move, the one beside it
    /// is not, or a list of one page draws arrows or a mark.
    #[test]
    fn the_arrows_stop_at_the_ends() {
        assert_eq!(arrows(0, 0), None);
        assert_eq!(arrows(0, 1), None);
        assert_eq!(arrows(0, 2), Some([false, true]));
        assert_eq!(arrows(1, 2), Some([true, false]));
        assert_eq!(arrows(1, 3), Some([true, true]));
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("a declared language");
        assert_eq!(mark(0, 1, &t), None);
        assert_eq!(mark(0, 2, &t).as_deref(), Some("1 of 2"));
        assert_eq!(pages_of(0, 25), 0);
        assert_eq!(pages_of(25, 25), 1);
        assert_eq!(pages_of(26, 25), 2);
    }
}
