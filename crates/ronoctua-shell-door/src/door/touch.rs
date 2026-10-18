//! A finger on a touch screen, turned into what a mouse would have done — and
//! the RIGHT button, which a finger has none of: holding it still.
//!
//! # Why a finger is not a left button the instant it lands
//!
//! Because by the time it is known to be a long press, a left button pressed
//! on landing would already have done its work — selected a row, opened a
//! menu, started a drag. So a finger is PENDING first:
//!
//! - lifted before [`LONG_PRESS`] — a tap, which is a left click;
//! - moved past [`SLOP`] — a drag, which is the left button held from where
//!   it landed;
//! - still at [`LONG_PRESS`] — a right click where it is, and lifting it
//!   afterwards does nothing more.
//!
//! Holding the LEFT button needs nothing of its own: a finger that lands and
//! moves is that already.
//!
//! # On our own surfaces, every one of them
//!
//! The owner's choice (2026-09-25). A foreign app hears the finger as the
//! compositor hands it, and most already open their context menu on a long
//! press; making that system-wide would mean taking the touch screen from the
//! compositor, which nothing here does.
//!
//! Every door of this shell hears it, because [`crate::door::Door`] binds
//! `wl_touch` for all of them and plays the finger through the SAME pointer
//! code a mouse goes through — a door written tomorrow answers a finger
//! without a line about touch. The footer keeps its own fingers (one per key
//! of the on-screen keyboard) and plays the rest through [`Finger`].

/// How long a still finger is held before it is a right click. Half a
/// second: what GTK and Android wait, so a hand that knows either knows this.
pub const LONG_PRESS: std::time::Duration = std::time::Duration::from_millis(500);

/// How far a finger may wander, in logical pixels, and still be "still": a
/// fingertip is wide, and it rolls as it presses.
pub const SLOP: f64 = 10.0;

/// A mouse button, as a finger plays it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// The primary.
    Left,
    /// The secondary: a long press.
    Right,
}

/// What the finger did, as a mouse would have: the position is where.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Out {
    /// The pointer is here.
    Motion((f64, f64)),
    /// A button went down here.
    Press(Button, (f64, f64)),
    /// A button came up here.
    Release(Button, (f64, f64)),
}

/// One finger's state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Finger {
    /// Landed, and not yet known to be a tap, a drag or a long press.
    Pending {
        /// Where it landed.
        at: (f64, f64),
        /// Which landing this is, so a timer armed for an earlier finger with
        /// the same id does not fire on this one.
        landing: u64,
    },
    /// Moved: the left button is held, and it follows.
    Dragging {
        /// Where it is now.
        at: (f64, f64),
    },
    /// Held still long enough: the right click has been made.
    Held,
}

impl Finger {
    /// A finger landing at `at`: the pointer goes there, and nothing is
    /// pressed yet.
    #[must_use]
    pub const fn down(at: (f64, f64), landing: u64) -> (Self, Vec<Out>) {
        (Self::Pending { at, landing }, Vec::new())
    }

    /// The finger moved to `to`.
    pub fn motion(&mut self, to: (f64, f64)) -> Vec<Out> {
        match *self {
            Self::Pending { at, .. } => {
                if (to.0 - at.0).hypot(to.1 - at.1) <= SLOP {
                    return Vec::new();
                }
                // A DRAG, from where it LANDED: the press happens there, and the
                // pointer then follows, as a mouse held down would.
                *self = Self::Dragging { at: to };
                vec![
                    Out::Motion(at),
                    Out::Press(Button::Left, at),
                    Out::Motion(to),
                ]
            }
            Self::Dragging { .. } => {
                *self = Self::Dragging { at: to };
                vec![Out::Motion(to)]
            }
            Self::Held => Vec::new(),
        }
    }

    /// The finger lifted.
    #[must_use]
    pub const fn up(self) -> [Option<Out>; 3] {
        match self {
            // A TAP: a whole left click where it landed.
            Self::Pending { at, .. } => [
                Some(Out::Motion(at)),
                Some(Out::Press(Button::Left, at)),
                Some(Out::Release(Button::Left, at)),
            ],
            Self::Dragging { at } => [Some(Out::Release(Button::Left, at)), None, None],
            // THE RIGHT CLICK WAS ALREADY MADE, and lifting is not a second
            // click of anything.
            Self::Held => [None, None, None],
        }
    }

    /// [`LONG_PRESS`] passed since landing `landing`: a right click, if this
    /// is still that finger and it never moved.
    pub fn held_long(&mut self, landing: u64) -> Vec<Out> {
        match *self {
            Self::Pending { at, landing: l } if l == landing => {
                *self = Self::Held;
                vec![
                    Out::Motion(at),
                    Out::Press(Button::Right, at),
                    Out::Release(Button::Right, at),
                ]
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lifted(f: Finger) -> Vec<Out> {
        f.up().into_iter().flatten().collect()
    }

    #[test]
    fn a_tap_is_a_left_click_where_it_landed() {
        let (f, now) = Finger::down((10.0, 20.0), 1);
        assert!(now.is_empty(), "nothing is pressed on landing");
        assert_eq!(
            lifted(f),
            [
                Out::Motion((10.0, 20.0)),
                Out::Press(Button::Left, (10.0, 20.0)),
                Out::Release(Button::Left, (10.0, 20.0)),
            ]
        );
    }

    #[test]
    fn a_still_finger_held_is_one_right_click_and_lifting_it_is_nothing() {
        let (mut f, _) = Finger::down((10.0, 20.0), 1);
        // A fingertip rolls a little as it presses.
        assert!(f.motion((14.0, 23.0)).is_empty());
        assert_eq!(
            f.held_long(1),
            [
                Out::Motion((10.0, 20.0)),
                Out::Press(Button::Right, (10.0, 20.0)),
                Out::Release(Button::Right, (10.0, 20.0)),
            ]
        );
        assert!(f.held_long(1).is_empty(), "once");
        assert!(f.motion((80.0, 80.0)).is_empty(), "and no drag after it");
        assert!(lifted(f).is_empty());
    }

    #[test]
    fn a_finger_that_moves_is_a_drag_from_where_it_landed() {
        let (mut f, _) = Finger::down((10.0, 20.0), 1);
        assert_eq!(
            f.motion((40.0, 20.0)),
            [
                Out::Motion((10.0, 20.0)),
                Out::Press(Button::Left, (10.0, 20.0)),
                Out::Motion((40.0, 20.0)),
            ]
        );
        assert_eq!(f.motion((60.0, 20.0)), [Out::Motion((60.0, 20.0))]);
        assert!(
            f.held_long(1).is_empty(),
            "a drag held still is not a right click"
        );
        assert_eq!(lifted(f), [Out::Release(Button::Left, (60.0, 20.0))]);
    }

    #[test]
    fn a_timer_for_an_earlier_landing_does_not_fire() {
        let (mut f, _) = Finger::down((10.0, 20.0), 2);
        assert!(f.held_long(1).is_empty());
        assert!(matches!(f, Finger::Pending { .. }));
    }
}

/// Every finger down on one door, and the hand they play.
///
/// # Why the fingers have ONE hand of their own
///
/// Because a finger is a pointer the way a mouse is, and each pointer keeps its
/// own drag, hover and wheel (`crate::seats`) — a finger that shared the
/// mouse's would drag the list the mouse was holding. One and not one per
/// finger: every finger of a door plays the SAME pointer, which is what the
/// compositor's own emulation did for every door before they heard touch.
#[derive(Debug, Default)]
pub struct Fingers<H> {
    down: Vec<(i32, Finger)>,
    landings: u64,
    hand: H,
}

impl<H> Fingers<H> {
    /// The hand the fingers play.
    #[must_use]
    pub const fn hand(&self) -> &H {
        &self.hand
    }

    /// Swaps the fingers' hand with `here`, so a door runs its pointer code
    /// over it.
    pub fn swap_hand(&mut self, here: &mut H) {
        std::mem::swap(&mut self.hand, here);
    }

    /// Finger `id` landed at `at`: the landing's number, for the clock that
    /// turns it into a long press, and what the pointer does now.
    pub fn down(&mut self, id: i32, at: (f64, f64)) -> (u64, Vec<Out>) {
        self.landings += 1;
        let landing = self.landings;
        let (finger, _) = Finger::down(at, landing);
        self.down.retain(|(f, _)| *f != id);
        self.down.push((id, finger));
        // THE POINTER GOES THERE, so what is under it lights — and nothing is
        // pressed yet: see [`Finger`].
        (landing, vec![Out::Motion(at)])
    }

    /// Finger `id` moved.
    pub fn motion(&mut self, id: i32, to: (f64, f64)) -> Vec<Out> {
        self.down
            .iter_mut()
            .find(|(f, _)| *f == id)
            .map(|(_, finger)| finger.motion(to))
            .unwrap_or_default()
    }

    /// Finger `id` lifted.
    pub fn up(&mut self, id: i32) -> Vec<Out> {
        let Some(at) = self.down.iter().position(|(f, _)| *f == id) else {
            return Vec::new();
        };
        let (_, finger) = self.down.remove(at);
        finger.up().into_iter().flatten().collect()
    }

    /// The long press's clock rang for finger `id`, landed as `landing`.
    pub fn held_long(&mut self, id: i32, landing: u64) -> Vec<Out> {
        self.down
            .iter_mut()
            .find(|(f, _)| *f == id)
            .map(|(_, finger)| finger.held_long(landing))
            .unwrap_or_default()
    }

    /// The compositor took the fingers back.
    pub fn cancel(&mut self) {
        self.down.clear();
    }
}

#[cfg(test)]
mod fingers {
    use super::*;

    /// TWO FINGERS ARE TWO, and each lifts its own: a finger lifting does not
    /// end another's drag.
    #[test]
    fn each_finger_is_its_own() {
        let mut f: Fingers<()> = Fingers::default();
        let (one, _) = f.down(1, (0.0, 0.0));
        let (two, _) = f.down(2, (100.0, 0.0));
        assert_ne!(one, two);
        assert_eq!(
            f.motion(2, (150.0, 0.0)).first(),
            Some(&Out::Motion((100.0, 0.0)))
        );
        assert_eq!(
            f.up(1),
            [
                Out::Motion((0.0, 0.0)),
                Out::Press(Button::Left, (0.0, 0.0)),
                Out::Release(Button::Left, (0.0, 0.0)),
            ]
        );
        assert_eq!(f.up(2), [Out::Release(Button::Left, (150.0, 0.0))]);
        assert!(f.up(2).is_empty(), "a finger lifts once");
    }

    /// A LONG PRESS FOR A FINGER THAT HAS LIFTED is nothing.
    #[test]
    fn a_long_press_after_the_lift_is_nothing() {
        let mut f: Fingers<()> = Fingers::default();
        let (landing, _) = f.down(1, (0.0, 0.0));
        let _ = f.up(1);
        assert!(f.held_long(1, landing).is_empty());
    }
}
