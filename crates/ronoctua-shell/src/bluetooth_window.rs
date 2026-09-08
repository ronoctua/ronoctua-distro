//! The bluetooth view's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a twelve-line ceiling (`space.expansion`,
//! invariant 14), and one device carries thirty facts and a row per service:
//! a glance at what is connected fits the footer, reading a device whole does
//! not. So this exists — and everything it knows lives in
//! [`crate::bluetooth::View`], which the footer draws from too. What this
//! module holds is a window, a GPU and the events of both; what it decides is
//! nothing. It is the arrangement `power_window.rs` and `files_window.rs`
//! share, and the same header.
//!
//! # Where its data comes from
//!
//! From BlueZ, on threads of its own (`ronoctua_bluetooth::watch`): a window
//! has no clock, and the bus says when something changed. Orders go to the
//! daemon directly — connecting, pairing, forgetting are the same call from
//! any process — EXCEPT a pairing QUESTION, which is answered in the footer:
//! the agent is registered by the footer's own connection, because it is the
//! process that outlives this window.

use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::calloop::{self, EventLoop, channel},
    reexports::calloop_wayland_source::WaylandSource,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface as _,
        xdg::{
            XdgShell,
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
        },
    },
};
use wayland_client::{
    Connection, Proxy as _, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.bluetooth";

/// `BTN_LEFT`, from `linux/input-event-codes.h`.
const LEFT_BUTTON: u32 = 0x110;

/// The first line of the list. Above it: the hint line, and nothing else —
/// the arrangement the notification window has, for the reason it gives.
const FIRST_OF_LIST: u16 = 1;

/// The margin inside the window, in cells — see `chrome::FIELDS_LEAD`.
const CONTENT_MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// Opens the bluetooth view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<BluetoothWindow> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;

    let surface_of = compositor.create_surface(&qh);
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    let texts = std::rc::Rc::new(ronoctua_i18n::Texts::from_env());
    window.set_title(format!(
        "*{}*",
        texts.t(ronoctua_i18n::key::BLUETOOTH_TITLE)
    ));
    window.set_app_id(APP_ID);
    window.commit();

    let mut cycle: EventLoop<BluetoothWindow> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();

    // THE THEME'S FILE, watched, so a choice made in the footer recolours this
    // window without it being reopened.
    let (theme_sender, theme_changed) = channel::channel::<()>();
    let _theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    handle
        .insert_source(theme_changed, |event, (), state: &mut BluetoothWindow| {
            if let channel::Event::Msg(()) = event
                && crate::theme::reload()
            {
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the theme channel: {e}"))?;

    // AND THE SIZE OF THE TEXT, watched for the same reason and by the same
    // means. A size chosen anywhere moves this window without it being
    // reopened — and without it this door would sit beside the footer at a
    // different size, which is the photograph that made this system have one
    // size at all.
    let (size_sender, size_changed) = channel::channel::<()>();
    let _size_watch = crate::toolkits::watch(move || size_sender.send(()).is_ok());
    handle
        .insert_source(size_changed, |event, (), state: &mut BluetoothWindow| {
            if let channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                state.text_size_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    // THE DAEMON, on threads of this process — the bus is the clock.
    let (bt_sender, bt_events) = channel::channel::<ronoctua_bluetooth::Event>();
    // FALSE: THE FOOTER OWNS THE PAIRING AGENT. Only one door may register
    // one — `RequestDefaultAgent` gives the questions to the last caller, so
    // a window that registered would take them away from the footer and hand
    // them back when it closed. See `ronoctua_bluetooth::watch`.
    let radio = match ronoctua_bluetooth::watch(
        move |e| {
            let _ = bt_sender.send(e);
        },
        false,
    ) {
        Ok(w) => Some(w.hand),
        Err(e) => {
            tracing::warn!(error = %e, "no bluetooth; the window opens with nothing to read");
            None
        }
    };
    handle
        .insert_source(bt_events, |event, (), state: &mut BluetoothWindow| {
            if let channel::Event::Msg(e) = event {
                state.bluetooth_event(e);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the bluetooth channel: {e}"))?;

    let mut state = BluetoothWindow {
        conn: conn.clone(),
        handle: handle.clone(),
        _theme_watch,
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboard: None,
        pointer: None,
        window,
        width: 0,
        height: 0,
        asked: (0, 0),
        // ONE TO ONE UNTIL A SCREEN SAYS OTHERWISE: the surface exists before
        // the compositor has said which output it is on, and
        // `scale_factor_changed` is what says so.
        scale: 1,
        scale_told: 1,
        columns: 80,
        lines: 24,
        mods: Modifiers::default(),
        quit: false,
        owed: false,
        gpu: None,
        text: None,
        quads: None,
        dragging: false,
        lines_owed: 0.0,
        view: crate::bluetooth::View::fresh(),
        scene: ronoctua_bluetooth::Scene::default(),
        radio,
        texts,
        row_under_pointer: None,
        offset: 0,
        message: None,
    };

    WaylandSource::new(conn, queue)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |e| {
            e.settle();
            if e.quit {
                signal.stop();
            }
        })
        .context("event loop")?;
    Ok(())
}

/// The window.
struct BluetoothWindow {
    conn: Connection,
    /// The loop, for the ONE thing that needs it here: a keyboard that
    /// REPEATS. SCTK only generates repeat events for a keyboard made with
    /// `get_keyboard_with_repeat`, and that wants a `LoopHandle` — so this is
    /// carried rather than reached for. Without it `repeat_key` below is dead
    /// code and holding an arrow moves exactly one row, which is the trap
    /// AGENTS.md records and which nine windows inherited.
    handle: calloop::LoopHandle<'static, Self>,
    _theme_watch: Option<ronoctua_files::notice::Notice>,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    window: Window,
    /// The window's size in LOGICAL pixels — what the compositor configured
    /// it to, what the columns are counted from, what the pointer arrives in.
    width: u32,
    /// See the field above.
    height: u32,
    /// The logical size the COMPOSITOR configured, before any clamp.
    ///
    /// `width`/`height` are what the BUFFER can show — which is what the
    /// columns, the bars and the hit tests are laid out against; this is what
    /// the GPU is asked for. They are the same pair on every machine anybody
    /// has, and they come apart exactly at the GPU's texture ceiling. Keeping
    /// only one of them is a ratchet: a window clamped once at scale 3 would
    /// ask for the size it was cut down to and could never grow back. See
    /// [`crate::hidpi`].
    asked: (u32, u32),
    /// How many physical pixels one logical pixel is on the screen this
    /// window is on — see [`crate::hidpi`], where the rule and its
    /// measurement live.
    scale: i32,
    /// What the SURFACE has been told, which lags `scale` by one frame.
    ///
    /// `set_buffer_scale` has to travel in the commit that carries a buffer of
    /// the matching size, and the only such commit is the present inside
    /// `Gpu::frame` — see [`crate::hidpi::told_before_present`].
    scale_told: i32,
    columns: u16,
    lines: u16,
    /// The modifiers held, kept WHOLE.
    ///
    /// `KeyEvent` carries no modifier state, and this view needs one: the
    /// bare sideways arrows cross the columns and `S-<arrow>` is more and
    /// less, so a press that could not see shift would answer both with the
    /// same thing. Cleared on losing focus, for the reason the file manager
    /// records — a state that only updates with focus cannot keep holding
    /// without it.
    mods: Modifiers,
    quit: bool,
    /// Whether a frame is owed — see `notifications_window`'s field of the
    /// same name for why nothing here paints straight away.
    owed: bool,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    /// The one rectangle this window draws: the list's scrollbar.
    ///
    /// It had none — see `chrome::list_bar` for what that cost — and the wheel
    /// was the only way down a list that does not fit.
    quads: Option<ronoctua_render::quad::Quads>,
    /// Is the pointer dragging the bar?
    ///
    /// Held, because a drag is three events: the press that grabs it, every
    /// motion until the release, and the release. Without the flag the list
    /// would only move where the pointer was PRESSED.
    dragging: bool,
    /// How many LINES the wheel and the touchpad have asked for and not yet
    /// been paid.
    ///
    /// LINES, and the unit is in the name on purpose: the footer's own
    /// accumulator holds DETENTS, because one of its two modes turns a page per
    /// detent. Two fields of one crate holding two units under one name is the
    /// `ink-is-not-a-count` confusion, and it costs a factor of three.
    ///
    /// A touchpad sends fractions of a line and a mouse sends a detent as
    /// fifteen pixels, which is also a fraction. Spending only the whole part
    /// and keeping the rest is what makes a slow gesture move the list at the
    /// speed of the finger instead of standing still and then jumping — the
    /// reason is written over `surface::wheel_arrived`.
    lines_owed: f64,
    view: crate::bluetooth::View,
    /// The last reading, from this process's own threads.
    scene: ronoctua_bluetooth::Scene,
    /// The orders thread, when bluetoothd was there.
    radio: Option<ronoctua_bluetooth::Hand>,
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    row_under_pointer: Option<usize>,
    /// The first visible row. A window has no page: it scrolls.
    offset: usize,
    message: Option<String>,
}

impl BluetoothWindow {
    fn bluetooth_event(&mut self, e: ronoctua_bluetooth::Event) {
        use ronoctua_bluetooth::Event;
        match e {
            Event::Scene(scene) => {
                // THE VIEW FOLLOWS THE NEW ROWS, the footer's reason: a
                // device switched off takes its row away, and a scan brings
                // four in a second.
                self.scene = *scene;
                let rows = self.rows();
                self.view.settle(&rows);
                self.ask_paint();
            }
            Event::Done { ask, refused } => {
                if let Some((why, words)) = refused {
                    self.message = Some(crate::bluetooth_words::refused(
                        &ask,
                        why,
                        &words,
                        &self.texts,
                    ));
                    self.ask_paint();
                }
            }
            // THESE TWO NEVER ARRIVE HERE, and the arm is not dead code: the
            // events are raised inside `agent::serve`, which this door does
            // not call (`serve_agent: false`, just above). A `_ => {}` would
            // go quietly wrong the day this window did register an agent.
            //
            // WHERE THE SENTENCE ACTUALLY COMES FROM is `apply`, below: this
            // window is the one that sent `Ask::Pair`, so it is the one that
            // knows a question is about to be asked somewhere else — and it
            // says so at the moment somebody presses the key, which is when
            // they are looking. Waiting for an event that cannot arrive left
            // `p` in this window doing nothing visible at all.
            Event::Asked(_) => {
                self.message = Some(
                    self.texts
                        .t(ronoctua_i18n::key::BLUETOOTH_ANSWER_IN_THE_FOOTER),
                );
                self.ask_paint();
            }
            Event::Unasked => {
                self.message = None;
                self.ask_paint();
            }
        }
    }

    /// How many rows fit under the hint line.
    const fn room(&self) -> usize {
        self.lines.saturating_sub(FIRST_OF_LIST) as usize
    }

    /// Keeps the cursor on screen, starting to scroll two rows before the edge.
    /// Keeps the offset inside the list, and nothing else.
    ///
    /// # Why this is not `follow`
    ///
    /// Because a paint may not move the list. `follow` slaves the offset to
    /// the CURSOR, and calling it from `paint` undid, on the same frame, every
    /// offset the pointer had just set: a drag of the scrollbar and a turn of
    /// the wheel both wrote an offset and then watched the paint put it back
    /// under the cursor — which on a freshly-opened window (cursor on row 1)
    /// means back to the top, every time. The bar looked like a bar that does
    /// not work, which is exactly the report it was built to answer.
    ///
    /// The footer never made this mistake: `Footer::follow_cursor` is called
    /// from twenty places and every one of them is an EVENT.
    ///
    /// What a paint may do is keep the offset inside a list that changed size
    /// under it — the watch thread brings a new reading while nobody is
    /// touching anything — and honour an anchor the view asked for.
    fn settle_offset(&mut self, rows: usize) {
        let room = self.room();
        if room == 0 {
            return;
        }

        self.offset = self.offset.min(rows.saturating_sub(room));
    }

    fn follow(&mut self, rows: usize) {
        const MARGIN: usize = 2;
        let room = self.room();
        if room == 0 {
            return;
        }
        let cursor = self.view.cursor();
        if cursor < self.offset + MARGIN {
            self.offset = cursor.saturating_sub(MARGIN);
        } else if cursor + MARGIN >= self.offset + room {
            self.offset = cursor + MARGIN + 1 - room;
        }
        self.offset = self.offset.min(rows.saturating_sub(room));
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, errand: crate::bluetooth::Errand) {
        use crate::bluetooth::Errand;
        match errand {
            Errand::Nothing => {}
            // AN INFORMATIVE ROW COPIES ITSELF — see
            // `chrome::information_of`, and `chrome::copied` for the
            // one place the six doors share.
            Errand::Copy(text) => {
                self.message = Some(crate::chrome::copied(&text, &self.texts));
            }
            Errand::Close => {
                self.quit = true;
                return;
            }
            // THE PROFILE IS PIPEWIRE'S, and this door may do it itself:
            // `wpctl` is a process, not a connection somebody else owns, so
            // there is nothing here for the footer to hold.
            Errand::Audio(card, profile) => {
                if let Err(e) = ronoctua_bluetooth::audio::choose(card, profile) {
                    tracing::warn!(error = %e, "the audio profile did not change");
                }
                let scene = std::mem::take(&mut self.scene);
                self.view.look_at_the_device(&scene);
                self.scene = scene;
            }
            // THE VOLUME AND THE MUTE DO NOT RE-READ THE GRAPH.
            //
            // The profile above does, because changing it recreates the node
            // and the view is holding a stale id. A volume changes a number on
            // an object that stays; `pw-dump` is a process and a parse, and
            // running one per arrow press would put a fork under the key
            // somebody is holding down. The view has already moved its own
            // number, which is what the row draws.
            // AND THIS DOOR SENDS IT rather than telling somebody to go to
            // the footer and do it themselves — the control socket exists for
            // exactly this, and the notification manager already forwards
            // through it.
            Errand::Run(id, about) => {
                let line = if about.is_empty() {
                    format!("command {id}")
                } else {
                    format!("command {id} {about}")
                };
                if let Err(e) = crate::leader::send_it(&line) {
                    tracing::warn!(id, error = %e, "the footer did not take the command");
                }
            }
            Errand::Volume(node, percent) => {
                if let Err(e) = ronoctua_bluetooth::audio::set_volume(node, percent) {
                    tracing::warn!(error = %e, "the volume did not change");
                }
            }
            Errand::Mute(node, on) => {
                if let Err(e) = ronoctua_bluetooth::audio::set_mute(node, on) {
                    tracing::warn!(error = %e, "the mute did not change");
                }
            }
            // THE ANSWER IS THE FOOTER'S TOO, and for the agent's reason:
            // it is the footer's connection bluetoothd is asking.
            Errand::Answer(_) | Errand::Type => {
                self.message = Some(
                    self.texts
                        .t(ronoctua_i18n::key::BLUETOOTH_ANSWER_IN_THE_FOOTER),
                );
            }
            Errand::Order(ask) => {
                // AND PAIRING SAYS WHERE THE QUESTION WILL BE. bluetoothd
                // asks the DEFAULT agent, which is the footer's — so from
                // this window `p` starts something whose only visible half
                // happens on another surface. Saying so here is the whole
                // reason `bluetooth-answer-in-the-footer` exists; without it
                // the key looked like it had done nothing.
                if matches!(ask, ronoctua_bluetooth::Ask::Pair(_)) {
                    self.message = Some(
                        self.texts
                            .t(ronoctua_i18n::key::BLUETOOTH_ANSWER_IN_THE_FOOTER),
                    );
                }
                match &self.radio {
                    Some(h) => h.send(ask),
                    None => {
                        self.message = Some(self.texts.t(ronoctua_i18n::key::BLUETOOTH_NO_DAEMON));
                    }
                }
            }
            // A NAME IS TYPED IN THE MINIBUFFER, which this window has not
            // got: the footer's is the one place anything is typed, and
            // saying which is better than a key that looks broken.
            Errand::Rename(_) => {
                self.message = Some(
                    self.texts
                        .t(ronoctua_i18n::key::BLUETOOTH_TYPE_IN_THE_FOOTER),
                );
            }
        }
        self.ask_paint();
    }

    /// The size of the text changed under this door.
    ///
    /// THE SHAPER HAS TO HEAR IT TOO. It was built with a size and keeps it;
    /// moving everything around it — the columns, the cell, every hit test —
    /// and leaving the letters gives a grid at one size with text at another,
    /// which reads as a broken font rather than as a setting that took.
    fn text_size_changed(&mut self) {
        if let Some(t) = self.text.as_mut() {
            t.draw_at(ronoctua_tokens::text::size());
        }
        self.recount();
        self.ask_paint();
    }

    /// Columns and rows from the size in pixels.
    fn recount(&mut self) {
        let size = tokens::text::size();
        let cell = tokens::metrics::cell_width(size);
        let line = tokens::metrics::line_height(size);
        let usable = (self.width as f32) - 2.0 * cell * f32::from(CONTENT_MARGIN);
        self.columns = tokens::metrics::columns(usable.max(0.0), size);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let lines = (self.height as f32 / line).floor().max(0.0) as u16;
        self.lines = lines;
    }

    fn assemble_gpu(&mut self) {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        };
        let (Some(disp), Some(surf)) = (
            std::ptr::NonNull::new(self.conn.backend().display_ptr().cast()),
            std::ptr::NonNull::new(self.window.wl_surface().id().as_ptr().cast()),
        ) else {
            tracing::error!("null Wayland pointers");
            return;
        };
        // SAFETY: `conn` and `window` are fields of `self`, like the `Gpu` — the
        // handles do not outlive what produced them.
        // ONE CALL, not the same one twice: two calls to a pure
        // function are two places the arguments can be edited apart.
        let first = crate::hidpi::physical(self.asked, self.scale);
        #[allow(unsafe_code)]
        let gpu = unsafe {
            ronoctua_render::gpu::Gpu::new(
                RawDisplayHandle::Wayland(WaylandDisplayHandle::new(disp)),
                RawWindowHandle::Wayland(WaylandWindowHandle::new(surf)),
                first.0,
                first.1,
            )
        };
        let Ok(mut g) = gpu else {
            tracing::error!("no usable GPU; the window will not draw");
            return;
        };
        // AND THE FIRST BUFFER IS DIVISIBLE TOO. `Gpu::new` clamps at the
        // GPU's ceiling like `resize` does, and the first frame sends the
        // scale — see `crate::hidpi::resized`.
        let (shown_w, shown_h) = crate::hidpi::resized(&mut g, self.asked, self.scale);
        self.width = shown_w;
        self.height = shown_h;
        // AND THE COLUMNS ARE COUNTED AGAIN. `configure` counts them before the
        // GPU exists, against the size the compositor gave; if the buffer came
        // back smaller than that, the count that is on file is for a picture
        // wider than the one this door draws into.
        self.recount();
        match ronoctua_render::Text::new(
            &g.device,
            &g.queue,
            g.config.format,
            self.scale,
            tokens::text::size(),
        ) {
            Ok(t) => self.text = Some(t),
            Err(e) => tracing::error!(error = %e, "assembling the text failed"),
        }
        // AND THE ONE RECTANGLE THIS WINDOW DRAWS: the list's scrollbar.
        match ronoctua_render::quad::Quads::new(&g.device, g.config.format) {
            Ok(q) => self.quads = Some(q),
            Err(e) => tracing::error!(error = %e, "assembling the quads failed"),
        }
        self.gpu = Some(g);
    }

    /// One key, from a press or from a REPEAT.
    ///
    /// Both doors of every view take this path: SCTK delivers a repeat
    /// through the callback given to `get_keyboard_with_repeat`, which has no
    /// `Connection` and no `QueueHandle` to hand on — so the work cannot live
    /// in `press_key`, and while it did, `repeat_key` was dead code and
    /// holding an arrow moved exactly one row.
    fn typed(&mut self, event: KeyEvent) {
        let Some(press) = press_of(&event, self.mods) else {
            return;
        };
        self.message = None;
        let scene = std::mem::take(&mut self.scene);
        let errand = self.view.key(press, &scene, self.columns, &self.texts);
        self.scene = scene;
        // THE CURSOR MOVED, so the list follows it — here, on the EVENT, and
        // never in the paint. See `settle_offset` for what following from a
        // paint cost.
        let rows = self.rows().len();
        self.follow(rows);
        self.apply(errand);
    }

    /// Asks for a frame. It is drawn when the event queue empties.
    const fn ask_paint(&mut self) {
        self.owed = true;
    }

    /// Draws the owed frame, if one is owed.
    fn settle(&mut self) {
        if std::mem::take(&mut self.owed) {
            self.paint();
        }
    }

    fn rows(&self) -> Vec<crate::bluetooth::Row> {
        self.view.rows(&self.scene, self.columns, &self.texts)
    }

    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let size = tokens::text::size();
        let margin = CONTENT_MARGIN;
        let rows = self.rows();
        self.settle_offset(rows.len());

        let mut spans: Vec<ronoctua_render::Span> = Vec::new();

        // LINE 0: what the last order said, or the keys — the footer's
        // minibuffer, through the other door. `columns` is already net of both
        // margins (`recount`), so the whole of it is the room.
        let room = self.columns;
        if let Some(text) = self.message.clone() {
            spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text: ronoctua_render::layout::elide_end(&text, room),
                column: margin,
                line: 0,
                color: tokens::color::TEXT_SUBTLE_STRONG,
                edge: false,
                foot: false,
            });
        } else {
            spans.extend(crate::chrome::hint_spans(
                &self.view.hint(&self.texts),
                ronoctua_render::Size::Px(size),
                margin,
                0,
                room,
            ));
        }

        // THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
        // draws for these rows.
        for (i, row) in rows.iter().skip(self.offset).take(self.room()).enumerate() {
            let y = FIRST_OF_LIST + u16::try_from(i).unwrap_or(0);
            for f in &row.fields {
                spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::Px(size),
                    text: f.text.clone(),
                    column: margin + f.at,
                    line: y,
                    color: f.color,
                    edge: false,
                    foot: false,
                });
            }
        }

        // THE BAR, from the same `Scroll` every other bar in this system uses.
        let scroll = crate::chrome::list_scroll(rows.len(), self.room(), self.offset);
        #[allow(clippy::cast_precision_loss)]
        let bar =
            crate::chrome::list_bar(scroll, FIRST_OF_LIST, self.width as f32, self.height as f32);
        // THE SURFACE AND THE TWO SCALE NUMBERS, taken before the borrows:
        // `set_buffer_scale` has to travel in the commit `Gpu::frame` makes,
        // and nothing else in this function may borrow `self` by then. See
        // `crate::hidpi::told_before_present`.
        let surface = self.window.wl_surface().clone();
        let scale = self.scale;
        let told = &mut self.scale_told;
        let (Some(gpu), Some(text)) = (self.gpu.as_mut(), self.text.as_mut()) else {
            return;
        };
        // THE WINDOW'S OWN PIXELS — `resized` already made them what the
        // buffer can show, so there is one number and this is it.
        let (logical_w, logical_h) = (self.width, self.height);
        if let Err(e) = text.prepare(
            &gpu.device,
            &gpu.queue,
            gpu.config.width,
            gpu.config.height,
            ronoctua_render::Atlas::Pinned,
            &spans,
        ) {
            tracing::error!(error = %e, "preparing the text failed");
            return;
        }
        if let Some(q) = self.quads.as_mut() {
            q.prepare(
                &gpu.device,
                &gpu.queue,
                logical_w,
                logical_h,
                bar.as_slice(),
            );
        }
        // THE RECTANGLE FIRST, THEN THE TEXT. There is no depth test: whoever
        // draws later is on top, and a bar over a letter would hide it.
        let quads_ref = self.quads.as_ref();
        if let Err(e) = gpu.frame(
            || crate::hidpi::told_before_present(scale, told, &surface),
            |pass| {
                if let Some(q) = quads_ref {
                    q.draw(pass);
                }
                text.draw(ronoctua_render::Atlas::Pinned, pass)
            },
        ) {
            tracing::error!(error = %e, "drawing the frame failed");
            return;
        }
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "the flush failed");
        }
    }

    /// Which drawn row is at height `y`.
    /// Which cell of the line a pointer at `x` is over.
    ///
    /// The twin of `row_at`, in the other axis, and it answers in the SAME
    /// units the view's extents are written in — the content margin is taken
    /// off first, because that is what the columns are counted from.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a pointer inside a window, divided by a cell"
    )]
    fn cell_at(&self, x: f64) -> Option<u16> {
        let cell = f64::from(tokens::metrics::cell_width(tokens::text::size()));
        if cell <= 0.0 {
            return None;
        }
        let inset = cell * f64::from(CONTENT_MARGIN);
        Some(((x - inset).max(0.0) / cell) as u16)
    }

    fn row_at(&self, y: f64) -> Option<usize> {
        let line = f64::from(tokens::metrics::line_height(tokens::text::size()));
        if line <= 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let which = (y / line) as usize;
        which
            .checked_sub(usize::from(FIRST_OF_LIST))
            .map(|i| i + self.offset)
    }
}

/// A `KeyEvent` as the view sees it — the same mapping the footer does.
fn press_of(event: &KeyEvent, mods: Modifiers) -> Option<crate::bluetooth::Press> {
    use crate::bluetooth::Press;
    Some(match event.keysym {
        Keysym::Escape => Press::Escape,
        Keysym::Return | Keysym::KP_Enter => Press::Enter,
        Keysym::Up => Press::Up,
        Keysym::Down => Press::Down,
        // THE THREE THAT ARE NOT LETTERS, and they need naming here.
        //
        // The fallback below matches `event.utf8` against the view's own
        // notation, which works for a bare letter and for nothing else: an
        // arrow and a `TAB` produce no text, so `<right>`, `<left>` and `TAB`
        // resolved to nothing and were unreachable in this door while the
        // footer answered them. The hint line taught all three, in both.
        //
        // AND SHIFT DECIDES WHICH PAIR THE ARROWS ARE. Bare, they cross the
        // columns; held, they are more and less of the row under the cursor.
        // The keysym is `Right` either way — X11 does not shift an arrow —
        // so the state this window keeps is the only thing that can tell
        // them apart.
        Keysym::Right if mods.shift => Press::More,
        Keysym::Left if mods.shift => Press::Less,
        Keysym::Right | Keysym::Left => Press::Cross,
        Keysym::Tab | Keysym::ISO_Left_Tab => Press::Cross,
        // AND THE BARE LETTERS, out of the view's own table — the footer
        // reads it the same way, so the two doors cannot teach different
        // keys. A letter with no press there is a letter that does nothing,
        // and the test beside `bluetooth_words` is what says so.
        _ => {
            let typed = event.utf8.as_deref()?;
            let id = crate::bluetooth::OF_THE_VIEW
                .iter()
                .find(|(notation, _, _)| *notation == typed)
                .map(|(_, id, _)| *id)?;
            crate::bluetooth_words::press_of(id)?
        }
    })
}

impl WindowHandler for BluetoothWindow {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.quit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &Window,
        cfg: WindowConfigure,
        _: u32,
    ) {
        // WHAT THE COMPOSITOR SAID IS KEPT SEPARATELY, because what the
        // buffer can show is what comes back below — see `asked`. It is
        // taken from the CONFIGURE and from nothing else: built out of
        // the door's own fields it would be the size `resized` last
        // answered, which can only ever shrink.
        self.asked = (
            cfg.new_size.0.map_or(900, std::num::NonZeroU32::get),
            cfg.new_size.1.map_or(600, std::num::NonZeroU32::get),
        );
        self.width = self.asked.0;
        self.height = self.asked.1;
        let asked = self.asked;
        let scale = self.scale;
        if let Some(g) = self.gpu.as_mut() {
            // AND THE WINDOW KEEPS WHAT THE BUFFER CAN SHOW. Below the GPU's
            // ceiling that is exactly what the compositor configured; above
            // it, laying out against the bigger number would put the columns
            // and the bars in a space the buffer does not have.
            let (shown_w, shown_h) = crate::hidpi::resized(g, asked, scale);
            self.width = shown_w;
            self.height = shown_h;
        }
        self.recount();
        self.ask_paint();
    }
}

impl SeatHandler for BluetoothWindow {
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
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            // A KEYBOARD THAT REPEATS — see the `handle` field. The rate and
            // the delay are the COMPOSITOR's, arriving as `RepeatInfo`;
            // nothing here invents either.
            match self.seats.get_keyboard_with_repeat(
                qh,
                &seat,
                None,
                self.handle.clone(),
                Box::new(|state: &mut Self, _, event| state.typed(event)),
            ) {
                Ok(k) => self.keyboard = Some(k),
                Err(e) => tracing::warn!(error = %e, "no keyboard; there is no typing"),
            }
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => self.pointer = Some(p),
                Err(e) => tracing::warn!(error = %e, "no pointer; keyboard only"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard
            && let Some(k) = self.keyboard.take()
        {
            k.release();
        }
        if cap == Capability::Pointer
            && let Some(p) = self.pointer.take()
        {
            p.release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for BluetoothWindow {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        // See the field: a modifier held when the focus goes cannot be seen
        // to come up, and a stuck shift would make every arrow the loud one.
        self.mods = Modifiers::default();
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.typed(event);
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
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        mods: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.mods = mods;
    }
}

impl PointerHandler for BluetoothWindow {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let before = self.row_under_pointer;
        let mut clicked: Option<usize> = None;
        // WHERE ALONG THE LINE, for the same reason the footer keeps it: a
        // device's page has two halves side by side, and a click on the right
        // one has to put the keyboard there.
        let mut clicked_cell: Option<u16> = None;
        let mut turned = false;
        // THE BAR IS A CONTROL, and a drag is three events. `Scroll::offset_at`
        // maps any point of the TRACK, so nobody has to aim at the handle.
        // THE ROWS ARE ONLY BUILT WHEN THE BAR IS IN PLAY.
        //
        // A `Motion` arrives on every pixel of movement, and these windows
        // deliberately skip a REPAINT on motion because it "spent a GPU frame
        // per row crossed". Building the whole list to answer "is the pointer
        // on the bar?" would spend the more expensive half of that anyway.
        let bar_in_play = self.dragging
            || events.iter().any(|e| {
                matches!(e.kind, PointerEventKind::Press { button, .. } if button == LEFT_BUTTON)
            });
        let scroll = if bar_in_play {
            crate::chrome::list_scroll(self.rows().len(), self.room(), self.offset)
        } else {
            crate::chrome::list_scroll(0, 0, 0)
        };
        #[allow(clippy::cast_precision_loss)]
        let (bar_w, bar_h) = (self.width as f32, self.height as f32);
        let mut dragged: Option<usize> = None;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    #[allow(clippy::cast_possible_truncation)]
                    if self.dragging {
                        dragged = Some(crate::chrome::list_offset_at(
                            scroll,
                            FIRST_OF_LIST,
                            bar_w,
                            bar_h,
                            e.position.1 as f32,
                        ));
                    } else {
                        self.row_under_pointer = self.row_at(e.position.1);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    // AND THE DRAG ENDS WITH THE POINTER. Wayland's implicit
                    // grab usually delivers the release, but a popup grab, an
                    // unmap or a broken grab drops it — and a `dragging` left
                    // true makes the next ENTER, with no button held, jump the
                    // list to wherever the pointer came in.
                    self.dragging = false;
                    self.row_under_pointer = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    self.dragging = false;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    #[allow(clippy::cast_possible_truncation)]
                    if crate::chrome::on_the_list_bar(
                        scroll,
                        FIRST_OF_LIST,
                        bar_w,
                        bar_h,
                        e.position.0 as f32,
                        e.position.1 as f32,
                    ) {
                        self.dragging = true;
                        dragged = Some(crate::chrome::list_offset_at(
                            scroll,
                            FIRST_OF_LIST,
                            bar_w,
                            bar_h,
                            e.position.1 as f32,
                        ));
                        continue;
                    }
                    self.row_under_pointer = self.row_at(e.position.1);
                    clicked = self.row_under_pointer;
                    clicked_cell = self.cell_at(e.position.0);
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    // THE ARITHMETIC IS SHARED — see `surface::wheel_arrived`.
                    // Written out here it was `(pixels / line) * 3`, which
                    // moved nothing for a slow gesture and three lines for the
                    // next one: a list that jumps.
                    crate::surface::wheel_arrived(&mut self.lines_owed, &vertical, &horizontal);
                    turned = true;
                }
                _ => {}
            }
        }
        if let Some(offset) = dragged {
            self.offset = offset;
            self.ask_paint();
            return;
        }
        // A CLICK IS THE `RET` OF THE ROW IT LANDS ON — the footer's branch
        // for this view says why, and the two doors have to agree.
        if let Some(row) = clicked {
            // THE PANE FIRST, THEN THE ROWS: they describe whichever half has
            // the keyboard, so reading them before aiming would put the cursor
            // on a row that is about to stop existing.
            self.view.aim(clicked_cell, self.columns);
            let rows = self.rows();
            self.view.clicked(row, clicked_cell, self.columns, &rows);
            // EXCEPT WHILE A PAIRING QUESTION IS UP: there the pointer only
            // aims, because the question opens over whatever was on screen
            // and yes must not be one click from an unknown device.
            if !self.view.asking() {
                let scene = std::mem::take(&mut self.scene);
                let errand = self.view.key(
                    crate::bluetooth::Press::Enter,
                    &scene,
                    self.columns,
                    &self.texts,
                );
                self.scene = scene;
                self.apply(errand);
            }
            self.follow(self.rows().len());
            self.ask_paint();
            return;
        }
        if turned {
            let by = crate::surface::wheel_lines(&mut self.lines_owed);
            if by == 0 {
                // WHAT IS LEFT OVER STAYS. A gesture too small to be a line
                // yet is not a gesture that did nothing — it is one that has
                // not finished adding up.
                return;
            }
            let rows = self.rows().len();
            let last = rows.saturating_sub(self.room());
            self.offset = usize::try_from(
                i64::try_from(self.offset)
                    .unwrap_or(0)
                    .saturating_add(by)
                    .max(0),
            )
            .unwrap_or(0)
            .min(last);
            self.ask_paint();
            return;
        }
        // NO FRAME FOR A POINTER THAT CHANGED NOTHING. Nothing on this
        // screen is drawn from `row_under_pointer` — the click reads it, and
        // the paint does not — so repainting when it moved spent a GPU frame
        // per row crossed, on an identical picture. The footer's own hover
        // band IS drawn, which is why it repaints there and not here.
        let _ = before;
    }
}

impl CompositorHandler for BluetoothWindow {
    /// The window moved to a screen of another scale — or was told its first.
    ///
    /// It was EMPTY here and in eight other doors, which is what made every
    /// one of them blurry on a HiDPI screen. The rule and the measurement are
    /// in [`crate::hidpi`].
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        // THE SURFACE IS NEEDED to ask its VERSION, not to tell it anything:
        // it is told its scale inside the frame, which is the only commit that
        // carries a matching buffer — see `crate::hidpi::told_before_present`.
        surface: &wl_surface::WlSurface,
        factor: i32,
    ) {
        if let Some((w, h)) = crate::hidpi::followed(
            &mut self.scale,
            factor,
            surface,
            self.gpu.as_mut(),
            self.text.as_mut(),
            self.asked,
        ) {
            // AND THE DOOR KEEPS WHAT THE BUFFER CAN SHOW. Below the
            // GPU's ceiling this is what the compositor configured;
            // above it, going on with the old number lays the picture
            // out in a space the buffer does not have.
            self.width = w;
            self.height = h;
            self.recount();
            self.ask_paint();
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
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
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

impl OutputHandler for BluetoothWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for BluetoothWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(BluetoothWindow);
smithay_client_toolkit::delegate_dispatch2!(BluetoothWindow);
