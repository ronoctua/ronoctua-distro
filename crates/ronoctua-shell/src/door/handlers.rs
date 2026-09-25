//! Every Wayland handler of every door, written ONCE for [`Door`].
//!
//! What was nineteen copies of each of these is one generic `impl` per
//! handler, and SCTK's `delegate_dispatch2!` accepts the generic form — so a
//! door written tomorrow hears every seat, every keyboard (and its repeat),
//! every pointer, every finger and every change of scale without a line of
//! its own. Where a window needs to do something at one of these moments it
//! is a method of [`Content`] with a default, never a second handler.

use super::{Content, Door, LEFT_BUTTON, RIGHT_BUTTON};
use smithay_client_toolkit::{
    compositor::CompositorHandler,
    output::{OutputHandler, OutputState},
    reexports::calloop,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        touch::TouchHandler,
    },
    shell::{
        WaylandSurface as _,
        wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
        xdg::window::{Window, WindowConfigure, WindowHandler},
    },
};
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface, wl_touch},
};

impl<C: Content> Door<C> {
    /// A size from the compositor: kept as ASKED, then made what the buffer
    /// can show — see [`crate::hidpi`].
    fn configured(&mut self, width: Option<u32>, height: Option<u32>) {
        // WHAT THE COMPOSITOR SAID IS KEPT SEPARATELY, and taken from the
        // CONFIGURE and nothing else: built out of the door's own fields it
        // would be the size `resized` last answered, which can only shrink.
        // An axis left to the window keeps what it had, and the window's own
        // default before it had anything.
        let (dw, dh) = self.content.default_size();
        let before = self.asked;
        self.asked = (
            width.unwrap_or(if before.0 == 0 { dw } else { before.0 }),
            height.unwrap_or(if before.1 == 0 { dh } else { before.1 }),
        );
        self.take_the_size();
    }

    /// Lays the door out at the size it was ASKED — or at its own, when the
    /// window owns one (`Content::own_size`).
    ///
    /// `asked` STAYS the compositor's number: a window leaving its own size
    /// lays out at it again, without waiting for a configure that may not
    /// come.
    pub(super) fn take_the_size(&mut self) {
        let asked = self.content.own_size().unwrap_or(self.asked);
        self.ui.width = asked.0;
        self.ui.height = asked.1;
        let scale = self.ui.scale;
        if let Some(g) = self.gpu.as_mut() {
            let (shown_w, shown_h) = crate::hidpi::resized(g, asked, scale);
            self.ui.width = shown_w;
            self.ui.height = shown_h;
        }
        self.ui.recount();
        self.content.resized(&mut self.ui);
        self.ui.ask_paint();
    }

    /// What a finger did, played as a pointer — the door's own pointer code,
    /// with the fingers' own hand.
    fn finger_did(&mut self, outs: &[super::touch::Out]) {
        use super::touch::{Button, Out};
        let button_of = |b: Button| match b {
            Button::Left => LEFT_BUTTON,
            Button::Right => RIGHT_BUTTON,
        };
        let surface = self.shell.wl_surface().clone();
        let events: Vec<PointerEvent> = outs
            .iter()
            .map(|o| match *o {
                Out::Motion(position) => PointerEvent {
                    surface: surface.clone(),
                    position,
                    kind: PointerEventKind::Motion { time: 0 },
                },
                Out::Press(b, position) => PointerEvent {
                    surface: surface.clone(),
                    position,
                    kind: PointerEventKind::Press {
                        time: 0,
                        button: button_of(b),
                        serial: 0,
                    },
                },
                Out::Release(b, position) => PointerEvent {
                    surface: surface.clone(),
                    position,
                    kind: PointerEventKind::Release {
                        time: 0,
                        button: button_of(b),
                        serial: 0,
                    },
                },
            })
            .collect();
        if events.is_empty() {
            return;
        }
        self.as_the_finger(&events);
    }

    /// Runs the pointer code with the fingers' hand `here`.
    fn as_the_finger(&mut self, events: &[PointerEvent]) {
        self.fingers.swap_hand(&mut self.here);
        self.pointer_frame_here(events);
        self.fingers.swap_hand(&mut self.here);
    }

    /// Arms the clock that turns finger `id`, landed as `landing`, into a
    /// right click if it is still there and still still.
    fn arm_long_press(&self, id: i32, landing: u64) {
        let armed = self.handle.insert_source(
            calloop::timer::Timer::from_duration(super::touch::LONG_PRESS),
            move |_, (), door: &mut Self| {
                let outs = door.fingers.held_long(id, landing);
                if !outs.is_empty() {
                    door.finger_did(&outs);
                }
                calloop::timer::TimeoutAction::Drop
            },
        );
        if let Err(e) = armed {
            tracing::warn!(error = %e, "no long press: the clock could not be armed");
        }
    }
}

impl<C: Content> WindowHandler for Door<C> {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        if self.content.closing(&mut self.ui) {
            self.ui.quit = true;
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &Window,
        cfg: WindowConfigure,
        _: u32,
    ) {
        self.content.window_configured(&mut self.ui, &cfg);
        self.configured(
            cfg.new_size.0.map(std::num::NonZeroU32::get),
            cfg.new_size.1.map(std::num::NonZeroU32::get),
        );
    }
}

impl<C: Content> LayerShellHandler for Door<C> {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.ui.quit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        cfg: LayerSurfaceConfigure,
        _: u32,
    ) {
        // A ZERO IS "keep yours": a column's width is its own, and its height
        // is what the compositor hands it.
        self.configured(
            Some(cfg.new_size.0).filter(|w| *w != 0),
            Some(cfg.new_size.1).filter(|h| *h != 0),
        );
        layer.commit();
    }
}

impl<C: Content> SeatHandler for Door<C> {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        let name = self.seats.info(&seat).and_then(|i| i.name);
        if cap == Capability::Keyboard {
            // A KEYBOARD THAT REPEATS — see `Door::handle`. The rate and the
            // delay are the COMPOSITOR's; nothing here invents either.
            match self.seats.get_keyboard_with_repeat(
                qh,
                &seat,
                None,
                self.handle.clone(),
                Box::new(|door: &mut Self, kb, event| {
                    door.keyboards.switch_to(kb, &mut door.mods);
                    let mods = door.mods;
                    door.ui.repeating = true;
                    door.typed(&event, mods);
                    door.ui.repeating = false;
                }),
            ) {
                // EVERY KEYBOARD, and not only the first: one per seat, and in
                // Independent mode one per cursor.
                Ok(k) => self.keyboards.took(&seat, k, name.as_deref()),
                Err(e) => tracing::warn!(error = %e, "no keyboard; there is no typing"),
            }
        }
        if cap == Capability::Pointer {
            match self.seats.get_pointer(qh, &seat) {
                // EVERY POINTER, and not only the first: one per cursor.
                Ok(p) => {
                    self.shapes.took(&seat, &p, qh);
                    self.pointers.took(&seat, p, name.as_deref());
                }
                Err(e) => tracing::warn!(error = %e, "no pointer; keyboard only"),
            }
        }
        if cap == Capability::Touch {
            // EVERY TOUCH SCREEN. Binding it turns the compositor's own
            // emulation off for this surface, which is why the finger is then
            // played here — tap, drag, and a long press that is the right
            // button a finger has none of (`door::touch`).
            match self.seats.get_touch(qh, &seat) {
                Ok(t) => self.touches.took(&seat, t, name.as_deref()),
                Err(e) => tracing::warn!(error = %e, "no touch; the pointer only"),
            }
        }
        C::seat_capability(self, qh, &seat);
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard {
            self.keyboards.lost(&seat, &mut self.mods);
        }
        if cap == Capability::Pointer {
            self.shapes.lost(&seat);
            self.pointers.lost(&seat, &mut self.here);
        }
        if cap == Capability::Touch {
            self.touches.lost(&seat, &mut ());
            self.fingers.cancel();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl<C: Content> KeyboardHandler for Door<C> {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        self.keyboards.switch_to(kb, &mut self.mods);
        self.ui.keyboard_seat = self.keyboards.colour_number_here();
        self.content.keyboard_entered(&mut self.ui);
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        self.keyboards.switch_to(kb, &mut self.mods);
        // A MODIFIER HELD WHEN THE FOCUS WENT IS A MODIFIER NOBODY WILL SEE
        // COME UP — the release goes to whoever has the keyboard now. Kept, a
        // `Ctrl` held while leaving would still be held on the way back, and
        // an arrow would be read as a chord.
        self.mods = Modifiers::default();
        self.ui.hinted.let_everything_go();
        self.ui.keyboard_seat = self.keyboards.colour_number_here();
        self.content.keyboard_left(&mut self.ui);
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.keyboards.switch_to(kb, &mut self.mods);
        // THE HEADER'S OWN KEYS FIRST — the right `Ctrl` and its pair, and the
        // `Esc` that shuts an open drawer before it closes the window.
        if self.header_took(event.keysym, true) {
            self.ui.ask_paint();
            return;
        }
        let mods = self.mods;
        self.typed(&event, mods);
    }

    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.press_key(conn, qh, kb, serial, event);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.keyboards.switch_to(kb, &mut self.mods);
        // THE MODIFIER IS LET GO HERE: xkb merges both Ctrls into one bit, so
        // the modifier event cannot say which of them came up.
        self.header_took(event.keysym, false);
        self.ui.keyboard_seat = self.keyboards.colour_number_here();
        self.content.released(&mut self.ui, &event);
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.keyboards.switch_to(kb, &mut self.mods);
        self.mods = modifiers;
    }
}

impl<C: Content> PointerHandler for Door<C> {
    /// EVERY CURSOR HAS ITS OWN STATE: the pointer that spoke is made `here`
    /// first, so a drag, a hover or a wheel's leftover never crosses from
    /// one cursor to another. See `crate::seats`.
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for serial in crate::cursor_shape::enters(events) {
            self.shapes.entered(self.pointers.seat_of(pointer), serial);
        }
        self.pointers.switch_to(pointer, &mut self.here);
        self.pointer_frame_here(events);
    }
}

impl<C: Content> TouchHandler for Door<C> {
    fn down(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: u32,
        _: u32,
        _: wl_surface::WlSurface,
        id: i32,
        position: (f64, f64),
    ) {
        let (landing, outs) = self.fingers.down(id, position);
        self.finger_did(&outs);
        self.arm_long_press(id, landing);
    }

    fn up(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        serial: u32,
        _: u32,
        id: i32,
    ) {
        let outs = self.fingers.up(id);
        self.finger_did(&outs);
        // AND THE HAND LEAVES WITH THE FINGER: a touch screen has no pointer
        // resting anywhere, so nothing is left lit after it.
        let events = [PointerEvent {
            surface: self.shell.wl_surface().clone(),
            position: (0.0, 0.0),
            kind: PointerEventKind::Leave { serial },
        }];
        self.as_the_finger(&events);
    }

    fn motion(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: u32,
        id: i32,
        position: (f64, f64),
    ) {
        let outs = self.fingers.motion(id, position);
        self.finger_did(&outs);
    }

    fn shape(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: i32,
        _: f64,
        _: f64,
    ) {
    }

    fn orientation(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_touch::WlTouch,
        _: i32,
        _: f64,
    ) {
    }

    fn cancel(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_touch::WlTouch) {
        self.fingers.cancel();
    }
}

impl<C: Content> CompositorHandler for Door<C> {
    /// The window moved to a screen of another scale — or was told its first.
    /// The rule and the measurement are in [`crate::hidpi`].
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        // THE SURFACE IS NEEDED to ask its VERSION: it is told its scale
        // inside the frame — see `crate::hidpi::told_before_present`.
        surface: &wl_surface::WlSurface,
        factor: i32,
    ) {
        let asked = self.content.own_size().unwrap_or(self.asked);
        if let Some((w, h)) = crate::hidpi::followed(
            &mut self.ui.scale,
            factor,
            surface,
            self.gpu.as_mut(),
            self.text.as_mut(),
            asked,
        ) {
            // AND THE DOOR KEEPS WHAT THE BUFFER CAN SHOW.
            self.ui.width = w;
            self.ui.height = h;
            self.ui.recount();
            self.content.resized(&mut self.ui);
            self.ui.ask_paint();
        }
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.content.frame_done(&mut self.ui);
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl<C: Content> OutputHandler for Door<C> {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.content.outputs_changed(&mut self.ui);
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.content.outputs_changed(&mut self.ui);
    }
}

impl<C: Content> ProvidesRegistryState for Door<C> {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(@<C: Content> Door<C>);
smithay_client_toolkit::delegate_dispatch2!(@<C: Content> Door<C>);
