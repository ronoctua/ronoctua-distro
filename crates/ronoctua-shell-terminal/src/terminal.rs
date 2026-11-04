//! A terminal in an `xdg_toplevel` of its own — the VIEWER of terminals that
//! live elsewhere.
//!
//! # Where each piece lives
//!
//! ```text
//!   keyboard, pointer (Wayland) -> this window -> frames -> holder -> PTY -> program
//!                                                            |
//!   screen (wgpu) <- cells <- render state <- mirror <- bytes, resizes, modes
//! ```
//!
//! The terminal itself is the holder's (`crate::terminal_link`): the PTY, the
//! program, the engine every answer to the program comes from. The window
//! keeps a MIRROR of it — decoded from a snapshot, then fed the very bytes the
//! holder's engine was fed — and draws the mirror as a grid of cells
//! (`Content::sheet`, `ronoctua_render::cells`). It says what the person did
//! (a key, a click, a paste, a file dropped) and never writes to the program
//! itself.
//!
//! # A window holds a group
//!
//! One terminal or several, one of them in front (`crate::terminal_group`):
//! `C-T` starts a new one right after the one in front, `C-TAB` and
//! `C-S-TAB` walk them, `C-S-RET` sends the one in front to a window of its
//! own, and from two up a thin line each across the top tells them apart
//! (`crate::terminal_strip`, the owner's exception to invariant 4). Every
//! terminal of the group keeps its mirror fed — a terminal brought to the
//! front is shown at once, its history, selection and place in it as they
//! were — and only the one in front is drawn. A terminal whose program ends
//! leaves the group, and the last one closes the window; closing the window
//! sends every terminal to the background, each holder learning it on its
//! own as its window's connection closes.
//!
//! # Why the window is an `xdg_toplevel`
//!
//! Invariant 4: the shell never reimplements window layout. A terminal is a
//! window, and whoever positions it is the compositor; the group is one
//! window showing one terminal at a time, and splitting stays the
//! compositor's.
//!
//! # Three identities, one viewer
//!
//! The terminal (`ronoctua-shell terminal`), the monitor (`monitor`, floated
//! by its `app_id`) and the rescue console (`rescue`) are the same window;
//! what changes is the identity the compositor reads, and the last two keep
//! their terminal in their own process (they must work without the session's
//! scopes), as `terminal --here` does. Only the terminal holds a group: a
//! monitor is one program in one floating window, and its keys are all the
//! program's.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use crate::daemon::CHROME_APP_PREFIX;
use crate::door::text_input::{Field, LogicalRect, Preedit, Purpose};
use crate::door::{Canvas, Content, Door, Hand, LEFT_BUTTON, Shell, Ui};
use crate::terminal_ask::{self, Asked, Click, Question, Run, Step};
use crate::terminal_group::{self, Group, Held, Left, Pending};
use crate::terminal_hyperlink::{self, Link};
use crate::terminal_keys::{self, Grouped, Kept, Leaf, Zoom};
use crate::terminal_link::{self, Arrival, Heard, Link as Attachment, Mirrored, Plan, Reaching};
use crate::terminal_pointer::{self, MIDDLE_BUTTON, Placed, Wheel};
use crate::terminal_search::{self, Finding};
use crate::terminal_sheet::{self, Palette};
use crate::terminal_strip::{self, Strip};
use anyhow::{Context as _, Result};
use ronoctua_render::cells::{self, Cell, Geometry, Grapheme, Rgba, Sheet, Width};
use ronoctua_shell_kit::chrome;
use ronoctua_term::ghostty::{self, Mode, Scroll};
use ronoctua_terminals::state::{Attention, Progress, TerminalState};
use ronoctua_terminals::wire::{self, ToHolder};
use ronoctua_terminals::{GroupId, PROTOCOL, TerminalId};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::data_device_manager::data_device::DataDeviceData;
use smithay_client_toolkit::data_device_manager::data_offer::DragOffer;
use smithay_client_toolkit::reexports::calloop;
use smithay_client_toolkit::reexports::client::protocol::wl_data_device_manager::DndAction;
use smithay_client_toolkit::seat::{
    keyboard::{KeyEvent, Keysym, Modifiers},
    pointer::{PointerEvent, PointerEventKind},
};

/// The terminal window's `app_id`, owned beside the table of the windows
/// that open floating (`crate::floating`) — with no row: it tiles.
pub const APP_ID: &str = crate::floating::APP_ID_TERMINAL;

/// The monitor's `app_id` — the same terminal, floated by its row of the
/// table of the windows that open floating (`crate::floating`).
pub const APP_ID_MONITOR: &str = crate::floating::APP_ID_MONITOR;

/// The rescue console's `app_id` — the same terminal, floated wider by its
/// row of the table (`crate::floating`).
pub const APP_ID_RESCUE: &str = crate::floating::APP_ID_RESCUE;

/// A POINT IN LOGICAL PIXELS: a point is 1/72 of an inch, and Ghostty on
/// Linux counts 96 pixels an inch (its `default_dpi`) — the arithmetic
/// Ghostty does with `font-size`, done here with the same
/// `typography.terminalPoints`, so the two terminals draw a program at the
/// same size and the swap between them (`RET RET`, `RET /`) changes nothing
/// the eye sees. A unit's conversion, not a size chosen.
const PIXELS_PER_POINT: f32 = 96.0 / 72.0;

/// HOW LONG A BLINKING CURSOR IS SHOWN, and then hidden: Ghostty's cadence
/// (600 ms), so a cursor blinks alike in both terminals.
///
/// A TIMING AND NOT A TOKEN: invariant 9 is about what a thing looks like —
/// a colour, a size, a codepoint — and this is how often a program's own
/// cursor is redrawn, the same kind of number as the double click's below.
/// It is the other terminal's, and it moves when that one's does.
const BLINK: std::time::Duration = std::time::Duration::from_millis(600);

/// HOW LONG THE SCROLLBAR STAYS after the last thing that showed it — a
/// wheel, a page key, a press on it, the pointer leaving its edge: GTK's
/// overlay scrollbar's own pause, which Ghostty's bar keeps, so the bar comes
/// and goes alike in both terminals.
///
/// A TIMING AND NOT A TOKEN, as the blink is: it is how long a thing that was
/// used stays at hand, not what anything looks like.
const BAR_SHOWN: std::time::Duration = std::time::Duration::from_millis(1000);

/// THE LONGEST BETWEEN TWO CLICKS OF A DOUBLE CLICK: GTK's own default
/// (`gtk-double-click-time`), which Ghostty takes when it is told none —
/// what every other program on the desktop counts a double click by.
///
/// A TIMING AND NOT A TOKEN, as the blink is: it is how a hand is read, not
/// how anything is drawn, and it is the desktop's number — a token of ours
/// would be a second answer to a question GTK already answers.
const CLICK_INTERVAL_NS: u64 = 400_000_000;

/// The opener of a link, by path (`RONOCTUA_XDG_OPEN`, `nix/rust.nix`) —
/// what every door opens a page with.
const XDG_OPEN: Option<&str> = option_env!("RONOCTUA_XDG_OPEN");

/// THE FAINT INK'S ALPHA (SGR 2), out of 255: `space.terminal.faintPct` of
/// a whole ink — Ghostty's `faint-opacity`, a half by default — rounded,
/// and never past a whole one.
fn faint_alpha() -> u8 {
    let percent = u32::from(tokens::space::TERMINAL_FAINTPCT.min(100));
    u8::try_from((255 * percent + 50) / 100).unwrap_or(u8::MAX)
}

/// Every window rule the compositor takes from the shell — what
/// `export-window-rules` prints.
///
/// The floating ones and the inhibitor's, one text: the two configs include
/// one file, and a second export would be a second thing each of them has to
/// remember to include.
#[must_use]
pub fn window_rules() -> String {
    crate::floating::rules() + &inhibitor_rules()
}

/// The rule that keeps a web app from swallowing the leader.
///
/// # What it stops
///
/// A Wayland client can ask for `zwp_keyboard_shortcuts_inhibitor_v1`, and
/// while it holds one the compositor hands it EVERY key, its own bindings
/// included. It is meant for a virtual machine or a remote desktop; Chrome's
/// `--app=` windows were reported asking for it, and this compositor grants it
/// unless told otherwise. Granted, the leader (`Super`, or `Ctrl+Alt+Space` in
/// the nested loop) goes to the page, and so does every other binding the
/// compositor holds — and the leader is how everything else here is reached,
/// so a web app in focus would be a screen with no way out but the mouse.
///
/// `disable` for those windows only: a virtual machine opened in a window of
/// its own keeps the inhibitor it asks for.
///
/// No `$`: the rest of the `app_id` is the site's address and the profile's
/// name, and the prefix is the whole of what identifies an app window.
#[must_use]
pub fn inhibitor_rules() -> String {
    format!("for_window [app_id=\"^{CHROME_APP_PREFIX}\"] shortcuts_inhibitor disable\n")
}

/// Which of the three windows this terminal is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    /// The terminal.
    Terminal,
    /// The monitor's floating one.
    Monitor,
    /// The rescue console.
    Rescue,
}

impl Identity {
    /// The `app_id` the compositor reads.
    #[must_use]
    pub const fn app_id(self) -> &'static str {
        match self {
            Self::Terminal => APP_ID,
            Self::Monitor => APP_ID_MONITOR,
            Self::Rescue => APP_ID_RESCUE,
        }
    }

    /// Its title until the program names the window.
    const fn title_key(self) -> &'static str {
        match self {
            Self::Terminal => crate::key::TERMINAL_TITLE,
            Self::Monitor => crate::key::TERMINAL_TITLE_MONITOR,
            Self::Rescue => crate::key::TERMINAL_TITLE_RESCUE,
        }
    }
}

/// `terminal [--cwd D] [--here] [--] [program args…]` and `terminal --show
/// ID[,ID…] [--front ID]`, read.
///
/// Flags come before the program: the first word that is none is the
/// program, and everything after it is its own — `terminal htop -d 5` runs
/// `htop -d 5`, and `--` ends the flags for a program whose name could be
/// taken for one.
///
/// # Errors
///
/// A flag with no value, an id that is not one, `--show` with a program, or
/// a flag this door does not know.
pub fn plan_of(words: &[String]) -> Result<Plan> {
    let mut words = words.iter();
    let (mut cwd, mut here, mut show, mut front) = (None, false, None::<Vec<TerminalId>>, None);
    let mut program: Option<String> = None;
    let mut args = Vec::new();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--" => {
                program = words.next().cloned();
                break;
            }
            "--cwd" => {
                cwd = Some(PathBuf::from(
                    words.next().context("`--cwd` with no folder")?,
                ))
            }
            "--here" => here = true,
            "--show" => {
                let ids = words.next().context("`--show` with no terminal")?;
                show = Some(
                    ids.split(',')
                        .map(|id| id.parse().with_context(|| format!("`--show {ids}`")))
                        .collect::<Result<_>>()?,
                );
            }
            "--front" => {
                let id = words.next().context("`--front` with no terminal")?;
                front = Some(
                    id.parse::<TerminalId>()
                        .with_context(|| format!("`--front {id}`"))?,
                );
            }
            flag if flag.starts_with("--") => {
                anyhow::bail!(
                    "`{flag}` is no word of `terminal`: [--cwd D] [--here] [--] [program args] or --show ID[,ID] [--front ID]"
                )
            }
            _ => {
                program = Some(word.clone());
                break;
            }
        }
    }
    args.extend(words.cloned());
    if let Some(show) = show {
        anyhow::ensure!(
            program.is_none() && cwd.is_none() && !here,
            "`--show` shows terminals that run already: it takes no program, `--cwd` or `--here`"
        );
        let front = match front {
            Some(f) => {
                anyhow::ensure!(show.contains(&f), "`--front {f}` is not one of `--show`");
                f
            }
            None => show.first().cloned().context("`--show` with no terminal")?,
        };
        return Ok(Plan::Show { show, front });
    }
    anyhow::ensure!(front.is_none(), "`--front` goes with `--show`");
    Ok(Plan::New {
        program: program.unwrap_or_else(ronoctua_term::pty::default_program),
        args,
        cwd: cwd.or_else(|| std::env::current_dir().ok()),
        here,
    })
}

/// Opens a terminal in a window of its own and runs until it closes.
///
/// # Errors
///
/// There is no compositor, or the window could not be put together. A
/// terminal that cannot start says so on the window's line, and the window
/// closes once it was read.
pub fn host(identity: Identity, plan: Plan) -> Result<()> {
    crate::door::run(|w| {
        // WHAT THE TERMINALS' THREADS HEAR comes into the loop here.
        let (heard, hearing) = calloop::channel::channel::<Arrival>();
        w.handle
            .insert_source(hearing, |event, (), door: &mut Door<TerminalWindow>| {
                if let calloop::channel::Event::Msg(a) = event {
                    arrived(door, a);
                }
            })
            .map_err(|e| anyhow::anyhow!("hang the terminal's channel: {e}"))?;
        // AND THAT THE SCREEN IN FRONT MOVED: a ping, which carries nothing
        // and coalesces — a flood of output is one paint per frame shown.
        let (ping, pinged) = calloop::ping::make_ping().context("create the terminal's ping")?;
        w.handle
            .insert_source(pinged, |(), (), door: &mut Door<TerminalWindow>| {
                door.content.moved(&mut door.ui);
            })
            .map_err(|e| anyhow::anyhow!("hang the terminal's ping: {e}"))?;
        let heard = Mutex::new(heard);
        let ping = Mutex::new(ping);
        let wires = Wires {
            heard: Arc::new(move |a| {
                if let Ok(h) = heard.lock() {
                    let _ = h.send(a);
                }
            }),
            ping: Arc::new(move || {
                if let Ok(p) = ping.lock() {
                    p.ping();
                }
            }),
            front: Arc::new(AtomicU64::new(u64::MAX)),
        };
        // THE PERSON'S SETTINGS, read now and again whenever the file moves:
        // a value turned in the settings view reaches every terminal window
        // without it being reopened.
        let dir = crate::terminal_settings::dir();
        let store = crate::terminal_settings::store_in(dir.clone());
        let (moved, heard_moved) = calloop::channel::channel::<()>();
        let settings_watch = dir.as_deref().and_then(|dir| {
            crate::terminal_settings::watch_in(
                dir,
                ronoctua_files::notice::Pace::Coalesced,
                move || moved.send(()).is_ok(),
            )
        });
        let reading = store.clone();
        w.handle
            .insert_source(
                heard_moved,
                move |event, (), door: &mut Door<TerminalWindow>| {
                    if let calloop::channel::Event::Msg(()) = event {
                        door.content.settings_are(&mut door.ui, reading.read());
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("hang the terminal.json watch: {e}"))?;
        let mut window = TerminalWindow::new(identity)?;
        window.settings = store.read();
        window._settings_watch = settings_watch;
        window.plan = Some(plan);
        window.wires = Some(wires);
        window.handle = Some(w.handle.clone());
        // FILES DROPPED ON IT are typed as their paths (`crate::terminal_drop`):
        // the door hears the drag, the window answers it (`Carries`).
        window.carrying = crate::carry::Carrying::bind(w.globals, w.qh);
        Ok(window)
    })
}

/// HOW EVERY THREAD OF THE WINDOW REACHES ITS LOOP: what it heard, that the
/// screen moved — and which attachment is in front, the only one whose
/// screen moving is worth a frame. A terminal behind is still followed, its
/// mirror fed, and wakes nobody.
struct Wires {
    heard: Arc<dyn Fn(Arrival) + Send + Sync>,
    ping: Arc<dyn Fn() + Send + Sync>,
    front: Arc<AtomicU64>,
}

impl Wires {
    /// The ears of attachment `generation`.
    fn tell(&self, generation: u64) -> Arc<terminal_link::Tell> {
        let heard = Arc::clone(&self.heard);
        let ping = Arc::clone(&self.ping);
        let front = Arc::clone(&self.front);
        Arc::new(terminal_link::Tell {
            heard: Box::new(move |a| heard(a)),
            moved: Box::new(move || {
                if front.load(Ordering::Relaxed) == generation {
                    ping();
                }
            }),
        })
    }
}

/// The faces a cell is measured from: the primary face alone, which is all
/// `cells::Geometry::measure` reads — a bank of its own, small, because the
/// door's (with every fallback) is the door's to keep.
struct Faces(cosmic_text::FontSystem);

impl Faces {
    fn new() -> Self {
        let mut db = cosmic_text::fontdb::Database::new();
        db.load_font_source(cosmic_text::fontdb::Source::Binary(Arc::new(
            ronoctua_render::text::EMBEDDED_FONT,
        )));
        Self(cosmic_text::FontSystem::new_with_locale_and_db(
            "en-GB".to_owned(),
            db,
        ))
    }

    /// The cell at `percent` of the terminal's size, on a screen of `scale`.
    fn measure(&mut self, percent: u16, scale: u32) -> Option<Geometry> {
        let points = tokens::font::TERMINAL_POINTS * f32::from(percent) / 100.0;
        Geometry::measure(
            &mut self.0,
            points * PIXELS_PER_POINT,
            u16::try_from(scale.max(1)).unwrap_or(u16::MAX),
        )
    }
}

/// What a pointer keeps over the terminal.
#[derive(Debug, Default)]
pub struct Pointing {
    /// The left button is held over the grid, and a selection follows it.
    selecting: bool,
    /// The left button grabbed the scrollbar.
    on_the_bar: bool,
    /// The buttons a program tracking the mouse was told went down — and so
    /// is owed their release.
    reported: Vec<u32>,
    /// What is left of the sideways wheel, in cells.
    across_owed: f64,
}

/// THE TERMINAL'S WINDOW.
pub struct TerminalWindow {
    identity: Identity,
    /// What to start once the window has a size — a shell is not born at
    /// 80×24 to be told its real size a moment later.
    plan: Option<Plan>,
    /// A recorded stream the camera feeds an engine in this process with,
    /// once the window has a size.
    corpus: Option<Vec<u8>>,
    /// What went wrong developing the corpus, for the camera to say.
    corpus_failed: Option<String>,
    /// The terminals this window holds, the one in front, and those on
    /// their way (`crate::terminal_group`).
    group: Group,
    /// The group's id, which every terminal of this window is attached with
    /// — a fresh one each window: a group is the window that shows it.
    group_id: GroupId,
    /// The number the next attachment takes.
    next_generation: u64,
    /// How the threads reach the loop; `None` for a window with no loop (the
    /// camera's, a test's).
    wires: Option<Wires>,
    handle: Option<calloop::LoopHandle<'static, Door<Self>>>,
    /// The drag-and-drop machinery, where the compositor offers it.
    carrying: Option<crate::carry::Carrying>,
    /// The drop being read off the loop, finished when its list arrives.
    dropping: Option<DragOffer>,
    faces: Faces,
    geometry: Geometry,
    /// The window's own size of text, in percent of the terminal's — `None`
    /// follows the person's (`C-0`).
    zoom: Option<u16>,
    /// The person's terminal settings (`terminal.json`), read when the window
    /// opens and again whenever the file moves (`host`'s watch).
    settings: crate::terminal_settings::Settings,
    /// That watch, kept alive.
    _settings_watch: Option<ronoctua_files::notice::Notice>,
    /// The window's size and scale as last laid out, logical pixels.
    laid_on: Option<(u32, u32, u32)>,
    placed: Placed,
    sheet: Sheet,
    /// The page's colours last resolved.
    palette: Option<Palette>,
    /// The ground the window is cleared with.
    background: Rgba,
    focused: bool,
    blink: Option<calloop::RegistrationToken>,
    blink_off: bool,
    /// WHAT A TERMINAL ON ITS WAY SAID before its launch came back: its
    /// reader runs from the attach on, and the launch thread tells the
    /// window only after — so its first state, or the end of a terminal
    /// shown after it ended, can come first. Heard once it is taken in.
    early: Vec<Arrival>,
    /// The question on the line — and whose it is, an attachment's number —
    /// and the ones waiting for it.
    question: Option<(u64, Question)>,
    waiting: VecDeque<(u64, Asked)>,
    /// A clock per program's clipboard read asked — `(attachment, read)` —
    /// armed only while it is asked: at its holder's deadline it is taken off.
    consent_clocks: Vec<((u64, u64), calloop::RegistrationToken)>,
    /// The run of the line under the pointer, lit.
    lit: Option<Click>,
    /// The line drawn over the last row, laid out — for a click on it.
    laid: Vec<(u16, Run)>,
    /// The search on the line (`C-S-f`).
    finding: Option<Finding>,
    /// The matches drawn on the last frame, and the one walked to.
    marked: terminal_search::Shown,
    /// The strip's line under the pointer.
    hovered: Option<usize>,
    /// Where the pointer is over the surface, logical pixels.
    pointer_at: Option<(f64, f64)>,
    /// `Ctrl` is held: a link under the pointer shows.
    ctrl_held: bool,
    /// The link under the pointer while `Ctrl` is held, and the one the
    /// last frame underlined.
    link: Option<Link>,
    underlined: Option<Link>,
    /// The pointer was hidden by a key typed, until it moves.
    hid_the_pointer: bool,
    /// What an input method is composing, drawn at the cursor until it
    /// commits (`crate::door::text_input`).
    preedit: Option<Preedit>,
    /// The program's cursor as the last frame drew it, `(column, line)` —
    /// where a composition is drawn and the input method's guesses open,
    /// kept while the program hides it (as it does to redraw).
    ime_at: Option<(u16, u16)>,
    /// The bar's track as last drawn — where a press grabs it. `None` while
    /// it is hidden: a bar is grabbed only where it is drawn.
    bar_track: Option<chrome::Rect>,
    /// Where the bar WOULD be while there is history to scroll, shown or not:
    /// the pointer entering it is what shows it.
    bar_zone: Option<chrome::Rect>,
    /// THE BAR IS DRAWN, over the grid's right edge (the owner's exception,
    /// 2026-11-03): only while it is used, so it never takes the program's
    /// room.
    bar_shown: bool,
    /// The pointer is on the bar's edge: it stays while it is.
    bar_hovered: bool,
    /// The clock that hides it [`BAR_SHOWN`] after the last use — armed only
    /// while it is shown.
    bar_hide: Option<calloop::RegistrationToken>,
    /// A retitle waits for the loop.
    retitle_owed: bool,
    /// The window closes once its line was read: no terminal could start.
    closing: bool,
    /// Every row is to be written on the next frame.
    refill: bool,
    /// A frame was wanted while one was in flight: drawn when it is shown.
    owed: bool,
    /// The keys whose press went to the program in front, owed their
    /// release — each by its code, with the frame it went as: a terminal
    /// that leaves the front before the hand lets go is sent the release of
    /// each ([`TerminalWindow::fronted`]).
    pressed: HashMap<u32, wire::KeyEvent>,
    mods: Modifiers,
    /// The scrollbar as last drawn.
    scroll: ronoctua_design::Scroll,
    /// The first frame was said in the log (the VM test waits for it).
    drawn: bool,
}

/// What a strip's line says of its terminal — what moving makes it repaint.
fn asks(held: &Held) -> (bool, Option<Progress>) {
    held.state
        .as_deref()
        .map_or((false, None), |s| (s.asks_attention(), s.progress))
}

impl TerminalWindow {
    /// A window with nothing to show yet.
    ///
    /// # Errors
    ///
    /// The terminal's face measures no cell, or no id could be made.
    pub fn new(identity: Identity) -> Result<Self> {
        let mut faces = Faces::new();
        let geometry = faces
            .measure(tokens::text::percent(), 1)
            .context("the terminal's face measures no cell")?;
        let palette = Palette::of_the_page();
        let sheet = Sheet::new(
            geometry,
            tokens::space::TERMINAL_COLUMNS,
            tokens::space::TERMINAL_LINES,
            ronoctua_render::cells::Defaults {
                foreground: palette.foreground,
                background: palette.background,
                faint: faint_alpha(),
            },
        );
        Ok(Self {
            identity,
            plan: None,
            corpus: None,
            corpus_failed: None,
            group: Group::default(),
            group_id: GroupId::fresh().context("a fresh group id")?,
            next_generation: 0,
            wires: None,
            handle: None,
            carrying: None,
            dropping: None,
            faces,
            geometry,
            zoom: None,
            // THE DEFAULTS until `host` reads the person's: a window built
            // for a test or a camera reads nothing of the machine.
            settings: crate::terminal_settings::Settings::default(),
            _settings_watch: None,
            laid_on: None,
            placed: Placed::of(1, 1, 1, &geometry),
            sheet,
            palette: None,
            background: palette.background,
            focused: false,
            blink: None,
            blink_off: false,
            early: Vec::new(),
            question: None,
            waiting: VecDeque::new(),
            consent_clocks: Vec::new(),
            lit: None,
            laid: Vec::new(),
            finding: None,
            marked: terminal_search::Shown::default(),
            hovered: None,
            pointer_at: None,
            ctrl_held: false,
            link: None,
            underlined: None,
            hid_the_pointer: false,
            preedit: None,
            ime_at: None,
            bar_track: None,
            bar_zone: None,
            bar_shown: false,
            bar_hovered: false,
            bar_hide: None,
            retitle_owed: false,
            closing: false,
            refill: true,
            owed: false,
            pressed: HashMap::new(),
            mods: Modifiers::default(),
            scroll: chrome::list_scroll(0, 0, 0),
            drawn: false,
        })
    }

    /// THE CAMERA'S WINDOW: `corpus` fed to an engine in this process at the
    /// size the window is laid out at, and the frame it leaves drawn.
    ///
    /// # Errors
    ///
    /// As [`TerminalWindow::new`].
    pub fn photographed(identity: Identity, corpus: Vec<u8>) -> Result<Self> {
        Self::photographed_group(identity, corpus, 1, 0)
    }

    /// THE CAMERA'S WINDOW OF A GROUP: `count` terminals, `front` (from 0)
    /// the one shown — `corpus` fed to its engine, the others' screens
    /// empty, and the strip drawn as it is for them.
    ///
    /// # Errors
    ///
    /// As [`TerminalWindow::new`], or the library made no mirror.
    pub fn photographed_group(
        identity: Identity,
        corpus: Vec<u8>,
        count: usize,
        front: usize,
    ) -> Result<Self> {
        let mut window = Self::new(identity)?;
        for n in 0..count.max(1) {
            let generation = window.fresh_generation();
            let held = Held::new(
                generation,
                TerminalId::fresh().context("a fresh terminal id")?,
                terminal_group::step(n),
                Mirrored::new()?,
            );
            window.group.take_in(held, n == front);
        }
        window.corpus = Some(corpus);
        Ok(window)
    }

    /// The camera's: the strip's line `line` (from 0) under the pointer.
    pub fn hover_line(&mut self, line: Option<usize>) {
        self.hovered = line.filter(|&i| i < self.group.len());
    }

    /// The camera's: the terminal of line `line` (from 0) asks for the
    /// person, as a bell rung behind the one in front does.
    pub fn ring(&mut self, line: usize) {
        let group = self.group_id.clone();
        let Some(held) = self.group.get_mut(line) else {
            return;
        };
        let mut state = held.state.take().unwrap_or_else(|| {
            Box::new(TerminalState {
                protocol: PROTOCOL,
                id: held.id.clone(),
                group,
                order: held.order,
                viewer: Some(std::process::id()),
                front: false,
                seen: false,
                title: String::new(),
                program: String::new(),
                cwd: None,
                started_s: 0,
                alone_since_s: None,
                attention: Attention::default(),
                notice: None,
                command: None,
                front_asked: 0,
                ended: None,
                statuses: Vec::new(),
                progress: None,
            })
        });
        state.attention.bell += 1;
        held.state = Some(state);
    }

    /// What went wrong feeding the camera's corpus, once.
    pub fn corpus_failed(&mut self) -> Option<String> {
        self.corpus_failed.take()
    }

    /// The grid as the window would draw it — for a test, and for the camera.
    #[must_use]
    pub const fn sheet_now(&self) -> &Sheet {
        &self.sheet
    }

    /// The cell as last measured — at one pixel a pixel until the window is
    /// laid out on a screen.
    #[must_use]
    pub const fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    /// The cell this window measures on a screen of `scale` — the camera's,
    /// for a grid asked in cells: each scale's cell is rounded to whole
    /// pixels of its own (`Geometry::measure`), so a cell at two is not
    /// always this one doubled, and a grid laid out by the one-pixel cell
    /// comes up a column or a row short at two.
    pub fn cell_at(&mut self, scale: u32) -> Geometry {
        let percent = self.percent();
        self.faces.measure(percent, scale).unwrap_or(self.geometry)
    }

    /// How many terminals the window holds now.
    #[must_use]
    pub fn terminals(&self) -> usize {
        self.group.len()
    }

    /// The size of text now, in percent: the window's own zoom, else the
    /// person's terminal size (`terminal.json`), else the system's.
    fn percent(&self) -> u16 {
        self.zoom.unwrap_or_else(|| self.settings.percent())
    }

    /// `terminal.json` moved — this window's own settings view, the footer's,
    /// a hand: what applies now is applied. The size of the text lays the
    /// grid out again; where a selection is copied is read at the next
    /// selection, and what a new terminal is born with at the next one — a
    /// terminal that runs keeps what it was born with, which its snapshot
    /// carries to this window's mirror.
    pub fn settings_are(&mut self, ui: &mut Ui, settings: crate::terminal_settings::Settings) {
        let before = std::mem::replace(&mut self.settings, settings);
        if before.percent() != settings.percent() && self.zoom.is_none() {
            self.lay_out(ui);
        }
    }

    /// A new attachment's number.
    const fn fresh_generation(&mut self) -> u64 {
        let g = self.next_generation;
        self.next_generation += 1;
        g
    }

    /// The attachment in front's number.
    fn front_generation(&self) -> Option<u64> {
        self.group.front().map(|h| h.generation)
    }

    /// The grid of a group of `count` on the window as last laid out —
    /// under the strip from two up.
    fn placed_for(&self, count: usize) -> Placed {
        self.laid_on.map_or(self.placed, |(width, height, scale)| {
            Placed::under(
                width,
                height,
                scale,
                terminal_strip::height(count),
                &self.geometry,
            )
        })
    }

    /// Lays the grid out on the window's size, scale and size of text: the
    /// cell measured again, the grid counted, and every terminal told — or,
    /// the first time, the terminals started at that size.
    fn lay_out(&mut self, ui: &mut Ui) {
        // A WINDOW WITH NO SIZE HAS NO GRID: before the first configure the
        // scale can arrive alone, and a terminal told it was one cell would
        // print its prompt for one cell.
        if ui.width == 0 || ui.height == 0 {
            return;
        }
        let scale = u32::try_from(ui.scale.max(1)).unwrap_or(1);
        if let Some(g) = self.faces.measure(self.percent(), scale) {
            self.geometry = g;
        }
        self.laid_on = Some((ui.width, ui.height, scale));
        self.lay_the_grid();
        if let Some(plan) = self.plan.take() {
            self.open_with(plan);
        } else if let Some(corpus) = self.corpus.take()
            && let Err(e) = self.develop(&corpus)
        {
            self.corpus_failed = Some(format!("{e:#}"));
        }
        self.sync_sizes();
        ui.ask_paint();
    }

    /// The grid placed for the group as it is now: its pixels off the top
    /// for the strip from two terminals up.
    fn lay_the_grid(&mut self) {
        self.placed = self.placed_for(self.group.len());
        self.sheet.set_geometry(self.geometry);
        self.sheet.origin = (
            i32::try_from(self.placed.origin.0).unwrap_or(0),
            i32::try_from(self.placed.origin.1).unwrap_or(0),
        );
    }

    /// EVERY TERMINAL OF THE GROUP at the window's grid: one size for them
    /// all, so a terminal brought to the front is the right size already —
    /// and each told only when its size moved, so a group that grew to two
    /// is ONE `SIGWINCH` to each.
    fn sync_sizes(&mut self) {
        let size = self.placed.size();
        for held in self.group.iter_mut() {
            if held.sent_size != Some(size) {
                held.send(ToHolder::Resize(size));
                held.sent_size = Some(size);
            }
        }
    }

    /// Every terminal of the group in the page's colours, those whose
    /// colours moved told.
    fn sync_colors(&mut self) {
        let Some(colors) = self.palette.as_ref().map(Palette::wire) else {
            return;
        };
        for held in self.group.iter_mut() {
            if held.sent_colors.as_ref() != Some(&colors) {
                held.send(ToHolder::Colors(colors.clone()));
                held.sent_colors = Some(colors.clone());
            }
        }
    }

    /// The page's colours: as the last frame resolved them on the window's
    /// own page, or — before the first frame — as the process wears them
    /// now (`open_with` wears the window's page first; the camera's develop
    /// wears what the camera does), and the first frame sends them again
    /// when they moved (`paint`, `sync_colors`).
    fn colors_now(&self) -> wire::Colors {
        self.palette
            .as_ref()
            .map_or_else(|| Palette::of_the_page().wire(), Palette::wire)
    }

    /// The window's first terminals, launched: the plan's one, or every one
    /// it shows in the strip's order, the one named in front.
    fn open_with(&mut self, plan: Plan) {
        // THE WINDOW'S OWN PAGE, worn before the first frame wears it (the
        // door wears it at every frame, so this changes nothing else): a
        // program that asks its colours as it starts — an editor choosing
        // light or dark by OSC 11 — is answered from the page it will be
        // drawn on, and not from the chrome's where the windows wear a desk.
        tokens::theme::set_ground(Content::page(self));
        let front = match &plan {
            Plan::Show { front, .. } => Some(front.clone()),
            Plan::New { .. } => None,
        };
        let reachings = plan.reachings();
        let count = reachings.len();
        for (n, reaching) in reachings.into_iter().enumerate() {
            let (in_front, new) = match &reaching {
                Reaching::Held(id) => (front.as_ref() == Some(id), false),
                Reaching::New { .. } => (true, true),
            };
            self.launch(reaching, terminal_group::step(n), in_front, new, count);
        }
    }

    /// Starts reaching one terminal, off the loop, its place in the group
    /// promised — at the size of the grid a group of `count` will have.
    fn launch(&mut self, reaching: Reaching, order: u32, front: bool, new: bool, count: usize) {
        let generation = self.fresh_generation();
        let size = self.placed_for(count).size();
        let colors = self.colors_now();
        let Some(tell) = self.wires.as_ref().map(|w| w.tell(generation)) else {
            return;
        };
        let mirrored = match Mirrored::new() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(error = format!("{e:#}"), "no mirror for a terminal");
                return;
            }
        };
        self.group.expect(Pending {
            generation,
            order,
            front,
            mirrored: Arc::clone(&mirrored),
            size,
            colors: colors.clone(),
            new,
        });
        terminal_link::launch(
            reaching,
            terminal_link::Showing {
                size,
                colors,
                group: self.group_id.clone(),
                order,
            },
            // WHAT THE PERSON CHOSE, as it is now: a new terminal is born
            // with it, one that runs keeps its own.
            self.settings.born(),
            mirrored,
            tell,
            generation,
        );
    }

    /// A launch came back: the terminal taken into the group at its place,
    /// or what failed said.
    fn started(
        &mut self,
        ui: &mut Ui,
        generation: u64,
        started: Result<terminal_link::Started, String>,
    ) {
        let early: Vec<Arrival> = {
            let (ours, others) = std::mem::take(&mut self.early)
                .into_iter()
                .partition(|a| a.generation == generation);
            self.early = others;
            ours
        };
        let Some(pending) = self.group.arrived(generation) else {
            if let Ok(s) = started {
                s.reach.window_gone();
            }
            return;
        };
        let started = match started {
            Ok(s) => s,
            Err(why) => {
                tracing::error!(why, "a terminal could not start");
                if self.group.is_done() {
                    self.fail_to_start(ui);
                } else if pending.new {
                    ui.say(ui.texts.t(crate::key::TERMINAL_NEW_FAILED));
                }
                // A TERMINAL SHOWN that is not there any more (it ended since,
                // its holder went away) is not there to say anything about.
                return;
            }
        };
        tracing::info!(
            engine = "ghostty",
            release = ghostty::RELEASE,
            id = %started.reach.id(),
            in_process = started.reach.in_process(),
            "terminal opened"
        );
        if started.fell_back {
            ui.say_for(
                ui.texts.t(crate::key::TERMINAL_IN_PROCESS),
                crate::echo::Echo::Long,
            );
        }
        let mut held = Held::new(
            generation,
            started.reach.id().clone(),
            pending.order,
            pending.mirrored,
        );
        held.reach = Some(started.reach);
        held.link = Some(started.link);
        held.sent_size = Some(pending.size);
        held.sent_colors = Some(pending.colors);
        self.took_in(ui, held, pending.front);
        // WHAT IT SAID ON ITS WAY, heard now in the order it was said — a
        // terminal shown after its program ended leaves at once, as it
        // would have had the launch come back first.
        for a in early {
            self.heard_from(ui, a.generation, a.heard);
        }
    }

    /// `held` taken into the group — in front when `front` says so — and
    /// the window laid out again when the strip came with it.
    fn took_in(&mut self, ui: &mut Ui, held: Held, front: bool) {
        let before = self.front_generation();
        let had = self.group.len();
        let generation = held.generation;
        self.group.take_in(held, front);
        // ONE BEHIND hears now that it is not focused: a program reporting
        // focus (1004) is told the truth from its first frame.
        if self.front_generation() != Some(generation)
            && let Some(h) = self.group.by_generation(generation)
        {
            h.send(ToHolder::Focus(wire::Focus { gained: false }));
        }
        if terminal_strip::height(had) != terminal_strip::height(self.group.len()) {
            self.lay_the_grid();
            self.refill = true;
        }
        self.sync_sizes();
        self.sync_colors();
        self.fronted(ui, before);
        self.place_all();
        ui.ask_paint();
    }

    /// NO TERMINAL COULD START, and nothing else is held: said on the line
    /// for as long as a refusal is, and then the window closes — a window
    /// that closed with only a line in the log was a window that vanished.
    fn fail_to_start(&mut self, ui: &mut Ui) {
        ui.say_for(
            ui.texts.t(crate::key::TERMINAL_COULD_NOT_START),
            crate::echo::Echo::Long,
        );
        self.closing = true;
        let Some(handle) = &self.handle else { return };
        let armed = handle.insert_source(
            calloop::timer::Timer::from_duration(crate::echo::Echo::Long.lasts()),
            |_, (), door: &mut Door<Self>| {
                door.ui.quit = true;
                calloop::timer::TimeoutAction::Drop
            },
        );
        if let Err(e) = armed {
            tracing::warn!(error = %e, "the closing clock could not be armed; the window closes now");
            ui.quit = true;
        }
    }

    /// THE CAMERA'S ENGINE: the corpus fed to one in this process at the
    /// grid's size, in the page's colours, and its snapshot decoded into the
    /// front's mirror — the same mirror a holder's attach gives, by the same
    /// road.
    fn develop(&mut self, corpus: &[u8]) -> Result<()> {
        let size = self.placed.size();
        let mut vt = ghostty::Vt::new(
            size.columns,
            size.rows,
            &ghostty::Config::for_version(terminal_sheet::VERSION),
        )?;
        vt.resize(
            size.columns,
            size.rows,
            u32::from(size.cell_width_px),
            u32::from(size.cell_height_px),
        )?;
        let colors = terminal_sheet::engine_colors(&self.colors_now())
            .context("the page's palette is not 256 colours")?;
        vt.set_colors(&colors)?;
        vt.feed(corpus);
        let mirror = ghostty::Decoder::decode(vt.snapshot()?)?;
        self.group
            .front()
            .context("the camera's window holds no terminal")?
            .mirrored
            .install(mirror);
        self.refill = true;
        Ok(())
    }

    /// The screen in front moved: the search looks again, the link under a
    /// pointer that `Ctrl` holds is read again off what is shown, and a
    /// frame, now or when the one in flight is shown.
    fn moved(&mut self, ui: &mut Ui) {
        if let Some(f) = &self.finding {
            f.order(terminal_search::Order::Look);
        }
        if self.ctrl_held {
            self.hover_link(ui);
        }
        self.frame_wanted(ui);
    }

    /// A frame, now or when the one in flight is shown.
    fn frame_wanted(&mut self, ui: &mut Ui) {
        if ui.frame_in_flight() {
            self.owed = true;
        } else {
            ui.ask_paint();
        }
    }

    /// The window's own view moved — scrolled, selected — which the
    /// engine's dirty rows may not all say: every row is written again on
    /// the next frame.
    fn view_moved(&mut self, ui: &mut Ui) {
        self.refill = true;
        self.moved(ui);
    }

    /// ANOTHER TERMINAL IS IN FRONT (or none, `before` being the one that
    /// was): its screen is the one followed and drawn, the keyboard's focus
    /// moves to it, every terminal is told where it is, and what belonged to
    /// the one before — its search, its link, the keys it was owed the
    /// release of — goes.
    fn fronted(&mut self, ui: &mut Ui, before: Option<u64>) {
        let now = self.front_generation();
        if now == before {
            return;
        }
        if let Some(w) = &self.wires {
            w.front.store(now.unwrap_or(u64::MAX), Ordering::Relaxed);
        }
        if self.focused
            && let Some(was) = before.and_then(|g| self.group.by_generation(g))
        {
            was.send(ToHolder::Focus(wire::Focus { gained: false }));
        }
        if let Some(front) = self.group.front() {
            front.send(ToHolder::Focus(wire::Focus {
                gained: self.focused,
            }));
        }
        if self
            .finding
            .as_ref()
            .is_some_and(|f| Some(f.generation) != now)
        {
            self.finding = None;
        }
        self.link = None;
        // THE KEYS STILL DOWN were the old front's: it hears each come up
        // now, as it would have when the hand let go — a program that asked
        // for release events (kitty's) is never left with `Ctrl` held for
        // good — and the new front hears none of them, having heard none go
        // down.
        let owed: Vec<wire::KeyEvent> = self.pressed.drain().map(|(_, k)| k).collect();
        if let Some(was) = before.and_then(|g| self.group.by_generation(g)) {
            for mut key in owed {
                key.action = wire::KeyAction::Release;
                was.send(ToHolder::Key(key));
            }
        }
        self.blink_off = false;
        self.place_all();
        self.refill = true;
        self.retitle_later();
        ui.ask_paint();
    }

    /// Tells every terminal whose place moved where it is: in front, and
    /// seen while the window holds the keyboard — what clears a terminal's
    /// call for attention.
    fn place_all(&mut self) {
        for (i, place) in self.group.places(self.focused) {
            if let Some(h) = self.group.get(i) {
                h.send(ToHolder::Place(place));
            }
            self.group.told(i, place);
        }
    }

    /// What the threads heard.
    fn hear(&mut self, ui: &mut Ui, a: Arrival) {
        let generation = a.generation;
        match a.heard {
            Heard::Started(started) => self.started(ui, generation, *started),
            Heard::Found(found) => {
                if let Some(f) = &mut self.finding
                    && f.generation == generation
                    && f.heard(*found)
                {
                    ui.ask_paint();
                }
            }
            Heard::Dropped(paths) => self.dropped(ui, &paths),
            Heard::Consent(consent, answering) => {
                if self.group.index_of(generation).is_some() {
                    self.ask_consent(ui, generation, consent);
                } else {
                    // A READ BEFORE ITS TERMINAL IS SHOWN — the attach still
                    // on its way to the window: nobody has seen the program
                    // that asks, so nobody is asked, and the holder is
                    // answered at once instead of at its deadline.
                    tracing::info!(
                        id = consent.id,
                        "a clipboard read came before its terminal was shown; refused"
                    );
                    answering.send(ToHolder::Answer(wire::Answer {
                        id: consent.id,
                        text: None,
                    }));
                }
            }
            heard => self.heard_from(ui, generation, heard),
        }
    }

    /// What one terminal of the group said.
    fn heard_from(&mut self, ui: &mut Ui, generation: u64, heard: Heard) {
        let Some(index) = self.group.index_of(generation) else {
            if self.group.is_pending(generation) {
                self.early.push(Arrival { generation, heard });
            }
            return;
        };
        let front = index == self.group.front_index();
        match heard {
            Heard::Ready => {
                if front {
                    self.refill = true;
                    self.moved(ui);
                }
            }
            Heard::State(state) => self.state_heard(ui, index, state),
            Heard::Clipboard(write) => copy_for_the_program(&write),
            Heard::Taken => {
                tracing::info!("another window took one of this window's terminals; it leaves");
                self.leave(ui, generation);
            }
            Heard::Lagging => self.attach_again(ui, generation),
            Heard::Exited(code) => {
                tracing::info!(code, "a terminal's program ended; it leaves the window");
                self.leave(ui, generation);
            }
            Heard::Unreadable => {
                if front {
                    ui.say_for(
                        ui.texts.t(crate::key::TERMINAL_OTHER_BUILD),
                        crate::echo::Echo::Long,
                    );
                }
            }
            Heard::Gone => {
                if !ui.quit {
                    tracing::info!("a terminal's holder went away; it leaves the window");
                    self.leave(ui, generation);
                }
            }
            // A PASTE THE HOLDER DID NOT WRITE is said where it was made, on
            // this window's line — whichever terminal of the group it went
            // to: the person waits for it to land.
            Heard::PasteRefused(why) => ui.say(ui.texts.t(match why {
                wire::PasteRefusal::Unsafe => crate::key::TERMINAL_PASTE_REFUSED_UNSAFE,
                wire::PasteRefusal::Failed => crate::key::TERMINAL_PASTE_REFUSED,
            })),
            Heard::Attached(_)
            | Heard::Started(_)
            | Heard::Found(_)
            | Heard::Dropped(_)
            | Heard::Consent(..) => {}
        }
    }

    /// A terminal's whole state: its title (the window's, while it is in
    /// front or its line is under the pointer), what its strip line says,
    /// and whether something asked for it to be put in front.
    fn state_heard(&mut self, ui: &mut Ui, index: usize, state: Box<TerminalState>) {
        let front = index == self.group.front_index();
        let hovered = self.hovered == Some(index);
        let Some(held) = self.group.get_mut(index) else {
            return;
        };
        let (title, line) = (held.title().to_owned(), asks(held));
        let asked = held.heard(state);
        let title_moved = held.title() != title;
        let line_moved = asks(held) != line;
        if asked {
            let before = self.front_generation();
            if self.group.front_to(index) {
                self.fronted(ui, before);
            }
        } else if title_moved && (front || hovered) {
            self.retitle_later();
        }
        if line_moved && self.group.len() >= terminal_strip::FEWEST {
            ui.ask_paint();
        }
    }

    /// The attachment `generation` leaves the group — ended, taken, gone.
    /// The last one closes the window; one that leaves a single terminal
    /// takes the strip with it, and that terminal's rows back.
    fn leave(&mut self, ui: &mut Ui, generation: u64) {
        let before = self.front_generation();
        let had = self.group.len();
        let (left, gone) = self.group.leave(generation);
        if let Some(reach) = gone.as_ref().and_then(|h| h.reach.as_ref()) {
            reach.window_gone();
        }
        self.forget_questions_of(ui, generation);
        match left {
            Left::Stranger => return,
            Left::Last => {
                ui.quit = true;
                return;
            }
            Left::Front | Left::Behind => {}
        }
        self.hovered = None;
        if terminal_strip::height(had) != terminal_strip::height(self.group.len()) {
            self.lay_the_grid();
            self.sync_sizes();
            self.refill = true;
        }
        self.fronted(ui, before);
        self.retitle_later();
        ui.ask_paint();
    }

    /// What was asked for attachment `generation` goes with it: its
    /// questions, their clocks, its search.
    fn forget_questions_of(&mut self, ui: &mut Ui, generation: u64) {
        self.waiting.retain(|(g, _)| *g != generation);
        let clocks: Vec<(u64, u64)> = self
            .consent_clocks
            .iter()
            .map(|(k, _)| *k)
            .filter(|(g, _)| *g == generation)
            .collect();
        for (g, id) in clocks {
            self.disarm_consent_clock(g, id);
        }
        if self
            .question
            .as_ref()
            .is_some_and(|(g, _)| *g == generation)
        {
            self.next_question(ui);
        }
        if self
            .finding
            .as_ref()
            .is_some_and(|f| f.generation == generation)
        {
            self.finding = None;
            self.refill = true;
        }
    }

    /// The window fell behind one terminal and was let go: a fresh attach,
    /// a fresh snapshot. On the loop — a connect to a local socket whose
    /// holder has just spoken is not a wait.
    fn attach_again(&mut self, ui: &mut Ui, generation: u64) {
        let fresh = self.fresh_generation();
        let Some(tell) = self.wires.as_ref().map(|w| w.tell(fresh)) else {
            return;
        };
        let size = self.placed.size();
        let colors = self.colors_now();
        let group = self.group_id.clone();
        let Some(held) = self.group.by_generation(generation) else {
            return;
        };
        let showing = terminal_link::Showing {
            size,
            colors,
            group,
            order: held.order,
        };
        let attached = match &held.reach {
            Some(reach) => Attachment::attach(reach, &showing, &held.mirrored, &tell, fresh),
            None => return,
        };
        match attached {
            Ok(link) => {
                held.generation = fresh;
                held.link = Some(link);
                held.sent_size = Some(showing.size);
                held.sent_colors = Some(showing.colors);
                held.told = None;
                let in_front = self.front_generation() == Some(fresh);
                if in_front && let Some(w) = &self.wires {
                    w.front.store(fresh, Ordering::Relaxed);
                }
                // WHAT THE OLD ATTACHMENT WAS ASKED went with it: its holder
                // refused it as it let the window go.
                self.forget_questions_of(ui, generation);
                if in_front && let Some(front) = self.group.front() {
                    front.send(ToHolder::Focus(wire::Focus {
                        gained: self.focused,
                    }));
                }
                self.place_all();
            }
            Err(e) => {
                tracing::warn!(
                    error = format!("{e:#}"),
                    "a terminal could not be attached again; it leaves the window"
                );
                self.leave(ui, generation);
            }
        }
    }

    /// Puts a question on the line, or behind the one there.
    fn ask(&mut self, ui: &mut Ui, generation: u64, asked: Asked) {
        if self.question.is_some() {
            self.waiting.push_back((generation, asked));
        } else {
            self.question = Some((generation, Question::new(asked)));
            self.lit = None;
        }
        ui.ask_paint();
    }

    /// The next question waiting takes the line, if one waits.
    fn next_question(&mut self, ui: &mut Ui) {
        self.question = self.waiting.pop_front().map(|(g, a)| (g, Question::new(a)));
        self.lit = None;
        self.refill = true;
        ui.ask_paint();
    }

    /// A program's clipboard read, asked IN PLACE OF any its terminal asked
    /// before.
    ///
    /// A holder waits on one read at a time and gives it up at its deadline
    /// (`holder::CONSENT_DEADLINE`), so a newer read means every older one
    /// of that terminal is refused there already: kept on the line, it would
    /// take every key from the program for an answer nobody waits for. A
    /// paste the person asked about stays where it is, and the read waits
    /// behind it — as another terminal's read does.
    fn ask_consent(&mut self, ui: &mut Ui, generation: u64, consent: wire::Consent) {
        self.waiting
            .retain(|(g, a)| !(*g == generation && matches!(a, Asked::ClipboardRead(_))));
        let stale: Vec<(u64, u64)> = self
            .consent_clocks
            .iter()
            .map(|(k, _)| *k)
            .filter(|(g, _)| *g == generation)
            .collect();
        for (g, id) in stale {
            self.disarm_consent_clock(g, id);
        }
        if self
            .question
            .as_ref()
            .is_some_and(|(g, q)| *g == generation && matches!(q.asked, Asked::ClipboardRead(_)))
        {
            self.question = None;
            self.refill = true;
        }
        let id = consent.id;
        self.ask(ui, generation, Asked::ClipboardRead(consent));
        self.arm_consent_clock(generation, id);
    }

    /// The read's clock, ONE-SHOT and at the holder's own deadline: the
    /// window heard the read after the holder asked, so it never takes a
    /// question off before the holder stopped waiting for it.
    fn arm_consent_clock(&mut self, generation: u64, id: u64) {
        let Some(handle) = &self.handle else { return };
        let armed = handle.insert_source(
            calloop::timer::Timer::from_duration(ronoctua_term::holder::CONSENT_DEADLINE),
            move |_, (), door: &mut Door<Self>| {
                door.content
                    .consent_clocks
                    .retain(|(k, _)| *k != (generation, id));
                door.content.consent_expired(&mut door.ui, generation, id);
                calloop::timer::TimeoutAction::Drop
            },
        );
        match armed {
            Ok(token) => self.consent_clocks.push(((generation, id), token)),
            Err(e) => {
                tracing::warn!(error = %e, "the clipboard read's deadline could not be armed");
            }
        }
    }

    fn disarm_consent_clock(&mut self, generation: u64, id: u64) {
        let Some(at) = self
            .consent_clocks
            .iter()
            .position(|(k, _)| *k == (generation, id))
        else {
            return;
        };
        let (_, token) = self.consent_clocks.remove(at);
        if let Some(handle) = &self.handle {
            handle.remove(token);
        }
    }

    /// The holder of attachment `generation` gave read `id` up unanswered:
    /// its question goes, waiting or on the line, and the next one takes the
    /// line.
    fn consent_expired(&mut self, ui: &mut Ui, generation: u64, id: u64) {
        let ours = |g: u64, a: &Asked| {
            g == generation && matches!(a, Asked::ClipboardRead(c) if c.id == id)
        };
        self.waiting.retain(|(g, a)| !ours(*g, a));
        if self
            .question
            .as_ref()
            .is_some_and(|(g, q)| ours(*g, &q.asked))
        {
            tracing::info!(id, "a program's clipboard read went unanswered; refused");
            self.next_question(ui);
        }
    }

    /// The question was answered.
    fn answered(&mut self, ui: &mut Ui, yes: bool) {
        let Some((generation, q)) = self.question.take() else {
            return;
        };
        match q.asked {
            Asked::Paste(text, source) => {
                if yes {
                    self.send_paste(generation, text, source, true);
                }
            }
            Asked::ClipboardRead(consent) => {
                self.disarm_consent_clock(generation, consent.id);
                self.answer_the_program(generation, &consent, yes);
            }
        }
        self.next_question(ui);
    }

    /// The program's clipboard read, answered: the clipboard read off the
    /// loop and handed straight to its holder, or a refusal.
    fn answer_the_program(&mut self, generation: u64, consent: &wire::Consent, yes: bool) {
        let Some(link) = self
            .group
            .by_generation(generation)
            .and_then(|h| h.link.as_ref())
        else {
            return;
        };
        let id = consent.id;
        if !yes {
            link.send(ToHolder::Answer(wire::Answer { id, text: None }));
            return;
        }
        let out = link.sender();
        let done = move |read: Result<String>| {
            let text = match read {
                Ok(t) => Some(t),
                Err(e) => {
                    tracing::warn!(error = %e, "the clipboard could not be read for the program");
                    None
                }
            };
            let _ = out.send(ToHolder::Answer(wire::Answer { id, text }));
        };
        match consent.clipboard {
            wire::ClipboardKind::Standard => crate::pasting::text_then(done),
            wire::ClipboardKind::Selection | wire::ClipboardKind::Primary => {
                crate::pasting::primary_then(done);
            }
        }
    }

    /// Text pasted from `source` — onto the search's needle while it is up,
    /// sent to the terminal in front, or asked about first when it could run
    /// a command as it lands.
    fn paste_from(&mut self, ui: &mut Ui, text: &str, source: wire::PasteSource) {
        if text.is_empty() {
            return;
        }
        if let Some(f) = &mut self.finding {
            let step = f.paste(text);
            self.searched(ui, step);
            return;
        }
        let Some(front) = self.group.front() else {
            return;
        };
        let generation = front.generation;
        let bracketed = front
            .mirrored
            .with(|m| m.mode(Mode::BRACKETED_PASTE))
            .flatten()
            .unwrap_or(false);
        if paste_is_safe(text, bracketed) {
            self.send_paste(generation, text.to_owned(), source, false);
            self.typed_through(ui);
        } else {
            self.ask(ui, generation, Asked::Paste(text.to_owned(), source));
        }
    }

    /// A paste, to the holder of attachment `generation` — which brackets it
    /// or turns its line breaks into returns, as the program's modes say, and
    /// tells a program that asked where it came from (`source`).
    fn send_paste(
        &mut self,
        generation: u64,
        text: String,
        source: wire::PasteSource,
        allow_unsafe: bool,
    ) {
        if let Some(held) = self.group.by_generation(generation) {
            held.send(ToHolder::Paste(wire::Paste {
                text,
                source,
                allow_unsafe,
            }));
        }
    }

    /// Files let go over the window: their paths typed, quoted, as a paste
    /// of them — and the drop finished now that its list was read.
    fn dropped(&mut self, ui: &mut Ui, paths: &[PathBuf]) {
        if let Some(offer) = self.dropping.take() {
            offer.finish();
        }
        tracing::info!(dropped = paths.len(), "files were let go over the terminal");
        // A DROP IS NO CLIPBOARD: its paths are text inserted another way —
        // what `ghostty::PasteSource::Text` names — and a program that tells
        // the two apart hears it as the drop it was.
        let text = crate::terminal_drop::typed(paths);
        if !text.is_empty() {
            self.paste_from(ui, &text, wire::PasteSource::Text);
        }
    }

    /// Reads a drop's list OFF THE LOOP — the other end of the pipe is the
    /// program the files were dragged from, and it writes when it likes
    /// (`carry::read` waits a moment for it) — and hands it to the window.
    fn read_the_drop(&self, pipe: smithay_client_toolkit::data_device_manager::ReadPipe) {
        let Some(heard) = self.wires.as_ref().map(|w| Arc::clone(&w.heard)) else {
            return;
        };
        let read = std::thread::Builder::new()
            .name("term-drop".to_owned())
            .spawn(move || {
                let paths = crate::carry::read(pipe).unwrap_or_default();
                heard(Arrival {
                    generation: u64::MAX,
                    heard: Heard::Dropped(paths),
                });
            });
        if let Err(e) = read {
            tracing::warn!(error = %e, "no thread to read a drop on");
        }
    }

    /// Whether this window keeps `kept` from the program: the group's keys
    /// are a terminal's window's — the monitor and the rescue console hold
    /// one program, and every key is its.
    fn keeps(&self, kept: Kept) -> bool {
        !matches!(kept, Kept::Group(_)) || self.identity == Identity::Terminal
    }

    /// One of the terminal's own keys.
    fn keep(&mut self, ui: &mut Ui, kept: Kept) {
        match kept {
            Kept::Group(grouped) => return self.grouped(ui, grouped),
            Kept::Search => return self.open_search(ui),
            _ => {}
        }
        let Some(front) = self.group.front() else {
            return;
        };
        let mirrored = Arc::clone(&front.mirrored);
        match kept {
            Kept::Copy => match mirrored.with(|m| m.selected_text()).flatten() {
                Some(text) if !text.is_empty() => {
                    let count = i64::try_from(text.chars().count()).unwrap_or(i64::MAX);
                    copy_off_the_loop(text, false);
                    ui.say_for(
                        ui.texts.t_args(
                            crate::key::TERMINAL_COPIED,
                            &[("count", ronoctua_i18n::Arg::Number(count))],
                        ),
                        crate::echo::Echo::Brief,
                    );
                }
                _ => ui.say(ui.texts.t(crate::key::TERMINAL_NOTHING_SELECTED)),
            },
            Kept::Paste => ui.paste(),
            Kept::PastePrimary => ui.paste_primary(),
            Kept::Leaf(leaf) => {
                let page = isize::try_from(self.placed.rows).unwrap_or(isize::MAX);
                let scroll = match leaf {
                    Leaf::PageUp => Scroll::Lines(-page),
                    Leaf::PageDown => Scroll::Lines(page),
                    Leaf::Top => Scroll::Top,
                    Leaf::Bottom => Scroll::Bottom,
                };
                mirrored.with(|m| m.scroll(scroll));
            }
            Kept::Prompt(back) => {
                mirrored.with(|m| m.scroll(Scroll::Prompts(if back { -1 } else { 1 })));
            }
            Kept::SelectAll => {
                let selected = mirrored
                    .with(|m| m.select_all().unwrap_or(false))
                    .unwrap_or(false);
                if let Some(front) = self.group.front_mut() {
                    front.selected = selected;
                }
            }
            Kept::Zoom(zoom) => {
                self.zoom = match zoom {
                    Zoom::In => Some(tokens::text::stepped(self.percent(), 1)),
                    Zoom::Out => Some(tokens::text::stepped(self.percent(), -1)),
                    Zoom::Reset => None,
                };
                self.lay_out(ui);
            }
            Kept::Group(_) | Kept::Search => {}
        }
        // THE HISTORY PAGED THROUGH shows where in it the view is.
        if matches!(kept, Kept::Leaf(_) | Kept::Prompt(_)) {
            self.show_bar(ui);
        }
        self.view_moved(ui);
    }

    /// One of the group's keys or commands.
    fn grouped(&mut self, ui: &mut Ui, grouped: Grouped) {
        match grouped {
            Grouped::New => self.new_terminal(),
            Grouped::Next => self.step_front(ui, 1),
            Grouped::Previous => self.step_front(ui, -1),
            Grouped::Detach => self.detach(ui),
        }
    }

    /// `C-T`: a new terminal, RIGHT AFTER THE ONE IN FRONT, in its folder
    /// (the shell's own report of it, OSC 7, else where it started) — held
    /// as it is: a window of the window's own process makes another of its
    /// own process.
    fn new_terminal(&mut self) {
        let (cwd, here) = self.group.front().map_or((None, false), |f| {
            (
                f.state.as_deref().and_then(|s| s.cwd.clone()),
                f.in_process(),
            )
        });
        let order = self.group.order_after_front();
        let count = self.group.len() + self.group.pending() + 1;
        self.launch(
            Reaching::New {
                program: ronoctua_term::pty::default_program(),
                args: Vec::new(),
                cwd: cwd.or_else(|| std::env::current_dir().ok()),
                here,
            },
            order,
            true,
            true,
            count,
        );
    }

    /// The next (`by` 1) or previous (`by` -1) terminal in front.
    fn step_front(&mut self, ui: &mut Ui, by: isize) {
        let before = self.front_generation();
        if self.group.step(by) {
            self.fronted(ui, before);
        }
    }

    /// Terminal `index` of the strip in front.
    fn front_line(&mut self, ui: &mut Ui, index: usize) {
        let before = self.front_generation();
        if self.group.front_to(index) {
            self.fronted(ui, before);
        }
    }

    /// `C-S-RET`: the terminal in front, in a WINDOW OF ITS OWN — another
    /// viewer of the shell's, opened on it alone. Its attach takes the
    /// terminal from this window (the newest attach wins), and the holder
    /// telling this one so is what moves the front to a neighbour: a window
    /// that could not be opened leaves everything as it was. A terminal of
    /// this window's own process cannot be attached from another, and says
    /// so.
    fn detach(&mut self, ui: &mut Ui) {
        if self.group.len() < terminal_strip::FEWEST {
            return;
        }
        let Some(front) = self.group.front() else {
            return;
        };
        if front.in_process() {
            ui.say(ui.texts.t(crate::key::TERMINAL_DETACH_HERE));
            return;
        }
        let id = front.id.as_str().to_owned();
        tracing::info!(id, "a terminal leaves for a window of its own");
        ronoctua_shell_door::region::Doorway::open_window(
            ui,
            vec![
                "terminal".to_owned(),
                "--show".to_owned(),
                id.clone(),
                "--front".to_owned(),
                id,
            ],
        );
    }

    /// `C-S-f`: the search line, over the terminal in front — its worker
    /// started on that terminal's mirror.
    fn open_search(&mut self, ui: &mut Ui) {
        if self.finding.is_some() {
            return;
        }
        let Some(front) = self.group.front() else {
            return;
        };
        let generation = front.generation;
        let started = self.wires.as_ref().map(|w| {
            let heard = Arc::clone(&w.heard);
            front.mirrored.with(move |m| {
                Finding::start(m, generation, move |found| {
                    heard(Arrival {
                        generation,
                        heard: Heard::Found(Box::new(found)),
                    });
                })
            })
        });
        self.finding = Some(match started.flatten() {
            Some(Ok(f)) => f,
            Some(Err(e)) => {
                tracing::warn!(error = format!("{e:#}"), "the terminal cannot be searched");
                Finding::idle(generation)
            }
            None => Finding::idle(generation),
        });
        self.lit = None;
        ui.ask_paint();
    }

    /// What a key or a click did to the search.
    fn searched(&mut self, ui: &mut Ui, step: terminal_search::Step) {
        use terminal_search::{Order, Step as Sought};
        let Some(f) = &self.finding else { return };
        match step {
            Sought::Typed => {
                f.order(Order::Needle(f.needle.clone()));
                ui.ask_paint();
            }
            Sought::Older => f.order(Order::Older),
            Sought::Newer => f.order(Order::Newer),
            Sought::Closed => {
                self.finding = None;
                self.lit = None;
                self.refill = true;
                ui.ask_paint();
            }
            Sought::Kept => {}
        }
    }

    /// THE COMPOSITION, after a frame's rows were written: the program's
    /// cursor kept (`ime_at`), and what an input method is composing drawn
    /// there — not on a line the window's own line covers (`covered`).
    fn place_the_preedit(&mut self, palette: &Palette, covered: Option<u16>) {
        if let Some(c) = self.sheet.cursor {
            self.ime_at = Some((c.column, c.line));
        }
        if let (Some(preedit), Some(at)) = (&self.preedit, self.ime_at)
            && covered != Some(at.1)
        {
            draw_preedit(&mut self.sheet, preedit, at, palette);
        }
    }

    /// A keystroke that reached the program: the view goes back to the live
    /// screen, the selection goes, and the cursor shows again.
    fn typed_through(&mut self, ui: &mut Ui) {
        let Some(front) = self.group.front_mut() else {
            return;
        };
        let cleared = std::mem::take(&mut front.selected);
        let scrolled = front
            .mirrored
            .with(|m| {
                let back = !m.at_bottom();
                if back {
                    m.scroll(Scroll::Bottom);
                }
                if cleared {
                    m.select_clear();
                }
                back
            })
            .unwrap_or(false);
        if self.blink_off {
            self.blink_off = false;
            self.restart_blink();
        }
        if scrolled || cleared {
            self.view_moved(ui);
        }
    }

    /// The blink's clock, armed while the cursor blinks in a window with the
    /// keyboard — and only then (an event, not a clock): a window in the
    /// background, or a cursor that does not blink, wakes nothing.
    fn want_blink(&mut self, wanted: bool) {
        match (wanted, self.blink.is_some()) {
            (true, false) => self.arm_blink(),
            (false, true) => {
                if let (Some(token), Some(handle)) = (self.blink.take(), &self.handle) {
                    handle.remove(token);
                }
                self.blink_off = false;
            }
            _ => {}
        }
    }

    fn arm_blink(&mut self) {
        let Some(handle) = &self.handle else { return };
        let armed = handle.insert_source(
            calloop::timer::Timer::from_duration(BLINK),
            |_, (), door: &mut Door<Self>| {
                door.content.blink_off = !door.content.blink_off;
                door.content.frame_wanted(&mut door.ui);
                calloop::timer::TimeoutAction::ToDuration(BLINK)
            },
        );
        match armed {
            Ok(token) => self.blink = Some(token),
            Err(e) => tracing::warn!(error = %e, "the cursor's blink could not be armed"),
        }
    }

    /// The blink starts over, shown: a cursor that just moved is seen.
    fn restart_blink(&mut self) {
        if self.blink.is_some() {
            self.want_blink(false);
            self.arm_blink();
        }
    }

    /// What is drawn over the last row: the question, the search, or the
    /// door's message.
    fn line_over(&self, ui: &Ui) -> Option<Vec<Run>> {
        if let Some((_, q)) = &self.question {
            return Some(q.runs(&ui.texts, self.lit));
        }
        if let Some(f) = &self.finding {
            return Some(f.runs(&ui.texts, self.lit));
        }
        ui.message.as_deref().map(terminal_ask::said)
    }

    /// Whether the program in front tracks the mouse now — and the person
    /// is not holding Shift, which takes the pointer back for a selection.
    fn tracking(&self, ui: &Ui) -> bool {
        !ui.pointer_mods.shift
            && self
                .group
                .front()
                .and_then(|f| f.mirrored.with(|m| m.mouse_tracking()))
                .unwrap_or(false)
    }

    /// Tells the program in front, which tracks the mouse, what it did.
    #[allow(clippy::cast_possible_truncation)]
    fn report(
        &self,
        ui: &Ui,
        action: wire::MouseAction,
        button: Option<wire::Button>,
        at: (f64, f64),
        held: bool,
    ) {
        let (x, y) = self.placed.from_corner(at.0, at.1);
        if let Some(front) = self.group.front() {
            front.send(ToHolder::Mouse(wire::MouseEvent {
                action,
                button,
                mods: terminal_keys::mods_of(ui.pointer_mods),
                x_px: x as f32,
                y_px: y as f32,
                any_button_held: held,
            }));
        }
    }

    /// THE BUTTONS A PROGRAM WAS TOLD WENT DOWN, released as the pointer
    /// leaves: a release the window never heard (the button let go over
    /// another window) would leave the program thinking it is still held —
    /// a drag in an editor that never ends.
    fn release_reported(&self, ui: &Ui, pointing: &mut Pointing, at: (f64, f64)) {
        for button in std::mem::take(&mut pointing.reported) {
            self.report(
                ui,
                wire::MouseAction::Release,
                terminal_pointer::button_of(button),
                at,
                false,
            );
        }
    }

    /// The selection gesture's pointer at a point of the surface.
    fn gesture_at(&self, at: (f64, f64)) -> ghostty::Pointer {
        let (column, row) = self.placed.cell_near(at.0, at.1);
        let (x_px, y_px) = self.placed.from_corner(at.0, at.1);
        ghostty::Pointer {
            column,
            row,
            x_px,
            y_px,
        }
    }

    /// The strip's height, in logical pixels — nothing below two terminals.
    #[allow(clippy::cast_precision_loss)]
    fn strip_top(&self) -> f32 {
        terminal_strip::height(self.group.len()) as f32
    }

    /// THE BAR SHOWN, and its clock started over: it goes [`BAR_SHOWN`]
    /// after this, unless the pointer is on it then.
    fn show_bar(&mut self, ui: &mut Ui) {
        if !self.bar_shown {
            self.bar_shown = true;
            ui.ask_paint();
        }
        if let (Some(token), Some(handle)) = (self.bar_hide.take(), &self.handle) {
            handle.remove(token);
        }
        let Some(handle) = &self.handle else { return };
        let armed = handle.insert_source(
            calloop::timer::Timer::from_duration(BAR_SHOWN),
            |_, (), door: &mut Door<Self>| {
                let t = &mut door.content;
                if t.bar_hovered {
                    return calloop::timer::TimeoutAction::ToDuration(BAR_SHOWN);
                }
                t.bar_hide = None;
                t.bar_shown = false;
                door.ui.ask_paint();
                calloop::timer::TimeoutAction::Drop
            },
        );
        match armed {
            Ok(token) => self.bar_hide = Some(token),
            Err(e) => tracing::warn!(error = %e, "the scrollbar's pause could not be armed"),
        }
    }

    /// The pointer at `at`: on the bar's edge, the bar shows and stays; off
    /// it, its pause runs out.
    fn hover_bar(&mut self, ui: &mut Ui, at: (f64, f64)) {
        let on = self
            .bar_zone
            .is_some_and(|z| crate::bars::Track::of(z).holds(at.0, at.1));
        if on && !self.bar_hovered {
            self.bar_hovered = true;
            self.show_bar(ui);
        } else if !on && self.bar_hovered {
            self.bar_hovered = false;
            self.show_bar(ui);
        }
    }

    /// Whether `at` is on the bar's track, as the last frame drew it.
    fn on_the_bar(&self, at: (f64, f64)) -> bool {
        self.bar_track
            .is_some_and(|t| crate::bars::Track::of(t).holds(at.0, at.1))
    }

    /// The scrollbar grabbed at height `y`: the view goes to the row there —
    /// over the track [`terminal_pointer::thumb`] drew, under the strip, in
    /// the terminal's own rows.
    #[allow(clippy::cast_possible_truncation)]
    fn grab_the_bar(&mut self, ui: &mut Ui, y: f64) {
        #[allow(clippy::cast_precision_loss)]
        let h = ui.height as f32;
        let row = terminal_pointer::offset_at(
            self.scroll,
            self.strip_top(),
            h,
            self.placed.row_logical() as f32,
            y as f32,
        );
        if let Some(front) = self.group.front() {
            front.mirrored.with(|m| m.scroll(Scroll::Row(row)));
        }
        self.view_moved(ui);
        self.show_bar(ui);
    }

    /// Whether `at` is on the line drawn over the last row — a question or
    /// the search — and what a click there does.
    fn click_under(&self, at: (f64, f64)) -> Option<Option<Click>> {
        if self.question.is_none() && self.finding.is_none() {
            return None;
        }
        let (column, row) = self.placed.cell_at(at.0, at.1)?;
        (row + 1 == self.sheet.lines()).then(|| terminal_ask::click_at(&self.laid, column))
    }

    /// A click on a run of the line.
    fn clicked(&mut self, ui: &mut Ui, click: Click) {
        match click {
            Click::Answer(yes) => {
                let step = self.question.as_mut().map(|(_, q)| q.clicked(yes));
                match step {
                    Some(Step::Answered(yes)) => self.answered(ui, yes),
                    Some(Step::Moved | Step::Kept) => ui.ask_paint(),
                    None => {}
                }
            }
            Click::Older => self.searched(ui, terminal_search::Step::Older),
            Click::Newer => self.searched(ui, terminal_search::Step::Newer),
            Click::Close => self.searched(ui, terminal_search::Step::Closed),
        }
    }

    /// The wheel's whole lines, wherever they go.
    fn wheel(&mut self, ui: &mut Ui, lines: i64, at: (f64, f64)) {
        let Some(front) = self.group.front() else {
            return;
        };
        let (alternate, alt_scroll) = front
            .mirrored
            .with(|m| {
                let on = |mode| m.mode(mode).unwrap_or(false);
                (
                    on(Mode::ALT_SCREEN_SAVE) || on(Mode::dec(1047)) || on(Mode::dec(47)),
                    on(Mode::ALT_SCROLL),
                )
            })
            .unwrap_or((false, false));
        match terminal_pointer::wheel(lines, self.tracking(ui), alternate, alt_scroll) {
            Some(Wheel::Report(button, times)) => {
                for _ in 0..times {
                    self.report(ui, wire::MouseAction::Press, Some(button), at, false);
                }
            }
            Some(Wheel::Arrows(up, times)) => {
                let (code, key) = if up {
                    (103, "ArrowUp")
                } else {
                    (108, "ArrowDown")
                };
                for _ in 0..times {
                    front.send(ToHolder::Key(wire::KeyEvent {
                        code,
                        key: key.to_owned(),
                        action: wire::KeyAction::Press,
                        mods: wire::Mods::default(),
                        consumed: wire::Mods::default(),
                        unshifted: None,
                        utf8: String::new(),
                        composing: false,
                    }));
                }
            }
            Some(Wheel::View(by)) => {
                front.mirrored.with(|m| m.scroll(Scroll::Lines(by)));
                self.view_moved(ui);
                self.show_bar(ui);
            }
            None => {}
        }
    }

    /// THE LINK UNDER `at` while `Ctrl` is held: a hyperlink the program
    /// wrote, else an address written as text — on the program's rows only,
    /// never on the window's own line over the last.
    fn link_under(&self, at: (f64, f64)) -> Option<Link> {
        if !self.ctrl_held {
            return None;
        }
        let (column, row) = self.placed.cell_at(at.0, at.1)?;
        let rows = if self.laid.is_empty() {
            self.sheet.lines()
        } else {
            self.sheet.lines().saturating_sub(1)
        };
        if row >= rows {
            return None;
        }
        let columns = self.sheet.columns();
        let written = self.group.front().and_then(|f| {
            f.mirrored
                .with(|m| {
                    terminal_hyperlink::osc8_at(columns, rows, column, row, |c, r| m.link_at(c, r))
                })
                .flatten()
        });
        written.or_else(|| terminal_hyperlink::plain_at(&self.sheet, rows, column, row))
    }

    /// The link under the pointer, again — `Ctrl` went down or up, or the
    /// pointer or the screen moved.
    fn hover_link(&mut self, ui: &mut Ui) {
        let link = self.pointer_at.and_then(|at| self.link_under(at));
        if link != self.link {
            self.link = link;
            ui.ask_paint();
        }
    }

    /// `Ctrl` and a click on a link: opened by the opener every door opens
    /// a page with, in a scope of its own, off the loop — when its scheme is
    /// one this system opens. The address is the program's, and is not
    /// written in the log.
    fn open_link(&self, ui: &mut Ui, url: &str) {
        if !terminal_hyperlink::openable(url) {
            tracing::info!("a link with a scheme this system does not open was left");
            ui.say(ui.texts.t(crate::key::TERMINAL_LINK_REFUSED));
            return;
        }
        let Some(opener) = XDG_OPEN.filter(|p| !p.is_empty()) else {
            tracing::error!("RONOCTUA_XDG_OPEN was not set at build time");
            ui.say(ui.texts.t(crate::key::TERMINAL_LINK_NO_OPENER));
            return;
        };
        ui.launch(opener, &[url.to_owned()], None);
    }

    /// The strip's line under the pointer — lit, and its terminal's title
    /// lent to the window's for as long as the pointer rests there.
    fn hover_strip(&mut self, ui: &mut Ui, line: Option<usize>) {
        if self.hovered != line {
            self.hovered = line;
            self.retitle_later();
            ui.ask_paint();
        }
    }

    /// The strip's lines as the group is now.
    fn strip_lines(&self) -> Vec<terminal_strip::Line> {
        let front = self.group.front_index();
        self.group
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let (attention, progress) = asks(h);
                terminal_strip::Line {
                    front: i == front,
                    attention,
                    lit: self.hovered == Some(i) && i != front,
                    progress,
                }
            })
            .collect()
    }

    /// The window's title is to change — the terminal in front was renamed,
    /// another came to the front, a strip line is under the pointer. A
    /// title is the shell surface's, which the content does not hold, so it
    /// is set from the loop, once however many asked.
    fn retitle_later(&mut self) {
        if std::mem::replace(&mut self.retitle_owed, true) {
            return;
        }
        let Some(handle) = &self.handle else {
            self.retitle_owed = false;
            return;
        };
        handle.insert_idle(|door: &mut Door<Self>| {
            door.content.retitle_owed = false;
            retitle(door);
        });
    }
}

/// Whether `text` may be pasted without asking: nothing in it a shell would
/// run as it lands — or the program asked for bracketed paste (2004), where
/// a line break is text, and the text cannot close the bracket itself.
#[must_use]
pub fn paste_is_safe(text: &str, bracketed: bool) -> bool {
    ghostty::is_paste_safe(text) || (bracketed && !text.contains("\x1b[201~"))
}

/// A clipboard a program wrote (OSC 52), put there — text only; an image a
/// program writes is said in the log and left.
fn copy_for_the_program(write: &wire::ClipboardWrite) {
    let text = write.contents.iter().find_map(|c| {
        let mime = c.mime.to_ascii_lowercase();
        (mime.starts_with("text/plain")
            || matches!(mime.as_str(), "utf8_string" | "string" | "text"))
        .then(|| String::from_utf8(c.data.clone()).ok())
        .flatten()
    });
    match text {
        Some(text) => copy_off_the_loop(
            text,
            matches!(
                write.clipboard,
                wire::ClipboardKind::Primary | wire::ClipboardKind::Selection
            ),
        ),
        None => tracing::info!(
            kinds = write.contents.len(),
            "a program wrote a clipboard with no text in it; left as it was"
        ),
    }
}

/// WHAT IS OWED THE CLIPBOARD AND THE PRIMARY SELECTION: the latest text of
/// each, and whether a thread is copying them.
///
/// A program writes the clipboard as often as it prints (OSC 52), and every
/// copy is a `wl-copy` — a process. A thread a write would be a process a
/// write, as many at once as a loop in the program prints. Only the last
/// write of each matters, being the one the clipboard ends up holding, so
/// each waits in a slot of its own, and ONE thread copies what is there
/// until nothing is.
#[derive(Debug, Default)]
struct Owed {
    clipboard: Option<String>,
    primary: Option<String>,
    copying: bool,
}

impl Owed {
    /// `text` is owed, in place of what was. `true` when a thread is to be
    /// started for it — none is copying.
    fn owe(&mut self, text: String, primary: bool) -> bool {
        if primary {
            self.primary = Some(text);
        } else {
            self.clipboard = Some(text);
        }
        !std::mem::replace(&mut self.copying, true)
    }

    /// The next text to copy, and whether it is the primary selection's —
    /// `None` when nothing is owed, and the copying thread ends.
    fn next(&mut self) -> Option<(String, bool)> {
        let next = self
            .clipboard
            .take()
            .map(|t| (t, false))
            .or_else(|| self.primary.take().map(|t| (t, true)));
        self.copying = next.is_some();
        next
    }
}

/// The process's one [`Owed`]: a terminal's window is a process of its own.
static OWED: Mutex<Owed> = Mutex::new(Owed {
    clipboard: None,
    primary: None,
    copying: false,
});

fn owed() -> std::sync::MutexGuard<'static, Owed> {
    OWED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Copies `text` onto the clipboard (or the primary selection), on a thread:
/// `wl-copy` is a program, and a window never waits for one — and a copy
/// owed while another runs takes that thread's next turn ([`Owed`]).
fn copy_off_the_loop(text: String, primary: bool) {
    if !owed().owe(text, primary) {
        return;
    }
    let born = std::thread::Builder::new()
        .name("term-copy".to_owned())
        .spawn(|| {
            loop {
                // THE LOCK GOES AT THE END OF THIS STATEMENT, and not of the
                // copy: a `while let` would hold it through `wl-copy`, and
                // the window's next copy would wait on it in the loop.
                let Some((text, primary)) = owed().next() else {
                    break;
                };
                let copied = if primary {
                    crate::clipboard_copy::copy_to_primary(&text)
                } else {
                    crate::clipboard_copy::copy_to_clipboard(&text)
                };
                if let Err(e) = copied {
                    tracing::warn!(error = %e, primary, "the terminal could not copy");
                }
            }
        });
    if let Err(e) = born {
        owed().copying = false;
        tracing::warn!(error = %e, "no thread to copy on");
    }
}

/// What the threads heard, into the window.
fn arrived(door: &mut Door<TerminalWindow>, a: Arrival) {
    door.content.hear(&mut door.ui, a);
}

/// The window's title, set on the shell surface: the terminal in front's
/// — or, while the pointer rests on a strip's line, that line's.
fn retitle(door: &mut Door<TerminalWindow>) {
    let title = Content::title(&door.content, &door.ui.texts);
    if let Shell::Window { window, .. } = &door.shell {
        window.set_title(title);
    }
}

impl Drop for TerminalWindow {
    /// The window going: a terminal of its own process goes with it, a held
    /// one to the background (its connection closes with this process) —
    /// every terminal of the group alike.
    fn drop(&mut self) {
        for held in self.group.iter() {
            if let Some(reach) = &held.reach {
                reach.window_gone();
            }
        }
    }
}

impl Content for TerminalWindow {
    type Hand = Pointing;

    fn app_id(&self) -> &'static str {
        self.identity.app_id()
    }

    /// What the program in front called the window — or the one whose strip
    /// line the pointer rests on — or the identity's own name.
    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        let held = self
            .hovered
            .and_then(|i| self.group.get(i))
            .or_else(|| self.group.front());
        match held.map(Held::title).filter(|t| !t.is_empty()) {
            Some(title) => title.to_owned(),
            None => texts.t(self.identity.title_key()),
        }
    }

    /// One key, from a press or from a REPEAT.
    ///
    /// In a terminal the repeat IS the right behaviour: holding the key
    /// deletes several characters, goes down several lines. Unlike the
    /// footer, where repeating a leader key is always a mistake.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        self.mods = mods;
        // `CTRL` DOWN SHOWS THE LINK under a pointer that does not move.
        if matches!(event.keysym, Keysym::Control_L | Keysym::Control_R) {
            self.ctrl_held = true;
            self.hover_link(ui);
        }
        if let Some((_, q)) = &mut self.question {
            match q.key(event.keysym) {
                Step::Moved => ui.ask_paint(),
                Step::Answered(yes) => self.answered(ui, yes),
                Step::Kept => {}
            }
            return;
        }
        if let Some(f) = &mut self.finding {
            let step = f.key(event, mods);
            self.searched(ui, step);
            return;
        }
        if let Some(kept) = terminal_keys::kept(event.keysym, mods).filter(|k| self.keeps(*k)) {
            self.keep(ui, kept);
            return;
        }
        let Some(front) = self.group.front() else {
            return;
        };
        if front.link.is_none() {
            return;
        }
        let action = if ui.repeating {
            wire::KeyAction::Repeat
        } else {
            wire::KeyAction::Press
        };
        let frame = terminal_keys::frame_of(event, mods, action);
        front.send(ToHolder::Key(frame.clone()));
        self.pressed.insert(event.raw_code, frame);
        // A MODIFIER ALONE is not a keystroke: Shift held for `S-PgUp` must
        // not first bring the view back to the bottom, nor hide the pointer.
        if !event.keysym.is_modifier_key() {
            // THE POINTER HIDES while somebody types, so it does not sit on
            // the letters being typed — and comes back as it moves.
            self.hid_the_pointer = true;
            self.typed_through(ui);
        }
        // It does NOT redraw here. The echo comes from the program through
        // the holder, and drawing now would show the screen from BEFORE the
        // key — a late frame that only shows up when one types fast.
    }

    /// A key came up: the program in front hears it if it heard it go down
    /// (kitty's release events; the holder's encoder sends one only to a
    /// program that asked) — and `Ctrl` up puts the link away.
    fn released(&mut self, ui: &mut Ui, event: &KeyEvent) {
        if matches!(event.keysym, Keysym::Control_L | Keysym::Control_R) {
            self.ctrl_held = false;
            self.hover_link(ui);
        }
        if self.pressed.remove(&event.raw_code).is_some()
            && let Some(front) = self.group.front()
        {
            front.send(ToHolder::Key(terminal_keys::frame_of(
                event,
                self.mods,
                wire::KeyAction::Release,
            )));
        }
    }

    /// The clipboard's (or the primary selection's) text — or files dropped,
    /// typed as their paths: onto the search's needle while it is up,
    /// pasted, or asked about first when it could run a command as it lands.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        self.paste_from(ui, text, wire::PasteSource::Clipboard);
    }

    /// A TERMINAL TYPES TEXT, and there Caps Lock is capitals: what reaches
    /// the program is what the keyboard typed. A chord is still read without
    /// the Lock (`door::keys::as_read`).
    fn types_capitals(&self) -> bool {
        true
    }

    /// THE HEADER'S KEYS ARE THE PROGRAM'S. This window draws no header, and
    /// the right `Ctrl` and `Esc` it would take are keys a program running
    /// here is owed — `Esc` above all, which is half of what vim is.
    fn header_keys(&self) -> bool {
        false
    }

    /// THE MOUSE'S BACK BUTTON IS THE PROGRAM'S when it tracks the mouse,
    /// and nobody's otherwise: a terminal has no view to go back from.
    fn back_is_esc(&self) -> bool {
        false
    }

    /// `space.terminal` CELLS, not pixels: the grid every program assumes
    /// until a `SIGWINCH` says otherwise, at the terminal's size of text.
    ///
    /// AT THE SCALE THE CELL WAS MEASURED AT (`Geometry::scale`): the cell is
    /// physical pixels, one a pixel until the window is laid out on a screen.
    fn default_size(&self) -> (u32, u32) {
        terminal_pointer::first_size(&self.geometry)
    }

    /// THE GROUND IS THE TERMINAL'S: the page's `terminal.background`, or
    /// what a program made it (OSC 11).
    fn clear(&self) -> [f32; 4] {
        self.background.to_linear()
    }

    fn sheet(&self) -> Option<&Sheet> {
        Some(&self.sheet)
    }

    /// NO SWEEP OF THE DOOR'S over the grid: the terminal selects its own
    /// text, history included, and a sweep would take `Esc` and `C-c` from
    /// the program while it lasted.
    fn text_area(&self, _: &Ui, _: u16, _: u16) -> Option<crate::text_select::Area> {
        None
    }

    /// The arrow over the strip, the margins and the bar; nothing over the
    /// grid while somebody types; the hand over a link `Ctrl` shows; the
    /// I-beam over the grid where a press selects, and the arrow over one
    /// whose program tracks the mouse.
    fn cursor_at(&self, ui: &Ui, at: (f64, f64)) -> crate::cursor_shape::Look {
        use crate::cursor_shape::Look;
        #[allow(clippy::cast_possible_truncation)]
        let on_the_bar = ui.room.on_the_bar(at.0 as f32);
        let cell = self.placed.cell_at(at.0, at.1);
        if at.1 < f64::from(self.strip_top()) || on_the_bar || cell.is_none() {
            return Look::Arrow;
        }
        if self.hid_the_pointer {
            return Look::Hidden;
        }
        if let (Some(link), Some((column, row))) = (&self.link, cell)
            && link.covers(column, row)
        {
            return Look::Pointer;
        }
        if self.tracking(ui) {
            Look::Arrow
        } else {
            Look::Text
        }
    }

    /// AN INPUT METHOD COMPOSES AT THE CURSOR of the terminal in front — the
    /// cell the program left it on, where the input method opens its
    /// guesses. None while the window's own line asks (a question, the
    /// search: it reads key by key, and a composition would be drawn on the
    /// grid and not on the line), none before a terminal is reached, and
    /// none ever for the monitor and the rescue console: they are the
    /// windows that must work when nothing else does, and an input method
    /// that hung would keep every key from them.
    fn text_input(&self, _: &Ui) -> Option<Field> {
        if self.identity != Identity::Terminal || self.question.is_some() || self.finding.is_some()
        {
            return None;
        }
        self.group.front()?.link.as_ref()?;
        let (column, line) = self.ime_at.unwrap_or((0, 0));
        Some(Field {
            purpose: Purpose::Terminal,
            cursor: cursor_area(&self.placed, column, line),
        })
    }

    /// The composition, drawn at the cursor from the next frame on — every
    /// row written again, so the one it leaves is the program's once more.
    ///
    /// COMPOSING IS TYPING: a composition brings the view back to the live
    /// screen, takes the selection away and hides the pointer, as a key
    /// does — the keys that compose it never reach the window, so nothing
    /// else would. Over a screen scrolled back the program's cursor is not
    /// in sight, and the composition would sit on the history at the cell
    /// the cursor last had, the input method's guesses opened beside it.
    fn preedit(&mut self, ui: &mut Ui, preedit: Option<Preedit>) {
        if preedit == self.preedit {
            return;
        }
        let composing = preedit.is_some();
        self.preedit = preedit;
        self.view_moved(ui);
        if composing {
            self.hid_the_pointer = true;
            self.typed_through(ui);
        }
    }

    /// TEXT AN INPUT METHOD COMMITTED, typed: one `Text` frame to the
    /// program in front — never keys, which the holder would encode for the
    /// program's keyboard protocol as if each character had been pressed —
    /// and, as a keystroke does, the view back at the bottom and the pointer
    /// hidden. A commit that comes after the window's own line opened (a
    /// batch on its way as the search was asked for) goes to the search's
    /// needle, and to nothing while a question waits for its yes or no.
    fn committed(&mut self, ui: &mut Ui, text: &str) {
        if text.is_empty() || self.question.is_some() {
            return;
        }
        if let Some(f) = &mut self.finding {
            let step = f.paste(text);
            self.searched(ui, step);
            return;
        }
        let Some(front) = self.group.front().filter(|f| f.link.is_some()) else {
            return;
        };
        front.send(ToHolder::Text(text.to_owned()));
        self.hid_the_pointer = true;
        self.typed_through(ui);
    }

    /// THE WINDOW'S SHORTCUTS — `LEADER Alt`'s list, the footer's line:
    /// the group's commands of [`terminal_keys::GROUP`], a new terminal
    /// always and the walk and the detach from two terminals up (with one
    /// there is nothing to walk to, and nothing to leave), each on its ONE key
    /// after `LEADER Alt` ([`terminal_keys::LEADER_ALT_KEYS`]) — the chords
    /// are this window's, and the footer hands the command back by its id.
    /// The rest of the terminal's keys are Ghostty's and the program's,
    /// taught nowhere.
    fn shortcuts(&self, _: &Ui) -> Vec<ronoctua_protocol::Binding> {
        if self.identity != Identity::Terminal {
            return Vec::new();
        }
        let many = self.group.len() >= terminal_strip::FEWEST;
        terminal_keys::GROUP
            .iter()
            .filter(|(_, _, g)| many || *g == Grouped::New)
            .filter_map(|(keys, id, _)| {
                let direct = ronoctua_keymap::Key::parse(keys).ok()?;
                let key =
                    crate::chrome::leader_alt_key(&direct, &[terminal_keys::LEADER_ALT_KEYS])?;
                Some(ronoctua_protocol::Binding {
                    keys: key.notation(),
                    command: (*id).to_owned(),
                })
            })
            .collect()
    }

    /// One of the group's commands, by its id — what a click on the
    /// shortcut, the footer's `LEADER :` and the key all do.
    fn run_shortcut(&mut self, ui: &mut Ui, command: &str) -> bool {
        match terminal_keys::grouped(command) {
            Some(grouped) if self.identity == Identity::Terminal => {
                self.grouped(ui, grouped);
                true
            }
            _ => false,
        }
    }

    /// The window was given a new size, a new scale or a new size of text.
    ///
    /// A terminal's grid IS its content, so a cell that stopped matching its
    /// glyph is not cosmetic here: the terminals are resized in CELLS, and
    /// the mismatch would be a wrong number of columns handed to whatever
    /// runs.
    fn resized(&mut self, ui: &mut Ui) {
        self.lay_out(ui);
    }

    fn keyboard_entered(&mut self, ui: &mut Ui) {
        self.focused = true;
        if let Some(front) = self.group.front() {
            front.send(ToHolder::Focus(wire::Focus { gained: true }));
        }
        self.place_all();
        ui.ask_paint();
    }

    fn keyboard_left(&mut self, ui: &mut Ui) {
        self.focused = false;
        self.pressed.clear();
        self.ctrl_held = false;
        self.link = None;
        if let Some(front) = self.group.front() {
            front.send(ToHolder::Focus(wire::Focus { gained: false }));
        }
        self.place_all();
        ui.ask_paint();
    }

    /// A frame wanted while one was in flight is handed to the door on the
    /// turn the compositor showed that one (the door clears the flight
    /// before this turn ends), and never twice.
    fn each_turn(&mut self, ui: &mut Ui) {
        if !ui.frame_in_flight() && std::mem::take(&mut self.owed) {
            ui.ask_paint();
        }
    }

    /// THE DATA DEVICE IS THE SEAT'S, and so cannot be made before one
    /// arrives — what a drop over the window is heard through. Asked on every
    /// capability, which is harmless: `took_a_seat` makes one device per seat.
    fn seat_capability(
        door: &mut Door<Self>,
        qh: &smithay_client_toolkit::reexports::client::QueueHandle<Door<Self>>,
        seat: &smithay_client_toolkit::reexports::client::protocol::wl_seat::WlSeat,
    ) {
        if let Some(c) = door.content.carrying.as_mut() {
            c.took_a_seat(qh, seat);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        // ONE FRAME IN FLIGHT: the next one is drawn when this one is shown
        // (`Content::frame_done`), so a flood of output never queues frames
        // on the loop's thread, and a hidden window draws nothing.
        ui.pace();
        let palette = Palette::of_the_page();
        let page_moved = self.palette.as_ref() != Some(&palette);
        if page_moved {
            // A THEME, or the windows moved to a desk: every terminal's
            // engine is given the page's colours, which come back into every
            // cell.
            self.palette = Some(palette.clone());
            self.sync_colors();
        }
        let over = self.line_over(ui);
        // WHAT IS DRAWN OVER THE PROGRAM'S CELLS — the search's matches, the
        // link `Ctrl` shows — and when it moved, every row is written again
        // from the engine first, so what it left behind goes.
        let matches = self
            .finding
            .as_ref()
            .filter(|f| f.found.needle == f.needle)
            .map(|f| terminal_search::Shown {
                matches: f.found.on_screen.clone(),
                current: f.found.current,
            })
            .unwrap_or_default();
        let marks_moved = matches != self.marked || self.link != self.underlined;
        let showing = terminal_sheet::Showing {
            focused: self.focused,
            blink_off: self.blink_off,
            // EVERY ROW WHILE SOMETHING IS COMPOSED: the composition is drawn
            // over the program's cells, and the row it was drawn on last
            // frame is written back from the engine whatever moved since.
            all: std::mem::take(&mut self.refill)
                || page_moved
                || marks_moved
                || self.preedit.is_some()
                || (over.is_none() && !self.laid.is_empty()),
            faint: faint_alpha(),
        };
        let mirrored = self.group.front().map(|f| Arc::clone(&f.mirrored));
        let filled = match mirrored {
            Some(m) if m.update() => {
                let mut frame = m.frame();
                Some(terminal_sheet::fill(
                    &mut self.sheet,
                    &mut frame,
                    &palette,
                    showing,
                ))
            }
            _ => None,
        };
        if filled.is_some() {
            terminal_search::marks(&mut self.sheet, &matches, &palette);
            self.marked = matches;
            if let Some(link) = &self.link {
                terminal_hyperlink::underline(&mut self.sheet, link);
            }
            self.underlined.clone_from(&self.link);
            let covered = over.is_some().then(|| self.sheet.lines().saturating_sub(1));
            self.place_the_preedit(&palette, covered);
        } else if marks_moved {
            // Nothing was written this frame: what moved is drawn on the next.
            self.refill = true;
        }
        self.background = filled.map_or(palette.background, |f| f.background);
        let last = self.sheet.lines().saturating_sub(1);
        match over {
            Some(runs) => {
                self.laid = terminal_ask::laid(&runs, self.sheet.columns());
                terminal_ask::lay_line(
                    &mut self.sheet,
                    last,
                    &self.laid,
                    Rgba::of(tokens::color::SURFACE_CHROME),
                );
                // The cursor does not show through the window's line.
                if self.sheet.cursor.is_some_and(|c| c.line == last) {
                    self.sheet.cursor = None;
                }
            }
            None => self.laid.clear(),
        }
        self.want_blink(filled.is_some_and(|f| f.blinks));
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
        let (w, h, row) = (
            ui.width as f32,
            ui.height as f32,
            self.placed.row_logical() as f32,
        );
        // THE GROUP'S STRIP, from two terminals up (`crate::terminal_strip`).
        if let Some(strip) = Strip::of(ui.width, self.group.len()) {
            canvas.under.extend(strip.quads(&self.strip_lines()));
        }
        // THE BAR, OVER THE GRID'S RIGHT EDGE and under the strip, while there
        // is history to scroll AND it is used (the owner's exception,
        // 2026-11-03: it takes the program no room, so it never changes its
        // width) — on the top layer, over the cells; grabbed where it is
        // drawn (`pointer`), its track counted in the terminal's rows
        // (`terminal_pointer::bar`).
        let bar = self
            .group
            .front()
            .and_then(|f| f.mirrored.with(|m| m.scrollbar()))
            .unwrap_or_default();
        self.scroll = terminal_pointer::scroll_of(bar);
        let (zone, thumb) = terminal_pointer::bar(self.scroll, w, self.strip_top(), h, row);
        self.bar_zone = zone;
        let shown = self.bar_shown && zone.is_some();
        // THE ROOM HAS A BAR only while it is drawn: hidden, its column is
        // the grid's, under the pointer as under the eye.
        ui.room = if shown {
            chrome::Room::of(self.scroll, w, h)
        } else {
            chrome::Room {
                width: w,
                bar_from: None,
            }
        };
        let track = zone.filter(|_| shown);
        canvas
            .over
            .extend(thumb.filter(|_| shown).map(chrome::thumb));
        canvas.bar(track.map(crate::bars::Track::of));
        self.bar_track = track;
        // THE FIRST FRAME, said out loud — `terminal-vm` waits for it: a
        // terminal that drew is told from one that never did.
        if filled.is_some() && !std::mem::replace(&mut self.drawn, true) {
            tracing::info!(
                columns = self.sheet.columns(),
                rows = self.sheet.lines(),
                "terminal drawn on the xdg_toplevel"
            );
        }
    }

    /// One pointer's frame: the strip, the bar, the line's answers, a link
    /// under `Ctrl`, the program's mouse when it tracks one, and the
    /// terminal's own selection when it does not.
    #[allow(clippy::too_many_lines)]
    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<Pointing>, events: &[PointerEvent]) {
        let strip = Strip::of(ui.width, self.group.len());
        let strip_bottom = f64::from(self.strip_top());
        for e in events {
            let at = e.position;
            let on_strip = at.1 < strip_bottom;
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pointer_at = Some(at);
                    self.hid_the_pointer = false;
                    self.ctrl_held = ui.pointer_mods.ctrl;
                    if hand.extra.on_the_bar {
                        self.grab_the_bar(ui, at.1);
                        continue;
                    }
                    self.hover_bar(ui, at);
                    let busy = hand.extra.selecting || !hand.extra.reported.is_empty();
                    let line = if busy {
                        None
                    } else {
                        strip.and_then(|s| s.line_at(at.0, at.1))
                    };
                    self.hover_strip(ui, line);
                    let lit = self.click_under(at).flatten();
                    if lit != self.lit {
                        self.lit = lit;
                        ui.ask_paint();
                    }
                    if hand.extra.selecting {
                        let rectangle = ui.pointer_mods.ctrl && ui.pointer_mods.alt;
                        let pointer = self.gesture_at(at);
                        let engine = self.placed.engine();
                        let moved = self.group.front().is_some_and(|f| {
                            f.mirrored
                                .with(|m| {
                                    m.select_drag(pointer, rectangle, &engine).unwrap_or(false)
                                })
                                .unwrap_or(false)
                        });
                        if moved {
                            if let Some(front) = self.group.front_mut() {
                                front.selected = true;
                            }
                            self.view_moved(ui);
                        }
                    } else if !on_strip
                        && self.tracking(ui)
                        && self.placed.cell_at(at.0, at.1).is_some()
                    {
                        let held = hand
                            .extra
                            .reported
                            .first()
                            .and_then(|b| terminal_pointer::button_of(*b));
                        self.report(ui, wire::MouseAction::Motion, held, at, held.is_some());
                    }
                    self.hover_link(ui);
                }
                PointerEventKind::Leave { .. } => {
                    hand.extra.on_the_bar = false;
                    self.bar_hovered = false;
                    self.hover_strip(ui, None);
                    if self.lit.take().is_some() {
                        ui.ask_paint();
                    }
                    self.pointer_at = None;
                    if self.link.take().is_some() {
                        ui.ask_paint();
                    }
                    self.release_reported(ui, &mut hand.extra, at);
                    // A SELECTION STILL FOLLOWING THE POINTER ends where it
                    // is: the release went to another surface, and a pointer
                    // that comes back with no button held is not dragging.
                    if std::mem::take(&mut hand.extra.selecting)
                        && let Some(front) = self.group.front()
                    {
                        front.mirrored.with(|m| {
                            let _ = m.select_release(None);
                        });
                    }
                }
                PointerEventKind::Press { button, time, .. } => {
                    if on_strip {
                        // THE FIRST CLICK FRONTS: being in front is what a
                        // strip's line is chosen for — the owner's exception
                        // to invariant 4 says so, and the strip's head too.
                        if button == LEFT_BUTTON
                            && let Some(line) = strip.and_then(|s| s.line_at(at.0, at.1))
                        {
                            self.front_line(ui, line);
                        }
                        continue;
                    }
                    if button == LEFT_BUTTON && self.on_the_bar(at) {
                        hand.extra.on_the_bar = true;
                        self.grab_the_bar(ui, at.1);
                        continue;
                    }
                    if let Some(click) = self.click_under(at) {
                        if let Some(click) = click {
                            self.clicked(ui, click);
                        }
                        continue;
                    }
                    if self.placed.cell_at(at.0, at.1).is_none() {
                        continue;
                    }
                    // `CTRL` AND A CLICK ON A LINK opens it — over a program
                    // that tracks the mouse too: the modifier is the person's,
                    // as Shift taking the pointer back for a selection is.
                    if button == LEFT_BUTTON && ui.pointer_mods.ctrl {
                        self.ctrl_held = true;
                        if let Some(link) = self.link_under(at) {
                            self.open_link(ui, &link.url);
                            continue;
                        }
                    }
                    if self.tracking(ui)
                        && let Some(b) = terminal_pointer::button_of(button)
                    {
                        hand.extra.reported.push(button);
                        self.report(ui, wire::MouseAction::Press, Some(b), at, true);
                        continue;
                    }
                    match button {
                        LEFT_BUTTON => {
                            let mods = ui.pointer_mods;
                            let pointer = self.gesture_at(at);
                            let cell = f64::from(self.placed.cell.0);
                            let selected = self.group.front().and_then(|f| {
                                f.mirrored.with(|m| {
                                    if mods.shift {
                                        m.select_extend(pointer.column, pointer.row)
                                    } else {
                                        m.select_press(
                                            pointer,
                                            u64::from(time) * 1_000_000,
                                            ghostty::Repeat {
                                                distance_px: cell,
                                                interval_ns: CLICK_INTERVAL_NS,
                                            },
                                            mods.ctrl,
                                        )
                                    }
                                })
                            });
                            if let Some(front) = self.group.front_mut() {
                                front.selected = matches!(selected, Some(Ok(true)));
                            }
                            hand.extra.selecting = true;
                            self.view_moved(ui);
                        }
                        MIDDLE_BUTTON => ui.paste_primary(),
                        _ => {}
                    }
                }
                PointerEventKind::Release { button, .. } => {
                    if button == LEFT_BUTTON && std::mem::take(&mut hand.extra.on_the_bar) {
                        continue;
                    }
                    if let Some(i) = hand.extra.reported.iter().position(|b| *b == button) {
                        hand.extra.reported.remove(i);
                        let held = !hand.extra.reported.is_empty();
                        self.report(
                            ui,
                            wire::MouseAction::Release,
                            terminal_pointer::button_of(button),
                            at,
                            held,
                        );
                        continue;
                    }
                    if button == LEFT_BUTTON && std::mem::take(&mut hand.extra.selecting) {
                        let cell = self.placed.cell_at(at.0, at.1);
                        let text = self
                            .group
                            .front()
                            .and_then(|f| {
                                f.mirrored.with(|m| {
                                    let _ = m.select_release(cell);
                                    m.selected_text()
                                })
                            })
                            .flatten();
                        // COPY ON SELECT, where the person said
                        // (`terminal.json`): by default onto the PRIMARY
                        // selection — what a middle click pastes — and not
                        // the clipboard: selecting is not copying.
                        if let Some(text) = text.filter(|t| !t.is_empty()) {
                            if let Some(front) = self.group.front_mut() {
                                front.selected = true;
                            }
                            let (clipboard, primary) = self.settings.copy_on_select.targets();
                            if clipboard {
                                copy_off_the_loop(text.clone(), false);
                            }
                            if primary {
                                copy_off_the_loop(text, true);
                            }
                        }
                        self.view_moved(ui);
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    if vertical.stop {
                        hand.lines_owed = 0.0;
                    }
                    if horizontal.stop {
                        hand.extra.across_owed = 0.0;
                    }
                    hand.lines_owed +=
                        crate::wheel::axis_lines(&vertical, self.placed.row_logical());
                    let lines = crate::wheel::wheel_lines(&mut hand.lines_owed);
                    // THE WHEEL OVER THE STRIP walks the group, a terminal a
                    // line of the wheel.
                    if on_strip {
                        for _ in 0..lines.unsigned_abs() {
                            self.step_front(ui, if lines < 0 { -1 } else { 1 });
                        }
                        continue;
                    }
                    self.wheel(ui, lines, at);
                    // SIDEWAYS is the program's alone, and only when it
                    // tracks the mouse: the history has no sideways.
                    if self.tracking(ui) {
                        let cell = f64::from(self.placed.cell.0) / f64::from(self.placed.scale);
                        hand.extra.across_owed += crate::wheel::axis_lines(&horizontal, cell);
                        let across = crate::wheel::wheel_lines(&mut hand.extra.across_owed);
                        let button = if across < 0 {
                            wire::Button::WheelLeft
                        } else {
                            wire::Button::WheelRight
                        };
                        for _ in 0..across.unsigned_abs() {
                            self.report(ui, wire::MouseAction::Press, Some(button), at, false);
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FILES DROPPED ON THE TERMINAL
// ---------------------------------------------------------------------------
//
// The drag's three handlers are the door's, written once in `crate::carry` —
// the one file that speaks `wl_data_device` (invariant 3) — and they hand
// this window what they heard. A list of files is taken anywhere on the
// window, always as a COPY (a move would tell the program it came from that
// it may delete what was dropped), its list read off the loop, and typed as
// the paths (`crate::terminal_drop`).

impl crate::carry::Carries for TerminalWindow {
    fn carried(door: &mut Door<Self>, heard: crate::carry::Heard<'_>) {
        use crate::carry::Heard as Carried;
        use smithay_client_toolkit::reexports::client::Proxy as _;
        match heard {
            Carried::Entered { device, .. } => {
                let Some(offer) = device
                    .data::<DataDeviceData>()
                    .and_then(DataDeviceData::drag_offer)
                else {
                    return;
                };
                let takeable = offer
                    .with_mime_types(|kinds| kinds.iter().any(|k| k == crate::carry::URI_LIST));
                offer.accept_mime_type(
                    offer.serial,
                    takeable.then(|| crate::carry::URI_LIST.to_owned()),
                );
                if takeable {
                    offer.set_actions(DndAction::Copy, DndAction::Copy);
                }
            }
            Carried::Dropped { device } => {
                let Some(offer) = device
                    .data::<DataDeviceData>()
                    .and_then(DataDeviceData::drag_offer)
                else {
                    return;
                };
                match offer.receive(crate::carry::URI_LIST.to_owned()) {
                    Ok(pipe) if door.content.wires.is_some() => {
                        // FINISHED WHEN ITS LIST ARRIVES (`dropped`): the
                        // drop is the source's until it has written it.
                        if let Some(earlier) = door.content.dropping.replace(offer) {
                            earlier.finish();
                        }
                        door.content.read_the_drop(pipe);
                    }
                    _ => offer.finish(),
                }
            }
            // NOTHING CHANGES WITH THE POINT: files are taken anywhere on the
            // window, and a drag of this window's own does not exist.
            Carried::Left
            | Carried::Moved { .. }
            | Carried::Chosen { .. }
            | Carried::Asked { .. }
            | Carried::Cancelled
            | Carried::Finished => {}
        }
    }
}

/// The cell `(column, line)` of the grid on the surface, in LOGICAL pixels —
/// what an input method opens its guesses beside.
fn cursor_area(placed: &Placed, column: u16, line: u16) -> LogicalRect {
    let scale = placed.scale.max(1);
    let x = placed.origin.0 + u32::from(column) * placed.cell.0;
    let y = placed.origin.1 + u32::from(line) * placed.cell.1;
    let logical = |v: u32| i32::try_from(v).unwrap_or(i32::MAX);
    LogicalRect {
        x: logical(x / scale),
        y: logical(y / scale),
        width: logical(placed.cell.0.div_ceil(scale)),
        height: logical(placed.cell.1.div_ceil(scale)),
    }
}

/// WHAT AN INPUT METHOD IS COMPOSING, drawn over the grid from the program's
/// cursor `at` (`(column, line)`): a grapheme a cell — two for a wide one —
/// in the terminal's own ink on its own ground, underlined, as every
/// terminal draws a composition; the part being converted in the selection's
/// colours. Over the cells the program drew there, which the next frame
/// writes back (`Showing::all` while one is composed). Never sent: only the
/// commit reaches the program.
///
/// IT FITS ON THE LINE: a composition that would run past the last column
/// starts earlier, so its end — where one types — stays in sight, and one
/// wider than the line shows its start. A wide character the composition
/// cuts in half, on either side, is blanked rather than drawn as half of
/// itself. The terminal's cursor moves to the composition's own, and hides
/// with it.
fn draw_preedit(sheet: &mut Sheet, preedit: &Preedit, at: (u16, u16), palette: &Palette) {
    let columns = sheet.columns();
    let (column, line) = at;
    if line >= sheet.lines() || columns == 0 {
        return;
    }
    let clusters: Vec<(usize, &str, u16)> = ronoctua_protocol::cells_of(&preedit.text)
        .scan(0usize, |offset, g| {
            let at = *offset;
            *offset += g.len();
            Some((at, g, ronoctua_protocol::cells_of_one(g)))
        })
        .collect();
    let wide: u16 = clusters
        .iter()
        .fold(0u16, |n, (_, _, w)| n.saturating_add(*w));
    let start = column.min(columns.saturating_sub(wide));
    let converting = preedit.cursor.clone().filter(|r| !r.is_empty());
    let blank = sheet.blank();
    let mut cursor_column = None;
    let row = sheet.row_mut(line);
    let mut x = start;
    for &(offset, g, w) in &clusters {
        if preedit.cursor.as_ref().is_some_and(|r| r.start == offset) {
            cursor_column = Some(x);
        }
        if x.saturating_add(w) > columns {
            break;
        }
        let selected = converting.as_ref().is_some_and(|r| r.contains(&offset));
        let (fg, bg) = if selected {
            (
                palette.selection_foreground,
                Some(palette.selection_background),
            )
        } else {
            (palette.foreground, None)
        };
        let cell = Cell {
            grapheme: Grapheme::of(g),
            fg,
            bg,
            underline_colour: None,
            style: cells::Style {
                underline: cells::Underline::Single,
                ..cells::Style::default()
            },
            width: if w == 2 { Width::Wide } else { Width::Narrow },
        };
        if w == 2 {
            row[usize::from(x) + 1] = Cell {
                width: Width::Spacer,
                ..cell.clone()
            };
        }
        row[usize::from(x)] = cell;
        x += w;
    }
    if preedit
        .cursor
        .as_ref()
        .is_some_and(|r| r.start == preedit.text.len())
    {
        cursor_column = Some(x.min(columns.saturating_sub(1)));
    }
    // HALVES THE COMPOSITION CUT, blanked: a wide character whose tail it
    // covers, before it, and one whose head it covers, after it.
    if let Some(before) = usize::from(start).checked_sub(1)
        && row[before].width == Width::Wide
    {
        row[before] = blank.clone();
    }
    if let Some(after) = row.get_mut(usize::from(x))
        && after.width == Width::Spacer
    {
        *after = blank;
    }
    if let Some(c) = sheet.cursor.as_mut() {
        match cursor_column {
            Some(at) => (c.column, c.line) = (at, line),
            None => sheet.cursor = None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn words(w: &[&str]) -> Vec<String> {
        w.iter().map(|s| (*s).to_owned()).collect()
    }

    /// A web app never takes the leader: the rule is exactly the compositor's
    /// grammar, it names the app windows by their prefix, and it reaches the
    /// export beside the floating ones.
    #[test]
    fn a_chrome_app_window_cannot_inhibit_the_shortcuts() {
        // FAILS IF the rule's grammar or prefix changes, or the export loses
        // either half.
        let rule = super::inhibitor_rules();
        assert_eq!(
            rule,
            "for_window [app_id=\"^chrome-\"] shortcuts_inhibitor disable\n"
        );
        assert!(!"google-chrome".starts_with(crate::daemon::CHROME_APP_PREFIX));
        assert!("chrome-web.whatsapp.com__-Default".starts_with(crate::daemon::CHROME_APP_PREFIX));
        let all = super::window_rules();
        assert!(
            all.contains(&rule),
            "the export lost the inhibitor rule:\n{all}"
        );
        assert!(
            all.contains(&crate::floating::rules()),
            "the export lost the floating rules:\n{all}"
        );
        assert!(all.lines().all(|l| l.starts_with("for_window [")), "{all}");
    }

    #[test]
    fn a_program_and_its_own_flags_are_the_programs() {
        // FAILS IF a program's arguments are read as the door's (`htop -d
        // 5`, `pkexec --keep-cwd`), `--` does not end the flags, or the
        // terminal is not held unless `--here` says so.
        match plan_of(&words(&["htop", "-d", "5"])).unwrap() {
            Plan::New {
                program,
                args,
                here,
                ..
            } => {
                assert_eq!(program, "htop");
                assert_eq!(args, ["-d", "5"]);
                assert!(!here);
            }
            other => panic!("{other:?}"),
        }
        match plan_of(&words(&[
            "--here",
            "--cwd",
            "/tmp",
            "--",
            "pkexec",
            "--keep-cwd",
            "sh",
        ]))
        .unwrap()
        {
            Plan::New {
                program,
                args,
                cwd,
                here,
            } => {
                assert_eq!(program, "pkexec");
                assert_eq!(args, ["--keep-cwd", "sh"]);
                assert_eq!(cwd, Some(PathBuf::from("/tmp")));
                assert!(here);
            }
            other => panic!("{other:?}"),
        }
        // NO PROGRAM is the person's shell.
        assert!(matches!(
            plan_of(&[]).unwrap(),
            Plan::New { program, .. } if program == ronoctua_term::pty::default_program()
        ));
        assert!(plan_of(&words(&["--frobnicate"])).is_err());
    }

    #[test]
    fn show_names_running_terminals_and_the_one_in_front() {
        // FAILS IF `--show` takes an id that names another file, `--front`
        // is not one of the shown, the first shown is not the front when
        // none is named, or the window does not reach every one shown —
        // in the order given, each its own launch.
        let (a, b) = ("0123456789abcdef", "fedcba9876543210");
        let plan = plan_of(&words(&["--show", &format!("{a},{b}"), "--front", b])).unwrap();
        match &plan {
            Plan::Show { show, front } => {
                assert_eq!(show.len(), 2);
                assert_eq!(front.as_str(), b);
            }
            other => panic!("{other:?}"),
        }
        let reached: Vec<String> = plan
            .reachings()
            .into_iter()
            .map(|r| match r {
                Reaching::Held(id) => id.as_str().to_owned(),
                Reaching::New { .. } => panic!("a shown terminal is not a new one"),
            })
            .collect();
        assert_eq!(reached, [a, b]);
        match plan_of(&words(&["--show", a])).unwrap() {
            Plan::Show { front, .. } => assert_eq!(front.as_str(), a),
            other => panic!("{other:?}"),
        }
        assert!(plan_of(&words(&["--show", "../etc/passwd"])).is_err());
        assert!(plan_of(&words(&["--show", a, "--front", b])).is_err());
        assert!(plan_of(&words(&["--show", a, "--here"])).is_err());
        assert!(plan_of(&words(&["--front", a])).is_err());
    }

    #[test]
    fn a_paste_is_asked_about_only_when_it_could_run_a_command() {
        // FAILS IF a line break pasted into a shell is written without a
        // question (the line runs), or a plain word is asked about, or the
        // bracket's own end slips through bracketed paste.
        assert!(paste_is_safe("ls", false));
        assert!(!paste_is_safe("rm -rf ~\n", false));
        assert!(
            paste_is_safe("rm -rf ~\n", true),
            "bracketed, a line break is text"
        );
        assert!(!paste_is_safe("x\x1b[201~rm -rf ~\n", true));
    }

    #[test]
    fn a_flood_of_copies_is_one_thread_copying_the_last_of_each() {
        // FAILS IF every copy a program writes (OSC 52, in a loop) starts a
        // thread and a `wl-copy` of its own, a copy owed while one runs is
        // lost, or one older than the last is copied after it.
        let mut owed = Owed::default();
        assert!(owed.owe("first".to_owned(), false), "nothing was copying");
        for n in 0..1000 {
            assert!(!owed.owe(format!("write {n}"), false), "a second thread");
        }
        assert!(!owed.owe("swept".to_owned(), true));
        assert_eq!(owed.next(), Some(("write 999".to_owned(), false)));
        assert!(!owed.owe("last".to_owned(), false), "owed while copying");
        assert_eq!(owed.next(), Some(("last".to_owned(), false)));
        assert_eq!(owed.next(), Some(("swept".to_owned(), true)));
        assert_eq!(owed.next(), None);
        assert!(owed.owe("again".to_owned(), false), "the thread ended");
    }

    #[test]
    fn the_window_is_born_holding_the_declared_grid() {
        // FAILS IF the terminal's first size is not `space.terminal`'s grid
        // in its own cells — a shell born at 79 columns — or, once the cell
        // was measured on a screen of two, the size asked is counted as if
        // its physical pixels were logical ones (a window twice as large).
        let w = TerminalWindow::new(Identity::Terminal).unwrap();
        let (width, height) = w.default_size();
        let placed = Placed::of(width, height, 1, &w.geometry);
        let declared = (
            tokens::space::TERMINAL_COLUMNS,
            tokens::space::TERMINAL_LINES,
        );
        assert_eq!((placed.columns, placed.rows), declared);
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let ui = Ui::detached(texts, (1, 1), 2).expect("a detached door");
        let mut d = crate::door::Detached::new(ui, w);
        d.lay_out_at((640, 300));
        assert_eq!(d.content.geometry.scale, 2, "laid out at two");
        let (width, height) = d.content.default_size();
        let placed = Placed::of(width, height, 2, &d.content.geometry);
        assert_eq!((placed.columns, placed.rows), declared, "at two");
    }

    /// The front's frame, filled into the sheet as `paint` fills it.
    fn fill_front(w: &mut TerminalWindow) {
        let mirrored = Arc::clone(&w.group.front().expect("a terminal").mirrored);
        assert!(mirrored.update());
        let palette = Palette::of_the_page();
        let mut frame = mirrored.frame();
        terminal_sheet::fill(
            &mut w.sheet,
            &mut frame,
            &palette,
            terminal_sheet::Showing {
                focused: true,
                blink_off: false,
                all: true,
                faint: faint_alpha(),
            },
        );
    }

    #[test]
    fn a_recorded_stream_is_drawn_in_its_own_cells() {
        // FAILS IF the camera's engine does not reach the sheet the door
        // draws: the text a program wrote must be the sheet's, cell by cell,
        // colour included.
        let corpus = b"plain \x1b[31mred\x1b[0m\r\nsecond line".to_vec();
        let w = TerminalWindow::photographed(Identity::Terminal, corpus).unwrap();
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut d = crate::door::Detached::new(crate::door::Ui::for_tests(texts), w);
        d.lay_out_at((640, 300));
        assert_eq!(d.content.corpus_failed(), None);
        fill_front(&mut d.content);
        let sheet = d.content.sheet_now();
        assert!(terminal_sheet::line_text(sheet, 0).starts_with("plain red"));
        assert!(terminal_sheet::line_text(sheet, 1).starts_with("second line"));
        let red = &sheet.row(0)[6];
        assert_ne!(
            red.fg,
            sheet.row(0)[0].fg,
            "the red cell kept the default ink"
        );
        assert_eq!(sheet.cursor.map(|c| (c.line, c.column)), Some((1, 11)));
    }

    fn state(title: &str) -> TerminalState {
        TerminalState {
            protocol: PROTOCOL,
            id: TerminalId::fresh().unwrap(),
            group: GroupId::fresh().unwrap(),
            order: 2,
            viewer: Some(1),
            front: true,
            seen: true,
            title: title.to_owned(),
            program: "sh".to_owned(),
            cwd: None,
            started_s: 0,
            alone_since_s: None,
            attention: Attention::default(),
            notice: None,
            command: None,
            front_asked: 0,
            ended: None,
            statuses: Vec::new(),
            progress: None,
        }
    }

    /// A window holding `count` terminals, attachments 0, 1, … in that
    /// order, the first in front — taken in as a started launch takes them.
    fn window_of(count: usize) -> (TerminalWindow, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut ui = Ui::for_tests(texts);
        let mut w = TerminalWindow::new(Identity::Terminal).unwrap();
        for n in 0..count {
            let g = w.fresh_generation();
            let held = Held::new(
                g,
                TerminalId::fresh().unwrap(),
                terminal_group::step(n),
                Mirrored::new().unwrap(),
            );
            w.took_in(&mut ui, held, false);
        }
        (w, ui)
    }

    /// The same, laid out on a window of `size` as the door lays it out.
    fn laid_out(count: usize, size: (u32, u32)) -> crate::door::Detached<TerminalWindow> {
        let (w, ui) = window_of(count);
        let mut d = crate::door::Detached::new(ui, w);
        d.lay_out_at(size);
        d
    }

    fn arrival(generation: u64, heard: Heard) -> Arrival {
        Arrival { generation, heard }
    }

    #[test]
    fn the_title_follows_the_program_and_a_stale_attachment_is_not_heard() {
        // FAILS IF the window keeps its own name when the program names it,
        // forgets its own when the program's name goes, or takes what a
        // replaced attachment's reader still says for the current one's.
        let (mut w, mut ui) = window_of(1);
        assert_eq!(
            Content::title(&w, &ui.texts),
            ui.texts.t(crate::key::TERMINAL_TITLE)
        );
        w.hear(
            &mut ui,
            arrival(0, Heard::State(Box::new(state("vim notes.md")))),
        );
        assert_eq!(Content::title(&w, &ui.texts), "vim notes.md");
        w.hear(&mut ui, arrival(9, Heard::State(Box::new(state("old")))));
        assert_eq!(Content::title(&w, &ui.texts), "vim notes.md");
        w.hear(&mut ui, arrival(9, Heard::Exited(0)));
        assert!(!ui.quit, "a stale attachment closed the window");
        w.hear(&mut ui, arrival(0, Heard::State(Box::new(state("")))));
        assert_eq!(
            Content::title(&w, &ui.texts),
            ui.texts.t(crate::key::TERMINAL_TITLE)
        );
    }

    #[test]
    fn the_window_closes_when_its_last_program_ends_or_another_takes_it() {
        // FAILS IF a window stays open on a program that ended (an empty
        // terminal nobody can type into), or on a terminal another window
        // took — two windows that both claim to show it.
        for heard in [Heard::Exited(3), Heard::Taken, Heard::Gone] {
            let (mut w, mut ui) = window_of(1);
            w.hear(&mut ui, arrival(0, heard));
            assert!(ui.quit);
        }
    }

    #[test]
    fn a_terminal_that_ends_leaves_the_group_and_the_last_closes_the_window() {
        // FAILS IF a terminal whose program ended stays in the strip, the
        // front is left on it, the strip stays for a lone terminal (a row
        // it does not give back), or the window closes while one remains.
        let mut d = laid_out(3, (900, 500));
        let with_strip = d.content.placed;
        d.content.front_line(&mut d.ui, 1);
        d.content.hear(&mut d.ui, arrival(1, Heard::Exited(0)));
        assert!(!d.ui.quit);
        assert_eq!(d.content.terminals(), 2);
        assert_eq!(
            d.content.front_generation(),
            Some(2),
            "the right-hand neighbour"
        );
        d.content.hear(&mut d.ui, arrival(0, Heard::Gone));
        assert_eq!(d.content.terminals(), 1);
        let alone = d.content.placed;
        assert_eq!(alone.origin.1, 0, "the strip went with the second");
        assert!(alone.rows >= with_strip.rows, "the rows came back");
        assert_eq!(
            d.content.group.front().and_then(|h| h.sent_size),
            Some(alone.size()),
            "the one left was told its size"
        );
        d.content.hear(&mut d.ui, arrival(2, Heard::Exited(1)));
        assert!(d.ui.quit, "the last one closes the window");
    }

    #[test]
    fn the_strip_takes_its_height_off_the_grid() {
        // FAILS IF the grid is laid out over the strip, the strip's rows are
        // still counted, or a group that grew to two does not tell each of
        // its terminals the new size — once.
        let one = laid_out(1, (900, 500));
        let two = laid_out(2, (900, 500));
        let scale = two.content.placed.scale;
        let strip = terminal_strip::height(2) * scale;
        assert_eq!(one.content.placed.origin.1, 0);
        assert_eq!(two.content.placed.origin.1, strip);
        assert_eq!(
            two.content.sheet.origin.1,
            i32::try_from(strip).unwrap(),
            "the sheet is drawn under the strip"
        );
        assert_eq!(
            u32::from(two.content.placed.rows),
            (500 * scale - strip) / two.content.placed.cell.1
        );
        assert_eq!(two.content.placed.columns, one.content.placed.columns);
        for held in two.content.group.iter() {
            assert_eq!(held.sent_size, Some(two.content.placed.size()));
        }
        // A THIRD changes nothing of the grid: no second SIGWINCH.
        let mut three = laid_out(2, (900, 500));
        let before = three.content.placed;
        let g = three.content.fresh_generation();
        let held = Held::new(
            g,
            TerminalId::fresh().unwrap(),
            terminal_group::step(5),
            Mirrored::new().unwrap(),
        );
        three.content.took_in(&mut three.ui, held, false);
        assert_eq!(three.content.placed, before);
    }

    fn press_at(ui: &Ui, at: (f64, f64)) -> PointerEvent {
        PointerEvent {
            surface: ui.surface.clone(),
            position: at,
            kind: PointerEventKind::Press {
                time: 0,
                button: LEFT_BUTTON,
                serial: 0,
            },
        }
    }

    fn motion_at(ui: &Ui, at: (f64, f64)) -> PointerEvent {
        PointerEvent {
            surface: ui.surface.clone(),
            position: at,
            kind: PointerEventKind::Motion { time: 0 },
        }
    }

    #[test]
    fn a_click_on_the_strip_fronts_the_line_under_it() {
        // FAILS IF the first click on a strip's line does not put its
        // terminal in front, the pointer resting on a line does not lend
        // its terminal's title to the window (nothing else says which line
        // is which), or the line in front lights under the pointer — a
        // click there takes nothing.
        let mut d = laid_out(3, (900, 500));
        let titles = ["one", "two", "three"];
        for (g, t) in titles.iter().enumerate() {
            d.content.hear(
                &mut d.ui,
                arrival(g as u64, Heard::State(Box::new(state(t)))),
            );
        }
        let strip = Strip::of(900, 3).unwrap();
        let (from, to) = strip.part(2);
        let at = (f64::from(from + to) / 2.0, 1.0);
        d.pointer(&[motion_at(&d.ui, at)]);
        assert_eq!(d.content.hovered, Some(2));
        assert_eq!(Content::title(&d.content, &d.ui.texts), "three");
        assert!(d.content.strip_lines()[2].lit);
        d.pointer(&[press_at(&d.ui, at)]);
        assert_eq!(d.content.group.front_index(), 2, "the first click fronts");
        assert!(!d.content.strip_lines()[2].lit, "the front never lights");
        // AND THE GRID BELOW is the program's: a click there fronts nothing.
        let below = (f64::from(from) + 1.0, f64::from(strip.height()) + 30.0);
        d.pointer(&[press_at(&d.ui, below)]);
        assert_eq!(d.content.group.front_index(), 2);
        // THE POINTER LEAVING gives the window its front's title back.
        d.pointer(&[PointerEvent {
            surface: d.ui.surface.clone(),
            position: at,
            kind: PointerEventKind::Leave { serial: 0 },
        }]);
        assert_eq!(d.content.hovered, None);
    }

    #[test]
    fn the_group_keys_walk_detach_and_are_listed_from_two() {
        // FAILS IF `C-TAB`/`C-S-TAB` do not walk the group round its end,
        // every terminal is not told where it is after a walk (its call for
        // attention cleared by the wrong one), the shortcuts list keys that
        // do nothing with one terminal (or not the new one), or on anything
        // but one key each after `LEADER Alt`, the monitor takes a
        // group's key from its program, or the detach does not open the
        // terminal in front in a window of its own.
        let mut d = laid_out(1, (900, 500));
        let listed = |d: &crate::door::Detached<TerminalWindow>| -> Vec<String> {
            Content::shortcuts(&d.content, &d.ui)
                .into_iter()
                .map(|b| b.command)
                .collect()
        };
        assert_eq!(listed(&d), [terminal_keys::NEW]);
        let mut d2 = laid_out(3, (900, 500));
        assert_eq!(
            listed(&d2),
            [
                terminal_keys::NEW,
                terminal_keys::NEXT,
                terminal_keys::PREVIOUS,
                terminal_keys::DETACH
            ]
        );
        // EACH ON ONE KEY after `LEADER Alt` — the chords are the window's.
        let keys: Vec<String> = Content::shortcuts(&d2.content, &d2.ui)
            .into_iter()
            .map(|b| b.keys)
            .collect();
        assert_eq!(keys, ["t", "n", "p", "w"]);
        d2.press(ronoctua_keymap::Key::parse("C-TAB").unwrap());
        assert_eq!(d2.content.group.front_index(), 1);
        d2.press(ronoctua_keymap::Key::parse("C-S-TAB").unwrap());
        d2.press(ronoctua_keymap::Key::parse("C-S-TAB").unwrap());
        assert_eq!(d2.content.group.front_index(), 2, "round the end");
        let told: Vec<bool> = d2
            .content
            .group
            .iter()
            .map(|h| h.told.is_some_and(|p| p.front))
            .collect();
        assert_eq!(told, [false, false, true]);
        let id = d2.content.group.front().unwrap().id.as_str().to_owned();
        assert!(Content::run_shortcut(
            &mut d2.content,
            &mut d2.ui,
            terminal_keys::DETACH
        ));
        let opened = d2.ui.launches().iter().any(|l| {
            matches!(l, crate::door::Launching::Program { args, .. }
                if args == &["terminal", "--show", &id, "--front", &id])
        });
        assert!(opened, "{:?}", d2.ui.launches());
        // ITS NEW WINDOW TAKES IT, and the neighbour comes to the front.
        d2.content.hear(&mut d2.ui, arrival(2, Heard::Taken));
        assert_eq!(d2.content.terminals(), 2);
        assert!(!d2.ui.quit);
        // ONE TERMINAL has nowhere to walk or to leave.
        d.press(ronoctua_keymap::Key::parse("C-TAB").unwrap());
        assert!(!Content::run_shortcut(
            &mut d.content,
            &mut d.ui,
            "elsewhere"
        ));
        // A MONITOR'S PROGRAM has all its keys.
        let monitor = TerminalWindow::new(Identity::Monitor).unwrap();
        assert!(Content::shortcuts(&monitor, &d.ui).is_empty());
        assert!(!monitor.keeps(Kept::Group(Grouped::New)));
        assert!(monitor.keeps(Kept::Copy));
    }

    #[test]
    fn front_asked_moving_fronts_its_terminal() {
        // FAILS IF the footer's `terminal-show` (the count moving) does not
        // bring a terminal of this window to the front, or the first state
        // a window hears fronts its terminal (a restored group fighting for
        // the front).
        let (mut w, mut ui) = window_of(2);
        let mut s = state("behind");
        s.front_asked = 3;
        w.hear(&mut ui, arrival(1, Heard::State(Box::new(s.clone()))));
        assert_eq!(w.group.front_index(), 0, "the first state only counts");
        s.front_asked = 4;
        w.hear(&mut ui, arrival(1, Heard::State(Box::new(s))));
        assert_eq!(w.group.front_index(), 1);
        assert_eq!(Content::title(&w, &ui.texts), "behind");
    }

    fn consent(id: u64) -> wire::Consent {
        wire::Consent {
            id,
            clipboard: wire::ClipboardKind::Standard,
            mimes: vec!["text/plain".to_owned()],
            program: "nvim".to_owned(),
        }
    }

    fn reading(w: &TerminalWindow) -> Option<(u64, u64)> {
        match &w.question.as_ref()?.1.asked {
            Asked::ClipboardRead(c) => Some((w.question.as_ref()?.0, c.id)),
            Asked::Paste(..) => None,
        }
    }

    fn answering() -> (terminal_link::Answering, mpsc::Receiver<ToHolder>) {
        let (tx, rx) = mpsc::channel();
        (terminal_link::Answering::new(tx), rx)
    }

    #[test]
    fn a_programs_clipboard_read_is_asked_no_first_and_a_newer_one_replaces_it() {
        // FAILS IF a program reads the clipboard without a question, the
        // question is born on the yes, or a read the holder has given up for
        // a newer one stays on the line — every key taken from the program
        // for an answer nobody waits for, the live read behind it.
        let (mut w, mut ui) = window_of(1);
        let (a, _rx) = answering();
        w.hear(&mut ui, arrival(0, Heard::Consent(consent(1), a.clone())));
        assert!(
            !w.question.as_ref().expect("asked").1.on(),
            "born on the no"
        );
        assert_eq!(reading(&w), Some((0, 1)));
        w.hear(&mut ui, arrival(0, Heard::Consent(consent(2), a)));
        assert_eq!(reading(&w), Some((0, 2)), "the newer read is the one asked");
        assert!(w.waiting.is_empty(), "the stale read still waits");
        w.answered(&mut ui, false);
        assert!(w.question.is_none());
    }

    #[test]
    fn a_read_waits_behind_a_paste_and_goes_when_the_holder_gives_it_up() {
        // FAILS IF a program's read pushes the person's own paste question
        // off the line, a read the holder gave up for a newer one still waits
        // behind it, a read the holder stopped waiting for (its deadline) is
        // still asked, or one terminal's read replaces another's.
        let (mut w, mut ui) = window_of(2);
        let (a, _rx) = answering();
        w.ask(
            &mut ui,
            0,
            Asked::Paste("rm -rf ~\n".to_owned(), wire::PasteSource::Clipboard),
        );
        w.hear(&mut ui, arrival(0, Heard::Consent(consent(3), a.clone())));
        w.hear(&mut ui, arrival(0, Heard::Consent(consent(4), a.clone())));
        w.hear(&mut ui, arrival(1, Heard::Consent(consent(4), a)));
        assert_eq!(reading(&w), None, "the paste keeps the line");
        let waiting: Vec<(u64, u64)> = w
            .waiting
            .iter()
            .filter_map(|(g, a)| match a {
                Asked::ClipboardRead(c) => Some((*g, c.id)),
                Asked::Paste(..) => None,
            })
            .collect();
        assert_eq!(waiting, [(0, 4), (1, 4)], "one read a terminal");
        w.consent_expired(&mut ui, 0, 4);
        assert_eq!(w.waiting.len(), 1, "an expired read still waits");
        assert!(w.question.is_some(), "the paste went with the read");
        w.answered(&mut ui, false);
        assert_eq!(reading(&w), Some((1, 4)), "the other terminal's read");
        w.consent_expired(&mut ui, 0, 4);
        assert_eq!(
            reading(&w),
            Some((1, 4)),
            "another terminal's deadline took it"
        );
        w.consent_expired(&mut ui, 1, 4);
        assert!(w.question.is_none(), "an expired read stayed on the line");
    }

    #[test]
    fn a_clipboard_read_before_its_terminal_is_shown_is_refused_at_once() {
        // FAILS IF a read that arrives before the window has its terminal
        // (the attach still on its way) is asked about a program nobody has
        // seen, or left for the holder to give up at its deadline — the
        // program stopped for a minute.
        let (mut w, mut ui) = window_of(1);
        let (a, rx) = answering();
        w.hear(&mut ui, arrival(7, Heard::Consent(consent(5), a)));
        assert!(w.question.is_none());
        match rx.try_recv() {
            Ok(ToHolder::Answer(wire::Answer { id: 5, text: None })) => {}
            other => panic!("not refused at once: {other:?}"),
        }
    }

    #[test]
    fn a_terminal_that_could_not_start_is_said_on_the_window_s_line() {
        // FAILS IF a window whose terminal could not start vanishes with the
        // reason only in the log, or a new terminal that failed in a group
        // closes the window with the others in it.
        let (mut w, mut ui) = window_of(0);
        let size = Placed::of(640, 400, 1, &w.geometry).size();
        let pending = |generation, new| Pending {
            generation,
            order: terminal_group::ORDER_STEP,
            front: true,
            mirrored: Mirrored::new().unwrap(),
            size,
            colors: Palette::of_the_page().wire(),
            new,
        };
        let first = pending(0, true);
        w.group.expect(first);
        w.hear(
            &mut ui,
            arrival(
                0,
                Heard::Started(Box::new(Err("no such program".to_owned()))),
            ),
        );
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::TERMINAL_COULD_NOT_START).as_str())
        );
        assert!(w.closing, "it closes once the line was read");
        let (mut w, mut ui) = window_of(1);
        let second = pending(5, true);
        w.group.expect(second);
        w.hear(
            &mut ui,
            arrival(5, Heard::Started(Box::new(Err("no".to_owned())))),
        );
        assert!(!w.closing && !ui.quit);
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::TERMINAL_NEW_FAILED).as_str())
        );
    }

    /// A terminal of this process running `sh -c script`, launched as the
    /// window launches one — `generation` promised a place first — and
    /// everything its reader said until `done` took one, held back: what
    /// the window would hear BEFORE the launch came back. Each wait is 20 s
    /// at most, a bound on a broken test.
    fn launched_early(
        w: &mut TerminalWindow,
        generation: u64,
        script: &str,
        done: impl Fn(&Heard) -> bool,
    ) -> (terminal_link::Started, Vec<Arrival>) {
        let (started, said, _) = launched(w, generation, script, done);
        (started, said)
    }

    /// [`launched_early`], and a word each time its mirror moves from then
    /// on — what a test waits on for the program's answer to reach the
    /// screen.
    fn launched(
        w: &mut TerminalWindow,
        generation: u64,
        script: &str,
        done: impl Fn(&Heard) -> bool,
    ) -> (terminal_link::Started, Vec<Arrival>, mpsc::Receiver<()>) {
        let (tx, rx) = mpsc::channel::<Arrival>();
        let tx = Mutex::new(tx);
        let (moved_tx, moved) = mpsc::channel::<()>();
        let moved_tx = Mutex::new(moved_tx);
        let tell = Arc::new(terminal_link::Tell {
            heard: Box::new(move |a| {
                let _ = tx.lock().unwrap().send(a);
            }),
            moved: Box::new(move || {
                let _ = moved_tx.lock().unwrap().send(());
            }),
        });
        let size = w.placed.size();
        let colors = Palette::of_the_page().wire();
        let mirrored = Mirrored::new().unwrap();
        w.group.expect(Pending {
            generation,
            order: terminal_group::step(9),
            front: true,
            mirrored: Arc::clone(&mirrored),
            size,
            colors: colors.clone(),
            new: true,
        });
        let started = terminal_link::start(
            &Reaching::New {
                program: "/bin/sh".to_owned(),
                args: vec!["-c".to_owned(), script.to_owned()],
                cwd: None,
                here: true,
            },
            &terminal_link::Showing {
                size,
                colors,
                group: w.group_id.clone(),
                order: terminal_group::step(9),
            },
            ronoctua_terminals::spawn::Born::default(),
            &mirrored,
            &tell,
            generation,
        )
        .expect("started");
        let mut said = Vec::new();
        loop {
            let a = rx
                .recv_timeout(std::time::Duration::from_secs(20))
                .expect("its reader spoke in time");
            let last = done(&a.heard);
            said.push(a);
            if last {
                return (started, said, moved);
            }
        }
    }

    #[test]
    fn what_a_terminal_says_before_its_launch_comes_back_is_heard() {
        // FAILS IF what a terminal's reader says before the launch thread's
        // `Started` reaches the window is dropped: its title never shows
        // until it changes again, and a terminal shown after its program
        // ended — the end said at once — stays in the window for good.
        let (mut w, mut ui) = window_of(1);
        let (started, said) = launched_early(
            &mut w,
            5,
            "printf '\\033]2;early\\007'; exec cat",
            |h| matches!(h, Heard::State(s) if s.title == "early"),
        );
        for a in said {
            w.hear(&mut ui, a);
        }
        w.hear(&mut ui, arrival(5, Heard::Started(Box::new(Ok(started)))));
        assert_eq!(w.terminals(), 2);
        assert_eq!(w.front_generation(), Some(5));
        assert_eq!(Content::title(&w, &ui.texts), "early");
        let (started, said) = launched_early(&mut w, 6, "exit 3", |h| {
            matches!(h, Heard::Exited(_) | Heard::Gone)
        });
        for a in said {
            w.hear(&mut ui, a);
        }
        w.hear(&mut ui, arrival(6, Heard::Started(Box::new(Ok(started)))));
        assert_eq!(w.group.index_of(6), None, "an ended terminal was kept");
        assert_eq!(w.terminals(), 2);
        assert!(!ui.quit && w.early.is_empty());
    }

    #[test]
    fn the_buttons_a_program_was_told_of_are_let_go_when_the_pointer_leaves() {
        // FAILS IF a button reported down to a program is still owed its
        // release after the pointer left — the next motion reported as a
        // drag the person is not making — or a selection the left button
        // began still follows a pointer that came back with no button held.
        let mut d = laid_out(1, (640, 400));
        d.here.extra.reported = vec![LEFT_BUTTON, MIDDLE_BUTTON];
        d.here.extra.selecting = true;
        d.pointer(&[PointerEvent {
            surface: d.ui.surface.clone(),
            position: (10.0, 10.0),
            kind: PointerEventKind::Leave { serial: 0 },
        }]);
        assert!(d.here.extra.reported.is_empty());
        assert!(!d.here.extra.selecting, "the selection still follows");
    }

    #[test]
    fn ctrl_over_an_address_underlines_it_and_ctrl_click_opens_it() {
        // FAILS IF `Ctrl` over an address on screen does not make it the
        // link under the pointer (the hand and the underline never show), a
        // pointer without `Ctrl` takes it for one, the underline is not
        // drawn on its cells, or `Ctrl` and a click does not open it by the
        // opener every door opens a page with.
        let corpus = b"see https://example.org/x ok".to_vec();
        let w = TerminalWindow::photographed(Identity::Terminal, corpus).unwrap();
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut d = crate::door::Detached::new(Ui::for_tests(texts), w);
        d.lay_out_at((900, 400));
        fill_front(&mut d.content);
        let placed = d.content.placed;
        let s = f64::from(placed.scale);
        let at = (
            (f64::from(placed.origin.0) + f64::from(placed.cell.0) * 8.5) / s,
            (f64::from(placed.origin.1) + f64::from(placed.cell.1) * 0.5) / s,
        );
        d.pointer(&[motion_at(&d.ui, at)]);
        assert!(d.content.link.is_none(), "no Ctrl, no link");
        d.ui.pointer_mods.ctrl = true;
        d.pointer(&[motion_at(&d.ui, at)]);
        let link = d.content.link.clone().expect("the link under Ctrl");
        assert_eq!(link.url, "https://example.org/x");
        assert_eq!(
            Content::cursor_at(&d.content, &d.ui, at),
            crate::cursor_shape::Look::Pointer
        );
        terminal_hyperlink::underline(&mut d.content.sheet, &link);
        assert_ne!(
            d.content.sheet.row(0)[4].style.underline,
            ronoctua_render::cells::Underline::None
        );
        d.pointer(&[press_at(&d.ui, at)]);
        match XDG_OPEN.filter(|p| !p.is_empty()) {
            Some(opener) => assert!(
                d.ui.launches().iter().any(|l| matches!(l,
                    crate::door::Launching::Program { program, args, .. }
                        if program == opener && args == &["https://example.org/x"])),
                "{:?}",
                d.ui.launches()
            ),
            None => assert_eq!(
                d.ui.message.as_deref(),
                Some(d.ui.texts.t(crate::key::TERMINAL_LINK_NO_OPENER).as_str())
            ),
        }
        // A KEY TYPED hides the pointer over the grid, until it moves.
        d.content.hid_the_pointer = true;
        assert_eq!(
            Content::cursor_at(&d.content, &d.ui, at),
            crate::cursor_shape::Look::Hidden
        );
        d.pointer(&[motion_at(&d.ui, at)]);
        assert!(
            !d.content.hid_the_pointer,
            "the pointer came back as it moved"
        );
    }

    #[test]
    fn the_search_line_takes_the_keys_and_esc_gives_them_back() {
        // FAILS IF `C-S-f` does not open the search over the last row, a
        // letter typed at it reaches the program instead of the needle,
        // a paste goes to the program, or `Esc` does not give the keys back.
        let mut d = laid_out(1, (640, 400));
        d.press(ronoctua_keymap::Key::parse("C-F").unwrap());
        assert!(d.content.finding.is_some(), "C-S-f opened no search");
        d.press(ronoctua_keymap::Key::parse("x").unwrap());
        assert_eq!(
            d.content.finding.as_ref().map(|f| f.needle.as_str()),
            Some("x")
        );
        Content::pasted(&mut d.content, &mut d.ui, "yz");
        assert_eq!(
            d.content.finding.as_ref().map(|f| f.needle.as_str()),
            Some("xyz")
        );
        let runs = d.content.line_over(&d.ui).expect("the search's line");
        assert!(runs.iter().any(|r| r.text.contains("xyz")));
        d.press(ronoctua_keymap::Key::parse("ESC").unwrap());
        assert!(d.content.finding.is_none());
    }

    #[test]
    fn files_dropped_are_typed_as_their_paths_and_ask_nothing() {
        // FAILS IF a drop of a file whose name holds a line break is asked
        // about as a paste that could run a command (its path is written in
        // `$'…'`, which runs nothing), or a drop reaches the search's needle
        // as anything but the quoted paths.
        let mut d = laid_out(1, (640, 400));
        d.content.hear(
            &mut d.ui,
            arrival(
                u64::MAX,
                Heard::Dropped(vec![PathBuf::from("/tmp/a\nb"), PathBuf::from("/tmp/c d")]),
            ),
        );
        assert!(
            d.content.question.is_none(),
            "a quoted drop was asked about"
        );
        d.press(ronoctua_keymap::Key::parse("C-F").unwrap());
        d.content.hear(
            &mut d.ui,
            arrival(u64::MAX, Heard::Dropped(vec![PathBuf::from("/tmp/c d")])),
        );
        assert_eq!(
            d.content.finding.as_ref().map(|f| f.needle.as_str()),
            Some("'/tmp/c d'")
        );
    }

    /// Every frame sent so far down a listened link.
    fn sent(heard: &std::sync::mpsc::Receiver<ToHolder>) -> Vec<ToHolder> {
        heard.try_iter().collect()
    }

    /// A key event, as the door hands one in.
    fn key_event(raw_code: u32, keysym: Keysym, utf8: Option<&str>) -> KeyEvent {
        KeyEvent {
            time: 0,
            raw_code,
            keysym,
            utf8: utf8.map(str::to_owned),
        }
    }

    #[test]
    fn a_drop_lands_as_text_and_a_paste_as_the_clipboard() {
        // FAILS IF files dropped reach the program as a clipboard's paste —
        // a program that tells the two apart (Ghostty's own `PasteSource`)
        // takes a drop for something the person pasted — or a paste from the
        // clipboard as anything else.
        let (mut w, mut ui) = window_of(1);
        let (link, heard) = Attachment::listened(0);
        w.group.get_mut(0).unwrap().link = Some(link);
        w.hear(
            &mut ui,
            arrival(u64::MAX, Heard::Dropped(vec![PathBuf::from("/tmp/a b")])),
        );
        Content::pasted(&mut w, &mut ui, "copied");
        let sources: Vec<(String, wire::PasteSource)> = sent(&heard)
            .into_iter()
            .filter_map(|f| match f {
                ToHolder::Paste(p) => Some((p.text, p.source)),
                _ => None,
            })
            .collect();
        assert_eq!(
            sources,
            [
                ("'/tmp/a b'".to_owned(), wire::PasteSource::Text),
                ("copied".to_owned(), wire::PasteSource::Clipboard),
            ]
        );
    }

    #[test]
    fn a_paste_the_holder_refused_is_said_on_the_line() {
        // FAILS IF a paste the holder did not write vanishes with nothing on
        // screen — the person waiting for text that never lands — or the two
        // reasons are said alike.
        let (mut w, mut ui) = window_of(1);
        w.hear(
            &mut ui,
            arrival(0, Heard::PasteRefused(wire::PasteRefusal::Unsafe)),
        );
        assert_eq!(
            ui.message.as_deref(),
            Some(
                ui.texts
                    .t(crate::key::TERMINAL_PASTE_REFUSED_UNSAFE)
                    .as_str()
            )
        );
        w.hear(
            &mut ui,
            arrival(0, Heard::PasteRefused(wire::PasteRefusal::Failed)),
        );
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::TERMINAL_PASTE_REFUSED).as_str())
        );
        assert_ne!(
            ui.texts.t(crate::key::TERMINAL_PASTE_REFUSED),
            ui.texts.t(crate::key::TERMINAL_PASTE_REFUSED_UNSAFE)
        );
    }

    #[test]
    fn the_keys_still_down_are_let_go_on_the_front_they_went_to() {
        // FAILS IF a key held while another terminal comes to the front is
        // never let go on the one that heard it go down (a program that asked
        // for release events — kitty's — keeps `Ctrl` held for good), its
        // release is sent to the new front, which never heard the press, or
        // the hand letting go afterwards sends a second one.
        let (mut w, mut ui) = window_of(2);
        let (first, first_heard) = Attachment::listened(0);
        let (second, second_heard) = Attachment::listened(1);
        w.group.get_mut(0).unwrap().link = Some(first);
        w.group.get_mut(1).unwrap().link = Some(second);
        let ctrl = key_event(29, Keysym::Control_L, None);
        let held = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        Content::typed(&mut w, &mut ui, &ctrl, held);
        assert!(
            sent(&first_heard).iter().any(|f| matches!(f,
                ToHolder::Key(k) if k.code == 29 && k.action == wire::KeyAction::Press)),
            "the press did not reach the front"
        );
        // `C-TAB`: the next terminal comes to the front, with `Ctrl` down.
        Content::typed(&mut w, &mut ui, &key_event(15, Keysym::Tab, None), held);
        assert_eq!(w.group.front_index(), 1, "C-TAB fronted nothing");
        let released: Vec<wire::KeyEvent> = sent(&first_heard)
            .into_iter()
            .filter_map(|f| match f {
                ToHolder::Key(k) => Some(k),
                _ => None,
            })
            .collect();
        assert_eq!(released.len(), 1, "{released:?}");
        assert_eq!(
            (released[0].code, released[0].action),
            (29, wire::KeyAction::Release)
        );
        Content::released(&mut w, &mut ui, &ctrl);
        assert!(
            !sent(&second_heard)
                .iter()
                .any(|f| matches!(f, ToHolder::Key(_))),
            "the new front heard a key it never saw go down"
        );
        assert!(
            !sent(&first_heard)
                .iter()
                .any(|f| matches!(f, ToHolder::Key(_))),
            "the old front was let go of the key twice"
        );
    }

    #[test]
    fn a_zoom_changes_the_cell_and_the_grid_and_reset_comes_back() {
        // FAILS IF `C-=` does not make the text larger (fewer cells in the
        // same window), or `C-0` does not come back to the person's size.
        let mut d = laid_out(1, (900, 500));
        let born = (d.content.placed.columns, d.content.placed.rows);
        for _ in 0..3 {
            d.press(ronoctua_keymap::Key::parse("C-=").unwrap());
        }
        let larger = (d.content.placed.columns, d.content.placed.rows);
        assert!(
            larger.0 < born.0 && larger.1 <= born.1,
            "{born:?} -> {larger:?}"
        );
        d.press(ronoctua_keymap::Key::parse("C-0").unwrap());
        assert_eq!((d.content.placed.columns, d.content.placed.rows), born);
    }

    #[test]
    fn the_persons_size_of_text_is_the_windows_and_its_zoom_counts_from_it() {
        // FAILS IF a size of its own in `terminal.json` does not reach the
        // grid at once (fewer cells for larger text), the window's own zoom
        // is overridden by it, or `C-0` comes back to the system's size
        // instead of the person's terminal size.
        let mut d = laid_out(1, (900, 500));
        let born = (d.content.placed.columns, d.content.placed.rows);
        let larger = crate::terminal_settings::Settings {
            text_percent: Some(tokens::text::stepped(tokens::text::percent(), 6)),
            ..crate::terminal_settings::Settings::default()
        };
        d.content.settings_are(&mut d.ui, larger);
        let large = (d.content.placed.columns, d.content.placed.rows);
        assert!(large.0 < born.0, "{born:?} -> {large:?}");
        d.press(ronoctua_keymap::Key::parse("C-=").unwrap());
        let zoomed = (d.content.placed.columns, d.content.placed.rows);
        assert!(zoomed.0 < large.0, "the zoom is not counted from it");
        d.content
            .settings_are(&mut d.ui, crate::terminal_settings::Settings::default());
        assert_eq!(
            (d.content.placed.columns, d.content.placed.rows),
            zoomed,
            "the settings moved a window zoomed by hand"
        );
        d.press(ronoctua_keymap::Key::parse("C-0").unwrap());
        assert_eq!((d.content.placed.columns, d.content.placed.rows), born);
    }

    #[test]
    fn the_faint_ink_is_the_tokens_share_of_a_whole_one() {
        // FAILS IF SGR 2's alpha is not the token's percentage of 255 —
        // Ghostty's half by default — rounded, never past a whole ink.
        let pct = u32::from(tokens::space::TERMINAL_FAINTPCT.min(100));
        assert_eq!(u32::from(faint_alpha()), (255 * pct + 50) / 100);
        assert!(faint_alpha() > 0);
    }

    #[test]
    fn the_terminals_commands_are_registered_by_name_with_their_labels() {
        // FAILS IF a command of the group is not one the registry takes —
        // a label that is not its `cmd-<id>`, a by-name reason for a
        // command of another crate — or one the shortcuts list is missing.
        let (_, registry) = crate::bindings::assemble(&[&crate::KEYS]).expect("the keys assemble");
        for &(_, id, _) in terminal_keys::GROUP {
            assert!(registry.search(id).is_some(), "{id}");
            assert!(crate::KEYS.by_name.contains(&id));
        }
    }

    #[test]
    fn a_commit_is_one_text_frame_the_program_reads_as_typed() {
        // FAILS IF the terminal asks for no input method at its cursor, the
        // monitor or the rescue console asks for one (a hung input method
        // would keep every key from the windows that must work when nothing
        // else does), or text an input method committed does not reach the
        // program as typed text: sent as a paste, a program that turned
        // bracketed paste on (`?2004h`) reads it between `200~` and `201~`,
        // which its echo shows; sent nowhere, nothing shows. Each wait is
        // 20 s at most, a bound on a broken test.
        // ON A GRID WIDE ENOUGH for what is typed: a window never laid out
        // is one cell.
        let crate::door::Detached {
            content: mut w,
            mut ui,
            ..
        } = laid_out(0, (640, 400));
        assert_eq!(Content::text_input(&w, &ui), None, "nothing reached yet");
        let (started, said, moved) = launched(&mut w, 0, "printf '\\033[?2004h'; exec cat", |h| {
            matches!(h, Heard::Ready)
        });
        for a in said {
            w.hear(&mut ui, a);
        }
        w.hear(&mut ui, arrival(0, Heard::Started(Box::new(Ok(started)))));
        // THE PROGRAM ASKED FOR BRACKETED PASTE before anything is sent, or
        // a paste would look like typing.
        let bracketed = |w: &TerminalWindow| {
            w.group
                .front()
                .and_then(|f| f.mirrored.with(|m| m.mode(Mode::BRACKETED_PASTE)).flatten())
                .unwrap_or(false)
        };
        while !bracketed(&w) {
            moved
                .recv_timeout(std::time::Duration::from_secs(20))
                .expect("the program's mode in time");
        }
        assert_eq!(
            Content::text_input(&w, &ui),
            Some(Field {
                purpose: Purpose::Terminal,
                cursor: cursor_area(&w.placed, 0, 0),
            })
        );
        Content::committed(&mut w, &mut ui, "日本語");
        assert!(w.hid_the_pointer, "a commit is typing");
        let line = loop {
            moved
                .recv_timeout(std::time::Duration::from_secs(20))
                .expect("the program's echo in time");
            fill_front(&mut w);
            let line = terminal_sheet::line_text(&w.sheet, 0);
            if line.contains('語') {
                break line;
            }
        };
        assert_eq!(line.trim_end(), "日本語", "not typed text: {line:?}");
        // THE SAME TERMINAL in the two windows that must work when nothing
        // else does asks for none.
        for identity in [Identity::Monitor, Identity::Rescue] {
            w.identity = identity;
            assert_eq!(Content::text_input(&w, &ui), None, "{identity:?}");
        }
        w.identity = Identity::Terminal;
        // THE WINDOW'S OWN LINE TAKES IT BACK: none while the search is up.
        w.open_search(&mut ui);
        assert_eq!(Content::text_input(&w, &ui), None);
    }

    #[test]
    fn a_composition_is_drawn_at_the_cursor_and_the_programs_cells_come_back() {
        // FAILS IF what an input method composes is not drawn at the
        // program's cursor (a grapheme a cell, two for a wide one), not
        // underlined, does not move the cursor to its own, is not asked to
        // be drawn again when it changes, or leaves its cells behind once
        // it is gone — the guess still on the screen after the commit.
        let w = TerminalWindow::photographed(Identity::Terminal, b"abc".to_vec()).unwrap();
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut d = crate::door::Detached::new(Ui::for_tests(texts), w);
        d.lay_out_at((640, 300));
        let palette = Palette::of_the_page();
        fill_front(&mut d.content);
        d.content.place_the_preedit(&palette, None);
        assert_eq!(d.content.ime_at, Some((3, 0)), "the program's cursor");
        d.content.refill = false;
        let _ = d.ui.take_the_paint();
        Content::preedit(
            &mut d.content,
            &mut d.ui,
            Some(Preedit {
                text: "にほ".to_owned(),
                cursor: Some(6..6),
            }),
        );
        assert!(
            d.content.refill && d.ui.take_the_paint(),
            "a new composition draws nothing"
        );
        fill_front(&mut d.content);
        d.content.place_the_preedit(&palette, None);
        let sheet = &d.content.sheet;
        assert!(terminal_sheet::line_text(sheet, 0).starts_with("abcにほ"));
        let row = sheet.row(0);
        assert_eq!(
            (row[3].width, row[4].width, row[5].width),
            (Width::Wide, Width::Spacer, Width::Wide)
        );
        assert_eq!(row[3].style.underline, cells::Underline::Single);
        assert_eq!(row[2].style.underline, cells::Underline::None);
        assert_eq!(sheet.cursor.map(|c| (c.column, c.line)), Some((7, 0)));
        // ON A LINE THE WINDOW'S OWN LINE COVERS, nothing.
        fill_front(&mut d.content);
        d.content.place_the_preedit(&palette, Some(0));
        assert!(terminal_sheet::line_text(&d.content.sheet, 0).starts_with("abc "));
        Content::preedit(&mut d.content, &mut d.ui, None);
        fill_front(&mut d.content);
        d.content.place_the_preedit(&palette, None);
        let sheet = &d.content.sheet;
        assert!(terminal_sheet::line_text(sheet, 0).starts_with("abc "));
        assert_eq!(sheet.row(0)[3].style.underline, cells::Underline::None);
        assert_eq!(sheet.cursor.map(|c| (c.column, c.line)), Some((3, 0)));
    }

    #[test]
    fn a_composition_keeps_its_end_on_the_line_and_halves_no_wide_character() {
        // FAILS IF a composition near the last column runs off the line
        // instead of starting earlier, or the half of a program's wide
        // character it covers is left drawn as half of one — a glyph two
        // cells wide over a cell the composition holds.
        let w = TerminalWindow::new(Identity::Terminal).unwrap();
        let mut sheet = w.sheet.clone();
        sheet.resize(10, 1);
        sheet.cursor = None;
        let palette = Palette::of_the_page();
        let blank = sheet.blank();
        let wide = |g: &str, width| Cell {
            grapheme: Grapheme::of(g),
            width,
            ..blank.clone()
        };
        {
            let row = sheet.row_mut(0);
            row[2] = wide("日", Width::Wide);
            row[3] = wide(" ", Width::Spacer);
            row[6] = wide("本", Width::Wide);
            row[7] = wide(" ", Width::Spacer);
        }
        let mut cut = sheet.clone();
        draw_preedit(
            &mut cut,
            &Preedit {
                text: "xyz".to_owned(),
                cursor: None,
            },
            (3, 0),
            &palette,
        );
        assert_eq!(terminal_sheet::line_text(&cut, 0), "   xyz本  ");
        assert_eq!(cut.row(0)[2].width, Width::Narrow, "the head before it");
        assert_eq!(cut.row(0)[6], sheet.row(0)[6], "a whole one stays");
        let mut late = sheet.clone();
        draw_preedit(
            &mut late,
            &Preedit {
                text: "wxyz".to_owned(),
                cursor: Some(4..4),
            },
            (8, 0),
            &palette,
        );
        assert_eq!(terminal_sheet::line_text(&late, 0), "  日  wxyz");
        assert_eq!(late.row(0)[6].grapheme, Grapheme::of("w"));
    }

    #[test]
    fn a_composition_brings_a_screen_scrolled_back_to_the_live_one() {
        // FAILS IF a composition leaves the view scrolled back — drawn on
        // the history at the cell the program's cursor last had, with the
        // input method's guesses opened beside a line nobody types on — or
        // leaves the pointer over the letters as a key would not, or the
        // composition GOING moves a view the person scrolled back since.
        let corpus: Vec<u8> = (0..200)
            .flat_map(|n| format!("line {n}\r\n").into_bytes())
            .collect();
        let w = TerminalWindow::photographed(Identity::Terminal, corpus).unwrap();
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut d = crate::door::Detached::new(Ui::for_tests(texts), w);
        d.lay_out_at((640, 300));
        fill_front(&mut d.content);
        let mirrored = Arc::clone(&d.content.group.front().expect("a terminal").mirrored);
        let at_bottom = || mirrored.with(|m| m.at_bottom()).expect("a mirror");
        let back = || mirrored.with(|m| m.scroll(Scroll::Lines(-10)));
        back();
        assert!(!at_bottom(), "the corpus left no history to scroll back to");
        Content::preedit(
            &mut d.content,
            &mut d.ui,
            Some(Preedit {
                text: "に".to_owned(),
                cursor: Some(3..3),
            }),
        );
        assert!(at_bottom(), "composed over the history");
        assert!(d.content.hid_the_pointer, "composing is typing");
        back();
        Content::preedit(&mut d.content, &mut d.ui, None);
        assert!(!at_bottom(), "the composition going moved the view");
    }
}
