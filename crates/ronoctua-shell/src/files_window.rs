//! The file manager's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a twelve-line ceiling (`space.expansion`,
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
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
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

/// The listing's own chrome inside the region: the path, then the header.
///
/// They are the LISTING's two lines and not the region's — every other column
/// starts at [`FIRST_OF_REGION`], which is what stopped them from having to
/// leave two blank lines under somebody else's path.
const CHROME_LINES: u16 = 2;

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
    window.set_title("*arquivos*");
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

    let texts = std::rc::Rc::new(ronoctua_i18n::Texts::from_env());
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
        sender,
        under_pointer: None,
        dragging: None,
        detents: 0.0,
        cell_under_pointer: None,
        owed: false,
        message: None,
        repeat: handle.clone(),
        spinning: None,
        notice,
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
    window: Window,
    width: u32,
    height: u32,
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
    /// What is left over of a touchpad gesture, in lines.
    ///
    /// A wheel sends whole detents; a touchpad sends fractions of one, and this
    /// door spent them: it asked for a whole line's worth in a single event and
    /// threw away everything below that. The footer has had the accumulator
    /// since it was written — the owner reported the consequence as the gesture
    /// working in one door and in neither section of the other.
    detents: f64,
    cell_under_pointer: Option<u16>,
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
    /// The loop, so a keyboard can be made that REPEATS.
    ///
    /// SCTK only generates repeat events for a keyboard created with
    /// `get_keyboard_with_repeat`, and that one wants a `LoopHandle` — which is
    /// why this is carried rather than reached for. See `new_capability`.
    repeat: calloop::LoopHandle<'static, Self>,
    /// The thread that draws a shape at the angle it is being looked at.
    turning: Option<crate::stage::Stage>,
    /// Where the pointer was when it last moved a shape.
    ///
    /// A gesture of the door's own and NOT a fourth `Axis`: `drag_to` inverts
    /// a track — one scalar, one direction — and turning something is two
    /// deltas with no track behind them. It sits beside `dragging` because it
    /// is the same kind of thing: a press that owns the pointer until it is
    /// let go.
    spinning: Option<(f64, f64)>,
    /// The directory reporting itself, when one could be had.
    ///
    /// `None` means the watch could not be armed and the fallback clock above
    /// is doing the work instead.
    notice: Option<ronoctua_files::notice::Notice>,
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
        if self.view.plain() { CHROME_LINES } else { 0 }
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
        self.follow_the_motion();
        self.follow_the_turn();
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
        let Some(stage) = self.turning.as_mut() else {
            return;
        };
        if let Some(ask) = self.view.wants_turn() {
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
                self.message = Some(self.texts.t_with(e.key, &e.borrowed()));
            }
            Step::Do(doing) => {
                // THE FOUR THAT SPAWN are the door's business; everything else
                // is the same code the footer runs.
                match doing {
                    Doing::Open(path) => {
                        self.open_the_file(&path);
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
                            self.message = Some(self.texts.t_with(
                                ronoctua_i18n::key::FILES_SIDEBAR_GONE,
                                &[("name", which.as_str())],
                            ));
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
                            self.message = Some(self.texts.t(ronoctua_i18n::key::ECHO_NO_BUFFER));
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
                        self.message = Some(self.texts.t(if ok {
                            ronoctua_i18n::key::FILES_COPIED
                        } else {
                            ronoctua_i18n::key::FILES_NOT_COPIED
                        }));
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
                                self.message = Some(self.texts.t_with(e.key, &e.borrowed()));
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
            self.message = Some(self.texts.t_with(
                ronoctua_i18n::key::FILES_NOTHING_OPENS,
                &[("type", of_type)],
            ));
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
            self.message = Some(self.texts.t(ronoctua_i18n::key::FILES_NO_PKEXEC));
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
            crate::files::Report::Failed(why) => self.message = Some(why),
            crate::files::Report::Refused(which, why) => {
                if matches!(which, ronoctua_disks::Refusal::AlreadyMounted) {
                    self.view.reread();
                } else {
                    let e = self.view.refusal_said(which, &why);
                    self.message = Some(self.texts.t_with(e.key, &e.borrowed()));
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
        // A key clears the last message: it answered something that is no
        // longer what is being done.
        self.message = None;
        let step = self.view.key(t);
        self.apply(step);
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
            crate::chrome::Door::Window,
            self.width as f32,
            self.height as f32,
            FIRST_OF_REGION,
        )
    }

    fn recount(&mut self) {
        // THROUGH THE FRAME, and not through a fourth spelling of the cell.
        // This function used to say `SIZE_LARGE * CELL_WIDTH_EM` — one of the
        // three spellings of one number that the survey found living in this
        // tree, beside `metrics::cell_width` and `Door::cell_width`.
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
        let k = line.checked_sub(FIRST_OF_REGION)?;
        Some((usize::from(k), cell))
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
        // SIZE_LARGE and not SIZE_BASE, for the reason the buffer host gives:
        // the footer is peripheral reference and this is what one reads.
        match ronoctua_render::Text::new(
            &g.device,
            &g.queue,
            g.config.format,
            tokens::font::SIZE_LARGE,
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
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let painted = frame(&mut self.scene());
        self.present(painted);
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
            under_pointer: self.under_pointer.as_ref(),
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
            picture,
        } = painted;
        #[allow(clippy::cast_possible_truncation)]
        let under = (quads.len() - over) as u32;
        let (Some(gpu), Some(text)) = (self.gpu.as_mut(), self.text.as_mut()) else {
            return;
        };
        if let Some(im) = self.images.as_mut() {
            im.clear();
            let batch = match picture {
                Some((key, area)) => {
                    // A MOVING FILE REPLACES THE TEXTURE UNDER THE SAME KEY,
                    // every frame — which is what keeps an animation to ONE of
                    // the thirty-nine slots. A key per frame would fill the
                    // cache with one file's film and evict every icon.
                    if let Some(frame) = self.view.live_frame() {
                        im.upload_it(&gpu.device, &gpu.queue, &key, frame);
                    } else if im.missing(&key)
                        // THE STILL ONE ONLY GOES UP WHEN IT IS MISSING: a
                        // preview is up to a mebibyte, and `prepare` is
                        // sixteen bytes.
                        && let Some(rgba) = self.view.preview_pixels()
                    {
                        im.upload_it(&gpu.device, &gpu.queue, &key, rgba);
                    }
                    vec![(key, area)]
                }
                None => Vec::new(),
            };
            im.prepare(&gpu.queue, gpu.config.width, gpu.config.height, &batch);
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
            q.prepare(
                &gpu.device,
                &gpu.queue,
                gpu.config.width,
                gpu.config.height,
                &quads,
            );
        }
        let quads_ref = self.quads.as_ref();
        // QUADS, THEN IMAGES, THEN THE FEW QUADS THAT GO ON TOP, THEN TEXT.
        //
        // There is no depth test: whoever draws later ends up on top. The
        // picture sits on the background and UNDER the text, which has to stay
        // legible over a light photograph — and under the film's transport,
        // which is drawn ON the film by design.
        let images_ref = self.images.as_ref();
        if let Err(e) = gpu.frame(|pass| {
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
        }) {
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
    /// The preview's texture and where it goes, when there is one.
    pub picture: Option<(String, ronoctua_render::image::Area)>,
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
                .saturating_sub(FIRST_OF_REGION + self.strip),
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
            list: self.list_room(),
            panel: self.panel_room(),
        }
    }

    /// Where the LISTING's region ends: the strip's rule when there is a strip
    /// under it, and the surface's own bottom when there is not.
    pub fn list_bottom(&self, line_px: f32) -> f32 {
        if self.strip > 0 {
            f32::from(FIRST_OF_REGION + u16::try_from(self.room()).unwrap_or(0)) * line_px
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
        let top = self.metric.y_of(FIRST_OF_REGION);
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
    /// What the pointer is resting on, already resolved by `View::hit`.
    pub under_pointer: Option<&'a crate::files::Target>,
}

/// One frame, as spans and rectangles. No Wayland, no wgpu, no `self`.
pub(crate) fn frame(s: &mut Scene<'_>) -> Painted {
    let size = tokens::font::SIZE_LARGE;
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
        .map(|e| s.texts.t_with(e.key, &e.borrowed()));
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
        .map(|e| s.texts.t_with(e.key, &e.borrowed()));
    let second = s
        .message
        .clone()
        .or(does)
        .or(explained)
        // WITH NOTHING TYPED AND NOTHING TO SAY, the hint. Typing is not a
        // key and so has no line in the manual; without this the one
        // gesture the list is built around is the one thing nothing on
        // screen mentions.
        .or_else(|| {
            (!s.view.filtering())
                .then(|| crate::files::hint_of(s.texts, s.view.split(s.geo.columns) > 0))
        })
        .unwrap_or_default();
    if !second.is_empty() {
        spans.push(ronoctua_render::Span {
            size: ronoctua_render::Size::Px(size),
            text: ronoctua_render::layout::elide_end(&second, s.geo.columns),
            column: margin,
            line: 0,
            color: tokens::color::TEXT_SECONDARY,
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
    // cover the listing, so there are no columns to lay out. See
    // `files::View::modal_rows`.
    let sections = if s.view.plain() {
        s.view
            .sections(&chrome, s.geo.room(), s.geo.columns, s.texts)
    } else {
        vec![crate::files::Section {
            // A fixture's one section, and nothing in it reads sideways.
            across: None,
            pane: crate::files::Pane::Listing,
            from: 0,
            to: s.geo.columns,
            top: 0,
            lines: s.geo.room(),
            offset: s.view.list_offset(),
            total: s.geo.room(),
            above: 0,
            time: None,
            transport: None,
            clock: 0,
            content: s
                .view
                .modal_rows(s.view.list_offset(), s.geo.room(), s.geo.columns, s.texts),
        }]
    };
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
            s.geo.metric.y_of(FIRST_OF_REGION),
            s.geo.list_bottom(line_px),
        ));
    }
    // THE BAND, once per line that the pointer is on — and it is the
    // POINTER's section that decides which line, so a band never lights a
    // row of a list the pointer is not over.
    if let Some(t) = s.under_pointer.as_ref()
        && s.view.band_belongs_at(
            s.cell_under_pointer.unwrap_or(0),
            s.geo.columns,
            t.pane == crate::files::Pane::Preview || t.pane == crate::files::Pane::Info,
        )
        && let Some(sec) = sections.iter().find(|s| s.pane == t.pane)
        && let Some(line) = sec.line_of(t.row)
    {
        #[allow(clippy::cast_possible_truncation)]
        let y = FIRST_OF_REGION + line as u16;
        let cell = tokens::metrics::cell_width(size);
        // THE SECTION'S OWN EDGES, in pixels.
        //
        // `band_of` answers for the sidebar and the listing, which is what
        // it was written for; down in the strip the panels are neither, and
        // it fell through to the listing's bounds. The section already says
        // where it begins and ends, so it is asked — and a band lights the
        // column the pointer is in, never the one beside it.
        #[allow(clippy::cast_precision_loss)]
        let (at, wide) = match t.pane {
            crate::files::Pane::Preview | crate::files::Pane::Info if sec.top > 0 => {
                crate::files::panel_px(
                    panes_now,
                    if matches!(t.pane, crate::files::Pane::Preview) {
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
        quads.push(ronoctua_render::quad::Quad {
            x: at,
            y: f32::from(y) * line_px,
            width: wide,
            height: line_px,
            color: tokens::color::SURFACE_SELECTION,
        });
    }
    for sec in &sections {
        for (i, line) in sec.content.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let y = FIRST_OF_REGION + (sec.top + i) as u16;
            let lit = s
                .under_pointer
                .as_ref()
                .is_some_and(|t| t.pane == sec.pane && sec.line_of(t.row) == Some(sec.top + i));
            for f in &line.fields {
                // THE FIELD UNDER THE POINTER LIGHTS UP, so the sentence on
                // the line above has a visible subject. A colour and not a
                // box — invariant 15.
                let under = lit && s.cell_under_pointer.is_some_and(|c| f.covers(c));
                spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::Px(size),
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
    let strip_top = FIRST_OF_REGION + u16::try_from(s.geo.room()).unwrap_or(0);
    if panes.right > 0 || panes.strip > 0 {
        // WHERE THEY START AND WHERE THEY STOP, and the stop is the
        // SURFACE'S — never a count of lines times a fractional line
        // height, which is what left every one of them short of the edge.
        let (rule_from, rule_to) = if panes.right > 0 {
            (
                s.geo.metric.y_of(FIRST_OF_REGION),
                s.geo.list_bottom(line_px),
            )
        } else {
            (s.geo.metric.y_of(strip_top), s.geo.metric.bottom())
        };
        for at in panes.rules(s.geo.columns) {
            quads.push(crate::chrome::divider(
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
        let (top, _) =
            crate::files::panel_band(FIRST_OF_REGION, strip_top - FIRST_OF_REGION, panes.strip);
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
        quads.len() - before
    };
    Painted {
        spans,
        quads,
        over,
        picture,
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
        self.width = cfg.new_size.0.map_or(900, std::num::NonZeroU32::get);
        self.height = cfg.new_size.1.map_or(600, std::num::NonZeroU32::get);
        if let Some(g) = self.gpu.as_mut() {
            g.resize(self.width, self.height);
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
                self.repeat.clone(),
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
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let before = (self.under_pointer.clone(), self.cell_under_pointer);
        let mut left = None;
        let mut right = None;
        // BOTH COORDINATES: a bar going across is dragged by `x`.
        let mut dragged: Option<(f64, f64)> = None;
        let mut spun = false;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if self.dragging.is_some() {
                        dragged = Some(e.position);
                    }
                    // AND A SHAPE TURNS WITH THE HAND. The deltas are in
                    // PIXELS and become radians here, because the view has no
                    // idea how big a pixel is; a whole panel's width is about
                    // half a turn, which is the ratio that makes a piece feel
                    // attached to the pointer rather than geared to it.
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
                    self.under_pointer = self.target_at(e.position.0, e.position.1);
                    self.cell_under_pointer = self
                        .line_and_cell(e.position.0, e.position.1)
                        .map(|(_, c)| c);
                    // AND WHICH SECTION, which is what decides the film's two
                    // buttons are drawn: they appear under the pointer, and
                    // their CELLS are reserved either way so nothing moves.
                    self.view
                        .pointing(self.under_pointer.as_ref().map(|t| t.pane));
                }
                PointerEventKind::Leave { .. } => {
                    self.under_pointer = None;
                    self.cell_under_pointer = None;
                    self.view.pointing(None);
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    self.under_pointer = self.target_at(e.position.0, e.position.1);
                    self.cell_under_pointer = self
                        .line_and_cell(e.position.0, e.position.1)
                        .map(|(_, c)| c);
                    // AND WHICH SECTION, which is what decides the film's two
                    // buttons are drawn: they appear under the pointer, and
                    // their CELLS are reserved either way so nothing moves.
                    self.view
                        .pointing(self.under_pointer.as_ref().map(|t| t.pane));
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
                    self.spinning = None;
                }
                // THE RIGHT BUTTON OPENS THE MENU. It draws in this window's own
                // region, has no surface of its own and does not float — see the
                // note in `surface.rs`, where the same decision is written out.
                PointerEventKind::Press { button, .. } if button == RIGHT_BUTTON => {
                    self.under_pointer = self.target_at(e.position.0, e.position.1);
                    self.cell_under_pointer = self
                        .line_and_cell(e.position.0, e.position.1)
                        .map(|(_, c)| c);
                    // AND WHICH SECTION, which is what decides the film's two
                    // buttons are drawn: they appear under the pointer, and
                    // their CELLS are reserved either way so nothing moves.
                    self.view
                        .pointing(self.under_pointer.as_ref().map(|t| t.pane));
                    right = self.under_pointer.as_ref().map(|t| t.row);
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
                    let line_px = f64::from(tokens::metrics::line_height(tokens::font::SIZE_LARGE));
                    self.detents += crate::surface::axis_detents(&vertical, line_px)
                        + crate::surface::axis_detents(&horizontal, line_px);
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
            let line_px = f64::from(tokens::metrics::line_height(tokens::font::SIZE_LARGE));
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
                let top = f64::from(FIRST_OF_REGION) * line_px + top_line as f64 * line_px;
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
        if let Some(row) = right {
            self.open_menu(row);
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
                _ => self.list_room(),
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
        let step = self.view.click(target.row, &target.hit);
        self.apply(step);
    }

    /// A right click: focus that row, then open the action menu.
    fn open_menu(&mut self, row: usize) {
        self.message = None;
        // The header has no menu: right-clicking a column name is not a
        // gesture about any file.
        if row == usize::MAX {
            return;
        }
        // ROW AND CELL, through the view's one decision — see
        // `files::View::menu_at`. This door had the same defect as the footer:
        // it kept the row and threw the cell away.
        let cell = self.cell_under_pointer.unwrap_or(0);
        let step = self.view.menu_at(row, cell, self.columns, self.room());
        self.apply(step);
    }
}

impl CompositorHandler for FilesWindow {
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
        crate::chrome::Frame::of(
            crate::chrome::Door::Window,
            width_px,
            height_px,
            FIRST_OF_REGION,
        )
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
        assert_eq!(geo.rooms().list, geo.list_room());
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
                under_pointer: None,
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
