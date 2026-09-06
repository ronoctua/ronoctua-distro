//! The power view's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a twelve-line ceiling (`space.expansion`,
//! invariant 14), and the power view has eight sections: a glance at the
//! battery fits the footer, reading every fact and every switch does not. So
//! this exists — and everything it knows lives in [`crate::power::View`],
//! which the footer draws from too. What this module holds is a window, a GPU
//! and the events of both; what it decides is nothing. It is the arrangement
//! `notifications_window.rs` and `files_window.rs` share, and the same header.
//!
//! # Where its data comes from
//!
//! From the same three daemons the footer listens to, on threads of its own
//! (`ronoctua_power::watch`): a window has no clock, and the bus says when
//! something changed. Orders go to those daemons directly — a profile, a
//! threshold, a suspend are the same call from any process — EXCEPT the two
//! inhibitor locks, which are file descriptors and have to live in the process
//! that outlives this window: those are forwarded to the footer as the
//! commands `LEADER s p k` and `LEADER s p l` run.

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
pub const APP_ID: &str = "br.ronoctua.power";

/// `BTN_LEFT`, from `linux/input-event-codes.h`.
const LEFT_BUTTON: u32 = 0x110;

/// The first line of the list. Above it: the hint line, and nothing else —
/// the arrangement the notification window has, for the reason it gives.
const FIRST_OF_LIST: u16 = 1;

/// The margin inside the window, in cells — see `chrome::FIELDS_LEAD`.
const CONTENT_MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// Opens the power view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<PowerWindow> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;

    let surface_of = compositor.create_surface(&qh);
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    let texts = std::rc::Rc::new(ronoctua_i18n::Texts::from_env());
    window.set_title(format!("*{}*", texts.t(ronoctua_i18n::key::POWER_TITLE)));
    window.set_app_id(APP_ID);
    window.commit();

    let mut cycle: EventLoop<PowerWindow> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();

    // THE THEME'S FILE, watched, so a choice made in the footer recolours this
    // window without it being reopened.
    let (theme_sender, theme_changed) = channel::channel::<()>();
    let _theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    handle
        .insert_source(theme_changed, |event, (), state: &mut PowerWindow| {
            if let channel::Event::Msg(()) = event
                && crate::theme::reload()
            {
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the theme channel: {e}"))?;

    // AND `power.json`, watched like the theme: a profile chosen in the
    // footer moves this window's "(your choice)" mark without it being
    // reopened. The footer is the one that APPLIES the file; here it is read.
    let (wish_sender, wish_changed) = channel::channel::<()>();
    let _power_watch = ronoctua_power::prefs::home().and_then(|dir| {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(error = %e, "no configuration directory to watch power.json in");
            return None;
        }
        ronoctua_files::notice::Notice::on_file(&dir, ronoctua_power::prefs::FILE, move || {
            wish_sender.send(()).is_ok()
        })
        .ok()
    });
    handle
        .insert_source(wish_changed, |event, (), state: &mut PowerWindow| {
            if let channel::Event::Msg(()) = event {
                let (wanted, _, complaints) = ronoctua_power::prefs::resolve_in(
                    ronoctua_power::prefs::home().as_deref(),
                    std::env::var(ronoctua_power::prefs::ENV).ok().as_deref(),
                );
                for c in complaints {
                    tracing::warn!(%c, "power.json");
                }
                if state.wanted != wanted {
                    state.wanted = wanted;
                    state.ask_paint();
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the power.json watch: {e}"))?;

    // THE DAEMONS, on threads of this process — the bus is the clock.
    let (power_sender, power_events) = channel::channel::<ronoctua_power::Event>();
    let power = match ronoctua_power::watch(move |e| {
        let _ = power_sender.send(e);
    }) {
        Ok(h) => Some(h),
        Err(e) => {
            tracing::warn!(error = %e, "no power daemons; the window opens with nothing to read");
            None
        }
    };
    handle
        .insert_source(power_events, |event, (), state: &mut PowerWindow| {
            if let channel::Event::Msg(e) = event {
                state.power_event(e);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the power channel: {e}"))?;

    let wanted = ronoctua_power::prefs::resolve_in(
        ronoctua_power::prefs::home().as_deref(),
        std::env::var(ronoctua_power::prefs::ENV).ok().as_deref(),
    )
    .0;

    let mut state = PowerWindow {
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
        columns: 80,
        lines: 24,
        quit: false,
        owed: false,
        gpu: None,
        text: None,
        view: crate::power::View::fresh(),
        snapshot: None,
        power,
        wanted,
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
struct PowerWindow {
    conn: Connection,
    /// The loop, for the ONE thing that needs it here: a keyboard that
    /// REPEATS. SCTK only generates repeat events for a keyboard made with
    /// `get_keyboard_with_repeat`, and that wants a `LoopHandle` — so this is
    /// carried rather than reached for. Without it `repeat_key` below is dead
    /// code and holding an arrow moves exactly one row, which is the trap
    /// CLAUDE.md records and which five windows inherited.
    handle: calloop::LoopHandle<'static, Self>,
    _theme_watch: Option<ronoctua_files::notice::Notice>,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    window: Window,
    width: u32,
    height: u32,
    columns: u16,
    lines: u16,
    quit: bool,
    /// Whether a frame is owed — see `notifications_window`'s field of the
    /// same name for why nothing here paints straight away.
    owed: bool,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    view: crate::power::View,
    /// The last reading, from this process's own threads.
    snapshot: Option<ronoctua_power::Snapshot>,
    /// The orders thread, when the daemons were there.
    power: Option<ronoctua_power::Handle>,
    /// The profile the person chose (or the distro's floor) — drawn as a
    /// mark beside the profile rows.
    wanted: ronoctua_power::Profile,
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    row_under_pointer: Option<usize>,
    /// The first visible row. A window has no page: it scrolls.
    offset: usize,
    message: Option<String>,
}

impl PowerWindow {
    fn power_event(&mut self, e: ronoctua_power::Event) {
        match e {
            ronoctua_power::Event::Snapshot(s) => {
                // THE VIEW FOLLOWS THE NEW ROWS, the footer's reason: rows
                // come and go, and a pair the arrows were moving is a lie
                // once somebody else has written the thresholds.
                let moved = self.snapshot.as_ref().map(|b| b.thresholds.clone())
                    != Some(s.thresholds.clone());
                self.snapshot = Some(*s);
                if moved {
                    self.view.forget_pending();
                }
                let rows = self.rows();
                self.view.settle(&rows);
                self.ask_paint();
            }
            ronoctua_power::Event::Done { order, refused } => {
                if let Some((why, words)) = refused {
                    self.message = Some(crate::power_words::refused(
                        &order,
                        why,
                        &words,
                        &self.texts,
                    ));
                    self.ask_paint();
                }
            }
        }
    }

    /// How many rows fit under the hint line.
    const fn room(&self) -> usize {
        self.lines.saturating_sub(FIRST_OF_LIST) as usize
    }

    /// Keeps the cursor on screen, starting to scroll two rows before the edge.
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

    /// "The lock is the footer's to hold", with the two sequences RESOLVED.
    ///
    /// The keys are an ARGUMENT and never text in the `.ftl`: a sentence that
    /// carried them went stale the day either moved, and the word "LEADER" is
    /// nobody's key — `nix/modules/niri.nix` says `SUPER` and `nix/dev.nix`
    /// says `CTRL ALT SPC`. `crate::leader()` is the one place that knows.
    ///
    /// `leader()` and not `leader_chord()`: these are sequences typed after
    /// the leader, and the chord's label is the modifier held for the arrows —
    /// in the nested loop the two are not the same key.
    fn answer_in_the_footer(&self) -> String {
        let leader = crate::leader();
        self.texts.t_with(
            ronoctua_i18n::key::POWER_ANSWER_IN_THE_FOOTER,
            &[
                (
                    "awake",
                    &format!("{leader} {}", crate::leader::KEEP_AWAKE_KEYS),
                ),
                ("lid", &format!("{leader} {}", crate::leader::KEEP_LID_KEYS)),
            ],
        )
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, errand: crate::power::Errand) {
        use crate::power::Errand;
        match errand {
            Errand::Nothing => {}
            Errand::Close => {
                self.quit = true;
                return;
            }
            // THE LOCKS ARE THE FOOTER'S TO HOLD: an inhibitor is an fd, and
            // this window will close. The command the footer runs for the same
            // key is what is asked for — one door, through the other.
            Errand::Order(ronoctua_power::Order::KeepAwake(keep, _)) => {
                // NOT `status()`, WHICH WAITS. This runs inside the calloop
                // callback, and waiting for a child there freezes the window
                // for as long as the child takes — the same trap `CLAUDE.md`
                // records about `systemd-run --scope` in the footer's loop. A
                // footer socket that is gone or slow made this window stop
                // painting; a `connect(2)` to a dead socket is fast, and a
                // busy one is not.
                //
                // AND A FAILURE IS A SENTENCE, not silence: `current_exe`
                // failing used to drop the whole thing with no message, which
                // from the outside is a key that did nothing.
                let id = crate::power_words::keep_command(keep);
                let sent = std::env::current_exe().ok().and_then(|me| {
                    std::process::Command::new(me)
                        .args(["command", id])
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                        .ok()
                });
                match sent {
                    // THE CHILD IS LET GO OF, and that is deliberate: it
                    // writes one line to the footer's socket and exits. Its
                    // exit code would say whether the socket took the line,
                    // and waiting for it is what this was doing wrong.
                    Some(child) => drop(child),
                    None => {
                        self.message = Some(self.answer_in_the_footer());
                    }
                }
            }
            Errand::Order(order) => match &self.power {
                Some(h) => h.send(order),
                None => {
                    self.message = Some(self.texts.t(ronoctua_i18n::key::POWER_NO_DAEMON));
                }
            },
            // THE ORDER AND THE FILE, as in the footer: a choice made here
            // has to survive the next login. The footer, which watches the
            // file, may send the same order once more — idempotent at the
            // daemon, and only while its own reading still disagrees.
            Errand::Choose(profile) => {
                self.wanted = profile;
                match &self.power {
                    Some(h) => h.send(ronoctua_power::Order::SetProfile(profile)),
                    None => {
                        self.message = Some(self.texts.t(ronoctua_i18n::key::POWER_NO_DAEMON));
                    }
                }
                if let Some(dir) = ronoctua_power::prefs::home()
                    && let Err(e) = ronoctua_power::prefs::persist(&dir, profile)
                {
                    tracing::warn!(error = %e, "could not write power.json");
                }
            }
        }
        self.ask_paint();
    }

    /// Columns and rows from the size in pixels.
    fn recount(&mut self) {
        let size = tokens::font::SIZE_BASE;
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
        #[allow(unsafe_code)]
        let gpu = unsafe {
            ronoctua_render::gpu::Gpu::new(
                RawDisplayHandle::Wayland(WaylandDisplayHandle::new(disp)),
                RawWindowHandle::Wayland(WaylandWindowHandle::new(surf)),
                self.width,
                self.height,
            )
        };
        let Ok(g) = gpu else {
            tracing::error!("no usable GPU; the window will not draw");
            return;
        };
        match ronoctua_render::Text::new(
            &g.device,
            &g.queue,
            g.config.format,
            tokens::font::SIZE_BASE,
        ) {
            Ok(t) => self.text = Some(t),
            Err(e) => tracing::error!(error = %e, "assembling the text failed"),
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
        let Some(press) = press_of(&event) else {
            return;
        };
        self.message = None;
        let errand = self
            .view
            .key(press, self.snapshot.as_ref(), self.wanted, &self.texts);
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

    fn rows(&self) -> Vec<crate::power::Row> {
        self.view.rows(
            self.snapshot.as_ref(),
            self.wanted,
            self.columns,
            &self.texts,
        )
    }

    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let size = tokens::font::SIZE_BASE;
        let margin = CONTENT_MARGIN;
        let rows = self.rows();
        self.follow(rows.len());

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

        let (Some(gpu), Some(text)) = (self.gpu.as_mut(), self.text.as_mut()) else {
            return;
        };
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
        if let Err(e) = gpu.frame(|pass| text.draw(ronoctua_render::Atlas::Pinned, pass)) {
            tracing::error!(error = %e, "drawing the frame failed");
            return;
        }
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "the flush failed");
        }
    }

    /// Which drawn row is at height `y`.
    fn row_at(&self, y: f64) -> Option<usize> {
        let line = f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE));
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
fn press_of(event: &KeyEvent) -> Option<crate::power::Press> {
    use crate::power::Press;
    Some(match event.keysym {
        Keysym::Escape => Press::Escape,
        Keysym::Return | Keysym::KP_Enter => Press::Enter,
        Keysym::Left => Press::Left,
        Keysym::Right => Press::Right,
        Keysym::Up => Press::Up,
        Keysym::Down => Press::Down,
        _ => return None,
    })
}

impl WindowHandler for PowerWindow {
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
        self.width = cfg.new_size.0.map_or(900, std::num::NonZeroU32::get);
        self.height = cfg.new_size.1.map_or(600, std::num::NonZeroU32::get);
        if let Some(g) = self.gpu.as_mut() {
            g.resize(self.width, self.height);
        }
        self.recount();
        self.ask_paint();
    }
}

impl SeatHandler for PowerWindow {
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

impl KeyboardHandler for PowerWindow {
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
        _: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
    }
}

impl PointerHandler for PowerWindow {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let before = self.row_under_pointer;
        let mut clicked: Option<usize> = None;
        let mut wheel = 0.0f64;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.row_under_pointer = self.row_at(e.position.1);
                }
                PointerEventKind::Leave { .. } => self.row_under_pointer = None,
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    self.row_under_pointer = self.row_at(e.position.1);
                    clicked = self.row_under_pointer;
                }
                PointerEventKind::Axis { vertical, .. } => wheel += vertical.absolute,
                _ => {}
            }
        }
        // THE MOUSE AIMS, THE KEYS ACT — a click puts the cursor on the row,
        // and `RET` is what applies, arms and confirms.
        if let Some(row) = clicked {
            let rows = self.rows();
            self.view.clicked(row, &rows);
            self.ask_paint();
            return;
        }
        if wheel.abs() >= 1.0 {
            let line_px = f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE));
            #[allow(clippy::cast_possible_truncation)]
            let by = (wheel / line_px.max(1.0)) as i64 * 3;
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

impl CompositorHandler for PowerWindow {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
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

impl OutputHandler for PowerWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for PowerWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(PowerWindow);
smithay_client_toolkit::delegate_dispatch2!(PowerWindow);
