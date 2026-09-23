//! Syncthing, in a window of its own — the second door onto `crate::syncthing`.
//!
//! # What is here and what is not
//!
//! The plumbing of a window: a surface, a keyboard that repeats, a pointer, a
//! scrollbar, and the four files that make a window follow the theme, the
//! language, the size of the text and its own settings. It DECIDES nothing —
//! which rows exist and what a key does to them is `syncthing.rs`, and both
//! doors ask the same view the same questions.
//!
//! It is `audio_window.rs` with the view swapped, deliberately: ten doors that
//! drifted apart would be ten answers to how a wheel scrolls.
//!
//! # The one thing this door CANNOT do
//!
//! **Type.** Adding a machine means a 63-character id, and adding a folder
//! means a path; the one place anything is typed in this system is the
//! footer's minibuffer, and this window has none. So it forwards the request
//! down the control socket and SAYS SO on its message line — the rule the
//! network view's password and the glance's time zone already follow.

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
pub const APP_ID: &str = "br.ronoctua.syncthing";

/// `BTN_LEFT`, from `linux/input-event-codes.h`.
const LEFT_BUTTON: u32 = 0x110;

/// The first line of the list, before the explanation takes its own.
///
/// Everything asks [`Self::first_of_list`] instead: while the pointer rests on
/// a shortcut of the hint, the sentence about it is drawn on the line under
/// the hint and the list starts one row lower. A window is not anchored to the
/// bottom of the screen, so that costs a row of list and nothing else — the
/// footer, which is anchored, pays for the same line another way (see
/// `Footer::explain_rows`).
const FIRST_OF_LIST: u16 = 1;

/// The margin inside the window, in cells — see `chrome::FIELDS_LEAD`.
const CONTENT_MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// Opens the Syncthing view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<SyncthingWindow> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;

    let surface_of = compositor.create_surface(&qh);
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    let texts = std::rc::Rc::new(crate::language::texts());
    window.set_title(format!(
        "*{}*",
        texts.t(ronoctua_i18n::key::SYNCTHING_TITLE)
    ));
    window.set_app_id(APP_ID);
    window.commit();

    let mut cycle: EventLoop<SyncthingWindow> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();

    // THE THEME'S FILE, watched, so a choice made in the footer recolours this
    // window without it being reopened.
    let (theme_sender, theme_changed) = channel::channel::<()>();
    let _theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    handle
        .insert_source(theme_changed, |event, (), state: &mut SyncthingWindow| {
            if let channel::Event::Msg(()) = event
                && crate::theme::reload()
            {
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the theme channel: {e}"))?;

    // THE LANGUAGE'S FILE, watched for the theme's reason and by the theme's
    // means: a language chosen in the footer has to reach this window
    // without it being reopened. A door that adopted it only at its next
    // start is the incomplete preview the owner read off the screen when
    // the theme did exactly that.
    let (language_sender, language_changed) = channel::channel::<()>();
    let _language_watch = crate::language::watch(move || language_sender.send(()).is_ok());
    handle
        .insert_source(
            language_changed,
            |event, (), state: &mut SyncthingWindow| {
                if let channel::Event::Msg(()) = event {
                    state.texts = std::rc::Rc::new(crate::language::texts());
                    state.ask_paint();
                }
            },
        )
        .map_err(|e| anyhow::anyhow!("could not hang the language channel: {e}"))?;

    // AND THE SIZE OF THE TEXT, watched for the same reason and by the same
    // means. A size chosen anywhere moves this window without it being
    // reopened — and without it this door would sit beside the footer at a
    // different size, which is the photograph that made this system have one
    // size at all.
    let (size_sender, size_changed) = channel::channel::<()>();
    let _size_watch = crate::toolkits::watch(move || size_sender.send(()).is_ok());
    handle
        .insert_source(size_changed, |event, (), state: &mut SyncthingWindow| {
            if let channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                state.text_size_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    // AND `syncthing.json`, watched like the theme: a setting changed in the
    // footer moves this window without it being reopened. Both doors write
    // this file, which is why it is read back rather than assumed.
    let (wish_sender, wish_changed) = channel::channel::<()>();
    let _prefs_watch = ronoctua_syncthing::prefs::home().and_then(|dir| {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(error = %e, "no configuration directory to watch syncthing.json in");
            return None;
        }
        ronoctua_files::notice::Notice::on_file(
            &dir,
            ronoctua_syncthing::prefs::FILE,
            ronoctua_files::notice::Pace::Coalesced,
            move || wish_sender.send(()).is_ok(),
        )
        .ok()
    });
    handle
        .insert_source(wish_changed, |event, (), state: &mut SyncthingWindow| {
            if let channel::Event::Msg(()) = event {
                let fresh = ronoctua_syncthing::prefs::store().read();
                if fresh != state.view.preferences() {
                    state.view.took_preferences(fresh);
                    state.ask_paint();
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the syncthing.json watch: {e}"))?;

    // SYNCTHING, on threads of this process — the long poll is the clock.
    let (syncthing_sender, syncthing_events) = channel::channel::<ronoctua_syncthing::Event>();
    let syncthing = Some(ronoctua_syncthing::watch(move |e| {
        let _ = syncthing_sender.send(e);
    }));
    handle
        .insert_source(
            syncthing_events,
            |event, (), state: &mut SyncthingWindow| {
                if let channel::Event::Msg(e) = event {
                    state.syncthing_event(e);
                }
            },
        )
        .map_err(|e| anyhow::anyhow!("could not hang the syncthing channel: {e}"))?;

    let mut state = SyncthingWindow {
        conn: conn.clone(),
        handle: handle.clone(),
        _theme_watch,
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboard: None,
        pointers: crate::seats::Seats::default(),
        here: PerSeat::default(),
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
        quit: false,
        owed: false,
        gpu: None,
        text: None,
        quads: None,
        mods: Modifiers::default(),
        view: crate::syncthing::View::fresh(ronoctua_syncthing::prefs::store().read()),
        snapshot: None,
        syncthing,
        texts,
        hinted: crate::chrome::Hinted::default(),
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
struct SyncthingWindow {
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
    /// Every pointer this window was offered — one per cursor — with the
    /// state of each while another is `here`. See `crate::seats`.
    pointers: crate::seats::Seats<PerSeat>,
    /// The state of the pointer whose events are being handled now.
    here: PerSeat,
    window: Window,
    /// The window's size in LOGICAL pixels — what the compositor configured
    /// it to, what the columns are counted from, what the pointer arrives in.
    width: u32,
    /// See [`SyncthingWindow::width`].
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
    /// What the seat last said was held. See `update_modifiers`.
    mods: Modifiers,
    view: crate::syncthing::View,
    /// The last reading, from this process's own threads.
    snapshot: Option<ronoctua_syncthing::Snapshot>,
    /// The orders thread, when the sound server was there.
    syncthing: Option<ronoctua_syncthing::Handle>,
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    /// The hint line, and the shortcut the pointer is resting on it.
    hinted: crate::chrome::Hinted,
    /// The first visible row. A window has no page: it scrolls.
    offset: usize,
    message: Option<String>,
}

/// What this window keeps for ONE pointer — see `crate::seats`.
#[derive(Debug, Default)]
struct PerSeat {
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
    row_under_pointer: Option<usize>,
    /// Whether this pointer is dragging the header's drawer bar — the
    /// per-pointer half of `chrome::Hinted`, swapped in around every frame.
    hint_drag: bool,
}

impl SyncthingWindow {
    fn syncthing_event(&mut self, e: ronoctua_syncthing::Event) {
        match e {
            ronoctua_syncthing::Event::Snapshot(s) => {
                self.snapshot = Some(*s);
                self.view
                    .settle_in(self.snapshot.as_ref(), self.columns, &self.texts);
                self.ask_paint();
            }
            ronoctua_syncthing::Event::Done { refused, .. } => {
                let Some(why) = refused else { return };
                self.message = Some(self.texts.t(crate::syncthing::refusal_key(why)));
                self.ask_paint();
            }
        }
    }

    /// How many rows fit under the hint line.
    fn room(&self) -> usize {
        self.lines.saturating_sub(self.first_of_list()) as usize
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
    fn apply(&mut self, errand: crate::syncthing::Errand) {
        use crate::syncthing::Errand;
        match errand {
            Errand::Nothing => {}
            // AN INFORMATIVE ROW COPIES ITSELF — `chrome::information_of`,
            // and `chrome::copied` for the one place the doors share. It earns
            // its keep here more than anywhere: a machine id is 63 characters
            // nobody types twice.
            Errand::Copy(text) => {
                self.message = Some(crate::chrome::copied(&text, &self.texts));
            }
            Errand::Speak(key) => {
                self.message = Some(self.texts.t(key));
            }
            Errand::Close => {
                self.quit = true;
                return;
            }
            Errand::Order(order) => match &self.syncthing {
                Some(h) => h.send(*order),
                None => {
                    self.message =
                        Some(self.texts.t(ronoctua_i18n::key::ECHO_SYNCTHING_NOT_RUNNING));
                }
            },
            // THE FILE IS WRITTEN HERE, and the footer watches it. Both doors
            // write it, which is why the crate's own `keep` is what decides
            // whether there is anything to overwrite — `Store::parsed`, not
            // `read`, and the reason is that a reader's default written back
            // over a file that will not parse is that file gone.
            Errand::Keep(prefs) => {
                let store = ronoctua_syncthing::prefs::store();
                if let Err(e) = ronoctua_syncthing::prefs::keep(&store, prefs) {
                    tracing::warn!(error = %e, "could not write syncthing.json");
                    self.message = Some(
                        self.texts
                            .t(ronoctua_i18n::key::ECHO_SYNCTHING_WILL_NOT_PARSE),
                    );
                }
            }
            Errand::Web(address) => self.open_the_page(&address),
            // TYPING IS THE FOOTER'S, and this window says so. Sending the
            // request down the control socket would open the minibuffer over
            // there while the person is looking at here, which is worse than
            // a sentence: it is a prompt on another screen.
            Errand::Ask(_) => {
                self.message = Some(
                    self.texts
                        .t(ronoctua_i18n::key::SYNCTHING_TYPED_IN_THE_FOOTER),
                );
            }
        }
        self.ask_paint();
    }

    /// Open Syncthing's own page in whatever opens links here.
    fn open_the_page(&mut self, address: &str) {
        let Some(opener) = option_env!("RONOCTUA_XDG_OPEN").filter(|p| !p.is_empty()) else {
            tracing::error!("RONOCTUA_XDG_OPEN was not set at build time");
            self.message = Some(self.texts.t(ronoctua_i18n::key::ECHO_SYNCTHING_STOPPED));
            return;
        };
        if address.is_empty() {
            self.message = Some(self.texts.t(ronoctua_i18n::key::ECHO_SYNCTHING_STOPPED));
            return;
        }
        // DETACHED, and never waited on: this runs inside the calloop
        // callback, and a browser that takes a second to start would freeze
        // the window for that second.
        match std::process::Command::new(opener)
            .arg(format!("http://{address}"))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => drop(child),
            Err(e) => tracing::warn!(error = %e, "could not open the page"),
        }
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
        let Some(press) = press_of(&event, self.mods.shift) else {
            return;
        };
        self.message = None;
        let errand = self
            .view
            .key(press, self.snapshot.as_ref(), self.columns, &self.texts);
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
            // AND ONCE MORE IF THE HEADER'S DRAWER JUST OPENED: the list's
            // first line is asked for before the frame and the wrap is only
            // known inside it. It answers once and then never again, so
            // nothing loops — see `fold::Fold::held`.
            if self.hinted.settled() {
                self.paint();
            }
        }
    }

    fn rows(&self) -> Vec<crate::syncthing::Row> {
        self.view
            .rows(self.snapshot.as_ref(), self.columns, &self.texts)
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
        // A MESSAGE TAKES THIS LINE, and then there is no shortcut on it to
        // point at. Decided BEFORE the list is measured, because the
        // explanation's row comes out of that measurement — see
        // `Self::first_of_list`. Left until the drawing, the list would be
        // measured against a row that the frame then does not draw.
        if self.message.is_some() {
            self.hinted.say(None);
        }
        let rows = self.rows();
        self.settle_offset(rows.len());

        let mut spans: Vec<ronoctua_render::Span> = Vec::new();

        // LINE 0: what the last order said, or the keys — the footer's
        // minibuffer, through the other door. `columns` is already net of both
        // margins (`recount`), so the whole of it is the room.
        let room = self.columns;
        // LINE 0 AND THE DRAWER IT OPENS INTO: the view's shortcuts or what
        // the last order said, the chevron on the right edge when the line
        // does not fit, the wrapped rest of it once that has been pressed,
        // and the sentence about the shortcut the pointer is resting on.
        //
        // ONE PAINTER for every door of this shape — see `chrome::Hinted`:
        // this was written out in each of the ten windows, and the drawer was
        // written in none of them.
        let (pieces, hinting) = self.message.clone().map_or_else(
            || {
                (
                    self.view.hint(self.snapshot.as_ref(), room, &self.texts),
                    true,
                )
            },
            |text| (vec![crate::chrome::Piece::said(text)], false),
        );
        spans.extend(
            self.hinted
                .head(&pieces, hinting, margin, room, &self.texts),
        );

        // THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
        // draws for these rows.
        for (i, row) in rows.iter().skip(self.offset).take(self.room()).enumerate() {
            let y = self.first_of_list() + u16::try_from(i).unwrap_or(0);
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
        // EVERY CURSOR'S HOVER, in its own colour — see `crate::seats`.
        #[allow(clippy::cast_precision_loss)]
        let bands = crate::chrome::hover_bands(
            self.pointers
                .lit(&self.here)
                .map(|(color, s)| (color, s.row_under_pointer)),
            self.offset,
            self.room(),
            self.rows().len(),
            self.first_of_list(),
            self.width as f32,
        );
        let mut bar: Vec<ronoctua_render::quad::Quad> = crate::chrome::list_bar(
            scroll,
            self.first_of_list(),
            self.width as f32,
            self.height as f32,
        )
        .into_iter()
        .collect();
        bar.splice(0..0, bands);
        // AND THE HEADER'S OWN, when its drawer is open and the wrap ran past
        // it. Two bars, two lists, and neither is the other's.
        #[allow(clippy::cast_precision_loss)]
        bar.extend(self.hinted.bar(crate::chrome::Frame::of(
            self.width as f32,
            self.height as f32,
            0,
        )));

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
        let (width, height) = (self.width, self.height);
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
            // LOGICAL, where the text above took the PHYSICAL surface.
            //
            // A `Quad` is a rectangle in the same pixels `list_bar` measured
            // it in, and those are the window's own — `self.width`. The
            // renderer turns it into NDC by dividing, so a logical rectangle
            // over a logical viewport lands exactly where a physical one over
            // a physical viewport would. Passing the physical size here with
            // logical rectangles is what would put the scrollbar at half its
            // place on a scale-2 screen.
            q.prepare(&gpu.device, &gpu.queue, width, height, bar.as_slice());
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

    /// The first line of the list, pushed down by the explanation line.
    fn first_of_list(&self) -> u16 {
        FIRST_OF_LIST + self.hinted.rows()
    }

    /// Which drawn row is at height `y`.
    fn row_at(&self, y: f64) -> Option<usize> {
        let line = f64::from(tokens::metrics::line_height(tokens::text::size()));
        if line <= 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let which = (y / line) as usize;
        which
            .checked_sub(usize::from(self.first_of_list()))
            .map(|i| i + self.offset)
    }
}

/// A `KeyEvent` as the view sees it — the same mapping the footer does.
///
/// BOTH DOORS GO THROUGH `syncthing::OF_THE_VIEW`. This one turns SCTK's keysym
/// into the table's notation and asks the view; the footer turns its own
/// `Key` into the same notation and asks the same question. A key written in
/// only one of the two would be a key one door answers and the other does
/// not, which is the defect `bluetooth-keys-reach-both-doors` was written for.
fn press_of(event: &KeyEvent, shifted: bool) -> Option<crate::syncthing::Press> {
    // THE NOTATION CARRIES SHIFT, unlike the sound window's. This view's value
    // keys ARE the shifted arrows, and a notation built without the modifier
    // turns `S-<right>` into a plain one — which walks the list instead of
    // turning the ring, silently.
    let notation = match event.keysym {
        Keysym::Escape => return Some(crate::syncthing::Press::Escape),
        Keysym::Return | Keysym::KP_Enter => "RET".to_owned(),
        Keysym::Up => "<up>".to_owned(),
        Keysym::Down => "<down>".to_owned(),
        Keysym::Left | Keysym::Right => {
            let arrow = if event.keysym == Keysym::Left {
                "<left>"
            } else {
                "<right>"
            };
            if shifted {
                format!("S-{arrow}")
            } else {
                arrow.to_owned()
            }
        }
        _ => event.utf8.as_deref()?.to_owned(),
    };
    crate::syncthing::pressed(&notation)
}

impl WindowHandler for SyncthingWindow {
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

impl SeatHandler for SyncthingWindow {
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
        if cap == Capability::Pointer {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => {
                    // EVERY POINTER, and not only the first: one per cursor.
                    let name = self.seats.info(&seat).and_then(|i| i.name);
                    self.pointers.took(&seat, p, name.as_deref());
                }
                Err(e) => tracing::warn!(error = %e, "no pointer; keyboard only"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard
            && let Some(k) = self.keyboard.take()
        {
            k.release();
        }
        if cap == Capability::Pointer {
            self.pointers.lost(&seat, &mut self.here);
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for SyncthingWindow {
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
        // A MODIFIER HELD WHEN THE FOCUS WENT IS A MODIFIER NOBODY WILL SEE
        // COME UP — the release goes to whoever has the keyboard now.
        self.hinted.let_everything_go();
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        // THE HEADER'S OWN KEYS FIRST — the right `Ctrl` and its pair, and the
        // `Esc` that shuts an open drawer before it closes the window. See
        // `chrome::Hinted::took_a_key`.
        if self.hinted.took_a_key(event.keysym, true) {
            self.ask_paint();
            return;
        }
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
        event: KeyEvent,
    ) {
        // THE MODIFIER IS LET GO HERE, and it has to be: xkb merges both
        // Ctrls into one bit, so the modifier event cannot say which of them
        // came up.
        self.hinted.took_a_key(event.keysym, false);
    }

    /// THE MODIFIERS ARE NOT ON THE KEY EVENT, and this view needs one.
    ///
    /// SCTK's `KeyEvent` carries the keysym, the raw code and the text — the
    /// seat announces the modifiers on their own. Every other window in this
    /// system can leave this empty because none of their keys is a chord; this
    /// one's value keys ARE `S-<arrows>`, so the state is kept.
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

impl PointerHandler for SyncthingWindow {
    /// EVERY CURSOR HAS ITS OWN STATE: the pointer that spoke is made `here`
    /// first, so a drag, a hover or a wheel's leftover never crosses from
    /// one cursor to another. See `crate::seats`.
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        self.pointers.switch_to(pointer, &mut self.here);
        self.hinted.swap_hand(&mut self.here.hint_drag);
        self.pointer_frame_here(conn, qh, events);
        self.hinted.swap_hand(&mut self.here.hint_drag);
    }
}

impl SyncthingWindow {
    /// One pointer's frame, with its state already `here`.
    fn pointer_frame_here(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        events: &[PointerEvent],
    ) {
        let before = self.here.row_under_pointer;
        let mut clicked: Option<usize> = None;
        let mut turned = false;
        // THE BAR IS A CONTROL, and a drag is three events. `Scroll::offset_at`
        // maps any point of the TRACK, so nobody has to aim at the handle.
        // THE ROWS ARE ONLY BUILT WHEN THE BAR IS IN PLAY.
        //
        // A `Motion` arrives on every pixel of movement, and these windows
        // deliberately skip a REPAINT on motion because it "spent a GPU frame
        // per row crossed". Building the whole list to answer "is the pointer
        // on the bar?" would spend the more expensive half of that anyway.
        let bar_in_play = self.here.dragging
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
        // THE HEADER'S OWN SURFACE, for its chevron and its bar. Line 0 is
        // where its drawer begins, which is what the `0` says.
        let head = crate::chrome::Frame::of(bar_w, bar_h, 0);
        // WHETHER THE HEADER TOOK THE GESTURE. Asked before the list's own
        // bar and before a click on a row, so each control keeps the meaning
        // it has.
        let mut header = false;
        // WHERE THE WHEEL WAS, so the drawer can answer for a gesture over
        // itself. The turn is accumulated and spent after the loop, and by
        // then the position is gone.
        let mut wheel_at: Option<f64> = None;
        let mut dragged: Option<usize> = None;
        // WHETHER THE LINE UNDER THE HINT CHANGED, which is the one thing
        // the pointer does draw in this door.
        let mut explained = false;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    // THE HEADER'S BAR IS BEING DRAGGED, and then nothing
                    // else on this surface is.
                    if self.hinted.dragged(head, e.position.1) {
                        header = true;
                        continue;
                    }
                    // WHICH SHORTCUT OF THE HINT LINE the pointer is resting
                    // on, read off what the last frame DREW — see
                    // `chrome::Hinted`.
                    let now = self
                        .hinted
                        .under(e.position.0, e.position.1)
                        .and_then(|id| crate::chrome::hint_sentence(id, &self.texts));
                    explained |= self.hinted.say(now);
                    #[allow(clippy::cast_possible_truncation)]
                    if self.here.dragging {
                        dragged = Some(crate::chrome::list_offset_at(
                            scroll,
                            self.first_of_list(),
                            bar_w,
                            bar_h,
                            e.position.1 as f32,
                        ));
                    } else {
                        self.here.row_under_pointer = self.row_at(e.position.1);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    // AND THE HEADER'S DRAG ENDS WITH THE POINTER, for the
                    // reason the list's does.
                    self.hinted.let_go();
                    // THE SENTENCE GOES WITH THE POINTER. A pointer that
                    // leaves sends no motion on the way out.
                    explained |= self.hinted.say(None);
                    // AND THE DRAG ENDS WITH THE POINTER. Wayland's implicit
                    // grab usually delivers the release, but a popup grab, an
                    // unmap or a broken grab drops it — and a `dragging` left
                    // true makes the next ENTER, with no button held, jump the
                    // list to wherever the pointer came in.
                    self.here.dragging = false;
                    self.here.row_under_pointer = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    self.hinted.let_go();
                    self.here.dragging = false;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    // THE HEADER'S CONTROLS FIRST: the chevron that opens the
                    // line, and the bar that walks it once it is open.
                    if self.hinted.pressed(head, e.position.0, e.position.1) {
                        header = true;
                        continue;
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    if crate::chrome::on_the_list_bar(
                        scroll,
                        self.first_of_list(),
                        bar_w,
                        bar_h,
                        e.position.0 as f32,
                        e.position.1 as f32,
                    ) {
                        self.here.dragging = true;
                        dragged = Some(crate::chrome::list_offset_at(
                            scroll,
                            self.first_of_list(),
                            bar_w,
                            bar_h,
                            e.position.1 as f32,
                        ));
                        continue;
                    }
                    self.here.row_under_pointer = self.row_at(e.position.1);
                    clicked = self.here.row_under_pointer;
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
                    crate::surface::wheel_arrived(
                        &mut self.here.lines_owed,
                        &vertical,
                        &horizontal,
                    );
                    wheel_at = Some(e.position.1);
                    turned = true;
                }
                _ => {}
            }
        }
        if header {
            self.ask_paint();
            return;
        }
        if let Some(offset) = dragged {
            self.offset = offset;
            self.ask_paint();
            return;
        }
        // THE MOUSE AIMS, THE KEYS ACT — a click puts the cursor on the row,
        // and `RET` is what applies, arms and confirms.
        if let Some(row) = clicked {
            let rows = self.rows();
            self.view.clicked(
                row,
                &rows,
                self.snapshot.as_ref(),
                self.columns,
                &self.texts,
            );
            // THE ROW COUNT MAY HAVE MOVED, because clicking a folder or a
            // drawer changes which level is showing. `self.rows()` above is
            // the list BEFORE that, so the list is asked again.
            let rows = self.rows().len();
            self.follow(rows);
            self.ask_paint();
            return;
        }
        if turned {
            let by = crate::surface::wheel_lines(&mut self.here.lines_owed);
            // A WHEEL OVER THE OPEN DRAWER WALKS THE DRAWER, and not the list
            // under it: the gesture belongs to what is under the pointer.
            if by != 0 && wheel_at.is_some_and(|y| self.hinted.turned(head, y, by)) {
                self.ask_paint();
                return;
            }
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
        // EXCEPT THE LINE UNDER THE HINT, which IS drawn from the pointer:
        // resting on a shortcut spells it out, and moving off it takes the
        // sentence away. The ROW under the pointer is the
        // paragraph below.
        if explained {
            self.ask_paint();
        }
        // A FRAME ONLY WHEN SOMETHING THAT IS DRAWN MOVED. The row under the
        // pointer is drawn now — every cursor's in its own colour — so a new
        // row is a new picture; a pointer moving inside the same row is not.
        if before != self.here.row_under_pointer {
            self.ask_paint();
        }
    }
}

impl CompositorHandler for SyncthingWindow {
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

impl OutputHandler for SyncthingWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for SyncthingWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(SyncthingWindow);
smithay_client_toolkit::delegate_dispatch2!(SyncthingWindow);
