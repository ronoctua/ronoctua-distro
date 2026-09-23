//! The notification manager's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a ceiling in lines (`space.expansion`,
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

/// The first line of the list, before the explanation takes its own.
///
/// Everything asks [`Self::first_of_list`] instead: while the pointer rests on
/// a shortcut of the hint, the sentence about it is drawn on the line under
/// the hint and the list starts one row lower. A window is not anchored to the
/// bottom of the screen, so that costs a row of list and nothing else — the
/// footer, which is anchored, pays for the same line another way (see
/// `Footer::explain_rows`).
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
    let texts = std::rc::Rc::new(crate::language::texts());
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
            |event, (), state: &mut NotificationsWindow| {
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
        .insert_source(
            size_changed,
            |event, (), state: &mut NotificationsWindow| {
                if let channel::Event::Msg(()) = event
                    && crate::toolkits::reload()
                {
                    state.text_size_changed();
                }
            },
        )
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    let mut state = NotificationsWindow {
        conn: conn.clone(),
        handle: handle.clone(),
        _theme_watch,
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboards: crate::seats::Seats::default(),
        pointers: crate::seats::Seats::default(),
        here: PerSeat::default(),
        mods: Modifiers::default(),
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
        // THE MACHINE'S CLOCK, read once and carried — an open record shows
        // when it arrived, and a `View` that read `/etc/localtime` itself would
        // be a `View` no test could pin to a date.
        view: crate::notifications::View::fresh().in_zone(ronoctua_files::when::Zone::read_from(
            std::path::Path::new("/etc/localtime"),
        )),
        records: Vec::new(),
        book: ronoctua_notify::Book::distro(),
        notes: ronoctua_notify::store::Where::of_env(),
        texts,
        hinted: crate::chrome::Hinted::default(),
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
    /// AGENTS.md records and which nine windows inherited.
    handle: calloop::LoopHandle<'static, Self>,
    /// The watch on `theme.json` — the file is the broadcast (see `theme.rs`).
    _theme_watch: Option<ronoctua_files::notice::Notice>,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboards: crate::seats::Seats<Modifiers, wl_keyboard::WlKeyboard>,
    /// Every pointer this window was offered — one per cursor — with the
    /// state of each while another is `here`. See `crate::seats`.
    pointers: crate::seats::Seats<PerSeat>,
    /// The state of the pointer whose events are being handled now.
    here: PerSeat,
    mods: Modifiers,
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
    /// The hint line, and the shortcut the pointer is resting on it.
    hinted: crate::chrome::Hinted,
    /// The first visible row. A window has no page: it scrolls, like the
    /// locator and the manual.
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
    /// Which CELL along that row, which the message line needs and the click
    /// already asked for.
    ///
    /// Side by side one drawn row is two records, one per column, so the row
    /// alone cannot say what the pointer is over — see
    /// `notifications::View::about_at`.
    cell_under_pointer: Option<u16>,
    /// Whether this pointer is dragging the header's drawer bar — the
    /// per-pointer half of `chrome::Hinted`, swapped in around every frame.
    hint_drag: bool,
}

impl NotificationsWindow {
    /// Re-reads what the footer wrote, and redraws if it changed.
    ///
    /// Compares before drawing: repainting on a beat that found nothing new is
    /// a frame spent to put the same pixels back.
    fn reread(&mut self) {
        let fresh = self.notes.history();
        let prefs = self.notes.preferences();
        // COMPARED AFTER `took`, not before it. A filter that has come back as
        // one of the distro's leaves `mine` — so the file and the book DIFFER
        // by design from that moment on, and comparing the raw file against
        // the book would say "changed" on every one of these ticks, for ever.
        let mut candidate = ronoctua_notify::Book::distro();
        candidate.took(prefs.rules, prefs.shared);
        candidate.adjustments = prefs.adjustments;
        if fresh == self.records
            && candidate.mine == self.book.mine
            && candidate.shared == self.book.shared
            && candidate.adjustments == self.book.adjustments
        {
            return;
        }
        self.records = fresh;
        self.book = candidate;
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

    fn room(&self) -> usize {
        self.lines.saturating_sub(self.first_of_list()) as usize
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
        // AN OPEN RECORD IS NOT A LIST EITHER, but it can be taller than the
        // window — that is why the level exists — so it DOES follow: what it
        // follows is the line the arrows moved, not a cursor.

        // THE DRAWN LINE OF THE CURSOR, asked of the view — which is what
        // knows how many titles are above it. Adding one for "the heading" was
        // the arrangement written down as a constant, and narrow stacks the
        // two columns, so there are two.
        let cursor = {
            let lists = crate::notifications::Lists::of(&self.records, &self.book);
            self.view
                .visual_cursor(&lists, &self.book, self.columns, &self.texts)
        };
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
            Errand::Read(key) => {
                self.set_read(key, true);
            }
            // THE OTHER WAY, and it writes the file just the same. The footer
            // reconciles `read` in BOTH directions now, so a row put back to
            // unread here stays unread there — it used to take only `true`,
            // which would have undone this on the next write.
            Errand::Unread(key) => {
                self.set_read(key, false);
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
            Errand::Forget(key) => {
                self.records.retain(|r| r.key() != key);
                self.save_history();
            }
            // AN ACTION IS THE FOOTER'S TO RUN — and this door SENDS it there
            // rather than telling somebody to go and do it themselves.
            //
            // The owner pressed one here and was told the action runs from the
            // footer, which is true and useless: the control socket exists for
            // exactly this, the buffer host already writes to it, and a
            // command of ours is a command id the footer knows how to run.
            //
            // The other three kinds stay the footer's alone and say so: a bus
            // reply belongs to the process that owns the name, the clipboard
            // is the footer's, and spawning into a scope of its own is section
            // 9.1. A second implementation of any of them here is the second
            // door disagreeing with the first.
            Errand::Act(key, which) => {
                let sent = self
                    .records
                    .iter()
                    .find(|r| r.key() == key)
                    .and_then(|r| r.notification.actions.get(which))
                    .and_then(|a| match &a.doing {
                        ronoctua_notify::Doing::Command(id) => Some(id.clone()),
                        _ => None,
                    })
                    .map(|id| crate::leader::send_it(&format!("command {id}")));
                match sent {
                    Some(Ok(())) => {}
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, "the footer did not take the action");
                        self.message = Some(self.texts.t(ronoctua_i18n::key::NOTIF_ACT_NO_FOOTER));
                    }
                    None => self.act_in_the_footer(),
                }
            }

            // TYPING HAPPENS IN THE FOOTER, and this door says so rather
            // than growing a minibuffer of its own — the same answer it gives
            // about running an action, and the same reason: a second place to
            // type is a second thing that can disagree.
            Errand::Ask(_) => {
                let keys = format!("{} {}", crate::leader(), crate::leader::NOTIF_OPEN_KEYS);
                self.message = Some(self.texts.t_with(
                    ronoctua_i18n::key::NOTIF_TYPE_IN_THE_FOOTER,
                    &[("keys", &keys)],
                ));
            }
            // THE PERSON'S OWN FILTERS. This door writes the same file the
            // footer does, and the footer is watching it — the arrangement
            // `Adjusted` below already has.
            Errand::Filter { was, rule } => {
                if let Some(old) = was {
                    self.book.mine.retain(|r| r.id != old);
                }
                self.book.mine.retain(|r| r.id != rule.id);
                self.book.mine.push(rule);
                self.save_the_book();
            }
            Errand::Unfilter(id) => {
                self.book.mine.retain(|r| r.id != id);
                self.book.adjustments.remove(&id);
                self.save_the_book();
            }
            // INTO THE REPOSITORY, one filter at a time, by name, after a
            // screen that said what it means. This writes SOURCE — what makes
            // it part of the distro is the next build, and what publishes it
            // is the owner's own commit, which needs them in front of the
            // machine anyway.
            Errand::Share(id) => {
                let said = self.share_one(&id);
                self.message = Some(self.texts.t(said));
            }
            Errand::Adjusted(id, verdict) => {
                match verdict {
                    Some(v) => self.book.adjustments.insert(id, v),
                    None => self.book.adjustments.remove(&id),
                };
                self.save_the_book();
            }
        }
        self.ask_paint();
    }

    /// Puts one filter of the person's into the repository.
    ///
    /// The file is a LIST and this replaces by id, so sharing the same filter
    /// twice leaves one line rather than two. It is written in the table's own
    /// shape (`ronoctua_notify::as_table`) — the shape `assets/notifications.nix`
    /// uses and the generator already validates — because somebody will open
    /// that file, and serde's `{"Contains": "backup"}` is exactly the thing the
    /// owner said was not an interface.
    ///
    /// It returns the text key to put on the message line.
    fn share_one(&mut self, id: &str) -> &'static str {
        let Some(rule) = self.book.mine.iter().find(|r| r.id == id).cloned() else {
            return ronoctua_i18n::key::NOTIF_SHARE_FAILED;
        };
        let shared = ronoctua_store::Shared::of(ronoctua_store::repo_dir());
        if !shared.is_open() {
            return ronoctua_i18n::key::NOTIF_SHARE_NOWHERE;
        }
        let mut held: Vec<serde_json::Value> =
            shared.read(crate::notifications::SHARED_NOTIFICATIONS);
        held.retain(|it| it.get("id").and_then(serde_json::Value::as_str) != Some(id));
        held.push(ronoctua_notify::as_table(&rule));
        if let Err(e) = shared.write(crate::notifications::SHARED_NOTIFICATIONS, &held) {
            tracing::warn!(error = %e, id, "the filter could not be shared");
            return ronoctua_i18n::key::NOTIF_SHARE_FAILED;
        }
        // AND THE SCREEN REMEMBERS IT IS TRAVELLING. The copy on this machine
        // keeps working until a build brings it back as one of the distro's —
        // see `Book::took`, which is where it retires.
        self.book.shared.insert(id.to_owned());
        self.save_the_book();
        tracing::info!(id, "a filter was written into the repository");
        ronoctua_i18n::key::NOTIF_SHARED_WRITTEN
    }

    /// Says the action is the footer's to run, and where the footer is.
    ///
    /// THE KEYS ARE AN ARGUMENT, read from the same constant the keymap binds:
    /// a sentence that carried `n h` as text went stale the day the key moved.
    /// `leader()` and not `leader_chord()`: the sequence is typed after the
    /// LEADER (`SPC`, or `CTRL ALT SPC` in the nested loop).
    fn act_in_the_footer(&mut self) {
        let keys = format!("{} {}", crate::leader(), crate::leader::NOTIF_OPEN_KEYS);
        self.message = Some(self.texts.t_with(
            ronoctua_i18n::key::NOTIF_ACT_IN_THE_FOOTER,
            &[("keys", &keys)],
        ));
    }

    /// Writes the person's half of the filter book: their filters and their
    /// adjustments.
    fn save_the_book(&mut self) {
        let prefs = ronoctua_notify::Preferences {
            rules: self.book.mine.clone(),
            shared: self.book.shared.clone(),
            adjustments: self.book.adjustments.clone(),
        };
        if let Err(e) = self.notes.save_preferences(&prefs) {
            tracing::warn!(error = %e, "could not write the preferences");
        }
    }

    /// Turns one record's `read` on or off, and writes the file.
    ///
    /// BY THE RECORD'S NAME and not by the protocol's id: the id repeats across
    /// sessions and inside one, so `id ==` acted on every record that had ever
    /// been given that number — and the list being newest first, the extra ones
    /// were the rows BELOW the one under the cursor.
    fn set_read(&mut self, key: ronoctua_notify::RecordKey, read: bool) {
        for r in self.records.iter_mut().filter(|r| r.key() == key) {
            r.read = read;
        }
        self.save_history();
    }

    fn save_history(&mut self) {
        if let Err(e) = self.notes.save_history(&mut self.records) {
            tracing::warn!(error = %e, "could not write the history");
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
        let Some(press) = press_of(&event) else {
            return;
        };
        // A key clears the last message: it answered something that is no
        // longer what is being done.
        self.message = None;
        let errand = {
            let lists = crate::notifications::Lists::of(&self.records, &self.book);
            self.view.key(press, &lists, &self.book, self.columns)
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
            // AND ONCE MORE IF THE HEADER'S DRAWER JUST OPENED: the list's
            // first line is asked for before the frame and the wrap is only
            // known inside it. It answers once and then never again, so
            // nothing loops — see `fold::Fold::held`.
            if self.hinted.settled() {
                self.paint();
            }
        }
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
        // LINE 0: what the last action said, the focused row's own note, or
        // the keys — the footer's minibuffer, through the other door.
        //
        // The message and the note win over the keys for the reason the footer
        // gives on its own line: they are what the manager has to SAY about
        // something just done, and the keys are always there to come back to.
        // The note is the same source as the footer's — the reason a switch
        // does nothing — so the two doors say the same sentence in the same
        // place.
        let said = self
            .message
            .clone()
            .or_else(|| self.view.note(&self.texts))
            // AND WHAT THE ROW UNDER THE POINTER HAS TO ADD: when the
            // notification arrived, and the whole of a sentence the row had to
            // cut. The footer's own line takes it from the same method, so the
            // two doors cannot say different things about the same row.
            .or_else(|| {
                let lists = crate::notifications::Lists::of(&self.records, &self.book);
                self.view.about_at(
                    &lists,
                    self.columns,
                    self.here.row_under_pointer?,
                    self.here.cell_under_pointer,
                    &self.texts,
                )
            });
        // A MESSAGE TAKES THIS LINE, and then there is no shortcut on it to
        // point at. Asked BEFORE the list is measured, because the
        // explanation's row comes out of that measurement — see
        // `Self::first_of_list`. It is also why `said` is worked out up here
        // rather than beside the span that draws it.
        if said.is_some() {
            self.hinted.say(None);
        }
        // THE LISTS ARE BUILT AND DROPPED before `follow` moves the offset:
        // they borrow the records, and the scroll is a write.
        let rows = {
            let lists = crate::notifications::Lists::of(&self.records, &self.book);
            self.view
                .rows(&lists, &self.book, self.columns, &self.texts)
        };
        self.settle_offset(rows.len());

        let mut spans: Vec<ronoctua_render::Span> = Vec::new();

        // THE ROOM IS THE WHOLE OF `columns`, and that is not the mistake it
        // looks like: `recount` already took BOTH margins off the width before
        // counting cells, so `columns` is what lies between them and a line
        // that starts `margin` in and runs `columns` wide ends `margin` short
        // of the edge. The file manager's window counts the same way.
        let room = self.columns;
        // LINE 0 AND THE DRAWER IT OPENS INTO: the view's shortcuts or what
        // the last order said, the chevron on the right edge when the line
        // does not fit, the wrapped rest of it once that has been pressed,
        // and the sentence about the shortcut the pointer is resting on.
        //
        // ONE PAINTER for every door of this shape — see `chrome::Hinted`:
        // this was written out in each of the ten windows, and the drawer was
        // written in none of them.
        let (pieces, hinting) = said.map_or_else(
            || {
                // THE KEYS, in two registers, from the SAME table the footer's
                // minibuffer reads. AND WHERE IN THE LIST THE CURSOR IS: this
                // window draws no rectangle, so with a fortnight of records
                // there was no sign that anything lay below the last drawn row.
                let lists = crate::notifications::Lists::of(&self.records, &self.book);
                let place = Some((self.view.cursor(), lists.len(self.view.pane())));
                (self.view.hint_of(place, &lists, &self.texts), true)
            },
            |text| (vec![crate::chrome::Piece::said(text)], false),
        );
        spans.extend(
            self.hinted
                .head(&pieces, hinting, margin, room, &self.texts),
        );

        // THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
        // draws for these rows, and the one `record_line` argues for: with
        // two columns on screen a band across the row would say the two
        // items on it were one thing. The window drew a band as well, which
        // was the one thing the two doors did differently; it went.
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

        // EVERY CURSOR'S HOVER, in its own colour — see `crate::seats`. Side
        // by side a drawn line is TWO records, one per column, so a band
        // lights only the column the pointer is in: a band across the whole
        // line would say both records are under it.
        #[allow(clippy::cast_precision_loss)]
        let bands: Vec<ronoctua_render::quad::Quad> = {
            let cell = crate::chrome::Frame::of(self.width as f32, self.height as f32, 0).cell();
            let split = f32::from(margin + crate::notifications::split_at(self.columns)) * cell;
            let two = crate::notifications::two_columns(self.columns);
            // ONE CURSOR AT A TIME, so each band keeps the column of the
            // pointer that made it; two cursors on one line but in different
            // columns are two bands, and on the same half the first wins.
            let mut seen: Vec<(usize, bool)> = Vec::new();
            let mut out = Vec::new();
            for (color, s) in self.pointers.lit(&self.here) {
                let right = two
                    && s.cell_under_pointer
                        .is_some_and(|c| f32::from(margin + c) * cell >= split);
                let Some(row) = s.row_under_pointer else {
                    continue;
                };
                if seen.contains(&(row, right)) {
                    continue;
                }
                seen.push((row, right));
                for mut band in crate::chrome::hover_bands(
                    std::iter::once((color, Some(row))),
                    self.offset,
                    self.room(),
                    rows.len(),
                    self.first_of_list(),
                    self.width as f32,
                ) {
                    if two {
                        if right {
                            band.x = split;
                            band.width = self.width as f32 - split;
                        } else {
                            band.width = split;
                        }
                    }
                    out.push(band);
                }
            }
            out
        };
        // THE BAR, from the same `Scroll` every other bar in this system uses.
        let scroll = crate::chrome::list_scroll(rows.len(), self.room(), self.offset);
        #[allow(clippy::cast_precision_loss)]
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
        // ONE RECTANGLE: the list's scrollbar. The focus is a colour (see the
        // paint loop) and the hint line is text, so this is the only quad the
        // window draws — the comment here said there was none, and had said so
        // since before `every-list-window-has-a-bar` gave every scrolling
        // window one.
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

    /// Which cell of the row is at `x`.
    ///
    /// The content's margin comes off first: the row's own cells start after
    /// it, and counting from the window's edge would put every target one
    /// margin to the right of where it is drawn.
    fn cell_at(&self, x: f64) -> Option<u16> {
        let cell = f64::from(tokens::metrics::cell_width(tokens::text::size()));
        if cell <= 0.0 || x < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let which = (x / cell) as u16;
        which.checked_sub(CONTENT_MARGIN)
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
                    state.keyboards.switch_to(kb, &mut state.mods);
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
            self.keyboards.lost(&seat, &mut self.mods);
        }
        if cap == Capability::Pointer {
            self.pointers.lost(&seat, &mut self.here);
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for NotificationsWindow {
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
        // The modifiers whole, for the reason the footer already records: a
        // state that only updates with focus cannot keep holding without it,
        // and a stuck `alt` makes every following key come out as `M-…`.
        self.mods = Modifiers::default();
        // AND THE HEADER'S OWN MODIFIER, for the same reason.
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
        self.keyboards.switch_to(kb, &mut self.mods);
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
        // Holding `j` walks the list, which is what a listing wants — unlike
        // the footer, where repeating a leader key is always a mistake.
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
        mods: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.keyboards.switch_to(kb, &mut self.mods);
        self.mods = mods;
    }
}

impl PointerHandler for NotificationsWindow {
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

impl NotificationsWindow {
    /// One pointer's frame, with its state already `here`.
    fn pointer_frame_here(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        events: &[PointerEvent],
    ) {
        let before = self.here.row_under_pointer;
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
        let bar_in_play = self.here.dragging
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
                        self.here.cell_under_pointer = self.cell_at(e.position.0);
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
                    clicked = self
                        .here
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
        // A CLICK IS THE `RET` OF THE ROW IT LANDS ON — `View::clicked` answers
        // with the same errand the key would (`open_here`), invariant 19.
        if let Some((row, cell)) = clicked {
            // THE DRAWN ROW, headings and all: which of them is an item of
            // which column is `View::place_of`, the one plan the drawing
            // walked. Subtracting one for "the heading" was the arrangement
            // written down as a constant, and narrow now stacks the two
            // columns, so there are two of them.
            let errand = {
                let lists = crate::notifications::Lists::of(&self.records, &self.book);
                self.view
                    .clicked(row, cell, &lists, &self.book, self.columns)
            };
            self.apply(errand);
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
        // EXCEPT THE LINE UNDER THE HINT, which IS drawn from the pointer:
        // resting on a shortcut spells it out, and moving off it takes the
        // sentence away. The ROW under the pointer is the
        // paragraph below.
        if explained {
            self.ask_paint();
        }
        // A FRAME ONLY WHEN SOMETHING THAT IS DRAWN MOVED. The row under the
        // pointer is drawn now — every cursor's in its own colour — so a new
        // row is a new picture.
        if before != self.here.row_under_pointer {
            self.ask_paint();
        }
    }
}
impl CompositorHandler for NotificationsWindow {
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
