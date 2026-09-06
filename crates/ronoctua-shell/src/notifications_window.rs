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
    reexports::calloop::{self, EventLoop, channel, timer},
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

/// The first line of the list. Above it: the hint line, and nothing else.
///
/// It was two — a title saying which pane and how many were unread, then the
/// keys — and the owner asked for the title gone: the pane is named by its
/// heading one row down, the count is what the bell's own hover says, and a
/// line that repeats both is a line the footer does not have. One line above
/// the list, the same one the footer's minibuffer draws — the file manager's
/// window has the same shape, and for the same reason.
const FIRST_OF_LIST: u16 = 1;

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
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<NotificationsWindow> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;

    let surface_of = compositor.create_surface(&qh);
    // `RequestServer`: decoration belongs to the COMPOSITOR (invariant 4).
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    let texts = std::rc::Rc::new(ronoctua_i18n::Texts::from_env());
    // `*name*` is Emacs's convention for a buffer that is not a file, the same
    // one the modeline already uses for `*scratch*`.
    window.set_title(format!("*{}*", texts.t(ronoctua_i18n::key::NOTIF_TITLE)));
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

    // THE THEME'S FILE, watched, so a choice made in the footer recolours this
    // window without it being reopened.
    let (theme_sender, theme_changed) = channel::channel::<()>();
    let _theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    handle
        .insert_source(
            theme_changed,
            |event, (), state: &mut NotificationsWindow| {
                if let channel::Event::Msg(()) = event
                    && crate::theme::reload()
                {
                    state.ask_paint();
                }
            },
        )
        .map_err(|e| anyhow::anyhow!("could not hang the theme channel: {e}"))?;

    let mut state = NotificationsWindow {
        conn: conn.clone(),
        handle: handle.clone(),
        _theme_watch,
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
        dragging: false,
        lines_owed: 0.0,
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
    /// The loop, for the ONE thing that needs it here: a keyboard that
    /// REPEATS. SCTK only generates repeat events for a keyboard made with
    /// `get_keyboard_with_repeat`, and that wants a `LoopHandle` — so this is
    /// carried rather than reached for. Without it `repeat_key` below is dead
    /// code and holding an arrow moves exactly one row, which is the trap
    /// CLAUDE.md records and which five windows inherited.
    handle: calloop::LoopHandle<'static, Self>,
    /// The watch on `theme.json` — the file is the broadcast (see `theme.rs`).
    _theme_watch: Option<ronoctua_files::notice::Notice>,
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
    /// How many rows the list has, right now.
    ///
    /// The pointer needs the count and only the count — for the scrollbar's
    /// arithmetic — and building the rows to take `.len()` is what the wheel
    /// below already does.
    fn rows_now(&self) -> usize {
        let lists = crate::notifications::Lists::of(&self.records, &self.book);
        self.view
            .rows(&lists, &self.book, self.columns, &self.texts)
            .len()
    }

    const fn room(&self) -> usize {
        self.lines.saturating_sub(FIRST_OF_LIST) as usize
    }

    /// Keeps the cursor on screen, starting to scroll two rows before the edge.
    ///
    /// `scrolloff` two, vi's number and the footer's: a cursor pinned to the
    /// last visible line gives no warning of what is coming.
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
    /// from seventeen places and every one of them is an EVENT.
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
        // THE SHEET IS NOT A LIST, and its rows are not the cursor's.
        //
        // With an entry open the drawn rows are the KNOBS — eight of them —
        // while `View::cursor()` is still the CATALOGUE's index, which can be
        // thirty. In a short window that pinned the sheet to its last rows,
        // and moving between knobs scrolled nothing at all. The sheet is
        // short by construction, so there is nothing to follow: the offset
        // goes home and stays there.
        if self.view.in_sheet() {
            self.offset = 0;
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
            // footer WATCHES both files by inotify — the same "the file is
            // the broadcast" the theme uses — so a record forgotten here is
            // forgotten there within the event loop's next turn.
            //
            // It used to say the footer picked the change up "on its next
            // write or restart", and that was the opposite of what happened:
            // the footer read the history once, at start-up, and wrote its
            // whole vector over the file on every change, so this door's `d`
            // came undone the moment any notification arrived.
            Errand::Read(id) => {
                for r in self.records.iter_mut().filter(|r| r.notification.id == id) {
                    r.read = true;
                }
                self.save_history();
            }
            Errand::ReadAll => {
                // NOT THE SUPPRESSED ONES, for the footer's reason: they are
                // already off the count, `SPC` refuses them one at a time,
                // and marking them read here would be the two doors
                // disagreeing about one word.
                for r in &mut self.records {
                    if !matches!(r.presentation.show, ronoctua_notify::Show::Hidden) {
                        r.read = true;
                    }
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
                // THE KEYS ARE AN ARGUMENT, read from the same constant the
                // keymap binds: a sentence that carried `n h` as text went
                // stale the day the key moved.
                // `leader()` and not `leader_chord()`: the sequence is typed
                // after the LEADER (`SPC`, or `CTRL ALT SPC` in the nested
                // loop); the chord's label is the modifier held for the
                // arrows, and in the nested loop the two are not the same.
                let keys = format!("{} {}", crate::leader(), crate::leader::NOTIF_OPEN_KEYS);
                self.message = Some(self.texts.t_with(
                    ronoctua_i18n::key::NOTIF_ACT_IN_THE_FOOTER,
                    &[("keys", &keys)],
                ));
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
        // THE CURSOR MOVED, so the list follows it — here, on the EVENT, and
        // never in the paint. See `settle_offset` for what following from a
        // paint cost.
        let rows = self.rows_now();
        self.follow(rows);
        self.apply(errand);
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
        let size = tokens::font::SIZE_BASE;
        let margin = CONTENT_MARGIN;
        // THE LISTS ARE BUILT AND DROPPED before `follow` moves the offset:
        // they borrow the records, and the scroll is a write.
        let rows = {
            let lists = crate::notifications::Lists::of(&self.records, &self.book);
            self.view
                .rows(&lists, &self.book, self.columns, &self.texts)
        };
        self.settle_offset(rows.len());

        let mut spans: Vec<ronoctua_render::Span> = Vec::new();

        // LINE 0: what the last action said, the focused row's own note, or
        // the keys — the footer's minibuffer, through the other door.
        //
        // The message and the note win over the keys for the reason the footer
        // gives on its own line: they are what the manager has to SAY about
        // something just done, and the keys are always there to come back to.
        // The note is the same source as the footer's — the reason a switch
        // does nothing — so the two doors say the same sentence in the same
        // place.
        let said = self.message.clone().or_else(|| self.view.note(&self.texts));
        // THE ROOM IS THE WHOLE OF `columns`, and that is not the mistake it
        // looks like: `recount` already took BOTH margins off the width before
        // counting cells, so `columns` is what lies between them and a line
        // that starts `margin` in and runs `columns` wide ends `margin` short
        // of the edge. The file manager's window counts the same way.
        let room = self.columns;
        if let Some(text) = said {
            spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text: ronoctua_render::layout::elide_end(&text, room),
                column: margin,
                line: 0,
                // THE SAME REGISTER AS THE HINT that shares this line, and as
                // the footer's minibuffer, which is the same line through the
                // other door. See `text.subtle`.
                color: tokens::color::TEXT_SUBTLE_STRONG,
                edge: false,
                foot: false,
            });
        } else {
            // THE KEYS, in two registers, from the SAME table and the SAME
            // painter as the footer's minibuffer — `chrome::hint_spans` is what
            // stops either door colouring a key on its own. It was one flat
            // span of a `.ftl` sentence here, in a colour the rest of the
            // system does not use for a key, which the owner reported.
            // AND WHERE IN THE LIST THE CURSOR IS. This window draws no
            // rectangle — text and a colour, one render pass — so it has no
            // scrollbar, and with a fortnight of records there was no sign
            // that anything lay below the last drawn row.
            let lists = crate::notifications::Lists::of(&self.records, &self.book);
            let place = Some((self.view.cursor(), lists.len(self.view.pane())));
            spans.extend(crate::chrome::hint_spans(
                &self.view.hint_of(place, &self.texts),
                ronoctua_render::Size::Px(size),
                margin,
                0,
                room,
            ));
        }

        // THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
        // draws for these rows, and the one `record_line` argues for: with
        // two columns on screen a band across the row would say the two
        // items on it were one thing. The window drew a band as well, which
        // was the one thing the two doors did differently; it went.
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
        // TEXT ONLY: this window draws no rectangle. The focus is a colour
        // (see the paint loop), there is no scrollbar, and the hint line is
        // text — so there is no quad pipeline here, and a frame is one pass.
        if let Some(q) = self.quads.as_mut() {
            q.prepare(
                &gpu.device,
                &gpu.queue,
                gpu.config.width,
                gpu.config.height,
                bar.as_slice(),
            );
        }
        // THE RECTANGLE FIRST, THEN THE TEXT. There is no depth test: whoever
        // draws later is on top, and a bar over a letter would hide it.
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
        let cell = f64::from(tokens::metrics::cell_width(tokens::font::SIZE_BASE));
        if cell <= 0.0 || x < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let which = (x / cell) as u16;
        which.checked_sub(CONTENT_MARGIN)
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
        Keysym::Tab => Press::Tab,
        // `ISO_Left_Tab` IS `S-TAB`: xkb has already applied the shift, and
        // this is the one keysym it gives back for it.
        Keysym::ISO_Left_Tab => Press::BackTab,
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
            crate::chrome::list_scroll(self.rows_now(), self.room(), self.offset)
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
                    clicked = self
                        .row_under_pointer
                        .map(|r| (r, self.cell_at(e.position.0)));
                }
                // THE RIGHT BUTTON DOES NOTHING HERE. There is no menu in this
                // view: every action it offers is one key, and a second meaning
                // for the same target would be chrome nobody asked for.
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
        if turned {
            let by = crate::surface::wheel_lines(&mut self.lines_owed);
            if by == 0 {
                // WHAT IS LEFT OVER STAYS. A gesture too small to be a line
                // yet is not a gesture that did nothing — it is one that has
                // not finished adding up.
                return;
            }
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
        // NO FRAME FOR A POINTER THAT CHANGED NOTHING. Nothing on this
        // screen is drawn from `row_under_pointer` — the click reads it, and
        // the paint does not — so repainting when it moved spent a GPU frame
        // per row crossed, on an identical picture. The footer's own hover
        // band IS drawn, which is why it repaints there and not here.
        let _ = before;
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
