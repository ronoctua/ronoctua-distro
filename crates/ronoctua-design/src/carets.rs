//! Where every KEYBOARD is in a buffer — one caret per seat.
//!
//! The keyboard twin of [`crate::Hovers`]. In Independent mode every keyboard
//! is its own seat, and the host says whose keys come next
//! (`Input::KeyboardSeat`, the cursor-colour number `pointed.seat` carries).
//! A buffer that keeps ONE selection has every keyboard moving the same row;
//! a buffer that keeps this has each keyboard moving its own — a selection
//! per person, a field per person — each drawn in its own cursor's colour
//! through [`crate::Role::selection_of`].
//!
//! # `T` is what a caret IS in this buffer
//!
//! A row index, a field and the text in it, a position in a document —
//! whatever the buffer's keyboard moves. A seat nobody has seen yet starts at
//! `T::default()`.

use alloc::vec::Vec;

/// A caret per seat, in the order the seats first typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Carets<T> {
    at: Vec<(u16, T)>,
    /// The seat whose keys come next.
    now: u16,
}

impl<T: Default> Carets<T> {
    /// No keyboard has typed yet — `const`, as [`crate::Hovers::new`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            at: Vec::new(),
            now: 0,
        }
    }

    /// The next keys are `seat`'s — `Input::KeyboardSeat`.
    pub fn seat(&mut self, seat: u16) {
        self.now = seat;
        if !self.at.iter().any(|(s, _)| *s == seat) {
            self.at.push((seat, T::default()));
        }
    }

    /// `seat`'s keyboard left the window — `Input::KeyboardLeft`. Its caret
    /// stays where it was, so coming back finds its work where it left it;
    /// only whether it is drawn changes (see [`Self::present`]).
    pub fn left(&mut self, seat: u16) {
        if let Some(i) = self.at.iter().position(|(s, _)| *s == seat) {
            let caret = self.at.remove(i);
            // TO THE BACK: a caret that left is drawn under the ones typing.
            self.at.push(caret);
        }
    }

    /// The caret of the seat whose keys come next.
    pub fn here(&mut self) -> &mut T {
        let now = self.now;
        if let Some(i) = self.at.iter().position(|(s, _)| *s == now) {
            return &mut self.at[i].1;
        }
        self.at.push((now, T::default()));
        let last = self.at.len() - 1;
        &mut self.at[last].1
    }

    /// The caret of `seat` — what a click moves: the hand that clicked is
    /// the seat's, and its keyboard's caret goes where it clicked.
    pub fn of(&mut self, seat: u16) -> &mut T {
        if let Some(i) = self.at.iter().position(|(s, _)| *s == seat) {
            return &mut self.at[i].1;
        }
        self.at.push((seat, T::default()));
        let last = self.at.len() - 1;
        &mut self.at[last].1
    }

    /// The seat whose keys come next.
    #[must_use]
    pub const fn now(&self) -> u16 {
        self.now
    }

    /// Every seat and its caret, first typist first.
    pub fn all(&self) -> impl Iterator<Item = (u16, &T)> + '_ {
        self.at.iter().map(|(s, t)| (*s, t))
    }

    /// Whether more than one keyboard has typed here — when each caret is
    /// drawn in its cursor's colour instead of the one selection colour.
    #[must_use]
    pub fn many(&self) -> bool {
        self.at.len() > 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_keyboards_keep_two_carets() {
        let mut c: Carets<usize> = Carets::new();
        c.seat(0);
        *c.here() = 3;
        c.seat(2);
        *c.here() += 1;
        c.seat(0);
        *c.here() += 1;
        let all: Vec<(u16, usize)> = c.all().map(|(s, t)| (s, *t)).collect();
        assert_eq!(all, alloc::vec![(0, 4), (2, 1)]);
        assert!(c.many());
        c.left(0);
        assert_eq!(
            c.all().next().map(|(s, _)| s),
            Some(2),
            "the one typing first"
        );
    }
}
