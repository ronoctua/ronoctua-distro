//! Which window the keyboards type into — pure, and the whole of the rule.
//!
//! # The keyboards are on ONE seat
//!
//! Every keyboard stays on cursor [`FIRST`]'s seat (`seat0`), and a seat's
//! keys go to that seat's focus. So without this, clicking a window with
//! cursor 2 gives it cursor 2's pointer and nothing to type with.
//!
//! # How typing follows another cursor without moving a keyboard
//!
//! Two facts of the compositor, read in its source:
//!
//! - its window `focus` event fires when ANY seat's focus changes, and
//!   `get_seats` says what every seat is focused on;
//! - a command that arrives over IPC runs on the DEFAULT seat, `seat0`
//!   (`execute_command` with no seat picks it) — so asking to focus a window
//!   is asking the keyboards' seat to focus it.
//!
//! So when the cursor that typing follows focuses a window, the keyboards'
//! seat is asked to focus the same one. No device moves, and nothing here is
//! a patch.
//!
//! # Only on a CHANGE of the followed cursor
//!
//! Typing moves when the followed cursor's focus MOVES, never merely because
//! the keyboards' seat is somewhere else: a window picked from the keyboard
//! (`LEADER w n`) is the keyboards' own choice, and pulling it back to the
//! other cursor's window on the next event would make that choice impossible.

use std::collections::BTreeMap;

use ronoctua_compositor::Seat;
use serde::{Deserialize, Serialize};

use crate::plan::seat_of;
use crate::prefs::FIRST;

/// Which cursor the keyboards type through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "follows", content = "cursor")]
pub enum Typing {
    /// Where cursor [`FIRST`] focused — the keyboards' own seat, and what the
    /// compositor does with nothing asked of it.
    First,
    /// Where this cursor focused.
    Cursor(u16),
    /// Where the last cursor to focus a window focused it — the distro's
    /// default, the owner's choice.
    #[default]
    Latest,
}

/// What every seat was focused on, as last seen.
pub type Focuses = BTreeMap<String, Option<u64>>;

/// What every seat is focused on now.
#[must_use]
pub fn focuses(seats: &[Seat]) -> Focuses {
    seats.iter().map(|s| (s.name.clone(), s.focus)).collect()
}

/// The window the keyboards' seat should focus, now that the seats are
/// `now` and were `before` — `None` when typing stays where it is.
///
/// `before` empty is "nothing seen yet": the first reading is only
/// recorded, or the thread's start would pick an arbitrary seat's window.
#[must_use]
pub fn goes_to(typing: Typing, before: &Focuses, now: &[Seat]) -> Option<u64> {
    if before.is_empty() {
        return None;
    }
    let keyboards = seat_of(FIRST);
    let typing_on = now
        .iter()
        .find(|s| s.name == keyboards)
        .and_then(|s| s.focus);
    let moved = |s: &&Seat| {
        s.name != keyboards && s.focus.is_some() && before.get(&s.name) != Some(&s.focus)
    };
    let target = match typing {
        Typing::First => None,
        Typing::Cursor(n) if n == FIRST => None,
        Typing::Cursor(n) => {
            let followed = seat_of(n);
            now.iter()
                .filter(|s| s.name == followed)
                .find(moved)
                .and_then(|s| s.focus)
        }
        Typing::Latest => now.iter().find(moved).and_then(|s| s.focus),
    }?;
    (Some(target) != typing_on).then_some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(name: &str, focus: Option<u64>) -> Seat {
        Seat {
            name: name.to_owned(),
            devices: Vec::new(),
            focus,
        }
    }

    fn desk(one: Option<u64>, two: Option<u64>, three: Option<u64>) -> Vec<Seat> {
        vec![
            seat("seat0", one),
            seat("ronoctua-cursor-2", two),
            seat("ronoctua-cursor-3", three),
        ]
    }

    #[test]
    fn by_default_typing_stays_on_the_first_cursor() {
        let before = focuses(&desk(Some(10), Some(20), None));
        assert_eq!(
            goes_to(Typing::First, &before, &desk(Some(10), Some(21), None)),
            None
        );
    }

    #[test]
    fn a_followed_cursor_takes_typing_where_it_clicks() {
        let before = focuses(&desk(Some(10), Some(20), Some(30)));
        let now = desk(Some(10), Some(21), Some(30));
        assert_eq!(goes_to(Typing::Cursor(2), &before, &now), Some(21));
        assert_eq!(
            goes_to(Typing::Cursor(3), &before, &now),
            None,
            "cursor 3 did not move"
        );
    }

    #[test]
    fn a_window_picked_from_the_keyboard_is_not_pulled_back() {
        // The keyboards' seat moved to 11 on its own; cursor 2 stayed on 20.
        let before = focuses(&desk(Some(10), Some(20), None));
        let now = desk(Some(11), Some(20), None);
        assert_eq!(goes_to(Typing::Cursor(2), &before, &now), None);
        assert_eq!(goes_to(Typing::Latest, &before, &now), None);
    }

    #[test]
    fn the_latest_cursor_to_click_takes_typing() {
        let before = focuses(&desk(Some(10), Some(20), Some(30)));
        assert_eq!(
            goes_to(Typing::Latest, &before, &desk(Some(10), Some(20), Some(31))),
            Some(31)
        );
    }

    #[test]
    fn nothing_is_asked_when_typing_is_already_there() {
        // After our own ask the keyboards' seat is on 21 as well: the event
        // that ask raises must not raise another.
        let before = focuses(&desk(Some(10), Some(20), None));
        assert_eq!(
            goes_to(Typing::Latest, &before, &desk(Some(21), Some(21), None)),
            None
        );
    }

    #[test]
    fn the_first_reading_is_only_recorded() {
        assert_eq!(
            goes_to(
                Typing::Latest,
                &Focuses::new(),
                &desk(Some(10), Some(20), None)
            ),
            None
        );
    }

    #[test]
    fn a_cursor_that_leaves_every_window_takes_nothing() {
        let before = focuses(&desk(Some(10), Some(20), None));
        assert_eq!(
            goes_to(Typing::Cursor(2), &before, &desk(Some(10), None, None)),
            None
        );
    }
}
