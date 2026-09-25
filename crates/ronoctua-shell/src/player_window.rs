//! The player in a window (`LEADER p w`) and in the column on the right
//! (`LEADER p c`) — two doors onto `crate::player`, one host.
//!
//! What a key does and which rows exist is `player.rs`; the plumbing of a
//! surface is `crate::door`'s, as every window's is. What this holds is the
//! two things only this door has — a transport line of glyphs, and a time
//! bar a hand can drag — and the pictures.
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

use std::{
    cell::{Cell, RefCell},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context as _, Result};
use ronoctua_player::wire::{Ask, Repeat, State};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::FrameCallbackData,
    reexports::calloop::{self, channel},
    seat::{
        keyboard::{KeyEvent, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
    shell::{
        WaylandSurface as _,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurface},
        xdg::{
            XdgShell,
            window::{Window, WindowDecorations},
        },
    },
};
use wayland_client::{QueueHandle, protocol::wl_surface};

use crate::door::{self as base, Canvas, Content, Hand, LEFT_BUTTON, Shell, Ui, Wiring};

/// The door this module is: the plumbing, and the player in it.
type Host = base::Door<Player>;

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.player";

/// The column's layer-shell namespace, so a compositor rule can tell it from
/// the footer.
pub const NAMESPACE: &str = "ronoctua-column";

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
const BUTTONS: [&str; 9] = [
    "player-previous",
    "player-toggle",
    "player-next",
    "player-stop",
    "player-shuffle",
    "player-repeat",
    "player-only",
    "player-pip",
    "player-maximize",
];

/// How quick a second click on the picture has to be to be a DOUBLE click,
/// which puts it on the whole screen and back.
const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(400);

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
    /// Only the picture, in the window made small and floated in the
    /// bottom-right corner — picture-in-picture.
    Small,
}

/// How many frames the corner is asked for while the window is not yet in
/// the compositor's tree.
const CORNER_TRIES: u8 = 20;

/// How long between two asks for the corner.
const CORNER_WAIT: std::time::Duration = std::time::Duration::from_millis(150);

/// How long the controls stay over the picture after the last activity.
const CONTROLS_LINGER: std::time::Duration = std::time::Duration::from_secs(3);

/// Each button's glyph, from the state and how the picture is shown.
fn glyph_of(id: &str, s: Option<&State>, stage: Stage) -> char {
    let playing = s.is_some_and(|s| s.playing);
    match id {
        "player-only" => tokens::icon::PLAYER_ONLY,
        "player-pip" => tokens::icon::PLAYER_PIP,
        "player-mute" if s.is_some_and(|s| s.muted) => tokens::icon::VOLUME_OFF,
        "player-mute" => tokens::icon::VOLUME,
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
        "player-pip" => stage == Stage::Small,
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
        "player-mute" => Ask::Mute,
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
    crate::door::run(|w: &Wiring<'_, Player>| {
        // THE PLAYER'S STATE, on its own thread. A door that opens starts the
        // daemon: the nested loop has no unit, and a door onto nothing is a
        // door that says "not running" for ever.
        let _ = ronoctua_player::client::ensure();
        let (state_sender, states) = channel::channel::<Option<State>>();
        let watch = ronoctua_player::client::watch(move |s| {
            let _ = state_sender.send(s);
        });
        w.handle
            .insert_source(states, |event, (), d: &mut Host| {
                if let channel::Event::Msg(s) = event {
                    d.content.state_arrived(&mut d.ui, s);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the player channel: {e}"))?;

        // THE COLUMN FOLLOWS THE FOCUSED SCREEN. It is born on it; when focus
        // moves to a window on another screen, the column moves there too —
        // it is the screen being worked on that the player sits beside. The
        // events come through the compositor CONTRACT (`crate::daemon`), and
        // the question "which screen?" is asked of it only when focus moved.
        if door == Door::Column {
            let (focus_sender, focus_events) = channel::channel::<ronoctua_compositor::Event>();
            crate::daemon::listen_to_the_compositor(focus_sender);
            w.handle
                .insert_source(focus_events, |event, (), d: &mut Host| {
                    if let channel::Event::Msg(ronoctua_compositor::Event::FocusChanged(_)) = event
                    {
                        follow_the_focus(d);
                    }
                })
                .map_err(|e| anyhow::anyhow!("could not hang the compositor channel: {e}"))?;
        }

        // A PING PER FRAME, from whichever picture's thread has one.
        let (frame_sender, frames) = channel::channel::<usize>();
        w.handle
            .insert_source(frames, |event, (), d: &mut Host| {
                if let channel::Event::Msg(k) = event {
                    d.content.frame_arrived(&mut d.ui, k);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the preview channel: {e}"))?;

        // WHERE IT IS BORN, asked now: the column is made with no screen
        // named, which is the focused one — and the first focus event may
        // already be the move to another.
        let (wm, on_screen) = if door == Door::Column {
            let mut wm = crate::daemon::open_the_compositor().ok();
            let on = wm
                .as_mut()
                .and_then(|c| c.focused_output().ok().flatten())
                .map(|o| o.name);
            (wm, on)
        } else {
            (None, None)
        };

        // THE STAGE IT WAS OPENED ON (`ronoctua-shell player
        // only|maximized|pip`), and the slot that lets the door that asked
        // close it again. The stage itself is set on the first turn, once the
        // window exists to be made whole-screen or small.
        let opened_for = (stage != Stage::Off).then_some(stage);
        if opened_for.is_some() {
            crate::player_stage::hold(match stage {
                Stage::Only | Stage::Off => crate::player::Show::Only,
                Stage::Maximized => crate::player::Show::Maximized,
                Stage::Small => crate::player::Show::Small,
            });
        }

        Ok(Player {
            door,
            handle: w.handle.clone(),
            qh: w.qh.clone(),
            surf: RefCell::new(None),
            told_width: Cell::new(0),
            on_screen,
            wm,
            view: crate::player::View::fresh(),
            player: None,
            pictures: Vec::new(),
            frame_sender,
            buttons: Vec::new(),
            layout: Layout::default(),
            stage: Stage::Off,
            first_stage: opened_for,
            opened_for,
            controls_until: None,
            controls_timer: None,
            settle_owed: door == Door::Column,
            settle_asked: false,
            corner_owed: 0,
            fullscreen: false,
            corner_timer: false,
            painted: false,
            button_under: None,
            pointer_at: None,
            _watch: watch,
        })
    })
}

/// The column's surface, on `output` — or, with none named, on the screen the
/// compositor has focused, which is where a column raised from a key belongs.
fn column_on(
    layer_shell: &LayerShell,
    surface: wl_surface::WlSurface,
    qh: &QueueHandle<Host>,
    output: Option<&wayland_client::protocol::wl_output::WlOutput>,
) -> LayerSurface {
    let layer = layer_shell.create_layer_surface(
        qh,
        surface,
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

/// Ends the column that is up, if one is — whoever holds the right column.
pub fn end_the_column() -> bool {
    crate::right_column::end()
}

/// Whether a column is up.
#[must_use]
pub fn column_is_up() -> bool {
    crate::right_column::holder().is_some()
}

/// The role this door's surface has, kept to act on it: made whole-screen,
/// small, or — the column — told its width again.
///
/// A CLONE of what the door holds (both are handles onto one object), taken
/// when the surface is made and replaced when the column moves.
enum Surf {
    Window(Window),
    Layer(LayerSurface),
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

/// What this door keeps for ONE pointer, past what every door keeps (the
/// list's bar, the wheel's leftover, the row under it) — see `crate::seats`.
#[derive(Debug, Default)]
struct PerPointer {
    /// Dragging the time bar.
    seeking: bool,
    /// Dragging the volume's bar.
    turning: bool,
    /// When the picture was last clicked: a second click soon after is a
    /// double click.
    picture_clicked: Option<std::time::Instant>,
    /// The transport button under the pointer.
    button_under: Option<&'static str>,
}

/// The player, as this door shows it.
struct Player {
    door: Door,
    handle: calloop::LoopHandle<'static, Host>,
    qh: QueueHandle<Host>,
    /// The surface's role, to act on it — see [`Surf`].
    surf: RefCell<Option<Surf>>,
    /// The width the column last told the compositor, so a size of text that
    /// moved tells it again and a configure does not.
    told_width: Cell<u32>,
    /// The screen the column is on, by its connector name, once known.
    on_screen: Option<String>,
    /// The compositor, through the contract — opened the first time it is
    /// needed.
    wm: Option<Box<dyn ronoctua_compositor::Compositor>>,
    view: crate::player::View,
    player: Option<State>,
    pictures: Vec<Picture>,
    frame_sender: channel::Sender<usize>,
    /// The transport as last DRAWN: `(line, first column, last column, id)`
    /// — the volume's glyph is on the time bar's line.
    buttons: Vec<(u16, u16, u16, &'static str)>,
    /// Where everything was last laid out.
    layout: Layout,
    /// How the picture is shown.
    stage: Stage,
    /// The stage to be set on the first turn, once the window exists.
    first_stage: Option<Stage>,
    /// The stage this window was OPENED for, by a door without a window of
    /// its own (`ronoctua-shell player maximized|pip|only`).
    opened_for: Option<Stage>,
    /// On stage, when the controls over the picture go.
    controls_until: Option<std::time::Instant>,
    /// The timer that takes them away, armed only while they are shown.
    controls_timer: Option<calloop::RegistrationToken>,
    /// The column's first frame is owed a frame callback, whose answer means
    /// the commit that mapped it was shown — see
    /// [`Player::settle_the_windows`].
    settle_owed: bool,
    /// That callback was asked, and not yet answered.
    settle_asked: bool,
    /// The window was made small on stage and the compositor has not yet
    /// been told to float it in the corner — a window it has not mapped is
    /// not in its tree to be found.
    corner_owed: u8,
    /// Whether the compositor last configured the window whole-screen — so
    /// leaving it by another way is noticed (`Content::window_configured`).
    fullscreen: bool,
    /// A timer is armed to ask for the corner again.
    corner_timer: bool,
    /// A frame was drawn since the last turn.
    painted: bool,
    /// The transport button under the pointer that spoke last — what the
    /// painter lights, and what line 0 explains.
    button_under: Option<&'static str>,
    /// Where the pointer that spoke last is, while it is over the surface:
    /// between an `Enter` and a `Leave`. A position alone cannot say it —
    /// before the first `Enter` it was `(0, 0)`, which is the header's
    /// corner, and the controls stayed up for good.
    pointer_at: Option<(f64, f64)>,
    /// The thread watching the player's state.
    _watch: Arc<AtomicBool>,
}

impl Drop for Player {
    fn drop(&mut self) {
        for p in &self.pictures {
            p.alive.store(false, Ordering::SeqCst);
        }
        if self.door == Door::Column {
            crate::right_column::leave();
        }
        if self.opened_for.is_some() {
            crate::player_stage::leave();
        }
    }
}

/// Moves the column to the screen that has focus, if that is another one.
///
/// The new surface is made with NO screen named, which the compositor gives
/// to the one it has focused — the same rule the column was born by, and the
/// screen this was just told about.
fn follow_the_focus(d: &mut Host) {
    let p = &mut d.content;
    if p.wm.is_none() {
        p.wm = crate::daemon::open_the_compositor().ok();
    }
    let Some(name) =
        p.wm.as_mut()
            .and_then(|w| w.focused_output().ok().flatten())
            .map(|o| o.name)
    else {
        return;
    };
    if p.on_screen.as_deref() == Some(name.as_str()) {
        return;
    }
    // THE SAME GLOBAL the column was made by, shared (`Door::layer_shell`).
    let Some(layer_shell) = d.layer_shell() else {
        return;
    };
    let surface = d.compositor().create_surface(&d.qh);
    let layer = column_on(&layer_shell, surface, &d.qh, None);
    let p = &mut d.content;
    p.surf.replace(Some(Surf::Layer(layer.clone())));
    p.told_width.set(column_width());
    p.settle_owed = true;
    p.settle_asked = false;
    // A NEW SURFACE IS A NEW GPU SURFACE: everything drawn on the old one is
    // made again on the first configure of this one.
    for pic in &mut p.pictures {
        pic.uploaded = false;
    }
    p.on_screen = Some(name);
    d.reopen(Shell::Layer(layer, layer_shell));
}

impl Player {
    /// Tells the compositor to send the windows the layout it already
    /// worked out — asked once the commit that MAPPED the column is known to
    /// have been shown (the first frame's callback answers only then).
    ///
    /// # Why the windows were not told on their own
    ///
    /// Two lines of the pinned source. A layer's exclusive zone is counted
    /// only once the surface is MAPPED (`wlr_scene_layer_surface_v1_get_box`,
    /// `surface->mapped && exclusive_zone > 0`), so the windows are re-tiled
    /// by the commit that carries the first frame. And a layer's commit sends
    /// the new layout out (`transaction_commit_dirty`) only when it also
    /// changed some layer state (`handle_surface_commit`), which a frame does
    /// not. The column opened over the windows, and they moved at the next
    /// click anywhere — the owner's screenshot, twice.
    ///
    /// The first attempt said the zone again in that commit, and did nothing:
    /// wlroots IGNORES a value set to what it already is
    /// (`layer_surface_handle_set_exclusive_zone`). This one asks over IPC,
    /// where every command ends in `transaction_commit_dirty`
    /// (`Compositor::settle`).
    fn settle_the_windows(&mut self) {
        if self.wm.is_none() {
            self.wm = crate::daemon::open_the_compositor().ok();
        }
        match self.wm.as_mut().map(|w| w.settle()) {
            Some(Ok(())) => tracing::info!("the windows were told the column's room"),
            Some(Err(e)) => {
                tracing::warn!(error = %e, "the windows were not told the column's room");
            }
            None => tracing::warn!("no compositor to tell the column's room to"),
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
    fn pointer_on_controls(&self, ui: &Ui) -> bool {
        let Some((_, y)) = self.pointer_at else {
            return false;
        };
        if y < 0.0 {
            return false;
        }
        let l = line_at(y);
        l <= ui.hinted.rows() || l >= self.layout.transport
    }

    /// Activity: on stage, the controls come back over the picture and a
    /// timer is armed to take them away — one timer, for as long as they
    /// are shown, and none at rest.
    fn wake_controls(&mut self, ui: &mut Ui) {
        if !self.on_stage() {
            return;
        }
        if !self.controls_shown() {
            ui.ask_paint();
        }
        self.controls_until = Some(std::time::Instant::now() + CONTROLS_LINGER);
        if self.controls_timer.is_some() {
            return;
        }
        let armed = self.handle.insert_source(
            calloop::timer::Timer::from_duration(CONTROLS_LINGER),
            |_, (), d: &mut Host| {
                let now = std::time::Instant::now();
                let p = &mut d.content;
                if p.on_stage() && p.pointer_on_controls(&d.ui) {
                    p.controls_until = Some(now + CONTROLS_LINGER);
                }
                match p.controls_until {
                    Some(until) if p.on_stage() && until > now => {
                        calloop::timer::TimeoutAction::ToDuration(until - now)
                    }
                    _ => {
                        p.controls_until = None;
                        p.controls_timer = None;
                        p.button_under = None;
                        d.here.extra.button_under = None;
                        d.ui.ask_paint();
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

    /// Shows the picture alone, on the whole screen, small in the corner, or
    /// back in the door.
    ///
    /// A column is a strip of the screen and can neither fill it nor float:
    /// there, the whole screen and the corner are the WINDOW, opened for it —
    /// and a second press closes that window instead of opening another
    /// (`crate::player_stage`).
    ///
    /// A window OPENED for a stage is that stage and nothing else: leaving
    /// it closes the window, which is the way back to the door that asked.
    fn set_stage(&mut self, ui: &mut Ui, to: Stage) {
        if self.door == Door::Column && matches!(to, Stage::Maximized | Stage::Small) {
            let show = if to == Stage::Small {
                crate::player::Show::Small
            } else {
                crate::player::Show::Maximized
            };
            if let Err(e) = crate::player_stage::toggle(show) {
                tracing::warn!(error = %e, "could not open the player's window for its stage");
            }
            return;
        }
        // LEAVING A STAGE the window was not opened for goes back to the one
        // it was — the small window made whole-screen and back is the small
        // window again.
        let to = match self.opened_for {
            Some(home) if to == Stage::Off && self.stage != home => home,
            Some(_) if to == Stage::Off => {
                ui.quit = true;
                return;
            }
            _ => to,
        };
        let window = match &*self.surf.borrow() {
            Some(Surf::Window(w)) => Some(w.clone()),
            _ => None,
        };
        if let Some(w) = &window {
            if to == Stage::Maximized {
                w.set_fullscreen(None);
            } else if self.stage == Stage::Maximized {
                w.unset_fullscreen();
            }
        }
        // THE CORNER is the compositor's to give: floated, sized and placed
        // there, and put back in the layout on the way out.
        //
        // A FLOATING WINDOW'S SIZE IS THE ONE ITS COMMITS CARRY: the
        // compositor adopts it. The window kept committing the tiled size it
        // had been configured, and the corner came out floated, placed, and
        // 512 by 768 — measured in the VM, however many times the corner was
        // asked again. So on this stage the window has to own its size, and
        // tell it as its minimum and maximum.
        //
        // Both halves: it DRAWS at its own size (`Content::own_size`, which
        // the door lays out again at once — `Ui::retake_the_size`), and tells
        // it as its minimum and maximum.
        let small = Some(small_size());
        if to == Stage::Small && self.stage != Stage::Small {
            if let Some(w) = &window {
                w.set_min_size(small);
                w.set_max_size(small);
            }
            self.corner_owed = CORNER_TRIES;
            self.float_in_the_corner();
        } else if to != Stage::Small && self.stage == Stage::Small {
            if let Some(w) = &window {
                w.set_min_size(None);
                w.set_max_size(None);
            }
            self.corner_owed = 0;
            self.with_my_window(|wm, id| wm.back_in_the_layout(id));
        }
        self.stage = to;
        ui.retake_the_size();
        ui.offset = 0;
        self.controls_until = None;
        self.wake_controls(ui);
        ui.ask_paint();
    }

    /// Asks the compositor for the corner; while it cannot find this window
    /// — not yet mapped — the ask is owed, and tried again on a TIMER.
    ///
    /// Not after the next frame: the frame that maps the window is the last
    /// one a paused picture paints, and the ask made right after it reaches
    /// the compositor before the commit does. With nothing playing no frame
    /// follows, and the window stayed in the layout — the VM caught it.
    fn float_in_the_corner(&mut self) {
        if self.corner_owed == 0 {
            return;
        }
        let corner = ronoctua_compositor::Corner {
            width: u32::from(tokens::space::PLAYER_PIPWIDTH),
            height: u32::from(tokens::space::PLAYER_PIPHEIGHT),
            air: u32::from(tokens::space::PLAYER_PIPAIR),
        };
        if self.with_my_window(|wm, id| wm.float_in_corner(id, corner)) {
            self.corner_owed = 0;
            tracing::info!("the window was floated in the corner");
            return;
        }
        self.corner_owed -= 1;
        if self.corner_owed == 0 {
            // SAID, because a small window that stayed big has no other way
            // of telling anyone why.
            tracing::warn!("the compositor never showed this window to float it in the corner");
            return;
        }
        if self.corner_timer {
            return;
        }
        let armed = self.handle.insert_source(
            calloop::timer::Timer::from_duration(CORNER_WAIT),
            |_, (), d: &mut Host| {
                d.content.corner_timer = false;
                d.content.float_in_the_corner();
                calloop::timer::TimeoutAction::Drop
            },
        );
        self.corner_timer = armed.is_ok();
    }

    /// Runs `ask` on this window, found in the compositor's tree by this
    /// process and the app id; whether it was found and the ask went.
    fn with_my_window(
        &mut self,
        ask: impl FnOnce(&mut dyn ronoctua_compositor::Compositor, u64) -> Result<()>,
    ) -> bool {
        if self.wm.is_none() {
            self.wm = crate::daemon::open_the_compositor().ok();
        }
        let Some(wm) = self.wm.as_mut() else {
            return false;
        };
        let me = i32::try_from(std::process::id()).ok();
        let Some(id) = wm.windows().ok().and_then(|all| {
            all.into_iter()
                .find(|w| w.pid == me && w.app_id.as_deref() == Some(APP_ID))
                .map(|w| w.id)
        }) else {
            return false;
        };
        match ask(wm.as_mut(), id) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(error = %e, "the compositor refused the window's place");
                // A CONNECTION THAT FAILED is opened again next time.
                self.wm = None;
                false
            }
        }
    }

    /// A press of `o`, `i` or `m`, or of their buttons: on, or back off.
    fn toggle_stage(&mut self, ui: &mut Ui, show: crate::player::Show) {
        let to = match show {
            crate::player::Show::Only => Stage::Only,
            crate::player::Show::Maximized => Stage::Maximized,
            crate::player::Show::Small => Stage::Small,
        };
        self.set_stage(ui, if self.stage == to { Stage::Off } else { to });
    }

    /// Where a picture is on the surface: its band, or the whole surface on
    /// stage.
    fn picture_area(&self, ui: &Ui, place: Place) -> ronoctua_render::image::Area {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (ui.width as f32, ui.height as f32);
        if self.on_stage() {
            return ronoctua_render::image::Area {
                x: 0.0,
                y: 0.0,
                width: w,
                height: h,
            };
        }
        place.area(w, line_px())
    }

    fn state_arrived(&mut self, ui: &mut Ui, s: Option<State>) {
        let spot = self
            .rows(ui)
            .get(self.view.cursor())
            .map(|r| r.spot.clone());
        self.player = s;
        if let Some(spot) = spot {
            self.view
                .follow(&spot, self.player.as_ref(), ui.columns, &ui.texts);
        }
        self.view
            .settle_in(self.player.as_ref(), ui.columns, &ui.texts);
        ui.ask_paint();
    }

    fn frame_arrived(&mut self, ui: &mut Ui, k: usize) {
        if let Some(p) = self.pictures.get_mut(k)
            && let Some((w, h, px)) = p.slot.lock().ok().and_then(|mut s| s.take())
        {
            p.frame = ronoctua_render::image::Rgba::new(w, h, px);
            tracing::trace!(k, w, h, taken = p.frame.is_some(), "a frame");
            p.uploaded = false;
            ui.ask_paint();
        }
    }

    fn rows(&self, ui: &Ui) -> Vec<crate::player::Row> {
        self.view.rows(self.player.as_ref(), ui.columns, &ui.texts)
    }

    /// Lays the door out from the state and the size.
    fn relayout(&mut self, ui: &Ui) {
        let video = self.player.as_ref().and_then(|s| s.video);
        let followers = self.player.as_ref().map_or(0, |s| s.participants.len());
        self.layout = if self.on_stage() {
            stage_layout(ui.lines)
        } else {
            layout(
                self.door,
                ui.first_of_list(),
                ui.lines,
                ui.columns + 2 * MARGIN,
                line_px() / cell_px(),
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
            let area = self.picture_area(ui, *place);
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
            let size = ((fw * ui.scale as f32) as u32, (fh * ui.scale as f32) as u32);
            if let Ok(mut w) = p.wanted.lock() {
                *w = size;
            }
        }
    }

    /// How many rows fit under the transport.
    fn room(&self, ui: &Ui) -> usize {
        usize::from(ui.lines.saturating_sub(self.layout.list))
    }

    /// Keeps the cursor on screen, starting to scroll two rows before the
    /// edge — the list here starts under the transport, not under the header,
    /// so this is the door's own and not `Ui::follow`.
    fn follow(&self, ui: &mut Ui, rows: usize) {
        const EDGE: usize = 2;
        let room = self.room(ui);
        if room == 0 {
            return;
        }
        let cursor = self.view.cursor();
        if cursor < ui.offset + EDGE {
            ui.offset = cursor.saturating_sub(EDGE);
        } else if cursor + EDGE >= ui.offset + room {
            ui.offset = cursor + EDGE + 1 - room;
        }
        ui.offset = ui.offset.min(rows.saturating_sub(room));
    }

    fn apply(&mut self, ui: &mut Ui, errand: crate::player::Errand) {
        match crate::player::carry_out(errand, &ui.texts) {
            Ok(said) => ui.message = said,
            Err(crate::player::Errand::Close) => {
                ui.quit = true;
                return;
            }
            Err(crate::player::Errand::Show(show)) => self.toggle_stage(ui, show),
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
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::PLAYER_TYPED_ELSEWHERE));
                }
            }
            Err(_) => {}
        }
        ui.ask_paint();
    }

    fn tell(ui: &mut Ui, ask: &Ask) {
        ui.message = crate::player::tell(ask, &ui.texts);
        ui.ask_paint();
    }

    fn pressed(&mut self, ui: &mut Ui, press: crate::player::Press) {
        ui.message = None;
        let errand = self
            .view
            .key(press, self.player.as_ref(), ui.columns, &ui.texts);
        let rows = self.rows(ui).len();
        self.follow(ui, rows);
        self.apply(ui, errand);
    }

    /// The volume on the time bar's line, in cells: its glyph's first
    /// column, and its track's `(first, end)` — at the right end, so the time
    /// bar keeps the width it reads by.
    fn volume_cells(ui: &Ui) -> (u16, (u16, u16)) {
        let end = MARGIN + ui.columns;
        let first = end.saturating_sub(tokens::space::PLAYER_VOLUMEBAR);
        (first.saturating_sub(3), (first, end))
    }

    /// The time bar's track, in logical pixels: `(left, right)` — up to a
    /// cell of air before the volume's glyph.
    fn bar_track(ui: &Ui) -> (f32, f32) {
        let cell = cell_px();
        let left = cell * f32::from(MARGIN);
        let right = cell * f32::from(Self::volume_cells(ui).0.saturating_sub(1));
        (left, right.max(left))
    }

    /// The volume's track, in logical pixels: `(left, right)`.
    fn volume_track(ui: &Ui) -> (f32, f32) {
        let cell = cell_px();
        let (from, to) = Self::volume_cells(ui).1;
        (cell * f32::from(from), cell * f32::from(to))
    }

    /// The volume at `x` on its track, in percent.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn volume_at(ui: &Ui, x: f64) -> u8 {
        let (l, r) = Self::volume_track(ui);
        let f = ((x as f32 - l) / (r - l).max(1.0)).clamp(0.0, 1.0);
        (f * f32::from(crate::player::VOLUME_MOST)).round() as u8
    }

    /// Whether `(x, y)` is on the volume's track.
    fn on_the_volume(&self, ui: &Ui, x: f64, y: f64) -> bool {
        let (l, r) = Self::volume_track(ui);
        #[allow(clippy::cast_possible_truncation)]
        let x = x as f32;
        line_at(y) == self.layout.bar && x >= l && x < r
    }

    /// Where on the time bar `x` is, as a fraction.
    #[allow(clippy::cast_possible_truncation)]
    fn fraction_at(ui: &Ui, x: f64) -> f64 {
        let (l, r) = Self::bar_track(ui);
        ((x as f32 - l) / (r - l).max(1.0)).clamp(0.0, 1.0).into()
    }

    /// The row of the list at `(x, y)`.
    ///
    /// `None` above the list — and ON THE BAR'S COLUMN while a bar is drawn
    /// (`ui.room`, written where the bar is painted): the bar has its own
    /// cells, and a pointer on them is on the bar and not on the row beside
    /// it. Without a bar the row answers across the whole width.
    fn row_at(&self, ui: &Ui, x: f64, y: f64) -> Option<usize> {
        #[allow(clippy::cast_possible_truncation)]
        if ui.room.on_the_bar(x as f32) {
            return None;
        }
        let l = line_at(y);
        (l >= self.layout.list).then(|| usize::from(l - self.layout.list) + ui.offset)
    }

    /// The transport button at `(x, y)`, where it was DRAWN.
    fn button_at(&self, x: f64, y: f64) -> Option<&'static str> {
        let (l, c) = (line_at(y), column_at(x));
        self.buttons
            .iter()
            .find(|(line, a, b, _)| l == *line && c >= *a && c < *b)
            .map(|(_, _, _, id)| *id)
    }

    /// Whether `y` is on a picture.
    fn on_a_picture(&self, y: f64) -> bool {
        let l = line_at(y);
        self.layout
            .pictures
            .iter()
            .any(|p| l >= p.first && l < p.first + p.lines)
    }

    fn seek_to(&self, ui: &mut Ui, fraction: f64) {
        if let Some(d) = self.player.as_ref().and_then(|s| s.duration) {
            Self::tell(
                ui,
                &Ask::Seek {
                    seconds: d * fraction,
                    relative: false,
                },
            );
        }
    }

    #[allow(clippy::too_many_lines)]
    fn pointer_here(&mut self, ui: &mut Ui, hand: &mut Hand<PerPointer>, events: &[PointerEvent]) {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (ui.width as f32, ui.height as f32);
        let head = ui.frame(0);
        let before = (hand.row_under_pointer, hand.extra.button_under);
        let total = self.rows(ui).len();
        let scroll = crate::chrome::list_scroll(total, self.room(ui), ui.offset);
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (x, y) = (e.position.0 as f32, e.position.1 as f32);
            // ACTIVITY IS ANY OF IT but leaving: on stage the controls come
            // back over the picture.
            if !matches!(e.kind, PointerEventKind::Leave { .. }) {
                self.pointer_at = Some(e.position);
                self.wake_controls(ui);
            }
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if ui.hinted.dragged(head, e.position.1) {
                        ui.ask_paint();
                        continue;
                    }
                    let now = ui
                        .hinted
                        .under(e.position.0, e.position.1)
                        .and_then(|id| crate::chrome::hint_sentence(id, &ui.texts));
                    if ui.hinted.say(now) {
                        ui.ask_paint();
                    }
                    if hand.dragging {
                        ui.offset =
                            crate::chrome::list_offset_at(scroll, self.layout.list, w, h, y);
                        ui.ask_paint();
                        continue;
                    }
                    if hand.extra.seeking {
                        let f = Self::fraction_at(ui, e.position.0);
                        self.seek_to(ui, f);
                        continue;
                    }
                    if hand.extra.turning {
                        let percent = Self::volume_at(ui, e.position.0);
                        Self::tell(ui, &Ask::Volume { percent });
                        continue;
                    }
                    hand.row_under_pointer = self.row_at(ui, e.position.0, e.position.1);
                    hand.extra.button_under = self.button_at(e.position.0, e.position.1);
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_at = None;
                    ui.hinted.let_go();
                    if ui.hinted.say(None) {
                        ui.ask_paint();
                    }
                    hand.dragging = false;
                    hand.extra.seeking = false;
                    hand.extra.turning = false;
                    hand.row_under_pointer = None;
                    hand.extra.button_under = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    ui.hinted.let_go();
                    hand.dragging = false;
                    hand.extra.seeking = false;
                    hand.extra.turning = false;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if ui.hinted.pressed(head, e.position.0, e.position.1) {
                        ui.ask_paint();
                        continue;
                    }
                    if crate::chrome::on_the_list_bar(scroll, self.layout.list, w, h, x, y) {
                        hand.dragging = true;
                        ui.offset =
                            crate::chrome::list_offset_at(scroll, self.layout.list, w, h, y);
                        ui.ask_paint();
                        continue;
                    }
                    // A BUTTON of the transport: what its key does.
                    if let Some(id) = self.button_at(e.position.0, e.position.1) {
                        if let Some(ask) = ask_of(id) {
                            Self::tell(ui, &ask);
                        }
                        match id {
                            "player-only" => self.toggle_stage(ui, crate::player::Show::Only),
                            "player-pip" => self.toggle_stage(ui, crate::player::Show::Small),
                            "player-maximize" => {
                                self.toggle_stage(ui, crate::player::Show::Maximized);
                            }
                            _ => {}
                        }
                        continue;
                    }
                    // THE VOLUME'S TRACK: a press sets it there, and holding
                    // drags — the time bar's gesture, on its own track.
                    if self.controls_shown() && self.on_the_volume(ui, e.position.0, e.position.1) {
                        hand.extra.turning = true;
                        let percent = Self::volume_at(ui, e.position.0);
                        Self::tell(ui, &Ask::Volume { percent });
                        continue;
                    }
                    // THE TIME BAR: a press seeks there, and holding drags.
                    if line_at(e.position.1) == self.layout.bar && self.controls_shown() {
                        hand.extra.seeking = true;
                        let f = Self::fraction_at(ui, e.position.0);
                        self.seek_to(ui, f);
                        continue;
                    }
                    // A PICTURE: a click plays or pauses, as on every player;
                    // and a DOUBLE click puts it on the whole screen, or back.
                    // The first click has already paused it by then, so the
                    // second plays it again — what a double click leaves is
                    // the screen changed and the film as it was.
                    if self.on_a_picture(e.position.1) {
                        Self::tell(ui, &Ask::Toggle);
                        let now = std::time::Instant::now();
                        let double = hand
                            .extra
                            .picture_clicked
                            .is_some_and(|then| now.duration_since(then) <= DOUBLE_CLICK);
                        if double {
                            hand.extra.picture_clicked = None;
                            self.toggle_stage(ui, crate::player::Show::Maximized);
                        } else {
                            hand.extra.picture_clicked = Some(now);
                        }
                        continue;
                    }
                    // A ROW: the first click selects, the second acts.
                    if let Some(row) = self.row_at(ui, e.position.0, e.position.1) {
                        let rows = self.rows(ui);
                        // BY ITS COLUMN too: the files are a grid, and a title
                        // can carry a control at the end of its line.
                        let column = column_at(e.position.0).saturating_sub(MARGIN);
                        let again = self.view.clicked_at(row, column, &rows);
                        let n = rows.len();
                        self.follow(ui, n);
                        if again {
                            self.pressed(ui, crate::player::Press::Enter);
                        } else {
                            ui.ask_paint();
                        }
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::surface::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
                    let by = crate::surface::wheel_lines(&mut hand.lines_owed);
                    if by == 0 {
                        continue;
                    }
                    if ui.hinted.turned(head, e.position.1, by) {
                        ui.ask_paint();
                        continue;
                    }
                    // OVER THE PICTURES AND THE TRANSPORT THE WHEEL IS THE
                    // VOLUME — the gesture belongs to what is under the hand,
                    // and there is no list there to walk.
                    if line_at(e.position.1) < self.layout.list {
                        let now = self.player.as_ref().map_or(100, |s| s.volume);
                        let step = i64::from(crate::player::VOLUME_STEP);
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let percent = (i64::from(now) - by * step)
                            .clamp(0, i64::from(crate::player::VOLUME_MOST))
                            as u8;
                        Self::tell(ui, &Ask::Volume { percent });
                        continue;
                    }
                    let last = total.saturating_sub(self.room(ui));
                    ui.offset = usize::try_from(
                        i64::try_from(ui.offset)
                            .unwrap_or(0)
                            .saturating_add(by)
                            .max(0),
                    )
                    .unwrap_or(0)
                    .min(last);
                    ui.ask_paint();
                }
                _ => {}
            }
        }
        if before != (hand.row_under_pointer, hand.extra.button_under) {
            ui.ask_paint();
        }
    }
}

fn line_px() -> f32 {
    tokens::metrics::line_height(tokens::text::size())
}

fn cell_px() -> f32 {
    tokens::metrics::cell_width(tokens::text::size())
}

fn line_at(y: f64) -> u16 {
    let line = f64::from(line_px()).max(1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let l = (y.max(0.0) / line) as u16;
    l
}

fn column_at(x: f64) -> u16 {
    let cell = f64::from(cell_px()).max(1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let c = (x.max(0.0) / cell) as u16;
    c
}

/// The small window's size: `space.player.pip*`, in logical pixels.
fn small_size() -> (u32, u32) {
    (
        u32::from(tokens::space::PLAYER_PIPWIDTH),
        u32::from(tokens::space::PLAYER_PIPHEIGHT),
    )
}

impl Content for Player {
    /// The size before the compositor tiles it: tall enough for the picture
    /// and the transport under it.
    fn default_size(&self) -> (u32, u32) {
        (760, 720)
    }

    /// THE CORNER'S SIZE IS THE WINDOW'S OWN: a floating window's size is the
    /// one its commits carry, and committing the tiled size it had been
    /// configured made the corner come out 512 by 768 — measured in the VM.
    fn own_size(&self) -> Option<(u32, u32)> {
        (self.stage == Stage::Small).then(small_size)
    }

    /// THE SCREEN LEFT BY ANOTHER WAY — the compositor's own key, another
    /// window taking it — takes the stage with it, or the door would sit on
    /// "whole screen" in a window.
    fn window_configured(
        &mut self,
        _: &mut Ui,
        cfg: &smithay_client_toolkit::shell::xdg::window::WindowConfigure,
    ) {
        let now = cfg.is_fullscreen();
        if self.fullscreen && !now && self.stage == Stage::Maximized {
            self.stage = Stage::Off;
            self.controls_until = None;
        }
        self.fullscreen = now;
    }

    type Hand = PerPointer;

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t(ronoctua_i18n::key::PLAYER_TITLE)
    }

    /// A window, or the column: an `xdg_toplevel`, or a layer surface
    /// anchored to the top, the right and the bottom.
    fn open(&self, wiring: &Wiring<'_, Self>, surface: wl_surface::WlSurface) -> Result<Shell> {
        match self.door {
            Door::Window => {
                let xdg = XdgShell::bind(wiring.globals, wiring.qh)
                    .context("the compositor does not expose xdg_shell")?;
                let window =
                    xdg.create_window(surface, WindowDecorations::RequestServer, wiring.qh);
                window.set_title(self.title(&wiring.texts));
                window.set_app_id(APP_ID);
                window.commit();
                self.surf.replace(Some(Surf::Window(window.clone())));
                Ok(Shell::Window {
                    window,
                    _global: xdg,
                })
            }
            Door::Column => {
                let layer_shell = base::layer_shell(wiring)?;
                // ONE COLUMN: the one already there goes.
                crate::right_column::take(crate::right_column::Tenant::Player);
                let layer = column_on(&layer_shell, surface, wiring.qh, None);
                self.told_width.set(column_width());
                self.surf.replace(Some(Surf::Layer(layer.clone())));
                Ok(Shell::Layer(layer, layer_shell))
            }
        }
    }

    fn margin(&self) -> u16 {
        MARGIN
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, _: Modifiers) {
        self.wake_controls(ui);
        // `Esc` ON STAGE brings the door back first — one level, as every
        // view with levels has it — and only then reaches the view.
        if event.keysym == Keysym::Escape && self.on_stage() {
            self.set_stage(ui, Stage::Off);
            return;
        }
        let notation = match event.keysym {
            // ESCAPE GOES BACK ONE LEVEL, and closes the door from the top:
            // the view answers which.
            Keysym::Escape => {
                self.pressed(ui, crate::player::Press::Escape);
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
            self.pressed(ui, press);
        }
    }

    /// The column tells the compositor its width again when the size of the
    /// text moved it — and every size is SAID.
    fn resized(&mut self, ui: &mut Ui) {
        // SAID, because a size the compositor computed and never SENT is the
        // defect `settle_the_windows` exists for, and only this line tells
        // the two apart: the compositor's own tree shows the new size either
        // way.
        tracing::info!(width = ui.width, height = ui.height, door = ?self.door, "configured");
        if self.door != Door::Column {
            return;
        }
        let w = column_width();
        if w == self.told_width.get() {
            return;
        }
        if let Some(Surf::Layer(l)) = &*self.surf.borrow() {
            l.set_size(w, 0);
            l.set_exclusive_zone(i32::try_from(w).unwrap_or(i32::MAX));
            l.commit();
        }
        self.told_width.set(w);
    }

    fn each_turn(&mut self, ui: &mut Ui) {
        if let Some(stage) = self.first_stage.take() {
            self.set_stage(ui, stage);
        }
        // AFTER A FRAME: the window is in the compositor's tree only once a
        // buffer mapped it.
        if std::mem::take(&mut self.painted) {
            self.float_in_the_corner();
        }
    }

    fn frame_done(&mut self, _: &mut Ui) {
        if std::mem::take(&mut self.settle_asked) {
            self.settle_the_windows();
        }
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<PerPointer>, events: &[PointerEvent]) {
        ui.hinted.swap_hand(&mut hand.hint_drag);
        self.pointer_here(ui, hand, events);
        ui.hinted.swap_hand(&mut hand.hint_drag);
        self.button_under = hand.extra.button_under;
    }

    #[allow(clippy::too_many_lines)]
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        if ui.message.is_some() {
            ui.hinted.say(None);
        }
        self.relayout(ui);
        let size = tokens::text::size();
        let line = line_px();
        let rows = self.rows(ui);
        let room = self.room(ui);
        ui.offset = ui.offset.min(rows.len().saturating_sub(room));
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (ui.width as f32, ui.height as f32);
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

        canvas.under.push(ronoctua_render::quad::Quad {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
            color: tokens::color::SURFACE_CHROME,
        });

        // LINE 0: the keys, what the last order said, or — with the pointer on
        // a button — what that button does.
        let under_button = self.button_under;
        let (pieces, hinting) = match (&ui.message, under_button) {
            (Some(t), _) => (vec![crate::chrome::Piece::said(t.clone())], false),
            (None, Some(id)) => (
                vec![crate::chrome::Piece::said(
                    crate::chrome::hint_sentence(id, &ui.texts).unwrap_or_default(),
                )],
                false,
            ),
            (None, None) => (
                self.view.hint(self.player.as_ref(), ui.columns, &ui.texts),
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
            canvas.spans.extend(
                ui.hinted
                    .head(&pieces, hinting, MARGIN, ui.columns, &ui.texts),
            );
        } else {
            // HIDDEN CONTROLS ARE NOT CONTROLS, the header's shortcuts
            // included: a click on the picture's top line is on the picture.
            ui.hinted.took(Vec::new());
        }
        if stage && shown {
            let header = line * f32::from(1 + ui.hinted.rows());
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
        for (p, place) in self.pictures.iter_mut().zip(&self.layout.pictures) {
            let band = {
                #[allow(clippy::cast_precision_loss)]
                let (w, h) = (ui.width as f32, ui.height as f32);
                if stage {
                    ronoctua_render::image::Area {
                        x: 0.0,
                        y: 0.0,
                        width: w,
                        height: h,
                    }
                } else {
                    place.area(w, line)
                }
            };
            canvas.under.push(ronoctua_render::quad::Quad {
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
                // A NEW FRAME UNDER THE SAME KEY is sent again: the key is the
                // picture, and what is in it moves every frame.
                let key = p.key();
                if !p.uploaded || canvas.images.missing(&key) {
                    canvas
                        .images
                        .upload_it(&canvas.gpu.device, &canvas.gpu.queue, &key, f);
                    p.uploaded = true;
                }
                // THE FRAME IS ALREADY THE BAND'S SHAPE (the player rendered it
                // at this size, letterboxed); `picture_in` only keeps it inside
                // the band while a new size is on its way.
                let fit =
                    crate::camera::picture_in(band.width, band.height, (f.width, f.height), false);
                canvas.pictures.push((
                    key,
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
            self.buttons.push((tl, at, at + 2, id));
            canvas.spans.push(span(
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
            let right = MARGIN + ui.columns;
            let from = right.saturating_sub(cells).max(at + 1);
            // WHERE IT FITS: the small window in the corner is narrower than
            // the buttons and the clock, and a clock cut at the edge reads as
            // a wrong time.
            if from + cells <= right {
                canvas
                    .spans
                    .push(span(clock, from, tl, tokens::color::TEXT_SECONDARY));
            }
        }

        // THE TIME BAR: a rule, and the part already played in the accent.
        // Under the pictures in the door; over the picture on stage.
        let mut bar: Vec<ronoctua_render::quad::Quad> = Vec::new();
        let (l, r) = Self::bar_track(ui);
        let thickness = f32::from(tokens::space::RULE_THICKNESS).max(1.0) * 3.0;
        let y = line * f32::from(self.layout.bar) + (line - thickness) / 2.0;
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

        // THE VOLUME, at the end of the same line: its glyph — a button that
        // mutes — and a track of its own, filled to the volume and dimmed
        // while muted. Hidden with the rest of the controls.
        if shown {
            let (glyph, _) = Self::volume_cells(ui);
            let bl = self.layout.bar;
            self.buttons.push((bl, glyph, glyph + 2, "player-mute"));
            let colour = if under_button == Some("player-mute") {
                tokens::color::STATE_ACCENT
            } else {
                tokens::color::TEXT_PRIMARY
            };
            canvas.spans.push(span(
                glyph_of("player-mute", s, self.stage).to_string(),
                glyph,
                bl,
                colour,
            ));
            let (vl, vr) = Self::volume_track(ui);
            bar.push(ronoctua_render::quad::Quad {
                x: vl,
                y,
                width: vr - vl,
                height: thickness,
                color: tokens::color::RULE_DEFAULT,
            });
            if let Some(s) = s {
                let filled = f32::from(s.volume.min(crate::player::VOLUME_MOST))
                    / f32::from(crate::player::VOLUME_MOST);
                bar.push(ronoctua_render::quad::Quad {
                    x: vl,
                    y,
                    width: (vr - vl) * filled,
                    height: thickness,
                    color: if s.muted {
                        tokens::color::TEXT_DIM
                    } else {
                        tokens::color::STATE_ACCENT
                    },
                });
            }
        }
        if stage {
            top.extend(bar);
        } else {
            canvas.under.extend(bar);
        }

        // THE ROWS, under all of it, scrolling.
        let first = self.layout.list;
        for (i, row) in rows.iter().skip(ui.offset).take(room).enumerate() {
            let y = first + u16::try_from(i).unwrap_or(0);
            for f in &row.fields {
                canvas
                    .spans
                    .push(span(f.text.clone(), MARGIN + f.at, y, f.color));
            }
        }
        // THE BAR'S COLUMN IS ITS OWN WHILE IT IS DRAWN, and only then —
        // written down here, where it is drawn, for the pointer to read. On
        // stage there is no list and no bar.
        let scroll = crate::chrome::list_scroll(rows.len(), room, ui.offset);
        ui.room = if stage {
            crate::chrome::Room::default()
        } else {
            crate::chrome::Room::of(scroll, w, h)
        };
        let bands = crate::chrome::hover_bands(
            canvas.lit.iter().copied(),
            ui.offset,
            room,
            rows.len(),
            first,
            ui.room.band_width(),
        );
        // UNDER THE PICTURES the grounds and the time bar; OVER them the bands
        // and the bars; the text last. No depth test: later is on top.
        if !stage || shown {
            canvas.over.extend(top);
        }
        canvas.over.extend(bands);
        if !stage {
            canvas
                .over
                .extend(crate::chrome::list_bar(scroll, first, w, h));
            canvas.bar(crate::chrome::list_track(scroll, first, w, h));
        }
        if shown {
            canvas.over.extend(ui.hinted.bar(ui.frame(0)));
        }

        // THE FRAME THAT MAPS THE COLUMN asks to be told when it is SHOWN:
        // the answer can only come after the compositor read that commit and
        // counted the zone, and only then is there a layout to send. A `sync`
        // after presenting was the second attempt, and it raced: the driver
        // may commit the frame on a thread of its own, so the `sync` could
        // arrive first — measured in the VM, the window told 512 again. The
        // callback is asked here, before the commit that carries this frame.
        if std::mem::take(&mut self.settle_owed) {
            let _ = ui
                .surface
                .frame(&self.qh, FrameCallbackData(ui.surface.clone()));
            self.settle_asked = true;
        }
        self.painted = true;
    }
}

#[cfg(test)]
mod tests {

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
        // THE VOLUME'S GLYPH is a button too, on the time bar's line.
        for id in BUTTONS.into_iter().chain(["player-mute"]) {
            // THE THREE STAGE BUTTONS ARE THE DOOR'S, not the player's: they
            // change how the picture is shown, and nothing is asked.
            let the_door_s = matches!(id, "player-only" | "player-pip" | "player-maximize");
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
