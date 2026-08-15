//! The footer's `wlr-layer-shell` surface — section 4.1.
//!
//! **ONE** surface, anchored at the bottom, full width, exclusive zone of two
//! lines at rest. There is no second surface anywhere in the system:
//! which-key, app finder, notification history and the shortcut manual all use
//! THIS one. "Do not invent a second surface."
//!
//! # The order of the lines
//!
//! ```text
//!   line 0         modeline    the buffer's name, background one tone above
//!   line 1         minibuffer  GLUED to it — what is typed
//!   lines 2..n     expansion   which-key, candidates, history, manual
//! ```
//!
//! The pair that is read together appears together, and what grows grows away
//! from it. The expansion has a CEILING (`expansion_ceiling`) and what goes
//! past it becomes a page — otherwise the footer would grow with the content
//! until it stopped being a footer (invariant 14).
//!
//! # About the exclusive zone
//!
//! It GROWS WITH THE SURFACE: expanding the list pushes the buffers up instead
//! of covering them.
//!
//! This reverses section 4.3 of the prompt, which wanted the list ON TOP
//! "without touching what is already on screen". The owner's decision, and
//! coherent with the project's reference: in Emacs, opening the which-key
//! window RESIZES the others — nothing stays hidden behind it. Covering the
//! content hides precisely the line that was being read, and in a system with
//! no popup (invariant 14) the footer's surface is the only thing that could
//! cover anything.
//!
//! The cost is the reflow: the buffers get a `configure` on every expansion.
//! It is the same cost Emacs pays, and for the same reason.

use anyhow::{Context as _, Result};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{self, EventLoop, channel},
        calloop_wayland_source::WaylandSource,
    },
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
    },
};
use wayland_client::{
    Connection, Proxy as _, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
};

use crate::leader::{Effect, Leader, Order};

/// Where a native buffer's component is — read from the ENVIRONMENT, by the
/// name of the variable that carries it.
///
/// The five that exist: `RONOCTUA_BUFFER_GLYPHS` (`LEADER i n`), `_EDITOR`
/// (`LEADER f f`), `_UNICODE` (`i u`), `_LUCIDE` and `_MEMORY`. The last two
/// carry something the others do not: Lucide (`i l`) also depends on a FACE —
/// the `RONOCTUA_FONT_LUCIDE` embedded in the renderer, and the two come out of
/// the same derivation, without which the 1,668 icons would be tofu —, and the
/// memory panel (`LEADER s m`) is the only one that embeds no database at all:
/// everything it draws is read by its host at that moment and pushed in, which
/// is what `capability::system` exists for.
///
/// # Why not `option_env!`, which is what this was
///
/// Baked in, a store path is a REFERENCE, and the reference is charged for at
/// the wrong end: the workspace's derivation was handed the five `.wasm` in its
/// `env`, so editing one COMMENT in any buffer changed its hash and recompiled
/// everything — the whole workspace, clippy and the tests, minutes of Rust for
/// a value that is pure data and that no `.rs` here reads at compile time.
///
/// It is the icon superpack's lesson (`nix/rust.nix`) reached from the other
/// side: there the baked path kept a switch from switching, here it kept the
/// loop from being a loop.
///
/// What supplies it is a WRAPPER around this binary, built by `nix/rust.nix`:
/// the reference lives one derivation above the compilation, so the closure
/// still carries the five and no launcher has to remember anything. Absent —
/// `cargo build` outside Nix, which is the case this keeps compiling — the
/// command says so on screen instead of the build breaking.
fn buffer_component(variable: &str) -> Option<String> {
    std::env::var(variable).ok().filter(|v| !v.is_empty())
}

/// The system's URL opener, for the Syncthing interface.
///
/// By its STORE PATH and not by name: in a systemd unit the `PATH` is minimal,
/// and the failure would show up far from the cause (CLAUDE.md). `option_env!`
/// by the same contract as the buffers — `cargo build` outside Nix keeps
/// compiling, and the command warns on screen.
const XDG_OPEN_PATH: Option<&str> = option_env!("RONOCTUA_XDG_OPEN");

/// The process monitor — `Ctrl+Alt+Esc` and `LEADER+Esc`.
///
/// By its STORE PATH, and for the sharpest version of the reason: this one is
/// spawned when the machine is already in trouble, inside a scope whose `PATH`
/// is minimal. A name that does not resolve would fail exactly when the person
/// most needs to see what is happening.
const BTOP_PATH: Option<&str> = option_env!("RONOCTUA_BTOP");

/// The other one — `LEADER+S-Esc`. Same contract.
const BOTTOM_PATH: Option<&str> = option_env!("RONOCTUA_BOTTOM");

/// The Wayland client's state.
struct Footer {
    /// The connection, KEPT on purpose.
    ///
    /// The contract of `Gpu::new` requires the `wl_display` to outlive the `Gpu`.
    /// While the connection only existed as a local variable of `run`, that held
    /// by accident. Kept here, it holds by construction — and it gives access to
    /// the `flush`, which whoever paints outside a Wayland callback needs.
    conn: Connection,
    registry: RegistryState,
    outputs: OutputState,
    _compositor: CompositorState,
    layer_shell: LayerShell,
    requested_height: u32,

    /// ONE SURFACE PER OUTPUT (section 9.2).
    ///
    /// Empty until there is an output: creating the surface before enumerating
    /// makes the compositor close it right away. See `run`.
    screens: Vec<Screen>,
    quit: bool,

    /// Is the locator currently picking a FILE, and not a buffer?
    ///
    /// The same list serves both, so what tells them apart is this — and it is
    /// a field and not a guess from the id, because a path and a window id are
    /// both just strings and confusing them would open a window numbered after
    /// a filename.
    picking_file: bool,

    /// What the compositor says about windows and focus. Fed by the daemon's
    /// thread; it lives HERE and not in the surface, for the reason of section 9.2
    /// explained in `daemon.rs`.
    niri: ronoctua_niri::State,

    // --- leader and keyboard (section 7.4) ---
    seats: SeatState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    /// Modifiers held RIGHT NOW. The key event does not carry them; they arrive in
    /// an event of their own and hold until the next one.
    mods: Modifiers,
    /// The ones already held when the footer gained the keyboard.
    ///
    /// # Why they have to be discounted
    ///
    /// In the nested session the leader **is** `Ctrl+Alt+Space`: when the sequence
    /// starts, Ctrl and Alt are physically pressed, because the hand has just
    /// pressed them. Demanding that they be released before typing `i` is a trap
    /// — and it is the one the screen fell into, showing
    /// `CTRL ALT SPC M-n is not bound`.
    ///
    /// Discounting what was already held gets the other case for free, which is
    /// worse because no gesture undoes it: a modifier stuck in the compositor.
    /// Leaving for a host shortcut (`Super+Space`) and coming back makes the
    /// nested niri miss the key release; from then on it announces the modifier
    /// forever, **every** binding stops matching, and there is nothing on screen
    /// saying why.
    ///
    /// `None` while there is no focus. It is taken on the first modifier event
    /// after the `enter` — which is when the compositor says what holds —, and
    /// **again every time the leader starts**.
    inheritance: Inheritance,
    leader: Leader,
    /// The interface texts in the active language. Loaded once at startup:
    /// switching language at runtime would require reloading the bundle, and
    /// nothing in the system does that today.
    texts: ronoctua_i18n::Texts,
    tree: ronoctua_keymap::Tree,
    /// The which-key lines on display. Empty when the leader is not running.
    inputs: Vec<ronoctua_keymap::Entry>,

    // --- the focused buffer's own keymap (section 7.9) ---
    /// What the focused buffer publishes: `(sequence, command id)`.
    ///
    /// Read ONCE, when the leader opens, and not on every key: asking a second
    /// process mid-sequence would put a socket round trip between a keystroke
    /// and the screen. Empty when the focused window is not one of our buffers,
    /// and THAT is what makes `SPC m` absent rather than empty — a submenu that
    /// opens onto nothing is the same lie as a list with no way back.
    buffer_bindings: Vec<(String, String)>,
    /// Whose. It is the pid `ronoctua_niri::Window` already carries.
    buffer_pid: Option<u32>,
    /// The keys typed since `SPC m`, the `m` itself included.
    ///
    /// The buffer's bindings are resolved HERE and not by `ronoctua_keymap`:
    /// that crate's `Binding` holds `&'static str`, and these arrive at runtime.
    /// The sequences are two or three keys deep, so what would be a tree is a
    /// filter over a list.
    buffer_pending: Vec<String>,

    // --- notifications (section 4.2) ---
    /// One at a time, the rest waiting. It lives in the daemon, like the niri
    /// state and for the same reason of section 9.2.
    notifications: ronoctua_notify::Queue,
    /// Is the history open? It uses THIS surface, growing below the minibuffer —
    /// never a second one (invariant 14).
    history_open: bool,
    /// How the leader shows up on screen — see [`crate::leader`].
    leader_label: String,
    /// The echo area: the shell's last message and when it expires.
    ///
    /// # Why it exists
    ///
    /// Without it, a key that is bound to nothing ends the leader **silently** —
    /// indistinguishable from "it did not register the key". In Doom, the echo
    /// area says `SPC g x is not bound`, and that is what turns a mute error into
    /// information.
    echo: Option<(String, std::time::Instant)>,
    /// The `M-x`. It uses THE surface too: prompt on the minibuffer's line and
    /// candidates in the expanded region, the same mechanism as which-key.
    mx: crate::minibuffer::Minibuffer,
    /// The seat's pointer, when there is one.
    ///
    /// The footer never needed a pointer: it has nothing clickable (invariant 14).
    /// It came to need one for ONE reason — the manual's scrollbar only shows up
    /// when the mouse is over the surface, and knowing that requires receiving the
    /// events.
    pointer: Option<wl_pointer::WlPointer>,
    /// The history of what was copied. Memory only — see `ronoctua_clipboard`.
    clipboard: ronoctua_clipboard::Queue,
    /// Is the clipboard view open in the region?
    clipboard_open: bool,
    /// Where the cursor is, inside the history.
    clip_cursor: usize,
    /// The selected item's QR, when asked for. `None` = there is no QR on screen.
    clip_qr: Option<ronoctua_clipboard::qr::Matrix>,
    /// The focused item's image, already decoded, and which item it belongs to.
    ///
    /// Decoding costs: a 4K capture is 33 MB of RGBA. Doing it every frame would
    /// put that into the frame budget; kept here, it happens once per item — and
    /// the index says when the work needs to be redone.
    clip_image: Option<(usize, ronoctua_render::image::Rgba)>,
    /// How many previews have already been decoded.
    ///
    /// It is the marker that tells the screen whether its texture is still valid.
    /// Comparing the dimensions would not do: two captures of the same monitor
    /// have the same size and are different images — and the screen would show the
    /// previous one.
    clip_version: u64,
    /// Fraction of a detent not yet spent by the wheel or by the touchpad.
    ///
    /// Without it the touchpad does not scroll: each of its events is worth a
    /// small fraction, and truncating event by event would give zero forever.
    /// Keeping the remainder, the pieces add up — which is what turns the
    /// continuous gesture into lines.
    detents: f64,
    /// Is the pointer OVER the footer right now?
    under_pointer: bool,
    /// The scrollbar is being DRAGGED right now.
    ///
    /// Without this state, the button pressed over the bar fell into the same path
    /// as a click on an item: the list scrolled and a command was run along with
    /// it, which is the worst kind of surprise — the person wanted to look and the
    /// system acted.
    dragging: bool,
    /// Which line of the visible PAGE is under the pointer.
    ///
    /// An index into the page, and not into the whole list: it is what the drawing
    /// positions, and converting here would avoid a second scroll calculation.
    row_under_pointer: Option<usize>,
    /// Is the shortcut manual open?
    ///
    /// It is the fourth use of the expanded region, and the second MODAL one:
    /// while it is open the keyboard is its, because the arrows scroll. Keeping a
    /// boolean and not the finished lines is deliberate — the lines are assembled
    /// from `actions::acordes()` on every draw, and a stored copy would be the
    /// second truth the manual exists in order not to be.
    manual: bool,
    /// The first visible line of the expanded region.
    ///
    /// The region has a ceiling (`expansion_ceiling`), and what goes past it only
    /// shows up by scrolling: `CtrlRight`+arrows page through, and in the manual
    /// the arrows alone step line by line.
    offset: usize,
    /// Is the RIGHT `Ctrl` held?
    ///
    /// # Why it is tracked by hand
    ///
    /// Wayland's `Modifiers` does not tell left from right: xkb has already merged
    /// the two into a single bit. What tells them apart is the key's keysym, and
    /// that is why it is read on the press and on the release.
    ///
    /// # Why it exists
    ///
    /// It is the key that does **not add itself to the leader**: with it held, the
    /// key goes to the navigation of the expanded region instead of entering the
    /// sequence. Without a key like that, paging would require spending a letter
    /// of the keymap — and the letter spent would be one less for a command.
    right_ctrl: bool,
    /// The command registry, for the `M-x` to complete over.
    command_registry: ronoctua_cmd::Registry,
    /// How many frames have been painted. Only to time the first ones.
    frames_painted: u8,
    /// Where the launching thread hands back its verdict.
    ///
    /// An `Option` because the `Footer` is also built outside the event loop
    /// (tests), and there there is no channel — nor is there anything to launch.
    launch_sender: Option<calloop::channel::Sender<crate::foreign::Launch>>,

    /// Says "the radio has finished a scan" — see [`Self::watch_scans`].
    ///
    /// `Some` also means the thread is ALREADY up: raising a second one would
    /// give a second connection to NetworkManager and two re-reads per scan.
    scan_sender: Option<calloop::channel::Sender<()>>,

    /// Is the thread already up? See [`Footer::watch_scans`].
    scan_watching: bool,

    // --- app finder (`LEADER a`) ---
    /// The apps view, when open.
    ///
    /// The `Option` **is** the open/closed. A parallel boolean beside the state
    /// could diverge from it, and the symptom would be the region reserving height
    /// for a list that does not exist — that is how the footer once asked for
    /// twenty-two lines to draw six.
    apps: Option<crate::apps::Finder>,

    // --- network (`LEADER r r`) ---
    /// The network view, when open.
    ///
    /// Same design as the app finder, and for the same reason: the `Option` **is**
    /// the open/closed, and there is no parallel boolean that can disagree with it.
    network: Option<crate::network::ViewState>,

    /// The freshly typed password, waiting for the answer to "remember?".
    ///
    /// It lives for the span of one key. It is the only point of the footer where
    /// a cleartext password is kept, and that is why it is **cleared on both
    /// branches** of the answer — including the "no", where it is tempting just to
    /// ignore it.
    network_remember: Option<(String, String)>,

    /// Everything the system offers, swept ONCE at startup.
    ///
    /// It is born empty and arrives by the thread. Opening before it arrives shows
    /// an empty list with the explanation on screen — which is honest, and lasts
    /// the milliseconds of the sweep.
    catalogue: crate::apps::Catalogue,
    /// When the `applications` directories were last touched when they were swept.
    ///
    /// It is what says whether the catalogue got old, without re-sweeping to find out.
    apps_stamp: Vec<Option<std::time::SystemTime>>,
    /// Where the sweeping thread hands back the catalogue.
    catalogue_sender: Option<calloop::channel::Sender<crate::apps::Catalogue>>,
    /// The already decoded icon pixmaps, by their `Icon=`.
    ///
    /// `Some(None)` is an ANSWER — "this name has no file" —, and not the absence
    /// of one. Without telling the two apart, an icon that does not exist would be
    /// asked for again on every frame, and the thread would sweep 335 thousand
    /// files sixty times a second.
    icons: std::collections::HashMap<String, Option<ronoctua_render::image::Rgba>>,
    /// What has already been asked for and has not come back yet.
    icons_requested: std::collections::HashSet<String>,
    /// The icon thread's queue.
    icon_queue: Option<std::sync::mpsc::Sender<crate::apps::Request>>,
}

/// What the expanded region is showing RIGHT NOW.
///
/// # Why this enum exists
///
/// The precedence order between the six views was written in FOUR places — the
/// height, the navigation, the drawing and the keyboard grab —, and the only
/// thing keeping the footer standing was the four agreeing. They diverged
/// once, and the symptom was the surface asking for twenty-two lines to draw six.
///
/// # Why it is DERIVED, and not stored
///
/// Because the flags are not really mutually exclusive: `history_open` is
/// passive and coexists with the leader. Storing the enum would create a
/// second truth that can diverge from the flags; deriving it makes the order
/// be written **once** without changing who is in charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewState {
    /// The clipboard history. Modal.
    Clipboard,
    /// The shortcut manual. Modal.
    Manual,
    /// The app finder. Modal.
    Applications,
    /// The network list. Modal.
    Network,
    /// The `M-x`. Modal.
    Commands,
    /// A leader sequence under way.
    Leader,
    /// The notification history. Passive.
    History,
    /// The region is closed.
    NoneOf,
}

impl ViewState {
    /// Does the view have the whole keyboard?
    ///
    /// The leader also holds the keyboard — it is a sequence under way —, and the
    /// history does not: it is passive and gives the key to whoever wants it.
    const fn grabs(self) -> bool {
        !matches!(self, Self::History | Self::NoneOf)
    }
}

/// The surface of ONE output.
///
/// # Why one per output, and not a single one
///
/// The Emacs analogy decides it: the modeline is per window, the minibuffer is
/// per frame, and output ~ frame. Two lines on EVERY output — that way the
/// exclusive zone is uniform and **the content does not reflow when focus
/// changes screen**. On the unfocused outputs the bottom line is passive.
///
/// # What is NOT here
///
/// Node state of the minibuffer, of the leader or of the notifications. That
/// lives in the `Footer`, which is the daemon: if the focused output vanishes
/// in the middle of a prompt, the state migrates without losing what was
/// typed. Keeping it here would mean losing the prompt along with the monitor.
struct Screen {
    /// The output this surface covers. Kept so we know which one to remove when
    /// the compositor takes it away.
    output: wl_output::WlOutput,
    layer: LayerSurface,
    width: u32,
    height: u32,
    /// The MONITOR's height, not the surface's.
    ///
    /// The expansion's ceiling is a percentage of the screen, and the surface does
    /// not know its own size: it knows what we asked for. Without this number, the
    /// ceiling would only be the absolute one in lines — which on a short screen
    /// is half the screen.
    output_height: u32,
    configured: bool,
    /// Assembled on the first `configure`, when there is a size. They stay `None`
    /// if there is no usable GPU — the footer still resolves the layout and logs,
    /// instead of bringing the session down because of a driver.
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    /// The modeline's background and the row highlight. See `ronoctua_render::quad`.
    quads: Option<ronoctua_render::quad::Quads>,
    /// The preview of the copied image. One per screen, and one at a time.
    images: Option<ronoctua_render::image::Images>,
}

/// One line of the expanded region, ready to draw.
///
/// Two columns: the key (when there is one) and the text. It is the form
/// common to the four uses of the region — which-key, `M-x` candidates,
/// history and manual —, and it is what lets height and drawing read the
/// SAME list.
struct ExpansionRow {
    /// A glyph before everything else, in the margin. `None` = the line has no icon.
    ///
    /// Only the app finder uses it: it is what answers "which program is this"
    /// before the eye reaches the name. It is a GLYPH of the font (invariant
    /// 10); the app's own pixmap, when it exists, is drawn on top at another
    /// speed — see [`crate::apps`].
    icon: Option<char>,
    /// The key of this line's PIXMAP, when it is already decoded.
    ///
    /// When there is a pixmap there is no glyph, and vice versa: the two in the
    /// same place would draw one over the other. Whoever chooses is
    /// [`Footer::expansion`], in a single place — the drawing only obeys.
    pixmap: Option<String>,
    /// The key, in the left column. Empty when there is none.
    key: String,
    /// The main text — the **command id**, when there is one.
    text: String,
    /// What the message area shows when the pointer goes over it.
    ///
    /// The list shows the command; the description stays here, and shows up in
    /// the minibuffer under the cursor. It is the Doom split: the list is to be
    /// swept with the eye, and the whole sentence only when asked for.
    description: Option<String>,
    /// What a click on this line does. `None` = line is not clickable.
    click: Option<Click>,
    /// Which CHARACTERS of the key matched the query, to light up.
    ///
    /// Empty when there is no search. With the list filtered by subsequence,
    /// without this it does not say **why** each item is in it.
    highlights: Vec<usize>,
    /// Is the key flush RIGHT within its own column?
    ///
    /// For a shortcut, yes: it is what lines the colon up with `RET` and with
    /// `a` in the same list. For a command name (the finder), no — one hundred
    /// and seventy names flush right would give a jagged left margin.
    key_flush_right: bool,
    /// Does the `:` come out between the two columns?
    ///
    /// It comes out where the left column is a KEY or a command id — there it
    /// says "this leads to that". It does not come out where the left one is an
    /// application's name and the right one is its description: `Firefox :
    /// navegador web` asserts a relation that does not exist, and the column's
    /// ruler already separates the two.
    separator: bool,
    key_color: tokens::Color,
    text_color: tokens::Color,
}

/// What a click on a line of the expansion does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Click {
    /// Press this key — the same thing the person would do on the keyboard.
    ///
    /// Uniform on purpose: on a group, it goes down one level; on a command, it
    /// runs. Node new path, and nothing that only the mouse reaches.
    Key(ronoctua_keymap::Key),
    /// Choose the candidate at index `n` in the finder and confirm.
    Candidate(usize),
    /// Open the application at index `n` of the visible list.
    ///
    /// Its OWN variant, and not `Candidate`: the two lists take the same region
    /// and the same line, and a click on the application list that fell into
    /// `mx.choose()` would run an `M-x` command — the index exists in both, so
    /// nothing would fail; something else would come out.
    Application(usize),
    /// Put the focus on line `n` of the network list.
    ///
    /// Its own variant for the same reason as `Application`: the lists share the
    /// same region and the same line, and a click that fell into the other's
    /// dispatch would find the index existing and do something else — without
    /// failing.
    ///
    /// FOCUS and not connect. Connecting asks for a password, and a click that
    /// fires a password prompt by accident is worse than a click that does
    /// nothing; the `Enter` is still the one that acts.
    Network(usize),
}

/// The signal's glyph, in four steps.
///
/// Converting a number into a step belongs to `crate::network::bars`; here
/// only the token is chosen. Two places deciding the same range would give a
/// bar that does not match the other part of the screen.
const fn signal_bars(signal: u8) -> char {
    match crate::network::bars(signal) {
        0 => tokens::icon::WIFI_NONE,
        1 => tokens::icon::WIFI_WEAK,
        2 => tokens::icon::WIFI_FAIR,
        3 => tokens::icon::WIFI_GOOD,
        _ => tokens::icon::WIFI_STRONG,
    }
}

impl ExpansionRow {
    /// A line with no key, in the colors of a plain label.
    const fn empty_one() -> Self {
        Self {
            icon: None,
            pixmap: None,
            key: String::new(),
            text: String::new(),
            highlights: Vec::new(),
            description: None,
            click: None,
            key_flush_right: true,
            separator: true,
            key_color: tokens::color::STATE_ACCENT,
            text_color: tokens::color::TEXT_SECONDARY,
        }
    }

    /// How much this line measures, for the grid.
    fn measurement(&self) -> Measure {
        Measure {
            key: layout::width(&self.key),
            text: layout::width(&self.text),
            icon: self.icon.is_some() || self.pixmap.is_some(),
        }
    }
}

/// How much a line of the expansion measures, in cells.
///
/// It exists because the grid needs the MAXIMA of the whole list to line the
/// columns up, and passing the list of finished lines would force `grid_for`
/// to know about color, click and highlight — which have nothing to do with
/// layout arithmetic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Measure {
    /// The left column: key, or command id, or application name.
    key: u16,
    /// The right column.
    text: u16,
    /// Does the line have an icon before everything else?
    icon: bool,
}

/// Breaks a text into runs of "matched" and "did not match".
///
/// Gives back `(span, relative column, matched)`. One run per color change,
/// and not one per character: the cell is exact, so each run's column is
/// arithmetic — and shaping letter by letter would multiply the renderer's
/// work by twenty on a list of one hundred and seventy items.
///
/// A pure function for the usual reason: it is arithmetic over text, and the
/// rest of the drawing does not exist outside a compositor.
fn runs_of(text: &str, highlights: &[usize]) -> Vec<(String, u16, bool)> {
    if highlights.is_empty() {
        return if text.is_empty() {
            Vec::new()
        } else {
            vec![(text.to_owned(), 0, false)]
        };
    }
    let mut out: Vec<(String, u16, bool)> = Vec::new();
    for (i, c) in text.chars().enumerate() {
        let casou = highlights.contains(&i);
        match out.last_mut() {
            // Joins the previous run when the color does not change.
            Some((s, _, anterior)) if *anterior == casou => s.push(c),
            // ONE CELL PER CHARACTER, the way `layout::width` counts — and not
            // by the Unicode width. The two rulers have to be the SAME: with one
            // counting two for an ideograph and the other one, the highlight
            // would come out offset from the text it was supposed to light.
            _ => out.push((c.to_string(), u16::try_from(i).unwrap_or(u16::MAX), casou)),
        }
    }
    out
}

/// How many lines stay between the focused item and the region's edge.
///
/// Two: enough for the next item to be on screen already when one decides to
/// go down, without spending half the region on context. It is vi's `scrolloff`.
const SCROLL_MARGIN: usize = 2;

/// How the expanded region is navigated — and there are TWO behaviors, not one.
///
/// # Why the distinction exists
///
/// A manual is content longer than the screen: it **scrolls**, and what says
/// where you are is the bar. A `1/2` counter there would not help — the
/// position in running text is not a page number.
///
/// A shortcut list is the opposite: it shows **what fits** and the rest is
/// another page. A scrollbar there would be a continuous control for content
/// that moves in jumps, and what is left to say is how many pages there are
/// and which key turns them.
///
/// Mixing the two gives the worst of both: a bar that jumps and a counter
/// that lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Navigation {
    /// It scrolls. Scrollbar, no counter.
    Scroll,
    /// It paginates. Counter with the key hint, no bar, in columns.
    Page,
}

/// Breaks a text into lines of up to `width` cells.
///
/// It breaks by WORD when it can, and in the middle of the word when the word
/// is longer than the line — the case of a long address, which is precisely
/// what gets copied most. Always cutting at the limit would split short words
/// for nothing; never cutting would leave the address invisible.
fn wrap_text(text: &str, width: u16, ceiling: usize) -> Vec<String> {
    let width = usize::from(width.max(1));
    let mut out = Vec::new();
    for bruta in text.lines() {
        let mut line = String::new();
        for palavra in bruta.split_whitespace() {
            if line.is_empty() && palavra.chars().count() > width {
                // WORD LONGER THAN THE LINE: split it into pieces the size of the
                // line. `chars`, and not bytes: cutting a UTF-8 sequence in half
                // would produce garbage on screen.
                let mut rest: Vec<char> = palavra.chars().collect();
                while rest.len() > width && out.len() < ceiling {
                    out.push(rest.drain(..width).collect());
                }
                line = rest.into_iter().collect();
                continue;
            }
            let fits = line.chars().count() + 1 + palavra.chars().count() <= width;
            if line.is_empty() {
                line.push_str(palavra);
            } else if fits {
                line.push(' ');
                line.push_str(palavra);
            } else {
                out.push(std::mem::take(&mut line));
                line.push_str(palavra);
            }
            if out.len() >= ceiling {
                return out;
            }
        }
        out.push(line);
        if out.len() >= ceiling {
            return out;
        }
    }
    out
}

/// One line of the panel, broken into the colored spans of the highlight.
///
/// With no language — or with no range at all — a single span comes out, in
/// the usual color: colored prose is worse than gray prose, because whoever
/// reads it starts looking for meaning in a color that means nothing.
///
/// The color of each role is chosen HERE, and not by the highlighter: it
/// gives back a role, and a color token is the host's business — the same
/// border the buffers' colors already respect.
fn highlight_line(
    line: &str,
    tongue: Option<ronoctua_clipboard::highlight::Language>,
    y: u16,
    column: u16,
) -> Vec<ronoctua_render::Span> {
    use ronoctua_clipboard::highlight::Role;

    let simple = |text: &str| ronoctua_render::Span {
        size: None,
        text: text.to_owned(),
        line: y,
        column,
        color: tokens::color::TEXT_PRIMARY,
    };
    let Some(l) = tongue else {
        return vec![simple(line)];
    };
    let ranges = ronoctua_clipboard::highlight::ranges(line, l);
    if ranges.is_empty() {
        return vec![simple(line)];
    }

    let mut out = Vec::new();
    let mut cursor = 0usize;
    let mut empurrar = |text: &str, kdl_of: usize, color| {
        if text.is_empty() {
            return;
        }
        out.push(ronoctua_render::Span {
            size: None,
            text: text.to_owned(),
            line: y,
            // THE COLUMN IS IN CELLS, and the range's offset is in BYTES.
            // Counting characters is what matches the two: an accent takes two
            // bytes and one cell, and adding bytes would misalign the whole line
            // from the first one on.
            column: column + u16::try_from(line[..kdl_of].chars().count()).unwrap_or(0),
            color,
        });
    };

    for f in ranges {
        empurrar(&line[cursor..f.start], cursor, tokens::color::TEXT_PRIMARY);
        let color = match f.role {
            Role::Comment => tokens::color::TEXT_DIM,
            Role::Text => tokens::color::STATE_SUCCESS,
            Role::Number => tokens::color::STATE_WARNING,
            Role::Word => tokens::color::TEXT_GROUP,
            Role::Normal => tokens::color::TEXT_PRIMARY,
        };
        empurrar(&line[f.start..f.end], f.start, color);
        cursor = f.end;
    }
    empurrar(&line[cursor..], cursor, tokens::color::TEXT_PRIMARY);
    out
}

/// A multi-line text on a single line.
///
/// The history list is one line per item: a text of five lines would take
/// five items, and the history would become a single item on the screen.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// How much of the width the clipboard list takes.
///
/// Two fifths: the list is for recognizing the item (the first words are
/// enough), and the content is for reading. Splitting it in half would give
/// the two the same importance, and the content is what one looks at.
///
/// The floor exists because on a narrow screen the proportion alone would
/// give a column that shows no word at all.
fn list_columns(columns: u16) -> u16 {
    const FLOOR: u16 = 16;
    (columns * 2 / 5).max(FLOOR).min(columns.saturating_sub(2))
}

/// How many cells separate one column from the next in the shortcut list.
///
/// Two: enough for the eye to see two columns and not one long line, without
/// spending width that the labels would use.
const GAP_BETWEEN_COLUMNS: u16 = 2;

/// How many cells the icon of a line of the expansion reserves.
///
/// Two: **one** of ink and **one** of breathing room. The icon glyph is drawn
/// at the text's body, and at that size it advances one cell — the same
/// premise the modeline has used for a long time, and which `test-faces`
/// measures against the actual font instead of leaving it written only here.
/// Without the breathing room, the first letter of the name would touch the
/// icon's ink.
const ICON_COLUMNS: u16 = 2;

/// Where each item of the page lands, and how much room that takes.
///
/// # Why in columns
///
/// It is Doom's arrangement: with the region limited in height, a list of ten
/// shortcuts in a single column wastes the screen's whole width and would
/// still paginate. Filling the width first, the same ten fit in three lines —
/// and the eye sweeps a block instead of a strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Grid {
    /// How many columns fit side by side.
    columns: usize,
    /// How many rows each column has.
    lines: usize,
    /// The width of a column, in cells.
    width: u16,
    /// The colon's column, INSIDE each column of the grid.
    ///
    /// **Already limited to half the screen.** The limit used to live in the
    /// drawing, and that is why the height, the pagination and the click read
    /// one number and the drawing read another: on a narrow screen,
    /// `niri-switch-focus-between-floating-and-tiling` pushed the text past
    /// the edge — and the compositor does not clip, it simply does not draw.
    /// Here there is a single number.
    ///
    /// It is computed from the WHOLE list and not from the page: aligning per
    /// page would make the description jump column on every scroll, and the
    /// list would look like another one. It used to be fixed at 12 cells,
    /// which fits `C-M-S-s-X`. It stopped fitting when the finder started
    /// showing the command's id: `niri-move-window-to-workspace-down` has 34
    /// cells and the description ended up on top of it.
    colon: u16,
    /// How many cells the icon reserves before everything. 0 = the list has none.
    ///
    /// It belongs to the GRID and not to the line: a line with no icon in the
    /// middle of a list with icons must not pull its text to the left.
    icon: u16,
}

impl Grid {
    /// How many items fit on a page.
    const fn per_page(self) -> usize {
        self.columns * self.lines
    }

    /// Where the left column starts, inside the grid's column.
    const fn start(self) -> u16 {
        tokens::space::MODELINE_EDGELEFT + self.icon
    }

    /// Where the RIGHT column starts, inside the grid's column.
    ///
    /// With no key it takes the whole line: that is the case of the manual and
    /// of the history, where there are no two columns to line up.
    const fn text_start(self, with_key: bool) -> u16 {
        if with_key {
            self.colon + 2
        } else {
            self.start()
        }
    }

    /// How many cells the right column has, in the grid's column `base`.
    ///
    /// # Why it belongs to the grid, and not to the drawing
    ///
    /// Because it is the "nothing leaks off the screen" arithmetic, and it has
    /// to hold for the last column of the grid as it does for the first. Written
    /// inside the painting loop, it was an expression nobody could exercise —
    /// and until now **nothing** in the expanded region elided: a description of
    /// two hundred cells went over the scrollbar and out past the edge, which
    /// the compositor does not clip.
    ///
    /// Two limits, and the smaller one holds: what is left of the COLUMN (so as
    /// not to invade the next column) and what is left of the SCREEN (so as not
    /// to go out past the edge).
    const fn fits_in_text(self, with_key: bool, base: u16, colunas_da_tela: u16) -> u16 {
        let start = self.text_start(with_key);
        let na_coluna = self
            .width
            .saturating_sub(start.saturating_add(GAP_BETWEEN_COLUMNS));
        let na_tela = colunas_da_tela.saturating_sub(base.saturating_add(start));
        if na_coluna < na_tela {
            na_coluna
        } else {
            na_tela
        }
    }

    /// Where the item at index `i` of the page lands: `(column in cells, row)`.
    ///
    /// Filled **by column**: it goes down the first one to the end, then moves
    /// to the next. It is how the Emacs which-key fills, and it is what keeps
    /// the alphabetical order legible from top to bottom.
    const fn position(self, i: usize) -> (u16, u16) {
        let column = i / self.lines;
        let line = i % self.lines;
        ((column as u16).saturating_mul(self.width), line as u16)
    }
}

/// Builds the grid for a list, on a screen of `colunas_da_tela` cells.
///
/// A pure function for the usual reason: it is arithmetic, and what wraps it
/// does not exist outside a compositor.
fn grid_for(widths: &[Measure], colunas_da_tela: u16, ceiling: usize, em_colunas: bool) -> Grid {
    let maior_tecla = widths.iter().map(|m| m.key).max().unwrap_or(0);
    let maior_texto = widths.iter().map(|m| m.text).max().unwrap_or(0);
    // THE ICON BELONGS TO THE LIST, and not to the line: ONE line with an icon
    // reserves the column for ALL of them, otherwise the ones with no icon would
    // pull the text left and the margin would become a zigzag.
    let icon = if widths.iter().any(|m| m.icon) {
        ICON_COLUMNS
    } else {
        0
    };
    // HALF THE SCREEN IS THE LEFT COLUMN'S CEILING. Without it, an id like
    // `niri-switch-focus-between-floating-and-tiling` pushes the text past
    // the edge — and the compositor does not clip, it just does not draw.
    let colon = maior_tecla
        .saturating_add(tokens::space::MODELINE_EDGELEFT + icon + 1)
        .min(colunas_da_tela / 2);
    let width = colon
        .saturating_add(2)
        .saturating_add(maior_texto)
        .saturating_add(GAP_BETWEEN_COLUMNS)
        // AND NOTHING GOES PAST THE SCREEN. On a single-column list, the width
        // feeds `item_at`: a column wider than the screen would make the
        // pointer find an item where there is no pixel at all.
        .min(colunas_da_tela.max(1));

    let n = widths.len();
    // AN EMPTY LIST IS ONE COLUMN, and not thirteen of minimum width. The number
    // does not change what shows up — there is nothing —, but it feeds `item_at`,
    // and thirteen empty columns are thirteen places where the pointer would find an item.
    if n == 0 {
        return Grid {
            columns: 1,
            lines: 1,
            width,
            colon,
            icon,
        };
    }
    // THE WIDTH FIRST, and the height after.
    //
    // A list of ten shortcuts in a region of twelve lines would fit in a single
    // column — and it would be a narrow strip with the whole screen empty beside
    // it. Using the columns that fit, the same ten become three lines.
    let columns = if em_colunas && width > 0 {
        (usize::from(colunas_da_tela) / usize::from(width)).max(1)
    } else {
        1
    };
    let lines = n.div_ceil(columns).min(ceiling).max(1);
    Grid {
        columns,
        lines,
        width,
        colon,
        icon,
    }
}

/// The first line of the expanded region.
///
/// Line 0 is the modeline, line 1 the minibuffer. The number used to appear
/// loose in the drawing and appeared again when the pointer needed to know
/// which item it is over — two calculations of the same number, and the one
/// that gets it wrong is the one nobody looks at: the cursor would describe
/// the line above.
pub const FIRST_OF_EXPANSION: u16 = 2;

/// The mouse's left button, in Linux's input protocol.
///
/// `BTN_LEFT` from `linux/input-event-codes.h`. Wayland passes the raw evdev
/// code, with no enum.
const LEFT_BUTTON: u32 = 0x110;

/// How many lines a wheel detent steps where there is scrolling.
///
/// Three, which is what every text viewer does. One line per detent would
/// turn reading a manual into a wrist exercise; one page per detent is the
/// other mode's behavior, and mixing the two would make the same wheel mean
/// different things with nothing on screen saying which.
const LINES_PER_DETENT: f64 = 3.0;

/// How many "detents" an axis event is worth.
///
/// # Why three sources, and not one
///
/// Wayland has three ways of saying the same thing, and a client that reads
/// only one of them works on half the devices:
///
/// - `value120` is today's, in 120ths of a detent — it is what gives a
///   high-resolution wheel without losing the step;
/// - `discrete` is the old one, an integer per detent, and it is still what
///   arrives from an older compositor;
/// - `absolute` is the continuous one, in **finger pixels**, and it is the
///   only one the touchpad sends.
///
/// The touchpad is converted by the LINE HEIGHT, and not by a number of
/// pixels picked by hand: that way the finger moves the content in the same
/// proportion as it moves on screen, and the arithmetic recalibrates itself
/// if the font changes. Without that it would be one more visual value
/// outside the tokens (invariant 9).
fn axis_detents(v: &smithay_client_toolkit::seat::pointer::AxisScroll, linha_px: f64) -> f64 {
    if v.value120 != 0 {
        return f64::from(v.value120) / 120.0;
    }
    if v.discrete != 0 {
        return f64::from(v.discrete);
    }
    if linha_px <= 0.0 {
        return 0.0;
    }
    v.absolute / (linha_px * LINES_PER_DETENT)
}

/// What separates the key from what it does.
///
/// A colon, and a discreet one: it is Doom's mark, and it takes one cell
/// where a gap of twelve said nothing. The colons of every line stay in the
/// SAME column — that is what makes the list be read as two columns instead
/// of as loose text.
pub const SEPARATOR_OF: &str = ":";

/// The mark of a SUBMENU in the shortcut list.
///
/// The convention comes from Doom, where `+file` says "this opens more keys"
/// and a label with no mark says "this happens now". It is put on by the
/// drawing, and not written into the translation: inside the text, three of
/// the six groups ended up without it and nothing caught it.
pub const GROUP_MARK: &str = "+";

/// What was already held when the sequence started — and is therefore not
/// the sequence's.
///
/// # Why it is a type, and not a loose `Option`
///
/// Because the rule has THREE moments, and they have already diverged: the
/// focus changes, the compositor announces, and the sequence starts. While
/// it was a loose field, only the first two existed — and the third is
/// precisely the one that saves the day when the `leave` does not arrive.
///
/// That is not hypothetical: in a hung session the nested compositor stopped
/// dispatching altogether (it did not even answer `wl_display.sync`, with
/// seven connections without `accept` and 1536 bytes from the footer stuck
/// in the queue). A `leave` lost in that hole leaves the inheritance holding
/// forever, and from then on **every** binding stops matching with nothing
/// on screen saying why.
#[derive(Debug, Clone, Copy, Default)]
struct Inheritance(Option<Modifiers>);

impl Inheritance {
    /// The keyboard focus changed. The previous session's inheritance no longer holds.
    const fn on_focus(&mut self) {
        self.0 = None;
    }

    /// The compositor announced the modifiers. The FIRST reading after the focus
    /// is the inheritance — the protocol requires an announcement right after the `enter`.
    fn on_announcement(&mut self, mods: Modifiers) {
        if self.0.is_none() {
            self.0 = Some(mods);
        }
    }

    /// The sequence has STARTED now: whatever is held is its inheritance.
    ///
    /// With no focus, `mods` is the zeroed state and this makes no difference —
    /// the `enter` that comes next zeroes it again, and the following
    /// announcement captures what holds. With focus (the case where the `leave`
    /// was lost), this is what gives the keyboard back.
    fn on_sequence_start(&mut self, mods: Modifiers) {
        self.0 = Some(mods);
    }

    /// The modifiers that hold for the sequence: the current ones, minus the
    /// inherited ones.
    ///
    /// A modifier pressed **afterwards**, in the middle of the sequence, still
    /// counts. It is what preserves the possibility of a `C-…` binding existing
    /// one day without being born broken.
    fn discount(self, now: Modifiers) -> ronoctua_keymap::Mods {
        let h = self.0.unwrap_or_default();
        ronoctua_keymap::Mods {
            ctrl: now.ctrl && !h.ctrl,
            alt: now.alt && !h.alt,
            // Shift does NOT enter: xkb has already applied it when producing the
            // character. Counting both would turn `F` into `S-F` and never match.
            shift: false,
            logo: now.logo && !h.logo,
        }
    }
}

/// The mark of a PINNED entry in the clipboard history.
///
/// One character, in the column the list already reserves for the key. It is
/// not a font icon on purpose: one cell fits there, and a Nerd Fonts glyph
/// would depend on the icon table to say what an asterisk already says.
pub const PINNED_MARK: &str = "*";

/// Brings the surface up and runs the event loop.
pub fn run() -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("nao ha compositor Wayland acessivel (WAYLAND_DISPLAY esta definida?)")?;
    let (globals, mut fila) = registry_queue_init(&conn).context("registry do Wayland")?;
    let qh: QueueHandle<Footer> = fila.handle();

    let compositor =
        CompositorState::bind(&globals, &qh).context("o compositor nao expoe wl_compositor")?;
    let layer_shell = LayerShell::bind(&globals, &qh).context(
        "o compositor nao expoe zwlr_layer_shell_v1.\n\
         \n\
         A integracao da ronoctua-distro com o compositor e EXCLUSIVAMENTE por\n\
         wlr-layer-shell e IPC (invariante 3). Sem layer-shell nao ha rodape,\n\
         e contornar isso violaria o invariante.",
    )?;

    let altura_rodape = tokens::metrics::footer_height(tokens::font::SIZE_BASE).ceil() as u32;

    // The keymap is assembled BEFORE the surface comes up. A collision here is a
    // program error, and failing at startup is better than failing on the first leader.
    let (tree, registry) = crate::leader::default_of().context("keymap padrao")?;

    // THE COPY CHANNEL IS BORN BEFORE the state, because the `Sender` lives
    // inside it — and the reading thread needs a copy of it.
    let (envio_copias, copias) = channel::channel::<ronoctua_clipboard::Entry>();
    // THE WATCHER COMES UP WITH ITS OWN CONNECTION, in a thread. See
    // `crate::clipboard`: the SCTK `wl_data_device` only hands the selection to
    // whoever has FOCUS, and the footer at rest never has it — that is why the
    // protocol is `zwlr_data_control`, which exists precisely for a clipboard
    // manager.
    crate::clipboard::watch(envio_copias);

    let mut state = Footer {
        conn: conn.clone(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        _compositor: compositor,
        layer_shell,
        requested_height: altura_rodape,
        screens: Vec::new(),
        quit: false,
        picking_file: false,
        niri: ronoctua_niri::State::default(),
        seats: SeatState::new(&globals, &qh),
        keyboard: None,
        mods: Modifiers::default(),
        inheritance: Inheritance::default(),
        leader: Leader::default(),
        texts: ronoctua_i18n::Texts::from_env(),
        tree,
        inputs: Vec::new(),
        buffer_bindings: Vec::new(),
        buffer_pid: None,
        buffer_pending: Vec::new(),
        notifications: ronoctua_notify::Queue::fresh_one(),
        history_open: false,
        leader_label: crate::leader(),
        echo: None,
        mx: crate::minibuffer::Minibuffer::default(),
        pointer: None,
        clipboard: ronoctua_clipboard::Queue::default(),
        clipboard_open: false,
        clip_cursor: 0,
        clip_qr: None,
        clip_image: None,
        clip_version: 0,
        detents: 0.0,
        under_pointer: false,
        dragging: false,
        row_under_pointer: None,
        manual: false,
        offset: 0,
        right_ctrl: false,
        command_registry: registry,
        frames_painted: 0,
        // The channels are hooked up later, along with the other sources.
        launch_sender: None,
        apps: None,
        network: None,
        network_remember: None,
        scan_sender: None,
        scan_watching: false,
        catalogue: std::sync::Arc::default(),
        apps_stamp: Vec::new(),
        catalogue_sender: None,
        icons: std::collections::HashMap::new(),
        icons_requested: std::collections::HashSet::new(),
        icon_queue: None,
    };

    // THE SURFACE IS ONLY CREATED WHEN THERE IS AN OUTPUT.
    //
    // A compositor may legitimately have no output at all for a moment: right
    // after coming up, or with every monitor switched off. A layer-shell
    // surface created at that moment is CLOSED right away, and the only sign is
    // a `closed` a few milliseconds later — nothing in the error says the cause.
    //
    // That is exactly how this bug showed up in a VM test: the client connected
    // as soon as the socket existed, with `outputs=0`, and died silently. The
    // diagnosis only became obvious after logging the count.
    //
    // Creating the surface in response to the OUTPUT solves both cases at once:
    // the cold start and the hotplug section 9.2 of the plan requires.
    fila.roundtrip(&mut state).context("roundtrip inicial")?;
    state.ensure_screens(&qh);

    // TWO event sources: Wayland and niri's IPC. Hence the `calloop` instead of
    // a `blocking_dispatch` in a loop — with a loop blocked on the Wayland
    // socket, a niri event would only be seen on the next mouse movement.
    // Milestone 5 adds keyboard and timers on the same rails.
    let mut cycle: EventLoop<Footer> = EventLoop::try_new().context("calloop")?;
    let alca = cycle.handle();

    // THE LAUNCH VERDICT. The thread that waits for `systemd-run --scope` —
    // which is SYNCHRONOUS, see `foreign::launch` — hands it back through here,
    // and this is where the echo area can be written: it needs `&mut Footer`.
    let (launch_sender, lancamentos) = channel::channel::<crate::foreign::Launch>();
    state.launch_sender = Some(launch_sender);
    alca.insert_source(lancamentos, |event, (), state: &mut Footer| {
        if let channel::Event::Msg(v) = event {
            state.launched(v);
        }
    })
    .map_err(|e| anyhow::anyhow!("nao consegui pendurar o canal de lancamento: {e}"))?;

    // THE END OF A RADIO SCAN.
    //
    // The channel is built here and always: what is lazy is the THREAD, raised
    // by `watch_scans` on the first `LEADER r`. Building the source on demand
    // would mean touching the loop from inside a key handler, which does not
    // have the handle.
    let (scan_sender, scans) = channel::channel::<()>();
    state.scan_sender = Some(scan_sender);
    alca.insert_source(scans, |event, (), state: &mut Footer| {
        if matches!(event, channel::Event::Msg(())) {
            state.rescanned();
        }
    })
    .map_err(|e| anyhow::anyhow!("nao consegui pendurar o canal de varredura: {e}"))?;

    // THE APPLICATION CATALOGUE, SWEPT AT STARTUP.
    //
    // Measured on this machine: 128 entries in 68 ms — four frame budgets.
    // Sweeping on pressing `LEADER a` would give a visible freeze every time the
    // list opened, and the gesture of opening it is precisely the one that has
    // to be instant. Here it happens once, in a thread, while the footer is
    // already standing.
    let (catalogue_sender, catalogos) = channel::channel::<crate::apps::Catalogue>();
    state.catalogue_sender = Some(catalogue_sender.clone());
    state.apps_stamp = crate::apps::stamp();
    crate::apps::scan(state.texts.language(), catalogue_sender);
    alca.insert_source(catalogos, |event, (), state: &mut Footer| {
        if let channel::Event::Msg(c) = event {
            state.catalogued(c);
        }
    })
    .map_err(|e| anyhow::anyhow!("nao consegui pendurar o canal do catalogo: {e}"))?;

    // THE ICON PIXMAPS, at the second speed.
    //
    // The list draws with the glyphs and does NOT wait for this: each icon that
    // arrives swaps one line. A long-lived thread, because the expensive part is
    // the same for all of them — `look_up` sweeps a superpack of 335,031 files.
    let (envio_pintura, pinturas) = channel::channel::<crate::apps::Painting>();
    state.icon_queue = crate::apps::painter(envio_pintura);
    alca.insert_source(pinturas, |event, (), state: &mut Footer| {
        if let channel::Event::Msg(p) = event {
            state.painted(p);
        }
    })
    .map_err(|e| anyhow::anyhow!("nao consegui pendurar o canal de icones: {e}"))?;

    let (envio, recebimento) = channel::channel::<ronoctua_niri::Event>();
    crate::daemon::listen_to_niri(envio);
    alca.insert_source(recebimento, |event, (), state: &mut Footer| {
        if let channel::Event::Msg(e) = event {
            state.niri.apply_to(&e);
            // Redrawing on every event is cheap because the layout is integer
            // arithmetic (section 4.1) and the text only changes when focus changes.
            state.draw_and_paint();
        }
    })
    .map_err(|e| anyhow::anyhow!("nao consegui pendurar o canal do niri: {e}"))?;

    // Third event source: the control socket, where the leader arrives. See
    // `leader.rs` for why the leader comes from OUTSIDE.
    let (envio_ordem, ordens) = channel::channel::<Order>();
    crate::leader::listen(envio_ordem);
    alca.insert_source(ordens, |event, (), state: &mut Footer| {
        if let channel::Event::Msg(o) = event {
            match o {
                Order::Leader => state.ativar_leader(),
                // THE SAME `run_command` as the key. A command coming from outside and a
                // command coming from the keymap have to do the SAME thing — two
                // paths for the same id are what made the terminal's chord open
                // without a scope while the sequence opened with one.
                Order::Command(id, argument) => {
                    tracing::info!(command = %id, "comando pelo socket de controle");
                    state.run_with(&id, argument.as_deref());
                    state.draw_and_paint();
                }
                // ANOTHER FOOTER TOOK OVER THIS DISPLAY. Leaving is the only right
                // answer: staying up would leave two surfaces anchored at the
                // bottom and — if this one had the leader open — this one holding
                // the keyboard the other needs. See `leader::yield_the_place`.
                Order::Quit => {
                    tracing::info!("outro rodape assumiu este display; saindo");
                    state.quit = true;
                }
            }
        }
    })
    .map_err(|e| anyhow::anyhow!("nao consegui pendurar o socket de controle: {e}"))?;

    // FIFTH SOURCE: what was copied. The reading thread sends it through here;
    // see `crate::clipboard::watch`.
    alca.insert_source(copias, |event, (), state: &mut Footer| {
        if let channel::Event::Msg(e) = event {
            let bytes = e.content.bytes().len();
            if state.clipboard.register(e) {
                tracing::debug!(bytes, "entrada nova no historico de clipboard");
                // It only redraws if the view is OPEN. Copying with the clipboard
                // closed must not cost a frame.
                if state.clipboard_open {
                    state.apply_geometry(state.interactivity());
                    state.draw_and_paint();
                }
            }
        }
    })
    .map_err(|e| anyhow::anyhow!("nao consegui pendurar o canal do clipboard: {e}"))?;

    // Fourth source: the session bus. The notification daemon runs in a thread
    // and sends through the same kind of channel as niri and the control socket.
    let (envio_notif, notifications) = channel::channel::<ronoctua_notify::Event>();
    let (bruto_tx, bruto_rx) = std::sync::mpsc::channel::<ronoctua_notify::Event>();
    // FIFTH SOURCE, THE SAME CHANNEL: what `systemd-oomd` closed to keep the
    // machine alive. It is the SYSTEM bus and not the session one, and it shares
    // the bridge below on purpose — one destination, one path to keep in step.
    //
    // It is started BEFORE the session daemon so that a boot in which the footer
    // comes up first still registers the match: the rule is filed with the bus,
    // and the signal arrives whenever `systemd-oomd` takes the name.
    ronoctua_notify::oom::listen(bruto_tx.clone());
    if ronoctua_notify::dbus::listen(bruto_tx) {
        // A bridge between zbus's `mpsc` and calloop's channel. They are two kinds
        // of channel because zbus does not know calloop and vice versa; the bridge
        // is a thread that only forwards.
        std::thread::Builder::new()
            .name("ponte-notif".into())
            .spawn(move || {
                while let Ok(e) = bruto_rx.recv() {
                    if envio_notif.send(e).is_err() {
                        return;
                    }
                }
            })
            .expect("a ponte de notificacoes nao pode falhar em nascer");
    }
    alca.insert_source(notifications, |event, (), state: &mut Footer| {
        if let channel::Event::Msg(e) = event {
            state.notification_of(e);
        }
    })
    .map_err(|e| anyhow::anyhow!("nao consegui pendurar as notificacoes: {e}"))?;

    // A timer to expire the notification on screen.
    //
    // One second is coarse on purpose: the resolution that matters is "it went
    // away after a few seconds", and waking the loop 60 times a second for that
    // would spend battery (section 2 aims at laptops).
    alca.insert_source(
        calloop::timer::Timer::from_duration(std::time::Duration::from_secs(1)),
        |_, (), state: &mut Footer| {
            let now = std::time::Instant::now();
            // Both on the SAME tick: a second clock just for the echo area would
            // wake the loop twice a second for nothing.
            let venceu = state.expire_echo(now);
            if state.notifications.tick(now) || venceu {
                state.draw_and_paint();
            }
            calloop::timer::TimeoutAction::ToDuration(std::time::Duration::from_secs(1))
        },
    )
    .map_err(|e| anyhow::anyhow!("nao consegui pendurar o timer: {e}"))?;

    WaylandSource::new(conn, fila)
        .insert(alca)
        .map_err(|e| anyhow::anyhow!("nao consegui pendurar o Wayland no calloop: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |state| {
            if state.quit {
                signal.stop();
            }
        })
        .context("loop de eventos")?;
    Ok(())
}

impl Footer {
    /// The modeline's segments with the texts of the buffer focused RIGHT NOW.
    ///
    /// It rebuilds every frame instead of storing. It is cheap — five short
    /// `String`s — and it avoids the class of bug where the modeline shows the
    /// previous buffer because someone forgot to invalidate a cache.
    fn segments_now(&self) -> Vec<ronoctua_render::layout::Segment> {
        // THE PENDING SEQUENCE DOES NOT COME IN HERE.
        //
        // It showed up in the mode indicator AND in the minibuffer's line — the
        // same thing twice on screen, one of them in a segment that elides.
        // Its place is the minibuffer, which is where Doom shows it and where
        // one looks while typing a chord.

        // The branch comes from the `cwd` of the focused window's process. With
        // no pid — niri may not have one — there is no branch, and the segment
        // disappears instead of showing the footer's, which would be the wrong repo.
        let branch = self
            .niri
            .focused()
            .and_then(|j| j.pid)
            .and_then(crate::vcs::branch_of_pid);
        crate::segments(
            &crate::daemon::buffer_name(&self.niri, &self.texts),
            &crate::daemon::buffer_mode(&self.niri, &self.texts),
            branch.as_deref(),
            self.notifications.pending(),
        )
    }

    /// Recomputes and logs the layout. The actual drawing comes in when the wgpu
    /// path is wired to this surface.
    fn draw(&mut self) {
        // The reference width is that of the FIRST configured screen. The log
        // exists for diagnostics; the actual drawing is per screen, in
        // `paint`, each one with its own width.
        let Some(t) = self.screens.iter().find(|t| t.configured && t.width > 0) else {
            return;
        };
        let (width, height) = (t.width, t.height);
        let columns = tokens::metrics::columns(width as f32, tokens::font::SIZE_BASE);
        let placed = layout::resolve(columns, &self.segments_now());

        tracing::info!(
            width,
            height,
            screens = self.screens.len(),
            columns,
            segments = placed.len(),
            // THE OFFSET COMES IN HERE because it is the number the scrollbar and
            // the page counter draw. Without it in the log, "the wheel did not
            // scroll" does not tell "the event did not arrive" from "it arrived
            // and changed nothing".
            offset = self.current_offset(),
            "layout do rodape resolvido"
        );
        for c in &placed {
            tracing::debug!(id = c.id, column = c.column, text = %c.text, "segmento");
        }
    }
}

impl LayerShellHandler for Footer {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        tracing::info!("o compositor fechou a superficie do rodape");
        self.quit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        cfg: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        // WHICH screen: with one surface per output, the `configure` arrives once
        // per monitor and each one has its own width. Matching by the surface is
        // the only way not to apply one's size to the other.
        let Some(i) = self
            .screens
            .iter()
            .position(|t| t.layer.wl_surface() == layer.wl_surface())
        else {
            return;
        };

        // Width 0 in the configure means "you choose" — but we asked for full
        // width via the anchor, so the compositor sends the actual one.
        self.screens[i].width = cfg.new_size.0;
        if cfg.new_size.1 != 0 {
            self.screens[i].height = cfg.new_size.1;
        }
        self.screens[i].configured = true;

        self.ensure_gpu(i);
        let (l, a) = (self.screens[i].width, self.screens[i].height);
        if let Some(g) = self.screens[i].gpu.as_mut() {
            g.resize(l, a);
        }

        self.draw();
        self.paint();
        self.screens[i].layer.commit();
    }
}

impl CompositorHandler for Footer {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _fator: i32,
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

    fn frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _tempo: u32,
    ) {
    }

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

impl OutputHandler for Footer {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }

    // ONE SURFACE PER OUTPUT (section 9.2), created here and not at startup:
    // at startup there may be no output at all — see the comment in `run`.
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.ensure_screens(qh);
    }
    // Here too, and not only in `new_output`: SCTK announces `new_output` as
    // soon as the global appears, but only fills in the output's information
    // after the `wl_output` events. If `outputs()` is still empty on the first
    // call, this is the second chance — and there is no polling.
    fn update_output(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.ensure_screens(qh);
        // IT IS THE MONITOR'S HEIGHT, which decides the expansion's ceiling. It
        // arrives AFTER the `new_output` — changing the video mode without
        // updating it would leave the ceiling computed over the old resolution.
        let height = self
            .outputs
            .info(&output)
            .and_then(|i| i.logical_size)
            .map_or(0, |(_, h)| u32::try_from(h).unwrap_or(0));
        if let Some(t) = self.screens.iter_mut().find(|t| t.output == output) {
            t.output_height = height;
        }
    }

    // HOTPLUG: the monitor is gone, its surface goes with it.
    //
    // The minibuffer's state does NOT live here (see `Screen`), so removing the
    // screen does not lose what was typed — it stays in the daemon and shows up
    // on the next screen that gains focus. Keeping the state in the surface
    // would make the prompt die with the monitor's cable.
    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let before = self.screens.len();
        self.screens.retain(|t| t.output != output);
        if self.screens.len() != before {
            tracing::info!(
                screens = self.screens.len(),
                "output removido; superficie fechada"
            );
        }
    }
}

impl SeatHandler for Footer {
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
        // The keyboard is created as soon as the seat announces it, EVEN at rest.
        // Having the object captures nothing — what decides that is the layer
        // surface's `keyboard-interactivity`. Creating it only on the first
        // leader would lose the first event.
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            match self.seats.get_keyboard(qh, &seat, None) {
                Ok(k) => {
                    tracing::debug!("teclado do seat pronto");
                    self.keyboard = Some(k);
                }
                Err(e) => tracing::warn!(error = %e, "sem teclado; a leader nao vai funcionar"),
            }
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => self.pointer = Some(p),
                // With no pointer the footer works whole: what is lost is the
                // scrollbar, which is a visual hint, not a path.
                Err(e) => {
                    tracing::warn!(error = %e, "sem ponteiro; a barra de rolagem nao aparece")
                }
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
            self.under_pointer = false;
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for Footer {
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
        tracing::debug!("o rodape ganhou o foco de teclado");
        // IT ZEROES THE INHERITANCE. Whatever is held now arrives on the next
        // modifier event — the protocol requires the compositor to send one right
        // after the `enter` —, and it is THAT one that becomes this keyboard
        // session's inheritance. See `Inheritance`.
        self.inheritance.on_focus();
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        // Losing focus happens: another surface asked for exclusivity, or the
        // compositor switched output. Staying "capturing" without receiving a key
        // would leave the footer expanded forever — and now there are THREE owners
        // of the keyboard, not just the leader: the `M-x` and the manual would hang
        // the same way, and by a path nobody notices.
        //
        // The `CtrlRight` falls in here too: if the key release does not arrive
        // (it was released with the focus somewhere else), the footer would keep
        // routing every key to the navigation.
        self.right_ctrl = false;
        // AND THE MODIFIERS WHOLE, for the same reason and with more cause: a
        // state that only updates with focus cannot keep holding without focus.
        // Keeping `alt: true` here makes EVERY key of the next sequence come out
        // as `M-…` and match nothing.
        self.mods = Modifiers::default();
        self.inheritance.on_focus();
        if self.grabbing() {
            tracing::debug!("foco de teclado perdido; fechando o que estava aberto");
            self.mx.close_it();
            self.manual = false;
            self.apps = None;
            self.first_page();
            self.close_leader();
            self.draw_and_paint();
        }
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        // The RIGHT `Ctrl`, tracked by its keysym.
        //
        // It does not come in the `Modifiers`: xkb merges both Ctrls into a single
        // bit, and the modifier event does not say which physical key went down. It
        // is read here, BEFORE the leader's guard, otherwise holding it with the
        // leader not running would leave the state stale when it opened.
        if event.keysym == Keysym::Control_R {
            self.right_ctrl = true;
            return;
        }

        // WHO HOLDS THE KEYBOARD IS NOT ONLY THE LEADER.
        //
        // It used to be `self.leader.active_one()`, and that broke both modals: the
        // `run_command` opened the `M-x`, and the `close_leader` right after
        // gave the keyboard back — the prompt showed up on screen and did NOT
        // receive a key. Nothing on screen gave it away; it looked as if the
        // keyboard had died.
        if !self.grabbing() {
            return;
        }
        let Some(key) = self.translate(&event) else {
            return;
        };

        // WITH THE CtrlRight HELD, THE KEY DOES NOT ENTER THE SEQUENCE.
        //
        // That is the request: a combination key that does NOT add itself to the
        // leader, so that there is a way left to navigate what is on screen without
        // spending a letter of the keymap. Paging with `←`/`→` would cost two
        // letters that could never be a command again.
        if self.right_ctrl {
            if self.navigate(key.sym) {
                self.apply_geometry(KeyboardInteractivity::Exclusive);
                self.draw_and_paint();
            }
            return;
        }

        // THE CLIPBOARD intercepts before the manual: with it open, the whole
        // keyboard is its — `d` removes, and does not fire `LEADER d`.
        if self.clipboard_open {
            self.clipboard_key(key.sym);
            return;
        }

        // THE APPLICATIONS intercept for the same reason as the other modals:
        // with the list open the whole keyboard is its — typing `f` filters, and
        // does not fire `SPC f`.
        if self.apps.is_some() {
            self.apps_key(key);
            return;
        }

        // THE ONE-KEY QUESTION intercepts before the rest of the view: with it
        // on screen, `s` and `n` answer and do not fire the list's search.
        if self.network_remember.is_some() {
            use ronoctua_keymap::Sym;
            match key.sym {
                Sym::Char('s') | Sym::Char('S') => self.answer_remember(true),
                // Any other key is "no". A question that only leaves with `s` or
                // `n` would trap the keyboard of whoever pressed Esc — and the
                // safe answer is the one that stores nothing.
                _ => self.answer_remember(false),
            }
            return;
        }

        // THE NETWORK intercepts for the same reason as the other modals, and
        // the order matters: it comes AFTER the `M-x` in the precedence of
        // `view()`, but here it comes before the test of `mx.active()` — and
        // there is no conflict, because with the list open and the password
        // prompt asking, whoever answers is the `M-x`. That is why the test of
        // the prompt comes first.
        if self.network.is_some() && self.mx.asking_for().is_none() {
            use ronoctua_keymap::Sym;
            match key.sym {
                Sym::Esc => {
                    self.network = None;
                    self.first_page();
                    self.apply_geometry(self.interactivity());
                    self.draw_and_paint();
                }
                Sym::Down | Sym::Up => {
                    let step = if key.sym == Sym::Down { 1 } else { -1 };
                    if let Some(v) = self.network.as_mut() {
                        v.move_by(step);
                    }
                    self.draw_and_paint();
                }
                // THE VIEW'S KEYS come out of the SAME table the manual reads and
                // that the orphan check consults — `leader::OF_NETWORK_VIEW`.
                // A `match` written by hand here would be the second truth this
                // project has already paid not to have.
                otherwise => {
                    // THE SAME NOTATION the table uses, by the same path as the
                    // clipboard. A key outside the table does nothing and does NOT
                    // close the view: closing on a wrong key would lose the place
                    // in the list.
                    let notation = ronoctua_keymap::Key::simple(otherwise).notation();
                    if let Some((_, id)) = crate::leader::OF_NETWORK_VIEW
                        .iter()
                        .find(|(t, _)| *t == notation)
                    {
                        self.run_command(id);
                    }
                    self.apply_geometry(self.interactivity());
                    self.draw_and_paint();
                }
            }
            return;
        }

        // THE MANUAL intercepts before everything: with it open the arrows
        // SCROLL, and an arrow that fell into the keymap would end the leader
        // by unknown sequence — the manual would close by itself on the first
        // gesture of reading.
        if self.manual {
            use ronoctua_keymap::Sym;
            match key.sym {
                Sym::Esc => {
                    self.manual = false;
                    self.first_page();
                    self.apply_geometry(self.interactivity());
                    self.draw_and_paint();
                }
                // One LINE at a time, and not a page: the manual is read
                // straight through, and skipping a screen per arrow loses the
                // context between what has just been read and what comes. Paging
                // has its own key (`CtrlRight`+arrows).
                Sym::Down | Sym::Up => {
                    let step = if key.sym == Sym::Down { 1 } else { -1 };
                    if self.scroll_by(step) {
                        self.draw_and_paint();
                    }
                }
                // Nothing else touches it. It is read-only, and a key that
                // "almost" works teaches that there is something to type there.
                _ => {}
            }
            return;
        }

        // The `M-x` intercepts BEFORE the leader: with it open, the whole
        // keyboard is its — typing `f` filters, and does not fire `SPC f`.
        if self.mx.active() {
            use crate::minibuffer::Action;
            match self.mx.key(key, &self.command_registry) {
                Action::Nothing => {}
                Action::Changed => {
                    // REDRAW HERE, AND NOT ONLY RE-COMMIT THE SIZE.
                    //
                    // THE DEFECT THIS WAS: `apply_geometry` only asks for the
                    // size; whoever paints is the `configure` that comes back — and
                    // the compositor only sends it when the height CHANGES. With the
                    // list at the ceiling, filtering from 172 down to 40 candidates
                    // changes no height at all, and moving the cursor does not
                    // either. Result: typing did not filter on screen and the arrow
                    // did not go down, and everything showed up at once when the
                    // mouse moved — which is the only other thing that painted.
                    self.follow_cursor();
                    self.apply_geometry(KeyboardInteractivity::Exclusive);
                    self.draw_and_paint();
                }
                Action::Closed => {
                    self.apply_geometry(self.interactivity());
                    self.draw_and_paint();
                }
                Action::Run(id) => {
                    tracing::info!(command = id, "comando executado pelo localizador");
                    self.run_command(&id);
                    self.apply_geometry(self.interactivity());
                    self.draw_and_paint();
                }
                Action::Selected(id) => {
                    self.select_item(&id);
                    self.apply_geometry(self.interactivity());
                    self.draw_and_paint();
                }
                Action::RunWith(id, arg) => {
                    tracing::info!(command = id, argument = %arg, "comando com argumento");
                    self.run_with(&id, Some(&arg));
                    self.apply_geometry(self.interactivity());
                    self.draw_and_paint();
                }
            }
            return;
        }

        // The sequence BEFORE applying the key: it is the one the message cites
        // when the key leads nowhere. After ending, the leader has already
        // forgotten.
        let attempt = format!(
            "{} {}",
            self.leader.raw_prefix(&self.leader_label),
            ronoctua_keymap::notation(&[key])
        );
        // THE FOCUSED BUFFER'S OWN KEYMAP, before the tree gets the key.
        //
        // It cannot go through `ronoctua_keymap::Tree`: that crate's bindings
        // are `&'static str` and these arrive over a socket at runtime. What it
        // would be as a tree is a filter over a list, because the sequences are
        // two or three keys deep.
        if self.buffer_key(key) {
            self.draw_and_paint();
            return;
        }

        let effect = self.leader.key(&self.tree, key);
        self.apply_leader_effect_with(effect, Some(&attempt), key.sym);
        self.draw_and_paint();
    }

    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        // A repeat inside a leader sequence would be typing the same key twice by
        // accident. But `SPC b b b` does not exist, so a repeat is always an error
        // here — we let it through to the same path, which ends by unknown
        // sequence instead of piling up state.
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
        if event.keysym == Keysym::Control_R {
            self.right_ctrl = false;
        }
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
        // IT RESYNCS THE CtrlRight. If no Ctrl is held, the right one is not
        // held either — and this is the net that catches the case where the key
        // release did not arrive (focus lost with it pressed). Without this, the
        // footer would keep routing every key to the navigation, and the leader
        // would look as if it had stopped working.
        if !mods.ctrl {
            self.right_ctrl = false;
        }
        self.mods = mods;
        tracing::debug!(
            ctrl = mods.ctrl,
            alt = mods.alt,
            logo = mods.logo,
            caps = mods.caps_lock,
            "modificadores anunciados pelo compositor"
        );
        self.inheritance.on_announcement(mods);
    }
}

impl Footer {
    /// The same thing a leader key does, without the key.
    ///
    /// It exists because the CLICK on a line of the list is, by definition,
    /// pressing its key: without sharing this, the mouse would have a path of
    /// its own to the commands — and two paths to the same thing is what this
    /// whole work has been undoing.
    fn apply_leader_effect(&mut self, effect: Effect) {
        self.apply_leader_effect_with(effect, None, ronoctua_keymap::Sym::Esc);
        self.draw_and_paint();
    }

    /// `attempt` is the typed sequence, for the "not bound to anything"
    /// message — the click does not have one, because it does not miss a key.
    fn apply_leader_effect_with(
        &mut self,
        effect: Effect,
        attempt: Option<&str>,
        sym: ronoctua_keymap::Sym,
    ) {
        match effect {
            Effect::Continue { inputs } => {
                self.inputs = inputs;
                self.first_page();
                tracing::info!(
                    inputs = self.inputs.len(),
                    prefix = %self.leader.prefix(&self.leader_label),
                    "which-key estreitou"
                );
                self.echo = None;
                self.apply_geometry(KeyboardInteractivity::Exclusive);
            }
            Effect::Close { other_case } => {
                // `Esc` is explicit cancellation; any other key that ends it is a
                // sequence that does not exist. They are different messages
                // because they are different things: one the user asked for, the
                // other they did not expect.
                let msg = match attempt {
                    // THE CASE IS THE CAUSE, and it goes to the screen. "not bound"
                    // on its own sends the person looking for a shortcut they have
                    // just pressed; saying that the OTHER case exists points at the
                    // Shift or the Caps Lock, which is what they need to do
                    // something about.
                    Some(seq) if sym != ronoctua_keymap::Sym::Esc && other_case.is_some() => {
                        let otherwise_one = other_case.map(|t| t.notation()).unwrap_or_default();
                        self.texts.t_with(
                            ronoctua_i18n::key::ECHO_NO_ON_BOX,
                            &[("sequencia", seq), ("outra", &otherwise_one)],
                        )
                    }
                    Some(seq) if sym != ronoctua_keymap::Sym::Esc => self
                        .texts
                        .t_with(ronoctua_i18n::key::ECHO_NO_ON, &[("sequencia", seq)]),
                    _ => self.texts.t(ronoctua_i18n::key::ECHO_CANCELLED),
                };
                self.close_leader();
                self.echo_message(msg);
            }
            Effect::Run(id, argument) => {
                tracing::info!(command = id, argument, "comando disparado pela leader");
                self.run_with(id, argument);
                self.close_leader();
            }
        }
    }
}

impl PointerHandler for Footer {
    /// The pointer serves ONE thing: knowing whether it is over the footer.
    ///
    /// There is no click, no drag and no target — the footer has nothing
    /// clickable (invariant 14). The manual's scrollbar shows up while the mouse
    /// goes over it and disappears when it leaves, which is the behavior one
    /// expects from a bar that stays hidden.
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let before = (self.under_pointer, self.row_under_pointer, self.offset);
        let mut clicou = None;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.under_pointer = true;
                    self.row_under_pointer = self.row_at(e.position.0, e.position.1);
                    // DRAGGING: the pointer's position BECOMES the offset.
                    if self.dragging {
                        self.drag_to(e.position.1);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.under_pointer = false;
                    self.row_under_pointer = None;
                    self.dragging = false;
                }
                // ONLY THE LEFT BUTTON. The others do nothing on purpose:
                // a context menu would be new chrome (invariant 14), and the
                // middle one pasting text into a list of commands would be a
                // command run by mistake.
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    self.row_under_pointer = self.row_at(e.position.0, e.position.1);
                    // ON THE BAR, IT IS NOT A CLICK ON AN ITEM.
                    //
                    // It used to be: pressing over the bar scrolled nothing and
                    // RAN the command of the line behind it. Whoever wants to
                    // drag does not want to run anything, and the bar sits
                    // precisely over the right end of the items.
                    if self.on_thumb(e.position.0) {
                        self.dragging = true;
                        self.drag_to(e.position.1);
                    } else {
                        clicou = self.row_under_pointer;
                    }
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    self.dragging = false;
                }
                // THE WHEEL AND THE TOUCHPAD GESTURE.
                //
                // A single handling for the region's two modes: where there is
                // a scrollbar, the detent steps LINES; where there is a page
                // counter, it TURNS THE PAGE. Whoever knows which case it is is
                // `navigation()`, and not the wheel — the wheel only says how
                // much and which way.
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    // The `stop` of a continuous gesture zeroes what was left:
                    // without that, half a detent left over from the previous
                    // gesture would add to the next one and the first scroll
                    // would come out bigger than the finger asked for.
                    if vertical.stop || horizontal.stop {
                        self.detents = 0.0;
                    }
                    let linha_px = f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE));
                    // THE HORIZONTAL AXIS COUNTS TOO. On a touchpad, turning
                    // a page sideways is the natural gesture; and the region
                    // has nowhere to scroll horizontally, so it competes with
                    // nothing.
                    self.detents +=
                        axis_detents(&vertical, linha_px) + axis_detents(&horizontal, linha_px);
                }
                _ => {}
            }
        }

        if let Some(i) = clicou {
            self.click_at(i);
            return;
        }

        // WHAT IS LEFT OF THE ACCUMULATOR STAYS. See the `detents` field: the
        // touchpad sends fractions, and spending only the whole part is what lets
        // the continuous gesture add up until it becomes a line.
        #[allow(clippy::cast_possible_truncation)]
        let inteiros = self.detents.trunc() as i32;
        if inteiros != 0 {
            self.detents -= f64::from(inteiros);
            // ONE detent = one page where it paginates, three lines where it
            // scrolls. The `page_by` already multiplies by the mode's step (which
            // is 1 on scrolling), so the difference fits in the factor.
            #[allow(clippy::cast_possible_truncation)]
            let step = match self.navigation() {
                Navigation::Page => 1,
                Navigation::Scroll => LINES_PER_DETENT as i32,
            };
            if self.page_by(inteiros.saturating_mul(step)) {
                self.apply_geometry(self.interactivity());
                self.draw_and_paint();
                return;
            }
        }
        // It only redraws if something CHANGED: a `Motion` arrives on every pixel
        // of movement, and painting a frame per pixel would spend the GPU drawing
        // exactly the same thing.
        if before != (self.under_pointer, self.row_under_pointer, self.offset) {
            self.draw_and_paint();
        }
    }
}

impl ProvidesRegistryState for Footer {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

// SCTK 0.21 consolidated `delegate_compositor!`, `delegate_output!` and
// `delegate_layer!` into a single `delegate_dispatch2!`. The old names no
// longer exist — and the error they give ("cannot find delegate_output") does
// not suggest the replacement.
smithay_client_toolkit::delegate_registry!(Footer);
smithay_client_toolkit::delegate_dispatch2!(Footer);

// ---------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------

impl Footer {
    /// Creates the footer's surface, if it does not exist yet and there is
    /// already an output.
    ///
    /// Idempotent: it leaves right away if the surface already exists. Called at
    /// startup and on every turn of the loop, to catch hotplug.
    fn ensure_screens(&mut self, qh: &QueueHandle<Self>) {
        // ONE per output, and not a single one on the first. Each output gets the
        // two lines; what changes between them is only the minibuffer being active.
        let saidas: Vec<_> = self.outputs.outputs().collect();
        if saidas.is_empty() {
            tracing::debug!("ainda sem output; as superficies esperam");
            return;
        }
        let atuais: Vec<_> = self.screens.iter().map(|t| t.output.clone()).collect();
        let (criar, _) = screen_difference(&atuais, &saidas);
        for i in criar {
            self.create_screen(qh, saidas[i].clone());
        }
    }

    /// Creates one output's surface.
    fn create_screen(&mut self, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        let surface_of = self._compositor.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface_of,
            // `Top`, not `Overlay`: the footer stays above the windows but
            // BELOW screen-lock dialogs. Overlay would put it on top of the
            // lock.
            Layer::Top,
            Some(crate::NAMESPACE),
            // An EXPLICIT output: it is what makes "one surface per output" be
            // true. Without it the compositor chooses, and two surfaces could
            // land on the same monitor.
            Some(&output),
        );

        // Anchored at the bottom and on both sides: the whole width (section 4.1).
        layer.set_anchor(Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, self.requested_height);

        // Exclusive zone = the two lines. The buffers' content stops here.
        layer.set_exclusive_zone(self.requested_height as i32);

        // At rest the footer does NOT capture the keyboard. It starts capturing
        // only while the leader is active (section 7.4), and goes back to `None`
        // afterwards — otherwise no buffer would receive a key.
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);

        layer.commit();
        // THE MONITOR'S HEIGHT comes from the output, and not from the surface:
        // it decides the expansion's ceiling. `logical_size` may be missing on an
        // output that has not announced a mode yet; then the ceiling falls back to
        // the absolute one in lines, which is the more conservative of the two.
        let output_height = self
            .outputs
            .info(&output)
            .and_then(|i| i.logical_size)
            .map_or(0, |(_, h)| u32::try_from(h).unwrap_or(0));
        self.screens.push(Screen {
            output,
            layer,
            width: 0,
            height: self.requested_height,
            output_height,
            configured: false,
            gpu: None,
            text: None,
            quads: None,
            images: None,
        });

        tracing::info!(
            height = self.requested_height,
            screens = self.screens.len(),
            namespace = crate::NAMESPACE,
            "superficie layer-shell criada, aguardando configure"
        );
    }

    /// Runs a command by its id.
    ///
    /// By ID, and never by closure: section 13 requires every feature to be
    /// addressable by name, and not only by key. It is what keeps the door open
    /// for the IPC surface without redesigning anything later.
    fn run_command(&mut self, id: &str) {
        self.run_with(id, None);
    }

    /// An item chosen from a list the shell itself built.
    ///
    /// Today the only such list is the buffers, and the id is the compositor's
    /// window id. It is NOT a command: the registry knows nothing about it, and
    /// running it as one would silently do nothing.
    fn select_item(&mut self, id: &str) {
        if std::mem::take(&mut self.picking_file) {
            self.open_editor(std::path::Path::new(id));
            return;
        }
        let Ok(window) = id.parse::<u64>() else {
            tracing::warn!(item = id, "item do localizador nao e id de janela");
            return;
        };
        tracing::info!(window, "buffer escolhido pelo localizador");
        // The SAME action the chord uses. Focus belongs to the compositor
        // (invariant 4), so the shell asks and does not move anything itself.
        self.niri_action(|n| n.action(serde_json::json!({ "FocusWindow": { "id": window } })));
    }

    /// Opens the editor buffer on a file.
    ///
    /// The path goes as an ARGUMENT to the host of the buffer, not into the
    /// component: the buffer receives the content through `Input::Paste` and
    /// asks to write with `Effect::Save`, and never learns a path
    /// (invariant 11).
    fn open_editor(&mut self, file: &std::path::Path) {
        let (Ok(me), Some(wasm)) = (
            std::env::current_exe(),
            buffer_component("RONOCTUA_BUFFER_EDITOR"),
        ) else {
            tracing::error!("RONOCTUA_BUFFER_EDITOR nao esta no ambiente");
            let m = self.texts.t(ronoctua_i18n::key::ECHO_NO_BUFFER);
            self.echo_message(m);
            return;
        };
        tracing::info!(file = %file.display(), "abrindo o editor");
        self.open_foreign(
            &me.to_string_lossy(),
            &[
                "buffer".to_owned(),
                wasm,
                file.to_string_lossy().into_owned(),
            ],
            None,
        );
    }

    /// The files of the current directory, as the locator's items.
    ///
    /// The CURRENT directory and not a recursive walk: a locator that lists a
    /// whole tree takes seconds to open on a repository, and the one thing it
    /// must be is instant. Directories come first, with a trailing slash, which
    /// is how one sees where a step leads before taking it.
    fn file_items(&self) -> Vec<crate::minibuffer::Item> {
        let base = std::env::var_os("XDG_PROJECTS_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from))
            .unwrap_or_else(|| std::path::PathBuf::from("/"));
        let Ok(entries) = std::fs::read_dir(&base) else {
            tracing::warn!(dir = %base.display(), "nao consegui listar o diretorio");
            return Vec::new();
        };
        let mut out: Vec<_> = entries
            .flatten()
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .map(|e| {
                let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
                let name = e.file_name().to_string_lossy().into_owned();
                crate::minibuffer::Item {
                    id: e.path().to_string_lossy().into_owned(),
                    label: if is_dir { format!("{name}/") } else { name },
                }
            })
            .collect();
        out.sort_by(|a, b| a.label.cmp(&b.label));
        out
    }

    /// The buffers, as the locator's items.
    ///
    /// The label is what one types: the title when there is one, the `app_id`
    /// otherwise. A window with neither still appears — vanishing from the list
    /// would be worse than an ugly line.
    fn buffer_items(&self) -> Vec<crate::minibuffer::Item> {
        self.niri
            .all_windows()
            .iter()
            .map(|w| crate::minibuffer::Item {
                id: w.id.to_string(),
                label: w
                    .title
                    .clone()
                    .or_else(|| w.app_id.clone())
                    .unwrap_or_else(|| format!("janela {}", w.id)),
            })
            .collect()
    }

    /// The same, with the argument some compositor actions require.
    ///
    /// It arrives from outside — `ronoctua-shell command espaco-ir 3` — because
    /// the keymap has no way of carrying a number: a sequence is a path to an
    /// id, not a call with a parameter.
    fn run_with(&mut self, id: &str, argument: Option<&str>) {
        // OUR OWN COMMANDS WITH AN ARGUMENT, by the same client and for the same
        // reason: the shape of the argument in the JSON is niri's, and assembling
        // it by hand would bring back the error the `Screenshot` cost.
        if let (Some(name), Some(arg)) = (crate::actions::with_argument(id), argument) {
            match ronoctua_niri::run_through_client(name, Some(arg)) {
                Ok(()) => tracing::info!(command = id, argument = arg, "acao com argumento"),
                Err(e) => {
                    let m = e.to_string();
                    tracing::warn!(command = id, error = %m, "o compositor recusou");
                    self.echo_message(m);
                }
            }
            return;
        }
        // THE MIRRORED ACTIONS of the compositor, through its own client.
        //
        // They do not go through our JSON: the arguments with defaults live in
        // niri's clap, and `{}` gives the SAME error as a name that does not
        // exist — see `ronoctua_niri::run_through_client`.
        if let Some(name) = id.strip_prefix("niri-") {
            match ronoctua_niri::run_through_client(name, argument) {
                Ok(()) => tracing::info!(action = name, "acao do compositor"),
                Err(e) => {
                    // NIRI'S TEXT, and not one of ours: it is the only clue to
                    // why the action was refused, and swapping it for "it failed"
                    // would wipe the information on the way.
                    let m = e.to_string();
                    tracing::warn!(action = name, error = %m, "o compositor recusou");
                    self.echo_message(m);
                }
            }
            return;
        }
        match id {
            // THE SAFETY NET. It always reports — see `keyboard.rs`.
            "keyboard-restore" => {
                let m = match crate::keyboard::restore_it() {
                    Ok(()) => self.texts.t(ronoctua_i18n::key::ECHO_KEYBOARD_RESTORED),
                    Err(reason) => {
                        tracing::error!(error = %reason, "restaurar o teclado falhou");
                        self.texts.t_with(
                            ronoctua_i18n::key::ECHO_KEYBOARD_FAILED,
                            &[("motivo", &reason)],
                        )
                    }
                };
                self.echo_message(m);
            }
            "notif-dismiss" => {
                self.notifications
                    .dismiss_current(std::time::Instant::now());
                self.apply_geometry(self.interactivity());
            }
            // QUITTING IS THE COMPOSITOR'S, not just the footer's. Ending the
            // footer would leave the session standing with no modeline and no
            // leader — a system with no chrome at all and no way out of it.
            "session-quit" => {
                self.niri_action(|n| {
                    n.action(serde_json::json!({
                        "Quit": { "skip_confirmation": true }
                    }))
                });
                self.quit = true;
            }
            "notif-history" => self.alternar_historico(),
            // THE CLIPBOARD. Like the manual, it keeps the keyboard while it is
            // open: without that the arrows would go to the buffers.
            "clipboard-open" => {
                tracing::info!(inputs = self.clipboard.how_many(), "clipboard aberto");
                self.clipboard_open = true;
                self.clip_cursor = 0;
                self.clip_qr = None;
                self.clip_image = None;
                self.decode_preview();
                self.first_page();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
                self.draw_and_paint();
            }
            "clipboard-copy-of-new" => {
                let Some(e) = self.clipboard.at(self.clip_cursor) else {
                    return;
                };
                // THE SAME path the buffer uses to copy. Two ways of putting
                // something on the clipboard would diverge the day one of them
                // changed.
                //
                // AN IMAGE IS NOT RE-COPIED YET: `wl-copy` takes text on
                // stdin, and doing it by bytes would require telling it the
                // type. It stays in docs/PENDING.md with the rest of the image.
                let Some(text) = e.text().map(ToOwned::to_owned) else {
                    self.echo_message(self.texts.t(ronoctua_i18n::key::ECHO_COPY_ONLY_TEXT));
                    return;
                };
                match crate::buffer::copy_to_clipboard(&text) {
                    Ok(()) => {
                        tracing::info!(bytes = text.len(), "recopiado do historico");
                        self.echo_message(self.texts.t(ronoctua_i18n::key::ECHO_COPIED));
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "copiar do historico FALHOU");
                        self.echo_message(self.texts.t(ronoctua_i18n::key::ECHO_COPY_FAILED));
                    }
                }
            }
            "clipboard-remove" => {
                if self.clipboard.remove(self.clip_cursor) {
                    // THE CURSOR MUST NOT STAY PAST THE END. Removing the last one
                    // would leave the list pointing at the void, and the right
                    // column would show nothing without saying why.
                    self.clip_cursor = self
                        .clip_cursor
                        .min(self.clipboard.how_many().saturating_sub(1));
                    self.clip_qr = None;
                }
            }
            "clipboard-pin" => {
                self.clipboard.toggle_pinned(self.clip_cursor);
            }
            "clipboard-clear" => {
                self.clipboard.clear();
                self.clip_cursor = 0;
                self.clip_qr = None;
            }
            "clipboard-qrcode" => {
                let Some(e) = self.clipboard.at(self.clip_cursor) else {
                    return;
                };
                // IT TOGGLES: pressing again takes the QR away. It takes the
                // whole right column, and with no way of closing it the content
                // would stay hidden until switching items.
                self.clip_qr = if self.clip_qr.is_some() {
                    None
                } else {
                    // A QR OF AN IMAGE DOES NOT EXIST: a QR carries text, and the
                    // bytes of a PNG neither fit nor would make sense read by a
                    // camera.
                    let m = e.text().and_then(ronoctua_clipboard::qr::generate);
                    if m.is_none() {
                        self.echo_message(self.texts.t(ronoctua_i18n::key::ECHO_QR_LARGE));
                    }
                    m
                };
            }
            "clipboard-clipcascade-configure" => {
                // IT DOES NOT EXIST YET, and the screen says so. A command that
                // "opens" a setting that does not exist would be the façade this
                // project spent months undoing — see docs/PENDING.md.
                self.echo_message(self.texts.t(ronoctua_i18n::key::ECHO_CLIPCASCADE_MISSING));
            }
            // THE MANUAL. It keeps the keyboard while open, like the `M-x`:
            // without that the arrows would go to the buffers and scroll nothing.
            "help-shortcuts" => {
                self.manual = true;
                self.first_page();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
            }
            "mx-open" => {
                self.mx.open(&self.command_registry);
                self.first_page();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
            }
            // THE APP FINDER. It opens with the catalogue ALREADY READY: the sweep
            // ran at startup, in a thread, and what happens here is a copy of an
            // `Arc` and a filter over memory.
            // `LEADER b b` — choose a buffer by name, with completion.
            //
            // It is the SAME locator as `M-x`: same filter, same keys, same
            // rendering. What changes is only where the list came from — see
            // `minibuffer::Item`, whose label is what made this possible.
            // `LEADER f f` — pick a file and open it in the editor.
            //
            // The locator does the picking: since `minibuffer::Item` carries its
            // own label, the SAME list that serves commands and buffers serves
            // paths. The host opens the buffer with the file as an argument, and
            // the buffer never learns where the text came from.
            "file-open" => {
                let items = self.file_items();
                if items.is_empty() {
                    let m = self.texts.t(ronoctua_i18n::key::ECHO_NO_WINDOW);
                    self.echo_message(m);
                    return;
                }
                tracing::info!(files = items.len(), "seletor de arquivo aberto");
                self.picking_file = true;
                self.mx.open_over(items);
                self.first_page();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
            }
            "buffer-switch" => {
                let items = self.buffer_items();
                if items.is_empty() {
                    // Nothing to choose between. Opening an empty locator would
                    // be a list that says nothing and still takes the keyboard.
                    let m = self.texts.t(ronoctua_i18n::key::ECHO_NO_WINDOW);
                    self.echo_message(m);
                    return;
                }
                tracing::info!(buffers = items.len(), "seletor de buffer aberto");
                self.mx.open_over(items);
                self.first_page();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
            }
            "apps-open" => {
                self.rescan_if_changed();
                tracing::info!(apps = self.catalogue.len(), "localizador aberto");
                self.apps = Some(crate::apps::Finder::fresh(self.catalogue.clone()));
                self.first_page();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
            }
            // ---------------------------------------------------------
            // NETWORK. The view opens with what NetworkManager ALREADY
            // KNOWS: the list of access points lives in the daemon's
            // memory, and giving it back is one trip to the bus. What
            // takes time is the radio's scan, and that one is NOT waited
            // for — see `network.rs`.
            // ---------------------------------------------------------
            "network-open" => {
                self.watch_scans();
                self.network = Some(crate::network::ViewState::fresh_one());
                self.first_page();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
            }
            "network-scan" => {
                self.watch_scans();
                match self.network.as_mut() {
                    Some(v) => {
                        v.scan();
                        self.first_page();
                    }
                    // Outside the view the key still holds: it asks for the scan and
                    // OPENS the list, which is where the result shows up. Asking
                    // without showing would be a key that does nothing visible.
                    None => {
                        let mut v = crate::network::ViewState::fresh_one();
                        v.scan();
                        self.network = Some(v);
                        self.first_page();
                        self.apply_geometry(KeyboardInteractivity::Exclusive);
                    }
                }
            }
            "network-wifi-toggle" => {
                let fresh = match self.network.as_mut() {
                    Some(v) => v.toggle_radio(),
                    None => crate::network::ViewState::fresh_one().toggle_radio(),
                };
                let key = match fresh {
                    Some(true) => ronoctua_i18n::key::ECHO_NETWORK_RADIO_ON,
                    Some(false) => ronoctua_i18n::key::ECHO_NETWORK_RADIO_OFF,
                    // polkit may refuse. Saying "on" when it did not turn on
                    // would be the screen lying about the hardware.
                    None => ronoctua_i18n::key::ECHO_NETWORK_UNAVAILABLE,
                };
                let m = self.texts.t(key);
                self.echo_message(m);
            }
            "network-disconnect" => {
                let mut v = match self.network.take() {
                    Some(v) => v,
                    None => crate::network::ViewState::fresh_one(),
                };
                // THE ACTIVE MEDIUM, and not "the Wi-Fi": with the cable
                // plugged in, disconnecting the radio would change nothing and
                // the key would look broken.
                let medium = v
                    .active()
                    .map_or(ronoctua_network::Medium::Radio, |e| e.medium);
                let key = if v.disconnect(medium).is_ok() {
                    ronoctua_i18n::key::ECHO_NETWORK_DISCONNECTED
                } else {
                    ronoctua_i18n::key::ECHO_NETWORK_UNAVAILABLE
                };
                let m = self.texts.t(key);
                self.echo_message(m);
            }
            // THE LINK, in the minibuffer: band, rate and signal. It is the answer
            // to "why is this slow?" without leaving the screen — and it was by
            // measuring exactly that that this project found a 16x difference
            // between two bands of the same router.
            "network-link" => {
                let v = crate::network::ViewState::fresh_one();
                let m = match v.active() {
                    Some(e) if e.medium == ronoctua_network::Medium::Wired => self.texts.t_with(
                        ronoctua_i18n::key::ECHO_NETWORK_LINK_WIRED,
                        &[("rede", &e.interface), ("taxa", &e.rate())],
                    ),
                    Some(e) => self.texts.t_with(
                        ronoctua_i18n::key::ECHO_NETWORK_LINK,
                        &[
                            ("rede", e.network.as_deref().unwrap_or(&e.interface)),
                            ("banda", e.band.label()),
                            ("taxa", &e.rate()),
                            ("sinal", &format!("{}%", e.signal)),
                        ],
                    ),
                    None => self.texts.t(ronoctua_i18n::key::ECHO_NETWORK_NO_LINK),
                };
                self.echo_message(m);
            }
            // CONNECT TO THE FOCUSED NETWORK.
            //
            // Three paths, and the difference between them is what the person does
            // NOT have to do: a saved network reuses the profile, an open network
            // has no password, and a secured network with no profile opens the
            // prompt. Treating the three alike would ask for a password from
            // someone who connected yesterday.
            "network-connect" => {
                let Some(target) = self
                    .network
                    .as_ref()
                    .and_then(crate::network::ViewState::focused_network)
                else {
                    return;
                };
                if target.secured && !target.saved && argument.is_none() {
                    // The password comes in through the minibuffer, and comes back
                    // here as an argument of this same command.
                    self.mx.request("network-connect", true);
                    self.apply_geometry(KeyboardInteractivity::Exclusive);
                    return;
                }
                // IT CONNECTS WITHOUT STORING, and asks afterwards.
                //
                // `remember = false` here means `psk-flags=2`: nothing is
                // persisted while the person has not said they want it. If they
                // do say so, the answer's branch reconnects with `psk-flags=1`
                // and seals. The opposite — storing and deleting if they refuse
                // — would leave the password on disk in the meantime.
                let result = self
                    .network
                    .as_mut()
                    .map(|v| v.connect(&target.ssid, argument, false));
                if let (Some(Ok(())), Some(s)) = (&result, argument)
                    && target.secured
                {
                    self.network_remember = Some((target.ssid.clone(), s.to_owned()));
                }
                let m = match result {
                    Some(Ok(())) => self.texts.t_with(
                        ronoctua_i18n::key::ECHO_NETWORK_CONNECTED,
                        &[("rede", &target.ssid)],
                    ),
                    Some(Err(e)) => self.texts.t_with(
                        ronoctua_i18n::key::ECHO_NETWORK_FAILED,
                        &[("rede", &target.ssid), ("motivo", &e.to_string())],
                    ),
                    None => self.texts.t(ronoctua_i18n::key::ECHO_NETWORK_UNAVAILABLE),
                };
                self.echo_message(m);
            }
            "network-forget" => {
                let Some(target) = self
                    .network
                    .as_ref()
                    .and_then(crate::network::ViewState::focused_network)
                else {
                    return;
                };
                let ok = self
                    .network
                    .as_mut()
                    .is_some_and(|v| v.forget(&target.ssid).is_ok());
                let m = if ok {
                    self.texts.t_with(
                        ronoctua_i18n::key::ECHO_NETWORK_FORGOTTEN,
                        &[("rede", &target.ssid)],
                    )
                } else {
                    self.texts.t(ronoctua_i18n::key::ECHO_NETWORK_UNAVAILABLE)
                };
                self.echo_message(m);
            }
            // ---------------------------------------------------------
            // SYNCTHING. It comes installed with the distro, and a service
            // the distro installs needs a path through the leader.
            // ---------------------------------------------------------
            "syncthing-state" => {
                let m = match crate::syncthing::state() {
                    Some(e) => self.texts.t_with(
                        ronoctua_i18n::key::ECHO_SYNCTHING_STATE,
                        &[
                            ("conectados", &e.connected.to_string()),
                            ("porcento", &e.percent.to_string()),
                            ("envio", &crate::network::readable_bytes(e.sent)),
                            ("recepcao", &crate::network::readable_bytes(e.received)),
                        ],
                    ),
                    None => self.texts.t(ronoctua_i18n::key::ECHO_SYNCTHING_STOPPED),
                };
                self.echo_message(m);
            }
            "syncthing-pause" => {
                let key = if crate::syncthing::pause() {
                    ronoctua_i18n::key::ECHO_SYNCTHING_PAUSED
                } else {
                    ronoctua_i18n::key::ECHO_SYNCTHING_STOPPED
                };
                let m = self.texts.t(key);
                self.echo_message(m);
            }
            "syncthing-resume" => {
                let key = if crate::syncthing::resume() {
                    ronoctua_i18n::key::ECHO_SYNCTHING_RESUMED
                } else {
                    ronoctua_i18n::key::ECHO_SYNCTHING_STOPPED
                };
                let m = self.texts.t(key);
                self.echo_message(m);
            }
            // The Syncthing interface is a page; opening it is the default
            // browser's job. `xdg-open` by its STORE PATH: in a systemd
            // unit the PATH is minimal, and the name would fail far from
            // the cause (CLAUDE.md).
            "syncthing-open" => match XDG_OPEN_PATH {
                Some(abridor) => self.open_foreign(
                    abridor,
                    &[format!("http://{}", crate::syncthing::address())],
                    None,
                ),
                None => {
                    tracing::error!("RONOCTUA_XDG_OPEN nao foi definida no build");
                    let m = self.texts.t(ronoctua_i18n::key::ECHO_SYNCTHING_STOPPED);
                    self.echo_message(m);
                }
            },
            // OUR terminal, and not Ghostty: section 7.10 wants the VT
            // engine inside the shell, with the grid crossing the protocol.
            // It comes up in its OWN systemd scope, like any buffer — that
            // is what makes the PID -> cgroup correlation work (section
            // 7.5). Without the scope, every window would land in the
            // shell's cgroup and the modeline could not tell them apart.
            //
            // `current_exe` and not the name on the PATH: the shell
            // installed by Nix is at a store path, and depending on the PATH
            // would make the terminal open a DIFFERENT VERSION from the running one.
            "buffer-terminal" => match std::env::current_exe() {
                Ok(me) => self.open_foreign(&me.to_string_lossy(), &["terminal".to_owned()], None),
                Err(e) => tracing::error!(error = %e, "nao sei qual e o meu proprio binario"),
            },
            // The system's terminal, for whoever wants the whole Ghostty. It is
            // a foreign buffer like any other.
            "buffer-terminal-external" => self.open_foreign("ghostty", &[], None),
            // THE TWO MONITORS, in a FLOATING window — which is niri's doing and
            // not ours (invariant 4): what the shell decides is the window's
            // `app_id`, and the rule that floats it is generated from the same
            // constant (`terminal::floating_rules`).
            //
            // Floating and not tiled because this is the window one opens ON TOP
            // of what is already there, looks at, and closes — tiled, it would
            // reflow every other buffer at the moment the machine is already
            // struggling. It is not chrome (invariant 14): nothing shows it
            // unless a key is pressed.
            "monitor-btop" => self.open_monitor(BTOP_PATH, "RONOCTUA_BTOP"),
            "monitor-bottom" => self.open_monitor(BOTTOM_PATH, "RONOCTUA_BOTTOM"),
            // The glyph browser, in an `xdg_toplevel` of its own. The
            // component's path comes from Nix, and not from the PATH: a
            // `.wasm` resolved by PATH does not exist, and a relative path
            // would depend on where the shell was launched from.
            //
            // The four below take the SAME route — this one used to have its
            // own copy of the sequence, from back when it was the only buffer.
            "glyphs-nerd-fonts" => self.open_buffer("RONOCTUA_BUFFER_GLYPHS"),
            "unicode-symbols" => self.open_buffer("RONOCTUA_BUFFER_UNICODE"),
            "lucide-icons" => self.open_buffer("RONOCTUA_BUFFER_LUCIDE"),
            "memory-open" => self.open_buffer("RONOCTUA_BUFFER_MEMORY"),
            // The two WINDOW actions are the compositor's (invariant 4). The
            // shell does not reimplement them: it asks over IPC, which is the
            // only border it has with niri (invariant 3).
            //
            // EVERYTHING ELSE goes through the action table, looked up by id.
            // It is the same one that generates niri's chord binds: an id that
            // exists in both places cannot diverge, because there is only one.
            otherwise => {
                if let Some(action) = crate::actions::kdl_of(otherwise) {
                    self.niri_action(move |n| n.action(action));
                    return;
                }
                // NOT just a `warn` in the log. A command that shows up in
                // which-key and does nothing is indistinguishable from an
                // unregistered key — that is how the five mute commands went
                // unnoticed.
                tracing::warn!(command = otherwise, "comando sem acao registrada");
                let m = self.texts.t_with(
                    ronoctua_i18n::key::ECHO_NO_ACTION,
                    &[("comando", otherwise)],
                );
                self.echo_message(m);
            }
        }
    }

    /// Asks the compositor for an action, opening a connection just for it.
    ///
    /// One connection per action, and not a stored one: the daemon's is blocked
    /// reading the event stream, and using the same one would require a `Mutex`
    /// on the path of an event that arrives dozens of times a second. Opening a
    /// local unix socket costs microseconds and happens once per key.
    fn niri_action(&mut self, f: impl FnOnce(&mut ronoctua_niri::Niri) -> anyhow::Result<()>) {
        let r = ronoctua_niri::Niri::connect().and_then(|mut n| f(&mut n));
        if let Err(e) = r {
            tracing::warn!(error = %e, "a acao no niri falhou");
            let m = self.texts.t(ronoctua_i18n::key::ECHO_NO_WINDOW);
            self.echo_message(m);
        }
    }

    /// Brings a foreign buffer up in its own scope, with its own satellite.
    ///
    /// # Why EVERY foreign buffer gets a satellite
    ///
    /// §9.1 asks for **one instance per buffer scope**. A shared satellite would
    /// make every X11 window report its pid, and the PID → cgroup correlation
    /// would start pointing all of them at the same buffer — with no error, with
    /// the modeline showing the wrong state.
    ///
    /// Giving one to each buffer, including to the ones that are native Wayland,
    /// is cheap: `xwayland-satellite` only brings the actual Xwayland up when an X
    /// client connects. A Wayland app with `DISPLAY` set simply ignores it. The
    /// alternative — guessing who is X11 — would get it wrong both ways and
    /// there would be no way of knowing.
    fn open_foreign(&mut self, program: &str, args: &[String], working_directory: Option<&str>) {
        let scope = match crate::foreign::scope_name() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "sem nome de scope; buffer nao aberto");
                return;
            }
        };
        let me = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(error = %e, "nao sei qual e o meu proprio binario");
                return;
            }
        };
        let ocupados = crate::foreign::taken_displays();
        let cmd = match crate::foreign::first_free_display(&ocupados) {
            Some(display) => crate::foreign::command_with_satellite(
                &scope,
                &me.to_string_lossy(),
                display,
                program,
                args,
                working_directory,
            ),
            // A hundred displays in use. Opening without X is better than not
            // opening: a Wayland app works the same, and an X11 one will say why it failed.
            None => {
                tracing::warn!("sem numero de display livre; buffer sobe sem satellite");
                crate::foreign::scope_command(&scope, working_directory, program, args)
            }
        };
        // The DIRECT command goes along: it is the one that runs if the scope
        // cannot be created. Rebuilding it in there would require taking the
        // `systemd-run` apart, and a new option would break the disassembly
        // silently.
        let mut direto = vec![program.to_owned()];
        direto.extend_from_slice(args);
        self.spawn_it(&scope, &direto, &cmd);
    }

    /// Orders a re-sweep if some `applications` directory changed.
    ///
    /// # Why it is still not a sweep per key
    ///
    /// Because what runs here is one `stat` per directory — three or four calls
    /// —, and the sweep itself, when it does need to happen, goes to the thread.
    /// The list opens with the catalogue that already exists and is SWAPPED when
    /// the new one arrives; nothing waits. Without this, an application installed
    /// after startup would stay invisible until the next session — and "I
    /// installed it and it does not show up" is the defect that makes a person
    /// doubt the installer.
    fn rescan_if_changed(&mut self) {
        let now = crate::apps::stamp();
        if now == self.apps_stamp {
            return;
        }
        let Some(envio) = self.catalogue_sender.clone() else {
            return;
        };
        tracing::info!("os diretorios de aplicativos mudaram; revarrendo");
        self.apps_stamp = now;
        crate::apps::scan(self.texts.language(), envio);
    }

    /// Is this `Icon=`'s pixmap already decoded?
    ///
    /// It gives back the KEY, and not the pixels: whoever draws needs it to ask
    /// the GPU whether the texture is already up, and walking around with a
    /// reference to the pixels would hold the borrow until the end of the painting.
    fn pixmap_ready(&self, icon: &str) -> Option<String> {
        match self.icons.get(icon) {
            Some(Some(_)) => Some(icon.to_owned()),
            _ => None,
        }
    }

    /// Asks the thread for the pixmaps of the VISIBLE lines that are still missing.
    ///
    /// # Why only the visible ones
    ///
    /// Because decoding four hundred SVGs costs seconds, and three hundred and
    /// eighty of them are not on screen. What is seen is at most twelve lines —
    /// the region's ceiling —, and scrolling asks for the next twelve. The cache
    /// keeps what has already arrived, so scrolling back repeats nothing.
    fn request_visible_icons(&mut self) {
        let Some(fila) = self.icon_queue.clone() else {
            return;
        };
        if self.apps.is_none() {
            return;
        }
        let (offset, per_page) = (self.current_offset(), self.grid().per_page());
        let Some(l) = self.apps.as_ref() else {
            return;
        };
        let faltam: Vec<String> = (offset..offset + per_page)
            .filter_map(|i| l.at(i))
            .map(|e| e.icon.clone())
            .filter(|n| {
                !n.is_empty() && !self.icons.contains_key(n) && !self.icons_requested.contains(n)
            })
            .collect();
        let side = crate::apps::pixmap_side().ceil() as u32;
        for icon in faltam {
            // A closed queue means the thread died. Registering the request
            // anyway would leave the name stuck in "requested" forever, and
            // the line would never try again.
            if fila
                .send(crate::apps::Request {
                    icon: icon.clone(),
                    side,
                })
                .is_err()
            {
                self.icon_queue = None;
                return;
            }
            self.icons_requested.insert(icon);
        }
    }

    /// A decoded pixmap, coming from the thread.
    fn painted(&mut self, p: crate::apps::Painting) {
        self.icons_requested.remove(&p.icon);
        let achou = p.rgba.is_some();
        // The `None` GOES INTO THE MAP. It is the answer "it does not exist", and
        // it is what stops the request from coming back on every frame.
        self.icons.insert(p.icon, p.rgba);
        // It only redraws when there is something new to show, and only with the
        // list open: an icon that arrives after the view closes must not cost a
        // frame.
        if achou && self.apps.is_some() {
            self.draw_and_paint();
        }
    }

    /// The new catalogue, coming from the sweeping thread.
    fn catalogued(&mut self, fresh: crate::apps::Catalogue) {
        self.catalogue = fresh;
        // WITH THE LIST OPEN, it updates itself WITHOUT losing what was typed.
        // Closing and reopening would wipe the query because of an event the
        // person did not cause.
        if let Some(l) = self.apps.as_mut() {
            l.recatalogue(self.catalogue.clone());
            self.follow_cursor();
            self.apply_geometry(KeyboardInteractivity::Exclusive);
            self.draw_and_paint();
        }
    }

    /// A key with the app finder open.
    fn apps_key(&mut self, key: ronoctua_keymap::Key) {
        let Some(l) = self.apps.as_mut() else {
            return;
        };
        let action = l.key(key);
        self.apply_app_action(action);
    }

    /// What the app finder asked for, with the screen already set right.
    fn apply_app_action(&mut self, action: crate::apps::Action) {
        use crate::apps::Action;
        match action {
            Action::Nothing => {}
            // REDRAW HERE, and not just re-commit the size: with the list at the
            // ceiling, filtering from 400 down to 40 changes no height at all, and
            // the compositor only sends a `configure` when the height CHANGES.
            // Without this, typing would not filter on screen — the same defect the `M-x` had.
            Action::Changed => {
                self.follow_cursor();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
                self.draw_and_paint();
            }
            Action::Closed => {
                self.apps = None;
                self.first_page();
                self.apply_geometry(self.interactivity());
                self.draw_and_paint();
            }
            Action::Open(e) => {
                // CLOSE BEFORE LAUNCHING. The launch is asynchronous (see
                // `spawn_it`), and leaving the list open on top of the application
                // that has just opened would be the footer stealing the keyboard
                // from the new window.
                self.apps = None;
                self.first_page();
                self.apply_geometry(self.interactivity());
                self.open_application(&e);
                self.draw_and_paint();
            }
        }
    }

    /// Brings up an application from the catalogue.
    ///
    /// # `Terminal=true` goes through OUR terminal
    ///
    /// Seven of 108 measured entries ask for a terminal, and they have no
    /// interface of their own: `htop` launched directly opens and dies with
    /// nothing on screen. They go by the same path as `buffer-terminal` —
    /// `current_exe terminal <prog>` —, and not by an `xterm -e` that would
    /// require one more emulator in the system.
    fn open_application(&mut self, e: &ronoctua_apps::DesktopEntry) {
        let (program, args) = &e.command;
        if !e.terminal {
            tracing::info!(app = %e.id, program, "abrindo aplicativo");
            self.open_foreign(program, args, e.working_directory.as_deref());
            return;
        }
        let Ok(me) = std::env::current_exe() else {
            tracing::error!("nao sei qual e o meu proprio binario");
            return;
        };
        let mut inside = vec!["terminal".to_owned(), program.clone()];
        inside.extend_from_slice(args);
        tracing::info!(app = %e.id, program, "abrindo aplicativo NO terminal");
        self.open_foreign(&me.to_string_lossy(), &inside, None);
    }

    /// Launches in a THREAD, and gives the event loop back right away.
    ///
    /// # Why not right here
    ///
    /// Because `systemd-run --scope` is SYNCHRONOUS and only comes back when the
    /// app CLOSES — see [`crate::foreign::launch`], which carries the quote from
    /// the manual. The `calloop` is a single thread: waiting here froze keyboard,
    /// painting and niri events for the application's whole lifetime.
    ///
    /// The verdict comes back through the channel, and that is where the echo
    /// area is written — it needs `&mut self`, which the thread does not have.
    fn spawn_it(&mut self, scope: &str, direto: &[String], cmd: &[String]) {
        let Some(envio) = self.launch_sender.clone() else {
            // With no channel there is no way of reporting, and launching blind would
            // hide the error. It only happens outside the event loop (tests).
            tracing::error!("sem canal de lancamento; buffer nao aberto");
            return;
        };
        let (scope, direto, cmd) = (scope.to_owned(), direto.to_vec(), cmd.to_vec());
        // THE THREAD'S NAME shows up in `perf` and in `gdb`. An anonymous thread
        // stuck in `wait4` for hours is exactly what nobody identifies.
        let born = std::thread::Builder::new()
            .name("ronoctua-lancar".to_owned())
            .spawn(move || {
                let verdict = crate::foreign::launch(&scope, &direto, &cmd);
                // A closed channel = the footer left. Nothing to report.
                let _ = envio.send(verdict);
            });
        if let Err(e) = born {
            tracing::error!(error = %e, "nao consegui criar a thread de lancamento");
        }
    }

    /// The verdict of a launch, already inside the event loop.
    /// Raises the thread that watches for the END of a radio scan — once.
    ///
    /// # Why it is lazy, and not raised at startup
    ///
    /// It costs a connection to NetworkManager and, on a machine with no radio,
    /// a warning in the log about something nobody asked for. Raising it when
    /// `LEADER r` is first pressed ties the cost to the interest.
    ///
    /// # Why a thread and not a timer
    ///
    /// `RequestScan` returns at once and the radio answers seconds later.
    /// Waiting for it here would freeze the footer — `calloop` runs in ONE
    /// thread, and this is the same trap `systemd-run --scope` set. A timer, in
    /// turn, would repaint a list that has not moved.
    fn watch_scans(&mut self) {
        if self.scan_watching {
            return;
        }
        let Some(echo) = self.scan_sender.clone() else {
            return;
        };
        // Marked BEFORE knowing whether it worked: on a machine with no radio it
        // fails every time, and retrying on every `LEADER r` would give a bus
        // round trip and a warning per keypress.
        self.scan_watching = true;
        if let Err(e) = ronoctua_network::watch_scan(move || {
            // The send failing means the loop is gone, which is to say the
            // footer is shutting down. There is nothing to report to.
            let _ = echo.send(());
        }) {
            tracing::debug!(error = %e, "sem vigia de varredura; a lista so rele a pedido");
        }
    }

    /// The radio has finished a scan: re-read, if the list is on screen.
    ///
    /// Re-reading with the view closed would spend a round trip on the bus to
    /// fill a state nobody is looking at — and which `fresh_one` rebuilds at
    /// opening anyway.
    fn rescanned(&mut self) {
        if let Some(v) = self.network.as_mut() {
            v.reread();
            self.draw_and_paint();
        }
    }

    fn launched(&mut self, v: crate::foreign::Launch) {
        use crate::foreign::Launch;
        match v {
            Launch::WithScope { scope, program } => {
                tracing::info!(scope, program, "buffer estrangeiro aberto");
            }
            Launch::Orphan { program, reason } => {
                tracing::warn!(program, reason, "aberto SEM cgroup proprio");
            }
            Launch::Failed { program, error } => {
                tracing::error!(error, program, "nao consegui abrir");
                let m = self.texts.t_with(
                    ronoctua_i18n::key::ECHO_NO_OPENED,
                    &[("programa", program.as_str())],
                );
                self.echo_message(m);
                self.draw_and_paint();
            }
        }
    }

    /// Takes an event from the bus and redraws if something changed.
    /// Asks the FOCUSED window's process what keys it answers to.
    ///
    /// Called when the leader opens, and only then. The alternative — asking on
    /// every key — would put a socket round trip between a keystroke and the
    /// screen, and "which-key in one frame" is a measured target.
    ///
    /// A window that is not one of our buffers answers nothing, and the empty
    /// list is what removes `SPC m` from the tree. That is deliberate: an entry
    /// that opens onto an empty screen teaches that there was something there.
    fn refresh_buffer_keymap(&mut self) {
        self.buffer_pending.clear();
        self.buffer_bindings.clear();
        self.buffer_pid = None;

        let Some(pid) = self.niri.focused().and_then(|w| w.pid) else {
            return;
        };
        let Ok(pid) = u32::try_from(pid) else { return };
        match crate::leader::buffer_keymap(pid) {
            Ok(bindings) if !bindings.is_empty() => {
                tracing::debug!(pid, bindings = bindings.len(), "keymap do buffer em foco");
                self.buffer_pid = Some(pid);
                self.buffer_bindings = bindings;
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "nao consegui ler o keymap do buffer"),
        }
    }

    /// The which-key rows for the buffer's keymap at the current depth.
    fn buffer_inputs(&self) -> Vec<ronoctua_keymap::Entry> {
        narrow_buffer_keymap(&self.buffer_bindings, &self.buffer_pending_refs())
    }

    /// Handles a key inside `SPC m`, and says whether it did.
    ///
    /// `false` means "not mine": the key goes on to the tree exactly as before,
    /// and nothing about the rest of the keymap changes.
    fn buffer_key(&mut self, key: ronoctua_keymap::Key) -> bool {
        let typed = ronoctua_keymap::notation(&[key]);

        if self.buffer_pending.is_empty() {
            // Opening it. Only at the ROOT of the sequence, and only when the
            // focused window actually published something — `m` after `w` is
            // `w m`, which belongs to the tree, and a window that is not one of
            // our buffers has no `SPC m` at all.
            if !(self.leader.at_root()
                && typed == ronoctua_protocol::LOCAL_PREFIX
                && !self.buffer_bindings.is_empty())
            {
                return false;
            }
        } else if typed == "ESC" {
            self.buffer_pending.clear();
            self.close_leader();
            return true;
        }

        self.buffer_pending.push(typed);

        // A COMPLETE SEQUENCE RUNS, in the buffer's own process. Listing without
        // running would teach keys that do nothing, which is worse than not
        // listing at all — it is the whole reason this went in with both halves.
        let done = self.buffer_bindings.iter().find(|(sequence, _)| {
            sequence.split_whitespace().collect::<Vec<_>>() == self.buffer_pending_refs()
        });
        if let Some((_, id)) = done {
            let id = id.clone();
            let pid = self.buffer_pid;
            self.buffer_pending.clear();
            self.close_leader();
            match pid.map(|p| crate::leader::buffer_command(p, &id)) {
                Some(Ok(())) => tracing::info!(command = %id, "comando do buffer em foco"),
                Some(Err(e)) => tracing::warn!(error = %e, command = %id, "o buffer recusou"),
                None => tracing::warn!(command = %id, "sem buffer para receber o comando"),
            }
            return true;
        }

        let narrowed = self.buffer_inputs();
        if narrowed.is_empty() {
            // A key the buffer does not answer to ends the sequence, like any
            // other key that leads nowhere.
            self.buffer_pending.clear();
            self.close_leader();
            return true;
        }

        self.inputs = narrowed;
        self.first_page();
        self.echo = None;
        self.apply_geometry(KeyboardInteractivity::Exclusive);
        true
    }

    fn buffer_pending_refs(&self) -> Vec<&str> {
        self.buffer_pending.iter().map(String::as_str).collect()
    }

    /// Turns a killed cgroup into something a person can read.
    ///
    /// The cgroup is resolved through [`crate::foreign::scope_from_cgroup`],
    /// which is the same parser the modeline already uses to tell which buffer a
    /// window belongs to — so the name comes from code that exists and is
    /// tested, not from a second guess at systemd's path format.
    ///
    /// When it resolves to nothing the notification says so instead of inventing
    /// a name: something outside the person's buffers was closed, and the raw
    /// cgroup is the only honest thing to show.
    fn oom_notification(
        &self,
        cgroup: &str,
        reason: &ronoctua_notify::Reason,
    ) -> ronoctua_notify::Notification {
        let scope = crate::foreign::scope_from_cgroup(cgroup);
        let body = match (&scope, reason) {
            (Some(app), ronoctua_notify::Reason::MemoryUsed) => self
                .texts
                .t_with(ronoctua_i18n::key::NOTIF_OOM_USED, &[("app", app)]),
            (Some(app), _) => self
                .texts
                .t_with(ronoctua_i18n::key::NOTIF_OOM_PRESSURE, &[("app", app)]),
            (None, _) => self
                .texts
                .t_with(ronoctua_i18n::key::NOTIF_OOM_ORPHAN, &[("cgroup", cgroup)]),
        };
        ronoctua_notify::Notification {
            id: 0,
            app: self.texts.t(ronoctua_i18n::key::NOTIF_OOM_APP),
            summary: self.texts.t(ronoctua_i18n::key::NOTIF_OOM_SUMMARY),
            body,
            urgency: ronoctua_notify::Urgency::Critical,
            icon: None,
            // NEVER EXPIRES. The queue already refuses to expire a critical one,
            // and this says the same thing at the source: a message explaining
            // why an application disappeared has to survive until it is read.
            expires: None,
        }
    }

    fn notification_of(&mut self, e: ronoctua_notify::Event) {
        let now = std::time::Instant::now();
        match e {
            ronoctua_notify::Event::New(n) => {
                let critical = n.urgency == ronoctua_notify::Urgency::Critical;
                self.notifications.enqueue(*n, now);
                // A CRITICAL one that does not fit forces the expansion (section 4.3),
                // instead of being truncated. Half a critical is worse than none: the
                // user sees that there is something and does not know what.
                if critical {
                    self.history_open = true;
                    self.first_page();
                }
            }
            ronoctua_notify::Event::Close(id) => {
                self.notifications.dismiss(id, now);
            }
            // THE MACHINE CLOSED AN APPLICATION SO IT WOULD NOT FREEZE.
            //
            // Always critical, and that is the whole point of routing it here:
            // a critical notification never expires and forces the history open,
            // so what would otherwise be a window vanishing without explanation
            // becomes a sentence the person can read at their own pace.
            ronoctua_notify::Event::OomKilled { cgroup, reason } => {
                let n = self.oom_notification(&cgroup, &reason);
                self.notifications.enqueue(n, now);
                self.history_open = true;
                self.first_page();
            }
        }
        // The queue's state on ONE greppable line. The modeline's segment also
        // shows it, but it starts with a PUA glyph — a grep for `text= 2` does
        // not match, because between the `=` and the space there is a glyph.
        tracing::info!(
            showing = self.notifications.showing().map(|n| n.id),
            pending = self.notifications.pending(),
            critical = self.notifications.has_critical(),
            "fila de notificacoes"
        );

        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// Recomputes the layout and paints. The pair shows up in every event source.
    fn draw_and_paint(&mut self) {
        self.draw();
        self.paint();
    }

    /// Activates the leader: asks for the keyboard and expands.
    ///
    /// The keyboard request is `Exclusive` on purpose. `OnDemand` would give
    /// focus only on a click, and the footer has nothing clickable.
    fn ativar_leader(&mut self) {
        // "which-key in one frame" (5.2), counted from here.
        //
        // What stays OUT: niri receives the key and runs `ronoctua-shell
        // leader`, a new process, which only then writes to the socket. That
        // stretch is bigger than ours and does not show up in this number;
        // measuring it requires timing the binary from outside. See
        // docs/ARCHITECTURE.md.
        //
        // The FIRST screen marks it: with two monitors, marking on all of them
        // would give one measurement per monitor of the same opening.
        if let Some(g) = self.screens.first_mut().and_then(|t| t.gpu.as_mut()) {
            g.mark_as("which-key", std::time::Instant::now());
        }
        self.leader.ativar();
        self.first_page();
        // WHAT THE FOCUSED BUFFER ANSWERS TO, asked once, here.
        //
        // The row only appears when it published something: a window that is not
        // one of our buffers leaves `SPC m` out of the list entirely, which is
        // the honest shape. A submenu that opens onto an empty screen teaches
        // that there was something there.
        self.refresh_buffer_keymap();

        self.inputs = match self.tree.resolve(&[]) {
            ronoctua_keymap::Resolution::Prefix(e) => e,
            // A keymap with no binding at all: there is nothing to show, and
            // capturing the keyboard would only take input away from the buffers.
            _ => {
                self.leader.finish();
                return;
            }
        };
        // The buffer's row goes in only when there IS a buffer answering.
        if !self.buffer_bindings.is_empty()
            && let Ok(key) = ronoctua_keymap::Key::parse(ronoctua_protocol::LOCAL_PREFIX)
        {
            self.inputs.push(ronoctua_keymap::Entry {
                key: ronoctua_protocol::LOCAL_PREFIX.to_owned(),
                sym: key,
                label: ronoctua_i18n::key::WHICH_BUFFER.to_owned(),
                command: None,
                is_group: true,
            });
        }
        // THE INHERITANCE IS RECAPTURED HERE, and not only on the `enter`.
        //
        // Whatever is held at the instant the sequence starts is not its — the
        // leader **is** `Ctrl+Alt+Space`, so Ctrl and Alt are down by construction. While this
        // only happened on gaining focus, a session that never got a `leave`
        // carried the old inheritance forever: the nested compositor has already
        // stopped dispatching altogether, and a `leave` lost in that hole brings
        // EVERY binding down with nothing saying why.
        //
        // Without focus this makes no difference: `self.mods` is zeroed, the
        // `enter` that comes next zeroes the inheritance, and the following
        // announcement captures it.
        self.inheritance.on_sequence_start(self.mods);
        tracing::info!(inputs = self.inputs.len(), "leader ativa");
        self.apply_geometry(KeyboardInteractivity::Exclusive);
        // AND IT PAINTS. Asking for the size is not enough: whoever paints is the
        // `configure` coming back, and the compositor only sends it when the
        // height CHANGES. Here it always changes, so this is redundant today —
        // and it was exactly that bet that left the finder without redrawing on filter.
        self.draw_and_paint();
    }

    /// Makes sure the focused item's image is decoded.
    ///
    /// Once per item, and not per frame: a 4K capture is 33 MB of RGBA, and
    /// redoing that every frame would put megabytes into §5.2's budget.
    fn decode_preview(&mut self) {
        if self
            .clip_image
            .as_ref()
            .is_some_and(|(i, _)| *i == self.clip_cursor)
        {
            return;
        }
        self.clip_image = None;
        let Some(bytes) = self.clipboard.at(self.clip_cursor).and_then(|e| e.image()) else {
            return;
        };
        let start = std::time::Instant::now();
        let Some(r) = ronoctua_clipboard::image::decode(bytes) else {
            // A format we cannot read, or too big. The entry stays in the
            // history — what is lost is the preview.
            tracing::debug!(bytes = bytes.len(), "a imagem copiada nao decodificou");
            return;
        };
        tracing::debug!(
            width = r.width,
            height = r.height,
            ms = start.elapsed().as_secs_f64() * 1000.0,
            "previa decodificada"
        );
        self.clip_version = self.clip_version.wrapping_add(1);
        self.clip_image = Some((
            self.clip_cursor,
            ronoctua_render::image::Rgba {
                width: r.width,
                height: r.height,
                pixels: r.pixels,
            },
        ));
    }

    /// A key with the clipboard view open.
    ///
    /// It does **not** fall into the leader's keymap: the view captured the
    /// keyboard, and `d` there is remove, not `LEADER d`. Each key fires a NAMED
    /// command — the same ones as [`crate::leader::OF_CLIPBOARD_VIEW`],
    /// which is the list the orphan check reads.
    fn clipboard_key(&mut self, sym: ronoctua_keymap::Sym) {
        use ronoctua_keymap::Sym;
        let how_many = self.clipboard.how_many();
        match sym {
            Sym::Esc => {
                self.clipboard_open = false;
                self.clip_qr = None;
                self.first_page();
                self.apply_geometry(self.interactivity());
                self.draw_and_paint();
                return;
            }
            // STEPPING THROUGH THE LIST. Without wrapping around: a list that goes
            // back to the top by itself makes the finger lose where it was.
            Sym::Down | Sym::Up => {
                if how_many == 0 {
                    return;
                }
                let fresh = if sym == Sym::Down {
                    self.clip_cursor.saturating_add(1).min(how_many - 1)
                } else {
                    self.clip_cursor.saturating_sub(1)
                };
                if fresh == self.clip_cursor {
                    return;
                }
                self.clip_cursor = fresh;
                self.decode_preview();
                // The QR belonged to the PREVIOUS item. Leaving it on screen would
                // show one thing's code beside another thing's text.
                self.clip_qr = None;
                self.follow_cursor();
            }
            otherwise => {
                // THE REST ARE NAMED COMMANDS, by the same table the orphan
                // check reads. A key that is not there does nothing — and does
                // not end the view, because ending on a wrong key would lose
                // the place in the list.
                let notation = ronoctua_keymap::Key::simple(otherwise).notation();
                let Some((_, id)) = crate::leader::OF_CLIPBOARD_VIEW
                    .iter()
                    .find(|(t, _)| *t == notation)
                else {
                    return;
                };
                self.run_command(id);
            }
        }
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// Answers the question "remember this network?".
    ///
    /// `sim` reconnects with `psk-flags=1` and seals the password in the TPM;
    /// any other answer leaves it as it is — `psk-flags=2`, nothing on disk.
    ///
    /// The `(ssid, password)` pair comes out of the state with `take()`
    /// **before** anything can fail: a password left behind after an error
    /// would be a password in memory with no owner and no deadline.
    fn answer_remember(&mut self, sim: bool) {
        let Some((ssid, password)) = self.network_remember.take() else {
            return;
        };
        let m = if sim {
            match self
                .network
                .as_mut()
                .map(|v| v.connect(&ssid, Some(&password), true))
            {
                Some(Ok(())) => self
                    .texts
                    .t_with(ronoctua_i18n::key::ECHO_NETWORK_STORED, &[("rede", &ssid)]),
                Some(Err(e)) => self.texts.t_with(
                    ronoctua_i18n::key::ECHO_NETWORK_STORE_FAILED,
                    &[("motivo", &e.to_string())],
                ),
                None => self.texts.t(ronoctua_i18n::key::ECHO_NETWORK_UNAVAILABLE),
            }
        } else {
            self.texts.t(ronoctua_i18n::key::ECHO_NETWORK_NO_STORED)
        };
        self.echo_message(m);
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// Opens or closes the notification history.
    ///
    /// It uses THE surface, in the expanded region — below the modeline +
    /// minibuffer pair. There is no second surface anywhere in the system
    /// (invariant 14).
    fn alternar_historico(&mut self) {
        self.history_open = !self.history_open;
        self.first_page();
        tracing::info!(
            open = self.history_open,
            lines = self.expansion().len(),
            pages = self.pages(),
            "historico de notificacoes"
        );
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// How long a message from the echo area stays on screen.
    ///
    /// Three seconds: time to read a short line without it becoming permanent
    /// noise. The one-second timer that expires a notification also wipes it —
    /// there is no second clock.
    const ECHO_SECONDS: u64 = 3;

    /// Puts a message in the echo area.
    fn echo_message(&mut self, text: String) {
        tracing::info!(echo = %text, "area de eco");
        self.echo = Some((
            text,
            std::time::Instant::now() + std::time::Duration::from_secs(Self::ECHO_SECONDS),
        ));
    }

    /// Wipes the message if it expired. Gives back `true` if something changed.
    fn expire_echo(&mut self, now: std::time::Instant) -> bool {
        match &self.echo {
            Some((_, up_to)) if now >= *up_to => {
                self.echo = None;
                true
            }
            _ => false,
        }
    }

    /// Leaves the leader: gives the keyboard back and returns to two lines.
    fn close_leader(&mut self) {
        self.leader.finish();
        self.inputs.clear();
        tracing::info!("leader encerrada");
        // IT GIVES THE KEYBOARD BACK ONLY IF NOBODY ELSE WANTS IT.
        //
        // The sequence that OPENS the `M-x` ends here: running the command
        // ends the leader. Giving it back unconditionally took the keyboard
        // away from the prompt that had just asked for it.
        self.apply_geometry(self.interactivity());
    }

    /// Which of the six views is in the expanded region right now.
    ///
    /// **The precedence order is written HERE, and only here.** Whoever is in
    /// the middle of something wins: the four modals captured the whole
    /// keyboard, then comes the leader (a sequence under way), and the history
    /// is passive and yields to all of them. See [`ViewState`].
    const fn view(&self) -> ViewState {
        if self.clipboard_open {
            ViewState::Clipboard
        } else if self.manual {
            ViewState::Manual
        } else if self.apps.is_some() {
            ViewState::Applications
        } else if self.network.is_some() {
            ViewState::Network
        } else if self.mx.active() {
            ViewState::Commands
        } else if self.leader.active_one() {
            ViewState::Leader
        } else if self.history_open {
            ViewState::History
        } else {
            ViewState::NoneOf
        }
    }

    /// Does the footer have the keyboard?
    ///
    /// Asking only about the leader — as it used to — gave the wrong answer on
    /// every modal: the prompt showed up on screen and did NOT receive a key,
    /// and nothing gave it away; it looked as if the keyboard had died.
    const fn grabbing(&self) -> bool {
        self.view().grabs()
    }

    /// What to ask the compositor for right now.
    const fn interactivity(&self) -> KeyboardInteractivity {
        if self.grabbing() {
            KeyboardInteractivity::Exclusive
        } else {
            KeyboardInteractivity::None
        }
    }

    /// The height wanted right now, in px.
    ///
    /// Two lines at rest; two plus one per which-key entry when the leader is
    /// active. The list stays BELOW the minibuffer — it is a vertico, not a
    /// menu that pops up (see `CLAUDE.md`).
    fn wanted_height(&self) -> u32 {
        // Which-key, history and `M-x` use THE SAME expanded area, and never two
        // at the same time: they are three uses of the same surface (invariant
        // 14), not three surfaces.
        //
        // The precedence order is that of whoever is in the middle of something:
        // the `M-x` captured the whole keyboard, so it wins; then the leader,
        // which is a sequence under way; the history is passive and yields to both.
        //
        let lines = 2 + u32::try_from(self.expansion_rows()).unwrap_or(0);
        (tokens::metrics::line_height(tokens::font::SIZE_BASE) * lines as f32).ceil() as u32
    }

    /// How far the expansion may grow, in lines.
    ///
    /// # Why there is a ceiling
    ///
    /// Without it the region grows with the content: an `M-x` with 170 commands
    /// took the whole screen, and the footer stopped being a footer (invariant
    /// 14). What does not fit becomes a PAGE — see [`Footer::pagina`].
    ///
    /// # Why two limits
    ///
    /// The ceiling in lines is blind to the screen's size: twelve lines are a
    /// third of a 1080 px monitor and more than half of a 600 px one. The
    /// percentage puts the second limit, and the smaller of the two holds —
    /// measured on the SMALLEST screen, because the footer's height is the same
    /// on all of them (see `apply_geometry`).
    fn expansion_ceiling(&self) -> usize {
        // The SMALLEST screen is in charge: the footer's height is the same on
        // all of them (see `apply_geometry`), so a ceiling that only fitted
        // on the big monitor would leak on the small one.
        let menor = self
            .screens
            .iter()
            .map(|t| t.output_height)
            .filter(|h| *h > 0)
            .min();
        expansion_ceiling(menor)
    }

    /// How the expanded region is navigated right now.
    ///
    /// The SHORTCUT list paginates; everything else scrolls. The difference is
    /// not a matter of taste: a shortcut list shows what fits and the rest is
    /// another page, and a manual is running text where "page 2 of 5" does not
    /// say where you are.
    const fn navigation(&self) -> Navigation {
        match self.view() {
            ViewState::Leader => Navigation::Page,
            _ => Navigation::Scroll,
        }
    }

    /// The expanded region's grid right now.
    ///
    /// On scrolling it is a single column — a list. On pagination it is columns
    /// side by side, filling the width before the height.
    fn grid(&self) -> Grid {
        let lines = self.expansion();
        let widths: Vec<Measure> = lines.iter().map(ExpansionRow::measurement).collect();
        // The width comes from the first CONFIGURED screen, and not from the
        // first of the list: `first()` could be at zero width, and then
        // everything would fit in one column.
        let largura_px = self
            .screens
            .iter()
            .find(|t| t.configured && t.width > 0)
            .map_or(0, |t| t.width);
        let columns = tokens::metrics::columns(largura_px as f32, tokens::font::SIZE_BASE);
        // THE CLIPBOARD USES ONLY THE LEFT COLUMN.
        //
        // Its list is the left panel; on the right sits the content, which is
        // drawn separately. Letting the grid spread into columns — as it does on
        // the shortcut list — would put the history on top of the content.
        if self.clipboard_open {
            return Grid {
                columns: 1,
                lines: self.expansion_ceiling().min(widths.len().max(1)),
                width: list_columns(columns),
                colon: 0,
                icon: 0,
            };
        }
        grid_for(
            &widths,
            columns,
            self.expansion_ceiling(),
            self.navigation() == Navigation::Page,
        )
    }

    /// How big a step the offset takes.
    ///
    /// On pagination, page by page: with a grid of three columns, stepping one
    /// line would make the list shiver instead of turn.
    fn scroll_step(&self) -> usize {
        match self.navigation() {
            Navigation::Page => self.grid().per_page(),
            Navigation::Scroll => 1,
        }
    }

    /// How many lines the expansion takes right now — ceiling already applied.
    fn expansion_rows(&self) -> usize {
        if self.expansion().is_empty() {
            return 0;
        }
        self.grid().lines
    }

    /// How many pages the current content has.
    fn pages(&self) -> usize {
        self.expansion()
            .len()
            .div_ceil(self.grid().per_page())
            .max(1)
    }

    /// The first visible line, corrected if the content shrank.
    ///
    /// Corrected on READING, and not stored corrected: the content changes
    /// underneath the offset (a notification expires, the filter narrows), and
    /// a stored value would be left pointing past the end with nothing warning
    /// it. The symptom would be a blank region.
    fn current_offset(&self) -> usize {
        // The LAST offset goes by PAGE when it paginates: with a grid of three
        // columns, stopping at `per_page` from the end would leave two empty
        // columns on the last turn.
        let per_page = self.grid().per_page();
        let last = last_offset(self.expansion().len(), per_page);
        let step = self.scroll_step().max(1);
        (self.offset - self.offset % step).min(last)
    }

    /// EVERY line of the expanded region, with no cut and no page.
    ///
    /// # Why a single function
    ///
    /// Before, the height counted the lines by one path and the drawing
    /// positioned them by another. They diverged once — the surface asked for
    /// twenty-two lines to draw six, and the symptom was a huge gap in the
    /// footer. Here the two read the SAME list: what the height reserves is
    /// exactly what the drawing emits.
    ///
    /// The precedence order is that of whoever is in the middle of something:
    /// the `M-x` captured the whole keyboard, so it wins; then the leader,
    /// which is a sequence under way; the history is passive and yields to both.
    fn expansion(&self) -> Vec<ExpansionRow> {
        // THE CLIPBOARD beats everything: like the manual and the `M-x`, it
        // captured the whole keyboard, and the region is its while it is open.
        if self.clipboard_open {
            if self.clipboard.how_many() == 0 {
                return vec![ExpansionRow {
                    text: self.texts.t("clipboard-empty"),
                    text_color: tokens::color::TEXT_DIM,
                    ..ExpansionRow::empty_one()
                }];
            }
            return self
                .clipboard
                .inputs()
                .iter()
                .enumerate()
                .map(|(i, e)| ExpansionRow {
                    // The PIN in the key's column: it is the list's only mark, and
                    // it fits where there is already a reserved column.
                    key: if e.pinned {
                        PINNED_MARK.to_owned()
                    } else {
                        String::new()
                    },
                    // ONE LINE. A line break becomes a space: a text of five
                    // lines would take five items of the list and the history
                    // would become a single item on the screen.
                    //
                    // AN IMAGE HAS NO LINE OF TEXT. The label is translated
                    // (invariant 8) and carries the size, which is what tells
                    // two captures apart in the list.
                    text: e.text().map_or_else(
                        || {
                            format!(
                                "{} · {} KB",
                                self.texts.t("clipboard-image"),
                                e.content.bytes().len() / 1024
                            )
                        },
                        one_line,
                    ),
                    text_color: if i == self.clip_cursor {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_SECONDARY
                    },
                    key_color: tokens::color::STATE_SUCCESS,
                    click: Some(Click::Candidate(i)),
                    ..ExpansionRow::empty_one()
                })
                .collect();
        }
        // THE MANUAL wins: it captured the whole keyboard, like the `M-x`.
        if self.manual {
            return crate::manual::assemble(&self.texts, &crate::leader_chord())
                .into_iter()
                .map(|l| ExpansionRow {
                    key: l.key,
                    // In the manual the description IS the item: there is no command
                    // to show, and there is nothing to click — it is only read.
                    text: l.description,
                    // A section title in the group tone, for the same reason as the
                    // `+submenu`: it organizes, it does not run.
                    text_color: if l.title {
                        tokens::color::TEXT_GROUP
                    } else {
                        tokens::color::TEXT_SECONDARY
                    },
                    ..ExpansionRow::empty_one()
                })
                .collect();
        }
        // THE APPLICATIONS, in the same region and by the same mechanism. Three
        // columns: the glyph, the name (with the search highlight) and the
        // description — which the drawing elides, because `Comment` is prose and
        // does not fit.
        if let Some(l) = &self.apps {
            if l.total() == 0 {
                // THE SWEEP HAS NOT ARRIVED YET (or found nothing). Saying that
                // is better than an empty region, which is indistinguishable
                // from "the key did not work".
                return vec![ExpansionRow {
                    text: self.texts.t(ronoctua_i18n::key::APPS_EMPTY),
                    text_color: tokens::color::TEXT_DIM,
                    ..ExpansionRow::empty_one()
                }];
            }
            let query = l.normalized_query();
            return l
                .result()
                .iter()
                .enumerate()
                .filter_map(|(i, _)| l.at(i).map(|e| (i, e)))
                .map(|(i, e)| ExpansionRow {
                    // THE PIXMAP ONCE IT HAS ARRIVED, the glyph while it has not.
                    //
                    // The choice is HERE, and in a single place: the two drawn
                    // together would end up one over the other, and the decision
                    // duplicated in the painting loop would diverge from the one
                    // that decides the column's width.
                    pixmap: self.pixmap_ready(&e.icon),
                    icon: self
                        .pixmap_ready(&e.icon)
                        .is_none()
                        .then(|| crate::apps::glyph_of(e)),
                    // THE NAME in the left column, flush with it: it is what the
                    // search matches and what the highlight lights up. Pushing four
                    // hundred names right would give a jagged left margin.
                    key: e.name.clone(),
                    text: e.description.clone(),
                    key_flush_right: false,
                    // NO `:` BETWEEN THEM. `Firefox : navegador web` asserts a
                    // relation that does not exist — the column already separates them.
                    separator: false,
                    // THE HIGHLIGHT IS OVER THE NAME, and not over the folded key:
                    // the folding is 1:1 in characters precisely so the positions
                    // hold in both. See `ronoctua_apps::fold`.
                    highlights: crate::minibuffer::match_up(&query, &e.key)
                        .map(|(_, p)| p)
                        .unwrap_or_default(),
                    click: Some(Click::Application(i)),
                    // THE DESCRIPTION IS ALREADY ON THE LINE. Repeating it in the
                    // message area on hover would show the same text twice; what
                    // helps there is the command that is going to run.
                    description: Some(e.command.0.clone()),
                    key_color: if i == l.cursor() {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_PRIMARY
                    },
                    text_color: if i == l.cursor() {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_DIM
                    },
                })
                .collect();
        }
        // THE NETWORK. It comes after the finder and before the `M-x` — the
        // precedence of the seven views lives only in `Footer::view()`, and
        // this order here only follows it.
        if let Some(v) = &self.network {
            // THE QUESTION COMES AT THE TOP, with the list below it.
            //
            // In the list's place, and not over it: whoever has just connected
            // needs to see which network is being talked about. A question alone
            // on an empty screen would lose the context the line itself gives.
            let mut header = Vec::new();
            if let Some((ssid, _)) = &self.network_remember {
                header.push(ExpansionRow {
                    icon: Some(tokens::icon::KEY),
                    text: self
                        .texts
                        .t_with(ronoctua_i18n::key::NETWORK_REMEMBER, &[("rede", ssid)]),
                    text_color: tokens::color::STATE_ACCENT,
                    ..ExpansionRow::empty_one()
                });
            }
            let lines = v.lines();
            if lines.is_empty() {
                // THREE DIFFERENT EMPTIES, and not one generic "no networks".
                //
                // Node bus, no radio and radio switched off have different fixes:
                // the first has none, the second none either, and the third is
                // one key. A single text for all three would send the person
                // looking in the wrong place.
                let key = if !v.available() {
                    ronoctua_i18n::key::ECHO_NETWORK_UNAVAILABLE
                } else if !v.tem_radio() {
                    ronoctua_i18n::key::NETWORK_NO_ADAPTER
                } else if v.radio_on() {
                    ronoctua_i18n::key::NETWORK_EMPTY
                } else {
                    ronoctua_i18n::key::NETWORK_RADIO_OFF
                };
                header.push(ExpansionRow {
                    text: self.texts.t(key),
                    text_color: tokens::color::TEXT_DIM,
                    ..ExpansionRow::empty_one()
                });
                return header;
            }
            header.extend(lines.into_iter().enumerate().map(|(i, l)| {
                let focus = i == v.cursor();
                let (icon, key, text) = match &l {
                    crate::network::Line::Wired {
                        interface,
                        state,
                        rate_kbps,
                    } => (
                        tokens::icon::ETHERNET,
                        interface.clone(),
                        if *state == ronoctua_network::State::Connected {
                            format!(
                                "{} · {}",
                                self.texts.t(ronoctua_i18n::key::NETWORK_WIRED),
                                ronoctua_network::readable_rate(*rate_kbps)
                            )
                        } else {
                            self.texts.t(ronoctua_i18n::key::NETWORK_WIRED)
                        },
                    ),
                    crate::network::Line::Wireless(r) => {
                        // THE PADLOCK ONLY ON THE SECURED ONE. An open network gets
                        // the absence of a padlock, and not an open padlock: the
                        // column already says so by being empty.
                        let mut parts = vec![r.band.label().to_owned()];
                        if r.active_one {
                            parts.push(self.texts.t(ronoctua_i18n::key::NETWORK_CONNECTING));
                        } else if r.saved {
                            parts.push(self.texts.t(ronoctua_i18n::key::NETWORK_SAVED));
                        } else if !r.secured {
                            parts.push(self.texts.t(ronoctua_i18n::key::NETWORK_OPEN));
                        }
                        (signal_bars(r.signal), r.ssid.clone(), parts.join(" · "))
                    }
                };
                ExpansionRow {
                    icon: Some(icon),
                    key,
                    text,
                    key_flush_right: false,
                    // Node `:` between name and detail: the column already separates
                    // them, and the `:` would assert a relation that does not exist.
                    separator: false,
                    click: Some(Click::Network(i)),
                    key_color: if focus {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_PRIMARY
                    },
                    text_color: if focus {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_DIM
                    },
                    ..ExpansionRow::empty_one()
                }
            }));
            return header;
        }

        if let Some(id) = self.mx.asking_for() {
            // ASKING FOR AN ARGUMENT: the candidate list disappears (the choice
            // has already been made) and what stays in the region is the chosen
            // command's description — which is the only thing that helps decide
            // what to type.
            return vec![ExpansionRow {
                text: self
                    .command_registry
                    .search(id)
                    .map(|c| self.texts.t(c.label))
                    .unwrap_or_default(),
                ..ExpansionRow::empty_one()
            }];
        }
        if self.mx.active() {
            return self
                .mx
                .candidates()
                .iter()
                .enumerate()
                .map(|(i, item)| ExpansionRow {
                    // TWO COLUMNS: the name and the description.
                    //
                    // The id alone was not enough with 172 commands —
                    // `niri-focus-column-left` says what it does, but
                    // `niri-set-column-display` does not, and guessing by the
                    // name is what the finder exists to avoid. The description
                    // comes from the command's own key, so it is the SAME one
                    // the shortcut list shows.
                    key: item.id.clone(),
                    // The item's own label wins; an empty one means "it is a
                    // command", and then the description comes from the registry
                    // — the SAME one the shortcut list shows.
                    text: if item.label.is_empty() {
                        self.command_registry
                            .search(&item.id)
                            .map(|c| self.texts.t(c.label))
                            .unwrap_or_default()
                    } else {
                        item.label.clone()
                    },
                    // One hundred and seventy names pushed right would give a
                    // jagged left margin; here the useful alignment is that of
                    // the start of the name.
                    key_flush_right: false,
                    highlights: crate::minibuffer::match_up(&self.mx.normalized_query(), &item.id)
                        .map(|(_, p)| p)
                        .unwrap_or_default(),
                    click: Some(Click::Candidate(i)),
                    description: None,
                    icon: None,
                    pixmap: None,
                    separator: true,
                    // Focus is told apart by COLOR, never by a box or an
                    // outline (invariant 15).
                    key_color: if i == self.mx.cursor() {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_PRIMARY
                    },
                    text_color: if i == self.mx.cursor() {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_SECONDARY
                    },
                })
                .collect();
        }
        if self.leader.active_one() {
            return self
                .inputs
                .iter()
                .map(|e| ExpansionRow {
                    key: e.key.clone(),
                    // THE ITEM IS THE COMMAND, as in Doom.
                    //
                    // It used to be the translated label, and it answered the
                    // wrong question: whoever looks at the list wants to know
                    // WHAT IS GOING TO RUN, and the command's name is what is
                    // typed later in the finder. The whole sentence did not
                    // disappear — it goes to the message area when the pointer
                    // passes over it.
                    //
                    // In a group there is no command, so what stays is the
                    // group's name with the `+` — which is put on HERE and not
                    // in the translation: it used to live in there
                    // (`group-window = +window`) and that is why three of the
                    // six groups did not have it.
                    text: match &e.command {
                        Some(id) => id.clone(),
                        None => format!("{GROUP_MARK}{}", self.texts.t(&e.label)),
                    },
                    description: Some(self.texts.t(&e.label)),
                    click: Some(Click::Key(e.sym)),
                    text_color: if e.is_group {
                        tokens::color::TEXT_GROUP
                    } else {
                        tokens::color::TEXT_SECONDARY
                    },
                    ..ExpansionRow::empty_one()
                })
                .collect();
        }
        if self.history_open {
            return self
                .notifications
                .history()
                .iter()
                .rev()
                .map(|n| ExpansionRow {
                    text: n.line(),
                    ..ExpansionRow::empty_one()
                })
                .collect();
        }
        Vec::new()
    }

    /// Is the pointer over the scrollbar?
    ///
    /// The bar is the last cell of the width, and it only exists when there is
    /// something to scroll. The width comes from the first configured screen —
    /// the bar is drawn on all of them, and whoever clicks is on one of them.
    fn on_thumb(&self, x: f64) -> bool {
        if !self.has_bar() {
            return false;
        }
        let Some(width) = self
            .screens
            .iter()
            .find(|t| t.configured && t.width > 0)
            .map(|t| f64::from(t.width))
        else {
            return false;
        };
        let cell = f64::from(tokens::metrics::cell_width(tokens::font::SIZE_BASE));
        x >= width - cell
    }

    /// Puts the top of what is seen where the pointer is, dragging the bar.
    ///
    /// The handle follows the cursor: the fraction of the region's height
    /// becomes the fraction of the list. There is no correction for "where
    /// inside the handle it was grabbed" — with a handle of one or two lines,
    /// it is worth less than the line of code that would implement it.
    fn drag_to(&mut self, y: f64) {
        let line_height = f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE));
        let ceiling = self.expansion_ceiling();
        let total = self.expansion().len();
        let start = f64::from(FIRST_OF_EXPANSION) * line_height;
        let gap = ceiling as f64 * line_height;
        if gap <= 0.0 {
            return;
        }
        let fracao = ((y - start) / gap).clamp(0.0, 1.0);
        self.offset = ((total as f64 * fracao) as usize).min(last_offset(total, ceiling));
    }

    /// Which line of the page is at height `y` of the surface.
    ///
    /// `None` when the pointer is over the modeline, over the minibuffer, or
    /// past the last line — there is no item to point at there, and inventing
    /// one would make the message area describe something that is not under
    /// the cursor.
    fn row_at(&self, x: f64, y: f64) -> Option<usize> {
        let grid = self.grid();
        item_at(
            x,
            y,
            f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE)),
            f64::from(tokens::metrics::cell_width(tokens::font::SIZE_BASE)),
            grid,
            self.expansion().len().saturating_sub(self.current_offset()),
        )
    }

    /// The visible line at index `i` on the current page.
    fn visible_row(&self, i: usize) -> Option<ExpansionRow> {
        self.expansion().into_iter().nth(self.current_offset() + i)
    }

    /// Runs whatever a click on that line means.
    ///
    /// # Why clicking is pressing the key
    ///
    /// On a group, the click goes down one level; on a command, it runs. Both
    /// things come for free out of "clicking = pressing the line's key", and
    /// what is gained is that there is no second path: nothing is reachable
    /// only by the mouse, and what the mouse does is always something the
    /// keyboard also does.
    fn click_at(&mut self, i: usize) {
        let Some(line) = self.visible_row(i) else {
            return;
        };
        match line.click {
            Some(Click::Key(t)) => {
                let effect = self.leader.key(&self.tree, t);
                self.apply_leader_effect(effect);
            }
            Some(Click::Application(n)) => {
                let Some(l) = self.apps.as_mut() else {
                    return;
                };
                // BY THE SAME `Enter` as the keyboard: nothing may be reachable only
                // by the mouse, nor behave differently depending on how one got
                // to it.
                let action = l.choose(n);
                self.apply_app_action(action);
                self.draw_and_paint();
            }
            Some(Click::Network(n)) => {
                if let Some(v) = self.network.as_mut() {
                    v.focus_on(n);
                    self.draw_and_paint();
                }
            }
            Some(Click::Candidate(n)) => {
                use crate::minibuffer::Action;
                match self.mx.choose(n, &self.command_registry) {
                    Action::Run(id) => {
                        self.run_command(&id);
                        self.apply_geometry(self.interactivity());
                    }
                    // A CLICK ON AN ITEM IS THE SAME AS `Enter` ON IT. The
                    // handling lives in one place on purpose: the mouse and the
                    // keyboard reaching different behaviours is exactly the bug
                    // `choose` was written to avoid.
                    Action::Selected(id) => {
                        self.select_item(&id);
                        self.apply_geometry(self.interactivity());
                    }
                    Action::RunWith(id, arg) => {
                        self.run_with(&id, Some(&arg));
                        self.apply_geometry(self.interactivity());
                    }
                    // `Changed` here is the action that ASKED for an argument: the list
                    // disappears and the line becomes a prompt, with the keyboard still
                    // in the footer.
                    Action::Changed => self.apply_geometry(self.interactivity()),
                    _ => {}
                }
                self.draw_and_paint();
            }
            None => {}
        }
    }

    /// There is more content than fits — that is, there is something to scroll.
    ///
    /// It is the bar's condition: a bar that takes the whole height says
    /// nothing, and one that shows up without there being scrolling lies about
    /// there being more.
    fn has_bar(&self) -> bool {
        // ONLY WHAT SCROLLS HAS A BAR. On a paginated list it would be a
        // continuous control for content that moves in jumps — and what says
        // where you are there is the counter.
        self.navigation() == Navigation::Scroll && self.expansion().len() > self.grid().per_page()
    }

    /// Navigates the expanded region without touching the leader's sequence.
    ///
    /// It is what the held `CtrlRight` does. It gives back `true` if something
    /// changed — redrawing with no change would spend a frame painting the same.
    ///
    /// A key outside this set is **ignored**, and not passed on: with the
    /// `CtrlRight` held the person is navigating, and a letter that escaped
    /// from there would fire a command in the middle of it.
    fn navigate(&mut self, sym: ronoctua_keymap::Sym) -> bool {
        match navigation_step(sym) {
            Some(p) => self.page_by(p),
            None => false,
        }
    }

    /// Scrolls to keep the focused item visible, with a margin.
    ///
    /// # Why a margin, and not "only when it leaves the screen"
    ///
    /// Scrolling only when the cursor touches the edge leaves whoever is
    /// reading without context: the next line only shows up after you have
    /// already reached it. With two lines of margin, what comes next is on
    /// screen already when you decide to go down — it is vi's `scrolloff`, and
    /// the request was exactly that: start following before the end.
    fn follow_cursor(&mut self) {
        let Some(cursor) = self.list_cursor() else {
            return;
        };
        let ceiling = self.expansion_ceiling();
        let total = self.expansion().len();
        let margem = SCROLL_MARGIN.min(ceiling.saturating_sub(1) / 2);

        let offset = self.current_offset();
        let fresh = if cursor < offset + margem {
            // Going up: the cursor does not touch the top of the region.
            cursor.saturating_sub(margem)
        } else if cursor + margem >= offset + ceiling {
            // Going down: it stops `margem` lines from the end.
            (cursor + margem + 1).saturating_sub(ceiling)
        } else {
            offset
        };
        self.offset = fresh.min(last_offset(total, ceiling));
    }

    /// Which item of the expanded region is focused, when there is one.
    ///
    /// Only the finder has focus: the shortcut list is navigated by key, and
    /// the manual has no selected item — it is read.
    // NOT `const` any more: `asking_for` now returns `Option<&str>` over an owned
    // `String`, because the locator's items are dynamic — a buffer's id is not a
    // `&'static str`. A `const fn` cannot deref that, and the alternative would be
    // keeping the whole locator on static ids, which is what kept `LEADER b b`
    // from existing.
    fn list_cursor(&self) -> Option<usize> {
        match self.view() {
            ViewState::Commands if self.mx.asking_for().is_none() => Some(self.mx.cursor()),
            // This used to be a hand-written `match` because the function was
            // `const` and a `const fn` cannot call `Option::map`. It is not const
            // any more (see the note on the signature), so the longhand is just
            // longhand now.
            ViewState::Applications => self.apps.as_ref().map(crate::apps::Finder::cursor),
            // THE "REMEMBER" QUESTION TAKES THE FIRST LINE when it is on screen,
            // and the cursor here is the VISUAL position — it is what decides
            // where the region scrolls to. Without the offset, the scrolling
            // would aim one line above the focused one.
            //
            // The click's index does NOT carry this adjustment, and that is
            // right: it is the list's index, not the screen's.
            ViewState::Network => self
                .network
                .as_ref()
                .map(|v| v.cursor() + usize::from(self.network_remember.is_some())),
            _ => None,
        }
    }

    /// Goes back to the top of the region.
    ///
    /// Called where the CONTENT changes — new key, new filter, new list.
    /// Without this, narrowing the list while scrolled down would show an
    /// empty region, and the key would look as if it had not worked.
    fn first_page(&mut self) {
        self.offset = 0;
    }

    /// Scrolls the region by LINES, without leaving the extremes.
    ///
    /// Gives back `true` if it changed — redrawing with no change spends a
    /// frame to paint the same thing. It does not wrap around on purpose:
    /// going back to the start after the end makes the eye lose its place.
    fn scroll_by(&mut self, lines: i32) -> bool {
        let last = last_offset(self.expansion().len(), self.grid().per_page());
        let current = self.current_offset();
        let fresh = scrolling(current, last, lines);
        self.offset = fresh;
        fresh != current
    }

    /// Scrolls a whole PAGE.
    ///
    /// A page is what fits right now, and not a fixed number: on a short
    /// screen the ceiling is smaller, and paging by twelve lines would skip
    /// content that never got to show up.
    fn page_by(&mut self, step: i32) -> bool {
        let per_page = i32::try_from(self.scroll_step()).unwrap_or(1);
        self.scroll_by(step.saturating_mul(per_page.max(1)))
    }

    /// Applies size and keyboard grab to the surface.
    ///
    /// The **exclusive zone follows the height**: expanding pushes the buffers
    /// up. See the top of the module.
    fn apply_geometry(&mut self, keyboard: KeyboardInteractivity) {
        // EVERY screen changes height together.
        //
        // It is not a detail: if only the focused screen grew, the zone would be
        // different between monitors and the buffers on one of them would reflow
        // when screen focus changed — with nothing having changed in them (section 9.2).
        let height = self.wanted_height();
        // THE ZONE FOLLOWS THE HEIGHT. See the top of the module: the expansion
        // pushes the buffers, it does not cover them.
        let exclusiva = height as i32;
        for t in &self.screens {
            t.layer.set_size(0, height);
            t.layer.set_exclusive_zone(exclusiva);
            t.layer.set_keyboard_interactivity(keyboard);
            t.layer.commit();
        }
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "flush da conexao falhou");
        }
    }

    /// Undoes the Caps Lock over the character xkb produced.
    ///
    /// # Why the Lock must not decide a shortcut
    ///
    /// The keymap tells case apart on purpose — `w h` focuses the window on the
    /// left, `w H` MOVES it there. That distinction is **Shift**: one more
    /// gesture, deliberate, at the moment. The Caps Lock is a state that stays
    /// on, that gets switched on by accident, and that the person forgot they
    /// switched on.
    ///
    /// With it on and without this function, xkb delivers `I` where `i` was
    /// typed, **every** lowercase binding of the keymap dies at once, and the
    /// only thing on screen is `CTRL ALT SPC I is not bound` — which shows the
    /// consequence and hides the cause. Worse still on the ones that exist in
    /// both cases: `w h` would become `w H`, and the window would MOVE when the
    /// request was to focus. A shortcut that does something else is worse than
    /// one that does nothing.
    ///
    /// Inverting the case is exactly undoing the Lock: with it on xkb has
    /// already applied the inversion, and the Shift along with it undid it.
    /// Only letters change.
    fn without_caps_lock(c: char, caps: bool) -> char {
        if !caps {
            return c;
        }
        // `next()` and not `collect()`: they are 1:1 mappings in the ASCII and
        // Latin-1 the keymap uses; German's `ß`/`SS` is not a shortcut key, and
        // falling on the first character is better than allocating.
        if c.is_uppercase() {
            c.to_lowercase().next().unwrap_or(c)
        } else if c.is_lowercase() {
            c.to_uppercase().next().unwrap_or(c)
        } else {
            c
        }
    }

    /// Translates an xkb key event into our `Key`.
    ///
    /// Gives back `None` for keys that do not represent input — modifiers on
    /// their own, for instance. Treating them as a key would make "holding
    /// Ctrl" cancel the leader.
    fn translate(&self, event: &KeyEvent) -> Option<ronoctua_keymap::Key> {
        use ronoctua_keymap::{Key, Sym};

        let sym = match event.keysym {
            Keysym::Escape => Sym::Esc,
            Keysym::Return | Keysym::KP_Enter => Sym::Enter,
            Keysym::Tab => Sym::Tab,
            Keysym::BackSpace => Sym::Backspace,
            Keysym::space => Sym::Space,
            // The arrows go into the keymap: `LEADER <left>` focuses the column on
            // the left. Without these cases they fall into `key_char()`, which gives
            // back `None`, and the key would be swallowed silently.
            Keysym::Left => Sym::Left,
            Keysym::Right => Sym::Right,
            Keysym::Up => Sym::Up,
            Keysym::Down => Sym::Down,
            otherwise => Sym::Char(Self::without_caps_lock(
                otherwise.key_char()?,
                self.mods.caps_lock,
            )),
        };

        let key = Key {
            mods: self.inheritance.discount(self.mods),
            sym,
        };
        // THE KEY AS IT ENDED UP, in the log.
        //
        // Without this, "I pressed it and nothing happened" does not tell "the
        // key did not arrive" from "it arrived with a stuck modifier and did not
        // match" — and the two are fixed in opposite places. It took four
        // occurrences for this line to exist.
        tracing::debug!(?key, inherited = ?self.inheritance, "tecla depois do desconto");
        Some(key)
    }

    /// Opens a native buffer: our own binary, on the `buffer` subcommand.
    ///
    /// # Why it exists
    ///
    /// Because there are FOUR of them now, and there will be more — the `i`
    /// group was born saying so. The sequence (find our own executable, pass
    /// the component's path, warn on screen when the variable is not in the
    /// environment) is the same for all of them, and copying it per buffer
    /// would give copies that diverge the day one of them gains a step.
    fn open_buffer(&mut self, variable: &str) {
        match (std::env::current_exe(), buffer_component(variable)) {
            (Ok(me), Some(w)) => {
                self.open_foreign(&me.to_string_lossy(), &["buffer".to_owned(), w], None)
            }
            (_, None) => {
                tracing::error!(variable, "a variavel do componente nao esta no ambiente");
                let m = self.texts.t(ronoctua_i18n::key::ECHO_NO_BUFFER);
                self.echo_message(m);
            }
            (Err(e), _) => tracing::error!(error = %e, "nao sei qual e o meu proprio binario"),
        }
    }

    /// Opens a process monitor in a floating terminal window.
    ///
    /// Same shape as [`Self::open_buffer`] and for the same reasons: the program
    /// arrives by store path, and an absent variable says so on screen instead
    /// of spawning a name that a scope's minimal `PATH` cannot resolve.
    ///
    /// The subcommand is `monitor` and not `terminal` because what tells one
    /// window from the other — and therefore what floats it — is the `app_id`.
    fn open_monitor(&mut self, program: Option<&str>, variable: &str) {
        match (std::env::current_exe(), program) {
            (Ok(me), Some(p)) => self.open_foreign(
                &me.to_string_lossy(),
                &["monitor".to_owned(), p.to_owned()],
                None,
            ),
            (_, None) => {
                tracing::error!(variable, "a variavel do monitor nao foi definida no build");
                let m = self.texts.t_with(
                    ronoctua_i18n::key::ECHO_NO_OPENED,
                    &[("programa", variable)],
                );
                self.echo_message(m);
            }
            (Err(e), _) => tracing::error!(error = %e, "nao sei qual e o meu proprio binario"),
        }
    }

    /// Assembles the graphics context the first time there is a size.
    ///
    /// There is no assembling it earlier: wgpu needs the dimensions, and they
    /// only arrive on the compositor's first `configure`.
    fn ensure_gpu(&mut self, i: usize) {
        let Some(screen) = self.screens.get(i) else {
            return;
        };
        if screen.gpu.is_some() || screen.width == 0 {
            return;
        }
        let layer = &screen.layer;

        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        };

        // With the `system` feature on, these give back a RAW pointer (the
        // backend is libwayland through FFI). Without the feature the methods do
        // not even exist — there is no C underneath the pure Rust backend.
        let (Some(disp), Some(surf)) = (
            std::ptr::NonNull::new(self.conn.backend().display_ptr().cast::<std::ffi::c_void>()),
            std::ptr::NonNull::new(layer.wl_surface().id().as_ptr().cast::<std::ffi::c_void>()),
        ) else {
            tracing::error!("ponteiros do Wayland nulos; sem contexto grafico");
            return;
        };

        let display = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(disp));
        let window = RawWindowHandle::Wayland(WaylandWindowHandle::new(surf));

        // SAFETY: `self.conn` and `self.layer` are fields of `self`, just like
        // the `Gpu` — so the handles do not outlive the objects they came from.
        // It is the contract of `Gpu::new`, now by construction.
        #[allow(unsafe_code)]
        let gpu = unsafe {
            ronoctua_render::gpu::Gpu::new(
                display,
                window,
                self.screens[i].width,
                self.screens[i].height,
            )
        };

        match gpu {
            Ok(g) => {
                match ronoctua_render::Text::new(
                    &g.device,
                    &g.queue,
                    g.config.format,
                    tokens::font::SIZE_BASE,
                ) {
                    Ok(t) => {
                        match ronoctua_render::quad::Quads::new(&g.device, g.config.format) {
                            Ok(q) => self.screens[i].quads = Some(q),
                            // With no rectangle the footer still draws the TEXT.
                            // Losing the modeline's background is ugly; losing the
                            // whole modeline would be worse.
                            Err(e) => {
                                tracing::error!(error = %e, "desenhador de retangulos falhou")
                            }
                        }
                        match ronoctua_render::image::Images::new(&g.device, g.config.format) {
                            Ok(im) => self.screens[i].images = Some(im),
                            // Without it the whole footer carries on: what is lost is
                            // the preview of the copied image, and the history's text
                            // stays on screen.
                            Err(e) => tracing::error!(error = %e, "desenhador de imagem falhou"),
                        }
                        self.screens[i].text = Some(t);
                        // "first frame in 200 ms" (5.2), counted from the start of
                        // the process. Only the FIRST screen marks it: with two
                        // monitors, the second would measure the time up to a frame
                        // that is not the first.
                        let mut g = g;
                        if i == 0 {
                            g.mark_as("primeiro-frame", ronoctua_render::start());
                        }
                        self.screens[i].gpu = Some(g);
                        tracing::info!(screen = i, "contexto grafico e desenhador prontos");
                    }
                    Err(e) => tracing::error!(error = %e, "desenhador de texto falhou"),
                }
            }
            // NOT fatal: with no GPU the footer still resolves the layout and logs.
            // Failing here would bring the whole session down because of a driver.
            Err(e) => {
                tracing::error!(error = %e, "sem contexto grafico; o rodape nao vai pintar")
            }
        }
    }

    /// Paints a frame on EVERY screen.
    ///
    /// The content is the same on all of them — modeline, minibuffer and the
    /// expansion — because the two lines exist on every output (section 9.2).
    /// What changes per screen is the WIDTH, and that is why the layout is
    /// resolved inside the loop: a narrow monitor elides segments the wide one shows.
    fn paint(&mut self) {
        // THE MISSING ICONS, ASKED FOR BEFORE ANY BORROW.
        //
        // Here is the only place that knows what is visible — and scrolling and
        // filtering change that without going through any other path. The
        // request is asynchronous: this frame goes out with the glyphs, and the
        // next one with whatever has arrived.
        self.request_visible_icons();
        // Everything that READS `self` comes out here before the loop's mutable
        // borrows — `segments_now` reads `self`, and the two things together
        // do not pass the borrow checker.
        let mensagem = self.notifications.showing().map(|n| {
            (
                n.line(),
                // A critical is told apart by COLOR, not by a box (invariant 15).
                if n.urgency == ronoctua_notify::Urgency::Critical {
                    tokens::color::STATE_CRITICAL
                } else {
                    tokens::color::STATE_ACCENT
                },
            )
        });

        // THE PENDING SEQUENCE on the minibuffer's line, as in Doom.
        //
        // It already showed up in the modeline's mode indicator. That is not
        // enough: the indicator is a narrow segment that ELIDES, and the place
        // one looks at while typing a chord is the line below — the echo area.
        let pendente = self
            .leader
            .active_one()
            .then(|| self.leader.prefix(&self.leader_label));
        let echo = self.echo.as_ref().map(|(t, _)| t.clone());

        // THE PREVIEW COMES OUT HERE, before the loop's mutable borrows. It is a
        // REFERENCE: cloning 33 MB of RGBA per frame would be worse than having
        // no preview.
        let imagem_pronta = self.clip_image.as_ref().map(|(_, r)| r);
        let version = self.clip_version;
        // THE ALREADY DECODED PIXMAPS, by reference and for the SAME reason: each
        // one is small, but cloning twelve per frame would be paying for the
        // decoding again in memory.
        let icones_prontos = &self.icons;

        let segments = self.segments_now();
        let mini_repouso = crate::minibuffer_at_rest();
        // THE PROMPT OF THE MINIBUFFER'S LINE, from either of the two
        // finders.
        //
        // The `M-x` one carries the `M-x ` in front: it is the notation of the
        // key itself, the same in every language, and it is what says that that
        // thing there is the COMMAND finder — which coexists with the shortcut
        // list.
        //
        // The applications one has NO prefix, by the owner's request. And it is
        // not only taste: a label that changes with nothing — not with the
        // arrow, not with what is typed — takes the line without answering any
        // question. The list below already says what it is, and what the line
        // has to add is what was typed.
        let prompt_mx = match &self.apps {
            Some(l) => Some(l.query().to_owned()),
            None => self.mx.active().then(|| self.mx.prompt()),
        };

        // THE VISIBLE PAGE of the expanded region.
        //
        // The whole list comes out of `expansion()`; here it is cut ONCE, and the
        // same cut holds for the height (`expansion_rows`). Cutting in two
        // places is what once gave a huge gap in the footer.
        let grid = self.grid();
        let navigation = self.navigation();
        let pages = self.pages();
        let offset = self.current_offset();
        let all_of = self.expansion();
        let visible: Vec<&ExpansionRow> =
            all_of.iter().skip(offset).take(grid.per_page()).collect();
        // `2/4` to the RIGHT of the minibuffer's line: it is already the line
        // that says what is under way, and the indicator only exists when there
        // is more than one page — a permanent `1/1` would be noise.
        //
        // THE PAGE INDICATOR DOES NOT HOLD FOR THE FINDER.
        //
        // There there is no page: the list SCROLLS behind the focused item, and
        // a `3/15` beside what is being typed tells a story that is not the
        // search's — what matters is what matched, not which slice of the list
        // one is in. The scrollbar already says the position, for whoever wants it.
        //
        // THE COUNTER BELONGS TO WHOEVER PAGINATES, and it comes with the KEY
        // that turns the page — a `1/2` on its own says that there is another
        // page and does not say how to get to it, which is the half that matters.
        //
        // Whoever SCROLLS has no counter: the bar already says the position, and
        // "page 2 of 5" in running text does not answer where you are.
        let indicador = (navigation == Navigation::Page && pages > 1).then(|| {
            format!(
                "{}  {}/{pages}",
                self.texts.t(ronoctua_i18n::key::EXPANSION_PAGE),
                offset / grid.per_page().max(1) + 1
            )
        });
        let total_expansao = all_of.len();
        // THE DESCRIPTION OF THE ITEM UNDER THE CURSOR.
        //
        // The list shows the COMMAND; the whole sentence shows up here, on the
        // message line, when the pointer passes over it. It is Doom's split: the
        // list is to be swept with the eye, and the description only when it is
        // asked for — and pointing with the mouse is the question.
        let sob_o_cursor = self
            .row_under_pointer
            .and_then(|i| visible.get(i))
            .and_then(|l| l.description.clone());
        // WHERE THE POINTER IS, marked by a BACKGROUND.
        //
        // # Why the pointer gets a background and the focus does not
        //
        // Because the two are different things and need different signals. The
        // focus — what the `Enter` is going to run — is told apart by the text's
        // COLOR, and it has been so since the `M-x` (invariant 15: never by a
        // box or an outline). Giving both a background would leave the question
        // "which one is selected?" unanswered at the moment the pointer rests
        // over another line.
        //
        // # Only what is CLICKABLE
        //
        // The background says "this responds to the click". In the manual, which
        // is only read, a lit line would promise an action that does not exist.
        let realce_do_ponteiro = self
            .row_under_pointer
            .filter(|i| visible.get(*i).is_some_and(|l| l.click.is_some()))
            .map(|i| grid.position(i));
        // Does the footer have the keyboard? It is what decides whether the
        // accent stays on the window (the compositor's border) or comes to the
        // buffer's name.
        let com_teclado = self.grabbing();
        let barra_visivel = self.under_pointer && self.has_bar();
        let primeira_pintura = self.frames_painted < 3;
        let mut pintou = false;

        for index in 0..self.screens.len() {
            let width = self.screens[index].width;
            if width == 0 {
                continue;
            }
            // PER SCREEN: a narrow monitor elides what the wide one shows.
            let columns = tokens::metrics::columns(width as f32, tokens::font::SIZE_BASE);
            let placed = layout::resolve(columns, &segments);

            // THE ORDER OF THE THREE REGIONS, from top to bottom:
            //
            //   line 0         modeline   — the buffer's name
            //   line 1         minibuffer — GLUED to it
            //   lines 2..n     expansion  — which-key, `M-x`, history
            //
            // The footer's two lines stay TOGETHER and the list grows BELOW
            // them. The owner's decision, and the third position the expansion
            // has taken in this project: it has been above the modeline (and the
            // buffer's name jumped on every key) and between the two (and the two
            // lines that are read together were separated by twenty of list).
            // Gluing the pair, what is read as one thing appears as one thing,
            // and what grows grows away from it.
            //
            // The surface is still anchored at the bottom: the pair goes up as the
            // list grows, and that is why the list has a CEILING (`expansion_ceiling`).
            let linha_modeline = 0u16;
            let linha_mini = 1u16;

            let mut spans: Vec<ronoctua_render::Span> = placed
                .iter()
                .map(|c| ronoctua_render::Span {
                    size: None,
                    text: c.text.clone(),
                    column: c.column,
                    line: linha_modeline,
                    color: segment_color(c.id, com_teclado),
                })
                .collect();

            // THE PRECEDENCE of the minibuffer's line, from top to bottom:
            //
            //   0. item under the cursor — asking with the mouse wins, because it
            //      is a deliberate and ephemeral gesture: it goes away when the
            //      pointer leaves, and the pending sequence comes back.
            //   1. `M-x` open           — the user is typing there.
            //   2. leader active        — the pending sequence, as in Doom.
            //   3. echo                 — the shell's last message.
            //   4. notification         — the message region of section 4.2.
            //   5. at rest              — empty.
            //
            // The notification on screen takes the message region — the minibuffer's
            // line. There is no popup and no card (invariants 14 and 15).
            //
            // The pending sequence comes BEFORE the notification on purpose: while a
            // chord is being typed, what matters is the chord. A notification
            // arriving in the middle would wipe from the screen what the user is
            // building.
            let (mini, cor_mini) = if let Some(d) = &sob_o_cursor {
                (d.clone(), tokens::color::TEXT_SECONDARY)
            } else if let Some(p) = &prompt_mx {
                (p.clone(), tokens::color::TEXT_PRIMARY)
            } else if let Some(p) = &pendente {
                (p.clone(), tokens::color::STATE_ACCENT)
            } else if let Some(e) = &echo {
                (e.clone(), tokens::color::TEXT_SECONDARY)
            } else {
                match &mensagem {
                    Some((t, c)) => (t.clone(), *c),
                    None => (mini_repouso.clone(), tokens::color::TEXT_PRIMARY),
                }
            };
            if !mini.is_empty() {
                spans.push(ronoctua_render::Span {
                    size: None,
                    text: mini,
                    column: tokens::space::MODELINE_EDGELEFT,
                    line: linha_mini,
                    color: cor_mini,
                });
            }

            // THE PAGE INDICATOR, flush against the right edge of the minibuffer's
            // line. There is no bar and no box: it is text on the line that already
            // exists (invariants 14 and 15).
            if let Some(ind) = &indicador {
                let largura_ind = layout::width(ind);
                if largura_ind < columns {
                    spans.push(ronoctua_render::Span {
                        size: None,
                        text: ind.clone(),
                        column: columns - largura_ind,
                        line: linha_mini,
                        color: tokens::color::TEXT_DIM,
                    });
                }
            }

            // THE EXPANDED REGION — which-key, `M-x` candidates, history.
            //
            // ONE loop for the three, over the SAME list that decided the height.
            // There used to be three loops, each with its own starting-line
            // arithmetic, and the height came from a fourth one; the chance of the
            // four agreeing was what kept the whole footer standing.
            for (i, line) in visible.iter().enumerate() {
                // THE GRID decides where: column and row come out of it, and not from
                // arithmetic here. On scrolling it has a single column, and this becomes
                // the usual list.
                let (base, y_grade) = grid.position(i);
                let y = FIRST_OF_EXPANSION + y_grade;
                let colon = base + grid.colon;
                let with_key = !line.key.is_empty();
                // THE ICON BEFORE EVERYTHING, in the margin. It is drawn even
                // when the rest of the line is elided: it is the fastest answer
                // to "which program is this".
                if let Some(g) = line.icon {
                    spans.push(ronoctua_render::Span {
                        size: None,
                        text: g.to_string(),
                        column: base + tokens::space::MODELINE_EDGELEFT,
                        line: y,
                        color: line.key_color,
                    });
                }
                if with_key {
                    // FLUSH RIGHT within its own column when it is a shortcut:
                    // that is what puts the colons of `RET` and of `a` in the same
                    // column. For a command name, flush left.
                    //
                    // ELIDED AT THE END, and not in the middle: the highlight's
                    // positions are counted from the start of the text, and cutting
                    // the middle would make the mark light a letter that is no
                    // longer there. At the end, what survived the cut keeps the
                    // positions it had.
                    let fits = grid.colon.saturating_sub(grid.start() + 1);
                    let key = layout::elide_end(&line.key, fits);
                    let width = layout::width(&key);
                    let column = if line.key_flush_right {
                        colon.saturating_sub(width + 1)
                    } else {
                        base + grid.start()
                    };
                    // THE LETTERS THAT MATCHED, IN ANOTHER COLOR.
                    //
                    // One `Span` per run of the same color, and not one per
                    // character: the cell is exact, so each run's column is
                    // arithmetic — and one `Span` per letter would multiply the
                    // shaping work by twenty.
                    for (text, coluna_relativa, casou) in runs_of(&key, &line.highlights) {
                        spans.push(ronoctua_render::Span {
                            size: None,
                            text,
                            column: column + coluna_relativa,
                            line: y,
                            color: if casou {
                                tokens::color::STATE_ACCENT
                            } else {
                                line.key_color
                            },
                        });
                    }
                    if line.separator {
                        spans.push(ronoctua_render::Span {
                            size: None,
                            text: SEPARATOR_OF.to_owned(),
                            column: colon,
                            line: y,
                            // DISCREET: it separates, it does not inform. In the tone
                            // the modeline gives to what dies first in the elision.
                            color: tokens::color::TEXT_DIM,
                        });
                    }
                }
                // THE RIGHT COLUMN ELIDES TOO.
                //
                // Until now NOTHING in the expanded region elided: a description
                // of two hundred cells went over the scrollbar and out past the
                // edge, and the compositor does not clip — it simply vanished,
                // with nothing saying that there was more.
                let start = grid.text_start(with_key);
                spans.push(ronoctua_render::Span {
                    size: None,
                    text: layout::elide_end(&line.text, grid.fits_in_text(with_key, base, columns)),
                    column: base + start,
                    line: y,
                    color: line.text_color,
                });
            }

            // THE CLIPBOARD'S RIGHT PANEL.
            //
            // The list on the left came out of the loop above, in the usual
            // model — that is what gives it scrolling, click and highlight for
            // free. What follows is only the other half: the item's content,
            // what it is, and what can be done with it.
            //
            // There is no box and no shadow (invariant 15): what separates the
            // two columns is a rule, which is the only separation the project uses.
            let mut qr_quads: Vec<ronoctua_render::quad::Quad> = Vec::new();
            let mut preview: Option<ronoctua_render::image::Area> = None;
            let line_height = tokens::metrics::line_height(tokens::font::SIZE_BASE);

            // THE APPLICATIONS' PIXMAPS, one per visible line that already has
            // its own. The ones that do not have it drew the glyph in the loop
            // above, and that is what makes the list wait for nobody.
            let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
            let side = crate::apps::pixmap_side();
            let mut pixmaps: Vec<(String, ronoctua_render::image::Area)> = Vec::new();
            for (i, line) in visible.iter().enumerate() {
                let Some(key) = &line.pixmap else {
                    continue;
                };
                let (base, y_grade) = grid.position(i);
                pixmaps.push((
                    key.clone(),
                    ronoctua_render::image::Area {
                        x: f32::from(base + tokens::space::MODELINE_EDGELEFT) * cell,
                        // CENTERED ON THE LINE, and not flush with the top: the
                        // font's box is `1.45 at` and the drawing is `1.00`, so
                        // without centering the icon would touch the line above.
                        y: f32::from(FIRST_OF_EXPANSION + y_grade) * line_height
                            + (line_height - side) / 2.0,
                        width: side,
                        height: side,
                    },
                ));
            }
            if self.clipboard_open {
                let start = list_columns(columns).saturating_add(2);
                let painel = columns.saturating_sub(start);
                let end = FIRST_OF_EXPANSION + u16::try_from(grid.lines).unwrap_or(0);
                let chosen = self.clipboard.at(self.clip_cursor);

                // THE LAST THREE LINES are fixed: what the item is, ClipCascade's
                // state, and the keys. They stay at the bottom because they are
                // reference — what is read is the content, and it stays at the top.
                let rodape_do_painel = 3u16;
                let alto = end
                    .saturating_sub(FIRST_OF_EXPANSION)
                    .saturating_sub(rodape_do_painel);

                if let Some(m) = &self.clip_qr {
                    // THE QR IN QUADS, one per module. The module's size comes from
                    // the available HEIGHT — the region has a ceiling (invariant 14),
                    // and a fixed number would give a code that does not fit on a
                    // short screen or is tiny on a tall one. Illegible is the same as
                    // absent: nobody can point the camera at it.
                    let available = f32::from(alto) * line_height;
                    let side = m.side as f32;
                    let modulo = (available / (side + 2.0)).floor().max(1.0);
                    let x0 =
                        f32::from(start) * tokens::metrics::cell_width(tokens::font::SIZE_BASE);
                    let y0 = f32::from(FIRST_OF_EXPANSION) * line_height + modulo;
                    // THE QUIET ZONE around it is part of the code: without it the
                    // camera does not find the edges. That is why the background is a
                    // bigger quad, and not only the dark modules.
                    qr_quads.push(ronoctua_render::quad::Quad {
                        x: x0,
                        y: y0 - modulo,
                        width: (side + 2.0) * modulo,
                        height: (side + 2.0) * modulo,
                        color: tokens::color::TEXT_PRIMARY,
                    });
                    for y in 0..m.side {
                        for x in 0..m.side {
                            if !m.dark(x, y) {
                                continue;
                            }
                            qr_quads.push(ronoctua_render::quad::Quad {
                                x: x0 + (x as f32 + 1.0) * modulo,
                                y: y0 + (y as f32) * modulo,
                                width: modulo,
                                height: modulo,
                                color: tokens::color::SURFACE_CHROME,
                            });
                        }
                    }
                } else if let Some((_, rgba)) = &self.clip_image {
                    // THE IMAGE PREVIEW takes the content area, fitted without
                    // stretching. It is a TEXTURED QUAD on the SAME surface — there
                    // is no window and no popup (invariant 14), and there is no
                    // border, radius or shadow (invariant 15).
                    let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
                    preview = Some(
                        ronoctua_render::image::Area {
                            x: f32::from(start) * cell,
                            y: f32::from(FIRST_OF_EXPANSION) * line_height,
                            width: f32::from(painel) * cell,
                            height: f32::from(alto) * line_height,
                        }
                        .fit_into(rgba.width, rgba.height),
                    );
                } else if let Some(t) = chosen.and_then(ronoctua_clipboard::Entry::text) {
                    // THE HIGHLIGHT IS PER LINE, and not over the whole text.
                    //
                    // The highlighter's ranges are offsets in BYTES of the original
                    // text, and `wrap_text` rewrites the text — it joins spaces,
                    // splits a long word. Matching the two would require `wrap_text`
                    // to give back the mapping, which would cut across its
                    // responsibility. Highlighting each already broken line gives the
                    // same result on screen and keeps the two things apart.
                    let tongue = ronoctua_clipboard::highlight::guess(t);
                    for (n, l) in wrap_text(t, painel, usize::from(alto))
                        .into_iter()
                        .enumerate()
                    {
                        let line = FIRST_OF_EXPANSION + u16::try_from(n).unwrap_or(0);
                        spans.extend(highlight_line(&l, tongue, line, start));
                    }
                }

                // WHAT THE ITEM IS: types and size, on a single dimmed line.
                if let Some(e) = chosen {
                    let sheet = format!(
                        "{}  {}  ·  {}  {} B",
                        self.texts.t("clipboard-types"),
                        e.kinds.first().map(String::as_str).unwrap_or("-"),
                        self.texts.t("clipboard-size"),
                        e.content.bytes().len()
                    );
                    spans.push(ronoctua_render::Span {
                        size: None,
                        text: layout::elide_end(&sheet, painel),
                        line: end.saturating_sub(3),
                        column: start,
                        color: tokens::color::TEXT_DIM,
                    });
                }

                // CLIPCASCADE, telling the truth. The integration does not exist
                // yet (docs/PENDING.md), and an indicator that pretended
                // "connected" would be the façade this project undoes.
                spans.push(ronoctua_render::Span {
                    size: None,
                    text: layout::elide_end(
                        &format!(
                            "{}: {}",
                            self.texts.t("clipboard-clipcascade"),
                            self.texts.t("clipboard-clipcascade-missing")
                        ),
                        painel,
                    ),
                    line: end.saturating_sub(2),
                    column: start,
                    color: tokens::color::STATE_WARNING,
                });

                // THE ACTIONS, with each one's key. They come out of the SAME
                // table the orphan check reads — a legend written by hand would be
                // the second copy that lies the day a key changes.
                let acoes = crate::leader::OF_CLIPBOARD_VIEW
                    .iter()
                    .map(|(t, id)| {
                        format!(
                            "{t} {}",
                            self.command_registry
                                .search(id)
                                .map_or_else(|| (*id).to_owned(), |c| self.texts.t(c.label))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("  ·  ");
                spans.push(ronoctua_render::Span {
                    size: None,
                    text: layout::elide_end(&acoes, painel),
                    line: end.saturating_sub(1),
                    column: start,
                    color: tokens::color::TEXT_DIM,
                });
            }

            // THE MODELINE'S BACKGROUND, one tone above the rest.
            //
            // It covers ONLY the buffer-name line, and not the whole pair: the
            // two play different roles — one says where you are, the other is
            // where you type — and the difference in background is what separates
            // them with no rule and no box (invariant 15). Glued as they are now,
            // that difference is the only thing that still tells them apart.
            //
            // The rest of the surface is already `surface.chrome`, the color the
            // `frame` clears with.
            let mut fundos = vec![ronoctua_render::quad::Quad {
                x: 0.0,
                y: f32::from(linha_modeline) * line_height,
                width: width as f32,
                height: line_height,
                color: tokens::color::MODELINE_BACKGROUND,
            }];

            // THE LINE UNDER THE POINTER.
            //
            // The `surface.selection` token existed from the beginning, described
            // as "background of the focused row inside an expanded list", and it
            // **never reached the footer**: only the installer's TUI used it. The
            // list gave no sign at all of where the mouse was — clicking was an
            // act of faith.
            //
            // It is a change of background color, and not a box: no border, no
            // radius, no shadow (invariant 15). It goes BEFORE the scrollbar in
            // the list of quads, so the bar stays on top of it.
            if let Some((base, y_grade)) = realce_do_ponteiro {
                let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
                fundos.push(ronoctua_render::quad::Quad {
                    x: f32::from(base) * cell,
                    y: f32::from(FIRST_OF_EXPANSION + y_grade) * line_height,
                    // THE WIDTH OF THE GRID'S COLUMN, and not that of the screen:
                    // with the shortcut list in three columns, a full-screen band
                    // would light the three neighboring lines together.
                    width: f32::from(grid.width) * cell,
                    height: line_height,
                    color: tokens::color::SURFACE_SELECTION,
                });
            }

            // THE VERTICAL RULE between the clipboard's two columns, and the
            // QR's modules. Both are quads — the same kind of thing as the
            // modeline's background, inside the SAME surface.
            if self.clipboard_open {
                let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
                let x = f32::from(list_columns(columns).saturating_add(1)) * cell;
                fundos.push(ronoctua_render::quad::Quad {
                    x,
                    y: f32::from(FIRST_OF_EXPANSION) * line_height,
                    width: 1.0,
                    height: grid.lines as f32 * line_height,
                    color: tokens::color::RULE_DEFAULT,
                });
                fundos.extend(qr_quads);
            }

            // THE SCROLLBAR, hidden until the mouse passes over it.
            //
            // It is not new chrome: it is a rectangle inside the SAME surface,
            // one cell wide, the same kind of thing as the modeline's
            // background. There is no box, shadow or elevation (invariant 15)
            // — it is a mark on the edge saying where you are in a list that
            // does not fit.
            //
            // Hidden by default because, with the keyboard scrolling, it is
            // second-order information: it shows up for whoever went looking
            // for it with the mouse, and does not take screen from whoever did not.
            if barra_visivel {
                let start = f32::from(FIRST_OF_EXPANSION) * line_height;
                let gap = grid.lines as f32 * line_height;
                let proporcao = grid.lines as f32 / total_expansao as f32;
                let position = offset as f32 / total_expansao as f32;
                let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
                fundos.push(ronoctua_render::quad::Quad {
                    x: width as f32 - cell,
                    y: start + gap * position,
                    width: cell,
                    // A floor of one line: with two hundred lines of content the
                    // thumb would have less than a pixel and would vanish.
                    height: (gap * proporcao).max(line_height),
                    color: tokens::color::RULE_STRONG,
                });
            }

            let Some(screen) = self.screens.get_mut(index) else {
                continue;
            };
            let (Some(gpu), Some(text)) = (screen.gpu.as_mut(), screen.text.as_mut()) else {
                continue;
            };
            if let Some(im) = screen.images.as_mut() {
                // THE FRAME'S BATCH: the lines' pixmaps and, if there is one, the
                // clipboard's preview. A single list — the same mechanics for both,
                // and not two that can diverge.
                let mut batch = pixmaps.clone();
                // THE TEXTURE ONLY GOES UP WHEN IT IS MISSING. Uploading transfers
                // megabytes in the preview's case; preparing is a `write_buffer` of
                // 16 bytes per instance. The preview's key carries the VERSION, and
                // not the dimensions: two captures of the same monitor have the same
                // size and are different images.
                if let (Some(area), Some(rgba)) = (preview, imagem_pronta) {
                    let key = format!("clip:{version}");
                    if im.missing(&key) {
                        im.upload_it(&gpu.device, &gpu.queue, &key, rgba);
                    }
                    batch.push((key, area));
                }
                for (key, _) in &pixmaps {
                    if im.missing(key)
                        && let Some(Some(rgba)) = icones_prontos.get(key)
                    {
                        im.upload_it(&gpu.device, &gpu.queue, key, rgba);
                    }
                }
                im.prepare(&gpu.queue, gpu.config.width, gpu.config.height, &batch);
            }
            if let Some(q) = screen.quads.as_mut() {
                q.prepare(
                    &gpu.device,
                    &gpu.queue,
                    gpu.config.width,
                    gpu.config.height,
                    &fundos,
                );
            }

            // Two phases, timed separately: `prepare` is text shaping plus atlas
            // upload (CPU + transfer); `frame` is the drawing, where the driver
            // compiles the pipeline the FIRST time. They are costs of a different
            // nature and are not optimized the same way.
            let t = std::time::Instant::now();
            if let Err(e) = text.prepare(
                &gpu.device,
                &gpu.queue,
                gpu.config.width,
                gpu.config.height,
                ronoctua_render::Atlas::Pinned,
                &spans,
            ) {
                tracing::error!(error = %e, screen = index, "preparar texto falhou");
                continue;
            }
            let preparo = t.elapsed();

            let t = std::time::Instant::now();
            // THE RECTANGLE BEFORE THE TEXT: with no depth test, whoever draws
            // later ends up on top. Inverted, the background would wipe the
            // whole modeline.
            let quads = screen.quads.as_ref();
            // THE IMAGE BETWEEN THE TWO: on top of the background, under the text.
            // It takes the content area, and what goes on being written over it
            // are the panel's three footer lines — which have to be legible even
            // with a light capture underneath.
            let images = screen.images.as_ref();
            if let Err(e) = gpu.frame(|pass| {
                if let Some(q) = quads {
                    q.draw(pass);
                }
                if let Some(im) = images {
                    im.draw(pass);
                }
                text.draw(ronoctua_render::Atlas::Pinned, pass)
            }) {
                tracing::error!(error = %e, screen = index, "desenhar o frame falhou");
                continue;
            }
            pintou = true;
            // Only the first frames, and only of the first screen: after that they
            // become noise on every 1 s tick.
            if primeira_pintura && index == 0 {
                tracing::info!(
                    preparo_ms = preparo.as_secs_f64() * 1000.0,
                    desenho_ms = t.elapsed().as_secs_f64() * 1000.0,
                    "partida: pintura"
                );
            }
        }

        if pintou && self.frames_painted < 3 {
            self.frames_painted += 1;
        }

        // FLUSH after presenting.
        //
        // wgpu does `attach` and `commit` on the `wl_surface`, but that only
        // queues bytes on the connection. Whoever paints from a Wayland callback
        // has the loop's own flush right after; whoever paints from ANOTHER event
        // source — niri's channel, in our case — does not. Without this, the
        // frame stays stuck in the output buffer until the compositor's next
        // event, and the modeline looks as if it does not update.
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "flush da conexao falhou");
        }
    }
}

/// How far the expansion may grow, in lines.
///
/// `altura_da_tela` is `None` when no output has announced a size yet; then
/// only the absolute ceiling holds, which is the more conservative of the two.
///
/// # Why a pure function
///
/// For the same reason as `screen_difference`: the rule is arithmetic, and
/// what wraps it — `wl_output`, `LayerSurface` — does not exist outside a
/// compositor. Separated, it is testable on screens this computer does not have.
fn expansion_ceiling(altura_da_tela: Option<u32>) -> usize {
    let line_height = tokens::metrics::line_height(tokens::font::SIZE_BASE);
    let por_tela = altura_da_tela.map_or(usize::MAX, |h| {
        let px = f64::from(h) * f64::from(tokens::space::EXPANSION_MAXFOOTERPCT) / 100.0;
        // MINUS THE TWO FIXED LINES: the percentage belongs to the whole footer,
        // and the modeline and the minibuffer are in it. Counting only the
        // expansion, the footer went past the limit the token itself names.
        ((px / f64::from(line_height)) as usize).saturating_sub(2)
    });
    // Never zero: an expansion of zero lines is the list not opening, and the
    // user would have no way of telling that from the key not having caught.
    por_tela
        .min(usize::from(tokens::space::EXPANSION_MAXLINES))
        .max(1)
}

/// The largest offset that still shows the region full.
///
/// Scrolling past it would leave blank lines at the end — and the eye would
/// read that as "the content is over", which is something else.
const fn last_offset(total: usize, ceiling: usize) -> usize {
    total.saturating_sub(ceiling)
}

/// The new offset after scrolling `step` lines.
///
/// It saturates at the extremes, and **does not wrap around**: going back to
/// the start after the end makes the eye lose its place in a list being read.
fn scrolling(current: usize, last: usize, step: i32) -> usize {
    let target = i64::try_from(current).unwrap_or(0) + i64::from(step);
    target
        .clamp(0, i64::try_from(last).unwrap_or(0))
        .try_into()
        .unwrap_or(0)
}

/// How many pages the key moves, with the `CtrlRight` held.
///
/// `None` is **ignore**, and not "pass it on": with the `CtrlRight` held the
/// person is navigating, and a letter that escaped from there would fire a
/// command in the middle of it. It is what makes it a key that does not add
/// itself to the leader.
const fn navigation_step(sym: ronoctua_keymap::Sym) -> Option<i32> {
    use ronoctua_keymap::Sym;
    match sym {
        Sym::Right => Some(1),
        Sym::Left => Some(-1),
        _ => None,
    }
}

/// Which line of the expansion is at height `y`, in pixels of the surface.
///
/// `None` over the modeline, over the minibuffer and past the last line:
/// there is no item there, and inventing one would make the message area
/// describe something that is not under the cursor.
fn row_at_height(y: f64, line_height: f64, how_many: usize) -> Option<usize> {
    if y < 0.0 || line_height <= 0.0 {
        return None;
    }
    let line = (y / line_height) as usize;
    let i = line.checked_sub(usize::from(FIRST_OF_EXPANSION))?;
    (i < how_many).then_some(i)
}

/// Which ITEM of the page is at `(x, y)`, with the grid in columns.
///
/// # Why the column enters the arithmetic
///
/// With the columns side by side, the row alone does not identify the item:
/// row 2 exists in every column. Ignoring the `x` would make the cursor
/// describe — and the click run — the first column's item, always.
fn item_at(
    x: f64,
    y: f64,
    line_height: f64,
    cell_width_px: f64,
    grid: Grid,
    quantos: usize,
) -> Option<usize> {
    let line = row_at_height(y, line_height, grid.lines)?;
    if cell_width_px <= 0.0 || x < 0.0 {
        return None;
    }
    let cell = (x / cell_width_px) as usize;
    let column = cell / usize::from(grid.width).max(1);
    if column >= grid.columns {
        return None;
    }
    let i = column * grid.lines + line;
    (i < quantos).then_some(i)
}

/// Which outputs still have no screen, and which screens lost their output.
///
/// # Why this is a pure function
///
/// The "one surface per output" decision is set arithmetic, and what wraps
/// it — `LayerSurface`, `wl_output` — does not exist outside a compositor.
/// Separated, it is testable; inside the handler, only a test with TWO
/// monitors would reach it, and the nested backend gives a single output.
///
/// Gives back `(to create, to remove)`, by index into `saidas` and `screens`.
fn screen_difference<T: PartialEq>(screens: &[T], saidas: &[T]) -> (Vec<usize>, Vec<usize>) {
    let criar = saidas
        .iter()
        .enumerate()
        .filter(|(_, s)| !screens.contains(s))
        .map(|(i, _)| i)
        .collect();
    let remove = screens
        .iter()
        .enumerate()
        .filter(|(_, t)| !saidas.contains(t))
        .map(|(i, _)| i)
        .collect();
    (criar, remove)
}

/// Each segment's color — emphasis by COLOR AND WEIGHT, never by a container
/// (section 5.1).
fn segment_color(id: &str, rodape_com_teclado: bool) -> tokens::Color {
    match id {
        // The mode indicator is what the eye looks for first.
        "mode" => tokens::color::STATE_ACCENT,
        // THE FOCUSED BUFFER, and the place the accent moves to.
        //
        // While the footer has the keyboard — leader, `M-x`, manual —, no window
        // is focused as far as the compositor is concerned, and its blue border
        // disappears. There is no asking niri to keep it: a window with no
        // keyboard is not active, and that is the truth the border tells.
        //
        // So the signal MOVES instead of vanishing: the blue leaves the window and
        // enters the line that names it. There is still exactly one blue thing on
        // screen, and it still says which buffer receives whatever comes next.
        "buffer" if rodape_com_teclado => tokens::color::STATE_ACCENT,
        "buffer" => tokens::color::TEXT_PRIMARY,
        // Secondary context: present, but it does not fight for attention.
        "context" | "position" => tokens::color::TEXT_DIM,
        _ => tokens::color::TEXT_SECONDARY,
    }
}

/// One which-key row per DISTINCT next key of a buffer's keymap.
///
/// # Why this is not `ronoctua_keymap::Tree`
///
/// That crate's `Binding` holds `&'static str`, and a buffer's bindings arrive
/// over a socket at runtime. What would be a tree is a filter over a list here,
/// because the sequences the buffers publish are two or three keys deep.
///
/// A sequence that ENDS at this depth is a command; one that continues is a
/// group, marked the same way the rest of the keymap marks its groups.
fn narrow_buffer_keymap(
    bindings: &[(String, String)],
    pending: &[&str],
) -> Vec<ronoctua_keymap::Entry> {
    let depth = pending.len();
    let mut out: Vec<ronoctua_keymap::Entry> = Vec::new();
    for (sequence, id) in bindings {
        let keys: Vec<&str> = sequence.split_whitespace().collect();
        if keys.len() <= depth || !keys.starts_with(pending) {
            continue;
        }
        let next = keys[depth];
        if out.iter().any(|e| e.key == next) {
            continue;
        }
        // `Key::parse` and not a lookup of our own: the buffer's notation has to
        // be the SAME one the rest of the keymap uses, or two spellings of one
        // key would drift apart.
        let Ok(sym) = ronoctua_keymap::Key::parse(next) else {
            tracing::warn!(key = next, "o buffer publicou uma tecla que nao existe");
            continue;
        };
        let complete = keys.len() == depth + 1;
        out.push(ronoctua_keymap::Entry {
            key: next.to_owned(),
            sym,
            // `cmd-<id>` is the label's key, as section 7.9 says. A buffer
            // command with no text of ours shows its id, which is its own
            // vocabulary and readable — never a blank row.
            label: format!("cmd-{id}"),
            command: complete.then(|| id.clone()),
            is_group: !complete,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    /// The focused buffer's keymap narrows like the rest of the which-key does.
    ///
    /// One row per distinct next key, a group when the sequence continues, a
    /// command when it ends — and NOTHING when the prefix does not match, which
    /// is what makes a key the buffer does not answer to end the sequence
    /// instead of opening an empty screen.
    #[test]
    fn the_buffer_keymap_narrows_by_prefix() {
        let bindings = vec![
            ("m f s".to_owned(), "editor-save".to_owned()),
            ("m f r".to_owned(), "editor-reload".to_owned()),
            ("m q".to_owned(), "editor-quit".to_owned()),
        ];

        let root = super::narrow_buffer_keymap(&bindings, &["m"]);
        assert_eq!(
            root.len(),
            2,
            "duas teclas distintas depois de `m`: {root:?}"
        );
        let f = root.iter().find(|e| e.key == "f").expect("sem `f`");
        assert!(f.is_group, "`m f` continua, entao e grupo");
        assert!(f.command.is_none(), "grupo nao roda nada");
        let q = root.iter().find(|e| e.key == "q").expect("sem `q`");
        assert!(!q.is_group);
        assert_eq!(q.command.as_deref(), Some("editor-quit"));
        assert_eq!(
            q.label, "cmd-editor-quit",
            "o rotulo e `cmd-<id>` (secao 7.9)"
        );

        let under_f = super::narrow_buffer_keymap(&bindings, &["m", "f"]);
        assert_eq!(under_f.len(), 2);
        assert!(
            under_f.iter().all(|e| !e.is_group),
            "`m f s` e `m f r` terminam"
        );

        assert!(
            super::narrow_buffer_keymap(&bindings, &["m", "z"]).is_empty(),
            "uma tecla que o buffer nao responde nao pode abrir tela nenhuma"
        );
    }

    /// A key the buffer publishes and this keymap cannot spell is DROPPED, and
    /// does not take the rest of the list with it.
    #[test]
    fn an_unspellable_key_does_not_lose_the_others() {
        let bindings = vec![
            ("m nao-existe-essa-tecla".to_owned(), "x".to_owned()),
            ("m s".to_owned(), "editor-save".to_owned()),
        ];
        let rows = super::narrow_buffer_keymap(&bindings, &["m"]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "s");
    }

    use super::{
        FIRST_OF_EXPANSION, Footer, Grid, Inheritance, Measure, expansion_ceiling, grid_for,
        item_at, last_offset, navigation_step, row_at_height, runs_of, screen_difference,
        scrolling, tokens,
    };

    /// A line with no icon, measuring `(key, text)`.
    const fn measurement(key: u16, text: u16) -> Measure {
        Measure {
            key,
            text,
            icon: false,
        }
    }

    #[test]
    fn caps_lock_does_not_decide_a_shortcut() {
        // With the Lock on, xkb delivers the UPPERCASE where the lowercase was
        // typed. Undoing that is the only way for `LEADER i` to go on being
        // `LEADER i`.
        assert_eq!(Footer::without_caps_lock('I', true), 'i');
        // And the other way round: with Shift along, xkb has already given back
        // the lowercase, and without inverting here too `w H` would stop existing
        // with the Lock on. The Lock is undone both ways or in neither.
        assert_eq!(Footer::without_caps_lock('i', true), 'I');
        // With no Lock nothing changes — the Shift STILL tells `h` from `H`,
        // which is the distinction the keymap wants.
        assert_eq!(Footer::without_caps_lock('I', false), 'I');
        assert_eq!(Footer::without_caps_lock('i', false), 'i');
        // A digit and punctuation have no case: `TAB 3` and `TAB ]` must not
        // become another key because of a keyboard state.
        assert_eq!(Footer::without_caps_lock('3', true), '3');
        assert_eq!(Footer::without_caps_lock(']', true), ']');
    }

    #[test]
    fn the_wheel_reads_the_three_axis_sources() {
        use super::axis_detents;
        use smithay_client_toolkit::seat::pointer::AxisScroll;
        let line = 20.0;

        // TODAY'S: 120ths of a detent. Half a turn is half a detent, and not
        // zero — it is what makes a high-resolution wheel not lose the step.
        let alta = AxisScroll {
            value120: 60,
            ..AxisScroll::default()
        };
        assert!((axis_detents(&alta, line) - 0.5).abs() < 1e-9);

        // THE OLD ONE, from an older compositor: an integer per detent.
        let velho = AxisScroll {
            discrete: -2,
            ..AxisScroll::default()
        };
        assert!((axis_detents(&velho, line) + 2.0).abs() < 1e-9);

        // THE CONTINUOUS one from the touchpad: finger pixels, converted by the
        // LINE HEIGHT. Three lines of finger = one detent.
        let dedo = AxisScroll {
            absolute: line * 3.0,
            ..AxisScroll::default()
        };
        assert!((axis_detents(&dedo, line) - 1.0).abs() < 1e-9);

        // AND A SMALL EVENT IS NOT ZERO. Truncating here would make the touchpad
        // never scroll: each of its events is worth a fraction, and what adds
        // them up is the accumulator.
        let migalha = AxisScroll {
            absolute: 2.0,
            ..AxisScroll::default()
        };
        assert!(axis_detents(&migalha, line) > 0.0);

        // With no line height there is no converting: zero, and not infinity.
        assert_eq!(axis_detents(&dedo, 0.0), 0.0);
    }

    #[test]
    fn the_modifier_that_opened_the_leader_does_not_count() {
        use smithay_client_toolkit::seat::keyboard::Modifiers;
        let alt = Modifiers {
            alt: true,
            ..Modifiers::default()
        };

        // THE CASE FROM THE SCREEN: `CTRL ALT SPC M-n is not bound`. In the nested
        // session the leader is `Ctrl+Alt+Space`, so Ctrl and Alt are held when the sequence
        // starts. Inherited, it must not show up in the key.
        let mut h = Inheritance::default();
        h.on_focus();
        h.on_announcement(alt);
        assert!(
            !h.discount(alt).alt,
            "o Alt que abriu a leader nao pode entrar na sequencia"
        );

        // With no inheritance recorded, whatever is held HOLDS — that is the case
        // of a actual `M-…` binding, if one ever exists.
        assert!(Inheritance::default().discount(alt).alt);

        // And what was pressed AFTERWARDS still counts: an inheritance of Ctrl
        // alone does not wipe a new Alt.
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::default()
        };
        let both = Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::default()
        };
        let mut h = Inheritance::default();
        h.on_announcement(ctrl);
        let m = h.discount(both);
        assert!(!m.ctrl, "o Ctrl herdado sai");
        assert!(m.alt, "o Alt apertado depois FICA");
    }

    #[test]
    fn the_leader_recaptures_the_inheritance_with_no_new_focus() {
        use smithay_client_toolkit::seat::keyboard::Modifiers;
        let nothing = Modifiers::default();
        let alt = Modifiers {
            alt: true,
            ..Modifiers::default()
        };

        // THE HOLE THIS CLOSES, which the hung session showed to be actual: the
        // nested compositor can stop dispatching altogether — it did not even
        // answer `wl_display.sync`, with seven connections without `accept` and
        // 1536 bytes from the footer stuck in the write queue. A `leave` lost in
        // that hole never arrives, and the previous session's inheritance keeps
        // holding forever.
        let mut h = Inheritance::default();
        h.on_focus();
        h.on_announcement(nothing); // the session started with nothing held
        // ... and now an Alt gets STUCK: the compositor announces that it is
        // down, and the release never arrives.
        h.on_announcement(alt); // changes nothing: the inheritance has already been taken
        assert!(
            h.discount(alt).alt,
            "sem recaptura, o Alt preso entra em toda tecla — e nada casa"
        );

        // WITH THE RECAPTURE ON STARTING THE SEQUENCE, it goes away.
        h.on_sequence_start(alt);
        assert!(
            !h.discount(alt).alt,
            "o que ja estava segurado quando a sequencia comecou nao e dela"
        );

        // And a Ctrl pressed AFTER the recapture still counts.
        let alt_e_ctrl = Modifiers {
            alt: true,
            ctrl: true,
            ..Modifiers::default()
        };
        assert!(h.discount(alt_e_ctrl).ctrl);
    }

    #[test]
    fn with_caps_lock_the_icon_group_still_opens() {
        // THE CRITERION, and not the function. This is literally the screen the
        // owner sent: `CTRL ALT SPC I is not bound`. With the Lock on xkb delivers
        // `I`, `LEADER I` is bound to nothing, and EVERY lowercase binding of
        // the keymap dies with it.
        use ronoctua_keymap::{Key, Resolution, Sym};
        let (tree, _) = crate::leader::default_of().expect("o keymap padrao tem de fechar");

        assert!(
            matches!(
                tree.resolve(&[Key::simple(Sym::Char('I'))]),
                Resolution::Nothing
            ),
            "se `I` passar a ser um atalho, este teste perdeu o sentido"
        );

        let c = Footer::without_caps_lock('I', true);
        assert!(
            matches!(
                tree.resolve(&[Key::simple(Sym::Char(c))]),
                Resolution::Prefix(_)
            ),
            "com Caps Lock ligado, `LEADER i` tem de abrir o grupo de icones"
        );
    }

    #[test]
    fn each_output_gets_a_screen() {
        // The case the nested backend cannot reach: two monitors. The decision
        // of section 9.2 is one surface PER OUTPUT.
        let (criar, remove) = screen_difference::<u32>(&[], &[1, 2]);
        assert_eq!(criar, vec![0, 1], "os dois outputs precisam de tela");
        assert!(remove.is_empty());
    }

    #[test]
    fn an_output_already_covered_gets_no_second_screen() {
        // `new_output` and `update_output` call the same routine, and SCTK fires
        // both. Without this guard the same monitor would get two stacked
        // surfaces, and the exclusive zone would double.
        let (criar, remove) = screen_difference(&[1, 2], &[1, 2]);
        assert!(criar.is_empty());
        assert!(remove.is_empty());
    }

    #[test]
    fn hotplug_removes_the_screen_of_the_monitor_that_left() {
        let (criar, remove) = screen_difference(&[1, 2], &[1]);
        assert!(criar.is_empty());
        assert_eq!(remove, vec![1], "a tela do output 2 tem de sair");
    }

    #[test]
    fn swapping_monitors_creates_and_removes_in_the_same_pass() {
        // Unplugging one and plugging another between two events is common on a
        // dock. Both things have to happen, not only the first.
        let (criar, remove) = screen_difference(&[1], &[2]);
        assert_eq!(criar, vec![0]);
        assert_eq!(remove, vec![0]);
    }

    #[test]
    fn the_expansion_never_takes_the_screen() {
        // INVARIANT 14 as a number: the footer has to go on being a footer. On a
        // short screen, the absolute ceiling of 12 lines on its own would go past
        // half of it — and the list would stop having content behind it.
        let line_height = f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE));
        for height in [600_u32, 720, 800, 900, 1080, 1440, 2160] {
            let lines = expansion_ceiling(Some(height));
            // THE WHOLE FOOTER: the two fixed lines count for whoever is looking.
            let px = (lines + 2) as f64 * line_height;
            let limit =
                f64::from(height) * f64::from(tokens::space::EXPANSION_MAXFOOTERPCT) / 100.0;
            assert!(
                px <= limit,
                "com {height} px de tela o rodape pega {px} px, mais que os \
                 {limit} px que o token permite"
            );
            assert!(lines >= 1, "expansao de zero linhas com {height} px");
        }
    }

    #[test]
    fn the_absolute_ceiling_holds_on_the_big_screen_and_the_percentage_on_the_small_one() {
        // Both limits exist because each one fails on its own: on a 2160 px
        // screen, 40% would give 33 lines (the footer would become the screen);
        // on a 600 px one, 12 lines would give more than half.
        assert_eq!(
            expansion_ceiling(Some(2160)),
            usize::from(tokens::space::EXPANSION_MAXLINES),
            "na tela grande quem limita e o teto absoluto"
        );
        assert!(
            expansion_ceiling(Some(600)) < usize::from(tokens::space::EXPANSION_MAXLINES),
            "na tela pequena quem limita e o percentual"
        );
        // Node output yet: the absolute one holds, which is the more conservative.
        assert_eq!(
            expansion_ceiling(None),
            usize::from(tokens::space::EXPANSION_MAXLINES)
        );
    }

    #[test]
    fn the_scrolling_stops_at_the_ends_and_does_not_wrap() {
        // Wrapping around makes the eye lose its place: reaching the end and going
        // back to the top is indistinguishable from the list having changed.
        let last = last_offset(30, 12); // 18
        assert_eq!(last, 18);
        assert_eq!(scrolling(0, last, -1), 0, "subiu do topo");
        assert_eq!(scrolling(last, last, 1), last, "passou do fim");
        assert_eq!(scrolling(0, last, 12), 12);
        assert_eq!(scrolling(12, last, 12), 18, "a ultima pagina para no fim");
    }

    #[test]
    fn content_that_fits_does_not_scroll() {
        // With everything visible there is nothing to scroll, and the bar does not
        // show up. A bar of full height says nothing.
        assert_eq!(last_offset(5, 12), 0);
        assert_eq!(scrolling(0, last_offset(5, 12), 1), 0);
    }

    #[test]
    fn the_colons_stay_in_the_same_column_with_keys_of_different_sizes() {
        // THE REQUEST: `RET` and `a` in the same column of the grid, and the
        // colons aligned. The arithmetic is over the WHOLE list, so it is the
        // short key that moves away from the margin — not the colons that move.
        let g = grid_for(
            &[measurement(3, 6), measurement(1, 4), measurement(5, 8)],
            200,
            12,
            true,
        );
        assert_eq!(g.colon, 5 + tokens::space::MODELINE_EDGELEFT + 1);
        for width in [1_u16, 3, 5] {
            let start = g.colon - (width + 1);
            assert_eq!(start + width + 1, g.colon);
        }
        // And the widest one starts exactly at the margin — with no dead gap.
        assert_eq!(g.colon - (5 + 1), tokens::space::MODELINE_EDGELEFT);
    }

    #[test]
    fn the_shortcut_list_uses_the_width_before_the_height() {
        // DOOM'S ARRANGEMENT. Ten shortcuts in a region of twelve lines would fit
        // in a single column — a narrow strip with the whole screen empty beside
        // it, and still paginating once they went past twelve. Using the columns
        // that fit, the same ten become a few lines.
        let items: Vec<Measure> = std::iter::repeat_n(measurement(3, 8), 10).collect();
        let g = grid_for(&items, 200, 12, true);
        assert!(g.columns > 1, "ficou em coluna unica numa tela larga");
        assert!(
            g.lines < 10,
            "usou {} linhas para 10 itens: nao aproveitou a largura",
            g.lines
        );
        assert!(g.per_page() >= 10, "dez itens deviam caber numa pagina");
    }

    #[test]
    fn whatever_scrolls_stays_in_a_single_column() {
        // The manual and the finder are LISTS: in columns, running text would
        // have to be read going down and back to the top — which is the opposite
        // of reading.
        let items: Vec<Measure> = std::iter::repeat_n(measurement(3, 8), 30).collect();
        let g = grid_for(&items, 200, 12, false);
        assert_eq!(g.columns, 1);
        assert_eq!(g.lines, 12, "a lista devia encher o teto");
    }

    #[test]
    fn the_grid_does_not_overflow_the_height_ceiling() {
        // Invariant 14 as a number: even with many items and a narrow screen, the
        // region stops at the ceiling.
        let items: Vec<Measure> = std::iter::repeat_n(measurement(3, 8), 500).collect();
        for width in [40_u16, 80, 200, 400] {
            let g = grid_for(&items, width, 12, true);
            assert!(g.lines <= 12, "{width} colunas deram {} linhas", g.lines);
            assert!(g.columns >= 1);
        }
    }

    /// THE DEFECT: until now NOTHING in the expanded region elided.
    ///
    /// A description of two hundred cells went over the scrollbar and out past
    /// the edge — and the compositor **does not clip**: it simply did not show
    /// up, with nothing saying that there was more.
    ///
    /// The property holds for EVERY column of the grid, and it is the last one
    /// that matters: with arithmetic that is right only for the first, the
    /// shortcut list would be correct and the third block would go out to the right.
    #[test]
    fn nothing_from_the_right_column_leaks_off_the_screen() {
        // A REALLY LONG DESCRIPTION. `Comment` is prose, and two hundred cells
        // are a common desktop-entry sentence; with a text that already fits,
        // this test would pass without the limits even existing.
        let items: Vec<Measure> = std::iter::repeat_n(measurement(6, 200), 40).collect();
        for colunas_da_tela in [40_u16, 80, 120, 176, 300] {
            let g = grid_for(&items, colunas_da_tela, 12, true);
            for i in 0..g.per_page() {
                let (base, _) = g.position(i);
                for with_key in [true, false] {
                    let end = base
                        + g.text_start(with_key)
                        + g.fits_in_text(with_key, base, colunas_da_tela);
                    assert!(
                        end <= colunas_da_tela,
                        "com {colunas_da_tela} colunas, o item {i} termina em {end}"
                    );
                }
            }
        }
    }

    /// And it does not invade the NEXT column of the grid.
    ///
    /// The other side of the same limit: fitting on the screen is not enough
    /// when there are three columns side by side — the first one's text would
    /// write over the second one's name, and both would still be inside the edge.
    #[test]
    fn the_right_column_does_not_invade_the_next_one() {
        let items: Vec<Measure> = std::iter::repeat_n(measurement(6, 30), 40).collect();
        let g = grid_for(&items, 300, 12, true);
        assert!(g.columns > 1, "a tela larga devia dar mais de uma coluna");
        for with_key in [true, false] {
            let used = g.text_start(with_key) + g.fits_in_text(with_key, 0, 300);
            assert!(
                used <= g.width,
                "o texto usa {used} de uma coluna de {}",
                g.width
            );
        }
    }

    #[test]
    fn an_empty_list_does_not_break_the_grid() {
        // `max()` of nothing is `None`. Without the `unwrap_or`s, the width would
        // become zero and the division by it would panic.
        let g = grid_for(&[], 80, 12, true);
        assert_eq!(g.columns, 1);
        assert_eq!(g.lines, 1);
        assert!(g.width > 0, "largura zero divide por zero adiante");
    }

    #[test]
    fn the_item_under_the_cursor_depends_on_the_column_too() {
        // With the columns side by side, the row alone does not identify the item:
        // row 2 exists in every column. Ignoring the `x` would make the cursor
        // describe — and the click RUN — the first column's item, always.
        let g = Grid {
            columns: 3,
            lines: 4,
            width: 20,
            colon: 5,
            icon: 0,
        };
        let (h, c) = (26.0, 10.0);
        let y0 = f64::from(FIRST_OF_EXPANSION) * h;
        // First column, first row.
        assert_eq!(item_at(0.0, y0, h, c, g, 12), Some(0));
        // First column, third row.
        assert_eq!(item_at(0.0, y0 + 2.0 * h, h, c, g, 12), Some(2));
        // SECOND column, first row: the fifth item, and not the first.
        assert_eq!(item_at(20.0 * c, y0, h, c, g, 12), Some(4));
        // Third column, last row.
        assert_eq!(item_at(40.0 * c, y0 + 3.0 * h, h, c, g, 12), Some(11));
        // Past the last column: there is no item there.
        assert_eq!(item_at(80.0 * c, y0, h, c, g, 12), None);
        // And past the end of the list, even inside the grid.
        assert_eq!(item_at(40.0 * c, y0 + 3.0 * h, h, c, g, 5), None);
    }

    #[test]
    fn the_cursor_only_points_at_a_row_that_exists() {
        // The modeline and the minibuffer are NOT items. Giving back `0` for them
        // would make the message area describe the list's first line while the
        // pointer is over the buffer's name.
        let h = 26.0;
        let n = 4;
        assert_eq!(row_at_height(0.0, h, n), None, "modeline virou item");
        assert_eq!(row_at_height(h + 1.0, h, n), None, "minibuffer virou item");
        assert_eq!(
            row_at_height(f64::from(FIRST_OF_EXPANSION) * h, h, n),
            Some(0)
        );
        assert_eq!(
            row_at_height((f64::from(FIRST_OF_EXPANSION) + 3.0) * h, h, n),
            Some(3)
        );
        // Past the list's last line: there is no item there.
        assert_eq!(
            row_at_height((f64::from(FIRST_OF_EXPANSION) + 4.0) * h, h, n),
            None
        );
        // And with the region closed, nothing is pointable.
        assert_eq!(row_at_height(100.0, h, 0), None);
    }

    #[test]
    fn the_matched_letters_come_out_in_a_run_of_their_own() {
        // THE HIGHLIGHT IS WHAT EXPLAINS THE LIST. With subsequence search, `jme`
        // matching `window-move-left` only makes sense when the three letters
        // light up — without that, the filtered list looks arbitrary.
        //
        // One run per CHANGE of color, and not one per character: on a list of 170
        // items, shaping letter by letter would multiply the renderer's work by
        // twenty.
        let c = runs_of("janela", &[0, 1]);
        assert_eq!(
            c,
            vec![("ja".to_owned(), 0, true), ("nela".to_owned(), 2, false)]
        );

        // The columns are the character's INDEX: one cell each, the same ruler as
        // `layout::width`. With two rulers, the highlight would come out offset
        // from the text it lights.
        let c = runs_of("abcde", &[1, 3]);
        assert_eq!(
            c,
            vec![
                ("a".to_owned(), 0, false),
                ("b".to_owned(), 1, true),
                ("c".to_owned(), 2, false),
                ("d".to_owned(), 3, true),
                ("e".to_owned(), 4, false),
            ]
        );
    }

    #[test]
    fn with_no_search_the_text_comes_out_whole_in_a_single_run() {
        // The common path is the list with no filter. It must not pay the price of
        // one `Span` per character.
        assert_eq!(
            runs_of("buffer-close", &[]),
            vec![("buffer-close".to_owned(), 0, false)]
        );
        assert!(runs_of("", &[]).is_empty());
    }

    #[test]
    fn the_right_ctrl_only_pages_with_the_side_arrows() {
        // THE REQUEST: a combination key that does NOT add itself to the leader. If
        // a letter escaped through here, it would become a sequence — and the
        // `CtrlRight` would stop being what it exists to be.
        use ronoctua_keymap::Sym;
        assert_eq!(navigation_step(Sym::Right), Some(1));
        assert_eq!(navigation_step(Sym::Left), Some(-1));
        for sym in [
            Sym::Char('a'),
            Sym::Char('q'),
            Sym::Enter,
            Sym::Esc,
            Sym::Up,
            Sym::Down,
            Sym::Space,
            Sym::Tab,
            Sym::Backspace,
        ] {
            assert_eq!(
                navigation_step(sym),
                None,
                "{sym:?} nao devia fazer nada com o CtrlRight segurado"
            );
        }
    }
}
