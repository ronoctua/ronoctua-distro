//! The power view's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a ceiling in lines (`space.expansion`,
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
    let texts = std::rc::Rc::new(crate::language::texts());
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

    // THE LANGUAGE'S FILE, watched for the theme's reason and by the theme's
    // means: a language chosen in the footer has to reach this window
    // without it being reopened. A door that adopted it only at its next
    // start is the incomplete preview the owner read off the screen when
    // the theme did exactly that.
    let (language_sender, language_changed) = channel::channel::<()>();
    let _language_watch = crate::language::watch(move || language_sender.send(()).is_ok());
    handle
        .insert_source(language_changed, |event, (), state: &mut PowerWindow| {
            if let channel::Event::Msg(()) = event {
                state.texts = std::rc::Rc::new(crate::language::texts());
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the language channel: {e}"))?;

    // AND THE SIZE OF THE TEXT, watched for the same reason and by the same
    // means. A size chosen anywhere moves this window without it being
    // reopened — and without it this door would sit beside the footer at a
    // different size, which is the photograph that made this system have one
    // size at all.
    let (size_sender, size_changed) = channel::channel::<()>();
    let _size_watch = crate::toolkits::watch(move || size_sender.send(()).is_ok());
    handle
        .insert_source(size_changed, |event, (), state: &mut PowerWindow| {
            if let channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                state.text_size_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    // AND `power.json`, watched like the theme: a profile chosen in the
    // footer moves this window's "(your choice)" mark without it being
    // reopened. The footer is the one that APPLIES the file; here it is read.
    let (wish_sender, wish_changed) = channel::channel::<()>();
    let _power_watch = ronoctua_power::prefs::home().and_then(|dir| {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(error = %e, "no configuration directory to watch power.json in");
            return None;
        }
        ronoctua_files::notice::Notice::on_file(
            &dir,
            ronoctua_power::prefs::FILE,
            ronoctua_files::notice::Pace::Coalesced,
            move || wish_sender.send(()).is_ok(),
        )
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
        keyboards: crate::seats::Seats::default(),
        key_here: Modifiers::default(),
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
        view: crate::power::View::fresh(),
        snapshot: None,
        power,
        wanted,
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
struct PowerWindow {
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
    keyboards: crate::seats::Seats<Modifiers, wl_keyboard::WlKeyboard>,
    /// The modifiers of the keyboard whose events are being handled — the
    /// per-seat state `keyboards` swaps in (`crate::seats`).
    key_here: Modifiers,
    /// Every pointer this window was offered — one per cursor — with the
    /// state of each while another is `here`. See `crate::seats`.
    pointers: crate::seats::Seats<PerSeat>,
    /// The state of the pointer whose events are being handled now.
    here: PerSeat,
    window: Window,
    /// The window's size in LOGICAL pixels — what the compositor configured
    /// it to, what the columns are counted from, what the pointer arrives in.
    width: u32,
    /// See [`PowerWindow::width`].
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
    view: crate::power::View,
    /// The last reading, from this process's own threads.
    snapshot: Option<ronoctua_power::Snapshot>,
    /// The orders thread, when the daemons were there.
    power: Option<ronoctua_power::Handle>,
    /// The profile the person chose (or the distro's floor) — drawn as a
    /// mark beside the profile rows.
    wanted: ronoctua_power::Profile,
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
    row_under_pointer: Option<usize>,
    /// Whether this pointer is dragging the header's drawer bar — the
    /// per-pointer half of `chrome::Hinted`, swapped in around every frame.
    hint_drag: bool,
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

    /// "The lock is the footer's to hold", with the two sequences RESOLVED.
    ///
    /// The keys are an ARGUMENT and never text in the `.ftl`: a sentence that
    /// carried them went stale the day either moved, and the word "LEADER" is
    /// nobody's key — `nix/modules/scroll.nix` says `SUPER` and `nix/dev.nix`
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
            // THE LOCKS ARE THE FOOTER'S TO HOLD: an inhibitor is an fd, and
            // this window will close. The command the footer runs for the same
            // key is what is asked for — one door, through the other.
            Errand::Order(ronoctua_power::Order::KeepAwake(keep, _)) => {
                // NOT `status()`, WHICH WAITS. This runs inside the calloop
                // callback, and waiting for a child there freezes the window
                // for as long as the child takes — the same trap `AGENTS.md`
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
        let Some(press) = press_of(&event) else {
            return;
        };
        self.message = None;
        // THE WIDTH THIS WINDOW DRAWS AT, so the key acts on the row on
        // screen — see `power::View::key`.
        let columns = self.columns;
        let errand = self.view.key(
            press,
            self.snapshot.as_ref(),
            self.wanted,
            columns,
            &self.texts,
        );
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
            || (self.view.hint(&self.texts), true),
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
        if cap == Capability::Keyboard {
            // A KEYBOARD THAT REPEATS — see the `handle` field. The rate and
            // the delay are the COMPOSITOR's, arriving as `RepeatInfo`;
            // nothing here invents either.
            match self.seats.get_keyboard_with_repeat(
                qh,
                &seat,
                None,
                self.handle.clone(),
                Box::new(|state: &mut Self, kb, event| {
                    state.keyboards.switch_to(kb, &mut state.key_here);
                    state.typed(event);
                }),
            ) {
                Ok(k) => {
                    // EVERY KEYBOARD, and not only the first: one per seat, and in
                    // Independent mode one per cursor.
                    let name = self.seats.info(&seat).and_then(|i| i.name);
                    self.keyboards.took(&seat, k, name.as_deref());
                }
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
        if cap == Capability::Keyboard {
            self.keyboards.lost(&seat, &mut self.key_here);
        }
        if cap == Capability::Pointer {
            self.pointers.lost(&seat, &mut self.here);
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for PowerWindow {
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
        self.keyboards.switch_to(kb, &mut self.key_here);
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
        // A MODIFIER HELD WHEN THE FOCUS WENT IS A MODIFIER NOBODY WILL SEE
        // COME UP — the release goes to whoever has the keyboard now.
        self.hinted.let_everything_go();
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
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
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
        // THE MODIFIER IS LET GO HERE, and it has to be: xkb merges both
        // Ctrls into one bit, so the modifier event cannot say which of them
        // came up.
        self.hinted.took_a_key(event.keysym, false);
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        _: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
    }
}

impl PointerHandler for PowerWindow {
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

impl PowerWindow {
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
            self.view.clicked(row, &rows);
            self.follow(rows.len());
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

impl CompositorHandler for PowerWindow {
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
