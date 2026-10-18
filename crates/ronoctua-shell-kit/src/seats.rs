//! Every cursor, heard — the one module every door answers pointers through.
//!
//! # Why this exists
//!
//! A cursor is a SEAT: the compositor draws one per seat and each moves its
//! own focus (`ronoctua-pointers` puts every pointing device on one). A
//! Wayland client is offered every seat, and every door here took the first
//! pointer it was offered and dropped the rest (`if self.pointer.is_none()`),
//! so a second mouse moved a cursor that no window of ours answered: no
//! hover, no click, nothing.
//!
//! # How a door stays one piece of code
//!
//! A door's per-pointer state — what is under the pointer, a drag, the
//! wheel's leftover fraction — is grouped into one value (`PerSeat` in each
//! door) that lives in a field of the door, `here`. That field is the state
//! of the pointer whose events are being handled NOW. [`Seats::switch_to`]
//! is called first thing in `pointer_frame`: it puts the current `here`
//! away under the pointer it belongs to and brings out the one of the
//! pointer that spoke. So the door's own logic — every `self.here.dragging`
//! — runs unchanged, and two cursors can drag two bars at once without ever
//! seeing each other's state.
//!
//! What is drawn from pointer state reads ALL of them: [`Seats::lit`] gives
//! every pointer's state with the colour its band is painted in.
//!
//! # The colour of a band
//!
//! A single cursor keeps the neutral hover the whole interface has always
//! had. With more than one, each band takes its own cursor's hover colour
//! (`cursor.hover.*`) — colour is spent only where it tells two hands apart.

use smithay_client_toolkit::reexports::client::Proxy;
use smithay_client_toolkit::reexports::client::backend::ObjectId;
use smithay_client_toolkit::reexports::client::protocol::{
    wl_keyboard, wl_pointer, wl_seat, wl_touch,
};

use ronoctua_tokens::Color;

/// A seat's device a door holds one of per seat — its pointer, or its
/// keyboard. The same bookkeeping answers both: a keyboard is a seat's too,
/// and in Independent mode there is one per cursor (`ronoctua-keys`).
pub trait Device: Proxy + Clone {
    /// Gives the device back to the compositor.
    fn let_go(&self);
}

impl Device for wl_pointer::WlPointer {
    fn let_go(&self) {
        self.release();
    }
}

impl Device for wl_keyboard::WlKeyboard {
    fn let_go(&self) {
        self.release();
    }
}

impl Device for wl_touch::WlTouch {
    fn let_go(&self) {
        self.release();
    }
}

/// One device and the state stored for it while another is `here`.
#[derive(Debug)]
struct Held<T, D> {
    seat: wl_seat::WlSeat,
    pointer: D,
    /// Which of the cursor colours it wears, from `0`.
    colour: u8,
    /// Its state, while it is not the one `here`. `None` while it IS.
    kept: Option<T>,
}

/// Every pointer — or every keyboard — a door has been offered, and the state
/// of each.
#[derive(Debug)]
pub struct Seats<T, D = wl_pointer::WlPointer> {
    held: Vec<Held<T, D>>,
    /// Which device the door's `here` belongs to.
    here: Option<ObjectId>,
}

impl<T, D> Default for Seats<T, D> {
    fn default() -> Self {
        Self {
            held: Vec::new(),
            here: None,
        }
    }
}

impl<T: Default, D: Device> Seats<T, D> {
    /// A device was offered on `seat`.
    ///
    /// `name` is the seat's name when the compositor has said it, which is
    /// what the cursor's colour is read from (see [`colour_of`]).
    pub fn took(&mut self, seat: &wl_seat::WlSeat, pointer: D, name: Option<&str>) {
        self.held.retain(|h| h.seat.id() != seat.id());
        let colour = colour_of(name, self.held.len());
        self.held.push(Held {
            seat: seat.clone(),
            pointer,
            colour,
            kept: Some(T::default()),
        });
    }

    /// The pointer of `seat` went away. Its state goes with it — a cursor
    /// that is gone cannot still be dragging a bar.
    pub fn lost(&mut self, seat: &wl_seat::WlSeat, here: &mut T) {
        let Some(at) = self.held.iter().position(|h| h.seat.id() == seat.id()) else {
            return;
        };
        let gone = self.held.remove(at);
        if self.here.as_ref() == Some(&gone.pointer.id()) {
            *here = T::default();
            self.here = None;
        }
        gone.pointer.let_go();
    }

    /// Makes `here` the state of `pointer`, putting the current one away.
    ///
    /// Called FIRST in `pointer_frame`, before anything reads `here`.
    pub fn switch_to(&mut self, pointer: &D, here: &mut T) {
        let id = pointer.id();
        if self.here.as_ref() == Some(&id) {
            return;
        }
        if let Some(old) = self.here.take()
            && let Some(h) = self.held.iter_mut().find(|h| h.pointer.id() == old)
        {
            h.kept = Some(std::mem::take(here));
        }
        if let Some(h) = self.held.iter_mut().find(|h| h.pointer.id() == id) {
            *here = h.kept.take().unwrap_or_default();
            self.here = Some(id);
        } else {
            *here = T::default();
        }
    }

    /// Every device held, in the order each was offered — the hand a
    /// photograph's footer points with (`photograph --door footer --hover`).
    pub fn devices(&self) -> impl Iterator<Item = &D> + '_ {
        self.held.iter().map(|h| &h.pointer)
    }

    /// Whether more than one cursor is on this door's screen at all.
    #[must_use]
    pub fn many(&self) -> bool {
        self.held.len() > 1
    }

    /// Every pointer's state, `here` included, with the colour its band is
    /// painted in — the order the pointers arrived in, which is the order a
    /// tie on one row is broken by.
    pub fn lit<'a>(&'a self, here: &'a T) -> impl Iterator<Item = (Color, &'a T)> + 'a {
        let many = self.many();
        self.held.iter().filter_map(move |h| {
            let state = if self.here.as_ref() == Some(&h.pointer.id()) {
                Some(here)
            } else {
                h.kept.as_ref()
            };
            state.map(|s| (band_colour(many, h.colour), s))
        })
    }

    /// The colour number of the pointer `here` — what a buffer is told a
    /// hover is by (`pointed.seat` in the WIT), so two cursors resting on it
    /// light two rows, each in its own colour.
    #[must_use]
    pub fn colour_number_here(&self) -> u16 {
        self.held
            .iter()
            .find(|h| self.here.as_ref() == Some(&h.pointer.id()))
            .map_or(0, |h| u16::from(h.colour))
    }

    /// The state of the device on `seat`, `here` included — what a pointer
    /// asks of the KEYBOARD on its own seat, which is the one whose
    /// modifiers a click reads.
    pub fn state_on<'a>(&'a self, seat: &wl_seat::WlSeat, here: &'a T) -> Option<&'a T> {
        let h = self.held.iter().find(|h| h.seat.id() == seat.id())?;
        if self.here.as_ref() == Some(&h.pointer.id()) {
            Some(here)
        } else {
            h.kept.as_ref()
        }
    }

    /// The seat `here` belongs to — what a drag is started on, since the
    /// press's serial is that seat's.
    #[must_use]
    pub fn seat_here(&self) -> Option<&wl_seat::WlSeat> {
        self.held
            .iter()
            .find(|h| self.here.as_ref() == Some(&h.pointer.id()))
            .map(|h| &h.seat)
    }

    /// The seat `device` was offered on — a touch point says which screen
    /// it is, and not which seat, and the on-screen keyboard types into the
    /// seat whose finger pressed.
    #[must_use]
    pub fn seat_of(&self, device: &D) -> Option<&wl_seat::WlSeat> {
        self.held
            .iter()
            .find(|h| h.pointer.id() == device.id())
            .map(|h| &h.seat)
    }
}

/// The band a pointer's hover is painted in.
#[must_use]
pub const fn band_colour(many: bool, colour: u8) -> Color {
    if !many {
        return ronoctua_tokens::color::SURFACE_HOVER;
    }
    CURSOR_HOVERS[(colour % 8) as usize]
}

/// Whether a colour is one of the eight cursors' hover colours — what a host
/// turns back into the neutral hover while only one cursor is on its window.
#[must_use]
pub fn is_a_cursor_hover(c: Color) -> bool {
    CURSOR_HOVERS.contains(&c)
}

/// One per KEY, the first to claim it, in the order they came: the bands the
/// pointers light, when two pointers may stand on one thing — a place two
/// cursors share takes the first one's colour, so the picture is the same on
/// every frame.
///
/// THE KEY IS WHAT ONE BAND COVERS, and that is the caller's to say: a row,
/// or — where a row holds lists side by side, each with its own hover (the
/// theme chooser) — a row and a column. Keyed by the row alone, the second
/// pointer on another list of that row lit nothing.
#[must_use]
pub fn first_of_each<K: PartialEq, T>(items: impl IntoIterator<Item = (K, T)>) -> Vec<(K, T)> {
    let mut kept: Vec<(K, T)> = Vec::new();
    for (key, item) in items {
        if !kept.iter().any(|(k, _)| *k == key) {
            kept.push((key, item));
        }
    }
    kept
}

/// The eight cursors' hover colours, in the order of the cursor colours.
const CURSOR_HOVERS: [Color; 8] = [
    ronoctua_tokens::color::CURSOR_HOVER_ONE,
    ronoctua_tokens::color::CURSOR_HOVER_TWO,
    ronoctua_tokens::color::CURSOR_HOVER_THREE,
    ronoctua_tokens::color::CURSOR_HOVER_FOUR,
    ronoctua_tokens::color::CURSOR_HOVER_FIVE,
    ronoctua_tokens::color::CURSOR_HOVER_SIX,
    ronoctua_tokens::color::CURSOR_HOVER_SEVEN,
    ronoctua_tokens::color::CURSOR_HOVER_EIGHT,
];

/// The colour a seat's cursor wears, read the way `ronoctua-pointers` wrote
/// it: from the seat's name to its cursor number, and from the number to the
/// colour chosen for it.
///
/// A seat that is not one of ours — somebody's hand-written `seat work …` —
/// takes the colour of its arrival order, which is at least stable for as
/// long as the door is open.
#[must_use]
pub fn colour_of(name: Option<&str>, arrival: usize) -> u8 {
    let number = match name {
        Some("seat0") => Some(ronoctua_pointers::FIRST),
        Some(n) => n
            .strip_prefix("ronoctua-cursor-")
            .and_then(|k| k.parse::<u16>().ok()),
        None => None,
    };
    match number {
        Some(n) => ronoctua_pointers::prefs::store().read().cursor(n).colour,
        None => u8::try_from(arrival % usize::from(ronoctua_pointers::COLOURS)).unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_cursor_keeps_the_neutral_hover() {
        assert_eq!(band_colour(false, 5), ronoctua_tokens::color::SURFACE_HOVER);
        assert_eq!(
            band_colour(true, 1),
            ronoctua_tokens::color::CURSOR_HOVER_TWO
        );
        assert_eq!(
            band_colour(true, 9),
            ronoctua_tokens::color::CURSOR_HOVER_TWO
        );
    }

    #[test]
    fn a_seat_that_is_not_ours_takes_its_arrival_order() {
        assert_eq!(colour_of(Some("work"), 3), 3);
        assert_eq!(colour_of(None, 10), 2);
    }
}
