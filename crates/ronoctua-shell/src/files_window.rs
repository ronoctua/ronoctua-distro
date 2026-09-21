//! The file manager's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a ceiling in lines (`space.expansion`,
//! invariant 14) and it is right that it does: it is a footer. Twelve rows are
//! enough to look something up and act on it, and not enough for the job people
//! open a file manager to do.
//!
//! So this exists — and everything it knows lives in [`crate::files::View`],
//! which the footer draws from too. What this module holds is a window, a GPU
//! and the events of both; what it decides is nothing. A second implementation
//! of "what does `d` do here" is how two doors come to disagree, and it is the
//! shape the rescue console already refused once.
//!
//! # Why it is not a wasm buffer
//!
//! Because a wasip2 component has no filesystem — invariant 11, and it is
//! structural: the `world` does not import one. A file manager as a buffer
//! would need a capability of its own, three new `input` cases for hover, the
//! secondary button and the wheel, and a whole directory crossing the boundary
//! by value on every change. `ronoctua-shell terminal` is already a native
//! window of exactly this shape, and that is the precedent this follows.

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
    protocol::{
        wl_data_device_manager::DndAction, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface,
    },
};

/// The window's `app_id`.
///
/// Its own, like the terminal's and the monitor's: it is by this that the
/// modeline and the VM test tell one window from another, and a shared id would
/// make two different things look like one in niri's IPC.
pub const APP_ID: &str = "br.ronoctua.files";

/// `BTN_LEFT` and `BTN_RIGHT`, from `linux/input-event-codes.h`.
const LEFT_BUTTON: u32 = 0x110;
const RIGHT_BUTTON: u32 = 0x111;

/// The first line of the listing. Above it: the path, and the message line.
///
/// The same arrangement as the footer — two lines glued, content below — and
/// repeated here deliberately: a person should not have to learn a second
/// shape for the same thing.
const FIRST_OF_REGION: u16 = 1;

impl FilesWindow {
    /// The region's first line, WITH the opened line's band counted in.
    ///
    /// It used to be the constant above and nothing else. The minibuffer's
    /// line can now be opened into several (see [`crate::fold`]), and every
    /// one of them is a line the listing does not get — so the number moved
    /// from a `const` to a question, and `chrome::Frame` carries the answer
    /// for everything downstream of it.
    /// AND THE SHORTCUT'S EXPLANATION, when the pointer is on one. It is the
    /// same argument one line further: a line the listing does not get.
    fn first_of_region(&self) -> u16 {
        FIRST_OF_REGION + crate::fold::rows(self.fold) + self.hinted.rows()
    }
}

/// Opens the manager in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it(start: Option<&std::path::Path>) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<FilesWindow> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;

    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let preferred = std::env::var("RONOCTUA_FILES_START").ok();
    let where_ = start.map_or_else(
        || ronoctua_files::listing::start_at(home.as_deref(), preferred.as_deref()),
        std::path::Path::to_path_buf,
    );

    let surface_of = compositor.create_surface(&qh);
    // `RequestServer`: decoration belongs to the COMPOSITOR (invariant 4).
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    // `*name*` is Emacs's convention for a buffer that is not a file, the same
    // one the modeline already uses for `*scratch*`.
    window.set_title("*files*");
    window.set_app_id(APP_ID);
    window.commit();

    // `'static` SPELLED OUT, because the handle is kept ON the state so the
    // keyboard can be created with repeat — see `new_capability`.
    let mut cycle: EventLoop<'static, FilesWindow> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();
    let (sender, reports) = channel::channel::<crate::files::Report>();
    // THE QUEUE, AS REPORTS. This door has no modeline and no panel: what it
    // needs from the transfer queue is "the thing you asked for is over, read
    // the directory again". Without it, `Errand::Done` would refresh the
    // listing at the instant the work was HANDED OVER — showing the directory
    // as it was before the copy, and never again.
    crate::transfers::listen_as_reports(sender.clone());
    // AND THE DISKS, on the same channel. This process has its own `calloop`
    // loop and no clock at all, so a signal is the only way a stick plugged in
    // while the window is open can ever appear in it.
    if let Err(e) = ronoctua_disks::watch({
        let s = sender.clone();
        move |rack| {
            let _ = s.send(crate::files::Report::Racked(Box::new(rack)));
        }
    }) {
        tracing::warn!(error = %e, "no disk watcher");
    }
    // WHERE THE SIDEBAR PERSISTS, read once and used twice: the watch below is
    // told about it, and the sidebar itself is built with it.
    let store = ronoctua_files::sidebar::store::config_dir();
    // AND THE DIRECTORY ITSELF, on the same channel again.
    //
    // This is the door the owner reported: it has no clock, so until now a file
    // put into the open folder by anything else — a download finishing, a copy
    // from a terminal, Syncthing — appeared only if the person left the folder
    // and came back. The footer had `View::stale` on its one-second tick and
    // this process had nothing.
    //
    // A watch and not a clock of its own, and that is the whole point: with
    // several of these windows open, a timer would be one wake-up per second per
    // window for ever, while a watch that nobody touches costs nothing at all.
    let notice = match ronoctua_files::notice::Notice::watching(store.as_deref(), {
        let s = sender.clone();
        move || s.send(crate::files::Report::Stirred).is_ok()
    }) {
        Ok(n) => Some(n),
        // NOT SILENT, and not the reported bug coming back either. Out of
        // inotify instances is the realistic failure, and a window that simply
        // stopped refreshing with nothing said would be exactly the silent
        // fallback this project refuses — so THIS is the one case that arms a
        // clock, and it is the only one.
        Err(e) => {
            tracing::warn!(error = %e, "no directory watcher; falling back to a clock");
            handle
                .insert_source(
                    calloop::timer::Timer::from_duration(std::time::Duration::from_secs(1)),
                    |_, (), state: &mut FilesWindow| {
                        if state.view.stale() {
                            state.view.reread();
                        }
                        if state.view.sidebar_stale() {
                            state.view.reread_sidebar();
                        }
                        state.ask_paint();
                        calloop::timer::TimeoutAction::ToDuration(std::time::Duration::from_secs(1))
                    },
                )
                .map_err(|e| anyhow::anyhow!("could not hang the fallback clock: {e}"))?;
            None
        }
    };
    handle
        .insert_source(reports, |event, (), state: &mut FilesWindow| {
            if let channel::Event::Msg(r) = event {
                state.reported(r);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the files channel: {e}"))?;

    // THE THEME'S FILE, watched — this door has no clock, and a watch that
    // nobody touches costs nothing (the same argument as the directory's).
    let (theme_sender, theme_changed) = channel::channel::<()>();
    let _theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    handle
        .insert_source(theme_changed, |event, (), state: &mut FilesWindow| {
            if let channel::Event::Msg(()) = event {
                state.theme_changed();
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
        .insert_source(language_changed, |event, (), state: &mut FilesWindow| {
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
        .insert_source(size_changed, |event, (), state: &mut FilesWindow| {
            if let channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                state.text_size_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    // THE FRAMES OF A MOVING FILE, on a channel of their own.
    //
    // Their own, and not the reports': a frame arrives ten times a second
    // while a cursor rests on a video, and putting that through the channel
    // that also carries "the directory changed" would make every one of those
    // wake the parts of the door that read a directory.
    //
    // AND THE FRAME ARRIVING IS THE CLOCK. Neither door has a timer for this
    // and neither gained one — the player sleeps for the frame's own delay and
    // sends it, so when it stops, the wake-ups stop with it.
    let (frames_sender, frames) = channel::channel::<crate::motion::Beat>();
    handle
        .insert_source(frames, |event, (), state: &mut FilesWindow| {
            if let channel::Event::Msg(f) = event {
                state.view.played(f);
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the motion channel: {e}"))?;

    // AND THE VIEWS OF A SHAPE, on a channel of their own too, for the reason
    // written just above about frames: a drag emits one every few tens of
    // milliseconds and none of them is news about a directory.
    let (turns_sender, turns) = channel::channel::<crate::stage::Beat>();
    handle
        .insert_source(turns, |event, (), state: &mut FilesWindow| {
            if let channel::Event::Msg(b) = event {
                state.view.drew(b);
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the stage channel: {e}"))?;

    // AND THE GLOBE'S, on a third — for the same reason as the second: a drag
    // across the world emits a picture every few tens of milliseconds, and none
    // of them is news about a directory or about a shape.
    let (globes_sender, globes) = channel::channel::<crate::globe::Beat>();
    handle
        .insert_source(globes, |event, (), state: &mut FilesWindow| {
            if let channel::Event::Msg(b) = event
                && state.view.saw_globe(&b)
            {
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the globe channel: {e}"))?;

    // AND THE POINTER'S FILM, on a fifth.
    //
    // Its own and not the preview player's, for the reason `View::tour` gives:
    // the two answer different questions — what the keyboard is ON and what the
    // hand is OVER — and a `Beat` that reached both would make one cell's frame
    // drop the other's.
    let (tours_sender, tours) = channel::channel::<crate::motion::Beat>();
    handle
        .insert_source(tours, |event, (), state: &mut FilesWindow| {
            if let channel::Event::Msg(b) = event
                && state.view.played_tour(&b)
            {
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the tour channel: {e}"))?;

    // AND THE THUMBNAILS', on a fourth. A folder opening answers dozens of
    // times in a second, and none of them is news about a directory, a film or
    // a shape.
    let (thumbs_sender, thumbs) = channel::channel::<crate::thumbs::Made>();
    handle
        .insert_source(thumbs, |event, (), state: &mut FilesWindow| {
            if let channel::Event::Msg(m) = event
                && state.view.thumbed(m)
            {
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the thumbnail channel: {e}"))?;

    let texts = std::rc::Rc::new(crate::language::texts());
    // The catalogue is swept HERE and not inherited: this is a process of its
    // own, and `open with` needs to know who declares a type. 128 entries in
    // 68 ms, measured — once, at startup, like the launcher's.
    let catalogue = std::sync::Arc::new(ronoctua_apps::scan(&language()));
    let data = std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from);
    let bin = ronoctua_files::trash::Bin::of_home(data.as_deref(), home.as_deref());
    let side = crate::files::sidebar_of(home.as_deref(), bin.as_ref(), store);
    let view = crate::files::View::at(
        &where_,
        ronoctua_files::who::Roster::read_from(&ronoctua_files::who::Files::default()),
        ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
        bin,
        catalogue.clone(),
        texts.clone(),
        side,
    );

    let mut state = FilesWindow {
        conn: conn.clone(),
        // THE DRAG-AND-DROP MACHINERY. `None` where the compositor has no
        // `wl_data_device_manager` — absence is not an error, and everything
        // else goes on working.
        carrying: crate::carry::Carrying::bind(&globals, &qh),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboard: None,
        pointer: None,
        mods: Modifiers::default(),
        pressed_at: 0,
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
        gpu: None,
        text: None,
        quads: None,
        images: None,
        view,
        catalogue,
        texts,
        looking: crate::files::inspector(sender.clone()),
        playing: crate::motion::player(frames_sender),
        turning: crate::stage::stage(turns_sender),
        spinning_globe: crate::globe::stage(globes_sender),
        thumbing: crate::thumbs::pool(
            thumbs_sender.clone(),
            ronoctua_xdg::cache_dir(),
            ronoctua_xdg::cache_root(),
        ),
        winding: crate::thumbs::animator(thumbs_sender),
        touring: crate::motion::player(tours_sender),
        wound: crate::thumbs::Wind(Vec::new()),
        sender,
        under_pointer: None,
        dragging: None,
        dragging_fold: false,
        detents: 0.0,
        cell_under_pointer: None,
        line_under_pointer: None,
        owed: false,
        message: None,
        said: 0,
        fold: crate::fold::Fold::default(),
        fold_control: None,
        fold_scroll: ronoctua_design::Scroll {
            total: 0,
            lines: 0,
            offset: 0,
            top: 0,
        },
        fold_wrapped: 0,
        hinted: crate::chrome::Hinted::default(),
        handle: handle.clone(),
        walk_armed: false,
        spinning: None,
        dragging_globe: None,
        notice,
        _theme_watch,
    };
    // POINTED AT WHERE THE VIEW ALREADY IS. `View::at` has read the directory
    // by now, so what this arms is the watch for everything that happens NEXT.
    state.follow_the_place();

    WaylandSource::new(conn, queue)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |e| {
            // THE ONE PLACE THAT KNOWS THE BATCH IS OVER. Everything else
            // marks a frame owed; here it is drawn, once.
            e.settle();
            if e.quit {
                signal.stop();
            }
        })
        .context("event loop")?;
    Ok(())
}

/// The session's language, for the catalogue's localised names.
fn language() -> String {
    std::env::var("LANG")
        .ok()
        .and_then(|l| l.split('.').next().map(ToOwned::to_owned))
        .unwrap_or_else(|| "en_GB".to_owned())
}

/// The window.
struct FilesWindow {
    conn: Connection,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    mods: Modifiers,
    /// Entries carried out of the listing, and drops landing in it. See
    /// [`crate::carry`].
    carrying: Option<crate::carry::Carrying>,
    /// The serial of the press a drag would start from.
    ///
    /// `start_drag` takes the PRESS's own serial and the compositor refuses
    /// any other, with no error on this side — so it is kept from the event
    /// that carried it rather than asked for later.
    pressed_at: u32,
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
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    quads: Option<ronoctua_render::quad::Quads>,
    /// The image pipeline, for the preview panel's picture.
    ///
    /// # Why this door needed one and the footer already had it
    ///
    /// Because the footer draws the clipboard's captures and the launcher's
    /// pixmaps, and this one drew glyphs and rectangles and nothing else. A
    /// preview that worked in one door and not the other is exactly the
    /// divergence the two-door arrangement exists to prevent — and it would
    /// have been reported as "the preview does not work in the window", one of
    /// the three causes with the same screen.
    images: Option<ronoctua_render::image::Images>,
    view: crate::files::View,
    catalogue: crate::apps::Catalogue,
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    sender: channel::Sender<crate::files::Report>,
    /// Where an ask for a preview and its facts is posted.
    ///
    /// `None` when the thread could not be raised: the panels then draw the
    /// glyph and nothing else, which is honest, rather than the door refusing
    /// to open.
    looking: Option<std::sync::mpsc::Sender<crate::files::Ask>>,
    /// The thread that follows a moving file, when there is one to follow.
    playing: Option<crate::motion::Player>,
    /// The hands that make thumbnails, when the entries are side by side.
    ///
    /// `None` when not one could be raised: the grid then draws its glyphs and
    /// nothing else, which is honest — the same shape `looking` has above.
    thumbing: Option<std::sync::mpsc::Sender<crate::thumbs::Ask>>,
    /// The thread that walks the frames of what moves by itself.
    winding: Option<std::sync::mpsc::Sender<crate::thumbs::Wind>>,
    /// The player that tours the film under the POINTER.
    ///
    /// A second one, and the second child process this manager will ever have
    /// running at once. Both are bounded: one cell has the keyboard and one has
    /// the hand.
    touring: Option<crate::motion::Player>,
    /// What it was last told to animate.
    ///
    /// Kept so an unchanged screen costs no message: the set is worked out from
    /// the layout every frame, and sending it every frame would wake a thread
    /// sixty times a second to be told nothing had changed.
    wound: crate::thumbs::Wind,
    /// WHAT IS UNDER THE POINTER, resolved by `files::View::hit`.
    ///
    /// One answer and not three numbers: the section, the row inside it and
    /// the field's target come out together, because separately they were
    /// three chances to resolve a point against a list it was not over. See
    /// `files::View::hit` for the four defects that came of asking otherwise.
    under_pointer: Option<crate::files::Target>,
    /// Which section's scrollbar the pointer took hold of, if any.
    ///
    /// The bars were drawn and nothing listened: the owner tried the facts'
    /// with the mouse and it did not move. A bar one cannot drag is a picture
    /// of a bar.
    dragging: Option<(crate::files::Pane, crate::files::Axis)>,
    /// Is the pointer holding the OPENED LINE's bar?
    ///
    /// A gesture of the door's own and not a fourth `Axis`, for the reason
    /// `spinning` is not one either: line 0 belongs to no section, so
    /// `Geometry::bars` — which walks the sections — can never produce it.
    /// That is exactly why the bar could not be dragged; see `fold_track`.
    dragging_fold: bool,
    /// What is left over of a touchpad gesture, in lines.
    ///
    /// A wheel sends whole detents; a touchpad sends fractions of one, and this
    /// door spent them: it asked for a whole line's worth in a single event and
    /// threw away everything below that. The footer has had the accumulator
    /// since it was written — the owner reported the consequence as the gesture
    /// working in one door and in neither section of the other.
    detents: f64,
    cell_under_pointer: Option<u16>,
    /// Which LINE of the region the pointer is over.
    ///
    /// Kept beside the cell and beside the resolved target, because the band
    /// wants a shallower answer than the target is: see `files::View::band_row`.
    line_under_pointer: Option<usize>,
    /// A frame is OWED, and will be drawn when the event queue empties.
    ///
    /// # Why not paint where the change happens
    ///
    /// Because presenting blocks. Every handler here used to call `paint()`
    /// itself, and a drag on a scrollbar sends a `Motion` per pixel of pointer
    /// travel: a hundred events, a hundred presents, each waiting for the next
    /// vblank. The owner described the consequence exactly — dragging the
    /// thumb to the bottom did not jump there, the listing *walked* through
    /// every item and arrived seconds later.
    ///
    /// The arithmetic was never slow. What was slow was doing it a hundred
    /// times and showing all hundred results. One flag, drained once where
    /// `calloop` finishes a batch, and the drag lands on the frame after the
    /// last event — which is what every other program does.
    owed: bool,
    message: Option<String>,
    /// Which message is on screen, counted.
    ///
    /// A one-shot timer is armed for each one and fires three seconds later;
    /// by then the message it was armed for may have been replaced or cleared,
    /// and a timer that wiped whatever it found would take the NEW one down
    /// early. It clears only its own.
    said: u64,
    /// Whether line 0 is OPEN, and how far it is scrolled. See [`crate::fold`].
    ///
    /// The same state the footer keeps for the same line — this door's line 0
    /// and that one's minibuffer are the one line, seen through two doors.
    fold: crate::fold::Fold,
    /// Where the control that opens it was drawn, in cells of line 0.
    fold_control: Option<u16>,
    /// The opened band's scroll, as it was last DRAWN.
    fold_scroll: ronoctua_design::Scroll,
    /// How many rows line 0's text came to at the width it was DRAWN at.
    ///
    /// The height is measured ONCE, when the band opens, and the content is
    /// only known where the line is laid out — but `first_of_region` is asked
    /// BEFORE the frame. So the drawing writes this down, `fold::settle` reads
    /// it and answers once, and `draw` goes round one more time. After that
    /// nothing reopens the question: see `fold::Fold::held`.
    fold_wrapped: usize,
    /// The hint line, and the shortcut the pointer is resting on it.
    hinted: crate::chrome::Hinted,
    /// The loop, for the two things that need one.
    ///
    /// A keyboard that REPEATS: SCTK only generates repeat events for one made
    /// with `get_keyboard_with_repeat`, and that wants a `LoopHandle` — which
    /// is why this is carried rather than reached for (see `new_capability`).
    ///
    /// And the message line's expiry: see [`FilesWindow::say`]. And the clock
    /// that walks a name too long for its column: see
    /// [`FilesWindow::arm_the_walk`].
    handle: calloop::LoopHandle<'static, Self>,
    /// Whether the walking name's clock is up.
    ///
    /// One at a time: armed when a name starts walking, dropped by its own
    /// tick when none is. Without the flag, every frame drawn while a long
    /// name is on screen would hang another timer on the loop.
    walk_armed: bool,
    /// The thread that draws a shape at the angle it is being looked at.
    turning: Option<crate::stage::Stage>,
    /// The thread that draws the globe. One per DOOR, because a door has one
    /// calloop and one channel; the pose is the view's.
    spinning_globe: Option<crate::globe::Stage>,
    /// Where the pointer was when it last moved a shape.
    ///
    /// A gesture of the door's own and NOT a fourth `Axis`: `drag_to` inverts
    /// a track — one scalar, one direction — and turning something is two
    /// deltas with no track behind them. It sits beside `dragging` because it
    /// is the same kind of thing: a press that owns the pointer until it is
    /// let go.
    spinning: Option<(f64, f64)>,
    /// Where the pointer was when it took hold of the GLOBE.
    dragging_globe: Option<(f64, f64)>,
    /// The directory reporting itself, when one could be had.
    ///
    /// `None` means the watch could not be armed and the fallback clock above
    /// is doing the work instead.
    notice: Option<ronoctua_files::notice::Notice>,
    /// The watch on `theme.json` — the file is the broadcast (see `theme.rs`).
    _theme_watch: Option<ronoctua_files::notice::Notice>,
}

impl FilesWindow {
    // EVERY ONE OF THESE IS `Geometry`'S ANSWER, and none of them is a second
    // one.
    //
    // They used to be the arithmetic itself, and that is what made this door
    // untestable: the numbers lived on a struct that owns a Wayland
    // connection. They are one line each now — the door asks the value and
    // the value answers — so the same question put by a test and by a frame
    // cannot come back different.

    /// The two rooms this door gives, for whatever asks about a bar.
    fn rooms(&self) -> crate::files::Room {
        self.geometry().rooms()
    }

    /// How many lines the PANELS are given — what the facts scroll inside.
    fn panel_room(&self) -> usize {
        self.geometry().panel_room()
    }

    /// How many rows of listing fit.
    ///
    /// One less than the region, because the header takes a line and it is
    /// drawn from the same arithmetic the hit test uses — a room that counted
    /// the header would put the last row under the window's edge.
    fn list_room(&self) -> usize {
        self.geometry().list_room()
    }

    /// How many lines the listing's chrome takes — two, or none at all when a
    /// menu, a picker or a sheet is up and there are no columns to name.
    fn chrome_lines(&self) -> u16 {
        u16::try_from(self.view.chrome_lines()).unwrap_or(0)
    }

    /// How many lines the region has.
    ///
    /// AND THE STRIP COMES OFF THE BOTTOM. It is pinned under the listing and
    /// does not scroll, so the rows it takes are rows the listing does not
    /// have; counting them would put the last entry underneath the preview.
    fn room(&self) -> usize {
        self.geometry().room()
    }

    /// What the manager should be asked about the file under the cursor.
    ///
    /// The theme's file changed: the roles resolve anew on the next paint, and
    /// what a child painted with a value — the waveform, the shape — is asked
    /// for again, because the theme is part of the preview's key.
    fn theme_changed(&mut self) {
        if !crate::theme::reload() {
            return;
        }
        self.look_again();
        self.ask_paint();
    }

    /// Called after anything that could have moved the cursor. `wants_look`
    /// answers `None` when the answer is already in hand, so this is a cheap
    /// call and not a decision made here.
    fn look_again(&mut self) {
        if let Some(queue) = self.looking.as_ref()
            && let Some(ask) = self
                .view
                .wants_look(u32::from(tokens::space::FILES_PANELPIXELS))
        {
            let _ = queue.send(ask);
        }
        self.thumbs_again();
        self.follow_the_motion();
        self.follow_the_turn();
    }

    /// Asks for the thumbnails on screen that are not in hand yet.
    ///
    /// Beside `wants_look` and for the same reason: the cursor and the offset
    /// can move a dozen ways, and enumerating them is how a case gets missed.
    /// `wants_thumbs` answers with nothing when everything visible is already
    /// known, so this is a cheap call rather than a decision made here.
    fn thumbs_again(&mut self) {
        let Some(queue) = self.thumbing.clone() else {
            return;
        };
        let size = tokens::text::size();
        // THE SCALE IS PART OF IT, and that is the whole point of taking the
        // target from here. `docs/PENDING.md` records that every texture in
        // this tree is still decoded at the LOGICAL size and stretched by the
        // compositor — sharp letters beside a soft picture — and names
        // `Stamp::key()` as the one already carrying the step it needs. A grid
        // decoded in physical pixels is one path off that list rather than one
        // more on it.
        #[allow(clippy::cast_precision_loss)]
        let scale = self.scale as f32;
        let cell = tokens::metrics::cell_width(size) * scale;
        let line = tokens::metrics::line_height(size) * scale;
        let geo = self.geometry();
        let (columns, room) = (geo.columns, geo.room());
        for ask in self.view.wants_thumbs(columns, room, cell, line) {
            let _ = queue.send(ask);
        }
        // AND THE POINTER'S FILM, which is a wish like the preview's: asked
        // once a frame and answered `None` when nothing changed.
        if let Some(player) = self.touring.as_mut()
            && let Some(ask) = self.view.wants_tour()
        {
            player.tell(ask);
        }
        // AND WHAT MOVES, when the set has changed.
        if let Some(wind) = self.winding.as_ref() {
            let now = self.view.moving_thumbs(columns, room, cell, line);
            if now != self.wound {
                let _ = wind.send(now.clone());
                self.wound = now;
            }
        }
    }

    /// Starts, or stops, the player that follows a moving file.
    ///
    /// Asked once a frame beside the still preview, and for the same reason:
    /// the cursor can move a dozen ways and enumerating them is how a case
    /// gets missed. One question, one place.
    fn follow_the_motion(&mut self) {
        let Some(player) = self.playing.as_mut() else {
            return;
        };
        // ONE QUESTION, ONE ANSWER, PASSED STRAIGHT ON — see the footer door's
        // twin, and the reason written there.
        if let Some(ask) = self.view.wants_motion() {
            player.tell(ask);
        }
    }

    /// Starts, or stops, the thread that draws a shape.
    ///
    /// The twin of `follow_the_motion`, asked in the same places and for the
    /// same reason: the pose changes from a key, from the wheel, from a drag
    /// and from the panel being resized, and enumerating those is how one gets
    /// missed.
    fn follow_the_turn(&mut self) {
        if let Some(stage) = self.turning.as_mut()
            && let Some(ask) = self.view.wants_turn()
        {
            stage.tell(ask);
        }
        // AND THE GLOBE, asked in the same place and by the same rule: a value
        // diff, so a repeat costs nothing.
        if let Some(stage) = self.spinning_globe.as_mut()
            && let Some(ask) = self.view.wants_globe()
        {
            stage.tell(ask);
        }
    }

    /// Keeps the cursor on screen, starting to scroll two rows before the edge.
    ///
    /// `SCROLL_MARGIN` two, the same as the footer's — it is vi's `scrolloff`,
    /// and the reason is that a cursor pinned to the last visible line gives no
    /// warning of what is coming.
    fn follow(&mut self) {
        // THE LISTING'S ROOM, and not the region's.
        //
        // This function used to compare the cursor against `room()` — chrome
        // included — while the wheel two hundred lines below used
        // `list_room()`, which takes the chrome off. Two answers to "how many
        // rows are visible" in one file, and the consequence was that the last
        // two entries of a directory could not be reached by holding `j`.
        //
        // The margin went with it: the footer had two and this door had two
        // written out separately, so they could drift. One function now, in
        // the view, for both doors.
        let room = self.list_room();
        if room == 0 {
            return;
        }
        self.view.follow(room);
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, step: crate::files::Step) {
        use crate::files::{Doing, Errand, Step};
        match step {
            Step::Nothing => return,
            Step::Changed => {}
            Step::Closed => {
                self.quit = true;
                return;
            }
            Step::Say(e) => {
                let m = self.texts.t_with(e.key, &e.borrowed());
                self.say(m);
            }
            // THE OPENED LINE IS THE DOOR'S, and this is where the keyboard's
            // ask lands. The view holds the focus that reached the control and
            // nothing else about it — see `files::Pane::Fold`.
            Step::Fold(ask) => match ask {
                crate::fold::Ask::Toggle => self.fold.toggle(),
                crate::fold::Ask::By(by) => {
                    let last = self
                        .fold_scroll
                        .total
                        .saturating_sub(self.fold_scroll.lines);
                    let next = i64::try_from(self.fold.offset).unwrap_or(0) + i64::from(by);
                    self.fold.offset = usize::try_from(next.max(0)).unwrap_or(0).min(last);
                }
            },
            Step::Do(doing) => {
                // THE FOUR THAT SPAWN are the door's business; everything else
                // is the same code the footer runs.
                match doing {
                    Doing::Open(path) => {
                        self.open_the_file(&path);
                        self.ask_paint();
                        return;
                    }
                    // THE MAP. DETACHED and never waited on: this runs inside
                    // the calloop callback, and waiting there is the frozen
                    // footer the `systemd-run --scope` lesson already cost this
                    // project twice. Same shape as `syncthing_window`'s page.
                    Doing::Web(address) => {
                        match option_env!("RONOCTUA_XDG_OPEN").filter(|p| !p.is_empty()) {
                            Some(opener) => {
                                match std::process::Command::new(opener)
                                    .arg(&address)
                                    .stdin(std::process::Stdio::null())
                                    .stdout(std::process::Stdio::null())
                                    .stderr(std::process::Stdio::null())
                                    .spawn()
                                {
                                    Ok(child) => drop(child),
                                    Err(e) => {
                                        tracing::warn!(error = %e, "could not open the map");
                                    }
                                }
                            }
                            None => {
                                tracing::error!("RONOCTUA_XDG_OPEN was not set at build time");
                            }
                        }
                        self.ask_paint();
                        return;
                    }
                    Doing::OpenWith(path, which) => {
                        if let Some(entry) = self.catalogue.iter().find(|e| e.id == which).cloned()
                        {
                            let (program, args) = crate::apps::command_for(&entry, &path);
                            self.spawn(&program, &args, entry.working_directory.as_deref());
                        }
                        self.ask_paint();
                        return;
                    }
                    // AN APPLICATION FROM THE SIDEBAR, through the launcher's
                    // own `command_for` — never a second reader of a `.desktop`
                    // file's `Exec`. Same arm as the footer's, same reason.
                    Doing::Launch(which) => {
                        if let Some(entry) = self.catalogue.iter().find(|e| e.id == which).cloned()
                        {
                            let (program, args) =
                                crate::apps::command_for(&entry, std::path::Path::new(""));
                            self.spawn(&program, &args, entry.working_directory.as_deref());
                        } else {
                            let m = self.texts.t_with(
                                ronoctua_i18n::key::FILES_SIDEBAR_GONE,
                                &[("name", which.as_str())],
                            );
                            self.say(m);
                        }
                        self.ask_paint();
                        return;
                    }
                    // FORMATTING OPENS THE BUFFER, from this door too — and
                    // through the same re-exec every other window uses.
                    Doing::Format(id) => {
                        if let Ok(me) = std::env::current_exe()
                            && let Ok(w) = std::env::var("RONOCTUA_BUFFER_FORMAT")
                            && !w.is_empty()
                        {
                            self.spawn(&me.to_string_lossy(), &["buffer".to_owned(), w, id], None);
                        } else {
                            let m = self.texts.t(ronoctua_i18n::key::ECHO_NO_BUFFER);
                            self.say(m);
                        }
                        self.ask_paint();
                        return;
                    }
                    Doing::Terminal(dir) => {
                        if let Ok(me) = std::env::current_exe() {
                            self.spawn(
                                &me.to_string_lossy(),
                                &["terminal".to_owned()],
                                Some(&dir.to_string_lossy()),
                            );
                        }
                        self.ask_paint();
                        return;
                    }
                    Doing::RootTerminal(dir) => {
                        self.root_terminal(&dir);
                        self.ask_paint();
                        return;
                    }
                    Doing::Copy(text) => {
                        let ok = crate::buffer::copy_to_clipboard(&text).is_ok();
                        let m = self.texts.t(if ok {
                            ronoctua_i18n::key::FILES_COPIED
                        } else {
                            ronoctua_i18n::key::FILES_NOT_COPIED
                        });
                        self.say(m);
                        self.ask_paint();
                        return;
                    }
                    other => {
                        let clip = self.view.clip().clone();
                        if matches!(other, Doing::Paste(_)) {
                            self.view.clip_used();
                        }
                        match crate::files::carry_out(other, &self.sender, &clip) {
                            Errand::Done => self.view.reread(),
                            Errand::Say(e) => {
                                let m = self.texts.t_with(e.key, &e.borrowed());
                                self.say(m);
                            }
                        }
                    }
                }
            }
        }
        self.follow();
        self.follow_the_place();
        self.ask_paint();
    }

    /// Points the watch at whatever directory the view is on now.
    ///
    /// Called after anything that could have navigated, rather than at the one
    /// place that navigates: `View::enter` is inside the view, which knows
    /// nothing about watches and should not start. It is a `PathBuf` compare and
    /// nothing else when the place has not moved, which is what makes calling it
    /// on every keypress honest.
    fn follow_the_place(&mut self) {
        if let Some(n) = self.notice.as_mut() {
            n.look_at(self.view.where_());
        }
    }

    /// Opens a file with whatever handles its type.
    fn open_the_file(&mut self, path: &std::path::Path) {
        let of_type = ronoctua_files::mime::of_path(path, ronoctua_files::Kind::File, true);
        let Some(entry) = crate::apps::opens(&self.catalogue, of_type).cloned() else {
            let m = self.texts.t_with(
                ronoctua_i18n::key::FILES_NOTHING_OPENS,
                &[("type", of_type)],
            );
            self.say(m);
            return;
        };
        let (program, args) = crate::apps::command_for(&entry, path);
        self.spawn(&program, &args, entry.working_directory.as_deref());
    }

    /// A root shell in this directory, through polkit.
    fn root_terminal(&mut self, dir: &std::path::Path) {
        let (Ok(me), Some(pkexec), Some(shell)) = (
            std::env::current_exe(),
            option_env!("RONOCTUA_PKEXEC"),
            option_env!("RONOCTUA_RESCUE_SHELL"),
        ) else {
            let m = self.texts.t(ronoctua_i18n::key::FILES_NO_PKEXEC);
            self.say(m);
            return;
        };
        self.spawn(
            &me.to_string_lossy(),
            &[
                "terminal".to_owned(),
                pkexec.to_owned(),
                "--keep-cwd".to_owned(),
                shell.to_owned(),
            ],
            Some(&dir.to_string_lossy()),
        );
    }

    /// Starts a program, in a scope of its own.
    ///
    /// Through `foreign::launch` and a NAMED thread: the manual is explicit
    /// that `systemd-run --scope` "is synchronous, and will return only when
    /// the command finishes". Waiting here would freeze this window for the
    /// whole life of whatever it opened — the defect the app launcher already
    /// paid for once.
    fn spawn(&mut self, program: &str, args: &[String], cwd: Option<&str>) {
        let Ok(scope) = crate::foreign::scope_name() else {
            tracing::error!("no scope name; nothing opened");
            return;
        };
        let cmd = crate::foreign::scope_command(&scope, cwd, program, args);
        // `direct` is what runs when there is no `systemd-run` — the nested
        // loop's case. It is the same command without the wrapper.
        let mut direct = vec![program.to_owned()];
        direct.extend(args.iter().cloned());
        if let Err(e) = std::thread::Builder::new()
            .name("files-open".to_owned())
            .spawn(move || {
                let _ = crate::foreign::launch(&scope, &direct, &cmd);
            })
        {
            tracing::error!(error = %e, "could not start the open thread");
        }
    }

    /// What a thread came back with.
    fn reported(&mut self, report: crate::files::Report) {
        match report {
            crate::files::Report::Done => self.view.reread(),
            crate::files::Report::Measured(name, bytes) => self.view.measured(&name, bytes),
            crate::files::Report::Failed(why) => self.say(why),
            crate::files::Report::Refused(which, why) => {
                if matches!(which, ronoctua_disks::Refusal::AlreadyMounted) {
                    self.view.reread();
                } else {
                    let e = self.view.refusal_said(which, &why);
                    let m = self.texts.t_with(e.key, &e.borrowed());
                    self.say(m);
                }
            }
            crate::files::Report::Mounted(at) => self.view.mounted(&at),
            // WHAT IS INSIDE THE FILE. It arrives on the same channel as
            // everything else a thread came back with, which is what keeps this
            // door with one place to receive and not two.
            crate::files::Report::Inspected(looked) => {
                self.view.looked(*looked);
                // AND THE PLAYER IS ASKED AGAIN, because THIS is the moment the
                // answer arrives — and the answer is what says the file moves.
                //
                // Without it the player only ever started when the cursor moved
                // AFTER the answer: on the file the cursor was already resting
                // on, the wish became askable a fraction of a second too late
                // and nothing ever asked again. The shots caught it —
                // *"the preview drew the same pixels a second apart"* — and
                // nothing else could have, because every unit test drives
                // `wants_motion` by hand.
                self.follow_the_motion();
                self.follow_the_turn();
            }
            crate::files::Report::Racked(rack) => {
                // AND THE SIDEBAR'S FILE, HERE. This process has no clock — a
                // disk signal is the only thing that wakes it on its own — so
                // the cheapest honest moment to notice that the footer pinned
                // something is when a disk moves. It is one `stat`, and it is
                // not the whole answer: see `DEVELOPMENT.md`.
                if self.view.sidebar_stale() {
                    self.view.reread_sidebar();
                }
                self.view.racked(&rack);
            }
            // SOMEBODY ELSE TOUCHED THE DIRECTORY. Answered with a plain
            // re-read and NOT with `stale()`: that asks the directory's mtime,
            // which a file rewritten in place does not move — and refusing the
            // kernel's word in favour of the poll's would drop exactly what the
            // watch was added to catch.
            //
            // The sidebar keeps its `stale()`, and that asymmetry is deliberate:
            // `save_sidebar` writes our OWN file, so an unconditional re-read
            // here would throw the sidebar's cursor away every time something is
            // pinned. See `View::sidebar_stale`.
            crate::files::Report::Stirred => {
                self.view.reread();
                if self.view.sidebar_stale() {
                    self.view.reread_sidebar();
                }
            }
        }
        // A MOUNT MOVES THE VIEW. `Report::Mounted` goes into the directory it
        // just mounted, so the watch has to follow from here too and not only
        // from the key that was pressed.
        self.follow_the_place();
        self.ask_paint();
    }

    /// One key, however it arrived — pressed, or repeated by the clock.
    ///
    /// One body and not two: a repeat that took a different path is a key that
    /// does something slightly different the second time, which is the kind of
    /// difference nobody looks for.
    fn typed(&mut self, event: KeyEvent) {
        let Some(t) = key_of(&event, self.mods) else {
            return;
        };
        // `Esc` CLOSES THE OPENED LINE FIRST, exactly as it does in the other
        // door: the drawer is the outermost thing a person opened, so it is
        // the first thing the outermost gesture shuts. The view's own `Esc`
        // ladder is one step behind and untouched.
        if matches!(t.sym, ronoctua_keymap::Sym::Esc) && self.fold.close() {
            self.ask_paint();
            return;
        }
        // A key clears the last message: it answered something that is no
        // longer what is being done.
        self.message = None;
        let step = self.view.key(t);
        self.apply(step);
    }

    /// Is the pointer inside the opened line's band?
    ///
    /// Line 0 and the rows under it — they are one thing, so a wheel on the
    /// summary row is a wheel on the drawer.
    fn pointer_on_fold(&self, y: f64) -> bool {
        let f = self.metric();
        #[allow(clippy::cast_possible_truncation)]
        let Some(line) = f.line_at(y as f32) else {
            return false;
        };
        line < self.first_of_region()
    }

    /// Is the pointer on the control that opens line 0?
    ///
    /// It asks the cell that was DRAWN, for the reason the footer's twin
    /// gives: what the pointer hits has to be what the eye saw.
    fn fold_at(&self, x: f64, y: f64) -> bool {
        let Some(at) = self.fold_control else {
            return false;
        };
        let f = self.metric();
        #[allow(clippy::cast_possible_truncation)]
        if f.line_at(y as f32) != Some(0) {
            return false;
        }
        #[allow(clippy::cast_possible_truncation)]
        let cell = f.column_at(x as f32);
        // The air beside the glyph is part of the target — a target the size
        // of the ink is a target one has to aim at.
        cell + 1 >= at
    }

    /// The opened line's bar, as the rectangle one may GRAB.
    ///
    /// # Why it exists at all
    ///
    /// Because it did not, and the owner found that from the screen: the bar
    /// on the opened hint line was drawn and could not be dragged. This door
    /// resolves a press by walking the manager's SECTIONS (`bar_at_point` over
    /// `Geometry::bars`), and line 0 belongs to no section — it is the door's
    /// own line, above the region. So nothing this window did could ever see
    /// that bar, and pressing it fell through to the row underneath.
    ///
    /// The footer's door has had this since the drawer existed; this is its
    /// twin, in this window's geometry.
    ///
    /// The TRACK and not the thumb: `Scroll::offset_at` maps any point of the
    /// track to a position, so aiming at a handle two rows tall is not
    /// required of anybody — the same promise the region's bars already make.
    ///
    /// This is the whole extent, which is what the DRAG maps against. Where it
    /// may be GRABBED is one line shorter — see `on_fold_thumb`.
    fn fold_track(&self) -> Option<(f32, f32, f32, f32)> {
        if !self.fold.open || !self.fold_scroll.cut() {
            return None;
        }
        let f = self.metric();
        let top = f.y_of(0);
        let tall = f.line() * f32::from(crate::fold::rows(self.fold).saturating_add(1));
        Some((f.width - f.cell(), top, f.cell(), tall))
    }

    /// Is the pointer somewhere the bar may be GRABBED?
    ///
    /// # Why line 0 is not part of it
    ///
    /// Because the control that opens the line is drawn there, in the same
    /// cell: the chevron sits at the end of line 0 and the bar runs down the
    /// last cell of the whole band, so on that one line they are the same
    /// target. Found by pressing it — the drawer shut instead of scrolling.
    ///
    /// The control wins, because it is the thing one aims AT: it is two cells
    /// of glyph and air, and it is the only way to close the drawer with the
    /// mouse. The bar loses one line of grab area and nothing else — the drag
    /// still maps against the FULL track, so a drag begun below can walk right
    /// up to the top.
    fn on_fold_thumb(&self, x: f64, y: f64) -> bool {
        let Some((bx, by, bw, bh)) = self.fold_track() else {
            return false;
        };
        let line = self.metric().line();
        let (from, tall) = (by + line, bh - line);
        if tall <= 0.0 {
            return false;
        }
        #[allow(clippy::cast_possible_truncation)]
        let (px, py) = (x as f32, y as f32);
        px >= bx && px < bx + bw && py >= from && py < from + tall
    }

    /// Drags the opened line to where the pointer is.
    ///
    /// `Scroll::offset_at` is the inverse of the call that drew the thumb,
    /// which is what stops the handle and the content disagreeing.
    fn fold_drag_to(&mut self, y: f64) {
        let Some((_, top, _, tall)) = self.fold_track() else {
            return;
        };
        let f = self.metric();
        #[allow(clippy::cast_possible_truncation)]
        let at = y as f32 - top;
        self.fold.offset = self.fold_scroll.offset_at(at, tall, f.line());
        self.ask_paint();
    }

    /// Columns and rows from the size in pixels.
    ///
    /// Integer arithmetic, without measuring text: the cell is exactly 0.600 ×
    /// the font size, including in the Nerd Fonts.
    /// This door's measurements — see `chrome::Frame`, which is the one place a
    /// token becomes a pixel.
    ///
    /// It is built rather than stored because it is derived: `width` and
    /// `height` are the only facts, and a stored copy of what they imply is a
    /// second home for them.
    #[allow(clippy::cast_precision_loss)]
    fn metric(&self) -> crate::chrome::Frame {
        crate::chrome::Frame::of(
            self.width as f32,
            self.height as f32,
            self.first_of_region(),
        )
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
        // AND THE TWO THREADS THAT DRAW INTO THE PANEL. `follow_the_motion`'s
        // own comment says it: the pose changes from a key, from the wheel,
        // from a drag and from the panel being resized, "and enumerating
        // those is how one gets missed". A change of size resizes the panel
        // without a `configure` — the window's pixels did not move, its cells
        // did — so this is a fifth way in, and leaving it out would have the
        // film keep the frame size of the old box.
        self.follow_the_motion();
        self.follow_the_turn();
        self.ask_paint();
    }

    fn recount(&mut self) {
        // THROUGH THE FRAME, and not through a fourth spelling of the cell.
        // This function used to say `SIZE_LARGE * CELL_WIDTH_EM` — one of the
        // three spellings of one number that the survey found living in this
        // tree, beside `metrics::cell_width` and `chrome::cell_width`.
        let f = self.metric();
        let (cell, line) = (f.cell(), f.line());
        let usable = f.width - 2.0 * cell * f32::from(CONTENT_MARGIN);
        self.columns = if cell > 0.0 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let n = ((usable / cell) as u16).max(1);
            n
        } else {
            1
        };
        self.lines = if line > 0.0 { f.lines().max(1) } else { 1 };
        self.follow();
    }

    /// Which row and cell a pointer position lands on.
    /// Where a point is, in the region's own coordinates.
    #[allow(clippy::cast_possible_truncation)]
    fn line_and_cell(&self, x: f64, y: f64) -> Option<(usize, u16)> {
        // THROUGH THE FRAME, which is the inverse of what the drawing used.
        // Spelled out here, as it was, the pointer had its own division and its
        // own cast — and a press on the first pixel of a cell resolved to the
        // cell before it, because `column * cell` and `x / cell` are not each
        // other's inverse in `f32` without the snap `Frame` applies.
        let f = self.metric();
        if x < 0.0 || y < 0.0 {
            return None;
        }
        let line = f.line_at(y as f32)?;
        let margin = f64::from(f.x_of(CONTENT_MARGIN));
        let cell = f.column_at((x - margin).max(0.0) as f32);
        // LINE 0 BELONGS TO NO COLUMN: it is the one line above the region.
        let k = line.checked_sub(self.first_of_region())?;
        Some((usize::from(k), cell))
    }

    /// Points everything the frame reads at `(x, y)`.
    ///
    /// ONE function, and it was written out three times — on the motion, on
    /// each of the two presses. Four numbers had to be set together and the day
    /// one of them was added is the day two of the three would have been left
    /// without it.
    fn aim(&mut self, x: f64, y: f64) {
        // WHICH SHORTCUT OF THE HINT LINE, first: it is ABOVE the region, so
        // `line_and_cell` answers nothing up there and everything below would
        // never be asked. Read off what the last frame DREW — see
        // `chrome::Hinted`.
        let now = self
            .hinted
            .under(x, y)
            .and_then(|id| crate::chrome::hint_sentence(id, &self.texts));
        if self.hinted.say(now) {
            self.ask_paint();
        }
        self.under_pointer = self.target_at(x, y);
        // AND WHETHER IT IS ON THE GLOBE, remembered where it was resolved —
        // the wheel arrives with less than this and must not ask again.
        self.view.pointing_globe(matches!(
            self.under_pointer.as_ref().map(|t| &t.hit),
            Some(crate::files::Hit::Globe | crate::files::Hit::GlobeDot)
        ));
        let at = self.line_and_cell(x, y);
        self.cell_under_pointer = at.map(|(_, c)| c);
        self.line_under_pointer = at.map(|(l, _)| l);
        // AND WHICH THUMBNAIL, so a film under the hand starts its tour. It
        // opens no file — see `View::hover_thumb`.
        let geo = self.geometry();
        self.view.hover_thumb(geo.columns, geo.room(), at);
        // AND WHICH SECTION, which is what decides the film's two buttons are
        // drawn: they appear under the pointer, and their CELLS are reserved
        // either way so nothing moves.
        //
        // ASKED OF `pane_at`, which needs neither a field nor a ROW — it is
        // arithmetic over the cell. `band_row` was the answer for one round and
        // it refuses a row with no fields on it, so the pointer crossing the
        // blank part of a panel said "nowhere" and every control that comes out
        // under the hand went with it. See `files::View::pane_at`.
        let pane = self.line_under_pointer.and_then(|line| {
            self.view.pane_at(
                self.columns,
                self.room(),
                line,
                self.cell_under_pointer.unwrap_or(0),
            )
        });
        self.view.pointing(pane);
        // AND WHICH ROW OF THE LISTING, which is what decides whose name
        // walks: the owner asked for the hover to move a name too long for its
        // column, as well as the cursor. Through the TARGET, which is the
        // resolution that already knows which row of which section a cell is.
        self.view.pointing_at(
            self.under_pointer
                .as_ref()
                .filter(|t| t.pane == crate::files::Pane::Listing && t.row != usize::MAX)
                .map(|t| t.row),
        );
    }

    /// What is under a point, asked of the view.
    fn target_at(&self, x: f64, y: f64) -> Option<crate::files::Target> {
        let (line, cell) = self.line_and_cell(x, y)?;
        self.view
            .hit(self.columns, self.room(), line, cell, &self.texts)
    }

    /// Which section's bar a POINT is on — the same rectangles that were drawn.
    fn bar_at_point(&self, x: f64, y: f64) -> Option<(crate::files::Pane, crate::files::Axis)> {
        // THE SAME RECTANGLES THE FRAME DREW, from the same value. Asking
        // `Geometry` here and in `frame()` is what makes "where it is drawn"
        // and "where it is grabbed" one answer instead of two that agree by
        // habit.
        let geo = self.geometry();
        let (cell, line_px) = (geo.metric.cell(), geo.metric.line());
        let chrome = self.view.chrome_rows(self.columns, &self.texts);
        let sections = self
            .view
            .sections(&chrome, geo.room(), self.columns, &self.texts);
        let panes = self.view.panes(self.columns);
        geo.bars(panes, &sections, cell, line_px)
            .into_iter()
            .find(|(_, _, r)| {
                let (bx, by, bw, bh) = *r;
                x >= f64::from(bx)
                    && x < f64::from(bx + bw)
                    && y >= f64::from(by)
                    && y < f64::from(by + bh)
            })
            .map(|(p, axis, _)| (p, axis))
    }

    fn assemble_gpu(&mut self) {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        };
        let (Some(disp), Some(surf)) = (
            std::ptr::NonNull::new(self.conn.backend().display_ptr().cast::<std::ffi::c_void>()),
            std::ptr::NonNull::new(
                self.window
                    .wl_surface()
                    .id()
                    .as_ptr()
                    .cast::<std::ffi::c_void>(),
            ),
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
        // THE SYSTEM'S ONE SIZE — see `ronoctua_tokens::text::size`. It used to be
        // `SIZE_LARGE`, on the argument that the footer is peripheral
        // reference and this is what one reads.
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
        // The quad pipeline is what draws the focused row's background. Its
        // failure costs the highlight, not the window.
        match ronoctua_render::quad::Quads::new(&g.device, g.config.format) {
            Ok(q) => self.quads = Some(q),
            Err(e) => tracing::error!(error = %e, "the quad pipeline failed"),
        }
        // AND THE IMAGE PIPELINE, beside it and for the same reason: its
        // failure costs the picture, not the window. The preview then draws the
        // type's glyph, which is what it draws for a file that has none.
        match ronoctua_render::image::Images::new(&g.device, g.config.format) {
            Ok(im) => self.images = Some(im),
            Err(e) => tracing::error!(error = %e, "the image pipeline failed"),
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
    /// batch is finished. A hundred `Motion` events in one batch come out as
    /// one frame, drawn from the LAST of them.
    fn settle(&mut self) {
        if std::mem::take(&mut self.owed) {
            self.paint();
        }
    }

    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        // WHAT THE PANELS NEED, asked ONCE PER FRAME and in one place — the
        // same arrangement the footer has. Every other one was a list of the
        // ways the cursor can move, and the one that got forgotten would be a
        // panel showing the previous file with nothing saying why.
        self.look_again();
        // A MESSAGE, A MENU ROW OR A FILTER TAKES THE HINT'S LINE, and then
        // there is no shortcut on it to point at.
        //
        // ASKED HERE and not inside the frame, because the row the explanation
        // costs comes out of the REGION (`first_of_region`) and the frame
        // measures the region on its first line. Cleared in there, the region
        // would be laid out one row short of what it drew — a blank line under
        // the message, for as long as nothing else asked for a frame.
        //
        // The pointer's own field explanation is not in this list on purpose:
        // it only exists while the pointer is over the REGION, and a pointer
        // cannot be there and on the hint line at once.
        if self.message.is_some() || self.view.cursor_explains().is_some() || self.view.filtering()
        {
            self.hinted.say(None);
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        // THE DRAWING ANSWERS TWO THINGS BACK: where it put the control that
        // opens the line, and how far the wrapped text ran. Both depend on a
        // width that only the drawing knows, and both are what the pointer
        // asks about afterwards — kept, and not worked out a second time.
        let mut scene = self.scene();
        let painted = frame(&mut scene);
        let (control, scroll, wrapped) =
            (scene.fold_control, scene.fold_scroll, scene.fold_wrapped);
        self.fold_control = control;
        self.fold_scroll = scroll;
        self.fold_wrapped = wrapped;
        self.present(painted);
        // AND ONCE MORE IF LINE 0 JUST OPENED: the region's first line is
        // asked for before the frame, and the wrap is only known inside it.
        // `fold::settle` answers once and then never again, so nothing loops —
        // and nothing the pointer does can reopen the question, which is the
        // whole of `Fold::held`.
        if crate::fold::settle(&mut self.fold, self.fold_wrapped) {
            self.ask_paint();
        }
        // AND THE GLOBE IS ASKED FOR AFTER THE FRAME, for the reason the
        // walking name's clock is: what the stage has to draw is the box the
        // LAYOUT measured, and the layout is this frame.
        //
        // `look_again` runs at the top of this function and tells the stage
        // what it knew BEFORE the frame — which, the first time a coordinate
        // appears, is nothing at all: `measured_globe` is called inside the
        // drawing. So the order was: subject arrives, ask (with no box), draw
        // (measure the box), and then silence — the picture appeared only if
        // something else asked for a second frame, which on a still screen is
        // the pointer moving. The window door showed it plainly: a panel with
        // the globe's lines reserved and nothing in them.
        self.follow_the_turn();
        // AND THE WALKING NAME'S CLOCK, asked for AFTER the frame: whether
        // there is a name to walk is written down by the layout, and the
        // layout is this frame.
        self.arm_the_walk();
        // AND THE REPLACED PICTURES HAVE NOW BEEN UPLOADED. After the frame for
        // the same reason: what asks for thumbnails runs at the TOP of this
        // function, and a flag cleared there is a flag the painter never sees.
        self.view.thumbs_uploaded();
    }

    /// Hangs the clock that walks a name too long for its column.
    ///
    /// The footer's twin, and the argument is written out there
    /// (`Footer::arm_the_walk`): armed while a name is walking and dropped by
    /// its own tick when none is. This door has no clock at all at rest — that
    /// is what lets several of these windows be open at once — so the
    /// alternative here is not a second timer but a first one.
    fn arm_the_walk(&mut self) {
        if self.walk_armed || !self.view.sliding() {
            return;
        }
        let armed = self.handle.insert_source(
            calloop::timer::Timer::from_duration(crate::slide::STEP),
            move |_, (), state: &mut Self| {
                if !state.view.sliding() {
                    state.walk_armed = false;
                    return calloop::timer::TimeoutAction::Drop;
                }
                // A TICK THAT MOVES NOTHING ASKS FOR NO FRAME: the walk rests
                // a second at each end, which at this step is five ticks of
                // standing still.
                if state.view.slide(std::time::Instant::now()) {
                    state.ask_paint();
                }
                calloop::timer::TimeoutAction::ToDuration(crate::slide::STEP)
            },
        );
        match armed {
            Ok(_) => self.walk_armed = true,
            Err(e) => tracing::warn!(error = %e, "could not hang the walking name's clock"),
        }
    }

    /// The window's arithmetic, as a value.
    ///
    /// # Why this is a struct and not eight methods on the window
    ///
    /// Because a method on `FilesWindow` cannot be tested: the window owns a
    /// Wayland `Connection`, a `Window` and a `Gpu`, and there is no way to
    /// stand one up in a unit test. That is the whole reason this file had
    /// ZERO of them while `files.rs` had three hundred and forty-seven — and
    /// it is also why five of the defects the owner reported were reported
    /// from his screen rather than caught here.
    ///
    /// [`Geometry`] is a plain value: a frame, how many columns the margins
    /// leave, and the two numbers the view asks for. Everything the door
    /// decides about WHERE follows from it, so everything the door decides
    /// about where can be asserted on.
    fn geometry(&self) -> Geometry {
        let panes = self.view.panes(self.columns);
        Geometry {
            metric: self.metric(),
            columns: self.columns,
            strip: panes.strip,
            chrome: self.chrome_lines(),
        }
    }

    /// Everything a frame is made of, borrowed out of the window.
    fn scene(&mut self) -> Scene<'_> {
        let geo = self.geometry();
        Scene {
            geo,
            message: self.message.clone(),
            cell_under_pointer: self.cell_under_pointer,
            line_under_pointer: self.line_under_pointer,
            under_pointer: self.under_pointer.as_ref(),
            fold: self.fold,
            fold_control: None,
            fold_scroll: self.fold_scroll,
            fold_wrapped: self.fold_wrapped,
            hinted: &mut self.hinted,
            view: &mut self.view,
            texts: &self.texts,
        }
    }

    /// The half that owns the GPU, and the only half that cannot be tested.
    ///
    /// It takes what [`frame`] produced and does nothing but hand it over:
    /// no arithmetic, no decision about where anything goes. That division is
    /// the point — a defect in this function is a defect in wgpu's arguments,
    /// and a defect anywhere else has a test that can reach it.
    fn present(&mut self, painted: Painted) {
        let Painted {
            spans,
            quads,
            over,
            pictures,
        } = painted;
        #[allow(clippy::cast_possible_truncation)]
        let under = (quads.len() - over) as u32;
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
        if let Some(im) = self.images.as_mut() {
            im.clear();
            let preview_key = self.view.preview_key();
            let globe_key = self.view.globe_key();
            for (key, _) in &pictures {
                // WHICH PICTURE THIS KEY IS, asked of the VIEW and not worked
                // out here: the choice between "replace it every frame" and
                // "only when it is missing" belongs where it can be made once.
                //
                // A MOVING FILE REPLACES THE TEXTURE UNDER THE SAME KEY, every
                // frame — which is what keeps an animation to ONE slot of the
                // renderer's ceiling. A key per frame would fill the cache with
                // one file's film and evict every icon. The globe is the same
                // kind of picture: it is redrawn as the hand moves.
                if preview_key.as_deref() == Some(key.as_str()) {
                    if let Some(frame) = self.view.live_frame() {
                        im.upload_it(&gpu.device, &gpu.queue, key, frame);
                    } else if im.missing(key)
                        // THE STILL ONE ONLY GOES UP WHEN IT IS MISSING: a
                        // preview is up to a mebibyte, and `prepare` is
                        // sixteen bytes.
                        && let Some(rgba) = self.view.preview_pixels()
                    {
                        im.upload_it(&gpu.device, &gpu.queue, key, rgba);
                    }
                } else if globe_key.as_deref() == Some(key.as_str())
                    && let Some(rgba) = self.view.globe_frame()
                {
                    im.upload_it(&gpu.device, &gpu.queue, key, &rgba);
                } else if (im.missing(key) || self.view.thumb_moves(key))
                    // A STILL THUMBNAIL ONLY WHEN IT IS MISSING, for the reason
                    // the still preview gives: the key carries the file, the
                    // size and the theme, so the same key is the same picture.
                    // A screenful of them re-uploaded every frame would be tens
                    // of mebibytes a second across the bus for nothing.
                    //
                    // AND A MOVING ONE EVERY TIME, under the SAME key — which
                    // is what keeps an animation to one slot of the renderer's
                    // ceiling instead of filling the cache with one file's film.
                    && let Some(rgba) = self.view.thumb_pixels(key)
                {
                    im.upload_it(&gpu.device, &gpu.queue, key, rgba);
                }
            }
            im.prepare(&gpu.queue, logical_w, logical_h, &pictures);
        }
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
            q.prepare(&gpu.device, &gpu.queue, logical_w, logical_h, &quads);
        }
        let quads_ref = self.quads.as_ref();
        // QUADS, THEN IMAGES, THEN THE FEW QUADS THAT GO ON TOP, THEN TEXT.
        //
        // There is no depth test: whoever draws later ends up on top. The
        // picture sits on the background and UNDER the text, which has to stay
        // legible over a light photograph — and under the film's transport,
        // which is drawn ON the film by design.
        let images_ref = self.images.as_ref();
        if let Err(e) = gpu.frame(
            || crate::hidpi::told_before_present(scale, told, &surface),
            |pass| {
                if let Some(q) = quads_ref {
                    q.draw_range(pass, 0, under);
                }
                if let Some(im) = images_ref {
                    im.draw(pass);
                }
                if let Some(q) = quads_ref {
                    q.draw_range(pass, under, u32::MAX);
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
}

/// What a frame is made of, before anything knows about a GPU.
pub(crate) struct Painted {
    /// Every run of text, in the grid's own coordinates.
    pub spans: Vec<ronoctua_render::Span>,
    /// Every rectangle: bands, rules, dividers and the scrollbars.
    ///
    /// The last [`Painted::over`] of them are drawn AFTER the pictures — see
    /// `ronoctua_render::quad::Quads::draw_range`.
    pub quads: Vec<ronoctua_render::quad::Quad>,
    /// How many of `quads`, at the end, belong ON TOP of the pictures.
    ///
    /// There is no depth test: whoever draws later ends up on top. Everything
    /// else in this manager is a background and belongs under the picture; the
    /// film's transport is the one thing that does not, because the owner asked
    /// for the film to keep the whole panel and be controlled where it plays.
    pub over: usize,
    /// The textures this frame draws, and where each goes.
    ///
    /// A LOT AND NOT ONE, because this manager now draws two pictures at once:
    /// the preview's, in the right-hand panel, and the globe's, at the top of
    /// the info panel beside it. `Images::prepare` has always taken a slice —
    /// the footer's door already hands it several — so the renderer needed no
    /// change; it was this field that capped it at one.
    pub pictures: Vec<(String, ronoctua_render::image::Area)>,
}

/// The window's geometry: one home for every number about WHERE.
///
/// See [`FilesWindow::geometry`] for why it is a value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Geometry {
    /// The surface, through the frame that owns the two rails.
    pub metric: crate::chrome::Frame,
    /// How many columns the content's margins leave.
    pub columns: u16,
    /// How many lines the bottom strip takes, or zero when the panels are
    /// columns on the right.
    pub strip: u16,
    /// How many lines the listing's own chrome takes — two, or none at all
    /// when a menu, a picker or a sheet is up.
    pub chrome: u16,
}

impl Geometry {
    /// How many lines the region has, the strip already taken off it.
    pub fn room(&self) -> usize {
        usize::from(
            self.metric
                .lines()
                .max(1)
                .saturating_sub(self.metric.first + self.strip),
        )
    }

    /// How many rows of listing fit: the region, less the listing's chrome.
    pub fn list_room(&self) -> usize {
        self.room().saturating_sub(usize::from(self.chrome))
    }

    /// How many lines the PANELS are given — what the facts scroll inside.
    pub fn panel_room(&self) -> usize {
        if self.strip > 0 {
            usize::from(self.strip)
        } else {
            self.room()
        }
    }

    /// The two rooms this door gives, for whatever asks about a bar.
    pub fn rooms(&self) -> crate::files::Room {
        crate::files::Room {
            // THE REGION'S OWN HEIGHT, chrome included — the sidebar shares
            // this number and really does show every line of it. The listing's
            // chrome comes off inside the view, which is the only place that
            // knows whether it is there. See `View::chrome_lines`.
            list: self.room(),
            panel: self.panel_room(),
        }
    }

    /// Where the LISTING's region ends: the strip's rule when there is a strip
    /// under it, and the surface's own bottom when there is not.
    pub fn list_bottom(&self, line_px: f32) -> f32 {
        if self.strip > 0 {
            f32::from(self.metric.first + u16::try_from(self.room()).unwrap_or(0)) * line_px
        } else {
            self.metric.bottom()
        }
    }

    /// EVERY SECTION'S SCROLLBAR, as rectangles — the drawing's and the
    /// pointer's one answer. See `files::Section::bar_box`.
    pub fn bars(
        &self,
        panes: crate::files::Panes,
        sections: &[crate::files::Section],
        cell: f32,
        line_px: f32,
    ) -> Vec<crate::files::Bar> {
        let edge = self.metric.width;
        let top = self.metric.y_of(self.metric.first);
        // WHERE THE SURFACE ENDS, and not where the last whole line does. See
        // `files::Section::bar_box`: the residue is the difference, and only
        // the last band in a column may take it.
        let bottom = self.metric.bottom();
        sections
            .iter()
            .filter_map(|sec| {
                let (x, wide) = crate::files::section_px(
                    panes,
                    sec.pane,
                    CONTENT_MARGIN,
                    self.columns,
                    cell,
                    edge,
                )?;
                // BOTH OF A SECTION'S BARS, and the axis with each: a pane
                // may have one going down and one going across, and a pointer
                // holding one of them is not holding the other.
                let down = sec
                    .bar_box(x + wide, top, bottom, cell, line_px)
                    .map(|r| (sec.pane, crate::files::Axis::Down, r));
                let across = sec
                    .wide_bar_box(x, top, bottom, wide, cell, line_px)
                    .map(|r| (sec.pane, crate::files::Axis::Across, r));
                // AND THE FILM'S, which is the same promise turned into time:
                // where the cursor is drawn is where it is grabbed.
                // AND THE FILM'S TRACK, which is what one grabs: `offset_at`
                // maps any point of it to a position, so aiming at a one-cell
                // cursor is not required of anybody.
                // THE SAME RECORD THE DRAWING USES, and that is the point of
                // it: this call had its arguments in the wrong order for as
                // long as the timeline existed, and every one of them was an
                // `f32`. See `files::Foot`.
                let time = sec
                    .time_track_box(crate::files::Foot {
                        left: x,
                        wide,
                        cell,
                        line_px,
                        bottom,
                    })
                    .map(|r| (sec.pane, crate::files::Axis::Time, r));
                Some([down, across, time])
            })
            .flatten()
            .flatten()
            .collect()
    }
}

/// Everything a frame is drawn FROM, borrowed for the length of one.
pub(crate) struct Scene<'a> {
    /// The arithmetic.
    pub geo: Geometry,
    /// The manager, which owns every offset and every list.
    pub view: &'a mut crate::files::View,
    /// The interface's words.
    pub texts: &'a ronoctua_i18n::Texts,
    /// What the last action had to say, if anything.
    pub message: Option<String>,
    /// Which cell of the region the pointer is over.
    pub cell_under_pointer: Option<u16>,
    /// Which line of the region the pointer is over.
    pub line_under_pointer: Option<usize>,
    /// What the pointer is resting on, already resolved by `View::hit`.
    pub under_pointer: Option<&'a crate::files::Target>,
    /// Whether line 0 is open, and how far it is scrolled.
    pub fold: crate::fold::Fold,
    /// Where the control that opens it was drawn — written by the drawing,
    /// read by the pointer. Out and not in, for the reason `fold_scroll` is.
    pub fold_control: Option<u16>,
    /// The opened band's scroll, as DRAWN.
    ///
    /// It travels out of the frame and not into it: the wrap depends on a
    /// width only the drawing knows, and the bar the pointer grabs has to be
    /// the bar the eye saw.
    pub fold_scroll: ronoctua_design::Scroll,
    /// How many rows the line's text came to — out, for the same reason.
    pub fold_wrapped: usize,
    /// The hint line, and the shortcut the pointer is resting on it.
    ///
    /// Borrowed MUTABLY, because the drawing is what writes down where each
    /// shortcut landed — the pointer then asks that, and not a second
    /// placement of its own. See `chrome::Hinted`.
    pub hinted: &'a mut crate::chrome::Hinted,
}

/// One frame, as spans and rectangles. No Wayland, no wgpu, no `self`.
pub(crate) fn frame(s: &mut Scene<'_>) -> Painted {
    let size = tokens::text::size();
    let margin = CONTENT_MARGIN;
    let mut spans: Vec<ronoctua_render::Span> = Vec::new();
    let mut quads: Vec<ronoctua_render::quad::Quad> = Vec::new();

    // LINE 0 IS THE ONE LINE THAT IS NOT A COLUMN'S.
    //
    // The owner asked for this order: *"the shortcut line goes first,
    // isolated at the top, and the path and filter line is folded into
    // the top of the listing column"*. It is the footer's
    // arrangement, where the hint line is the minibuffer's — above the
    // region and belonging to none of its columns — and the path is inside.
    //
    // So the path is not drawn here any more: it goes through
    // `files::View::region_rows` as the LISTING's chrome, which is what
    // leaves the sidebar and the panels their own first line instead of a
    // blank one under somebody else's path.
    //
    // LINE 0: what is typed, what the pointer explains, or what went wrong.
    //
    // The pointer's explanation WINS, which is the same precedence the
    // footer's message line uses: pointing at something is a question, and
    // a question asked now beats a message from before.
    // WHAT THE POINTER IS ON says it itself: `View::hit` resolved the
    // section, the row inside it and the field in one answer, so this line
    // no longer guesses which list the `y` belonged to.
    let explained = s
        .under_pointer
        .as_ref()
        .and_then(|t| t.explain.clone())
        // AND WHAT WENT WRONG WITH A FILM, BELOW IT — see the footer door's
        // twin: refusal is a value, and pointing at something is a question
        // asked now.
        .or_else(|| s.view.film_trouble())
        // AND WHAT WENT WRONG WITH A SHAPE, on the same line and for the same
        // reason: a blank panel and a refused one look identical, and only one
        // of them has a sentence.
        .or_else(|| s.view.shape_said())
        // AS PIECES, because a sentence about a fact has two halves — the name
        // of it and the fact itself — and they were drawn in one colour. See
        // `chrome::sentence`.
        .map(|e| crate::chrome::sentence(&e, s.texts));
    // THE QUERY IS NOT HERE. It goes at the end of the path, on the line
    // above — see `files::View::crumbs`. This line is for what the pointer
    // is resting on and what the last action said, and while the query
    // shared it the two took turns erasing each other.
    // THE MESSAGE COMES FIRST, for the reason the footer's own precedence
    // now gives: it is what the manager has to SAY about an action just
    // asked for, and behind the pointer's description it was unreachable in
    // the one situation it exists for — the hand on the mouse, the pointer
    // resting on the row that was just clicked.
    // AND WHAT THE MENU ROW UNDER THE CURSOR ACTUALLY DOES.
    //
    // The same second register the footer's door gained, and the same
    // reason: the rows are written for somebody starting out, and somebody
    // who already knows has nowhere to read which call and what can be
    // undone. It arrives by moving the CURSOR — the explanations here have
    // needed a pointer until now, and the owner's rule is that whatever the
    // mouse can do the keyboard can do.
    //
    // Behind the message and ahead of the pointer, exactly as over there: a
    // menu covers the listing, so with one open the pointer is resting on a
    // row and the cursor is on the choice about to be made.
    let does = s
        .view
        .cursor_explains()
        .map(|e| crate::chrome::sentence(&e, s.texts));
    // A MESSAGE IS ONE REGISTER, and saying so at the boundary is better than
    // making thirty `Step::Say` sites carry pieces they have nothing to say
    // about: what a view raised is an answer, whole.
    let second = s
        .message
        .clone()
        .map(|t| vec![crate::chrome::Piece::said(t)])
        .or(does)
        .or(explained);
    // THE LINE AS PIECES, whatever is on it — the same representation the
    // footer's minibuffer uses, and for the same reason: the drawer is drawn
    // by the painter that drew the line, so a key is the same colour three
    // rows down as it is on the first.
    let hinting = second.is_none() && !s.view.filtering();
    let line_pieces: Vec<crate::chrome::Piece> = if hinting {
        let mut out = s.view.hint(s.texts);
        // AND THE KEYS THAT WALK IT, once it is open — see `fold::teaching`.
        if s.fold.open {
            out.extend(crate::fold::teaching(s.texts));
        }
        out
    } else {
        second.clone().unwrap_or_default()
    };
    let line_text: String = line_pieces.iter().map(|p| p.text.as_str()).collect();
    // WHERE THE SHORTCUTS OF THIS LINE LAND, collected while they are drawn.
    let mut marks: Vec<crate::chrome::Mark> = Vec::new();
    // THE ROOM IS THE WHOLE OF `columns`: `recount` took BOTH margins off the
    // width before counting cells, so a line that starts `margin` in and
    // runs `columns` wide ends `margin` short of the edge. It used to take
    // the margin off a second time here, which cut the hint one margin
    // early — found by the review of the notification window, whose paint
    // was written against this one.
    let plain_room = s.geo.columns;
    let opens = s.fold.open
        || crate::fold::wanted(&line_text, plain_room.saturating_sub(crate::fold::WIDTH));
    let room = plain_room.saturating_sub(if opens { crate::fold::WIDTH } else { 0 });
    // HOW TALL THE LINE WOULD BE, written down for the geometry — see
    // `fold_wrapped`. Measured whenever there is a control, open or not,
    // because opening has to know the answer before it draws.
    s.fold_wrapped = if opens {
        crate::fold::wrap_pieces(&line_pieces, room).len()
    } else {
        0
    };
    if s.fold.open {
        // OPENED, the line is its own first row and the rest follow under it.
        let (showing, scroll) = crate::fold::opened(&line_pieces, room, s.fold);
        for (n, row) in showing.into_iter().enumerate() {
            let at = u16::try_from(n).unwrap_or(0);
            if hinting {
                spans.extend(crate::chrome::hint_spans(
                    &row,
                    ronoctua_render::Size::Px(size),
                    margin,
                    at,
                    room,
                ));
                // EVERY ROW OF THE DRAWER ANSWERS THE POINTER, not just the
                // first: opened, this line is where the whole table is
                // legible, which is where somebody goes looking.
                marks.extend(crate::chrome::hint_marks(&row, margin, at, room));
            } else {
                spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::Px(size),
                    text: row.into_iter().map(|p| p.text).collect(),
                    column: margin,
                    line: at,
                    color: tokens::color::TEXT_SUBTLE_STRONG,
                    edge: false,
                    foot: false,
                });
            }
        }
        // AND ITS BAR, when the wrap ran past the band. One rectangle, from
        // `Scroll` — the same call every other bar in this system makes.
        if scroll.cut() {
            let f = s.geo.metric;
            let top = f.y_of(0);
            let bottom =
                top + f.line() * f32::from(u16::try_from(scroll.lines).unwrap_or(u16::MAX));
            if let Some((bx, by, bw, bh)) = scroll.bar(f.width, top, bottom, f.cell(), f.line()) {
                quads.push(ronoctua_render::quad::Quad {
                    x: bx,
                    y: by,
                    width: bw,
                    height: bh,
                    color: tokens::color::RULE_STRONG,
                });
            }
        }
        s.fold_scroll = scroll;
    } else if let Some(pieces) = second {
        // THE SAME PAINTER THE HINT USES, in the runs' own registers — and it
        // used to be a second painter with one hand-written token, which is
        // what kept a sentence about a fact in a single colour. See
        // `chrome::sentence` and the footer door's twin.
        spans.extend(crate::chrome::hint_spans(
            &pieces,
            ronoctua_render::Size::Px(size),
            margin,
            0,
            room,
        ));
    } else if hinting {
        // WITH NOTHING TYPED AND NOTHING TO SAY, the hint. Typing is not a
        // key and so has no line in the manual; without this the one
        // gesture the list is built around is the one thing nothing on
        // screen mentions.
        //
        // It is the ONE thing on this line that is not drawn here: the keys
        // and the prose are two colours, and both doors ask
        // `chrome::hint_spans` for them so that neither can answer on its own
        // — which is exactly what they were doing when the owner photographed
        // them side by side.
        spans.extend(crate::chrome::hint_spans(
            &line_pieces,
            ronoctua_render::Size::Px(size),
            margin,
            0,
            room,
        ));
        marks.extend(crate::chrome::hint_marks(&line_pieces, margin, 0, room));
    }
    // WHERE EACH SHORTCUT LANDED — what the pointer asks, and what was drawn.
    s.hinted.took(marks);

    // AND THE SHORTCUT UNDER THE POINTER, SPELT OUT, on the line below the
    // hint: the line the owner asked for. It costs the region one row for as
    // long as the pointer is on a key — `first_of_region` is where that is
    // paid — and nothing above it moves.
    if let Some(said) = s.hinted.said() {
        spans.push(ronoctua_render::Span {
            size: ronoctua_render::Size::Px(size),
            text: ronoctua_render::layout::elide_end(said, plain_room),
            column: margin,
            line: FIRST_OF_REGION + crate::fold::rows(s.fold),
            color: crate::chrome::Register::Explain.color(),
            edge: false,
            foot: false,
        });
    }

    // THE CONTROL THAT OPENS THE LINE, flush against the right edge. No box
    // and no border (invariant 15): a glyph on the line that already exists.
    // AND THE MANAGER IS TOLD, so `Tab` may land there — and may not, when
    // there is nothing to land on.
    s.view.fold_is_shown(opens);
    let lit = s.view.fold_focused();
    s.fold_control = opens.then_some(s.geo.columns.saturating_sub(1) + margin);
    if let Some(at) = s.fold_control {
        let chevron = if s.fold.open {
            tokens::icon::COLLAPSE
        } else {
            tokens::icon::EXPAND
        };
        spans.push(ronoctua_render::Span {
            size: ronoctua_render::Size::OnTheCell(crate::surface::chrome_size_of(chevron)),
            text: chevron.to_string(),
            column: at,
            line: 0,
            // WITH THE KEYBOARD ON IT, the accent — one glyph has no row to
            // band, so the colour is the whole of the focus.
            color: if lit {
                tokens::color::STATE_ACCENT
            } else if s.fold.open {
                tokens::color::TEXT_SUBTLE_STRONG
            } else {
                tokens::color::TEXT_SUBTLE_DEFAULT
            },
            edge: false,
            foot: false,
        });
    }

    // THE HEADER, on the line above the listing. It names the columns and
    // says which one is sorting; clicking it sorts, as it does in the
    // footer. Not drawn over the menu, the picker or the detail sheet:
    // none of those has columns.
    let line_px = tokens::metrics::line_height(size);

    // THE LISTING — only the rows that fit, and not the whole directory.
    //
    // The same reason the footer door has: building a row costs a mode's
    // nine cells, a size, a date, an owner and a group, and this window
    // draws `room()` of them. See `files::View::rows_from` for the numbers.
    // THE LISTING'S OWN CHROME, handed to `region_rows` rather than drawn
    // above the whole region: the path and the header are the listing's,
    // and the other three columns start at the region's first line.
    let chrome = s.view.chrome_rows(s.geo.columns, s.texts);
    // THE SECTIONS, each painted on its own rail.
    //
    // Nothing here composes a line out of four columns' fields. Every
    // section says where it is and what it holds, and this loop draws it
    // there — so a section's length, its offset and its top reach no other
    // section, by construction rather than by care.
    // A MODAL VIEW IS ONE LIST — the menu, the picker and the detail sheet
    // cover the listing, so there are no columns to lay out. It is a `Section`
    // like the other four now, built by `View::sections_of`.
    //
    // This door used to assemble a FIXTURE for it here, with `total` set to the
    // room so `Scroll::cut()` could never be true — a section that by
    // construction had no bar, over a list that is routinely longer than the
    // window. And the fixture was only half the door: `bar_at_point` asked
    // `view.sections(...)` unconditionally, so while a menu was up the pointer
    // could grab a bar this frame had never drawn and start dragging the
    // listing behind it. One question, one answer.
    let sections = s
        .view
        .sections(&chrome, s.geo.room(), s.geo.columns, s.texts);
    // WHERE THE SIDEBAR ENDS, asked once for the whole frame. It is what
    // the rule is drawn at and what decides which half the hover band
    // covers; asking twice would be two answers on the day one of them was
    // edited.
    let split = s.view.split(s.geo.columns);
    // THE WHOLE DIVISION, asked once for the frame. `split` is the sidebar's
    // half of it and the band needs the panels' half too — see
    // `files::band_of`, which is where both doors' arithmetic lives.
    let panes_now = s.view.panes(s.geo.columns);
    if split > 0 {
        let cell = tokens::metrics::cell_width(size);
        // THE LINE SITS IN THE MIDDLE OF THE CELL RESERVED FOR IT — see
        // `files::rule_x`. Drawn at the cell's left edge, as it was, the
        // whole cell became air on the listing's side and none at all on
        // the sidebar's.
        quads.push(crate::chrome::divider(
            crate::files::rule_x(split, margin, cell),
            s.geo.metric.y_of(s.geo.metric.first),
            s.geo.list_bottom(line_px),
        ));
    }
    // THE BAND, once per line that the pointer is on — and it is the
    // POINTER's section that decides which line, so a band never lights a
    // row of a list the pointer is not over.
    // WHERE THE KEYBOARD IS, by the STRONGER of the two bands.
    //
    // It had none. `files::Row::focused` was written at seven sites in the view
    // and read at none outside the tests, so the whole signal was
    // `files::colour_of` accenting the file's NAME — one field of seven, the
    // same colour a folder already has, and in the action menu nothing at all.
    // The owner reported it from the screen: *"ao trocar de item com as setas
    // nenhuma alteração/indicador visual é feita"*.
    //
    // The line comes from `View::focus_line`, which resolves it through the
    // same `Section::line_of` the pointer's band uses — a row is not a line,
    // and two conversions would put one row in two places. The cell comes from
    // the PANE that has the keyboard, never from the pointer: `band_of` picks
    // its side of the sidebar's rule from a cell, and a focus band placed by
    // the mouse would change sides when the hand moved.
    let focus_at = s.view.focus_line(s.geo.columns, s.geo.room());
    if let Some((pane, line)) = focus_at {
        #[allow(clippy::cast_possible_truncation)]
        let y = s.geo.metric.first + line as u16;
        let cell = tokens::metrics::cell_width(size);
        #[allow(clippy::cast_precision_loss)]
        // AND SIDE BY SIDE IT IS THE CELL and not the column. `band_cells_of`
        // answers `None` for a list, which is every band there was, so this is
        // the same rectangle it always drew until there is a shelf.
        let (at, wide) = s.view.focus_cells(s.geo.columns, s.geo.room()).map_or_else(
            || {
                crate::files::band_for(
                    panes_now,
                    pane,
                    margin,
                    s.geo.columns,
                    cell,
                    s.geo.metric.width,
                )
            },
            |c| crate::files::band_px(c, margin, cell),
        );
        #[allow(clippy::cast_precision_loss)]
        let tall = line_px * s.view.focus_lines(s.geo.columns, s.geo.room()) as f32;
        quads.push(ronoctua_render::quad::Quad {
            x: at,
            y: f32::from(y) * line_px,
            width: wide,
            height: tall,
            color: tokens::color::SURFACE_SELECTION,
        });
    }
    // AND THE LISTING KEEPS A QUIETER ONE while the keyboard is away, because
    // the panels are about that row and nothing else on screen said so. See
    // `View::resting_line`.
    let resting = s.view.resting_line(s.geo.columns, s.geo.room());
    if let Some(line) = resting {
        let y = s.geo.metric.first + line as u16;
        let cell = tokens::metrics::cell_width(size);
        #[allow(clippy::cast_precision_loss)]
        let (at, wide) = s
            .view
            .resting_cells(s.geo.columns, s.geo.room())
            .map_or_else(
                || {
                    crate::files::band_for(
                        panes_now,
                        crate::files::Pane::Listing,
                        margin,
                        s.geo.columns,
                        cell,
                        s.geo.metric.width,
                    )
                },
                |c| crate::files::band_px(c, margin, cell),
            );
        #[allow(clippy::cast_precision_loss)]
        let tall = line_px * s.view.resting_lines(s.geo.columns, s.geo.room()) as f32;
        quads.push(ronoctua_render::quad::Quad {
            x: at,
            y: f32::from(y) * line_px,
            width: wide,
            height: tall,
            color: tokens::color::SURFACE_HOVER,
        });
    }
    // WHERE THE POINTER IS, and the question is which LINE — never which field.
    //
    // This asked `View::hit`, which ends in `field_at(cell)?`, and a field
    // covers the width of its own TEXT. The listing's name column is
    // `Width::Fill`, so on a wide window most of a row belongs to a field that
    // covers only the filename's letters and the rest of the row resolved to
    // nothing at all: *"as vezes o item é destacado, as vezes não, e muitos
    // nunca são destacado"*. The footer never had it — its band asks the line —
    // and `View::band_row` is that rule for both doors.
    //
    // The FIELD's own accent (below) still needs the deep answer, and that is
    // right: it says which word the click would reach, and over dead space it
    // reaches none.
    let hover_at = s.line_under_pointer.and_then(|line| {
        let cell = s.cell_under_pointer.unwrap_or(0);
        let (pane, at) = s.view.band_row(s.geo.columns, s.geo.room(), line, cell)?;
        s.view
            .band_belongs_at(
                cell,
                s.geo.columns,
                pane == crate::files::Pane::Preview || pane == crate::files::Pane::Info,
            )
            .then_some((pane, at))
    });
    if let Some((pane, at)) = hover_at
        && let Some(sec) = sections.iter().find(|s| s.pane == pane)
        // ONE BAND WHERE IT IS ONE ROW. The weaker colour painted over the
        // stronger would dim the very row one is standing on, which is the
        // opposite of what either band is for. Same rule in the footer.
        && focus_at != Some((pane, at))
    {
        #[allow(clippy::cast_possible_truncation)]
        let y = s.geo.metric.first + at as u16;
        let cell = tokens::metrics::cell_width(size);
        // THE SECTION'S OWN EDGES, in pixels.
        //
        // `band_of` answers for the sidebar and the listing, which is what
        // it was written for; down in the strip the panels are neither, and
        // it fell through to the listing's bounds. The section already says
        // where it begins and ends, so it is asked — and a band lights the
        // column the pointer is in, never the one beside it.
        #[allow(clippy::cast_precision_loss)]
        let (at, wide) = match pane {
            crate::files::Pane::Preview | crate::files::Pane::Info if sec.top > 0 => {
                crate::files::panel_px(
                    panes_now,
                    if matches!(pane, crate::files::Pane::Preview) {
                        crate::files::Panel::Preview
                    } else {
                        crate::files::Panel::Info
                    },
                    margin,
                    s.geo.columns,
                    cell,
                    s.geo.metric.width,
                )
                .unwrap_or((0.0, 0.0))
            }
            _ => crate::files::band_of(
                panes_now,
                margin,
                s.geo.columns,
                s.cell_under_pointer,
                cell,
                s.geo.metric.width,
            ),
        };
        let (at, wide) = s
            .line_under_pointer
            .and_then(|line| {
                s.view.band_cells_at(
                    s.geo.columns,
                    s.geo.room(),
                    line,
                    s.cell_under_pointer.unwrap_or(0),
                )
            })
            .map_or((at, wide), |c| crate::files::band_px(c, margin, cell));
        #[allow(clippy::cast_precision_loss)]
        let tall = line_px
            * s.view.band_lines_at(
                s.geo.columns,
                s.geo.room(),
                s.line_under_pointer.unwrap_or(0),
                s.cell_under_pointer.unwrap_or(0),
            ) as f32;
        quads.push(ronoctua_render::quad::Quad {
            x: at,
            y: f32::from(y) * line_px,
            width: wide,
            height: tall,
            // THE WEAKER OF THE TWO. Two rows, two colours, and which is
            // which is legible; one row, and only the focus is drawn.
            color: tokens::color::SURFACE_HOVER,
        });
    }
    // WHAT THE POINTER LIT, found ONCE and before the lines.
    //
    // A thumbnail's name is up to two runs on two LINES of one cell, so the run
    // that lights WITH the one under the hand is not on the line being drawn —
    // it cannot be found from inside this loop. See `files::lit_with`.
    let lit_hit = hover_at.and_then(|(pane, at)| {
        let sec = sections.iter().find(|s| s.pane == pane)?;
        let line = sec.content.get(at.checked_sub(sec.top)?)?;
        let k = s.cell_under_pointer.and_then(|c| line.field_index_at(c))?;
        line.fields.get(k).map(|f| f.hit.clone())
    });
    for sec in &sections {
        for (i, line) in sec.content.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let y = s.geo.metric.first + (sec.top + i) as u16;
            // THE SAME ROW THE BAND LIT, and not a second opinion about it:
            // through the target, the accent and the band disagreed over every
            // cell that belongs to no field.
            let lit = hover_at == Some((sec.pane, sec.top + i));
            // WHICH field, asked ONCE and by the walk the click makes. Asking
            // `covers` per field lights every field that MAY own the cell, and
            // the gap rule gives a packed cell to two of them — the owner read
            // two permission letters lighting where one should. See
            // `Row::field_index_at`.
            let alight = lit
                .then(|| s.cell_under_pointer.and_then(|c| line.field_index_at(c)))
                .flatten();
            for (which, f) in line.fields.iter().enumerate() {
                // THE FIELD UNDER THE POINTER LIGHTS UP, so the sentence on
                // the line above has a visible subject. A colour and not a
                // box — invariant 15.
                let under = alight == Some(which)
                    || lit_hit
                        .as_ref()
                        .is_some_and(|h| crate::files::lit_with(f, h));
                spans.push(ronoctua_render::Span {
                    // THE ROW'S OWN SIZE, through the one function that turns
                    // lines into one — see `files::run_size`. Nought is the
                    // ordinary size, which is every row but a thumbnail's
                    // fallback glyph.
                    size: crate::files::run_size(f.ink, size, line_px),
                    text: f.text.clone(),
                    column: margin + f.at,
                    line: y,
                    color: if under {
                        tokens::color::STATE_ACCENT
                    } else {
                        f.color
                    },
                    edge: false,
                    // AND THE TRANSPORT'S ROW IS ANCHORED TO THE FOOT, which is
                    // the only row in this manager that is — see `Row::foot`.
                    foot: line.foot,
                });
            }
        }
    }

    // THE PANELS' VERTICAL RULES, and the horizontal one over the strip.
    //
    // Quads, exactly like the sidebar's: a one-pixel line inside the same
    // surface. There is no panel, no border and no shadow (invariant 15) —
    // separation by whitespace and a rule is the only separation this
    // project has.
    let panes = panes_now;
    let cell = tokens::metrics::cell_width(size);
    let strip_top = s.geo.metric.first + u16::try_from(s.geo.room()).unwrap_or(0);
    if panes.right > 0 || panes.strip > 0 {
        // WHERE THEY START AND WHERE THEY STOP, and the stop is the
        // SURFACE'S — never a count of lines times a fractional line
        // height, which is what left every one of them short of the edge.
        let (rule_from, rule_to) = if panes.right > 0 {
            (
                s.geo.metric.y_of(s.geo.metric.first),
                s.geo.list_bottom(line_px),
            )
        } else {
            (s.geo.metric.y_of(strip_top), s.geo.metric.bottom())
        };
        // AND THE ONE THAT BEGINS THE FOCUSED SECTION IS LIT — the mark that
        // replaced the panels' titles, and then their lit scrollbars. See
        // `files::focus_rule`.
        let lit = crate::files::focus_rule(panes, s.geo.columns, s.view.pane());
        let mut drawn = false;
        for at in panes.rules(s.geo.columns) {
            let x = crate::files::rule_x(at, margin, cell);
            if Some(at) == lit {
                drawn = true;
                quads.push(crate::chrome::divider_lit(x, rule_from, rule_to));
            } else {
                quads.push(crate::chrome::divider(x, rule_from, rule_to));
            }
        }
        // AND THE ONE THE STRIP HAS NO ROOM FOR. With the panels below the
        // listing the first of them begins at cell nought and `Panes::rules`
        // draws nothing there — rightly, because there is nothing to its left.
        // A boundary that exists because the section is FOCUSED is a different
        // statement, and without it the preview would be the one section the
        // mark could never reach.
        if let Some(at) = lit
            && !drawn
        {
            quads.push(crate::chrome::divider_lit(
                crate::files::rule_x(at, margin, cell),
                rule_from,
                rule_to,
            ));
        }
    }
    // EVERY SECTION'S BAR, from the section itself.
    //
    // Three of these were written out by hand, each with its own idea of
    // where its column ends, and the pointer test had a fourth. The owner
    // found every disagreement: the facts' bar unreachable in one door, the
    // listing's unreachable in the other, one drawn a cell past the border
    // and gone off screen, and a thumb that stopped half way down its own
    // column. `Section::bar_box` is the rectangle now — drawn here, and
    // tested against by `FilesWindow::bar_at_point`.
    // THE FILM'S OWN THREE ARE NOT HERE. They go over the picture — see the
    // overlay at the end of this function — and a rectangle pushed here would
    // be painted before it and hidden by it.
    #[allow(clippy::cast_precision_loss)]
    for (sec, axis, rect) in s.geo.bars(panes, &sections, cell, line_px) {
        let _ = sec;
        if matches!(axis, crate::files::Axis::Time) {
            continue;
        }
        quads.push(ronoctua_render::quad::Quad {
            x: rect.0,
            y: rect.1,
            width: rect.2,
            height: rect.3,
            color: tokens::color::RULE_STRONG,
        });
    }
    if panes.strip > 0 {
        // THE LINE THAT SAYS WHERE THE LISTING ENDS. Without it the strip's
        // first row reads as one more entry that happens to say something
        // else.
        #[allow(clippy::cast_precision_loss)]
        quads.push(crate::chrome::rule(
            s.geo.metric.width,
            s.geo.metric.y_of(strip_top),
        ));
        // AND THE STRIP'S ROWS ARE THE SECTIONS' OWN, drawn in the loop
        // above at their own `top`. Drawing them a second time here is
        // what put the facts under the header AND at the bottom at once.
    }

    // THE PICTURE, fitted into the preview's box without stretching.
    //
    // A textured quad on the SAME surface — no window and no popup
    // (invariant 14), no border, radius or shadow (invariant 15). A
    // deformed photograph is worse than a small one: whoever looks does not
    // know whether what is skewed is the image or the panel.
    // `preview_box` is asked as the QUESTION "is there a preview at all";
    // where it goes is `files::panel_px`'s answer, in pixels.
    // AND THE BOX IS MEASURED WHETHER OR NOT THERE IS ANYTHING IN IT YET.
    //
    // It used to be measured inside the closure below, after `preview_key`, and
    // that DEADLOCKED: a film has no still picture any more, so there is
    // nothing to draw until the player produces a frame — and the player is not
    // started until the box has been measured, because the frame is decoded at
    // the box's shape. Each waited for the other and no film ever played. The
    // shots caught it, and nothing else could have: the box is a door's number.
    let preview_box = panes.preview_box(s.geo.columns).and_then(|_| {
        // THE PANELS' OWN ROWS, and never the whole surface: this door puts
        // three lines above the listing — the path, the message and the header
        // — and a picture centred in a box that included them would sit a line
        // and a half too high. `files::panel_band` is the one place that
        // arithmetic lives, and the footer had it wrong. ONLY THE TOP is wanted
        // here: where the box ENDS is the surface's business.
        let (top, _) = crate::files::panel_band(
            s.geo.metric.first,
            strip_top - s.geo.metric.first,
            panes.strip,
        );
        #[allow(clippy::cast_precision_loss)]
        let (box_x, box_wide) = crate::files::panel_px(
            panes,
            crate::files::Panel::Preview,
            margin,
            s.geo.columns,
            cell,
            s.geo.metric.width,
        )?;
        Some((top, box_x, box_wide))
    });
    if let Some((top, _, box_wide)) = preview_box {
        let air = f32::from(tokens::space::FILES_PICTUREAIR);
        let box_tall = (s.geo.metric.bottom() - f32::from(top) * line_px).max(0.0);
        s.view
            .measured_preview(box_wide - air - air, box_tall - air - air);
    }
    let picture = preview_box.and_then(|(top, box_x, box_wide)| {
        let key = s.view.preview_key()?;
        // THE PICTURE THAT IS DRAWN IS THE PICTURE THAT IS MEASURED — the
        // footer door's twin, and the defect written out there: a film measured
        // by its poster comes out stretched to the poster's shape.
        let rgba = s.view.preview_now()?;
        // AND THE AIR ON ALL FOUR SIDES IS THE VIEW'S. `fit_into` centres
        // and that was never the whole promise: whichever dimension is the
        // tight one comes out exactly as long as the box, which in this
        // door was the vertical one — the picture centred AND against the
        // top edge. See `files::picture_in`.
        Some((
            key,
            crate::files::picture_in(
                ronoctua_render::image::Area {
                    // THE REGION'S RAILS, not the text's — see
                    // `files::panel_px`. The box is what lies between the
                    // rules, and where there is no rule it is the window's
                    // own edge.
                    x: box_x,
                    y: f32::from(top) * line_px,
                    width: box_wide,
                    // TO THE SURFACE'S BOTTOM, and not `tall * line_px`.
                    //
                    // The strip is the last thing this door draws, and a
                    // count of lines times a line height falls short of the
                    // edge — the same arithmetic that left every divider
                    // one pixel high of it. Here it left a whole line: the
                    // box was four lines while the region under the rule
                    // was five, so the picture was centred in the top four
                    // fifths of it and read as pushed up. Measured on the
                    // window door: 5 px of air above and 28 below.
                    //
                    // `tall` still decides where the box BEGINS, through
                    // `top`; what it may not decide is where the surface
                    // ends, because it does not know.
                    //
                    // AND THE TRANSPORT TAKES NOTHING OFF IT. The controls are
                    // painted OVER the film's last line now, on a band of their
                    // own — the owner asked for the picture to have *"toda a
                    // altura possível"*, and a line reserved for controls that
                    // are only there under the pointer is a line spent on
                    // nothing the rest of the time.
                    height: (s.geo.metric.bottom() - f32::from(top) * line_px).max(0.0),
                },
                rgba.width,
                rgba.height,
            ),
        ))
    });

    // AND THE GLOBE, in the info panel beside it — across its top, or in the
    // leftmost column of the expanded sheet.
    //
    // The same three steps the preview takes — measure the box, tell the view,
    // ask for the picture — because the picture that is DRAWN has to be the
    // picture that was MEASURED. WHICH of the two shapes it is, is
    // `View::globe_place`'s answer and not this door's. See `files::globe_px`.
    let (info_top, _) = crate::files::panel_band(
        s.geo.metric.first,
        strip_top - s.geo.metric.first,
        panes.strip,
    );
    let place = s.view.globe_place();
    let globe = place.and_then(|(lines, cells, _)| {
        crate::files::globe_px(
            panes,
            margin,
            s.geo.columns,
            (cell, line_px, s.geo.metric.width),
            info_top,
            (lines, cells),
        )
    });
    if let Some(area) = globe {
        // AND HOW MUCH OF ITS TOP HAS SCROLLED AWAY, which is what keeps the
        // map's SIZE as its rows leave. See `crate::globe::paint`.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let gone = (place.map_or(0, |(_, _, cut)| cut) as f32 * line_px) as u32;
        // THE BOX THE PICTURE IS DRAWN IN, and not the box it was measured
        // from — see `files::globe_texture`.
        s.view.measured_globe(
            crate::files::globe_texture(area),
            gone,
            ronoctua_tokens::theme::current().position(),
        );
    }
    let globe_picture = globe.and_then(|area| {
        let key = s.view.globe_key()?;
        let rgba = s.view.globe_frame()?;
        Some((key, crate::files::picture_in(area, rgba.width, rgba.height)))
    });
    // AND EVERY THUMBNAIL ON SCREEN. `Painted::pictures` has been a batch since
    // the globe joined the preview; this is the same batch, longer.
    #[allow(clippy::cast_precision_loss)]
    let thumbnails = s.view.grid_pictures(
        s.geo.columns,
        s.geo.room(),
        margin,
        cell,
        line_px,
        f32::from(s.geo.metric.first) * line_px,
    );
    let pictures: Vec<(String, ronoctua_render::image::Area)> = picture
        .into_iter()
        .chain(globe_picture)
        .chain(thumbnails)
        .collect();

    // THE FILM'S CONTROLS, LAST AND ON TOP.
    //
    // Three rectangles that go over the picture: the half-transparent band that
    // makes the glyphs legible against any frame, the timeline's track, and the
    // cursor on it. They arrive and leave together with the pointer, which is
    // what the owner asked for — *"tudo deve sumir e aparecer apenas com o
    // hover do mouse"*.
    let over = {
        let before = quads.len();
        if s.view.transport_showing() {
            let edge = s.geo.metric.width;
            for sec in &sections {
                let Some((_x, _wide)) = crate::files::section_px(
                    panes,
                    sec.pane,
                    CONTENT_MARGIN,
                    s.geo.columns,
                    cell,
                    edge,
                ) else {
                    continue;
                };
                let mut band = |rect: (f32, f32, f32, f32), color| {
                    quads.push(ronoctua_render::quad::Quad {
                        x: rect.0,
                        y: rect.1,
                        width: rect.2,
                        height: rect.3,
                        color,
                    });
                };
                let (fx, fwide) = sec.fields_px(CONTENT_MARGIN, cell);
                // EVERY PART OF THE TRANSPORT FROM THE SAME FOOT — see
                // `files::transport_top`. The row itself is placed by the text
                // renderer counting up from the bottom (`Row::foot`), and a
                // rectangle counted down from the region's top lands a
                // remainder away from it.
                let foot = crate::files::Foot {
                    left: fx,
                    wide: fwide,
                    cell,
                    line_px,
                    bottom: s.geo.metric.bottom(),
                };
                if let Some(r) = crate::files::transport_band(sec, foot) {
                    band(r, tokens::color::SURFACE_TRANSPORT);
                }
                // THE TRACK IS THE QUIETER TONE, because it is the part that
                // does not move: the cursor on it is what says where the film
                // is, and two rectangles in one colour say nothing.
                if let Some(r) = sec.time_track_box(foot) {
                    band(r, tokens::color::RULE_STRONG);
                }
                if let Some(r) = sec.time_bar_box(foot) {
                    band(r, tokens::color::TEXT_PRIMARY);
                }
            }
        }
        // AND THE RUBBER BAND, with them and for the same reason: it is a
        // claim about what is UNDER it, and a grid is pictures edge to edge.
        // Drawn with the other backgrounds it would be invisible exactly
        // where it matters.
        //
        // AND THE FOLDER A DROP WOULD LAND IN, in the same ink and for the
        // same reason: both say *"this is what the button is about"*, and both
        // have to be legible over a picture.
        for claim in [
            s.view.sweep_box(s.geo.columns, s.geo.room()),
            s.view.landing_box(s.geo.columns, s.geo.room()),
        ]
        .into_iter()
        .flatten()
        {
            let (c0, c1, l0, l1) = claim;
            quads.push(crate::files::sweep_px(
                (c0, c1),
                (l0, l1),
                CONTENT_MARGIN,
                cell,
                line_px,
                s.geo.metric.y_of(s.geo.metric.first),
            ));
        }
        quads.len() - before
    };
    Painted {
        spans,
        quads,
        over,
        pictures,
    }
}

/// Where the listing starts, in cells from the window's edge.
///
/// THE ROW ALREADY BRINGS ITS OWN FIRST CELL: `files::layout_for` starts every
/// row at the gutter — the mark's cell, and the air the icon needs. What this
/// door owes is the DIFFERENCE up to the content's inset.
///
/// The arithmetic used to be written out here, and again in the notification
/// window, and NOT AT ALL in the footer — which is how one table came to begin
/// one cell in through one door and two through the other. It lives in
/// [`crate::chrome::FIELDS_LEAD`] now, with the three readers named.
///
/// The local name stays because the number is used three times in this file —
/// drawing, measuring how many columns fit, and deciding which cell the pointer
/// is over — and two of the three disagreeing is a tooltip about the
/// neighbouring field.
const CONTENT_MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// A `KeyEvent` as the manager's keymap sees it.
///
/// The same mapping `Footer::translate` does, minus the leader's inheritance:
/// that exists because the footer receives the keyboard with the leader still
/// held down, and this window is given focus like any other.
fn key_of(event: &KeyEvent, mods: Modifiers) -> Option<ronoctua_keymap::Key> {
    use ronoctua_keymap::{Key, Sym};
    let sym = match event.keysym {
        Keysym::Escape => Sym::Esc,
        Keysym::Return | Keysym::KP_Enter => Sym::Enter,
        Keysym::Tab | Keysym::ISO_Left_Tab => Sym::Tab,
        Keysym::BackSpace => Sym::Backspace,
        Keysym::space => Sym::Space,
        Keysym::Left => Sym::Left,
        Keysym::Right => Sym::Right,
        Keysym::Up => Sym::Up,
        Keysym::Down => Sym::Down,
        otherwise => Sym::Char(otherwise.key_char()?),
    };
    Some(Key {
        mods: ronoctua_keymap::Mods {
            ctrl: mods.ctrl,
            alt: mods.alt,
            // SHIFT DOES NOT ENTER FROM THE MODIFIERS, for the reason
            // `Footer::discount` gives: xkb has already applied it when
            // producing the character, so counting both would turn `M-S` into
            // `M-S-S` and never match. The two doors have to answer the same
            // key the same way, and this one did not — it was invisible only
            // while every key in the table was a bare letter and the notation
            // dropped the modifiers anyway.
            //
            // IT ENTERS FROM THE KEYSYM, and that is the same rule and not an
            // exception to it. `ISO_Left_Tab` is what xkb produces for
            // Shift+Tab — the application has already been made, exactly as it
            // is for `S` — so reading it here is reading what xkb decided
            // rather than guessing at a modifier.
            //
            // AND FROM THE MODIFIER FOR EVERY KEY THAT IS NOT A CHARACTER,
            // because there xkb has nothing to apply it TO: Shift+Left is
            // `Left` and the modifier is the only place the shift exists. The
            // rule above was written when `S-TAB` was the only shifted binding
            // and it read as "shift comes from the keysym"; what it actually
            // says is "shift is not counted twice", and an arrow cannot count
            // it twice. Without this, `S-<left>` arrived as a bare `<left>`
            // and went UP A DIRECTORY — caught on screen, turning a shape.
            shift: event.keysym == Keysym::ISO_Left_Tab
                || (mods.shift && !matches!(sym, Sym::Char(_))),
            logo: mods.logo,
        },
        sym,
    })
}

impl WindowHandler for FilesWindow {
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

impl SeatHandler for FilesWindow {
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
            // WITH REPEAT, and it had never been. `repeat_key` is written in
            // both doors and was documented as making holding `j` walk the
            // list — and SCTK only ever calls it for a keyboard created THIS
            // way, so it was dead code in both and no key repeated anywhere.
            // Found looking for why holding `S-<left>` turned a shape once and
            // then stopped.
            //
            // The rate and the delay are the COMPOSITOR's, arriving as
            // `RepeatInfo`; nothing here invents either.
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
            // AND THE SEAT'S DATA DEVICE, which is what carries a drag. It
            // hangs off the SEAT and so cannot be made before one arrives.
            if let Some(c) = self.carrying.as_mut() {
                c.took_a_seat(qh, &seat);
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

// ---------------------------------------------------------------------------
// THE DRAG — entries carried out, and files dropped in
// ---------------------------------------------------------------------------
//
// Three traits, and each is a thin translation into `files::View`. The DRAG's
// point comes in the surface's own pixels, so it goes through the same
// `line_and_cell` and the same `View::hit` the pointer does — what lights a
// folder under a drag and what a drop lands in are one answer, read twice.

impl smithay_client_toolkit::data_device_manager::data_device::DataDeviceHandler for FilesWindow {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        device: &wayland_client::protocol::wl_data_device::WlDataDevice,
        x: f64,
        y: f64,
        _: &wl_surface::WlSurface,
    ) {
        let Some(offer) = device
            .data::<smithay_client_toolkit::data_device_manager::data_device::DataDeviceData>()
            .and_then(|d| d.drag_offer())
        else {
            return;
        };
        // WHAT WE CAN TAKE, said out loud. A destination that accepts nothing
        // is a destination the compositor draws a "no" cursor over, which is
        // the right answer for a drag of plain text and the wrong one for a
        // drag of files.
        let takeable =
            offer.with_mime_types(|kinds| kinds.iter().any(|k| k == crate::carry::URI_LIST));
        offer.accept_mime_type(
            offer.serial,
            takeable.then(|| crate::carry::URI_LIST.to_owned()),
        );
        if !takeable {
            return;
        }
        // AND WHICH ACTION WE PREFER. `Move` unless the source only offers a
        // copy — the destination has a say, and a drop into a different
        // filesystem or out of a program that cannot give its file up is a
        // copy whatever anybody asked for.
        let wanted = if offer.source_actions.contains(DndAction::Move) {
            DndAction::Move
        } else {
            DndAction::Copy
        };
        offer.set_actions(DndAction::Copy | DndAction::Move, wanted);
        self.view.carrying_from_afar(wanted == DndAction::Copy);
        self.carried_to(x, y, wanted == DndAction::Copy);
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_data_device::WlDataDevice,
    ) {
        if self.view.carried_away() {
            self.ask_paint();
        }
    }

    fn motion(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        device: &wayland_client::protocol::wl_data_device::WlDataDevice,
        x: f64,
        y: f64,
    ) {
        let offer = device
            .data::<smithay_client_toolkit::data_device_manager::data_device::DataDeviceData>()
            .and_then(|d| d.drag_offer());
        let copies = offer
            .as_ref()
            .is_some_and(|o| o.selected_action == DndAction::Copy);
        self.carried_to(x, y, copies);
        // AND THE OFFER IS TOLD WHETHER THERE IS ANYWHERE TO GO. Accepting
        // everywhere and doing nothing on the drop is a cursor that says "yes"
        // over a file it cannot take — see `View::landing_now`.
        if let Some(offer) = offer {
            offer.accept_mime_type(
                offer.serial,
                self.takes_a_drop()
                    .then(|| crate::carry::URI_LIST.to_owned()),
            );
        }
    }

    /// THE SELECTION IS NOT THIS MODULE'S. `crate::clipboard` watches it
    /// through `zwlr_data_control`, for the reason written out there.
    fn selection(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_data_device::WlDataDevice,
    ) {
    }

    fn drop_performed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        device: &wayland_client::protocol::wl_data_device::WlDataDevice,
    ) {
        let offer = device
            .data::<smithay_client_toolkit::data_device_manager::data_device::DataDeviceData>()
            .and_then(|d| d.drag_offer());
        let Some(offer) = offer else {
            self.view.carried_away();
            return;
        };
        // OUR OWN DRAG IS NOT READ BACK THROUGH THE COMPOSITOR.
        //
        // A client that is both ends of a drag cannot read its own pipe from
        // inside this handler: the write is a Wayland event, and the read
        // blocks the loop that would deliver it — see `carry::Carrying::offering`
        // for the two log lines that measured it. A drag from somewhere else
        // IS read, with a deadline.
        let mine = self
            .carrying
            .as_ref()
            .and_then(crate::carry::Carrying::offering)
            .map(<[_]>::to_vec);
        let paths = mine.unwrap_or_else(|| {
            offer
                .receive(crate::carry::URI_LIST.to_owned())
                .ok()
                .and_then(crate::carry::read)
                .unwrap_or_default()
        });
        offer.finish();
        tracing::info!(dropped = paths.len(), "files were let go over the listing");
        let step = self.view.dropped(paths);
        self.apply(step);
    }
}

impl smithay_client_toolkit::data_device_manager::data_source::DataSourceHandler for FilesWindow {
    fn accept_mime(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_data_source::WlDataSource,
        _: Option<String>,
    ) {
    }

    fn send_request(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_data_source::WlDataSource,
        mime: String,
        pipe: smithay_client_toolkit::data_device_manager::WritePipe,
    ) {
        if let Some(c) = self.carrying.as_ref() {
            c.send(&mime, pipe);
        }
    }

    fn cancelled(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
        self.let_the_drag_go();
    }

    fn dnd_dropped(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
    }

    fn dnd_finished(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
        self.let_the_drag_go();
    }

    fn action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_data_source::WlDataSource,
        _: DndAction,
    ) {
    }
}

impl smithay_client_toolkit::data_device_manager::data_offer::DataOfferHandler for FilesWindow {
    fn source_actions(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut smithay_client_toolkit::data_device_manager::data_offer::DragOffer,
        _: DndAction,
    ) {
    }

    fn selected_action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        offer: &mut smithay_client_toolkit::data_device_manager::data_offer::DragOffer,
        action: DndAction,
    ) {
        // THE COMPOSITOR HAS DECIDED, and the line has to say the same thing
        // the drop will do.
        self.carried_to(offer.x, offer.y, action == DndAction::Copy);
    }
}

impl KeyboardHandler for FilesWindow {
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
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        // Holding `j` walks the list, which is what a listing wants — unlike
        // the footer, where repeating a leader key is always a mistake.
        //
        // SCTK delivers a repeat through the CALLBACK given to
        // `get_keyboard_with_repeat` and not through this method, so this is
        // the second door to the same room and both lead to `typed`.
        self.typed(event);
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

impl PointerHandler for FilesWindow {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let before = (self.under_pointer.clone(), self.cell_under_pointer);
        let mut left = None;
        let mut right = false;
        // BOTH COORDINATES: a bar going across is dragged by `x`.
        let mut dragged: Option<(f64, f64)> = None;
        let mut spun = false;
        let mut wheel_at: Option<f64> = None;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if self.dragging.is_some() {
                        dragged = Some(e.position);
                    }
                    // THE OPENED LINE'S BAR IS DRAGGED HERE and not through
                    // `dragged`, which carries a point to `View::drag_to` —
                    // and the view does not own this line.
                    if self.dragging_fold {
                        self.fold_drag_to(e.position.1);
                    }
                    // AND A SHAPE TURNS WITH THE HAND. The deltas are in
                    // PIXELS and become radians here, because the view has no
                    // idea how big a pixel is; a whole panel's width is about
                    // half a turn, which is the ratio that makes a piece feel
                    // attached to the pointer rather than geared to it.
                    if let Some((was_x, was_y)) = self.dragging_globe {
                        #[allow(clippy::cast_possible_truncation)]
                        let moved = self.view.pan_globe(
                            (e.position.0 - was_x) as f32,
                            (e.position.1 - was_y) as f32,
                        );
                        self.dragging_globe = Some(e.position);
                        if moved {
                            self.ask_paint();
                        }
                    }
                    if let Some((was_x, was_y)) = self.spinning {
                        let wide = f64::from(self.width.max(1));
                        let by = std::f64::consts::PI / wide;
                        #[allow(clippy::cast_possible_truncation)]
                        // WHERE THE HAND WENT, in radians of turn. Which way a
                        // yaw counts is `View::spin`'s business and written out
                        // there — a convention spelled at two call sites is a
                        // convention that gets one of them backwards, which is
                        // what happened.
                        let turned = self.view.spin(
                            ((e.position.0 - was_x) * by) as f32,
                            ((e.position.1 - was_y) * by) as f32,
                        );
                        self.spinning = Some(e.position);
                        if turned {
                            spun = true;
                        }
                    }
                    self.aim(e.position.0, e.position.1);
                    // AND A BUTTON THAT IS DOWN ON THE LISTING IS A GESTURE
                    // NOW, or still a click — see `files::View::moved_to`.
                    if let Some((line, cell)) = self.line_and_cell(e.position.0, e.position.1)
                        && self.view.moved_to(self.columns, self.room(), line, cell)
                    {
                        self.ask_paint();
                    }
                    // AND A DRAG THAT HAS JUST PICKED SOMETHING UP HANDS IT TO
                    // THE COMPOSITOR. From here on the pointer is its, and
                    // what this surface hears is `wl_data_device`.
                    if let Some((paths, copies)) = self.view.begun_carrying() {
                        let surface = self.window.wl_surface().clone();
                        let serial = self.pressed_at;
                        if let Some(c) = self.carrying.as_mut() {
                            c.begin(qh, &surface, serial, paths, copies);
                        }
                    }
                }
                PointerEventKind::Leave { .. } => {
                    // THE SENTENCE GOES WITH THE POINTER: one that leaves
                    // sends no motion on the way out.
                    if self.hinted.say(None) {
                        self.ask_paint();
                    }
                    self.under_pointer = None;
                    self.cell_under_pointer = None;
                    self.line_under_pointer = None;
                    self.view.pointing(None);
                    self.view.pointing_at(None);
                    // AND A BAND ENDS WHERE THE HAND LEFT IT. What it took was
                    // drawn, entry by entry, the whole way — undoing that
                    // silently is the worse of the two answers.
                    if self.view.gave_up() {
                        self.ask_paint();
                    }
                }
                PointerEventKind::Press { button, serial, .. } if button == LEFT_BUTTON => {
                    // THE PRESS'S OWN SERIAL, kept because a drag started from
                    // any other is refused by the compositor with nothing said
                    // on this side — see `carry::Carrying::begin`.
                    self.pressed_at = serial;
                    self.aim(e.position.0, e.position.1);
                    // THE LINE'S OWN CONTROL FIRST: it sits on line 0, which
                    // belongs to no section, so nothing below here would ever
                    // resolve it.
                    if self.fold_at(e.position.0, e.position.1) {
                        self.fold.toggle();
                        self.ask_paint();
                        continue;
                    }
                    // AND ITS BAR, for the same reason and in the same place:
                    // it is drawn on line 0, which no section owns.
                    if self.on_fold_thumb(e.position.0, e.position.1) {
                        self.dragging_fold = true;
                        self.fold_drag_to(e.position.1);
                        continue;
                    }
                    // A BAR TAKES THE PRESS, and the row does not get it: the
                    // thumb is a control, not a line of the list under it.
                    self.dragging = self.bar_at_point(e.position.0, e.position.1);
                    if self.dragging.is_some() {
                        dragged = Some(e.position);
                    } else if self
                        .under_pointer
                        .as_ref()
                        .is_some_and(|t| matches!(t.pane, crate::files::Pane::Preview))
                        && self.view.shape_here()
                    {
                        // A PRESS ON THE PIECE TAKES THE POINTER. After the
                        // bar, because a bar drawn over the panel is still a
                        // control; before the row, because a shape has no rows
                        // and the click would otherwise open the file — which
                        // is what pressing a picture does, and is exactly what
                        // somebody about to turn it does not want.
                        self.spinning = Some(e.position);
                    } else if self
                        .under_pointer
                        .as_ref()
                        .is_some_and(|t| matches!(t.hit, crate::files::Hit::Globe))
                    {
                        // A PRESS ON THE GLOBE TAKES THE POINTER — the footer door's
                        // twin, and a field of its own for the same reason. The
                        // DOT is not part of it, for the reason written there:
                        // it is a target, and a drag starting on it could never
                        // be the press that frames on the coordinate.
                        self.dragging_globe = Some(e.position);
                    } else if let Some((line, cell)) =
                        self.line_and_cell(e.position.0, e.position.1)
                        && self.view.pressed(
                            self.columns,
                            self.room(),
                            line,
                            cell,
                            crate::chrome::held(self.mods),
                            &self.texts,
                        )
                    {
                        // THE VIEW TOOK IT, and a press on the listing is no
                        // longer a click: until the pointer moves or the button
                        // comes up there is nothing that says which of the
                        // three gestures it is. The click fires on the RELEASE
                        // — see `files::View::pressed`.
                    } else {
                        left = self.under_pointer.clone();
                    }
                }
                // AND THE BUTTON COMING BACK UP LETS THE THUMB GO.
                //
                // There was no `Release` arm at all: once a bar had been
                // pressed, every motion afterwards went on dragging it, and
                // the row under the pointer never got another click. A control
                // one cannot let go of is worse than one that does not move.
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    self.dragging = None;
                    self.dragging_fold = false;
                    self.spinning = None;
                    self.dragging_globe = None;
                    // AND A PRESS THAT NEVER BECAME A GESTURE IS A CLICK, on
                    // what it landed on and not on what is under the pointer
                    // now — a hand that slipped a cell still clicked what it
                    // aimed at.
                    if let Some(at) = self.view.let_go() {
                        left = Some(at);
                    }
                }
                // THE RIGHT BUTTON OPENS THE MENU. It draws in this window's own
                // region, has no surface of its own and does not float — see the
                // note in `surface.rs`, where the same decision is written out.
                PointerEventKind::Press { button, .. } if button == RIGHT_BUTTON => {
                    // THE OTHER BUTTON ENDS A BAND FIRST. The menu is about
                    // `chosen()` and its title counts the marks, so one opened
                    // mid-sweep would be about a set that is still moving.
                    self.view.gave_up();
                    self.aim(e.position.0, e.position.1);
                    // THE PRESS ITSELF, and not the row it landed on: the
                    // subject is `under_pointer`, which was just resolved, and
                    // `None` there is an answer too — an empty directory has no
                    // field anywhere and still has a menu.
                    right = true;
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    // THE `stop` OF A CONTINUOUS GESTURE ZEROES WHAT WAS LEFT,
                    // or half a detent from the last one would add itself to
                    // the next and the first scroll would come out bigger than
                    // the finger asked for. Same shape as the footer's.
                    if vertical.stop || horizontal.stop {
                        self.detents = 0.0;
                    }
                    let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
                    self.detents += crate::surface::axis_detents(&vertical, line_px)
                        + crate::surface::axis_detents(&horizontal, line_px);
                    // WHERE THE WHEEL WAS: line 0 belongs to no section, so
                    // the resolved target cannot answer for it.
                    wheel_at = Some(e.position.1);
                }
                _ => {}
            }
        }
        // A SHAPE TURNED, which is neither a drag along a track nor a click:
        // it has already changed the view, so all that is left is to draw it
        // and to tell the stage.
        if spun {
            self.follow_the_turn();
            self.ask_paint();
        }
        // THE DRAG, before the row and before the wheel: while the thumb is
        // held, the pointer is not pointing at rows.
        if let Some((px, py)) = dragged
            && let Some((pane, axis)) = self.dragging
        {
            // AGAINST THE SECTION'S OWN BAND, and not the region's.
            //
            // The facts sit in the strip, below the listing: measured from the
            // region's top, every `y` inside them came out past the end and the
            // fraction saturated at one — the thumb jumped to the bottom and
            // stayed there, which reads exactly like a bar that does not work.
            let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
            let chrome = self.view.chrome_rows(self.columns, &self.texts);
            let found = self
                .view
                .sections(&chrome, self.room(), self.columns, &self.texts)
                .into_iter()
                .find(|s| s.pane == pane);
            // AND THE CLOCK'S WIDTH COMES FROM THE SECTION TOO. It is what
            // decides where the track ends, so grabbing it anywhere else would
            // read a position the cursor was never drawn at.
            let sec_clock = found.as_ref().map_or(0, |s| s.clock);
            let (top_line, lines) = found.map_or((0, self.room()), |s| (s.top, s.lines));
            // ALONG THE TRACK THAT WAS GRABBED. A bar going across is dragged
            // by `x` over the panel's own pixels; one going down by `y` over
            // its band's.
            let geo = self.geometry();
            let fraction = if matches!(axis, crate::files::Axis::Across | crate::files::Axis::Time)
            {
                let panes = self.view.panes(self.columns);
                let cell = geo.metric.cell();
                match crate::files::section_px(
                    panes,
                    pane,
                    CONTENT_MARGIN,
                    self.columns,
                    cell,
                    geo.metric.width,
                ) {
                    Some((x, wide)) if wide > 0.0 => {
                        // THE FILM'S TRACK IS INSET, and by the same answer the
                        // cursor was drawn at — see `files::track_px`.
                        let (x, wide) = if matches!(axis, crate::files::Axis::Time) {
                            match crate::files::track_px(x, wide, cell, sec_clock) {
                                Some(it) => it,
                                None => return,
                            }
                        } else {
                            (x, wide)
                        };
                        (px - f64::from(x)) / f64::from(wide)
                    }
                    _ => return,
                }
            } else {
                #[allow(clippy::cast_precision_loss)]
                let top = f64::from(self.first_of_region()) * line_px + top_line as f64 * line_px;
                #[allow(clippy::cast_precision_loss)]
                let tall = (line_px * lines as f64).max(1.0);
                (py - top) / tall
            };
            if self
                .view
                .drag_to(pane, axis, fraction, self.columns, self.rooms())
            {
                self.ask_paint();
                return;
            }
        }
        if right {
            self.open_menu();
            return;
        }
        if let Some(row) = left {
            self.click(&row);
            return;
        }
        // WHAT IS LEFT OF THE ACCUMULATOR STAYS: spending only the whole part
        // is what lets a continuous gesture add up until it becomes a line.
        #[allow(clippy::cast_possible_truncation)]
        let whole = self.detents.trunc() as i64;
        if whole != 0 {
            #[allow(clippy::cast_precision_loss)]
            {
                self.detents -= whole as f64;
            }
            let by = whole * crate::surface::LINES_PER_DETENT as i64;
            // THE OPENED LINE TAKES IT FIRST, when the pointer is on it.
            //
            // Line 0 and the rows under it belong to no section, so nothing
            // below here would ever be asked about them — `under_pointer` is
            // resolved out of `View::hit`, which only knows the region. A bar
            // with no wheel is half a control.
            if self.fold.open && wheel_at.is_some_and(|y| self.pointer_on_fold(y)) {
                let last = self
                    .fold_scroll
                    .total
                    .saturating_sub(self.fold_scroll.lines);
                let next = i64::try_from(self.fold.offset).unwrap_or(0) + by;
                self.fold.offset = usize::try_from(next.max(0)).unwrap_or(0).min(last);
                self.ask_paint();
                return;
            }
            // THE SECTION UNDER THE POINTER TAKES IT FIRST, and the target is
            // what says which — it is the thing that resolved the point. Asking
            // `section_at` again here was a second answer to a question already
            // answered, and in the strip the two disagreed: the gesture went to
            // the listing while the pointer was over the facts.
            //
            // Every section's offset is the view's; see `files::View::wheel`.
            let which = self
                .under_pointer
                .as_ref()
                .map_or(crate::files::Pane::Listing, |t| t.pane);
            // BOTH PANELS SCROLL INSIDE THE PANEL'S OWN HEIGHT. The preview
            // joined that sentence the day it began to show a file's letters;
            // without it, a preview in the bottom strip would page four lines
            // at a time inside a room the code believed was the whole listing.
            let room = match which {
                crate::files::Pane::Info | crate::files::Pane::Preview => self.panel_room(),
                // THE REGION, and the view takes the listing's chrome off —
                // the same rule `rooms` follows, and for the same reason: the
                // sidebar has no chrome and was losing two rows to it.
                _ => self.room(),
            };
            // EVERY SECTION ANSWERS, the listing included: there is no
            // fallback here any more, and that is the point. What stood here
            // was this door's own copy of the listing's offset, moved by hand
            // because `View::wheel` used to answer `false` for it.
            if self.view.wheel(which, by, self.columns, room) {
                self.ask_paint();
            }
            return;
        }
        // Only redraws if something CHANGED: a `Motion` arrives on every pixel
        // of movement, and painting per pixel would spend the GPU drawing the
        // same thing.
        if before != (self.under_pointer.clone(), self.cell_under_pointer) {
            self.ask_paint();
        }
    }
}

impl FilesWindow {
    /// Puts a sentence on the message line, and takes it off again.
    ///
    /// **THE ONE PLACE**, and there were nine. Nine assignments meant nine
    /// chances to forget whatever a message needs, and one of them was already
    /// forgotten by all nine: the message never went away. The owner found it
    /// with `copy the path` — *"no filemanager versão janela, esse texto não
    /// desapareceu"* — where the footer's `copied` fades after three seconds
    /// and this door's stayed for the rest of the session.
    ///
    /// # Why a one-shot timer, and not a clock
    ///
    /// Because this window has no tick, deliberately: with several of them
    /// open, a timer would be one wake-up per second per window for ever, and
    /// the directory is watched by inotify precisely so that nothing has to
    /// wake at rest (see the `Notice` above). A one-shot keeps that property
    /// exactly — a window with nothing to say wakes not once — and pays for the
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

    /// A left click, on whatever cell it landed on.
    fn click(&mut self, target: &crate::files::Target) {
        // A CLICK CLEARS THE LAST MESSAGE, like a key does. This window has no
        // timer, so the message stays until something else is being done — and
        // with it drawn ABOVE the pointer's description (so a refusal cannot be
        // covered), "something else" has to include the mouse. Otherwise a
        // single error would own the line until a key was pressed.
        self.message = None;
        // THE TARGET ALREADY SAYS WHAT WAS PRESSED. It came out of
        // `files::View::hit`, which resolved the section first and the field
        // among that section's own — so a press in the sidebar is the
        // sidebar's, one in the facts is the facts', and one on the listing's
        // chrome is the path's or the header's. Working it out again here was
        // the second place it could be got wrong, and it was.
        let step = self
            .view
            .click(target.row, &target.hit, crate::chrome::held(self.mods));
        self.apply(step);
    }

    /// WHERE A DRAG WOULD LAND, resolved through the pointer's own path.
    ///
    /// The same `line_and_cell` and the same `View::hit`: what lights under a
    /// drag and what a drop takes are one answer, which is the law this
    /// manager's controls are all held to.
    fn carried_to(&mut self, x: f64, y: f64, copies: bool) {
        let Some((line, cell)) = self.line_and_cell(x, y) else {
            return;
        };
        if self
            .view
            .carry_to(self.columns, self.room(), line, cell, copies, &self.texts)
        {
            self.ask_paint();
        }
    }

    /// Is there anywhere under the pointer for a drop to go?
    fn takes_a_drop(&self) -> bool {
        self.view.landing_now()
    }

    /// A drag of OURS ended, however it ended.
    fn let_the_drag_go(&mut self) {
        if let Some(c) = self.carrying.as_mut() {
            c.done();
        }
        if self.view.carried_away() {
            self.ask_paint();
        }
    }

    /// A right click: focus that row, then open the action menu.
    ///
    /// THROUGH THE VIEW'S ONE DECISION — see `files::View::menu_at`. What the
    /// pointer is over was already resolved on the press; handing the row over
    /// and letting the view resolve it a second time is what made the parameter
    /// mean two things at once.
    fn open_menu(&mut self) {
        self.message = None;
        let target = self.under_pointer.clone();
        let step = self.view.menu_at(target.as_ref());
        self.apply(step);
    }
}

impl CompositorHandler for FilesWindow {
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

impl OutputHandler for FilesWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for FilesWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(FilesWindow);
smithay_client_toolkit::delegate_dispatch2!(FilesWindow);

#[cfg(test)]
mod tests {
    use super::{FIRST_OF_REGION, Geometry, Scene, frame};
    use std::path::Path;

    /// A frame of THIS door at a surface, so no test writes `(12.0, 26.5)` by
    /// hand.
    ///
    /// Those literals are the same disease one register down: the door's
    /// geometry with a third home, in the test file. Eight assertions in
    /// `files.rs` carried them; here there is one place, and when a token
    /// moves the tests move with it.
    fn a_frame(width_px: f32, height_px: f32) -> crate::chrome::Frame {
        crate::chrome::Frame::of(width_px, height_px, FIRST_OF_REGION)
    }

    fn a_geometry(width_px: f32, height_px: f32, strip: u16, chrome: u16) -> Geometry {
        Geometry {
            metric: a_frame(width_px, height_px),
            columns: 60,
            strip,
            chrome,
        }
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-window-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test directory");
        dir
    }

    fn a_view(dir: &Path) -> crate::files::View {
        let mut prefs = ronoctua_files::sidebar::Preferences::fresh();
        prefs.showing = false;
        crate::files::View::at(
            dir,
            ronoctua_files::who::Roster::default(),
            ronoctua_files::when::Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(ronoctua_i18n::Texts::from_env()),
            ronoctua_files::sidebar::Sidebar::of(&[], None, None, prefs, None),
        )
    }

    /// EVERY HEIGHT, AND ESPECIALLY THE ONES THAT ARE NOT WHOLE LINES.
    ///
    /// The plan names this one as a law no grep can see: a surface is
    /// `floor(height / line)` lines plus a residue, and every geometry defect
    /// this door was asked to fix lived in that residue. A window is resized by
    /// dragging, so the residue is the common case and not the corner one.
    #[test]
    fn the_bottom_is_the_surface_and_never_a_count_of_lines() {
        let mut checked = 0;
        for h in (200..=1200).step_by(7) {
            #[allow(clippy::cast_precision_loss)]
            let height = h as f32;
            for strip in [0u16, 4] {
                let geo = a_geometry(900.0, height, strip, 2);
                let line = geo.metric.line();
                if geo.metric.lines() <= FIRST_OF_REGION + strip {
                    continue;
                }
                checked += 1;

                // THE REGION AND THE STRIP TOGETHER NEVER PASS THE SURFACE.
                let used =
                    f32::from(FIRST_OF_REGION + strip).mul_add(line, geo.room() as f32 * line);
                assert!(
                    used <= geo.metric.bottom() + 0.001,
                    "h={height}: the lines add up to {used} on a surface of {}",
                    geo.metric.bottom()
                );

                // AND WHAT IS LEFT UNDER THEM IS LESS THAN A LINE. A whole
                // line left over is a line nobody draws — the blank strip the
                // owner photographed under the facts.
                assert!(
                    geo.metric.bottom() - used < line,
                    "h={height}: {} px are left under the last line, and a line is {line}",
                    geo.metric.bottom() - used
                );

                // AND THE LISTING'S OWN BOTTOM IS THE STRIP'S RULE, or the
                // surface when there is no strip.
                let list = geo.list_bottom(line);
                if strip == 0 {
                    assert!(
                        (list - geo.metric.bottom()).abs() < 0.001,
                        "h={height}: with no strip the listing has to end at the surface"
                    );
                } else {
                    assert!(
                        list < geo.metric.bottom(),
                        "h={height}: the listing ran into the strip"
                    );
                }
            }
        }
        assert!(checked > 200, "only {checked} heights were exercised");
    }

    /// The four rooms are one arithmetic, and they agree with each other.
    #[test]
    fn the_rooms_come_apart_the_way_the_lines_do() {
        let geo = a_geometry(900.0, 800.0, 4, 2);
        assert_eq!(
            geo.list_room(),
            geo.room() - 2,
            "the chrome takes two lines"
        );
        assert_eq!(geo.panel_room(), 4, "with a strip the panels are the strip");
        // AND `rooms().list` IS THE REGION, not the listing's own rows.
        //
        // They used to be the same number and that was the defect: the sidebar
        // shares this room and has no chrome, so it was losing two rows to the
        // listing's header — while the listing's own subtraction now happens
        // in the view, which is the only place that knows whether the chrome
        // is there at all. See `View::chrome_lines`.
        assert_eq!(geo.rooms().list, geo.room());
        assert_eq!(
            geo.rooms().list - geo.list_room(),
            2,
            "the two differ by exactly the chrome"
        );
        assert_eq!(geo.rooms().panel, geo.panel_room());

        // WITHOUT A STRIP the panels are columns, and they get the region.
        let beside = a_geometry(900.0, 800.0, 0, 2);
        assert_eq!(beside.panel_room(), beside.room());
    }

    /// WHERE A BAR IS DRAWN IS WHERE IT IS GRABBED, which is the law the plan
    /// names and the defect the owner reported in both doors.
    #[test]
    fn a_bar_is_inside_its_own_column_and_its_own_band() {
        let dir = scratch("bars");
        for n in 0..80 {
            std::fs::write(dir.join(format!("f{n:02}.txt")), b"x").unwrap();
        }
        let texts = ronoctua_i18n::Texts::from_env();
        let mut view = a_view(&dir);
        // THE PANELS ARE ON BY DEFAULT — `View::at` says so, and this test
        // is about the strip they make when they do not fit as columns.
        view.reread();

        // BOTH ARRANGEMENTS, and the test says which is which.
        //
        // The first draft used one width and it happened to be a width where
        // the panels are COLUMNS — so the strip, which is where the listing's
        // band stops early, was never exercised at all. With `strip == 0`
        // every band ends at the surface and the assertion below cannot tell a
        // right answer from a wrong one.
        let mut with_a_strip = 0;
        for columns in [53u16, 120] {
            // A HEIGHT THAT IS NOT A WHOLE NUMBER OF LINES, on purpose.
            let geo = {
                let panes = view.panes(columns);
                if panes.strip > 0 {
                    with_a_strip += 1;
                }
                Geometry {
                    metric: a_frame(900.0, 777.0),
                    columns,
                    strip: panes.strip,
                    chrome: 2,
                }
            };
            let (cell, line_px) = (geo.metric.cell(), geo.metric.line());
            // AT THE END OF EVERY LIST, and not only at the top.
            //
            // At offset zero a thumb is at the start of its track, and a track
            // that runs too far is indistinguishable from one that does not: the
            // defect only shows at the LAST offset. The first draft measured at
            // zero and stayed green with the regression planted back in.
            for pane in [crate::files::Pane::Listing, crate::files::Pane::Info] {
                let _ = view.drag_to(
                    pane,
                    crate::files::Axis::Down,
                    1.0,
                    geo.columns,
                    geo.rooms(),
                );
            }
            let chrome = view.chrome_rows(geo.columns, &texts);
            let sections = view.sections(&chrome, geo.room(), geo.columns, &texts);
            let panes = view.panes(geo.columns);
            let bars = geo.bars(panes, &sections, cell, line_px);
            assert!(!bars.is_empty(), "eighty files and not one bar");

            for (pane, _axis, (bx, by, bw, bh)) in bars {
                let sec = sections
                    .iter()
                    .find(|s| s.pane == pane)
                    .expect("the bar belongs to a section");
                let (x, wide) = crate::files::section_px(
                    panes,
                    pane,
                    super::CONTENT_MARGIN,
                    geo.columns,
                    cell,
                    geo.metric.width,
                )
                .expect("the section has a box");
                assert!(
                    bx >= x - 0.5 && bx + bw <= x + wide + 0.5,
                    "{pane:?}: the bar ({bx}..{}) left its column ({x}..{})",
                    bx + bw,
                    x + wide
                );
                assert!(
                    bx + bw <= geo.metric.width + 0.5,
                    "{pane:?}: the bar went past the surface's edge"
                );

                // AND INSIDE ITS BAND, top and bottom — measured against the
                // DOOR's boundary and not against `bar_box`'s own arithmetic
                // written out again.
                //
                // It was written out again in the first draft, and that made the
                // assertion tautological: planting the defect back into `bar_box`
                // left this test green, because it was asking the function whether
                // it agreed with itself. The listing and the sidebar end where the
                // strip begins; the panels down in the strip end at the surface.
                let band_top = geo.metric.y_of(FIRST_OF_REGION)
                    + f32::from(u16::try_from(sec.top).unwrap_or(0)) * line_px;
                let floor = match pane {
                    crate::files::Pane::Listing | crate::files::Pane::Sidebar => {
                        geo.list_bottom(line_px)
                    }
                    _ => geo.metric.bottom(),
                };
                assert!(
                    by >= band_top - 0.5,
                    "{pane:?}: the bar rose out of its band"
                );
                assert!(
                    by + bh <= floor + 0.5,
                    "{pane:?}: the bar ends at {} and its band ends at {floor}",
                    by + bh
                );
            }
        }
        assert_eq!(
            with_a_strip, 1,
            "one of the two widths has to put the panels below"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THE PURE HALF DRAWS NOTHING OFF THE SURFACE.
    ///
    /// This is the assertion the door could not make while `paint()` was one
    /// function: it needed a GPU to reach the arithmetic. `frame()` needs
    /// nothing but a view and a frame, so what it produces can be measured.
    #[test]
    fn a_frame_draws_nothing_past_the_surface() {
        let dir = scratch("frame");
        for n in 0..40 {
            std::fs::write(dir.join(format!("f{n:02}.txt")), b"x").unwrap();
        }
        let texts = ronoctua_i18n::Texts::from_env();
        let mut view = a_view(&dir);
        view.reread();

        // TWO SURFACES, and neither a whole number of lines.
        for (w, h) in [(900.0f32, 777.0f32), (640.0, 483.0)] {
            let panes = view.panes(60);
            let geo = Geometry {
                metric: a_frame(w, h),
                columns: 60,
                strip: panes.strip,
                chrome: 2,
            };
            let painted = frame(&mut Scene {
                geo,
                view: &mut view,
                texts: &texts,
                message: None,
                cell_under_pointer: None,
                line_under_pointer: None,
                under_pointer: None,
                fold: crate::fold::Fold::default(),
                fold_control: None,
                fold_wrapped: 0,
                hinted: &mut crate::chrome::Hinted::default(),
                fold_scroll: ronoctua_design::Scroll {
                    total: 0,
                    lines: 0,
                    offset: 0,
                    top: 0,
                },
            });
            assert!(!painted.spans.is_empty(), "{w}x{h}: nothing was drawn");
            for q in &painted.quads {
                assert!(
                    q.y + q.height <= geo.metric.bottom() + 0.5,
                    "{w}x{h}: a quad ends at {} and the surface at {}",
                    q.y + q.height,
                    geo.metric.bottom()
                );
                assert!(
                    q.x + q.width <= geo.metric.width + 0.5,
                    "{w}x{h}: a quad ends at {} and the surface is {} wide",
                    q.x + q.width,
                    geo.metric.width
                );
                assert!(
                    q.y >= -0.5 && q.x >= -0.5,
                    "{w}x{h}: a quad started off the surface"
                );
            }
            for span in &painted.spans {
                assert!(
                    span.line < geo.metric.lines(),
                    "{w}x{h}: a span is on line {} and the surface holds {}",
                    span.line,
                    geo.metric.lines()
                );
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
