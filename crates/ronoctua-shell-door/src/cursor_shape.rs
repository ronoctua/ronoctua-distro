//! The pointer's image over our surfaces — the one door to the compositor for
//! it.
//!
//! # Why a surface has to say it
//!
//! A Wayland client that never sets a cursor leaves the image UNDEFINED over
//! its surface, and this compositor keeps whatever it drew last. Crossing a
//! window's border draws the resize arrows; entering the window then kept
//! them, for as long as the pointer stayed inside — the owner saw it from the
//! first day. So every surface of ours asks for the ordinary arrow the moment
//! a pointer enters it.
//!
//! # Why a SHAPE and not an image
//!
//! Because the image belongs to the SEAT: every cursor wears its own recoloured
//! theme (`ronoctua-pointer-N`, `crate::seats`), and only the compositor knows
//! which seat wears which. `wp_cursor_shape_v1` names a shape and lets the
//! compositor draw it in that seat's theme; `wl_pointer.set_cursor` would hand
//! it a picture of ours, in one theme for every hand. It is the owner's
//! approved exception to invariant 3 (2026-09-25), a STANDARD Wayland protocol
//! of the same kind as `zwp_virtual_keyboard_v1`, and `one-cursor-shape` keeps
//! every other file from naming it. A compositor without it costs the arrow,
//! never the surface.

use smithay_client_toolkit::globals::GlobalData;
use smithay_client_toolkit::reexports::client::{
    Dispatch, QueueHandle,
    globals::GlobalList,
    protocol::{wl_pointer::WlPointer, wl_seat::WlSeat},
};
use smithay_client_toolkit::reexports::protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1},
    wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
};
use smithay_client_toolkit::seat::pointer::cursor_shape::CursorShapeManager;

/// What a surface of ours wants the pointer to look like — OUR word for it,
/// so no other file names the protocol (`one-cursor-shape`).
///
/// Only the shapes a surface of ours has a reason to ask for: the arrow
/// everywhere, the three resize arrows over the resizer's grips (a control
/// that resizes says so under the hand, as every border does), the I-beam
/// over text a sweep selects, the hand over a link a click opens — and no
/// pointer at all while somebody types into a grid of cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Look {
    /// The ordinary arrow.
    #[default]
    Arrow,
    /// Resizing left and right: a vertical edge.
    Across,
    /// Resizing up and down: a horizontal edge.
    UpDown,
    /// Resizing both, from a bottom-right corner.
    Corner,
    /// The I-beam: text, which a held button sweeps.
    Text,
    /// The hand: a link, which a click opens.
    Pointer,
    /// NO POINTER: hidden while somebody types, so it does not sit on the
    /// letters being typed — `wl_pointer.set_cursor` with no surface. Only
    /// where the compositor offers shapes: a pointer hidden with no shape to
    /// bring it back would stay hidden.
    Hidden,
}

impl Look {
    /// The protocol's shape for it — none for [`Look::Hidden`], which is no
    /// shape at all.
    const fn shape(self) -> Option<Shape> {
        match self {
            Self::Arrow => Some(Shape::Default),
            Self::Across => Some(Shape::EwResize),
            Self::UpDown => Some(Shape::NsResize),
            Self::Corner => Some(Shape::NwseResize),
            Self::Text => Some(Shape::Text),
            Self::Pointer => Some(Shape::Pointer),
            Self::Hidden => None,
        }
    }
}

/// One seat's pointer: the pointer itself (what hides it), its device, the
/// serial of its last enter — the one the compositor accepts a shape with —
/// and the look it was last given.
#[derive(Debug)]
struct Worn {
    seat: WlSeat,
    pointer: WlPointer,
    device: WpCursorShapeDeviceV1,
    serial: Option<u32>,
    look: Look,
}

/// The manager, when the compositor offers one, and one device per seat's
/// pointer.
#[derive(Debug, Default)]
pub struct Shapes {
    manager: Option<CursorShapeManager>,
    devices: Vec<Worn>,
}

impl Shapes {
    /// Binds the manager. Its absence is not an error: the pointer keeps
    /// whatever image the compositor gives it.
    pub fn bind<S>(globals: &GlobalList, qh: &QueueHandle<S>) -> Self
    where
        S: Dispatch<WpCursorShapeManagerV1, GlobalData> + 'static,
    {
        let manager = CursorShapeManager::bind(globals, qh)
            .map_err(
                |e| tracing::debug!(error = %e, "no cursor shapes; the arrow is the compositor's"),
            )
            .ok();
        Self {
            manager,
            devices: Vec::new(),
        }
    }

    /// A seat's pointer arrived.
    pub fn took<S>(&mut self, seat: &WlSeat, pointer: &WlPointer, qh: &QueueHandle<S>)
    where
        S: Dispatch<WpCursorShapeDeviceV1, GlobalData> + 'static,
    {
        self.lost(seat);
        if let Some(m) = self.manager.as_ref() {
            self.devices.push(Worn {
                seat: seat.clone(),
                pointer: pointer.clone(),
                device: m.get_shape_device(pointer, qh),
                serial: None,
                look: Look::Arrow,
            });
        }
    }

    /// A seat's pointer went away.
    pub fn lost(&mut self, seat: &WlSeat) {
        self.devices.retain(|w| {
            let keep = &w.seat != seat;
            if !keep {
                w.device.destroy();
            }
            keep
        });
    }

    /// A pointer of this seat entered one of our surfaces: the plain arrow,
    /// with the serial of that enter — the compositor refuses any other.
    pub fn entered(&mut self, seat: Option<&WlSeat>, serial: u32) {
        let Some(seat) = seat else {
            return;
        };
        if let Some(w) = self.devices.iter_mut().find(|w| &w.seat == seat) {
            w.device.set_shape(serial, Shape::Default);
            w.serial = Some(serial);
            w.look = Look::Arrow;
        }
    }

    /// The surface under this seat's pointer wants `look` where the pointer
    /// is now: asked only when it CHANGES, with the serial of the enter that
    /// brought the pointer in. Before any enter there is no serial, and
    /// nothing is asked.
    pub fn wear(&mut self, seat: Option<&WlSeat>, look: Look) {
        let Some(seat) = seat else {
            return;
        };
        if let Some(w) = self.devices.iter_mut().find(|w| &w.seat == seat)
            && w.look != look
            && let Some(serial) = w.serial
        {
            match look.shape() {
                Some(shape) => w.device.set_shape(serial, shape),
                // NO SURFACE is no pointer, until the next shape is asked.
                None => w.pointer.set_cursor(serial, None, 0, 0),
            }
            w.look = look;
        }
    }
}

/// The serials of every enter in a frame.
pub fn enters(
    events: &[smithay_client_toolkit::seat::pointer::PointerEvent],
) -> impl Iterator<Item = u32> + '_ {
    use smithay_client_toolkit::seat::pointer::PointerEventKind;
    events.iter().filter_map(|e| match e.kind {
        PointerEventKind::Enter { serial } => Some(serial),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a look is asked of the compositor as another shape — the
    /// I-beam over text drawn as the arrow, the hand over a link as the
    /// I-beam — or the hidden pointer is asked as a shape, which would draw
    /// one instead of hiding it.
    #[test]
    fn every_look_is_its_own_shape_and_hidden_is_none() {
        let looks = [
            Look::Arrow,
            Look::Across,
            Look::UpDown,
            Look::Corner,
            Look::Text,
            Look::Pointer,
        ];
        let shapes: Vec<Shape> = looks.iter().filter_map(|l| l.shape()).collect();
        assert_eq!(shapes.len(), looks.len(), "a visible look has no shape");
        for (i, a) in shapes.iter().enumerate() {
            assert!(
                !shapes[i + 1..].contains(a),
                "{:?} is asked twice",
                looks[i]
            );
        }
        assert_eq!(Look::Text.shape(), Some(Shape::Text));
        assert_eq!(Look::Pointer.shape(), Some(Shape::Pointer));
        assert_eq!(Look::Hidden.shape(), None);
    }
}
