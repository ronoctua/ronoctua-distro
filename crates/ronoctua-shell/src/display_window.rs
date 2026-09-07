//! The screens view's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a twelve-line ceiling (`space.expansion`,
//! invariant 14), and one screen's sheet is eighteen rows: a glance at what is
//! plugged in fits the footer, reading a monitor whole does not. So this
//! exists — and everything it knows lives in [`crate::display::View`], which
//! the footer draws from too. What this module holds is a window, a GPU and
//! the events of both; what it decides is nothing. It is the arrangement
//! `bluetooth_window.rs` and `power_window.rs` share, and the same header.
//!
//! # Where its data comes from
//!
//! From niri and the kernel, on a thread of its own
//! (`ronoctua_display::watch`), and orders go to it directly: a mode, a scale,
//! a brightness are the same call from any process.
//!
//! # THE ONE THING THIS DOOR HAS THAT THE OTHERS DO NOT: A CLOCK
//!
//! FOUR of the other nine doors keep one UNCONDITIONALLY, for the life of the
//! window: the network's and the update's beat once a second, the notification
//! manager's every two, and the buffer host's once a second whenever the
//! memory panel is what it is hosting. The file manager's arms one ONLY when
//! its inotify watcher failed to start, and says so beside it; the power
//! window, the radio's and the terminal have none. This one is armed only
//! while there is something to count, and disarms itself — which is the
//! difference worth writing down, and the count beside `arm_the_clock` says
//! the same number.
//!
//! What it counts is the COUNTDOWN: a change that can leave a screen
//! unreadable is applied provisionally and goes back by itself if nobody
//! answers. The footer's clock is no help — `Footer::display_tick` returns at
//! once unless the FOOTER's own view is open, and the two processes hold
//! separate `View`s — so without a clock here the question would stand for
//! ever, saying the same number, and the change would never be put back at
//! all.
//!
//! ARMED ONLY WHILE ONE RUNS: `Asking` says whether it is on the clock (the
//! forget question is not), and the tick does not re-arm when the question has
//! gone. In repose this window wakes for nothing.

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
pub const APP_ID: &str = "br.ronoctua.display";

/// `BTN_LEFT`, from `linux/input-event-codes.h`.
const LEFT_BUTTON: u32 = 0x110;

/// The first line of the list. Above it: the hint line, and nothing else —
/// the arrangement the notification window has, for the reason it gives.
const FIRST_OF_LIST: u16 = 1;

/// The margin inside the window, in cells — see `chrome::FIELDS_LEAD`.
const CONTENT_MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// Opens the screens view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<DisplayWindow> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;

    let surface_of = compositor.create_surface(&qh);
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    let texts = std::rc::Rc::new(ronoctua_i18n::Texts::from_env());
    window.set_title(format!("*{}*", texts.t(ronoctua_i18n::key::DISPLAY_TITLE)));
    window.set_app_id(APP_ID);
    window.commit();

    let mut cycle: EventLoop<DisplayWindow> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();

    // THE THEME'S FILE, watched, so a choice made in the footer recolours this
    // window without it being reopened.
    let (theme_sender, theme_changed) = channel::channel::<()>();
    let _theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    handle
        .insert_source(theme_changed, |event, (), state: &mut DisplayWindow| {
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
    let size_watch = crate::toolkits::watch(move || size_sender.send(()).is_ok());
    handle
        .insert_source(size_changed, |event, (), state: &mut DisplayWindow| {
            if let channel::Event::Msg(()) = event {
                // THE FILE CARRIES TWO THINGS, and this watch used to answer
                // for one. `display.json` holds the size of the text AND every
                // screen's `Wanted`, so a scale kept in the FOOTER changed it
                // and this window changed nothing — and then wrote its own
                // stale copy back on the next keep, silently undoing the
                // footer's change. It is the second-writer defect CLAUDE.md
                // records for the notification history, one file over.
                if crate::toolkits::reload() {
                    state.text_size_changed();
                }
                state.screens_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    // THE READER, on a thread of this process. It is the same `watch` the
    // footer starts: niri over the IPC, the backlights out of `/sys`, and the
    // monitors over DDC/CI when a reading asks deeply.
    let (screens_sender, screens_events) = channel::channel::<ronoctua_display::Event>();
    let screens = match ronoctua_display::watch(move |e| {
        let _ = screens_sender.send(e);
    }) {
        Ok(h) => Some(h),
        Err(e) => {
            tracing::warn!(error = %e, "no screen thread; the window opens with nothing to read");
            None
        }
    };
    handle
        .insert_source(screens_events, |event, (), state: &mut DisplayWindow| {
            if let channel::Event::Msg(e) = event {
                state.screens_event(e);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the screens channel: {e}"))?;

    // AND WHAT THE COMPOSITOR SAYS ABOUT ITS OWN CONFIGURATION.
    //
    // `Order::Reloaded` is the ONLY thing that sets `config_failed`, and it
    // only ever arrives from niri's event stream — a reading cannot tell,
    // because niri keeps running on the configuration it had. The footer
    // forwards it; without the same thread here, the door that WROTE the
    // fragment is the one door that could never say the compositor refused
    // it. The same listener, on this process's own connection.
    let (niri_sender, niri_events) = channel::channel::<ronoctua_niri::Event>();
    crate::daemon::listen_to_niri(niri_sender);
    handle
        .insert_source(niri_events, |event, (), state: &mut DisplayWindow| {
            if let channel::Event::Msg(ronoctua_niri::Event::ConfigLoaded { failed }) = event
                && let Some(h) = state.screens.as_ref()
            {
                // NOT THROUGH `order`, which says "the screens cannot be read"
                // when there is no reader. Nobody asked for this — it is
                // niri talking — so a window with no reader has nothing to
                // report and should not replace whatever is on the message
                // line with a complaint about an event the person did not
                // cause. The footer's twin guards the same way.
                h.send(ronoctua_display::Order::Reloaded { failed });
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the niri channel: {e}"))?;

    // AND THE COUNTDOWN'S OWN SECOND. Four siblings keep a clock too; what is
    // different here is that it is armed only while there is something to
    // count — see this module's header.
    let (tick_sender, ticks) = channel::channel::<()>();
    handle
        .insert_source(ticks, |event, (), state: &mut DisplayWindow| {
            if let channel::Event::Msg(()) = event {
                state.countdown();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the countdown channel: {e}"))?;

    let mut state = DisplayWindow {
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
        quit: false,
        owed: false,
        gpu: None,
        text: None,
        quads: None,
        dragging: false,
        lines_owed: 0.0,
        view: {
            let mut v = crate::display::View::fresh();
            if screens.is_none() {
                // A VIEW WITH NO READER SAYS SO, rather than blaming the
                // compositor — the footer's door does the same.
                v.without_a_reader();
            }
            if size_watch.is_none() {
                // AND A DOOR THAT CANNOT SEE THE FILE CHANGE does not step the
                // size of the text — see `View::blind` for what it would ask
                // for instead.
                v.without_a_watch();
            }
            v
        },
        snapshot: ronoctua_display::Snapshot::default(),
        screens,
        tick: tick_sender,
        ticking: false,
        texts,
        row_under_pointer: None,
        offset: 0,
        message: None,
        said: 0,
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
struct DisplayWindow {
    conn: Connection,
    /// The loop, for the ONE thing that needs it here: a keyboard that
    /// REPEATS. SCTK only generates repeat events for a keyboard made with
    /// `get_keyboard_with_repeat`, and that wants a `LoopHandle` — so this is
    /// carried rather than reached for. Without it `repeat_key` below is dead
    /// code and holding an arrow moves exactly one row, which is the trap
    /// CLAUDE.md records and which nine windows inherited.
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
    view: crate::display::View,
    /// The last reading, from this process's own threads.
    snapshot: ronoctua_display::Snapshot,
    /// The orders thread, when the reader started.
    screens: Option<ronoctua_display::Handle>,
    /// How the countdown wakes this window — see the module's header.
    tick: channel::Sender<()>,
    /// Whether a second is already on its way, so two questions in a row do
    /// not leave two timers running.
    ticking: bool,
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    row_under_pointer: Option<usize>,
    /// The first visible row. A window has no page: it scrolls.
    offset: usize,
    message: Option<String>,
    /// How many messages have been said, so a timer only clears its own.
    ///
    /// The footer's line fades after `ECHO_SECONDS` and this one did not: it
    /// sat until the next keypress, which on a window somebody is reading
    /// rather than typing in is for ever. `files_window::say` carries the same
    /// pair and the owner's report that produced it.
    said: u64,
}

impl DisplayWindow {
    fn screens_event(&mut self, e: ronoctua_display::Event) {
        match e {
            ronoctua_display::Event::Snapshot(now) => {
                // THE VIEW FOLLOWS THE NEW ROWS, the footer's reason: a
                // monitor unplugged takes its row away, and the list reorders
                // under a cursor standing in it.
                self.snapshot = *now;
                // AND THE VIEW KNOWS IT HAS BEEN READ TO. The first frame of
                // this window is drawn against a default `Snapshot`, whose
                // `compositor` is `false` — see `display::View::looked`.
                self.view.saw_a_reading();
                let rows = self.rows();
                self.view.settle(&self.snapshot, &rows);
                self.ask_paint();
            }
            ronoctua_display::Event::Done { order, refused } => {
                let Some(why) = refused else {
                    return;
                };
                // A PREVIEW THAT DID NOT HAPPEN TAKES ITS QUESTION WITH IT —
                // the footer's door carries the whole reason. The cursor is
                // settled here because `never_mind` restores the remembered
                // target and has no rows to look an index up in.
                if let ronoctua_display::Order::Try { at, ask } = &order {
                    let ask = *ask;
                    let at = at.clone();
                    // AND ONLY WHERE A QUESTION CAME DOWN. A refusal for a
                    // `Try` whose question was already answered matches
                    // nothing and changes no row, and settling and following
                    // there pulled the page back to the cursor while somebody
                    // was reading the bottom of a sheet.
                    if self.view.never_mind(&at, ask) {
                        let rows = self.rows();
                        self.view.settle(&self.snapshot, &rows);
                        // AND THE PAGE FOLLOWS. The question is five rows and
                        // the sheet under it eighteen: settling the cursor
                        // without moving the page leaves the restored row off
                        // the glass until the next keypress — and nobody is
                        // pressing, which is what a refusal arriving by itself
                        // means. The footer's twin and the countdown beside
                        // this both do it.
                        let n = rows.len();
                        self.follow(n);
                    }
                }
                if matches!(order, ronoctua_display::Order::Text(_)) {
                    self.view.text_never_landed();
                }
                // AND A PLACE THAT NEVER LANDED — the footer's twin carries
                // the whole reason.
                if let ronoctua_display::Order::Keep { at, wanted } = &order {
                    self.view.place_never_landed(at, wanted);
                }
                let said = crate::display_words::refused(&why, &self.texts);
                self.say(said);
                self.ask_paint();
            }
        }
    }

    /// One second of a standing countdown — see this module's header.
    ///
    /// IT REARMS ITSELF, and only while there is something to count. The
    /// alternative is a window that wakes once a second for the life of the
    /// session to redraw a screen nothing is changing on.
    fn countdown(&mut self) {
        self.ticking = false;
        let errand = self.view.ran_out(std::time::Instant::now());
        let ran_out = !matches!(errand, crate::display::Errand::Nothing);
        if ran_out {
            // AND IT SAYS SO. This is the door's one feature the other nine
            // do not have, and the only observable it had was the file NOT
            // changing — an absence, which a door whose clock never armed
            // satisfies just as well. `tests/display-vm.nix` reads this.
            tracing::info!("the countdown ran out; putting the change back");
        }
        // THE SECONDS ON THE SCREEN COUNT DOWN whether or not it ran out, so
        // the frame is drawn either way: a question that says "15s" for
        // fifteen seconds is a question nobody believes.
        self.apply(errand);
        // AND THE CURSOR SETTLES ON THE ROWS THAT COME BACK. `ran_out` calls
        // `answered`, which restores the remembered TARGET; only `View::key`
        // settles an INDEX, and nobody has pressed a key — that is the whole
        // situation this exists for. Without it the sheet came back with the
        // question's row 3 still in the cursor, lighting a fact four rows
        // above the knob the hand had turned, and the page did not move to it.
        //
        // THE ORDER IS NOT LOAD-BEARING: `ran_out` takes the question down
        // itself (`answered` does the `take`), so `rows()` yields the sheet
        // either way. The footer's twin settles before its errand and is
        // equally right.
        if ran_out {
            let rows = self.rows();
            self.view.settle(&self.snapshot, &rows);
            let n = rows.len();
            self.follow(n);
        }
        // NO RE-ARM HERE: `apply` is the one place that remembers, and it has
        // just run. A second call was a no-op on every reachable path —
        // `arm_the_clock` returns on `ticking` — and it contradicted the
        // comment in `apply` that says where the remembering lives.
    }

    /// Puts one second on the wire, when a question is waiting for it.
    ///
    /// A CHANNEL AND A THREAD rather than a `calloop` timer, because the
    /// timer would have to be inserted and removed from inside its own
    /// callback — and the thread is what every other one-shot in this
    /// repository already is.
    fn arm_the_clock(&mut self) {
        if self.ticking
            || !self
                .view
                .asking()
                .is_some_and(crate::display::Asking::on_the_clock)
        {
            return;
        }
        let wire = self.tick.clone();
        // NAMED, and a failure to spawn is not a panic: `thread::spawn` would
        // bring the window down mid-question with the provisional change
        // standing, which is the one outcome the countdown exists to prevent.
        // Every other thread in this tree is a named `Builder` for the first
        // reason and this one is for the second.
        match std::thread::Builder::new()
            .name("display-countdown".into())
            .spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let _ = wire.send(());
            }) {
            Ok(_) => self.ticking = true,
            Err(e) => tracing::warn!(error = %e, "no clock; the question will wait for a key"),
        }
    }

    /// Goes away, after everything this door asked for has been carried out.
    ///
    /// # Why it waits, and why the guard is not "is a question standing"
    ///
    /// A preview is a TRANSIENT change over niri's IPC, and the only thing
    /// that takes one down is this process's own thread saying the file back.
    /// Setting `quit` and returning dropped the sender and let `main` end, so
    /// a queued order was never carried out — and no door opened afterwards
    /// can help, because `previewed` is per-thread. The change stood until
    /// niri restarted.
    ///
    /// THE FIRST VERSION OF THIS GUARDED ON A STANDING QUESTION, and that is
    /// exactly the case that is already safe. The dangerous one is a question
    /// ANSWERED a moment ago: holding `Esc` out of a preview is three key
    /// repeats thirty milliseconds apart — answer, level, close — and the
    /// `Undo` the first one queued is hundreds of milliseconds of work. By the
    /// time `Close` arrives `asking` is `None` and the old guard skipped the
    /// wait. Answering "put it back" and then clicking the window's close
    /// button is the same race with a mouse.
    ///
    /// So it waits for whatever is outstanding, and only ADDS an `Undo` when a
    /// question is still standing — which is the one case where nothing has
    /// been queued yet.
    fn leave(&mut self) {
        if let Some(h) = self.screens.take() {
            let standing = self
                .view
                .asking()
                .is_some_and(crate::display::Asking::on_the_clock);
            if standing {
                tracing::info!("closing over a standing preview; putting it back first");
            }
            h.finish(standing.then_some(ronoctua_display::Order::Undo));
        }
        self.quit = true;
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, errand: crate::display::Errand) {
        use crate::display::Errand;
        match errand {
            Errand::Nothing => {}
            // AN INFORMATIVE ROW COPIES ITSELF — see `chrome::information_of`,
            // and `chrome::copied` for the one place the doors share.
            Errand::Copy(text) => {
                self.say(crate::chrome::copied(&text, &self.texts));
            }
            Errand::Close => {
                self.leave();
                return;
            }
            // THE SCREEN WENT AWAY under a standing question — the footer's
            // door says the same sentence and sends the same order.
            Errand::Gone(at, applied) => {
                let said = crate::display_words::went_away(&at, applied, &self.texts);
                self.say(said);
                // AND ONLY WHEN SOMETHING WAS APPLIED — see `Errand::Gone`.
                if applied {
                    self.order(ronoctua_display::Order::Undo);
                }
            }
            Errand::Order(order) => self.order(*order),
            // IN THE ORDER THEY WERE MADE, because the last one is the change
            // the person actually asked for — see `display::Target::First`.
            Errand::Orders(orders) => {
                for order in orders {
                    self.order(order);
                }
            }
        }
        // AND THE CLOCK IS ARMED IF A QUESTION WENT UP. Every gesture that
        // raises one comes through here, so this is the one place that has to
        // remember — see `arm_the_clock`.
        self.arm_the_clock();
        self.ask_paint();
    }

    /// Hands one order to the reading thread, or says there is none.
    fn order(&mut self, order: ronoctua_display::Order) {
        match &self.screens {
            Some(h) => h.send(order),
            None => {
                let said = self.texts.t(ronoctua_i18n::key::DISPLAY_NO_THREAD);
                self.say(said);
            }
        }
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
    /// under it — the reading thread brings a new snapshot while nobody is
    /// touching anything — and nothing else. It clamps; it does not follow,
    /// and there is no anchor here to honour.
    fn settle_offset(&mut self, rows: usize) {
        let room = self.room();
        if room == 0 {
            return;
        }

        self.offset = self.offset.min(rows.saturating_sub(room));
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
        let now = std::mem::take(&mut self.snapshot);
        let errand = self
            .view
            .key(press, &now, std::time::Instant::now(), &self.texts);
        self.snapshot = now;
        // THE CURSOR MOVED, so the list follows it — here, on the EVENT, and
        // never in the paint. See `settle_offset` for what following from a
        // paint cost.
        let rows = self.rows().len();
        self.follow(rows);
        self.apply(errand);
    }

    /// Puts a sentence on the message line, and takes it off again.
    ///
    /// **THE ONE PLACE** a message is set in this door, which is the shape the
    /// file manager's window arrived at the hard way: nine assignments there
    /// meant nine chances to forget what a message needs, and all nine had
    /// forgotten the same thing — the message never went away. The owner found
    /// it with `copy the path`, where the footer's `copied` fades after three
    /// seconds and that door's stayed for the rest of the session.
    ///
    /// # Why a one-shot timer, and not a clock
    ///
    /// Because the clock this window HAS is armed only while a countdown runs
    /// (see the module header), and a message can outlive one — or arrive with
    /// no question at all. A clock kept for the message would be a wake-up per
    /// second for the life of the window, which is the property the countdown
    /// is arranged to avoid. A one-shot keeps it exactly — a window with
    /// nothing to say wakes not once — and pays for the
    /// three seconds after a message, which is the only time there is anything
    /// to do.
    ///
    /// The same three seconds as the footer, from the same constant: a message
    /// that lingered here and not there would be two answers to "how long is a
    /// message on screen".
    fn say(&mut self, text: String) {
        tracing::info!(echo = %text, "message line");
        self.message = Some(text);
        self.said = self.said.wrapping_add(1);
        let mine = self.said;
        let armed = self.handle.insert_source(
            calloop::timer::Timer::from_duration(std::time::Duration::from_secs(
                crate::surface::ECHO_SECONDS,
            )),
            move |_, (), state: &mut Self| {
                // ONLY ITS OWN. Between arming and firing the line may have
                // been replaced or cleared by a key; wiping whatever is there
                // would take a newer sentence down early.
                if state.said == mine {
                    state.message = None;
                    state.ask_paint();
                }
                calloop::timer::TimeoutAction::Drop
            },
        );
        if let Err(e) = armed {
            // NOT SILENT, and the message STAYS. A line that outstays its
            // welcome is a smaller failure than one that vanishes because a
            // timer could not be hung, and the log says which happened.
            tracing::warn!(error = %e, "no timer for the message line; it will stay");
        }
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

    fn rows(&self) -> Vec<crate::display::Row> {
        self.view.rows(
            &self.snapshot,
            self.columns,
            std::time::Instant::now(),
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
                &self
                    .view
                    .hint(&self.snapshot, std::time::Instant::now(), &self.texts),
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
        // AND A FRAME THAT WENT OUT SAYS SO. `set_title` and `set_app_id` run
        // before the event loop, so the compositor lists this window whether
        // or not it ever drew — every assertion a VM could make about it was
        // an ABSENCE (no error sentence), and absences pass over a `paint`
        // whose body is deleted. `buffer.rs` logs the same line for the same
        // reason, and `tests/display-vm.nix` reads it.
        // AT DEBUG, which is where `toolkits.rs` put the decision: "a line per
        // keypress in the compositor's journal is noise". This fires on every
        // frame that goes out — every key, every wheel detent, every second of
        // a standing countdown — and the one reader that needs it
        // (`tests/display-vm.nix`) runs the unit at debug.
        tracing::debug!(
            width = self.width,
            height = self.height,
            columns = self.columns,
            rows = rows.len(),
            "the screens window drew"
        );
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "the flush failed");
        }
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
            .checked_sub(usize::from(FIRST_OF_LIST))
            .map(|i| i + self.offset)
    }
}

/// A `KeyEvent` as the view sees it — the same mapping the footer does.
fn press_of(event: &KeyEvent) -> Option<crate::display::Press> {
    use crate::display::Press;
    Some(match event.keysym {
        Keysym::Escape => Press::Escape,
        Keysym::Return | Keysym::KP_Enter => Press::Enter,
        Keysym::Up => Press::Up,
        Keysym::Down => Press::Down,
        // `←`/`→` MOVE THE VALUE, which this view has and the radio has not.
        Keysym::Left => Press::Less,
        Keysym::Right => Press::More,
        // AND THE BARE LETTERS, out of the view's own table. A letter with no
        // press there is a letter that does nothing, and the test beside
        // `display_words` is what says so.
        //
        // BY THE CHARACTER, which is what SCTK hands this door; the footer
        // matches `Key::simple(sym).notation()`, which is what ITS toolkit
        // hands it. The two are not the same expression and they agree on
        // exactly the notations the table holds — `a`, `d` and `m`, three
        // unmodified lowercase letters — because that is what both spell for
        // an unmodified letter key. A table row spelt any other way would be a
        // key one door answers and the other does not, which is why the table
        // is the only place a letter is written.
        _ => {
            let typed = event.utf8.as_deref()?;
            let id = crate::display::OF_THE_VIEW
                .iter()
                .find(|(notation, _, _)| *notation == typed)
                .map(|(_, id, _)| *id)?;
            crate::display_words::press_of(id)?
        }
    })
}

impl WindowHandler for DisplayWindow {
    /// The compositor asked this window to go away.
    ///
    /// A STANDING PREVIEW GOES BACK FIRST, and it took the final gate to find
    /// this. A change that can blind a screen is applied over niri's IPC and
    /// held up by a question; the thing that takes it down is this process's
    /// own thread saying the file back (`Order::Undo`). Setting `quit` and
    /// returning dropped the sender and let `main` end, so the queued order
    /// was never carried out — and a door opened afterwards cannot help,
    /// because `previewed` is per-thread and a fresh one's is empty. The
    /// change stood until niri restarted, with `Ctrl+Alt+Del` as the only way
    /// out of a screen nobody can read.
    ///
    /// `Handle::finish` is what waits; the door cannot, because the answer
    /// comes back through a `calloop` source this function is inside of.
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.leave();
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

impl SeatHandler for DisplayWindow {
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

impl KeyboardHandler for DisplayWindow {
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

impl PointerHandler for DisplayWindow {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
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
        }
        // NO FRAME FOR A POINTER THAT CHANGED NOTHING. Nothing on this
        // screen is drawn from `row_under_pointer` — the click reads it, and
        // the paint does not — so repainting when it moved spent a GPU frame
        // per row crossed, on an identical picture. The footer's own hover
        // band IS drawn, which is why it repaints there and not here.
    }
}

impl CompositorHandler for DisplayWindow {
    /// The window moved to a screen of another scale — or was told its first.
    ///
    /// The rule and the measurement are in [`crate::hidpi`], which is where
    /// the nine doors that came before this one had it written after every
    /// one of them was found leaving this callback empty.
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

impl DisplayWindow {
    /// A monitor arrived or left, and Wayland is the one that said so.
    ///
    /// niri publishes NO event about outputs — the footer's own note says so —
    /// and `ronoctua_display::watch` reads again only when an order arrives.
    /// So without this the window never saw a second monitor appear, and kept
    /// drawing a row for one that had been unplugged: `RET` on it opened a
    /// sheet whose only content was that the screen was gone.
    ///
    /// SHALLOW, for the reason the footer gives: asking every monitor over
    /// DDC/CI on a hotplug is a fifth of a second per port.
    fn screens_changed(&self) {
        if let Some(h) = self.screens.as_ref() {
            h.send(ronoctua_display::Order::Look { deep: false });
        }
    }
}

impl OutputHandler for DisplayWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    // THE TWO THAT MATTER HERE, and they are empty in every other window
    // for a reason that does not hold in this one — see `screens_changed`.
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.screens_changed();
    }
    // AND NOT ON `update_output`, which the footer also leaves empty.
    //
    // It looks like the event this view most wants — a mode or a scale
    // changed. It is also the event OUR OWN writes produce: every `Keep` and
    // every `Undo` sends six `output_action`s, each of which makes niri emit a
    // `wl_output` batch, and each batch would cost a whole reading here — a
    // walk of `/sys/class/drm` and two connections to niri, several times per
    // keypress, on a machine with seven connectors. The reading that follows
    // an order is already sent by the crate.
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.screens_changed();
    }
}

impl ProvidesRegistryState for DisplayWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(DisplayWindow);
smithay_client_toolkit::delegate_dispatch2!(DisplayWindow);
