//! A pointer moved by something that is not a mouse — the hand gestures' one
//! door to the compositor.
//!
//! # Why this is the one file that names the protocol
//!
//! Invariant 3 keeps this shell to IPC and layer-shell. `zwlr_virtual_pointer_v1`
//! is the owner's approved exception (2026-09-25), of the same kind as
//! `zwp_virtual_keyboard_v1`: a STANDARD Wayland protocol, not a patch. The
//! pinned compositor creates the manager, hides it only from sandboxed
//! clients, and attaches every pointer made through it to the seat the client
//! names — where it moves that seat's cursor like any mouse would, and every
//! window hears it as a mouse. `one-virtual-pointer` keeps every other file
//! from naming it.
//!
//! # Absolute, over the whole desk
//!
//! A finger in front of the screen says WHERE, not how far: `motion_absolute`
//! over an extent of our choosing, which the compositor maps onto the whole
//! output layout when the pointer is mapped to no output. One monitor is the
//! monitor; two are the desk the person sees.
//!
//! # It is never a cursor of its own
//!
//! The compositor names every such device `0:0:wlr_virtual_pointer_v1`, and
//! `ronoctua-pointers` leaves it where it was put instead of giving it a seat
//! (`plan::is_a_virtual_pointer`): a hand moving a cursor nobody else moves
//! would be a second cursor on screen.

use smithay_client_toolkit::dispatch2::Dispatch2;
use smithay_client_toolkit::reexports::client::{
    Connection, Dispatch, Proxy as _, QueueHandle,
    globals::GlobalList,
    protocol::{wl_pointer, wl_seat::WlSeat},
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::{self, ZwlrVirtualPointerManagerV1},
    zwlr_virtual_pointer_v1::{self, ZwlrVirtualPointerV1},
};

/// The user data both objects carry. Neither sends any event.
#[derive(Debug, Clone, Copy, Default)]
pub struct Quiet;

impl<S> Dispatch2<ZwlrVirtualPointerManagerV1, S> for Quiet {
    fn event(
        &self,
        _: &mut S,
        _: &ZwlrVirtualPointerManagerV1,
        _: zwlr_virtual_pointer_manager_v1::Event,
        _: &Connection,
        _: &QueueHandle<S>,
    ) {
    }
}

impl<S> Dispatch2<ZwlrVirtualPointerV1, S> for Quiet {
    fn event(
        &self,
        _: &mut S,
        _: &ZwlrVirtualPointerV1,
        _: zwlr_virtual_pointer_v1::Event,
        _: &Connection,
        _: &QueueHandle<S>,
    ) {
    }
}

/// The resolution of an absolute position: the compositor divides by it.
const EXTENT: u32 = 10_000;
/// `BTN_LEFT` and `BTN_RIGHT`, from `linux/input-event-codes.h`.
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
/// Surface pixels one line of scroll is worth — what a wheel's detent sends.
const PER_LINE: f64 = 15.0;

/// A button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// The primary.
    Left,
    /// The secondary.
    Right,
}

/// The manager, and the one pointer in use.
#[derive(Debug)]
pub struct Pointer {
    manager: ZwlrVirtualPointerManagerV1,
    made: Option<(WlSeat, ZwlrVirtualPointerV1)>,
    /// The buttons held down now: let go of before the pointer goes.
    held: Vec<u32>,
    began: std::time::Instant,
}

impl Pointer {
    /// Binds the manager, or answers `None` where the compositor has none —
    /// absence is not an error, and the gestures say on screen that they
    /// move nothing.
    pub fn bind<D>(globals: &GlobalList, qh: &QueueHandle<D>) -> Option<Self>
    where
        D: Dispatch<ZwlrVirtualPointerManagerV1, Quiet> + 'static,
    {
        match globals.bind::<ZwlrVirtualPointerManagerV1, D, Quiet>(qh, 1..=2, Quiet) {
            Ok(manager) => Some(Self {
                manager,
                made: None,
                held: Vec::new(),
                began: std::time::Instant::now(),
            }),
            Err(e) => {
                tracing::info!(error = %e, "no virtual pointer manager; gestures move no pointer");
                None
            }
        }
    }

    /// Makes sure the pointer of `seat` exists.
    pub fn ready<D>(&mut self, qh: &QueueHandle<D>, seat: &WlSeat)
    where
        D: Dispatch<ZwlrVirtualPointerV1, Quiet> + 'static,
    {
        if self.made.as_ref().is_some_and(|(s, _)| s.id() != seat.id()) {
            self.let_go();
            if let Some((_, p)) = self.made.take() {
                p.destroy();
            }
        }
        if self.made.is_none() {
            tracing::info!(
                seat = seat.id().protocol_id(),
                "the gestures' pointer is made"
            );
            let pointer = self.manager.create_virtual_pointer(Some(seat), qh, Quiet);
            self.made = Some((seat.clone(), pointer));
        }
    }

    /// Moves to `x`, `y` in `0..1` of the desk.
    pub fn to(&mut self, x: f32, y: f32) {
        let time = self.now();
        let Some((_, p)) = &self.made else { return };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let at = |v: f32| (v.clamp(0.0, 1.0) * EXTENT as f32).round() as u32;
        p.motion_absolute(time, at(x), at(y), EXTENT, EXTENT);
        p.frame();
    }

    /// A button down or up.
    pub fn button(&mut self, which: Button, down: bool) {
        let time = self.now();
        let Some((_, p)) = &self.made else { return };
        let code = match which {
            Button::Left => BTN_LEFT,
            Button::Right => BTN_RIGHT,
        };
        if down {
            if !self.held.contains(&code) {
                self.held.push(code);
            }
        } else {
            self.held.retain(|c| *c != code);
        }
        let state = if down {
            wl_pointer::ButtonState::Pressed
        } else {
            wl_pointer::ButtonState::Released
        };
        p.button(time, code, state);
        p.frame();
    }

    /// `lines` of vertical scroll, positive downward, as a finger on a
    /// touchpad sends it: continuous, not in detents.
    pub fn scroll(&mut self, lines: f32) {
        let time = self.now();
        let Some((_, p)) = &self.made else { return };
        p.axis_source(wl_pointer::AxisSource::Finger);
        p.axis(
            time,
            wl_pointer::Axis::VerticalScroll,
            f64::from(lines) * PER_LINE,
        );
        p.frame();
    }

    /// Lets go of every button held — before the pointer goes, or gestures
    /// are turned off. A button left down is a drag that never ends.
    pub fn let_go(&mut self) {
        let time = self.now();
        let Some((_, p)) = &self.made else {
            self.held.clear();
            return;
        };
        for code in std::mem::take(&mut self.held) {
            p.button(time, code, wl_pointer::ButtonState::Released);
        }
        p.frame();
    }

    /// Milliseconds, from a clock of our own: the protocol only asks that they
    /// go forward.
    fn now(&self) -> u32 {
        u32::try_from(self.began.elapsed().as_millis() % u128::from(u32::MAX)).unwrap_or(0)
    }
}
