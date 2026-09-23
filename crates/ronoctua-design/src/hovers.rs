//! Where every pointer rests on a buffer — one cell per cursor.
//!
//! Every cursor is a seat, and the host tells a buffer which one a hover is
//! (`pointed.seat` in the WIT). A buffer that kept ONE hover would have two
//! cursors taking it from each other on every motion, and one leaving would
//! put out the other's highlight. So a buffer keeps this instead: a cell per
//! cursor, set by `Input::Hover`, cleared by `Input::Unhover`, and read when
//! the rows are drawn — each lit row in its own cursor's colour, through
//! [`crate::Role::hover_of`].

use alloc::vec::Vec;

/// A cell per cursor, in the order the cursors first arrived.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hovers {
    at: Vec<(u16, (u16, u16))>,
}

impl Hovers {
    /// No cursor resting anywhere — `const`, because a buffer's state is often
    /// built in a `const fn`.
    #[must_use]
    pub const fn new() -> Self {
        Self { at: Vec::new() }
    }

    /// Cursor `seat` rests on `cell`. `true` if that moved anything.
    pub fn set(&mut self, seat: u16, cell: (u16, u16)) -> bool {
        match self.at.iter_mut().find(|(s, _)| *s == seat) {
            Some((_, here)) if *here == cell => false,
            Some((_, here)) => {
                *here = cell;
                true
            }
            None => {
                self.at.push((seat, cell));
                true
            }
        }
    }

    /// Cursor `seat` left. `true` if it had been resting anywhere.
    pub fn clear(&mut self, seat: u16) -> bool {
        let before = self.at.len();
        self.at.retain(|(s, _)| *s != seat);
        self.at.len() != before
    }

    /// Every cursor and the cell it rests on, first arrival first.
    pub fn cells(&self) -> impl Iterator<Item = (u16, (u16, u16))> + '_ {
        self.at.iter().copied()
    }

    /// Whether any cursor rests anywhere.
    #[must_use]
    pub fn any(&self) -> bool {
        !self.at.is_empty()
    }

    /// The first cursor whose cell answers `on` — what a row asks when it
    /// is drawn: "is a hand on me, and whose?". The first to arrive wins a
    /// row two hands share, so the picture is the same on every frame.
    pub fn first(&self, mut on: impl FnMut((u16, u16)) -> bool) -> Option<u16> {
        self.at.iter().find(|(_, c)| on(*c)).map(|(s, _)| *s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_cursors_keep_two_cells() {
        let mut h = Hovers::default();
        assert!(h.set(0, (1, 4)));
        assert!(h.set(3, (1, 9)));
        assert!(!h.set(0, (1, 4)), "the same cell is no change");
        assert_eq!(h.first(|(_, l)| l == 9), Some(3));
        assert!(h.clear(0));
        assert_eq!(
            h.first(|(_, l)| l == 4),
            None,
            "one leaving put out only its own"
        );
        assert_eq!(h.first(|(_, l)| l == 9), Some(3));
    }

    #[test]
    fn a_row_two_hands_share_is_the_first_ones() {
        let mut h = Hovers::default();
        h.set(5, (0, 2));
        h.set(1, (3, 2));
        assert_eq!(h.first(|(_, l)| l == 2), Some(5));
    }
}
