//! The player in a window (`LEADER p w`) and in the column on the right
//! (`LEADER p c`) — two doors onto `crate::player`, one host.
//!
//! What a key does and which rows exist is `player.rs`; this is the plumbing
//! of a surface — `camera_settings_window.rs` for the rows, `camera_window.rs`
//! for the pictures — plus the two things only this door has: a transport
//! line of glyphs, and a time bar a hand can drag.
//!
//! # Two surfaces, one host
//!
//! The window is an `xdg_toplevel`; the column is a layer-shell surface
//! anchored to the top, the right and the bottom, with an exclusive zone as
//! wide as itself — the windows move over and nothing is covered. Everything
//! else is the same, down to the last pixel: a column is a window the
//! compositor places for us.
//!
//! # The column never narrows the footer
//!
//! A layer surface's exclusive zone is carved out of what the surfaces laid
//! out BEFORE it left, in the order they were created. The footer is created
//! first, so the column ends where the footer's area begins — the modeline,
//! the minibuffer and the notification band keep the whole width. A footer
//! that starts again after the column would be laid out after it; that is why
//! the footer, on its start, raises a column that is already up again
//! (`Footer::column_again`).
//!
//! # Only one column
//!
//! Its pid is kept in the runtime directory (`column.pid`). A column that
//! starts ends the one already there, which is how "another column" will take
//! the right edge when there is more than one tenant.
//!
//! # The frames are the clock
//!
//! Each picture has a thread of its own reading the player's ring, and every
//! frame that arrives is what asks for the next paint — no timer. The newest
//! frame wins: one slot, so a door that was behind never plays a backlog.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Context as _, Result};
use ronoctua_player::wire::{Ask, Repeat, State};
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
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
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
pub const APP_ID: &str = "br.ronoctua.player";

/// The column's layer-shell namespace, so a compositor rule can tell it from
/// the footer.
pub const NAMESPACE: &str = "ronoctua-column";

/// The runtime file the column keeps its pid in.
pub const COLUMN_PID: &str = "column.pid";

/// `BTN_LEFT`.
const LEFT_BUTTON: u32 = 0x110;

/// The margin inside the surface, in cells — see `chrome::FIELDS_LEAD`.
const MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// Which surface this host is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Door {
    /// A floating window.
    Window,
    /// The column on the right.
    Column,
}

/// The newest frame of one picture, and nothing queued.
type Slot = Arc<Mutex<Option<(u32, u32, Vec<u8>)>>>;

/// The transport's buttons, in the order they are drawn: the command each one
/// runs, which is also what the header says under the pointer.
const BUTTONS: [&str; 8] = [
    "player-previous",
    "player-toggle",
    "player-next",
    "player-stop",
    "player-shuffle",
    "player-repeat",
    "player-only",
    "player-maximize",
];

/// How the picture is shown in this door.
///
/// # The picture alone, and the controls over it
///
/// Off, the door is the view: the picture on top, the transport, the list.
/// On stage — `Only`, or `Maximized` (the window filling the screen) — the
/// picture takes the whole surface, letterboxed in black by the player, and
/// everything else is hidden. The transport and the header come back OVER the
/// picture, on the `surface.transport` band, on any activity: the pointer
/// moving, a click, the wheel, a key. They go again [`CONTROLS_LINGER`] after
/// the last one, and never while the hand rests on them — a control that
/// vanished under the pointer about to press it is one nobody can press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The door as it is: the picture, the transport, the list.
    Off,
    /// Only the picture, in this door.
    Only,
    /// Only the picture, on the whole screen.
    Maximized,
}

/// How long the controls stay over the picture after the last activity.
const CONTROLS_LINGER: std::time::Duration = std::time::Duration::from_secs(3);

/// Each button's glyph, from the state and how the picture is shown.
fn glyph_of(id: &str, s: Option<&State>, stage: Stage) -> char {
    let playing = s.is_some_and(|s| s.playing);
    match id {
        "player-only" => tokens::icon::PLAYER_ONLY,
        // WHERE IT GOES, as the file manager's pair: out when on the screen.
        "player-maximize" if stage == Stage::Maximized => tokens::icon::THEATRE_OFF,
        "player-maximize" => tokens::icon::THEATRE,
        "player-previous" => tokens::icon::PLAYER_PREVIOUS,
        "player-toggle" if playing => tokens::icon::PAUSE,
        "player-toggle" => tokens::icon::PLAY,
        "player-next" => tokens::icon::PLAYER_NEXT,
        "player-stop" => tokens::icon::PLAYER_STOP,
        "player-shuffle" if s.is_some_and(|s| s.shuffle) => tokens::icon::PLAYER_SHUFFLE,
        "player-shuffle" => tokens::icon::PLAYER_SHUFFLE_OFF,
        "player-repeat" => match s.map_or(Repeat::Nothing, |s| s.repeat) {
            Repeat::Nothing => tokens::icon::PLAYER_REPEAT_OFF,
            Repeat::Queue => tokens::icon::PLAYER_REPEAT,
            Repeat::One => tokens::icon::PLAYER_REPEAT_ONE,
        },
        _ => ' ',
    }
}

/// Whether a button's glyph is drawn lit: a switch that is on.
fn lit(id: &str, s: Option<&State>, stage: Stage) -> bool {
    match id {
        "player-only" => stage == Stage::Only,
        "player-shuffle" => s.is_some_and(|s| s.shuffle),
        "player-repeat" => s.is_some_and(|s| s.repeat != Repeat::Nothing),
        _ => false,
    }
}

/// What a button asks the player.
fn ask_of(id: &str) -> Option<Ask> {
    Some(match id {
        "player-previous" => Ask::Previous,
        "player-toggle" => Ask::Toggle,
        "player-next" => Ask::Next,
        "player-stop" => Ask::Stop,
        "player-shuffle" => Ask::Shuffle,
        "player-repeat" => Ask::Repeat,
        _ => return None,
    })
}

/// Where one picture is: its lines, and which share of the width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    /// Its first line.
    pub first: u16,
    /// How many lines it takes.
    pub lines: u16,
    /// Which share of the width, counted from the left…
    pub share: u16,
    /// …of how many.
    pub shares: u16,
}

impl Place {
    /// Its rectangle, in logical pixels, on a surface `width` wide.
    #[must_use]
    pub fn area(self, width: f32, line: f32) -> ronoctua_render::image::Area {
        let w = width / f32::from(self.shares.max(1));
        ronoctua_render::image::Area {
            x: w * f32::from(self.share),
            y: line * f32::from(self.first),
            width: w,
            height: line * f32::from(self.lines),
        }
    }
}

/// Where everything is, in lines of the grid — ONE answer the painter and
/// the pointer both read, so a control is taken where it was drawn.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Layout {
    /// Each picture: the main one first, then every follower.
    pub pictures: Vec<Place>,
    /// The transport's buttons and the clock.
    pub transport: u16,
    /// The time bar.
    pub bar: u16,
    /// The list's first line.
    pub list: u16,
}

/// The largest `video`-shaped rectangle inside `width × height`, or the whole
/// of it when the shape is not known.
#[must_use]
pub fn fitted(width: f32, height: f32, video: Option<(u32, u32)>) -> (f32, f32) {
    let Some((vw, vh)) = video.filter(|(w, h)| *w > 0 && *h > 0) else {
        return (width, height);
    };
    #[allow(clippy::cast_precision_loss)]
    let ratio = vw as f32 / vh as f32;
    if width / height.max(1.0) > ratio {
        (height * ratio, height)
    } else {
        (width, width / ratio)
    }
}

/// The door on stage: one picture over the whole surface, the transport and
/// the time bar on its last two lines, and no list.
#[must_use]
pub fn stage_layout(lines: u16) -> Layout {
    Layout {
        pictures: vec![Place {
            first: 0,
            lines: lines.max(1),
            share: 0,
            shares: 1,
        }],
        transport: lines.saturating_sub(2),
        bar: lines.saturating_sub(1),
        list: lines,
    }
}

/// The smallest a follower's strip gets, in lines.
const FOLLOWER_FLOOR: u16 = 3;

/// Lays the door out. `first` is the line under the header, `lines` how many
/// the surface has, `columns` how wide it is in cells, `line_over_cell` the
/// font's line height over its cell width, and `video` the main picture's
/// size, when there is one.
#[must_use]
pub fn layout(
    door: Door,
    first: u16,
    lines: u16,
    columns: u16,
    line_over_cell: f32,
    video: Option<(u32, u32)>,
    followers: usize,
) -> Layout {
    let mut pictures = Vec::new();
    let mut at = first;
    if let Some((vw, vh)) = video {
        // THE MAIN PICTURE takes the height its shape wants at this width —
        // in lines, from the width in cells and the font's own proportion —
        // and never more than half the surface: the rows are the controls.
        #[allow(clippy::cast_precision_loss)]
        let wanted = f32::from(columns) * (vh.max(1) as f32 / vw.max(1) as f32) / line_over_cell;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let wanted = wanted.ceil().max(1.0) as u16;
        let ceiling = match door {
            Door::Window => lines / 2,
            Door::Column => tokens::space::COLUMN_PICTURES.min(lines / 2),
        };
        // THE FOLLOWERS SHARE ONE STRIP under it, side by side, and the
        // strip comes out of the same ceiling: a video added in step never
        // pushes the rows off. A third of it, and never less than
        // `FOLLOWER_FLOOR` lines — a smaller picture says nothing.
        let (main, strip) = if followers == 0 {
            (wanted.min(ceiling).max(1), 0)
        } else {
            let strip = (ceiling / 3)
                .max(FOLLOWER_FLOOR)
                .min(ceiling.saturating_sub(1));
            (wanted.min(ceiling.saturating_sub(strip)).max(1), strip)
        };
        pictures.push(Place {
            first: at,
            lines: main,
            share: 0,
            shares: 1,
        });
        at += main;
        if strip > 0 {
            let n = u16::try_from(followers).unwrap_or(u16::MAX);
            for i in 0..n {
                pictures.push(Place {
                    first: at,
                    lines: strip,
                    share: i,
                    shares: n,
                });
            }
            at += strip;
        }
    }
    Layout {
        pictures,
        transport: at,
        bar: at + 1,
        list: at + 2,
    }
}

/// Opens the player in `door`.
///
/// # Errors
///
/// No Wayland compositor, or none of the shell the door needs.
pub fn host_it(door: Door, stage: Stage) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<PlayerDoor> = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let texts = std::rc::Rc::new(crate::language::texts());

    let mut column_shell: Option<LayerShell> = None;
    let shell = match door {
        Door::Window => {
            let xdg = XdgShell::bind(&globals, &qh)
                .context("the compositor does not expose xdg_shell")?;
            let window = xdg.create_window(
                compositor.create_surface(&qh),
                WindowDecorations::RequestServer,
                &qh,
            );
            window.set_title(texts.t(ronoctua_i18n::key::PLAYER_TITLE));
            window.set_app_id(APP_ID);
            window.commit();
            Shell::Window(window)
        }
        Door::Column => {
            let layer_shell = LayerShell::bind(&globals, &qh)
                .context("the compositor does not expose wlr-layer-shell")?;
            // ONE COLUMN: the one already there goes.
            end_the_column();
            keep_the_pid();
            let layer = column_on(&layer_shell, &compositor, &qh, None);
            column_shell = Some(layer_shell);
            Shell::Layer(layer)
        }
    };

    let mut cycle: EventLoop<PlayerDoor> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();

    // THE THEME, THE LANGUAGE AND THE SIZE OF THE TEXT, watched like every
    // door watches them.
    let (theme_sender, theme_changed) = channel::channel::<()>();
    let _theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    handle
        .insert_source(theme_changed, |event, (), d: &mut PlayerDoor| {
            if let channel::Event::Msg(()) = event
                && crate::theme::reload()
            {
                d.owed = true;
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the theme channel: {e}"))?;
    let (language_sender, language_changed) = channel::channel::<()>();
    let _language_watch = crate::language::watch(move || language_sender.send(()).is_ok());
    handle
        .insert_source(language_changed, |event, (), d: &mut PlayerDoor| {
            if let channel::Event::Msg(()) = event {
                d.texts = std::rc::Rc::new(crate::language::texts());
                d.owed = true;
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the language channel: {e}"))?;
    let (size_sender, size_changed) = channel::channel::<()>();
    let _size_watch = crate::toolkits::watch(move || size_sender.send(()).is_ok());
    handle
        .insert_source(size_changed, |event, (), d: &mut PlayerDoor| {
            if let channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                d.text_size_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    // THE PLAYER'S STATE, on its own thread. A door that opens starts the
    // daemon: the nested loop has no unit, and a door onto nothing is a door
    // that says "not running" for ever.
    let _ = ronoctua_player::client::ensure();
    let (state_sender, states) = channel::channel::<Option<State>>();
    let _watch = ronoctua_player::client::watch(move |s| {
        let _ = state_sender.send(s);
    });
    handle
        .insert_source(states, |event, (), d: &mut PlayerDoor| {
            if let channel::Event::Msg(s) = event {
                d.state_arrived(s);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the player channel: {e}"))?;

    // THE COLUMN FOLLOWS THE FOCUSED SCREEN. It is born on it; when focus
    // moves to a window on another screen, the column moves there too — it is
    // the screen being worked on that the player sits beside. The events come
    // through the compositor CONTRACT (`crate::daemon`), and the question
    // "which screen?" is asked of it only when focus moved.
    if door == Door::Column {
        let (focus_sender, focus_events) = channel::channel::<ronoctua_compositor::Event>();
        crate::daemon::listen_to_the_compositor(focus_sender);
        handle
            .insert_source(focus_events, |event, (), d: &mut PlayerDoor| {
                if let channel::Event::Msg(ronoctua_compositor::Event::FocusChanged(_)) = event {
                    d.follow_the_focus();
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the compositor channel: {e}"))?;
    }

    // A PING PER FRAME, from whichever picture's thread has one.
    let (frame_sender, frames) = channel::channel::<usize>();
    handle
        .insert_source(frames, |event, (), d: &mut PlayerDoor| {
            if let channel::Event::Msg(k) = event {
                d.frame_arrived(k);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the preview channel: {e}"))?;

    let mut state = PlayerDoor {
        door,
        conn: conn.clone(),
        handle: handle.clone(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboards: crate::seats::Seats::default(),
        key_here: Modifiers::default(),
        pointers: crate::seats::Seats::default(),
        here: PerSeat::default(),
        shell,
        column_shell,
        qh: qh.clone(),
        on_screen: None,
        wm: None,
        width: 0,
        height: 0,
        asked: (0, 0),
        scale: 1,
        scale_told: 1,
        columns: 40,
        lines: 24,
        quit: false,
        owed: false,
        gpu: None,
        text: None,
        quads: None,
        images: None,
        texts,
        hinted: crate::chrome::Hinted::default(),
        view: crate::player::View::fresh(),
        player: None,
        pictures: Vec::new(),
        frame_sender,
        offset: 0,
        message: None,
        buttons: Vec::new(),
        layout: Layout::default(),
        stage: Stage::Off,
        fullscreen: false,
        controls_until: None,
        controls_timer: None,
        zone_owed: door == Door::Column,
    };
    // THE STAGE IT WAS OPENED ON (`ronoctua-shell player only|maximized`).
    if stage != Stage::Off {
        state.set_stage(stage);
    }

    WaylandSource::new(conn, queue)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;
    // WHERE IT WAS BORN, asked now: the column was made with no screen named,
    // which is the focused one — and the first focus event may already be the
    // move to another.
    if door == Door::Column {
        state.wm = crate::daemon::open_the_compositor().ok();
        state.on_screen = state
            .wm
            .as_mut()
            .and_then(|w| w.focused_output().ok().flatten())
            .map(|o| o.name);
    }
    let signal = cycle.get_signal();
    let run = cycle.run(None, &mut state, |d| {
        d.settle();
        if d.quit {
            signal.stop();
        }
    });
    for p in &state.pictures {
        p.alive.store(false, Ordering::SeqCst);
    }
    if door == Door::Column {
        forget_the_pid();
    }
    run.context("event loop")?;
    Ok(())
}

/// The column's surface, on `output` — or, with none named, on the screen the
/// compositor has focused, which is where a column raised from a key belongs.
fn column_on(
    layer_shell: &LayerShell,
    compositor: &CompositorState,
    qh: &QueueHandle<PlayerDoor>,
    output: Option<&wl_output::WlOutput>,
) -> LayerSurface {
    let layer = layer_shell.create_layer_surface(
        qh,
        compositor.create_surface(qh),
        // `Top`, the footer's layer: above the windows, below a lock.
        Layer::Top,
        Some(NAMESPACE),
        output,
    );
    let width = column_width();
    layer.set_anchor(Anchor::TOP | Anchor::RIGHT | Anchor::BOTTOM);
    layer.set_size(width, 0);
    layer.set_exclusive_zone(i32::try_from(width).unwrap_or(i32::MAX));
    // ON DEMAND, the lifeline's reasoning: a click gives it the keys, and
    // opening it takes them from nobody.
    layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    layer.commit();
    layer
}

/// The column's width in logical pixels: `space.column.width` cells and the
/// margin on both sides.
fn column_width() -> u32 {
    let cell = tokens::metrics::cell_width(tokens::text::size());
    let cells = tokens::space::COLUMN_WIDTH + 2 * MARGIN;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let px = (cell * f32::from(cells)).ceil() as u32;
    px
}

/// The pid in `column.pid`, when that process is alive, is not this one, and
/// IS a column.
///
/// # A living pid is not enough
///
/// A pid is recycled. A column that died without its `Drop` — killed, or
/// taken down with a session — leaves its number in the file, and the next
/// process to get that number is somebody else's: asked only "is it alive",
/// the toggle would send it `TERM`, answer "closed", and raise nothing — a
/// click on the mark that did nothing, which is what the owner saw. So the
/// command line is read too: this program, with `column` as its word.
#[must_use]
pub fn column_pid() -> Option<i32> {
    let path = ronoctua_xdg::runtime_file(COLUMN_PID)?;
    let pid: i32 = std::fs::read_to_string(path).ok()?.trim().parse().ok()?;
    let me = i32::try_from(std::process::id()).ok()?;
    if pid == me {
        return None;
    }
    let line = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    is_a_column(&line).then_some(pid)
}

/// Whether a `/proc/<pid>/cmdline` is this program started as the column.
fn is_a_column(cmdline: &[u8]) -> bool {
    let mut words = cmdline.split(|b| *b == 0).filter(|w| !w.is_empty());
    let program = words
        .next()
        .map(|w| String::from_utf8_lossy(w).into_owned())
        .unwrap_or_default();
    // THE WRAPPED NAME TOO: the installed shell is `wrapProgram`'d, and a
    // column raised by `current_exe()` from inside it starts the real binary,
    // `.ronoctua-shell-wrapped`.
    let ours = std::path::Path::new(&program).file_name().is_some_and(|n| {
        n.to_string_lossy()
            .trim_start_matches('.')
            .starts_with("ronoctua-shell")
    });
    ours && words.next() == Some(b"column".as_slice())
}

/// Ends the column that is up, if one is.
pub fn end_the_column() -> bool {
    column_pid().is_some_and(|pid| {
        rustix::process::Pid::from_raw(pid).is_some_and(|p| {
            rustix::process::kill_process(p, rustix::process::Signal::TERM).is_ok()
        })
    })
}

fn keep_the_pid() {
    if let Some(path) = ronoctua_xdg::runtime_file(COLUMN_PID) {
        let _ = std::fs::write(path, std::process::id().to_string());
    }
}

fn forget_the_pid() {
    if let Some(path) = ronoctua_xdg::runtime_file(COLUMN_PID)
        && std::fs::read_to_string(&path).is_ok_and(|p| p.trim() == std::process::id().to_string())
    {
        let _ = std::fs::remove_file(path);
    }
}

/// What the compositor configured, from either kind of surface: a side it
/// did not give is `None`, and the door keeps the one it had.
struct Asked {
    new_size: (Option<u32>, Option<u32>),
}

/// The surface this host draws on.
enum Shell {
    Window(Window),
    Layer(LayerSurface),
}

impl Shell {
    fn wl_surface(&self) -> &wl_surface::WlSurface {
        match self {
            Self::Window(w) => w.wl_surface(),
            Self::Layer(l) => l.wl_surface(),
        }
    }
}

/// One picture: the size its thread should ask for, and what it received.
struct Picture {
    /// Which video: `None` the main one, `Some(i)` the i-th follower.
    of: Option<usize>,
    wanted: Arc<Mutex<(u32, u32)>>,
    slot: Slot,
    alive: Arc<AtomicBool>,
    frame: Option<ronoctua_render::image::Rgba>,
    uploaded: bool,
}

impl Picture {
    fn key(&self) -> String {
        format!("player:picture:{}", self.of.map_or(0, |i| i + 1))
    }

    /// A picture and its thread.
    fn start(of: Option<usize>, k: usize, ping: channel::Sender<usize>) -> Self {
        let wanted: Arc<Mutex<(u32, u32)>> = Arc::default();
        let slot: Slot = Arc::default();
        let alive = Arc::new(AtomicBool::new(true));
        let (w2, s2, a2) = (wanted.clone(), slot.clone(), alive.clone());
        let _ = std::thread::Builder::new()
            .name("player-picture".into())
            .spawn(move || {
                let mut buf = Vec::new();
                while a2.load(Ordering::SeqCst) {
                    let size = w2.lock().map_or((0, 0), |g| *g);
                    if size.0 == 0 || size.1 == 0 {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        continue;
                    }
                    let mut feed = match ronoctua_player::client::preview(size.0, size.1, of) {
                        Ok(f) => f,
                        Err(e) => {
                            tracing::debug!(error = %e, "no preview yet");
                            std::thread::sleep(std::time::Duration::from_secs(1));
                            continue;
                        }
                    };
                    // PATIENCE, so a paused film still notices the door
                    // changed size: no frame comes, and the thread has to
                    // look up anyway.
                    let _ = feed.patience(Some(std::time::Duration::from_millis(250)));
                    tracing::debug!(of = ?of, width = feed.width, height = feed.height, "a preview");
                    loop {
                        if !a2.load(Ordering::SeqCst) {
                            return;
                        }
                        if w2.lock().map_or(size, |g| *g) != size {
                            break;
                        }
                        match feed.next(&mut buf) {
                            Ok(true) => {
                                if let Ok(mut s) = s2.lock() {
                                    *s = Some((feed.width, feed.height, buf.clone()));
                                }
                                if ping.send(k).is_err() {
                                    return;
                                }
                            }
                            Ok(false) => {}
                            Err(e) => {
                                tracing::debug!(error = %e, "the preview ended");
                                break;
                            }
                        }
                    }
                }
            });
        Self {
            of,
            wanted,
            slot,
            alive,
            frame: None,
            uploaded: false,
        }
    }
}

/// What this door keeps for ONE pointer — see `crate::seats`.
#[derive(Debug, Default)]
struct PerSeat {
    at: (f64, f64),
    /// Dragging the list's bar.
    dragging: bool,
    /// Dragging the time bar.
    seeking: bool,
    lines_owed: f64,
    row_under_pointer: Option<usize>,
    /// The transport button under the pointer.
    button_under: Option<&'static str>,
    hint_drag: bool,
}

struct PlayerDoor {
    door: Door,
    conn: Connection,
    handle: calloop::LoopHandle<'static, Self>,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboards: crate::seats::Seats<Modifiers, wl_keyboard::WlKeyboard>,
    key_here: Modifiers,
    pointers: crate::seats::Seats<PerSeat>,
    here: PerSeat,
    shell: Shell,
    /// The layer shell, kept to raise the column again on another screen.
    column_shell: Option<LayerShell>,
    qh: QueueHandle<Self>,
    /// The screen the column is on, by its connector name, once known.
    on_screen: Option<String>,
    /// The compositor, through the contract — opened the first time focus
    /// moves.
    wm: Option<Box<dyn ronoctua_compositor::Compositor>>,
    width: u32,
    height: u32,
    asked: (u32, u32),
    scale: i32,
    scale_told: i32,
    columns: u16,
    lines: u16,
    quit: bool,
    owed: bool,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    quads: Option<ronoctua_render::quad::Quads>,
    images: Option<ronoctua_render::image::Images>,
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    hinted: crate::chrome::Hinted,
    view: crate::player::View,
    player: Option<State>,
    pictures: Vec<Picture>,
    frame_sender: channel::Sender<usize>,
    offset: usize,
    message: Option<String>,
    /// The transport as last DRAWN: `(first column, last column, id)`.
    buttons: Vec<(u16, u16, &'static str)>,
    /// Where everything was last laid out.
    layout: Layout,
    /// How the picture is shown.
    stage: Stage,
    /// Whether the compositor says the window fills the screen.
    fullscreen: bool,
    /// On stage, when the controls over the picture go.
    controls_until: Option<std::time::Instant>,
    /// The timer that takes them away, armed only while they are shown.
    controls_timer: Option<calloop::RegistrationToken>,
    /// The column's exclusive zone, to be said AGAIN in the commit that
    /// maps it — see [`PlayerDoor::zone_in_the_mapping_commit`].
    zone_owed: bool,
}

impl PlayerDoor {
    /// Says the column's exclusive zone again, so it travels in the commit
    /// that MAPS the surface.
    ///
    /// # Why, when it was said at creation already
    ///
    /// Because of what this compositor does with a layer's commit
    /// (`sway/desktop/layer_shell.c`, `handle_surface_commit` in the pinned
    /// source): every commit re-arranges the layers — and so the windows —
    /// but the arrangement is only SENT to them (`transaction_commit_dirty`)
    /// when the commit carries some layer state other than keyboard
    /// interactivity. The commit that maps the column carries a buffer and no
    /// state, so the windows were re-tiled and not told: the column opened
    /// over them, and they moved at the next click anywhere — which is what
    /// the owner saw. Invariant 3 forbids the patch; putting a state in that
    /// commit is the client's half, and wlroots counts a value set again as
    /// changed.
    fn zone_in_the_mapping_commit(&mut self) {
        if !std::mem::take(&mut self.zone_owed) {
            return;
        }
        if let Shell::Layer(layer) = &self.shell {
            layer.set_exclusive_zone(i32::try_from(column_width()).unwrap_or(i32::MAX));
        }
    }

    /// Whether the picture is shown alone.
    fn on_stage(&self) -> bool {
        self.stage != Stage::Off
    }

    /// Whether the transport and the header are drawn — always off stage,
    /// and on stage for a while after activity.
    fn controls_shown(&self) -> bool {
        !self.on_stage()
            || self
                .controls_until
                .is_some_and(|u| std::time::Instant::now() < u)
    }

    /// Whether the pointer rests on the controls drawn over the picture.
    fn pointer_on_controls(&self) -> bool {
        let (_, y) = self.here.at;
        if y < 0.0 {
            return false;
        }
        let l = Self::line_at(y);
        l <= self.hinted.rows() || l >= self.layout.transport
    }

    /// Activity: on stage, the controls come back over the picture and a
    /// timer is armed to take them away — one timer, for as long as they
    /// are shown, and none at rest.
    fn wake_controls(&mut self) {
        if !self.on_stage() {
            return;
        }
        if !self.controls_shown() {
            self.owed = true;
        }
        self.controls_until = Some(std::time::Instant::now() + CONTROLS_LINGER);
        if self.controls_timer.is_some() {
            return;
        }
        let armed = self.handle.insert_source(
            calloop::timer::Timer::from_duration(CONTROLS_LINGER),
            |_, (), d: &mut Self| {
                let now = std::time::Instant::now();
                if d.on_stage() && d.pointer_on_controls() {
                    d.controls_until = Some(now + CONTROLS_LINGER);
                }
                match d.controls_until {
                    Some(until) if d.on_stage() && until > now => {
                        calloop::timer::TimeoutAction::ToDuration(until - now)
                    }
                    _ => {
                        d.controls_until = None;
                        d.controls_timer = None;
                        d.here.button_under = None;
                        d.owed = true;
                        calloop::timer::TimeoutAction::Drop
                    }
                }
            },
        );
        match armed {
            Ok(token) => self.controls_timer = Some(token),
            // NOT SILENT, and the controls STAY: a transport that does not go
            // is a smaller failure than one that cannot be brought back.
            Err(e) => tracing::warn!(error = %e, "no timer for the controls; they will stay"),
        }
    }

    /// Shows the picture alone, on the whole screen, or back in the door.
    ///
    /// A column is a strip of the screen and cannot fill it: there, the
    /// whole screen is the WINDOW, opened on it.
    fn set_stage(&mut self, to: Stage) {
        if self.door == Door::Column && to == Stage::Maximized {
            if let Ok(me) = std::env::current_exe()
                && let Err(e) = std::process::Command::new(me)
                    .args(["player", "maximized"])
                    .stdin(std::process::Stdio::null())
                    .spawn()
            {
                tracing::warn!(error = %e, "could not open the player on the whole screen");
            }
            return;
        }
        if let Shell::Window(w) = &self.shell {
            if to == Stage::Maximized {
                w.set_fullscreen(None);
            } else if self.stage == Stage::Maximized {
                w.unset_fullscreen();
            }
        }
        self.stage = to;
        self.offset = 0;
        self.controls_until = None;
        self.wake_controls();
        self.owed = true;
    }

    /// A press of `o` or `m`, or of their buttons: on, or back off.
    fn toggle_stage(&mut self, show: crate::player::Show) {
        let to = match show {
            crate::player::Show::Only => Stage::Only,
            crate::player::Show::Maximized => Stage::Maximized,
        };
        self.set_stage(if self.stage == to { Stage::Off } else { to });
    }

    /// Where a picture is on the surface: its band, or the whole surface on
    /// stage.
    fn picture_area(&self, place: Place) -> ronoctua_render::image::Area {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (self.width as f32, self.height as f32);
        if self.on_stage() {
            return ronoctua_render::image::Area {
                x: 0.0,
                y: 0.0,
                width: w,
                height: h,
            };
        }
        place.area(w, Self::line_px())
    }
    /// What the compositor configured, from either surface — the ONE place
    /// `asked` is written (`crate::hidpi`: the size asked of the GPU is the
    /// compositor's number, and what is laid out is what the buffer can show).
    fn configure(&mut self, cfg: &Asked) {
        self.asked = (
            cfg.new_size.0.unwrap_or(self.asked.0),
            cfg.new_size.1.unwrap_or(self.asked.1),
        );
        // SAID, because a size the compositor computed and never SENT is the
        // defect `zone_in_the_mapping_commit` exists for, and only this line
        // tells the two apart: the compositor's own tree shows the new size
        // either way.
        tracing::info!(width = self.asked.0, height = self.asked.1, door = ?self.door, "configured");
        self.width = self.asked.0;
        self.height = self.asked.1;
        let (asked, scale) = (self.asked, self.scale);
        if let Some(g) = self.gpu.as_mut() {
            let (shown_w, shown_h) = crate::hidpi::resized(g, asked, scale);
            self.width = shown_w;
            self.height = shown_h;
        }
        self.recount();
        self.owed = true;
    }

    /// Moves the column to the screen that has focus, if that is another one.
    fn follow_the_focus(&mut self) {
        if self.wm.is_none() {
            self.wm = crate::daemon::open_the_compositor().ok();
        }
        let Some(name) = self
            .wm
            .as_mut()
            .and_then(|w| w.focused_output().ok().flatten())
            .map(|o| o.name)
        else {
            return;
        };
        if self.on_screen.as_deref() == Some(name.as_str()) {
            return;
        }
        let Some(output) = self
            .outputs
            .outputs()
            .find(|o| self.outputs.info(o).and_then(|i| i.name).as_deref() == Some(name.as_str()))
        else {
            return;
        };
        let Some(layer_shell) = self.column_shell.as_ref() else {
            return;
        };
        let layer = column_on(layer_shell, &self._compositor, &self.qh, Some(&output));
        self.zone_owed = true;
        // A NEW SURFACE IS A NEW GPU SURFACE: everything drawn on the old one
        // is made again on the first configure of this one.
        self.gpu = None;
        self.text = None;
        self.quads = None;
        self.images = None;
        for p in &mut self.pictures {
            p.uploaded = false;
        }
        self.width = 0;
        self.shell = Shell::Layer(layer);
        self.on_screen = Some(name);
    }

    fn state_arrived(&mut self, s: Option<State>) {
        let spot = self.rows().get(self.view.cursor()).map(|r| r.spot.clone());
        self.player = s;
        if let Some(spot) = spot {
            self.view
                .follow(&spot, self.player.as_ref(), self.columns, &self.texts);
        }
        self.view
            .settle_in(self.player.as_ref(), self.columns, &self.texts);
        self.owed = true;
    }

    fn frame_arrived(&mut self, k: usize) {
        if let Some(p) = self.pictures.get_mut(k)
            && let Some((w, h, px)) = p.slot.lock().ok().and_then(|mut s| s.take())
        {
            p.frame = ronoctua_render::image::Rgba::new(w, h, px);
            tracing::trace!(k, w, h, taken = p.frame.is_some(), "a frame");
            p.uploaded = false;
            self.owed = true;
        }
    }

    fn rows(&self) -> Vec<crate::player::Row> {
        self.view
            .rows(self.player.as_ref(), self.columns, &self.texts)
    }

    fn line_px() -> f32 {
        tokens::metrics::line_height(tokens::text::size())
    }

    fn cell_px() -> f32 {
        tokens::metrics::cell_width(tokens::text::size())
    }

    /// Lays the door out from the state and the size.
    fn relayout(&mut self) {
        let video = self.player.as_ref().and_then(|s| s.video);
        let followers = self.player.as_ref().map_or(0, |s| s.participants.len());
        self.layout = if self.on_stage() {
            stage_layout(self.lines)
        } else {
            layout(
                self.door,
                1 + self.hinted.rows(),
                self.lines,
                self.columns + 2 * MARGIN,
                Self::line_px() / Self::cell_px(),
                video,
                followers,
            )
        };
        // A THREAD PER PICTURE, started and ended with the layout.
        let n = self.layout.pictures.len();
        while self.pictures.len() > n {
            if let Some(p) = self.pictures.pop() {
                p.alive.store(false, Ordering::SeqCst);
            }
        }
        while self.pictures.len() < n {
            let k = self.pictures.len();
            let of = k.checked_sub(1);
            self.pictures
                .push(Picture::start(of, k, self.frame_sender.clone()));
        }
        // AND THE SIZE EACH ASKS FOR, in PHYSICAL pixels: the frame is drawn
        // one pixel to one pixel, sharp on a scale-2 screen.
        //
        // THE VIDEO'S OWN PROPORTION, fitted in the band — not the band's: a
        // frame asked in the band's shape comes back letterboxed by the player,
        // in black, and the band's surface tone behind it never shows. Asked
        // in the video's shape, the band is what surrounds it. Only the main
        // picture's shape is known (`State::video`); a follower keeps the
        // band's.
        //
        // ON STAGE THE BAND'S SHAPE, on purpose: there the letterbox the
        // player draws is the black a picture on its own is framed in.
        let video = self.player.as_ref().and_then(|s| s.video);
        for (p, place) in self.pictures.iter().zip(&self.layout.pictures) {
            let area = self.picture_area(*place);
            let (fw, fh) = fitted(
                area.width,
                area.height,
                if p.of.is_none() && !self.on_stage() {
                    video
                } else {
                    None
                },
            );
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            let size = (
                (fw * self.scale as f32) as u32,
                (fh * self.scale as f32) as u32,
            );
            if let Ok(mut w) = p.wanted.lock() {
                *w = size;
            }
        }
    }

    /// How many rows fit under the transport.
    fn room(&self) -> usize {
        usize::from(self.lines.saturating_sub(self.layout.list))
    }

    fn settle_offset(&mut self, rows: usize) {
        self.offset = self.offset.min(rows.saturating_sub(self.room()));
    }

    fn follow(&mut self, rows: usize) {
        const EDGE: usize = 2;
        let room = self.room();
        if room == 0 {
            return;
        }
        let cursor = self.view.cursor();
        if cursor < self.offset + EDGE {
            self.offset = cursor.saturating_sub(EDGE);
        } else if cursor + EDGE >= self.offset + room {
            self.offset = cursor + EDGE + 1 - room;
        }
        self.offset = self.offset.min(rows.saturating_sub(room));
    }

    fn apply(&mut self, errand: crate::player::Errand) {
        match crate::player::carry_out(errand, &self.texts) {
            Ok(said) => self.message = said,
            Err(crate::player::Errand::Close) => {
                self.quit = true;
                return;
            }
            Err(crate::player::Errand::Show(show)) => self.toggle_stage(show),
            // THIS DOOR SENDS IT through the footer's control socket. A
            // question opens in the FOOTER's minibuffer, and this door says
            // so: letters typed here would go nowhere.
            Err(crate::player::Errand::Run(id)) => {
                if let Err(e) = crate::leader::send_it(&format!("command {id}")) {
                    tracing::warn!(id, error = %e, "the footer did not take the command");
                } else if matches!(
                    id,
                    "player-play-file" | "player-enqueue" | "player-follow" | "player-pair"
                ) {
                    self.message = Some(self.texts.t(ronoctua_i18n::key::PLAYER_TYPED_ELSEWHERE));
                }
            }
            Err(_) => {}
        }
        self.owed = true;
    }

    fn tell(&mut self, ask: &Ask) {
        self.message = crate::player::tell(ask, &self.texts);
        self.owed = true;
    }

    fn text_size_changed(&mut self) {
        if let Some(t) = self.text.as_mut() {
            t.draw_at(tokens::text::size());
        }
        if self.door == Door::Column
            && let Shell::Layer(l) = &self.shell
        {
            let w = column_width();
            l.set_size(w, 0);
            l.set_exclusive_zone(i32::try_from(w).unwrap_or(i32::MAX));
            l.commit();
        }
        self.recount();
        self.owed = true;
    }

    fn recount(&mut self) {
        let size = tokens::text::size();
        let cell = tokens::metrics::cell_width(size);
        let line = tokens::metrics::line_height(size);
        #[allow(clippy::cast_precision_loss)]
        let usable = (self.width as f32) - 2.0 * cell * f32::from(MARGIN);
        self.columns = tokens::metrics::columns(usable.max(0.0), size);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let lines = (self.height as f32 / line).floor().max(0.0) as u16;
        self.lines = lines;
    }

    fn assemble_gpu(&mut self) {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        };
        let (Some(disp), Some(surf)) = (
            std::ptr::NonNull::new(self.conn.backend().display_ptr().cast()),
            std::ptr::NonNull::new(self.shell.wl_surface().id().as_ptr().cast()),
        ) else {
            tracing::error!("null Wayland pointers");
            return;
        };
        let first = crate::hidpi::physical(self.asked, self.scale);
        // SAFETY: `conn` and `shell` are fields of `self`, like the `Gpu` —
        // the handles do not outlive what produced them.
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
            tracing::error!("no usable GPU; the player will not draw");
            return;
        };
        let (w, h) = crate::hidpi::resized(&mut g, self.asked, self.scale);
        self.width = w;
        self.height = h;
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
        match ronoctua_render::quad::Quads::new(&g.device, g.config.format) {
            Ok(q) => self.quads = Some(q),
            Err(e) => tracing::error!(error = %e, "assembling the quads failed"),
        }
        match ronoctua_render::image::Images::new(&g.device, g.config.format) {
            Ok(i) => self.images = Some(i),
            Err(e) => tracing::error!(error = %e, "assembling the images failed"),
        }
        self.gpu = Some(g);
    }

    fn typed(&mut self, event: KeyEvent) {
        self.wake_controls();
        // `Esc` ON STAGE brings the door back first — one level, as every
        // view with levels has it — and only then reaches the view.
        if event.keysym == Keysym::Escape && self.on_stage() {
            self.set_stage(Stage::Off);
            return;
        }
        let notation = match event.keysym {
            // ESCAPE GOES BACK ONE LEVEL, and closes the door from the top:
            // the view answers which.
            Keysym::Escape => {
                self.pressed(crate::player::Press::Escape);
                return;
            }
            Keysym::Return | Keysym::KP_Enter => "RET",
            Keysym::space => "SPC",
            Keysym::Up => "<up>",
            Keysym::Down => "<down>",
            Keysym::Left => "<left>",
            Keysym::Right => "<right>",
            Keysym::BackSpace => "DEL",
            _ => match event.utf8.as_deref() {
                Some(t) => t,
                None => return,
            },
        };
        if let Some(press) = crate::player::pressed(notation) {
            self.pressed(press);
        }
    }

    fn pressed(&mut self, press: crate::player::Press) {
        self.message = None;
        let errand = self
            .view
            .key(press, self.player.as_ref(), self.columns, &self.texts);
        let rows = self.rows().len();
        self.follow(rows);
        self.apply(errand);
    }

    fn settle(&mut self) {
        if std::mem::take(&mut self.owed) {
            self.paint();
            if self.hinted.settled() {
                self.paint();
            }
        }
    }

    /// The time bar's track, in logical pixels: `(left, right)`.
    fn bar_track(&self) -> (f32, f32) {
        let cell = Self::cell_px();
        #[allow(clippy::cast_precision_loss)]
        let right = self.width as f32 - cell * f32::from(MARGIN);
        (
            cell * f32::from(MARGIN),
            right.max(cell * f32::from(MARGIN)),
        )
    }

    /// Where on the time bar `x` is, as a fraction.
    #[allow(clippy::cast_possible_truncation)]
    fn fraction_at(&self, x: f64) -> f64 {
        let (l, r) = self.bar_track();
        ((x as f32 - l) / (r - l).max(1.0)).clamp(0.0, 1.0).into()
    }

    #[allow(clippy::too_many_lines)]
    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        if self.message.is_some() {
            self.hinted.say(None);
        }
        self.relayout();
        let size = tokens::text::size();
        let line = Self::line_px();
        let cell = Self::cell_px();
        let rows = self.rows();
        self.settle_offset(rows.len());
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (self.width as f32, self.height as f32);
        let span =
            |text: String, column: u16, line: u16, color: tokens::Color| ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text,
                column,
                line,
                color,
                edge: false,
                foot: false,
            };

        let mut spans: Vec<ronoctua_render::Span> = Vec::new();
        let mut under: Vec<ronoctua_render::quad::Quad> = vec![ronoctua_render::quad::Quad {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
            color: tokens::color::SURFACE_CHROME,
        }];

        // LINE 0: the keys, what the last order said, or — with the pointer on
        // a button — what that button does.
        let under_button = self.here.button_under;
        let (pieces, hinting) = match (&self.message, under_button) {
            (Some(t), _) => (vec![crate::chrome::Piece::said(t.clone())], false),
            (None, Some(id)) => (
                vec![crate::chrome::Piece::said(
                    crate::chrome::hint_sentence(id, &self.texts).unwrap_or_default(),
                )],
                false,
            ),
            (None, None) => (
                self.view
                    .hint(self.player.as_ref(), self.columns, &self.texts),
                true,
            ),
        };
        // ON STAGE, what is drawn over the picture: the translucent band
        // under the header and the transport, and the time bar — after the
        // images, or the picture would cover them.
        let stage = self.on_stage();
        let shown = self.controls_shown();
        let mut top: Vec<ronoctua_render::quad::Quad> = Vec::new();
        if shown {
            spans.extend(
                self.hinted
                    .head(&pieces, hinting, MARGIN, self.columns, &self.texts),
            );
        }
        if stage && shown {
            let header = line * f32::from(1 + self.hinted.rows());
            let foot = line * f32::from(self.layout.transport);
            for (y, height) in [(0.0, header), (foot, h - foot)] {
                top.push(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y,
                    width: w,
                    height,
                    color: tokens::color::SURFACE_TRANSPORT,
                });
            }
        }

        // THE PICTURES: a band each, and the frame fitted into it. The band is
        // a surface of the theme in the door, and the content's ground on
        // stage, where a picture alone is framed in black.
        let mut pictures: Vec<(String, ronoctua_render::image::Area)> = Vec::new();
        for (p, place) in self.pictures.iter().zip(&self.layout.pictures) {
            let band = self.picture_area(*place);
            under.push(ronoctua_render::quad::Quad {
                x: band.x,
                y: band.y,
                width: band.width,
                height: band.height,
                color: if stage {
                    tokens::color::SURFACE_BASE
                } else {
                    tokens::color::SURFACE_BACKDROP
                },
            });
            if let Some(f) = &p.frame {
                // THE FRAME IS ALREADY THE BAND'S SHAPE (the player rendered it
                // at this size, letterboxed); `picture_in` only keeps it inside
                // the band while a new size is on its way.
                let fit =
                    crate::camera::picture_in(band.width, band.height, (f.width, f.height), false);
                pictures.push((
                    p.key(),
                    ronoctua_render::image::Area {
                        x: band.x + fit.x,
                        y: band.y + fit.y,
                        width: fit.width,
                        height: fit.height,
                    },
                ));
            }
        }

        // THE TRANSPORT: glyph buttons, then the clock.
        let tl = self.layout.transport;
        self.buttons.clear();
        let mut at = MARGIN;
        let s = self.player.as_ref();
        // HIDDEN CONTROLS ARE NOT CONTROLS: nothing is laid out for the hand,
        // so a click on stage lands on the picture and not on a button nobody
        // can see.
        for id in BUTTONS.iter().copied().filter(|_| shown) {
            let colour = if under_button == Some(id) || lit(id, s, self.stage) {
                tokens::color::STATE_ACCENT
            } else {
                tokens::color::TEXT_PRIMARY
            };
            // A GLYPH AND AIR ON BOTH SIDES: two cells a button, and the
            // click answers on both.
            self.buttons.push((at, at + 2, id));
            spans.push(span(
                glyph_of(id, s, self.stage).to_string(),
                at,
                tl,
                colour,
            ));
            at += 3;
        }
        if let Some(s) = s.filter(|s| s.current.is_some() && shown) {
            let clock = s.duration.map_or_else(
                || crate::player::clock(s.position),
                |d| {
                    format!(
                        "{} / {}",
                        crate::player::clock(s.position),
                        crate::player::clock(d)
                    )
                },
            );
            let cells = ronoctua_protocol::cells(&clock);
            let right = MARGIN + self.columns;
            let from = right.saturating_sub(cells).max(at + 1);
            spans.push(span(clock, from, tl, tokens::color::TEXT_SECONDARY));
        }

        // THE TIME BAR: a rule, and the part already played in the accent.
        let (l, r) = self.bar_track();
        let thickness = f32::from(tokens::space::RULE_THICKNESS).max(1.0) * 3.0;
        let y = line * f32::from(self.layout.bar) + (line - thickness) / 2.0;
        let bar = if stage { &mut top } else { &mut under };
        bar.push(ronoctua_render::quad::Quad {
            x: l,
            y,
            width: r - l,
            height: thickness,
            color: tokens::color::RULE_DEFAULT,
        });
        if let Some(s) = s
            && let Some(d) = s.duration.filter(|d| *d > 0.0)
        {
            #[allow(clippy::cast_possible_truncation)]
            let played = ((s.position / d).clamp(0.0, 1.0)) as f32;
            bar.push(ronoctua_render::quad::Quad {
                x: l,
                y,
                width: (r - l) * played,
                height: thickness,
                color: tokens::color::STATE_ACCENT,
            });
        }

        // THE ROWS, under all of it, scrolling.
        let first = self.layout.list;
        for (i, row) in rows.iter().skip(self.offset).take(self.room()).enumerate() {
            let y = first + u16::try_from(i).unwrap_or(0);
            for f in &row.fields {
                spans.push(span(f.text.clone(), MARGIN + f.at, y, f.color));
            }
        }
        let scroll = crate::chrome::list_scroll(rows.len(), self.room(), self.offset);
        let bands = crate::chrome::hover_bands(
            self.pointers
                .lit(&self.here)
                .map(|(color, s)| (color, s.row_under_pointer)),
            self.offset,
            self.room(),
            rows.len(),
            first,
            w,
        );
        let mut over: Vec<ronoctua_render::quad::Quad> =
            if stage && !shown { Vec::new() } else { top };
        over.extend(bands);
        if !stage {
            over.extend(crate::chrome::list_bar(scroll, first, w, h));
        }
        if shown {
            over.extend(self.hinted.bar(crate::chrome::Frame::of(w, h, 0)));
        }
        let _ = cell;

        let under_count = u32::try_from(under.len()).unwrap_or(0);
        under.extend(over);
        self.zone_in_the_mapping_commit();
        let surface = self.shell.wl_surface().clone();
        let scale = self.scale;
        let told = &mut self.scale_told;
        let (Some(gpu), Some(text)) = (self.gpu.as_mut(), self.text.as_mut()) else {
            return;
        };
        if let Some(im) = self.images.as_mut() {
            im.clear();
            for p in &mut self.pictures {
                if let Some(f) = &p.frame
                    && (!p.uploaded || im.missing(&p.key()))
                {
                    im.upload_it(&gpu.device, &gpu.queue, &p.key(), f);
                    p.uploaded = true;
                }
            }
            im.prepare(&gpu.queue, self.width, self.height, &pictures);
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
            q.prepare(&gpu.device, &gpu.queue, self.width, self.height, &under);
        }
        // UNDER THE PICTURES the grounds and the time bar; OVER them the bands
        // and the bars; the text last. No depth test: later is on top.
        let quads_ref = self.quads.as_ref();
        let images_ref = self.images.as_ref();
        if let Err(e) = gpu.frame(
            || crate::hidpi::told_before_present(scale, told, &surface),
            |pass| {
                if let Some(q) = quads_ref {
                    q.draw_range(pass, 0, under_count);
                }
                if let Some(im) = images_ref {
                    im.draw(pass);
                }
                if let Some(q) = quads_ref {
                    q.draw_range(pass, under_count, u32::MAX);
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

    fn line_at(y: f64) -> u16 {
        let line = f64::from(Self::line_px()).max(1.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let l = (y.max(0.0) / line) as u16;
        l
    }

    fn column_at(x: f64) -> u16 {
        let cell = f64::from(Self::cell_px()).max(1.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let c = (x.max(0.0) / cell) as u16;
        c
    }

    /// The row of the list at height `y`.
    fn row_at(&self, y: f64) -> Option<usize> {
        let l = Self::line_at(y);
        (l >= self.layout.list).then(|| usize::from(l - self.layout.list) + self.offset)
    }

    /// The transport button at `(x, y)`, where it was DRAWN.
    fn button_at(&self, x: f64, y: f64) -> Option<&'static str> {
        if Self::line_at(y) != self.layout.transport {
            return None;
        }
        let c = Self::column_at(x);
        self.buttons
            .iter()
            .find(|(a, b, _)| c >= *a && c < *b)
            .map(|(_, _, id)| *id)
    }

    /// Whether `y` is on a picture.
    fn on_a_picture(&self, y: f64) -> bool {
        let l = Self::line_at(y);
        self.layout
            .pictures
            .iter()
            .any(|p| l >= p.first && l < p.first + p.lines)
    }
}

impl WindowHandler for PlayerDoor {
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
        // THE SCREEN LEFT BY ANOTHER WAY — the compositor's own key, another
        // window taking it — takes the stage with it, or the door would sit
        // on "whole screen" in a window.
        let now = cfg.is_fullscreen();
        if self.fullscreen && !now && self.stage == Stage::Maximized {
            self.stage = Stage::Off;
            self.controls_until = None;
        }
        self.fullscreen = now;
        self.configure(&Asked {
            new_size: (
                Some(cfg.new_size.0.map_or(760, std::num::NonZeroU32::get)),
                Some(cfg.new_size.1.map_or(720, std::num::NonZeroU32::get)),
            ),
        });
    }
}

impl LayerShellHandler for PlayerDoor {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.quit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        cfg: LayerSurfaceConfigure,
        _: u32,
    ) {
        // A ZERO IS "keep yours": the column's width is its own, and its
        // height is what the compositor hands it.
        self.configure(&Asked {
            new_size: (
                Some(cfg.new_size.0).filter(|w| *w != 0),
                Some(cfg.new_size.1).filter(|h| *h != 0),
            ),
        });
        layer.commit();
    }
}

impl SeatHandler for PlayerDoor {
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
                    let name = self.seats.info(&seat).and_then(|i| i.name);
                    self.keyboards.took(&seat, k, name.as_deref());
                }
                Err(e) => tracing::warn!(error = %e, "no keyboard; there is no typing"),
            }
        }
        if cap == Capability::Pointer {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => {
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

impl KeyboardHandler for PlayerDoor {
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
        if self.hinted.took_a_key(event.keysym, true) {
            self.owed = true;
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
        self.hinted.took_a_key(event.keysym, false);
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.keyboards.switch_to(kb, &mut self.key_here);
        self.key_here = modifiers;
    }
}

impl PointerHandler for PlayerDoor {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        self.pointers.switch_to(pointer, &mut self.here);
        self.hinted.swap_hand(&mut self.here.hint_drag);
        self.pointer_frame_here(events);
        self.hinted.swap_hand(&mut self.here.hint_drag);
    }
}

impl PlayerDoor {
    #[allow(clippy::too_many_lines)]
    fn pointer_frame_here(&mut self, events: &[PointerEvent]) {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (self.width as f32, self.height as f32);
        let head = crate::chrome::Frame::of(w, h, 0);
        let before = (self.here.row_under_pointer, self.here.button_under);
        let total = self.rows().len();
        let scroll = crate::chrome::list_scroll(total, self.room(), self.offset);
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (x, y) = (e.position.0 as f32, e.position.1 as f32);
            // ACTIVITY IS ANY OF IT but leaving: on stage the controls come
            // back over the picture.
            if !matches!(e.kind, PointerEventKind::Leave { .. }) {
                self.here.at = e.position;
                self.wake_controls();
            }
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.here.at = e.position;
                    if self.hinted.dragged(head, e.position.1) {
                        self.owed = true;
                        continue;
                    }
                    let now = self
                        .hinted
                        .under(e.position.0, e.position.1)
                        .and_then(|id| crate::chrome::hint_sentence(id, &self.texts));
                    if self.hinted.say(now) {
                        self.owed = true;
                    }
                    if self.here.dragging {
                        self.offset =
                            crate::chrome::list_offset_at(scroll, self.layout.list, w, h, y);
                        self.owed = true;
                        continue;
                    }
                    if self.here.seeking {
                        let f = self.fraction_at(e.position.0);
                        self.seek_to(f);
                        continue;
                    }
                    self.here.row_under_pointer = self.row_at(e.position.1);
                    self.here.button_under = self.button_at(e.position.0, e.position.1);
                }
                PointerEventKind::Leave { .. } => {
                    self.here.at = (-1.0, -1.0);
                    self.hinted.let_go();
                    if self.hinted.say(None) {
                        self.owed = true;
                    }
                    self.here.dragging = false;
                    self.here.seeking = false;
                    self.here.row_under_pointer = None;
                    self.here.button_under = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    self.hinted.let_go();
                    self.here.dragging = false;
                    self.here.seeking = false;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if self.hinted.pressed(head, e.position.0, e.position.1) {
                        self.owed = true;
                        continue;
                    }
                    if crate::chrome::on_the_list_bar(scroll, self.layout.list, w, h, x, y) {
                        self.here.dragging = true;
                        self.offset =
                            crate::chrome::list_offset_at(scroll, self.layout.list, w, h, y);
                        self.owed = true;
                        continue;
                    }
                    // A BUTTON of the transport: what its key does.
                    if let Some(id) = self.button_at(e.position.0, e.position.1) {
                        if let Some(ask) = ask_of(id) {
                            self.tell(&ask);
                        }
                        match id {
                            "player-only" => self.toggle_stage(crate::player::Show::Only),
                            "player-maximize" => {
                                self.toggle_stage(crate::player::Show::Maximized);
                            }
                            _ => {}
                        }
                        continue;
                    }
                    // THE TIME BAR: a press seeks there, and holding drags.
                    if Self::line_at(e.position.1) == self.layout.bar && self.controls_shown() {
                        self.here.seeking = true;
                        let f = self.fraction_at(e.position.0);
                        self.seek_to(f);
                        continue;
                    }
                    // A PICTURE: a click plays or pauses, as on every player.
                    if self.on_a_picture(e.position.1) {
                        self.tell(&Ask::Toggle);
                        continue;
                    }
                    // A ROW: the first click selects, the second acts.
                    if let Some(row) = self.row_at(e.position.1) {
                        let rows = self.rows();
                        let again = self.view.clicked(row, &rows);
                        let n = rows.len();
                        self.follow(n);
                        if again {
                            self.pressed(crate::player::Press::Enter);
                        } else {
                            self.owed = true;
                        }
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::surface::wheel_arrived(
                        &mut self.here.lines_owed,
                        &vertical,
                        &horizontal,
                    );
                    let by = crate::surface::wheel_lines(&mut self.here.lines_owed);
                    if by == 0 {
                        continue;
                    }
                    if self.hinted.turned(head, e.position.1, by) {
                        self.owed = true;
                        continue;
                    }
                    // OVER THE PICTURES AND THE TRANSPORT THE WHEEL IS THE
                    // VOLUME — the gesture belongs to what is under the hand,
                    // and there is no list there to walk.
                    if Self::line_at(e.position.1) < self.layout.list {
                        let now = self.player.as_ref().map_or(100, |s| s.volume);
                        let step = i64::from(crate::player::VOLUME_STEP);
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let percent = (i64::from(now) - by * step)
                            .clamp(0, i64::from(crate::player::VOLUME_MOST))
                            as u8;
                        self.tell(&Ask::Volume { percent });
                        continue;
                    }
                    let last = total.saturating_sub(self.room());
                    self.offset = usize::try_from(
                        i64::try_from(self.offset)
                            .unwrap_or(0)
                            .saturating_add(by)
                            .max(0),
                    )
                    .unwrap_or(0)
                    .min(last);
                    self.owed = true;
                }
                _ => {}
            }
        }
        if before != (self.here.row_under_pointer, self.here.button_under) {
            self.owed = true;
        }
    }

    fn seek_to(&mut self, fraction: f64) {
        if let Some(d) = self.player.as_ref().and_then(|s| s.duration) {
            self.tell(&Ask::Seek {
                seconds: d * fraction,
                relative: false,
            });
        }
    }
}

impl CompositorHandler for PlayerDoor {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
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
            self.width = w;
            self.height = h;
            self.recount();
            self.owed = true;
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

impl OutputHandler for PlayerDoor {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for PlayerDoor {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(PlayerDoor);
smithay_client_toolkit::delegate_dispatch2!(PlayerDoor);

#[cfg(test)]
mod tests {
    /// FAILS IF: a recycled pid is taken for the column — the toggle would
    /// kill somebody else's process and raise nothing.
    #[test]
    fn only_the_column_is_the_column() {
        assert!(super::is_a_column(
            b"/nix/store/x/bin/ronoctua-shell\0column\0"
        ));
        assert!(!super::is_a_column(
            b"/nix/store/x/bin/ronoctua-shell\0player\0"
        ));
        assert!(super::is_a_column(
            b"/nix/store/x/bin/.ronoctua-shell-wrapped\0column\0"
        ));
        assert!(!super::is_a_column(b"/usr/bin/zsh\0column\0"));
        assert!(!super::is_a_column(b""));
    }

    use super::*;

    #[test]
    fn with_no_video_the_transport_is_under_the_header() {
        let l = layout(Door::Column, 1, 40, 50, 2.0, None, 0);
        assert!(l.pictures.is_empty());
        assert_eq!((l.transport, l.bar, l.list), (1, 2, 3));
    }

    #[test]
    fn a_wide_video_takes_the_height_its_shape_wants() {
        // 60 cells wide, a line twice a cell: 16:9 wants 60 × 9/16 / 2 ≈ 17.
        let l = layout(Door::Window, 1, 60, 60, 2.0, Some((1920, 1080)), 0);
        assert_eq!((l.pictures[0].first, l.pictures[0].lines), (1, 17));
        assert_eq!(l.transport, 18);
    }

    /// FAILS IF: a picture asks for the band's shape again, and the player
    /// letterboxes it in black where the band's surface should show.
    #[test]
    fn a_picture_asks_for_the_video_s_shape() {
        assert_eq!(
            super::fitted(400.0, 300.0, Some((1920, 1080))),
            (400.0, 225.0)
        );
        assert_eq!(super::fitted(400.0, 100.0, Some((1920, 1080))).1, 100.0);
        assert_eq!(
            super::fitted(400.0, 300.0, Some((500, 500))),
            (300.0, 300.0)
        );
        assert_eq!(super::fitted(400.0, 300.0, None), (400.0, 300.0));
    }

    #[test]
    fn the_pictures_never_take_more_than_half() {
        let l = layout(Door::Window, 1, 20, 200, 2.0, Some((1080, 1920)), 0);
        assert_eq!(l.pictures[0].lines, 10);
        let c = layout(Door::Column, 1, 80, 50, 2.0, Some((1080, 1920)), 0);
        assert!(c.pictures[0].lines <= tokens::space::COLUMN_PICTURES);
    }

    #[test]
    fn followers_share_a_strip_inside_the_same_ceiling() {
        let l = layout(Door::Window, 1, 60, 60, 2.0, Some((1920, 1080)), 2);
        assert_eq!(l.pictures.len(), 3);
        let (main, a, b) = (l.pictures[0], l.pictures[1], l.pictures[2]);
        assert_eq!(a.first, main.first + main.lines);
        assert_eq!((a.first, a.lines), (b.first, b.lines), "one strip");
        assert_eq!((a.share, b.share, a.shares), (0, 1, 2));
        assert!(main.lines + a.lines <= 30, "never past half: {l:?}");
        // AND ON A SHORT SURFACE the strip is still there, at its floor.
        let tight = layout(Door::Window, 1, 24, 60, 2.0, Some((1920, 1080)), 3);
        assert_eq!(tight.pictures.len(), 4);
        assert!(tight.pictures[1].lines >= FOLLOWER_FLOOR);
        assert!(tight.list <= 1 + 12 + 2, "{tight:?}");
    }

    #[test]
    fn a_place_divides_the_width_and_keeps_its_lines() {
        let p = Place {
            first: 2,
            lines: 3,
            share: 1,
            shares: 4,
        };
        let a = p.area(400.0, 20.0);
        assert_eq!((a.x, a.y, a.width, a.height), (100.0, 40.0, 100.0, 60.0));
    }

    #[test]
    fn every_button_asks_the_player_something_and_has_a_command() {
        for id in BUTTONS {
            // THE TWO STAGE BUTTONS ARE THE DOOR'S, not the player's: they
            // change how the picture is shown, and nothing is asked.
            let the_door_s = matches!(id, "player-only" | "player-maximize");
            assert!(ask_of(id).is_some() || the_door_s, "{id}");
            assert_ne!(glyph_of(id, None, Stage::Off), ' ', "{id}");
            assert!(
                crate::leader::default_of().is_ok_and(|(_, r)| r.search(id).is_some()),
                "{id} is not a command of the registry"
            );
        }
    }

    /// FAILS IF: the stage stops being the picture over the whole surface
    /// with the transport on its last two lines — where the controls are
    /// drawn over the picture, and where the pointer keeps them up.
    #[test]
    fn on_stage_the_picture_is_everything_and_the_transport_is_its_foot() {
        let l = super::stage_layout(20);
        assert_eq!(l.pictures.len(), 1);
        assert_eq!((l.pictures[0].first, l.pictures[0].lines), (0, 20));
        assert_eq!((l.transport, l.bar, l.list), (18, 19, 20));
    }
}
