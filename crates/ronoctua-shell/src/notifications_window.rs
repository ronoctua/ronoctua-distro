//! The notification manager's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a twelve-line ceiling (`space.expansion`,
//! invariant 14) and it is right that it does: it is a footer. Twelve rows are
//! enough to see what just happened and act on it, and not enough to read a
//! fortnight of history or to go through every filter the distro ships with.
//!
//! So this exists — and everything it knows lives in
//! [`crate::notifications::View`], which the footer draws from too. What this
//! module holds is a window, a GPU and the events of both; what it decides is
//! nothing. It is the same arrangement, and the same header, as
//! `files_window.rs`.
//!
//! # Where its data comes from
//!
//! From the same two files the footer reads: the history under
//! `$XDG_STATE_HOME` and the person's filters under `$XDG_CONFIG_HOME` (see
//! `ronoctua_notify::store`). It re-reads them on a beat rather than sharing
//! memory with the footer — they are two processes, and a socket for a list
//! that changes a few times an hour would be a mechanism to keep in step for
//! nothing.

use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::calloop::{EventLoop, timer},
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
///
/// Its own, like the file manager's and the terminal's: it is by this that the
/// modeline and the VM test tell one window from another.
pub const APP_ID: &str = "br.ronoctua.notifications";

/// `BTN_LEFT`, from `linux/input-event-codes.h`.
const LEFT_BUTTON: u32 = 0x110;

/// The first line of the list. Above it: the title, and the message line.
///
/// The same arrangement as the footer — two lines glued, content below — and
/// repeated here deliberately: a person should not have to learn a second shape
/// for the same thing.
const FIRST_OF_LIST: u16 = 2;

/// The margin inside the window, in cells.
///
/// The same difference every other surface pays, and for the reason written at
/// [`crate::chrome::FIELDS_LEAD`]: the rows already carry a gutter, so what a
/// surface owes is what is left over.
const CONTENT_MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// How often the window re-reads what the footer wrote.
///
/// Two seconds. The history changes when something happens, which on a desktop
/// is a few times an hour; polling faster would spend a wake-up to find the
/// same file, and polling slower would make the window feel stale next to the
/// band that raised the notification.
const REREAD: std::time::Duration = std::time::Duration::from_secs(2);

/// Opens the manager in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("registry do Wayland")?;
    let qh: QueueHandle<NotificationsWindow> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("sem wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;

    let surface_of = compositor.create_surface(&qh);
    // `RequestServer`: decoration belongs to the COMPOSITOR (invariant 4).
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    let texts = std::rc::Rc::new(ronoctua_i18n::Texts::from_env());
    // `*name*` is Emacs's convention for a buffer that is not a file, the same
    // one the modeline already uses for `*scratch*`.
    window.set_title(format!("*{}*", texts.t(ronoctua_i18n::key::NOTIF_HISTORY)));
    window.set_app_id(APP_ID);
    window.commit();

    let mut cycle: EventLoop<NotificationsWindow> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();
    handle
        .insert_source(
            timer::Timer::from_duration(REREAD),
            |_, (), state: &mut NotificationsWindow| {
                state.reread();
                timer::TimeoutAction::ToDuration(REREAD)
            },
        )
        .map_err(|e| anyhow::anyhow!("could not hang the clock: {e}"))?;

    let mut state = NotificationsWindow {
        conn: conn.clone(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboard: None,
        pointer: None,
        mods: Modifiers::default(),
        window,
        width: 0,
        height: 0,
        columns: 80,
        lines: 24,
        quit: false,
        owed: false,
        gpu: None,
        text: None,
        quads: None,
        view: crate::notifications::View::fresh(),
        records: Vec::new(),
        book: ronoctua_notify::Book::distro(),
        notes: ronoctua_notify::store::Where::of_env(),
        texts,
        row_under_pointer: None,
        offset: 0,
        message: None,
    };
    state.reread();

    WaylandSource::new(conn, queue)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |e| {
            // THE ONE PLACE THAT KNOWS THE BATCH IS OVER.
            e.settle();
            if e.quit {
                signal.stop();
            }
        })
        .context("event loop")?;
    Ok(())
}

/// The window.
struct NotificationsWindow {
    conn: Connection,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    mods: Modifiers,
    window: Window,
    width: u32,
    height: u32,
    columns: u16,
    lines: u16,
    quit: bool,
    /// Whether a frame is owed.
    ///
    /// # Why nothing here paints straight away
    ///
    /// Because presentation blocks on the vertical blank, and a handler that
    /// paints is a handler that waits for the monitor. A drag delivers motion
    /// as fast as the device reports it, so painting per event turned the
    /// gesture into a walk: the list crawled through its rows instead of
    /// arriving where the pointer already was.
    ///
    /// The shape is `files_window`'s, and the reason is `Footer::owes_paint`'s
    /// before that. A hundred events in one batch come out as ONE frame, drawn
    /// from the last of them.
    owed: bool,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    quads: Option<ronoctua_render::quad::Quads>,
    view: crate::notifications::View,
    /// The history, as it was last read off disk.
    records: Vec<ronoctua_notify::Record>,
    /// The catalogue, the distro's filters and the person's.
    book: ronoctua_notify::Book,
    /// Where this process persists — the two directories, found ONCE.
    ///
    /// A SEPARATE PROCESS from the footer, with its own copy of both files;
    /// carrying the roots rather than finding them per write is what keeps a
    /// test from reaching the real home. See `sidebar::Sidebar`'s `store` for
    /// the incident that made it a rule.
    notes: ronoctua_notify::store::Where,
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    row_under_pointer: Option<usize>,
    /// The first visible row. A window has no page: it scrolls, like the
    /// locator and the manual.
    offset: usize,
    message: Option<String>,
}

impl NotificationsWindow {
    /// Re-reads what the footer wrote, and redraws if it changed.
    ///
    /// Compares before drawing: repainting on a beat that found nothing new is
    /// a frame spent to put the same pixels back.
    fn reread(&mut self) {
        let fresh = self.notes.history();
        let prefs = self.notes.preferences();
        let same = fresh == self.records
            && prefs.adjustments == self.book.adjustments
            && prefs.rules == self.book.mine;
        if same {
            return;
        }
        self.records = fresh;
        self.book.mine = prefs.rules;
        self.book.adjustments = prefs.adjustments;
        self.ask_paint();
    }

    /// How many rows fit under the two fixed lines.
    const fn room(&self) -> usize {
        self.lines.saturating_sub(FIRST_OF_LIST) as usize
    }

    /// Keeps the cursor on screen, starting to scroll two rows before the edge.
    ///
    /// `scrolloff` two, vi's number and the footer's: a cursor pinned to the
    /// last visible line gives no warning of what is coming.
    fn follow(&mut self, rows: usize) {
        const MARGIN: usize = 2;
        let room = self.room();
        if room == 0 {
            return;
        }
        // PLUS THE HEADING, which is row zero of what is drawn. Without it the
        // scrolling aims one line above the focused one — the same adjustment
        // the footer's own `list_cursor` makes.
        let cursor = self.view.cursor() + 1;
        if cursor < self.offset + MARGIN {
            self.offset = cursor.saturating_sub(MARGIN);
        } else if cursor + MARGIN >= self.offset + room {
            self.offset = cursor + MARGIN + 1 - room;
        }
        self.offset = self.offset.min(rows.saturating_sub(room));
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, errand: crate::notifications::Errand) {
        use crate::notifications::Errand;
        match errand {
            Errand::Nothing => {}
            Errand::Close => {
                self.quit = true;
                return;
            }
            // THE WINDOW WRITES THE SAME FILES THE FOOTER READS. It is not a
            // second store: `store::write_history` is one function, and the
            // footer picks the change up on its next write or restart.
            Errand::Read(id) => {
                for r in self.records.iter_mut().filter(|r| r.notification.id == id) {
                    r.read = true;
                }
                self.save_history();
            }
            Errand::ReadAll => {
                for r in &mut self.records {
                    r.read = true;
                }
                self.save_history();
            }
            Errand::Forget(id) => {
                self.records.retain(|r| r.notification.id != id);
                self.save_history();
            }
            // AN ACTION IS THE FOOTER'S TO RUN, and it says so instead of
            // running it. The footer is the process that knows how to spawn
            // into a scope of its own (section 9.1) and that holds the
            // clipboard; a second implementation here is exactly the second
            // door disagreeing with the first.
            Errand::Act(_, _) => {
                self.message = Some(self.texts.t(ronoctua_i18n::key::NOTIF_ACT_IN_THE_FOOTER));
            }
            Errand::Adjusted(id, verdict) => {
                match verdict {
                    Some(v) => self.book.adjustments.insert(id, v),
                    None => self.book.adjustments.remove(&id),
                };
                let prefs = ronoctua_notify::Preferences {
                    rules: self.book.mine.clone(),
                    adjustments: self.book.adjustments.clone(),
                };
                if let Err(e) = self.notes.save_preferences(&prefs) {
                    tracing::warn!(error = %e, "could not write the preferences");
                }
            }
        }
        self.ask_paint();
    }

    fn save_history(&mut self) {
        if let Err(e) = self.notes.save_history(&mut self.records) {
            tracing::warn!(error = %e, "could not write the history");
        }
    }

    /// Columns and rows from the size in pixels.
    fn recount(&mut self) {
        let size = tokens::font::SIZE_LARGE;
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
            tokens::font::SIZE_LARGE,
        ) {
            Ok(t) => self.text = Some(t),
            Err(e) => tracing::error!(error = %e, "assembling the text failed"),
        }
        match ronoctua_render::quad::Quads::new(&g.device, g.config.format) {
            Ok(q) => self.quads = Some(q),
            Err(e) => tracing::error!(error = %e, "the quad pipeline failed"),
        }
        self.gpu = Some(g);
    }

    /// Asks for a frame. It is drawn when the event queue empties.
    ///
    /// See the `owed` field for why nothing here presents directly.
    const fn ask_paint(&mut self) {
        self.owed = true;
    }

    /// Draws the owed frame, if one is owed.
    ///
    /// Called from the event loop's callback — the one place that knows the
    /// batch is finished.
    fn settle(&mut self) {
        if std::mem::take(&mut self.owed) {
            self.paint();
        }
    }

    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let size = tokens::font::SIZE_LARGE;
        let margin = CONTENT_MARGIN;
        // THE LISTS ARE BUILT AND DROPPED before `follow` moves the offset:
        // they borrow the records, and the scroll is a write.
        let (rows, unread) = {
            let lists = crate::notifications::Lists::of(&self.records, &self.book);
            let rows = self
                .view
                .rows(&lists, &self.book, self.columns, &self.texts);
            let unread = lists.happened.iter().filter(|r| !r.read).count();
            (rows, unread)
        };
        self.follow(rows.len());

        let mut spans: Vec<ronoctua_render::Span> = Vec::new();
        let mut quads: Vec<ronoctua_render::quad::Quad> = Vec::new();

        // LINE 0: which pane, and how many are unread.
        spans.push(ronoctua_render::Span {
            size: ronoctua_render::Size::Px(size),
            text: self.texts.t_args(
                ronoctua_i18n::key::NOTIF_WINDOW_TITLE,
                &[
                    // WHICH PANE, because the window shows one at a time when
                    // it is narrow and `TAB` is the only thing that says so.
                    (
                        "pane",
                        ronoctua_i18n::Arg::Text(&self.texts.t(self.view.pane().heading())),
                    ),
                    (
                        "n",
                        ronoctua_i18n::Arg::Number(i64::try_from(unread).unwrap_or(0)),
                    ),
                ],
            ),
            column: margin,
            line: 0,
            color: tokens::color::TEXT_PRIMARY,
            edge: false,
        });

        // LINE 1: what the last action said, or the keys.
        let second = self
            .message
            .clone()
            // The focused row's own note — the reason a switch does nothing.
            // The same source as the footer's, so the two doors say the same
            // sentence in the same place.
            .or_else(|| self.view.hint(&self.texts))
            .unwrap_or_else(|| self.texts.t(ronoctua_i18n::key::NOTIF_WINDOW_KEYS));
        spans.push(ronoctua_render::Span {
            size: ronoctua_render::Size::Px(size),
            text: ronoctua_render::layout::elide_end(&second, self.columns),
            column: margin,
            line: 1,
            color: tokens::color::TEXT_SECONDARY,
            edge: false,
        });

        let line_px = tokens::metrics::line_height(size);
        let cell_px = tokens::metrics::cell_width(size);
        for (i, row) in rows.iter().skip(self.offset).take(self.room()).enumerate() {
            let y = FIRST_OF_LIST + u16::try_from(i).unwrap_or(0);
            if row.focused {
                quads.push(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y: f32::from(y) * line_px,
                    width: self.width as f32,
                    height: line_px,
                    color: tokens::color::SURFACE_SELECTION,
                });
            }
            for f in &row.fields {
                spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::Px(size),
                    text: f.text.clone(),
                    column: margin + f.at,
                    line: y,
                    color: f.color,
                    edge: false,
                });
            }
        }
        let _ = cell_px;

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
        if let Some(q) = self.quads.as_mut() {
            q.prepare(
                &gpu.device,
                &gpu.queue,
                gpu.config.width,
                gpu.config.height,
                &quads,
            );
        }
        let quads_ref = self.quads.as_ref();
        if let Err(e) = gpu.frame(|pass| {
            if let Some(q) = quads_ref {
                q.draw(pass);
            }
            text.draw(ronoctua_render::Atlas::Pinned, pass)
        }) {
            tracing::error!(error = %e, "drawing the frame failed");
            return;
        }
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "the flush failed");
        }
    }

    /// Which cell of the row is at `x`.
    ///
    /// The content's margin comes off first: the row's own cells start after
    /// it, and counting from the window's edge would put every target one
    /// margin to the right of where it is drawn.
    fn cell_at(&self, x: f64) -> Option<u16> {
        let cell = f64::from(tokens::metrics::cell_width(tokens::font::SIZE_LARGE));
        if cell <= 0.0 || x < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let which = (x / cell) as u16;
        which.checked_sub(CONTENT_MARGIN)
    }

    /// Which drawn row is at height `y`.
    fn row_at(&self, y: f64) -> Option<usize> {
        let line = f64::from(tokens::metrics::line_height(tokens::font::SIZE_LARGE));
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

/// A `KeyEvent` as the manager sees it.
///
/// The same mapping the footer does — see `Footer::manager_key`. Two doors
/// answering one key differently is the defect this whole arrangement exists to
/// prevent, so the table is the same table.
fn press_of(event: &KeyEvent) -> Option<crate::notifications::Press> {
    use crate::notifications::Press;
    Some(match event.keysym {
        Keysym::Escape => Press::Escape,
        Keysym::Return | Keysym::KP_Enter => Press::Enter,
        Keysym::Tab | Keysym::ISO_Left_Tab => Press::Tab,
        Keysym::space => Press::Space,
        Keysym::Left => Press::Left,
        Keysym::Right => Press::Right,
        Keysym::Up => Press::Up,
        Keysym::Down => Press::Down,
        otherwise => Press::Letter(otherwise.key_char()?),
    })
}
impl WindowHandler for NotificationsWindow {
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

impl SeatHandler for NotificationsWindow {
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
            match self.seats.get_keyboard(qh, &seat, None) {
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

impl KeyboardHandler for NotificationsWindow {
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
        // The modifiers whole, for the reason the footer already records: a
        // state that only updates with focus cannot keep holding without it,
        // and a stuck `alt` makes every following key come out as `M-…`.
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
        let Some(press) = press_of(&event) else {
            return;
        };
        // A key clears the last message: it answered something that is no
        // longer what is being done.
        self.message = None;
        let errand = {
            let lists = crate::notifications::Lists::of(&self.records, &self.book);
            self.view.key(press, &lists, &self.book)
        };
        self.apply(errand);
    }

    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        // Holding `j` walks the list, which is what a listing wants — unlike
        // the footer, where repeating a leader key is always a mistake.
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

impl PointerHandler for NotificationsWindow {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let before = self.row_under_pointer;
        // THE CELL TRAVELS WITH THE ROW. A row has two targets on it — the
        // envelope and the bin — so the row alone cannot say what was clicked,
        // and looking the position up again later would be a second answer to
        // one question.
        let mut clicked: Option<(usize, Option<u16>)> = None;
        let mut wheel = 0.0f64;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.row_under_pointer = self.row_at(e.position.1);
                }
                PointerEventKind::Leave { .. } => self.row_under_pointer = None,
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    self.row_under_pointer = self.row_at(e.position.1);
                    clicked = self
                        .row_under_pointer
                        .map(|r| (r, self.cell_at(e.position.0)));
                }
                // THE RIGHT BUTTON DOES NOTHING HERE. There is no menu in this
                // view: every action it offers is one key, and a second meaning
                // for the same target would be chrome nobody asked for.
                PointerEventKind::Axis { vertical, .. } => wheel += vertical.absolute,
                _ => {}
            }
        }
        // THE MOUSE AIMS, THE KEYS ACT — the transfer panel's rule, and this
        // view has the same shape: marking read, running an action, adjusting a
        // filter are all things somebody meant to do.
        if let Some((row, cell)) = clicked {
            // Row zero of the drawing is the heading; it names the column and
            // is not in it.
            if let Some(i) = row.checked_sub(1) {
                let errand = {
                    let lists = crate::notifications::Lists::of(&self.records, &self.book);
                    self.view.clicked(i, cell, &lists, self.columns)
                };
                self.apply(errand);
                return;
            }
            self.ask_paint();
            return;
        }
        if wheel.abs() >= 1.0 {
            let line_px = f64::from(tokens::metrics::line_height(tokens::font::SIZE_LARGE));
            #[allow(clippy::cast_possible_truncation)]
            let by = (wheel / line_px.max(1.0)) as i64 * 3;
            let rows = {
                let lists = crate::notifications::Lists::of(&self.records, &self.book);
                self.view
                    .rows(&lists, &self.book, self.columns, &self.texts)
                    .len()
            };
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
        if before != self.row_under_pointer {
            self.ask_paint();
        }
    }
}
impl CompositorHandler for NotificationsWindow {
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

impl OutputHandler for NotificationsWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for NotificationsWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(NotificationsWindow);
smithay_client_toolkit::delegate_dispatch2!(NotificationsWindow);
