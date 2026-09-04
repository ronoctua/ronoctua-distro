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
use ronoctua_transfers::where_to::Wish;
use ronoctua_transfers::{Identity, Want};

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
/// The six that exist: `RONOCTUA_BUFFER_GLYPHS` (`LEADER i n`), `_EDITOR`
/// (`LEADER f f`), `_UNICODE` (`i u`), `_LUCIDE`, `_MEMORY` and `_TRANSFERS`.
/// The last three carry something the others do not: Lucide (`i l`) also
/// depends on a FACE — the `RONOCTUA_FONT_LUCIDE` embedded in the renderer, and
/// the two come out of the same derivation, without which the 1,668 icons would
/// be tofu —, and the memory panel (`LEADER s m`) and the queue (`LEADER t w`)
/// embed no database at all: everything they draw is read by their host at that
/// moment and pushed in, which is what `capability::system` and
/// `capability::transfers` exist for.
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
    /// A paint is owed, and it happens once the queue is empty.
    ///
    /// # Why the pointer does not paint where it is handled
    ///
    /// Because the surface presents in `Fifo`: `get_current_texture` waits for
    /// the compositor to hand a buffer back, which is up to a whole refresh.
    /// Painting inside the event handler meant paying that wait once per
    /// pointer frame — and a fast sweep produces a burst of them, so the
    /// handler blocked while more motion piled up in the socket. Every paint
    /// then drew a position the pointer had already left, and the highlight
    /// visibly trailed the mouse. It got worse the faster one moved, which is
    /// exactly how it was reported.
    ///
    /// Marking instead of painting collapses the burst: `calloop` dispatches
    /// everything that is pending, and the callback paints ONCE, with the
    /// latest position. The wait is still there — it is what `Fifo` is for —
    /// but it is paid once per frame instead of once per event, and never with
    /// stale input behind it.
    owes_paint: bool,

    /// Is the locator currently picking a FILE, and not a buffer?
    ///
    /// The same list serves both, so what tells them apart is this — and it is
    /// a field and not a guess from the id, because a path and a window id are
    /// both just strings and confusing them would open a window numbered after
    /// a filename.

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
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
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
    /// The catalogue, the distro's filters and the person's.
    ///
    /// It decides how every notification appears. Read once at startup: the
    /// distro's half is generated and cannot change while the shell runs, and
    /// the person's half is written by the manager, which is right here.
    book: ronoctua_notify::Book,
    /// Is the manager open? It uses THIS surface, growing below the minibuffer —
    /// never a second one (invariant 14).
    notifications_open: bool,
    /// The manager's cursor and panes. It holds no records — see
    /// [`crate::notifications`].
    manager: crate::notifications::View,
    /// The half-second phase of the blink, ridden by the clock that expires.
    ///
    /// One bool for the whole footer and not one per notification: only one is
    /// on the band at a time, which is the whole shape of section 4.2.
    blink_on: bool,
    /// Which frame of the update's mark is on the modeline.
    ///
    /// It rides the SAME one-second clock as `blink_on`, for the reason that
    /// clock's own comment gives: a second timer for the same second would
    /// wake the loop twice for one effect. The hand goes round once a minute —
    /// see [`crate::update_window::frame_of`].
    update_tick: u64,
    /// The band, as the LAST FRAME laid it out — for the pointer.
    ///
    /// Remembered and not recomputed for the same reason the modeline's
    /// segments are: a `Motion` arrives per pixel, and what the pointer is over
    /// has to be what was DRAWN rather than what a second calculation says
    /// would be drawn.
    band_placed: Vec<crate::band::Line>,
    /// The last mind the brain pushed, KEPT with nothing on screen.
    ///
    /// `None` until the daemon answers for the first time — which is a
    /// different thing from "there is nothing to do", and the line draws only
    /// on the second.
    brain: Option<ronoctua_brain::Mind>,
    /// The brain's line, as the LAST FRAME laid it out — for the pointer.
    ///
    /// Remembered and not recomputed, for the reason `band_placed` is: what the
    /// pointer is over has to be what was DRAWN.
    brain_placed: Option<crate::brain::Line>,
    /// What the brain line's control under the pointer says, for the message
    /// line.
    brain_hint: Option<String>,
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
    /// The transfer queue's panel, when it is open. `None` = it is not.
    transfers: Option<crate::transfers::View>,
    /// The last list the queue sent, KEPT even with the panel closed.
    ///
    /// The modeline's segment is drawn from it, and the modeline is always on
    /// screen. Throwing the list away when the panel closes would make the
    /// percentage disappear the moment somebody stopped looking at the panel —
    /// which is precisely when they want it in the corner instead.
    jobs: Vec<ronoctua_transfers::Job>,
    /// Which state each open buffer window is in, by its process id.
    ///
    /// # Why a map, and not just the focused one
    ///
    /// Because a buffer reports when it CHANGES, not when it is looked at. With
    /// only the focused one kept, Alt-Tabbing away and back would show whatever
    /// the last window reported until the next keystroke — the modeline naming
    /// one buffer and describing another.
    ///
    /// It is pruned against the compositor's window list, so it holds at most
    /// one entry per open window. See [`Footer::states_arrived`].
    states: std::collections::HashMap<u32, String>,
    /// `SPC b s` is open and the next key picks a state.
    ///
    /// A flag and not a pending sequence, because the level is exactly ONE deep:
    /// a buffer declares a handful of states and each has a letter.
    state_pending: bool,
    /// The modeline's segments as the LAST FRAME placed them: id, first column,
    /// width in columns.
    ///
    /// # Why remembered, and not recomputed for the pointer
    ///
    /// Because a `Motion` event arrives per pixel of movement, and resolving the
    /// layout builds a `String` per segment. It is the same lesson `row_at`
    /// already carries a comment about — and it is also the only version that
    /// cannot be wrong: what the pointer is over is what was DRAWN, not what a
    /// second calculation says would be drawn.
    modeline_placed: Vec<(&'static str, u16, u16)>,
    /// Which segment the pointer is over.
    segment_under_pointer: Option<&'static str>,
    /// What the band's control under the pointer says, already translated.
    ///
    /// On the MESSAGE LINE and never in a tooltip: a floating surface is what
    /// invariant 14 forbids, and the file manager set the precedent — the
    /// explanation of what is under the pointer beats the line being typed.
    band_hint: Option<String>,
    /// The focused buffer's declared states, read when the leader opened.
    ///
    /// Read there and not per frame: it is a socket round trip, and the answer
    /// cannot change between opening the leader and pressing the next key —
    /// the buffer is not receiving keys while the footer holds the keyboard.
    state_menu: Vec<(String, char)>,
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
    /// Which of the manager's sections has the thumb, when one does.
    ///
    /// The listing's bar is this door's own (`dragging`); the sidebar's and the
    /// facts' belong to the view, and until this existed they were drawn and
    /// nothing listened — the owner tried the facts' with the mouse in both
    /// doors and neither moved.
    dragging_section: Option<(crate::files::Pane, crate::files::Axis)>,
    /// Which line of the visible PAGE is under the pointer.
    ///
    /// An index into the page, and not into the whole list: it is what the drawing
    /// positions, and converting here would avoid a second scroll calculation.
    row_under_pointer: Option<usize>,
    /// Which CELL of that row, counting from the region's left edge.
    ///
    /// Only the file manager needs it, and it needs it badly: ten of its cells
    /// are separate permission bits, and a row index alone cannot tell the
    /// group's `w` from the owner's `r`. It is what makes both the explanation
    /// on hover and the click on a bit possible at all.
    cell_under_pointer: Option<u16>,
    /// Is the shortcut manual open?
    ///
    /// It is the fourth use of the expanded region, and the second MODAL one:
    /// while it is open the keyboard is its, because the arrows scroll. Keeping a
    /// boolean and not the finished lines is deliberate — the lines are assembled
    /// from `actions::chords()` on every draw, and a stored copy would be the
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
    /// Where the notifications persist — the two directories, found ONCE.
    ///
    /// Carried and not looked up per write, for the reason `sidebar::Sidebar`
    /// carries its own: a function that finds `$XDG_CONFIG_HOME` on its own is
    /// a function `cargo test` can reach, and the sidebar's tests reached one.
    notes: ronoctua_notify::store::Where,
    /// Is the disk-watching thread up? It is raised at most once, on the first
    /// `LEADER .`, and it outlives every open — a stick plugged in with the
    /// manager closed has to be there when it opens.
    disks_watching: bool,
    /// The directory on screen reporting itself.
    ///
    /// Raised at most once, on the first `LEADER .`, and it outlives every open
    /// for the same reason `disks_watching` does. Closing the manager RESTS it
    /// rather than dropping it: a watch nobody is touching costs nothing, and
    /// tearing the thread down to raise it again on the next open would be
    /// churn with something to get wrong in between.
    files_notice: Option<ronoctua_files::notice::Notice>,
    /// The last reading of the machine's disks.
    ///
    /// Kept even with no view on screen, so opening the manager draws the
    /// sticks that are already in rather than an empty list that fills in a
    /// moment later.
    rack: Option<Box<ronoctua_disks::Rack>>,

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

    // --- file manager (`LEADER .`) ---
    /// The file manager, when open. Same design, same reason.
    ///
    /// It holds MORE than the others: a directory one navigated to, a set of
    /// marks, a filter, an armed confirmation. That is why it wins the
    /// precedence in `view()` — the rest can be reopened in a keystroke.
    files: Option<crate::files::View>,
    /// Where the manager's threads hand their result back.
    files_sender: Option<calloop::channel::Sender<crate::files::Report>>,
    /// Where an ask for a preview and its facts is posted.
    ///
    /// Raised with the channel and not per open: the thread sleeps on a
    /// blocking `recv` while nobody asks, which costs nothing, and raising it
    /// per open would leave one running per open. `None` when it could not be
    /// raised — the panels then draw the type's glyph, which is what they draw
    /// for a file that has no picture anyway.
    files_looking: Option<std::sync::mpsc::Sender<crate::files::Ask>>,
    /// The thread that follows a moving file, when there is one to follow.
    files_playing: Option<crate::motion::Player>,
    /// The loop, so a keyboard can be made that REPEATS.
    ///
    /// `Option` because the seat may announce itself before the loop exists —
    /// it does not any more, and the fallback is still here rather than a
    /// `panic` on an ordering nobody would think to check.
    repeat: Option<calloop::LoopHandle<'static, Self>>,
    /// The thread that draws a shape at the angle it is being looked at.
    files_turning: Option<crate::stage::Stage>,
    /// Where the pointer was when it last moved a shape.
    ///
    /// A gesture of the door's own and NOT a fourth `Axis`: `drag_to` inverts
    /// a track — one scalar, one direction — and turning something is two
    /// deltas with no track behind them. Beside `dragging_section` because it
    /// is the same kind of thing: a press that owns the pointer until it is
    /// let go.
    spinning: Option<(f64, f64)>,
    /// Who the numeric ids belong to, read ONCE.
    ///
    /// `/etc/passwd` and `/etc/group` do not change while a listing is on
    /// screen, and re-reading them per row would be two hundred opens inside
    /// the frame that draws.
    roster: ronoctua_files::who::Roster,
    /// How far ahead of UTC this machine is.
    ///
    /// Read once from `/etc/localtime`. When it could not be read it says so,
    /// and the date column is labelled UTC rather than showing a local time
    /// nobody can vouch for.
    zone: ronoctua_files::when::Zone,

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
/// The precedence order between the views was written in FOUR places — the
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
    /// The file manager. Modal.
    Files,
    /// The clipboard history. Modal.
    Clipboard,
    /// The transfer queue's panel. Modal.
    ///
    /// Next to the clipboard rather than next to the file manager, and the
    /// reason is what each one HOLDS: the manager sits below the locator
    /// because it carries a directory somebody navigated to, a set of marks and
    /// a half-answered question. This panel carries a cursor. Reopening it
    /// costs one keystroke and loses nothing.
    Transfers,
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
    /// The notification manager. Modal.
    ///
    /// It used to be the history, and it used to be passive — a list one read,
    /// which gave the keys to whoever wanted them. The manager has a cursor,
    /// three panes and a sheet, and a cursor nobody can move is a view only the
    /// mouse reaches.
    Notifications,
    /// The region is closed.
    NoneOf,
}

impl ViewState {
    /// Does the view have the whole keyboard?
    ///
    /// The leader also holds the keyboard — it is a sequence under way. Only
    /// the closed region does not.
    const fn grabs(self) -> bool {
        !matches!(self, Self::NoneOf)
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
    /// The file manager's row, when this line is one.
    ///
    /// # Why it hangs off the same struct
    ///
    /// Because `expansion()` is the SINGLE list that feeds both the height and
    /// the drawing, and a second list for a second kind of row would be the
    /// divergence this enum's own doc describes — the footer asking for
    /// twenty-two lines to draw six. A `Vec` that is empty for every other
    /// view costs a pointer per line and keeps one list.
    ///
    /// When it is non-empty the two columns above are not drawn: a listing has
    /// seven columns with a hit target each, which is exactly what `key`/`text`
    /// cannot express.
    fields: Vec<crate::files::Field>,
    /// Whether this line is anchored to the surface's BOTTOM edge.
    ///
    /// The film's transport, and nothing else — see `files::Row::foot`. A
    /// surface is not a whole number of lines, and the leftover under the last
    /// one is where the owner found the controls sitting high.
    foot: bool,
}

/// How long a message stays on screen — BOTH doors.
///
/// Three seconds: time to read a short line without it becoming permanent
/// noise. In this door the one-second timer that expires a notification wipes
/// it too, so there is no second clock; the window door has no clock at all and
/// arms a one-shot per message (see `FilesWindow::say`). Two mechanisms, and
/// they have to agree about the number or "how long is a message on screen"
/// would have two answers.
pub(crate) const ECHO_SECONDS: u64 = 3;

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
    /// A cell of the file manager's PATH line.
    ///
    /// No index: which crumb comes from the cell, exactly as the header's
    /// columns do. The row alone would say "the path", which is not a place.
    FilesPath,
    /// A cell of the file manager's listing.
    ///
    /// No index, and it USED to carry one. The row alone is not enough here —
    /// a listing row has seven columns and ten of its cells are separate
    /// permission bits — so both buttons resolve the point through
    /// `files::View::hit` and this variant only says WHOSE line it is.
    ///
    /// The index that was here could not be trusted, and one reader believed
    /// it: `Footer::visible_row` asks for a window of ONE line, and a
    /// one-line window of the manager is its chrome, so the number it carried
    /// described the path line no matter which file was under the pointer.
    /// Nothing reads it now, and nothing can.
    Files,
    /// A cell of the manager's bottom STRIP — the two panels, side by side.
    ///
    /// No index, for the reason `FilesPath` has none: the strip is not part of
    /// the list, does not scroll with it and has a line numbering of its own,
    /// so an index here would be an index into something else. WHICH line and
    /// WHICH cell come from `row_under_pointer` and `cell_under_pointer`,
    /// recorded by the same press.
    ///
    /// It carries no new `Hit`: clicking a picture of a file opens it, exactly
    /// as clicking its name does, and a fact answers with a sentence and
    /// nothing more.
    FilesStrip,
    /// Put the focus on line `n` of the transfer panel.
    ///
    /// Focus and nothing else. Every action in that panel is destructive in the
    /// small — giving a copy up, taking a row away — and a single click that
    /// carried one out would be the gesture nobody meant. The keys act; the
    /// mouse aims.
    Transfers(usize),
    /// Put the focus on line `n` of the notification manager.
    ///
    /// FOCUS and not act, for the same reason the transfer panel gives: every
    /// action up there — marking read, editing a capture, adjusting a filter —
    /// is a thing one meant to do. The keys act; the mouse aims.
    Notifications(usize),
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
            fields: Vec::new(),
            foot: false,
        }
    }

    /// How much this line measures, for the grid.
    fn measurement(&self) -> Measure {
        // A file manager row already knows its own columns — it resolved them
        // against the screen's width before it existed. Reporting a key and a
        // text width would make `grid_for` try to lay it out a second time,
        // with a different answer.
        if !self.fields.is_empty() {
            return Measure {
                key: 0,
                text: u16::MAX,
                icon: false,
            };
        }
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
    for (i, c) in layout::cells_of(text).enumerate() {
        let matched = highlights.contains(&i);
        match out.last_mut() {
            // Joins the previous run when the color does not change.
            Some((s, _, previous)) if *previous == matched => s.push_str(c),
            // ONE CELL PER CLUSTER, which is what `layout::width` counts and
            // what `elide_end` walks. The two rulers have to be the SAME: this
            // used to say "one cell per character" and to iterate `chars()`,
            // and the claim was simply false — a combining accent is two
            // characters and one cell, so the highlight walked ahead of the
            // text it was supposed to light. `match_up` hands over cluster
            // indices for the same reason.
            _ => out.push((c.to_owned(), u16::try_from(i).unwrap_or(u16::MAX), matched)),
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
    for raw in text.lines() {
        let mut line = String::new();
        for word in raw.split_whitespace() {
            if line.is_empty() && usize::from(layout::width(word)) > width {
                // WORD LONGER THAN THE LINE: split it into pieces the size of the
                // line. By CLUSTER, and not by character: this is a cut, and a
                // cut between the two halves of an emoji sequence does not
                // shorten the word, it draws a different one — the same fault
                // `elide_end` exists to avoid.
                let mut rest: Vec<&str> = layout::cells_of(word).collect();
                while rest.len() > width && out.len() < ceiling {
                    out.push(rest.drain(..width).collect());
                }
                line = rest.into_iter().collect();
                continue;
            }
            let fits =
                usize::from(layout::width(&line)) + 1 + usize::from(layout::width(word)) <= width;
            if line.is_empty() {
                line.push_str(word);
            } else if fits {
                line.push(' ');
                line.push_str(word);
            } else {
                out.push(std::mem::take(&mut line));
                line.push_str(word);
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
/// The color of each role is chosen by `chrome::syntax_color`, and not by the
/// highlighter: it gives back a role, and a color token is the host's business
/// — the same border the buffers' colors already respect. One map, because the
/// file manager's preview reads it too, and two would be the two panels
/// colouring the same Rust differently.
fn highlight_line(
    line: &str,
    tongue: Option<ronoctua_syntax::Language>,
    y: u16,
    column: u16,
) -> Vec<ronoctua_render::Span> {
    let simple = |text: &str| ronoctua_render::Span {
        size: ronoctua_render::Size::Grid,
        text: text.to_owned(),
        line: y,
        column,
        color: tokens::color::TEXT_PRIMARY,
        edge: false,
        foot: false,
    };
    let Some(l) = tongue else {
        return vec![simple(line)];
    };
    let ranges = ronoctua_syntax::ranges(line, l);
    if ranges.is_empty() {
        return vec![simple(line)];
    }

    let mut out = Vec::new();
    let mut cursor = 0usize;
    let mut push = |text: &str, kdl_of: usize, color| {
        if text.is_empty() {
            return;
        }
        out.push(ronoctua_render::Span {
            size: ronoctua_render::Size::Grid,
            text: text.to_owned(),
            line: y,
            // THE COLUMN IS IN CELLS, and the range's offset is in BYTES.
            // What matches the two is counting CLUSTERS — the ruler the rest of
            // the drawing uses. Counting characters was nearly right and wrong
            // in the same place as everywhere else: an accent takes two bytes
            // and one cell, but a COMBINING accent takes two characters and one
            // cell, and adding either bytes or characters misaligns the whole
            // line from the first one on.
            column: column + layout::width(&line[..kdl_of]),
            color,
            edge: false,
            foot: false,
        });
    };

    for f in ranges {
        push(&line[cursor..f.start], cursor, tokens::color::TEXT_PRIMARY);
        push(
            &line[f.start..f.end],
            f.start,
            crate::chrome::syntax_color(f.role),
        );
        cursor = f.end;
    }
    push(&line[cursor..], cursor, tokens::color::TEXT_PRIMARY);
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
    ///
    /// It was `modeline.edgeLeft`, and that was the expanded region borrowing
    /// the STATUS LINE's number. The two are different things: the modeline and
    /// the minibuffer are peripheral reference one glances at, and their own
    /// token argues that two cells there "would start to look like a margin";
    /// the expanded region is a list one READS, which is content.
    ///
    /// Left alone it would have become a fresh inconsistency rather than an
    /// inherited one — the file manager's rows moved to `chrome::INSET` in the
    /// same pass, so which-key would have begun one cell in and a file row two.
    const fn start(self) -> u16 {
        crate::chrome::INSET + self.icon
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
    const fn fits_in_text(self, with_key: bool, base: u16, screen_columns: u16) -> u16 {
        let start = self.text_start(with_key);
        let in_column = self
            .width
            .saturating_sub(start.saturating_add(GAP_BETWEEN_COLUMNS));
        let on_screen = screen_columns.saturating_sub(base.saturating_add(start));
        if in_column < on_screen {
            in_column
        } else {
            on_screen
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

/// Builds the grid for a list, on a screen of `screen_columns` cells.
///
/// A pure function for the usual reason: it is arithmetic, and what wraps it
/// does not exist outside a compositor.
fn grid_for(widths: &[Measure], screen_columns: u16, ceiling: usize, in_columns: bool) -> Grid {
    let widest_key = widths.iter().map(|m| m.key).max().unwrap_or(0);
    let widest_text = widths.iter().map(|m| m.text).max().unwrap_or(0);
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
    // THE SAME INSET `Grid::start` uses, and it has to be the same expression:
    // the colon's column is measured from where the key begins, so a mismatch
    // here does not shift the list — it puts the colons out of line with the
    // text under them, which is the one thing this arrangement exists to keep
    // straight.
    let colon = widest_key
        .saturating_add(crate::chrome::INSET + icon + 1)
        .min(screen_columns / 2);
    let width = colon
        .saturating_add(2)
        .saturating_add(widest_text)
        .saturating_add(GAP_BETWEEN_COLUMNS)
        // AND NOTHING GOES PAST THE SCREEN. On a single-column list, the width
        // feeds `item_at`: a column wider than the screen would make the
        // pointer find an item where there is no pixel at all.
        .min(screen_columns.max(1));

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
    let columns = if in_columns && width > 0 {
        (usize::from(screen_columns) / usize::from(width)).max(1)
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

/// The tag that makes the catalogue recognise the out-of-memory notification.
///
/// It is a string that has to be the SAME here and in
/// `assets/notifications.nix`, and there is no compiler between them — so it
/// lives in one place on this side and the `notification-tags-are-known` check
/// compares the two.
pub const MEMORY_KILLED: &str = "memory-killed";

/// The footer's two fixed lines: the modeline and the minibuffer.
///
/// They are the only two that always exist (section 4.1). Everything else on
/// the surface is transient and measured against these: the notification band
/// above, and the expansion below.
pub const FOOTER_LINES: u16 = 2;

/// How many lines the notification band may take, at most.
///
/// Three, and it is [`ronoctua_notify::Show::Full`]'s ceiling: summary, body,
/// actions. The number lives in the protocol crate — this is the surface
/// agreeing with it out loud, so a change over there cannot leave the geometry
/// behind.
pub const BAND_CEILING: u16 = 3;

/// The mouse's left button, in Linux's input protocol.
///
/// `BTN_LEFT` from `linux/input-event-codes.h`. Wayland passes the raw evdev
/// code, with no enum.
const LEFT_BUTTON: u32 = 0x110;
/// `BTN_RIGHT`. It opens the file manager's menu, and nothing else.
const RIGHT_BUTTON: u32 = 0x111;

/// How many lines a wheel detent steps where there is scrolling.
///
/// Three, which is what every text viewer does. One line per detent would
/// turn reading a manual into a wrist exercise; one page per detent is the
/// other mode's behavior, and mixing the two would make the same wheel mean
/// different things with nothing on screen saying which.
pub const LINES_PER_DETENT: f64 = 3.0;

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
pub fn axis_detents(v: &smithay_client_toolkit::seat::pointer::AxisScroll, line_px: f64) -> f64 {
    if v.value120 != 0 {
        return f64::from(v.value120) / 120.0;
    }
    if v.discrete != 0 {
        return f64::from(v.discrete);
    }
    if line_px <= 0.0 {
        return 0.0;
    }
    v.absolute / (line_px * LINES_PER_DETENT)
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
        let Some(held) = self.0.as_mut() else {
            self.0 = Some(mods);
            return;
        };
        // A MODIFIER THAT WAS RELEASED STOPS BEING INHERITED.
        //
        // Without this the discount lasted the whole session: `LEADER .` in the
        // nested loop starts with Ctrl and Alt held, so from then on EVERY
        // `M-…` arrived with the Alt subtracted and matched the bare letter
        // instead. The docstring above already claimed the opposite — "a
        // modifier pressed afterwards, in the middle of the sequence, still
        // counts" — and it did not, because nothing ever cleared the flag.
        //
        // It stayed invisible while no view had a `C-` or `M-` binding. The
        // file manager's do, and there the failure is loud in the wrong way:
        // `M-s` would not sort, it would type an `s` into the query.
        //
        // Only the announcement can say this. The release does not come as a
        // key event — it comes as this, the compositor's own statement of what
        // is held now.
        held.ctrl &= mods.ctrl;
        held.alt &= mods.alt;
        held.shift &= mods.shift;
        held.logo &= mods.logo;
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
            // SHIFT IS SUBTRACTED LIKE THE OTHERS AND JUDGED LATER.
            //
            // It used to be hard `false` here, with the note that xkb has
            // already applied it when producing the character — which is true
            // of a CHARACTER and of nothing else. An arrow is `Left` whether or
            // not Shift is down, so for those keys this is the only place the
            // shift exists, and clearing it here made `S-<left>` unreachable in
            // this door however carefully `translate` asked for it. The rule
            // "shift is not counted twice" now lives where the two cases can be
            // told apart: `translate`, which knows the symbol.
            shift: now.shift && !h.shift,
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
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, mut queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<Footer> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh)
        .context("the compositor does not expose wl_compositor")?;
    let layer_shell = LayerShell::bind(&globals, &qh).context(
        "the compositor does not expose zwlr_layer_shell_v1.\n\
         \n\
         ronoctua-distro's integration with the compositor is EXCLUSIVELY through\n\
             wlr-layer-shell and IPC (invariant 3). With no layer-shell there is no footer,\n\
         and working around that would violate the invariant.",
    )?;

    let footer_height = tokens::metrics::footer_height(tokens::font::SIZE_BASE).ceil() as u32;

    // The keymap is assembled BEFORE the surface comes up. A collision here is a
    // program error, and failing at startup is better than failing on the first leader.
    let (tree, registry) = crate::leader::default_of().context("default keymap")?;

    // THE COPY CHANNEL IS BORN BEFORE the state, because the `Sender` lives
    // inside it — and the reading thread needs a copy of it.
    let (copies_sender, copies) = channel::channel::<ronoctua_clipboard::Entry>();
    // THE WATCHER COMES UP WITH ITS OWN CONNECTION, in a thread. See
    // `crate::clipboard`: the SCTK `wl_data_device` only hands the selection to
    // whoever has FOCUS, and the footer at rest never has it — that is why the
    // protocol is `zwlr_data_control`, which exists precisely for a clipboard
    // manager.
    crate::clipboard::watch(copies_sender);

    // THE TWO DIRECTORIES, found once and handed to everything that persists.
    let notes = ronoctua_notify::store::Where::of_env();
    let mut state = Footer {
        conn: conn.clone(),
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        _compositor: compositor,
        layer_shell,
        requested_height: footer_height,
        screens: Vec::new(),
        quit: false,
        owes_paint: false,
        niri: ronoctua_niri::State::default(),
        seats: SeatState::new(&globals, &qh),
        keyboard: None,
        mods: Modifiers::default(),
        inheritance: Inheritance::default(),
        leader: Leader::default(),
        texts: std::rc::Rc::new(ronoctua_i18n::Texts::from_env()),
        tree,
        inputs: Vec::new(),
        buffer_bindings: Vec::new(),
        buffer_pid: None,
        buffer_pending: Vec::new(),
        notifications: ronoctua_notify::Queue::with_history(notes.history()),
        book: {
            // THE DISTRO'S, PLUS THE PERSON'S. A broken preferences file leaves
            // the distro's standing rather than taking the shell down — see
            // `store::read_preferences`.
            let mut b = ronoctua_notify::Book::distro();
            let prefs = notes.preferences();
            b.mine = prefs.rules;
            b.adjustments = prefs.adjustments;
            b
        },
        notifications_open: false,
        manager: crate::notifications::View::fresh(),
        blink_on: true,
        update_tick: 0,
        band_placed: Vec::new(),
        brain: None,
        brain_placed: None,
        brain_hint: None,
        band_hint: None,
        leader_label: crate::leader(),
        echo: None,
        mx: crate::minibuffer::Minibuffer::default(),
        pointer: None,
        transfers: None,
        jobs: Vec::new(),
        states: std::collections::HashMap::new(),
        modeline_placed: Vec::new(),
        segment_under_pointer: None,
        state_pending: false,
        state_menu: Vec::new(),
        clipboard: ronoctua_clipboard::Queue::default(),
        clipboard_open: false,
        clip_cursor: 0,
        clip_qr: None,
        clip_image: None,
        clip_version: 0,
        detents: 0.0,
        under_pointer: false,
        dragging: false,
        dragging_section: None,
        row_under_pointer: None,
        cell_under_pointer: None,
        manual: false,
        offset: 0,
        right_ctrl: false,
        command_registry: registry,
        frames_painted: 0,
        // The channels are hooked up later, along with the other sources.
        launch_sender: None,
        apps: None,
        network: None,
        files: None,
        files_sender: None,
        files_looking: None,
        files_playing: None,
        repeat: None,
        files_turning: None,
        spinning: None,
        // Read ONCE, at startup. Neither answer changes while a listing is on
        // screen, and asking per row would put two hundred opens inside the
        // frame that draws it.
        roster: ronoctua_files::who::Roster::read_from(&ronoctua_files::who::Files::default()),
        zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
        network_remember: None,
        scan_sender: None,
        scan_watching: false,
        notes,
        disks_watching: false,
        files_notice: None,
        rack: None,
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
    // THE LOOP FIRST, and only then the roundtrip.
    //
    // It used to be the other way round, and it cost the keyboard its repeat:
    // the seat announces its capabilities DURING this roundtrip, and
    // `new_capability` needs a `LoopHandle` to create a keyboard that repeats.
    // `EventLoop::try_new` depends on nothing, so the order was free to change.
    let mut cycle: EventLoop<'static, Footer> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();
    state.repeat = Some(handle.clone());
    queue.roundtrip(&mut state).context("initial roundtrip")?;
    state.ensure_screens(&qh);

    // TWO event sources: Wayland and niri's IPC. Hence the `calloop` instead of
    // a `blocking_dispatch` in a loop — with a loop blocked on the Wayland
    // socket, a niri event would only be seen on the next mouse movement.
    // Milestone 5 adds keyboard and timers on the same rails.

    // THE LAUNCH VERDICT. The thread that waits for `systemd-run --scope` —
    // which is SYNCHRONOUS, see `foreign::launch` — hands it back through here,
    // and this is where the echo area can be written: it needs `&mut Footer`.
    let (launch_sender, launches) = channel::channel::<crate::foreign::Launch>();
    state.launch_sender = Some(launch_sender);
    handle
        .insert_source(launches, |event, (), state: &mut Footer| {
            if let channel::Event::Msg(v) = event {
                state.launched(v);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the launch channel: {e}"))?;

    // WHAT THE FILE MANAGER'S THREADS COME BACK WITH.
    //
    // Copying a directory, walking one and waiting for `systemctl start` are
    // all unbounded, and the loop that draws cannot wait for any of them —
    // `systemd-run --scope` already taught this project what that costs. The
    // channel is built here and always; the threads are raised per operation.
    let (files_sender, files) = channel::channel::<crate::files::Report>();
    // AND THE ONE THAT LOOKS INSIDE A FILE, on the same channel back. Decoding
    // a photograph or seeking a video frame is a child process, and waiting for
    // one inside the loop that draws is the trap this project already paid for.
    state.files_looking = crate::files::inspector(files_sender.clone());
    state.files_sender = Some(files_sender);
    handle
        .insert_source(files, |event, (), state: &mut Footer| {
            if let channel::Event::Msg(r) = event {
                state.files_reported(r);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the files channel: {e}"))?;

    // THE FRAMES OF A MOVING FILE, on a channel of their own — see the window
    // door's twin for why they are not on the reports': a frame arrives ten
    // times a second, and the reports' channel wakes the parts that read a
    // directory.
    //
    // AND THE FRAME ARRIVING IS THE CLOCK. This door has a one-second timer
    // and it is nowhere near fast enough; rather than a second timer that
    // would have to be told when to run, the player sleeps for the frame's own
    // delay and sends it. When it stops, the wake-ups stop with it.
    let (frames_sender, frames) = channel::channel::<crate::motion::Beat>();
    state.files_playing = crate::motion::player(frames_sender);
    handle
        .insert_source(frames, |event, (), state: &mut Footer| {
            if let channel::Event::Msg(f) = event {
                if let Some(v) = state.files.as_mut() {
                    v.played(f);
                }
                state.owes_paint = true;
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the motion channel: {e}"))?;

    // AND THE VIEWS OF A SHAPE, on a channel of their own too, for the reason
    // written above about frames: a drag emits one every few tens of
    // milliseconds and none of them is news about a directory.
    let (turns_sender, turns) = channel::channel::<crate::stage::Beat>();
    state.files_turning = crate::stage::stage(turns_sender);
    handle
        .insert_source(turns, |event, (), state: &mut Footer| {
            if let channel::Event::Msg(b) = event {
                if let Some(v) = state.files.as_mut() {
                    v.drew(b);
                }
                state.owes_paint = true;
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the stage channel: {e}"))?;

    // THE END OF A RADIO SCAN.
    //
    // The channel is built here and always: what is lazy is the THREAD, raised
    // by `watch_scans` on the first `LEADER r`. Building the source on demand
    // would mean touching the loop from inside a key handler, which does not
    // have the handle.
    let (scan_sender, scans) = channel::channel::<()>();
    state.scan_sender = Some(scan_sender);
    handle
        .insert_source(scans, |event, (), state: &mut Footer| {
            if matches!(event, channel::Event::Msg(())) {
                state.rescanned();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the scan channel: {e}"))?;

    // THE APPLICATION CATALOGUE, SWEPT AT STARTUP.
    //
    // Measured on this machine: 128 entries in 68 ms — four frame budgets.
    // Sweeping on pressing `LEADER a` would give a visible freeze every time the
    // list opened, and the gesture of opening it is precisely the one that has
    // to be instant. Here it happens once, in a thread, while the footer is
    // already standing.
    let (catalogue_sender, catalogues) = channel::channel::<crate::apps::Catalogue>();
    state.catalogue_sender = Some(catalogue_sender.clone());
    state.apps_stamp = crate::apps::stamp();
    crate::apps::scan(state.texts.language(), catalogue_sender);
    handle
        .insert_source(catalogues, |event, (), state: &mut Footer| {
            if let channel::Event::Msg(c) = event {
                state.catalogued(c);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the catalogue channel: {e}"))?;

    // THE ICON PIXMAPS, at the second speed.
    //
    // The list draws with the glyphs and does NOT wait for this: each icon that
    // arrives swaps one line. A long-lived thread, because the expensive part is
    // the same for all of them — `look_up` sweeps a superpack of 335,031 files.
    let (painting_sender, paintings) = channel::channel::<crate::apps::Painting>();
    state.icon_queue = crate::apps::painter(painting_sender);
    handle
        .insert_source(paintings, |event, (), state: &mut Footer| {
            if let channel::Event::Msg(p) = event {
                state.painted(p);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the icons channel: {e}"))?;

    let (sender, receiving) = channel::channel::<ronoctua_niri::Event>();
    crate::daemon::listen_to_niri(sender);
    handle
        .insert_source(receiving, |event, (), state: &mut Footer| {
            if let channel::Event::Msg(e) = event {
                state.niri.apply_to(&e);
                state.states_arrived();
                // Redrawing on every event is cheap because the layout is integer
                // arithmetic (section 4.1) and the text only changes when focus changes.
                state.draw_and_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang niri's channel: {e}"))?;

    // Third event source: the control socket, where the leader arrives. See
    // `leader.rs` for why the leader comes from OUTSIDE.
    let (order_sender, orders) = channel::channel::<Order>();
    crate::leader::listen(order_sender);
    handle
        .insert_source(orders, |event, (), state: &mut Footer| {
            if let channel::Event::Msg(o) = event {
                match o {
                    Order::Leader => state.activate_leader(),
                    // THE SAME `run_command` as the key. A command coming from outside and a
                    // command coming from the keymap have to do the SAME thing — two
                    // paths for the same id are what made the terminal's chord open
                    // without a scope while the sequence opened with one.
                    Order::Command(id, argument) => {
                        tracing::info!(command = %id, "command through the control socket");
                        state.run_with(&id, argument.as_deref());
                        state.draw_and_paint();
                    }
                    // ANOTHER FOOTER TOOK OVER THIS DISPLAY. Leaving is the only right
                    // answer: staying up would leave two surfaces anchored at the
                    // bottom and — if this one had the leader open — this one holding
                    // the keyboard the other needs. See `leader::yield_the_place`.
                    Order::Quit => {
                        tracing::info!("another footer took this display; leaving");
                        state.quit = true;
                    }
                    // A BUFFER SAYS WHICH STATE IT IS IN. It only draws when the
                    // window that reported is the one in FOCUS: a buffer changing
                    // state behind another window changes no pixel, and repainting
                    // for it would put a frame on the screen per key pressed in a
                    // window nobody is looking at.
                    Order::State { pid, id } => {
                        let showing = state.states.get(&pid) != Some(&id);
                        state.states.insert(pid, id);
                        if showing
                            && state.niri.focused().and_then(|w| w.pid) == i32::try_from(pid).ok()
                        {
                            state.draw_and_paint();
                        }
                    }
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the control socket: {e}"))?;

    // FIFTH SOURCE: what was copied. The reading thread sends it through here;
    // see `crate::clipboard::watch`.
    handle
        .insert_source(copies, |event, (), state: &mut Footer| {
            if let channel::Event::Msg(e) = event {
                let bytes = e.content.bytes().len();
                if state.clipboard.register(e) {
                    tracing::debug!(bytes, "new entry in the clipboard history");
                    // It only redraws if the view is OPEN. Copying with the clipboard
                    // closed must not cost a frame.
                    if state.clipboard_open {
                        state.apply_geometry(state.interactivity());
                        state.draw_and_paint();
                    }
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the clipboard's channel: {e}"))?;

    // Fourth source: the session bus. The notification daemon runs in a thread
    // and sends through the same kind of channel as niri and the control socket.
    let (notify_sender, notifications) = channel::channel::<ronoctua_notify::Event>();
    let (raw_tx, raw_rx) = std::sync::mpsc::channel::<ronoctua_notify::Event>();
    // FIFTH SOURCE, THE SAME CHANNEL: what `systemd-oomd` closed to keep the
    // machine alive. It is the SYSTEM bus and not the session one, and it shares
    // the bridge below on purpose — one destination, one path to keep in step.
    //
    // It is started BEFORE the session daemon so that a boot in which the footer
    // comes up first still registers the match: the rule is filed with the bus,
    // and the signal arrives whenever `systemd-oomd` takes the name.
    ronoctua_notify::oom::listen(raw_tx.clone());
    if ronoctua_notify::dbus::listen(raw_tx) {
        // A bridge between zbus's `mpsc` and calloop's channel. They are two kinds
        // of channel because zbus does not know calloop and vice versa; the bridge
        // is a thread that only forwards.
        std::thread::Builder::new()
            .name("ponte-notif".into())
            .spawn(move || {
                while let Ok(e) = raw_rx.recv() {
                    if notify_sender.send(e).is_err() {
                        return;
                    }
                }
            })
            .expect("the notification bridge cannot fail to be born");
    }
    handle
        .insert_source(notifications, |event, (), state: &mut Footer| {
            if let channel::Event::Msg(e) = event {
                state.notification_of(e);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the notifications: {e}"))?;

    // SIXTH SOURCE: the transfer queue. The watching thread sends the whole
    // list every time it changes; see `crate::transfers::listen`.
    let (queue_sender, transfers_queue) = channel::channel::<Vec<ronoctua_transfers::Job>>();
    crate::transfers::listen(queue_sender);
    handle
        .insert_source(transfers_queue, |event, (), state: &mut Footer| {
            if let channel::Event::Msg(jobs) = event {
                state.ledger_arrived(jobs);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the transfer queue: {e}"))?;

    // SEVENTH SOURCE: the brain. The watching thread sends the whole mind every
    // time it changes; see `crate::brain::listen`.
    //
    // A thread and a channel, and not a socket in the loop, for the two reasons
    // the transfer queue's already gives — plus the one that belongs to this
    // one: `client::watch` starts the daemon when nobody is home and retries
    // when it goes away, which is a sleep, and a sleep here is a frozen footer.
    let (brain_sender, minds) = channel::channel::<ronoctua_brain::Mind>();
    crate::brain::listen(brain_sender);
    handle
        .insert_source(minds, |event, (), state: &mut Footer| {
            if let channel::Event::Msg(mind) = event {
                state.mind_arrived(mind);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the brain: {e}"))?;

    // A timer to expire the notification on screen.
    //
    // One second is coarse on purpose: the resolution that matters is "it went
    // away after a few seconds", and waking the loop 60 times a second for that
    // would spend battery (section 2 aims at laptops).
    handle
        .insert_source(
            calloop::timer::Timer::from_duration(std::time::Duration::from_secs(1)),
            |_, (), state: &mut Footer| {
                let now = std::time::Instant::now();
                // Both on the SAME tick: a second clock just for the echo area would
                // wake the loop twice a second for nothing.
                let won = state.expire_echo(now);
                // AND THE DIRECTORY, when the manager is open. One `stat` a second
                // against the directory's mtime: something else moved a file into
                // it, an operation of ours finished, an archive was unpacked. A
                // listing that only refreshes when a key is pressed is a listing
                // that lies quietly.
                //
                // IT STAYS EVEN THOUGH `notice` NOW WATCHES THE SAME DIRECTORY,
                // and it is not redundant. inotify reports what THIS machine
                // does; a file put there by another machine over NFS, sshfs or
                // SMB emits nothing at all, and the mtime is the only thing that
                // moves. The tick was already being paid for the blink, the pie
                // and the brain's countdown, so the floor is one `stat` — the
                // window door, which has no tick, does without it and says so.
                let changed = state.files.as_ref().is_some_and(crate::files::View::stale);
                // AND THE SIDEBAR'S FILE, on the same tick and for the same reason:
                // `LEADER a` pins an application without this view being open, and
                // the window door is a separate process with its own copy. A row
                // that only appears after a restart is a row somebody thinks did
                // not get pinned.
                let bar = state
                    .files
                    .as_ref()
                    .is_some_and(crate::files::View::sidebar_stale);
                if changed || bar {
                    if let Some(v) = state.files.as_mut() {
                        if changed {
                            v.reread();
                        }
                        if bar {
                            v.reread_sidebar();
                        }
                    }
                    state.apply_geometry(state.interactivity());
                }
                // THE BLINK RIDES THIS CLOCK. It is why a notification that asks to
                // blink costs nothing: the loop was already waking once a second to
                // expire things, and a second timer for the same second would wake
                // it twice for one effect.
                state.blink_on = !state.blink_on;
                let blinking = state
                    .notifications
                    .showing()
                    .is_some_and(|s| s.presentation.blink);
                // THE PIE MOVES EVERY SECOND TOO, so a notification on screen
                // redraws whether or not it expired: eight steps over six seconds
                // means the drawing changes on most ticks, and a band whose
                // countdown only moves when something else happens is a countdown
                // that lies.
                let na_faixa = state.notifications.showing().is_some();
                // AND THE BRAIN'S COUNTDOWN RIDES THE SAME CLOCK, for the reason
                // written just above about the pie: the brain sends the warning
                // ONCE and the number on screen is `deadline - now`, so a line
                // whose seconds only moved when something else happened would be a
                // countdown that lies. A timer of its own would wake the loop twice
                // in the same second for one effect.
                let counting = state.brain_rows() > 0;
                // AND THE UPDATE'S MARK RIDES IT TOO, one frame per tick. The
                // counter always advances — it is a `u64` add — but the REDRAW is
                // asked for only while something is actually running, so a machine
                // with nothing in flight does not paint once a second for a glyph
                // that is not on screen.
                state.update_tick = state.update_tick.wrapping_add(1);
                let updating = crate::update_window::running(state.brain.as_ref());
                let beat = state.notifications.tick(now);
                // WHOEVER SENT IT IS WAITING TO BE TOLD. A client that offered a
                // button, or that meant to replace this notification with the next
                // one, is blocked on `NotificationClosed` — and `Expired` is a
                // different answer from `ByHand`, which is why the reason travels.
                if let Some(id) = beat.expired {
                    ronoctua_notify::dbus::notification_was_closed(
                        id,
                        ronoctua_notify::dbus::Closed::Expired,
                    );
                }
                if beat.changed {
                    // The queue MOVED: something left the band and something else
                    // may have taken it. The height changes with it, and so does
                    // the record on disk.
                    state.after_the_queue_moved();
                } else if won || changed || bar || blinking || na_faixa || counting || updating {
                    state.draw_and_paint();
                }
                calloop::timer::TimeoutAction::ToDuration(std::time::Duration::from_secs(1))
            },
        )
        .map_err(|e| anyhow::anyhow!("could not hang the timer: {e}"))?;

    WaylandSource::new(conn, queue)
        .insert(handle)
        .map_err(|e| anyhow::anyhow!("could not hang Wayland on calloop: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |state| {
            // THE PAINT OWED, once the queue is empty. See `Footer::owes_paint`.
            if state.owes_paint {
                state.owes_paint = false;
                state.draw_and_paint();
            }
            if state.quit {
                signal.stop();
            }
        })
        .context("event loop")?;
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

        // THE STATE IS A KEY, like everything else on this line that is not the
        // application's own data: the buffer declares an English id and the
        // screen reads `state-<id>` (invariants 8 and 18). The buffer's NAME is
        // the exception, and stays the exception — it comes from the
        // application, and translating an open file's name would be absurd.
        let state = self
            .state_now()
            .map(|id| self.texts.t(&format!("state-{id}")))
            .unwrap_or_default();
        crate::segments(
            &crate::daemon::buffer_name(&self.niri, &self.texts),
            &crate::daemon::buffer_mode(&self.niri, &self.texts),
            &state,
            // WHAT THE BELL COUNTS IS UNREAD, and it used to be "waiting in the
            // queue". The two answer different questions: the queue empties by
            // itself in seconds, so the badge went back to zero without
            // anybody having read anything. The count that survives the band is
            // the one worth showing.
            //
            // And a SUPPRESSED one is not in it — a badge that counts what the
            // person asked never to see is a badge that cannot be brought to
            // zero. See `Record::counts`.
            self.notifications.unread(),
            crate::transfers::segment(&self.jobs).as_deref(),
            // THE UPDATE'S MARK, out of the mind the footer already watches —
            // never a second subscription and never a schedule of its own. It
            // is `None` with nothing in flight, and the segment then does not
            // exist at all.
            crate::update_window::mark(self.brain.as_ref(), self.update_tick).as_deref(),
        )
    }

    /// Windows changed: prune the state map and ask the focused buffer.
    ///
    /// # Why it asks at all, when the buffer pushes
    ///
    /// The push covers every change while the window has the focus, which is
    /// nearly all of them. It does not cover a footer that RESTARTED with
    /// buffers already open — a real case, because replacing the footer is one
    /// key — nor a buffer that changed state while another window had focus.
    /// One socket round trip per Alt-Tab closes both.
    ///
    /// The prune is against the compositor's list and not a timer: a buffer that
    /// dies takes its entry with it, and a PID is reused by the kernel sooner
    /// than anybody expects.
    fn states_arrived(&mut self) {
        let alive: std::collections::HashSet<u32> = self
            .niri
            .all_windows()
            .iter()
            .filter_map(|w| w.pid)
            .filter_map(|p| u32::try_from(p).ok())
            .collect();
        self.states.retain(|pid, _| alive.contains(pid));

        // ONLY A WINDOW OF OURS IS ASKED. This runs on every event the
        // compositor sends, and a foreign window has no socket to answer on —
        // so without this line every workspace change would try to connect to a
        // path that is not there, once per window that is not ours, forever.
        let Some(window) = self.niri.focused() else {
            return;
        };
        if window
            .app_id
            .as_deref()
            .and_then(|a| a.strip_prefix(crate::daemon::OURS))
            .is_none()
        {
            return;
        }
        let Some(pid) = window.pid.and_then(|p| u32::try_from(p).ok()) else {
            return;
        };
        // ALREADY KNOWN IS NOT ASKED AGAIN. Most focus changes are between
        // windows that were already open, and asking each time would put a
        // connect and a 200 ms timeout on the path of every Alt-Tab, for an
        // answer that has not changed.
        if self.states.contains_key(&pid) {
            return;
        }
        if let Ok(Some((now, _))) = crate::leader::buffer_states(pid) {
            self.states.insert(pid, now);
        }
    }

    /// Which modeline segment is at `(x, y)`, if any.
    ///
    /// # Why the modeline and not the minibuffer too
    ///
    /// Because the minibuffer has nothing to point at: it is one run of text
    /// that is either what somebody is typing or a message. The modeline is a
    /// row of things that each mean something different, which is what makes
    /// pointing at one of them a question worth answering.
    ///
    /// The modeline is on line [`Footer::modeline_line`], in the same top-down
    /// coordinates the band and the expansion use.
    fn segment_at(&self, x: f64, y: f64) -> Option<&'static str> {
        let line_height = f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE));
        if line_height <= 0.0 {
            return None;
        }
        // THE MODELINE MOVED DOWN, by however many lines the band is taking.
        // With this fixed at line zero, every click on a notification would
        // land on the buffer's name instead — and the name is not clickable, so
        // it would look like a click that did nothing.
        let top = f64::from(self.modeline_line()) * line_height;
        if y < top || y >= top + line_height {
            return None;
        }
        let cell = f64::from(tokens::metrics::cell_width(tokens::font::SIZE_BASE));
        if cell <= 0.0 || x < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let column = (x / cell) as u32;
        self.modeline_placed
            .iter()
            .find(|(_, at, width)| {
                column >= u32::from(*at) && column < u32::from(*at) + u32::from(*width)
            })
            .map(|(id, _, _)| *id)
    }

    /// Which piece of the band is at `(x, y)`.
    ///
    /// From what was DRAWN (`band_placed`), and not from a second layout — the
    /// same rule the modeline's segments follow, and for the same reason: a
    /// `Motion` arrives per pixel of movement, and two calculations of one
    /// position are two chances to point at the wrong thing.
    fn band_at(&self, x: f64, y: f64) -> Option<&crate::band::Piece> {
        let line_height = f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE));
        let cell = f64::from(tokens::metrics::cell_width(tokens::font::SIZE_BASE));
        if line_height <= 0.0 || cell <= 0.0 || x < 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let row = (y / line_height) as u16;
        if row >= self.band_rows() {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let column = (x / cell) as u16;
        crate::band::at(&self.band_placed, row, column)
    }

    /// A click on the band: the circle marks it read, an action runs.
    ///
    /// # The mouse reaches nothing the keyboard does not
    ///
    /// The circle is `notif-dismiss`, which has a key (`n d`). An action is
    /// pressed by its letter in the manager, which is one key away (`n h`) and
    /// where the keyboard actually is — the footer at rest does not capture it,
    /// or no buffer would ever receive one.
    fn band_clicked(&mut self, hit: crate::band::Hit) {
        let Some(id) = self.notifications.showing().map(|s| s.notification.id) else {
            return;
        };
        match hit {
            crate::band::Hit::Read => {
                self.manager_errand(crate::notifications::Errand::Read(id));
            }
            crate::band::Hit::Action(n) => self.run_notification_action(id, n),
        }
    }

    /// Which piece of the brain's line is at `(x, y)`.
    ///
    /// From what was DRAWN (`brain_placed`), and not from a second layout — the
    /// same rule the modeline's segments and the band already follow.
    fn brain_at(&self, x: f64, y: f64) -> Option<&crate::brain::Piece> {
        let line_height = f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE));
        let cell = f64::from(tokens::metrics::cell_width(tokens::font::SIZE_BASE));
        if line_height <= 0.0 || cell <= 0.0 || x < 0.0 || y < 0.0 || self.brain_rows() == 0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let row = (y / line_height) as u16;
        if row != self.brain_line_at() {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let column = (x / cell) as u16;
        crate::brain::at(self.brain_placed.as_ref(), column)
    }

    /// A click on the brain's line: the two controls, and nothing else.
    ///
    /// # The mouse reaches nothing the keyboard does not
    ///
    /// Each one IS a registered command — `brain-abort` and `brain-proceed` —
    /// and the sequence that runs it is drawn on the line beside the label. The
    /// click does not do a second thing: it runs the same command the key runs,
    /// through the same door.
    fn brain_clicked(&mut self, hit: crate::brain::Hit) {
        match hit {
            crate::brain::Hit::Abort => self.run_command("brain-abort"),
            crate::brain::Hit::Proceed => self.run_command("brain-proceed"),
        }
    }

    /// A click on a modeline segment does what its key does.
    ///
    /// # The mouse reaches nothing the keyboard does not
    ///
    /// TWO segments answer, and each has a key. On the `state`, left is the
    /// next state and right the previous — exactly what holding `Esc` and
    /// pressing an arrow does. On the `notif`, either button opens the manager,
    /// which is what `LEADER n h` does.
    ///
    /// That is the rule invariant 14 turns into for a pointer: the mouse is a
    /// second way to the same door, and never a door of its own.
    fn segment_clicked(&mut self, id: &str, forward: bool) {
        // THE BELL AND ITS COUNT OPEN THE MANAGER.
        //
        // It is the one thing on that line the eye goes to when it wants to
        // know what it missed, and until now clicking it did nothing — a mark
        // that says "there are six" and answers nothing when asked.
        //
        // Both buttons, because there is only one thing to do here: a second
        // meaning for the right button would be a menu nobody asked for.
        if id == "notif" {
            self.run_command("notif-history");
            return;
        }
        // THE UPDATE'S MARK OPENS THE WINDOW THAT WATCHES IT.
        //
        // It is the one thing on this line that appears without anybody having
        // asked for it and says only "something is happening" — the question
        // it raises is "what?", and until now the mark could not answer it.
        //
        // Both buttons, for the reason the bell's note gives: there is one
        // thing to do here, and a second meaning for the right button would be
        // a menu nobody asked for. And it is the SAME command `LEADER s w`
        // runs, which is what keeps the mouse from reaching a door the
        // keyboard cannot.
        if id == "update" {
            self.run_command("update-watch");
            return;
        }
        if id != "state" {
            return;
        }
        let Some(pid) = self
            .niri
            .focused()
            .and_then(|w| w.pid)
            .and_then(|p| u32::try_from(p).ok())
        else {
            return;
        };
        let order = if forward { "state-next" } else { "state-prev" };
        match crate::leader::buffer_state(pid, order) {
            Ok(()) => tracing::info!(order, "state cycled by the pointer"),
            Err(e) => tracing::debug!(error = %e, "no buffer to cycle the state"),
        }
    }

    /// What the minibuffer says while the pointer rests on a segment.
    ///
    /// Only the CLICKABLE ones answer. The others already say what they are —
    /// the buffer's name is the buffer's name — and a sentence explaining each
    /// of them would be a tooltip in everything but the surface it is drawn on.
    fn segment_hint(&self) -> Option<String> {
        match self.segment_under_pointer? {
            "notif" => Some(self.texts.t(ronoctua_i18n::key::HINT_NOTIF_BELL)),
            // The third clickable thing on this line, and it explains itself
            // on the message line like the other two — never in a tooltip,
            // which would be a surface of its own (invariant 14).
            "update" => Some(self.texts.t(ronoctua_i18n::key::HINT_UPDATE_MARK)),
            "state" => {
                let now = self.state_now()?;
                Some(self.texts.t_with(
                    ronoctua_i18n::key::HINT_STATE,
                    &[("state", &self.texts.t(&format!("state-{now}")))],
                ))
            }
            _ => None,
        }
    }

    /// The focused buffer's state, when the focused window is one of ours.
    ///
    /// `None` for a foreign window, and that is the design: the compositor tells
    /// us a title, an app id and a pid, and none of those says what state `vim`
    /// is in. Drawing `normal` for it would be the same false assertion the
    /// `fundamental` label used to make — a word that takes up a column and says
    /// something nobody checked.
    fn state_now(&self) -> Option<&str> {
        let window = self.niri.focused()?;
        window
            .app_id
            .as_deref()?
            .strip_prefix(crate::daemon::OURS)?;
        let pid = u32::try_from(window.pid?).ok()?;
        self.states.get(&pid).map(String::as_str)
    }

    /// A new list from the queue.
    ///
    /// # Why it does not simply repaint
    ///
    /// Because the queue beats every 120 ms while a copy runs, and the segment
    /// shows WHOLE per cent: eight of those beats out of nine change no pixel
    /// at all. Repainting on each would put the frame cost of a copy onto every
    /// screen in the session for a number that did not move.
    ///
    /// So it compares what the modeline would SAY. If the sentence is the same
    /// and no panel is open, the list is stored and nothing is drawn.
    fn ledger_arrived(&mut self, jobs: Vec<ronoctua_transfers::Job>) {
        let before = crate::transfers::segment(&self.jobs);
        // WHAT ENDED, by the same function the window door uses. Two ways of
        // deciding "this job is over" is how the two doors come to disagree
        // about whether to read the directory again.
        for report in crate::transfers::what_ended(&self.jobs, &jobs) {
            self.files_reported(report);
        }
        self.jobs = jobs;
        if let Some(v) = self.transfers.as_mut() {
            v.refresh(self.jobs.clone());
        }
        let changed = before != crate::transfers::segment(&self.jobs);
        if self.transfers.is_some() {
            self.apply_geometry(KeyboardInteractivity::Exclusive);
        }
        if changed || self.transfers.is_some() {
            self.draw_and_paint();
        }
    }

    /// A fresh mind arrived from the brain.
    ///
    /// # Why it compares the ANNOUNCEMENT and not the mind
    ///
    /// The same reason `ledger_arrived` compares the modeline's sentence rather
    /// than the job list: the daemon republishes on every beat of its own tick
    /// — a new pulse, a moment considered — and almost none of that reaches
    /// this surface. What the footer draws out of the whole mind is ONE line,
    /// and that line is the announcement.
    ///
    /// So the mind is kept whatever happens (the two commands read `running`
    /// out of it), and the screen only moves when the thing on it changed.
    fn mind_arrived(&mut self, mind: ronoctua_brain::Mind) {
        let before = self.announced().cloned();
        let height = self.brain_rows();
        self.brain = Some(mind);
        if before == self.announced().cloned() {
            return;
        }
        tracing::info!(
            announced = self.announced().map(|a| a.task.as_str()),
            running = self.brain.as_ref().map(|m| m.running.as_str()),
            // THE ROW COUNT, in the shape `band=` already has on the
            // notification queue's line. It is what a VM waits on: "the line is
            // on screen" is a height, and a test that slept instead would be
            // either slow or flaky, and on a loaded machine both.
            brain = self.brain_rows(),
            "the brain spoke"
        );
        // A LINE APPEARING OR GOING is a change of HEIGHT, and the surface has
        // to ask for it before it draws: painting first would put the line
        // outside the surface it was measured for.
        if height != self.brain_rows() {
            self.apply_geometry(self.interactivity());
        }
        self.draw_and_paint();
    }

    /// `LEADER s u`: the whole update, asked for by hand.
    ///
    /// # It names no step, and it cannot
    ///
    /// What an update IS lives in `assets/updates.nix` and nowhere else
    /// (invariant 5): this triggers the same unit the brain triggers, by the
    /// brain's own constant. The footer holds no list of steps, and
    /// `update-has-one-list` is the check that keeps it from growing one.
    ///
    /// # And it says which door it went through
    ///
    /// Because the two are different promises. Through the brain the run is
    /// QUEUED — it waits for a machine that is not busy, it gets a countdown
    /// that can be stopped, and it is written down. Straight to systemd it
    /// starts now. A single sentence for both would be right about one of them.
    fn start_the_update(&mut self) {
        let said = match crate::update_window::begin() {
            crate::update_window::Started::Brain => ronoctua_i18n::key::ECHO_UPDATE_QUEUED,
            crate::update_window::Started::Systemd => ronoctua_i18n::key::ECHO_UPDATE_STARTED,
            // LOUD, and on screen. A key that appears in which-key and does
            // nothing is indistinguishable from a key that did not register —
            // which is how five mute commands went unnoticed once already.
            crate::update_window::Started::Nowhere => ronoctua_i18n::key::ECHO_UPDATE_NOWHERE,
        };
        let m = self.texts.t(said);
        self.echo_message(m);
    }

    /// The countdown's two answers, from the keyboard or from the line itself.
    ///
    /// # Why the refusal is spoken and not swallowed
    ///
    /// Because `LEADER s a` with nothing about to happen is a key that appears
    /// in which-key and does nothing — indistinguishable from a key that did
    /// not register, which is how five mute commands went unnoticed once
    /// already.
    fn brain_answer(&mut self, proceeding: bool) {
        let ask = self.brain.as_ref().and_then(|m| {
            if proceeding {
                // PROCEED IS THE DEADLINE MOVED and not a second way to start
                // something: with nothing announced there is no deadline to
                // move, and starting one here would be a path into a run that
                // the brain's own queue never saw.
                m.announcement
                    .as_ref()
                    .map(|_| ronoctua_brain::Ask::Proceed)
            } else if m.announcement.is_some() || !m.running.is_empty() {
                Some(crate::brain::abort(m))
            } else {
                None
            }
        });
        let Some(ask) = ask else {
            let m = self.texts.t(ronoctua_i18n::key::ECHO_BRAIN_IDLE);
            self.echo_message(m);
            return;
        };
        let said = match crate::brain::tell(&ask) {
            Ok(()) => {
                tracing::info!(?ask, "answer to the brain");
                if proceeding {
                    ronoctua_i18n::key::ECHO_BRAIN_PROCEEDING
                } else {
                    ronoctua_i18n::key::ECHO_BRAIN_POSTPONED
                }
            }
            // LOUD, and said on screen. A countdown that goes on running after
            // somebody pressed "not now" is the one failure here that must
            // never be silent.
            Err(e) => {
                tracing::error!(error = %e, ?ask, "the brain did not get the answer");
                ronoctua_i18n::key::ECHO_BRAIN_UNREACHABLE
            }
        };
        let m = self.texts.t(said);
        self.echo_message(m);
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
            "footer layout resolved"
        );
        for c in &placed {
            tracing::debug!(id = c.id, column = c.column, text = %c.text, "segment");
        }
    }
}

impl LayerShellHandler for Footer {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        tracing::info!("the compositor closed the footer's surface");
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
        _factor: i32,
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
                "output removed; surface closed"
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
            // WITH REPEAT, and it had never been. SCTK only calls `repeat_key`
            // for a keyboard created this way, so that method was dead code in
            // both doors and NO key repeated anywhere in this shell — holding
            // an arrow in the file manager moved the cursor exactly once. The
            // rate and the delay are the compositor's, arriving as
            // `RepeatInfo`; nothing here invents either.
            let made = if let Some(handle) = self.repeat.clone() {
                self.seats.get_keyboard_with_repeat(
                    qh,
                    &seat,
                    None,
                    handle,
                    Box::new(|state: &mut Self, _, event| state.typed(event)),
                )
            } else {
                self.seats.get_keyboard(qh, &seat, None)
            };
            match made {
                Ok(k) => {
                    tracing::debug!("the seat's keyboard is ready");
                    self.keyboard = Some(k);
                }
                Err(e) => tracing::warn!(error = %e, "no keyboard; the leader will not work"),
            }
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => self.pointer = Some(p),
                // With no pointer the footer works whole: what is lost is the
                // scrollbar, which is a visual hint, not a path.
                Err(e) => {
                    tracing::warn!(error = %e, "no pointer; the scrollbar does not show")
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
        tracing::debug!("the footer gained keyboard focus");
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
            tracing::debug!("keyboard focus lost; closing what was open");
            self.mx.close_it();
            self.manual = false;
            self.apps = None;
            // THE MANAGER GOES TOO, and it is the one that loses the most: a
            // directory, a set of marks, an armed confirmation. Leaving it open
            // without the keyboard would be worse — a listing on screen that
            // answers nothing, with an armed `delete` nobody can see is armed.
            self.files = None;
            self.rest_the_watch();
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
        // A repeat inside a leader sequence would be typing the same key twice by
        // accident. But `SPC b b b` does not exist, so a repeat is always an error
        // here — we let it through to the same path, which ends by unknown
        // sequence instead of piling up state.
        self.typed(event);
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
            "modifiers announced by the compositor"
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
                // THE STATE ROW, grafted onto `SPC b` the same way the focused
                // buffer's row is grafted onto the root. It cannot be in the
                // tree: the states come from whatever window has the focus, and
                // the tree's bindings are `&'static str`.
                //
                // It appears only when there is a CHOICE — see
                // `refresh_buffer_keymap`.
                if !self.state_menu.is_empty()
                    && ronoctua_keymap::notation(self.leader.typed()) == "b"
                    && let Ok(key) = ronoctua_keymap::Key::parse("s")
                {
                    self.inputs.push(ronoctua_keymap::Entry {
                        key: "s".to_owned(),
                        sym: key,
                        label: ronoctua_i18n::key::WHICH_STATE.to_owned(),
                        command: None,
                        is_group: true,
                    });
                }
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
                            &[("sequence", seq), ("other", &otherwise_one)],
                        )
                    }
                    Some(seq) if sym != ronoctua_keymap::Sym::Esc => self
                        .texts
                        .t_with(ronoctua_i18n::key::ECHO_NO_ON, &[("sequence", seq)]),
                    _ => self.texts.t(ronoctua_i18n::key::ECHO_CANCELLED),
                };
                self.close_leader();
                self.echo_message(msg);
            }
            Effect::Run(id, argument) => {
                tracing::info!(command = id, argument, "command fired by the leader");
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
        let before = (
            self.under_pointer,
            self.row_under_pointer,
            self.cell_under_pointer,
            // THROUGH `current_offset`, because with the manager open the
            // number is the view's — see `Footer::scroll_offset_to`. Reading
            // the field directly here would say "nothing changed" for every
            // scroll of the file manager.
            self.current_offset(),
        );
        let segment_before = self.segment_under_pointer;
        let mut clicked = None;
        let mut menu = false;
        // THE MODELINE'S OWN TWO CLICKS, kept apart from `clicked` and `menu`
        // above: those two are about a ROW of the expanded region, and the
        // modeline has no rows. Folding them together is how the right button
        // came to act one line off once already.
        let mut segment_next = None;
        let mut spun = false;
        let mut segment_prev = None;
        // AND THE BAND'S, kept apart for the same reason: its lines are not
        // rows of the expanded region either.
        let mut band_hit = None;
        // AND THE BRAIN'S, which is a third region and not a row: it sits
        // between the minibuffer and the list, and everything below asks the
        // row what it is.
        let mut brain_hit = None;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.under_pointer = true;
                    self.row_under_pointer = self.row_at(e.position.0, e.position.1);
                    self.cell_under_pointer = self.cell_at(e.position.0);
                    self.segment_under_pointer = self.segment_at(e.position.0, e.position.1);
                    // WHAT THE BAND'S CONTROL SAYS, on the message line and
                    // never in a tooltip — the precedent the file manager set,
                    // and what keeps invariant 14 standing now that the band is
                    // clickable.
                    self.band_hint = self
                        .band_at(e.position.0, e.position.1)
                        .and_then(|p| p.explain.clone());
                    // AND THE BRAIN'S LINE, the same way and on the same line:
                    // whatever explains itself does it on the message line and
                    // never in a tooltip (invariant 14).
                    self.brain_hint = self
                        .brain_at(e.position.0, e.position.1)
                        .and_then(|p| p.explain.clone());
                    // AND WHICH OF THE MANAGER'S SECTIONS, which is what
                    // decides the film's two buttons are drawn — see the
                    // window door's twin. Their CELLS are reserved either way,
                    // so nothing moves when they arrive.
                    let over = self.files_target().map(|t| t.pane);
                    if let Some(v) = self.files.as_ref() {
                        v.pointing(over);
                    }
                    // AND A SHAPE TURNS WITH THE HAND. The deltas are in
                    // PIXELS and become radians here, because the view has no
                    // idea how big a pixel is; a whole screen's width is about
                    // half a turn, which is the ratio that makes a piece feel
                    // attached to the pointer rather than geared to it.
                    if let Some((was_x, was_y)) = self.spinning {
                        let wide = self
                            .screens
                            .iter()
                            .find(|t| t.configured && t.width > 0)
                            .map_or(1.0, |t| f64::from(t.width));
                        let by = std::f64::consts::PI / wide;
                        #[allow(clippy::cast_possible_truncation)]
                        // WHERE THE HAND WENT, in radians of turn. Which way a
                        // yaw counts is `View::spin`'s business and written out
                        // there — a convention spelled at two call sites is a
                        // convention that gets one of them backwards, which is
                        // what happened.
                        let turned = self.files.as_mut().is_some_and(|v| {
                            v.spin(
                                ((e.position.0 - was_x) * by) as f32,
                                ((e.position.1 - was_y) * by) as f32,
                            )
                        });
                        self.spinning = Some(e.position);
                        if turned {
                            spun = true;
                        }
                    }
                    // DRAGGING: the pointer's position BECOMES the offset.
                    if let Some((pane, axis)) = self.dragging_section {
                        self.drag_section_to(pane, axis, e.position.0, e.position.1);
                    } else if self.dragging {
                        self.drag_to(e.position.1);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.under_pointer = false;
                    self.row_under_pointer = None;
                    self.cell_under_pointer = None;
                    self.segment_under_pointer = None;
                    self.band_hint = None;
                    self.brain_hint = None;
                    self.dragging = false;
                    if let Some(v) = self.files.as_ref() {
                        v.pointing(None);
                    }
                }
                // THE RIGHT BUTTON OPENS THE FILE MANAGER'S MENU, and nothing
                // else.
                //
                // This comment used to say the other buttons do nothing because
                // "a context menu would be new chrome (invariant 14)". That was
                // true of the only implementation anybody had in mind — a popup
                // surface floating at the pointer. The menu that exists now is
                // drawn in the SAME expanded region as which-key, has no
                // surface of its own and does not float; each of its rows shows
                // the key that runs it, so the mouse reaches nothing the
                // keyboard does not. Invariant 14 is untouched.
                //
                // The middle button still does nothing: pasting text into a
                // list of commands would be a command run by mistake.
                PointerEventKind::Press { button, .. } if button == RIGHT_BUTTON => {
                    self.row_under_pointer = self.row_at(e.position.0, e.position.1);
                    self.cell_under_pointer = self.cell_at(e.position.0);
                    // THE MODELINE FIRST, because it is not a row: a press on
                    // line 0 has no `ExpansionRow` behind it, and everything
                    // below asks the row what it is.
                    if let Some(id) = self.segment_at(e.position.0, e.position.1) {
                        segment_prev = Some(id);
                        continue;
                    }
                    // AND THAT IS ALL THE PRESS DECIDES. What it is over is
                    // `files_target()`'s answer, asked once at dispatch — the
                    // same question the left button asks.
                    //
                    // IT USED TO ASK THE `ExpansionRow` INSTEAD, and that is
                    // why the owner reported the button as dead in this door
                    // while it worked in the window. `visible_row(i)` builds
                    // ONE line — `expansion_window(offset + i, 1)` — and with
                    // the manager open a window of one line is the listing's
                    // chrome: every line on screen answered `Click::FilesPath`,
                    // whichever file was on it. The match here wanted
                    // `Click::Files(n)`, never saw one, and fell through to
                    // `None`. The left button never noticed because it throws
                    // the payload away and re-resolves through
                    // `files::View::hit`; this one read the number.
                    //
                    // The only case that survived was the empty directory,
                    // which had a guard of its own — so the button worked on
                    // the one listing with nothing to act on.
                    menu = true;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    self.row_under_pointer = self.row_at(e.position.0, e.position.1);
                    self.cell_under_pointer = self.cell_at(e.position.0);
                    // THE BAND FIRST, because it is above everything and is not
                    // a row: its lines have no `ExpansionRow` behind them, and
                    // everything below here asks the row what it is.
                    //
                    // Only the LEFT button. The right one does nothing on the
                    // band: every control up there is a single, obvious
                    // gesture, and a second meaning for the same target would
                    // be a menu nobody asked for.
                    if let Some(hit) = self.band_at(e.position.0, e.position.1).and_then(|p| p.hit)
                    {
                        band_hit = Some(hit);
                        continue;
                    }
                    // AND THE BRAIN'S LINE, for the same reason and with the
                    // same restriction: only the LEFT button, because both
                    // controls up there are single obvious gestures and a
                    // second meaning for the same target would be a menu
                    // nobody asked for.
                    if let Some(hit) = self
                        .brain_at(e.position.0, e.position.1)
                        .and_then(|p| p.hit)
                    {
                        brain_hit = Some(hit);
                        continue;
                    }
                    if let Some(id) = self.segment_at(e.position.0, e.position.1) {
                        segment_next = Some(id);
                        continue;
                    }
                    // ON THE BAR, IT IS NOT A CLICK ON AN ITEM.
                    //
                    // It used to be: pressing over the bar scrolled nothing and
                    // RAN the command of the line behind it. Whoever wants to
                    // drag does not want to run anything, and the bar sits
                    // precisely over the right end of the items.
                    // A SECTION'S OWN BAR FIRST — the sidebar's and the facts'
                    // sit in their own columns, and the pointer test for the
                    // listing's does not know about them.
                    let section = self.files_bar_at_point(e.position.0, e.position.1);
                    if let Some((pane, axis)) = section {
                        self.dragging_section = Some((pane, axis));
                        self.drag_section_to(pane, axis, e.position.0, e.position.1);
                    } else if self.on_thumb(e.position.0, e.position.1) {
                        self.dragging = true;
                        self.drag_to(e.position.1);
                    } else if self
                        .files_target()
                        .is_some_and(|t| matches!(t.pane, crate::files::Pane::Preview))
                        && self
                            .files
                            .as_ref()
                            .is_some_and(crate::files::View::shape_here)
                    {
                        // A PRESS ON THE PIECE TAKES THE POINTER. After the
                        // bars, because a bar drawn over the panel is still a
                        // control; before the row, because a shape has no rows
                        // and the click would otherwise open the file — which
                        // is what pressing a picture does, and is exactly what
                        // somebody about to turn it does not want.
                        self.spinning = Some(e.position);
                    } else {
                        clicked = self.row_under_pointer;
                    }
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    self.dragging = false;
                    self.dragging_section = None;
                    self.spinning = None;
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
                    let line_px = f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE));
                    // THE HORIZONTAL AXIS COUNTS TOO. On a touchpad, turning
                    // a page sideways is the natural gesture; and the region
                    // has nowhere to scroll horizontally, so it competes with
                    // nothing.
                    self.detents +=
                        axis_detents(&vertical, line_px) + axis_detents(&horizontal, line_px);
                }
                _ => {}
            }
        }

        // THE BAND'S CLICK, before everything: it is the topmost region, and
        // like the modeline's it is decided by the `y`, so no two of these can
        // be true for one press.
        if let Some(hit) = band_hit {
            self.band_clicked(hit);
            return;
        }

        // AND THE BRAIN'S, on the same footing: it is decided by the `y` too,
        // so no two of these can be true for one press.
        if let Some(hit) = brain_hit {
            self.brain_clicked(hit);
            return;
        }

        // THE MODELINE'S CLICKS, before the region's: they are decided by the
        // `y`, so the two can never both be true for one press — and putting
        // them first keeps that obvious rather than incidental.
        if let Some(id) = segment_prev {
            self.segment_clicked(id, false);
            return;
        }
        if let Some(id) = segment_next {
            self.segment_clicked(id, true);
            return;
        }

        // THE RIGHT BUTTON FIRST: with both pressed in one frame, opening the
        // menu is the safer of the two — a left click on a listing can run
        // something.
        if menu {
            self.open_files_menu();
            return;
        }
        // A SHAPE TURNED, which is neither a drag along a track nor a click:
        // it has already changed the view, so all that is left is to draw it
        // and to tell the stage.
        if spun {
            self.follow_the_turn();
            self.ask_paint();
        }
        if let Some(i) = clicked {
            self.click_at(i);
            return;
        }

        // WHAT IS LEFT OF THE ACCUMULATOR STAYS. See the `detents` field: the
        // touchpad sends fractions, and spending only the whole part is what lets
        // the continuous gesture add up until it becomes a line.
        #[allow(clippy::cast_possible_truncation)]
        let whole = self.detents.trunc() as i32;
        if whole != 0 {
            self.detents -= f64::from(whole);
            // ONE detent = one page where it paginates, three lines where it
            // scrolls. The `page_by` already multiplies by the mode's step (which
            // is 1 on scrolling), so the difference fits in the factor.
            #[allow(clippy::cast_possible_truncation)]
            let step = match self.navigation() {
                Navigation::Page => 1,
                Navigation::Scroll => LINES_PER_DETENT as i32,
            };
            // THE SECTION UNDER THE POINTER TAKES IT FIRST.
            //
            // Only the listing's offset is the footer's — it is the one the
            // page keys and the scrollbar's drag move. A gesture over the
            // sidebar or over the facts belongs to that section, and until it
            // did the owner got all three moving at once: *"using the
            // touchpad's scroll gestures over the info region, EVERYTHING ends
            // up scrolling"*.
            let columns = self.screen_columns();
            let cell = self.cell_under_pointer.unwrap_or(0);
            // BELOW THE LISTING IS THE STRIP, and down there the same cell
            // belongs to a panel rather than to the listing — see
            // `Panes::section_at`. `row_at` counts from the top of the region,
            // so a row past the grid's lines is one of the strip's.
            let strip = self.files_strip();
            let in_strip = strip > 0
                && self
                    .row_under_pointer
                    .is_some_and(|r| r >= self.grid().lines);
            // AND EACH SECTION SCROLLS INSIDE ITS OWN HEIGHT, which is what
            // the window door already did and this one did not. The ceiling
            // was handed to EVERY pane, so a panel in the four-line strip was
            // paged twelve lines at a time — already wrong for the facts, and
            // it would have been wrong for the preview the same way.
            let ceiling = self.expansion_ceiling();
            let panel_room = if strip > 0 { strip } else { ceiling };
            if let Some(v) = self.files.as_mut() {
                let which = v.panes(columns).section_at(cell, columns, in_strip);
                let room = match which {
                    crate::files::Pane::Info | crate::files::Pane::Preview => panel_room,
                    _ => ceiling,
                };
                if v.wheel(which, i64::from(whole.saturating_mul(step)), columns, room) {
                    self.owes_paint = true;
                    return;
                }
            }
            if self.page_by(whole.saturating_mul(step)) {
                self.apply_geometry(self.interactivity());
                self.ask_paint();
                return;
            }
        }
        // It only redraws if something CHANGED: a `Motion` arrives on every pixel
        // of movement, and painting a frame per pixel would spend the GPU drawing
        // exactly the same thing.
        if segment_before != self.segment_under_pointer
            || before
                != (
                    self.under_pointer,
                    self.row_under_pointer,
                    self.cell_under_pointer,
                    self.current_offset(),
                )
        {
            // THE MEASUREMENT, when it is asked for.
            //
            // `RONOCTUA_MEASURE_HOVER=1` marks this frame, and `Gpu::present`
            // logs how long it took to reach `present` — OUR half of the
            // latency, with no compositor and no scanout in it. It is the same
            // milestone machinery `which-key` uses.
            //
            // Behind a variable because it would otherwise log a line per
            // pointer movement, and because the question it answers is asked
            // rarely: "is the delay ours or the chain's?". The nested loop puts
            // a whole extra compositor between the hand and the screen, in both
            // directions, and no number we can take from inside separates the
            // two — but this one bounds ours.
            if std::env::var_os("RONOCTUA_MEASURE_HOVER").is_some()
                && let Some(g) = self.screens.first_mut().and_then(|t| t.gpu.as_mut())
            {
                g.mark_as("hover", std::time::Instant::now());
            }
            self.ask_paint();
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
        let outputs: Vec<_> = self.outputs.outputs().collect();
        if outputs.is_empty() {
            tracing::debug!("no output yet; the surfaces wait");
            return;
        }
        let current_ones: Vec<_> = self.screens.iter().map(|t| t.output.clone()).collect();
        let (to_create, _) = screen_difference(&current_ones, &outputs);
        for i in to_create {
            self.create_screen(qh, outputs[i].clone());
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
            "layer-shell surface created, awaiting configure"
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
        let Ok(window) = id.parse::<u64>() else {
            tracing::warn!(item = id, "the finder's item is not a window id");
            return;
        };
        tracing::info!(window, "buffer chosen by the finder");
        // The SAME action the chord uses. Focus belongs to the compositor
        // (invariant 4), so the shell asks and does not move anything itself.
        self.niri_action(|n| n.action(serde_json::json!({ "FocusWindow": { "id": window } })));
    }

    /// Opens a file in the distro's editor, which is Emacs.
    ///
    /// # Why not one of ours
    ///
    /// There was one: a wasm buffer, `buffers/editor`. It existed for a single
    /// stated reason — `LEADER f f` promised to open a file and did nothing —
    /// and its own docstring said it was not Emacs and did not try to be. With
    /// `f f` opening the manager, that reason was gone, and what was left was a
    /// second editor nobody asked for on a system whose editor is Doom Emacs.
    ///
    /// `emacsclient` and not `emacs`, because `nix/modules/emacs.nix` runs a
    /// DAEMON: a second process would take seconds and would not see the
    /// buffers the session already has.
    fn open_editor(&mut self, file: &std::path::Path) {
        // FROM THE ENVIRONMENT, and not from an `option_env!`.
        //
        // Baked in, the path is a reference the binary carries into every
        // closure — so `ronoctua.editor.emacs = false` in the `minimal` profile
        // removed the package from `systemPackages` and left the whole of Emacs
        // in the system anyway. It is the trap the icon superpack already
        // sprang once, and `profile-minimal-closes` is what caught it a second
        // time. `nix/modules/emacs.nix` sets it, inside its own `mkIf`.
        //
        // IT MAY NOT BE THERE, and then a file that opens nothing has to SAY
        // so — silence reads as a key that stopped working.
        let Ok(client) = std::env::var("RONOCTUA_EMACSCLIENT") else {
            let m = self.texts.t(ronoctua_i18n::key::FILES_NO_EDITOR);
            self.echo_message(m);
            return;
        };
        tracing::info!(file = %file.display(), "opening in emacs");
        self.open_foreign(
            &client,
            &["-c".to_owned(), file.to_string_lossy().into_owned()],
            file.parent()
                .map(|d| d.to_string_lossy().into_owned())
                .as_deref(),
        );
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
                    .unwrap_or_else(|| format!("window {}", w.id)),
            })
            .collect()
    }

    /// The same, with the argument some compositor actions require.
    ///
    /// It arrives from outside — `ronoctua-shell command workspace-go 3` — because
    /// the keymap has no way of carrying a number: a sequence is a path to an
    /// id, not a call with a parameter.
    fn run_with(&mut self, id: &str, argument: Option<&str>) {
        // OUR OWN COMMANDS WITH AN ARGUMENT, by the same client and for the same
        // reason: the shape of the argument in the JSON is niri's, and assembling
        // it by hand would bring back the error the `Screenshot` cost.
        if let (Some(name), Some(arg)) = (crate::actions::with_argument(id), argument) {
            match ronoctua_niri::run_through_client(name, Some(arg)) {
                Ok(()) => {
                    tracing::info!(command = id, argument = arg, "an action with an argument")
                }
                Err(e) => {
                    let m = e.to_string();
                    tracing::warn!(command = id, error = %m, "the compositor refused");
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
                Ok(()) => tracing::info!(action = name, "a compositor action"),
                Err(e) => {
                    // NIRI'S TEXT, and not one of ours: it is the only clue to
                    // why the action was refused, and swapping it for "it failed"
                    // would wipe the information on the way.
                    let m = e.to_string();
                    tracing::warn!(action = name, error = %m, "the compositor refused");
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
                        tracing::error!(error = %reason, "restoring the keyboard failed");
                        self.texts.t_with(
                            ronoctua_i18n::key::ECHO_KEYBOARD_FAILED,
                            &[("reason", &reason)],
                        )
                    }
                };
                self.echo_message(m);
            }
            // THE KEYBOARD'S HALF OF THE CIRCLE ON THE BAND.
            //
            // Marking it read and taking it off the screen are ONE gesture:
            // read-but-still-there would be two states for one thing, and the
            // count would say nothing while the band still did.
            "notif-dismiss" => {
                let id = self.notifications.showing().map(|s| s.notification.id);
                if self
                    .notifications
                    .dismiss_current(std::time::Instant::now())
                    && let Some(id) = id
                {
                    ronoctua_notify::dbus::notification_was_closed(
                        id,
                        ronoctua_notify::dbus::Closed::ByHand,
                    );
                }
                self.after_the_queue_moved();
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
            "notif-history" => self.toggle_manager(),
            // THE SECOND DOOR, in a window of its own — the same `View`, and
            // the same reason the file manager has two: the footer's region has
            // a twelve-line ceiling (invariant 14), which is right for a footer
            // and not for reading a fortnight of history.
            "notif-window" => {
                if let Ok(me) = std::env::current_exe() {
                    self.open_foreign(&me.to_string_lossy(), &["notifications".to_owned()], None);
                }
            }
            "notif-read-all" => {
                let n = self.notifications.mark_all_read();
                tracing::info!(marked = n, "notifications marked as read");
                self.after_the_queue_moved();
            }
            // THE THREE THAT ACT ON THE MANAGER'S FOCUSED ROW.
            //
            // They go through the SAME `View::key` the keyboard does, rather
            // than reaching into the list here: a second path to "what is the
            // focused row" is a second answer, and the two doors would diverge
            // on the day one of them learned to sort differently.
            //
            // With the manager closed they do nothing, and that is the honest
            // answer — outside the view there is no focused row. It is the
            // shape `network-connect` already has.
            "notif-read" => self.manager_press(crate::notifications::Press::Space),
            "notif-forget" => self.manager_press(crate::notifications::Press::Letter('d')),
            "notif-act" => self.manager_press(crate::notifications::Press::Enter),
            // THE CLIPBOARD. Like the manual, it keeps the keyboard while it is
            // open: without that the arrows would go to the buffers.
            "clipboard-open" => {
                tracing::info!(inputs = self.clipboard.how_many(), "clipboard opened");
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
                        tracing::info!(bytes = text.len(), "recopied from the history");
                        self.echo_message(self.texts.t(ronoctua_i18n::key::ECHO_COPIED));
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "copying from the history FAILED");
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
            // THE LAUNCHER'S OWN KEY, and it only means anything with the
            // launcher open — which is where its key lives. Reached from `M-x`
            // with nothing focused it does nothing and says nothing, exactly
            // like `clipboard-pin` on an empty clipboard: the id exists so the
            // key is enumerable, and the action is where the state is.
            "apps-pin" => {
                let action = self
                    .apps
                    .as_ref()
                    .and_then(crate::apps::Finder::chosen)
                    .map(|e| crate::apps::Action::Pin(Box::new(e.clone())));
                if let Some(action) = action {
                    self.apply_app_action(action);
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
            "buffer-switch" => {
                let items = self.buffer_items();
                if items.is_empty() {
                    // Nothing to choose between. Opening an empty locator would
                    // be a list that says nothing and still takes the keyboard.
                    let m = self.texts.t(ronoctua_i18n::key::ECHO_NO_WINDOW);
                    self.echo_message(m);
                    return;
                }
                tracing::info!(buffers = items.len(), "buffer chooser opened");
                self.mx.open_over(items);
                self.first_page();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
            }
            // `LEADER .` — the file manager, in the footer's expanded region.
            "files-open" => self.open_files(),
            // `LEADER f m` — the same manager, in a window of its own.
            //
            // A WINDOW AND NOT A SECOND IMPLEMENTATION: it re-execs this binary
            // with `files`, and what draws there is the same `files::View` this
            // one uses. The footer's region has a twelve-line ceiling
            // (`space.expansion`, invariant 14) and that is right for a footer;
            // it is not right for the job people open a file manager to do.
            "files-window" => {
                if let Ok(me) = std::env::current_exe() {
                    let where_ = self
                        .files
                        .as_ref()
                        .map(|v| v.where_().to_string_lossy().into_owned());
                    let mut args = vec!["files".to_owned()];
                    args.extend(where_);
                    self.open_foreign(&me.to_string_lossy(), &args, None);
                }
            }
            // THE TRANSFER QUEUE. Like the clipboard, it keeps the keyboard
            // while it is open: without that the arrows would go to the
            // buffers and `C-c` would never reach the row it is aimed at.
            "transfers-open" => {
                let mut view = crate::transfers::View::fresh_one();
                // THE LIST IT ALREADY HAS, immediately. The daemon will send
                // another within the frame, but a panel that opens empty and
                // fills in a moment later reads as "nothing is running" at the
                // exact instant somebody opened it to check.
                view.refresh(self.jobs.clone());
                tracing::info!(jobs = self.jobs.len(), "transfers panel opened");
                self.transfers = Some(view);
                self.first_page();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
                self.draw_and_paint();
            }
            "apps-open" => {
                self.rescan_if_changed();
                tracing::info!(apps = self.catalogue.len(), "finder opened");
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
                        &[("network", &e.interface), ("rate", &e.rate())],
                    ),
                    Some(e) => self.texts.t_with(
                        ronoctua_i18n::key::ECHO_NETWORK_LINK,
                        &[
                            ("network", e.network.as_deref().unwrap_or(&e.interface)),
                            ("band", e.band.label()),
                            ("rate", &e.rate()),
                            ("signal", &format!("{}%", e.signal)),
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
                        &[("network", &target.ssid)],
                    ),
                    Some(Err(e)) => self.texts.t_with(
                        ronoctua_i18n::key::ECHO_NETWORK_FAILED,
                        &[("network", &target.ssid), ("reason", &e.to_string())],
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
                        &[("network", &target.ssid)],
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
                            ("connected", &e.connected.to_string()),
                            ("percent", &e.percent.to_string()),
                            ("up", &crate::network::readable_bytes(e.sent)),
                            ("down", &crate::network::readable_bytes(e.received)),
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
                    tracing::error!("RONOCTUA_XDG_OPEN was not set at build time");
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
                Err(e) => tracing::error!(error = %e, "I do not know my own binary"),
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
            // THE RESCUE CONSOLE — `Ctrl+Alt+Del`, and the only command here
            // that has a second way in. When this one cannot run, because the
            // compositor is what broke, systemd opens the same program on a VT.
            "rescue-open" => self.open_rescue(),
            "glyphs-nerd-fonts" => self.open_buffer("RONOCTUA_BUFFER_GLYPHS"),
            "unicode-symbols" => self.open_buffer("RONOCTUA_BUFFER_UNICODE"),
            "lucide-icons" => self.open_buffer("RONOCTUA_BUFFER_LUCIDE"),
            "memory-open" => self.open_buffer("RONOCTUA_BUFFER_MEMORY"),
            "brain-open" => self.open_buffer("RONOCTUA_BUFFER_BRAIN"),
            // THE COUNTDOWN'S TWO ANSWERS. They are the band's two controls and
            // they are reachable with no window open, which is the whole point:
            // the line appears on its own, and what answers it must not require
            // opening a screen first.
            "brain-abort" => self.brain_answer(false),
            "brain-proceed" => self.brain_answer(true),
            // `LEADER s u` — the whole sequence, now, outside any schedule.
            "system-update" => self.start_the_update(),
            // `LEADER s w` — the window that watches it, and the door the
            // modeline's mark opens. It re-execs this binary with `update`,
            // exactly as the file manager's second door does: what draws over
            // there is a window and a GPU, and what it decides is nothing.
            "update-watch" => {
                if let Ok(me) = std::env::current_exe() {
                    self.open_foreign(&me.to_string_lossy(), &["update".to_owned()], None);
                }
            }
            "transfers-window" => self.open_buffer("RONOCTUA_BUFFER_TRANSFERS"),
            "format-open" => self.open_buffer("RONOCTUA_BUFFER_FORMAT"),
            "transfers-add" => self.queue_the_clipboard(Wish::plain()),
            "transfers-video" => self.queue_the_clipboard(Wish::media(Want::Video)),
            "transfers-audio" => self.queue_the_clipboard(Wish::media(Want::Audio)),
            "transfers-subtitles" => self.queue_the_clipboard(Wish::media(Want::Subtitles)),
            "transfers-add-as-me" => self.queue_the_clipboard(Wish::plain().as_me()),
            "transfers-video-as-me" => {
                self.queue_the_clipboard(Wish::media(Want::Video).as_me());
            }
            "transfers-audio-as-me" => {
                self.queue_the_clipboard(Wish::media(Want::Audio).as_me());
            }
            "transfers-subtitles-as-me" => {
                self.queue_the_clipboard(Wish::media(Want::Subtitles).as_me());
            }
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
                tracing::warn!(command = otherwise, "a command with no registered action");
                let m = self.texts.t_with(
                    ronoctua_i18n::key::ECHO_NO_ACTION,
                    &[("command", otherwise)],
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
            tracing::warn!(error = %e, "the action on niri failed");
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
                tracing::error!(error = %e, "no scope name; buffer not opened");
                return;
            }
        };
        let me = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(error = %e, "I do not know my own binary");
                return;
            }
        };
        let taken = crate::foreign::taken_displays();
        let cmd = match crate::foreign::first_free_display(&taken) {
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
                tracing::warn!("no free display number; the buffer comes up with no satellite");
                crate::foreign::scope_command(&scope, working_directory, program, args)
            }
        };
        // The DIRECT command goes along: it is the one that runs if the scope
        // cannot be created. Rebuilding it in there would require taking the
        // `systemd-run` apart, and a new option would break the disassembly
        // silently.
        let mut direct = vec![program.to_owned()];
        direct.extend_from_slice(args);
        self.spawn_it(&scope, &direct, &cmd);
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
        let Some(sender) = self.catalogue_sender.clone() else {
            return;
        };
        tracing::info!("the application directories changed; rescanning");
        self.apps_stamp = now;
        crate::apps::scan(self.texts.language(), sender);
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
        let Some(queue) = self.icon_queue.clone() else {
            return;
        };
        if self.apps.is_none() {
            return;
        }
        let (offset, per_page) = (self.current_offset(), self.grid().per_page());
        let Some(l) = self.apps.as_ref() else {
            return;
        };
        let missing_ones: Vec<String> = (offset..offset + per_page)
            .filter_map(|i| l.at(i))
            .map(|e| e.icon.clone())
            .filter(|n| {
                !n.is_empty() && !self.icons.contains_key(n) && !self.icons_requested.contains(n)
            })
            .collect();
        let side = crate::apps::pixmap_side().ceil() as u32;
        for icon in missing_ones {
            // A closed queue means the thread died. Registering the request
            // anyway would leave the name stuck in "requested" forever, and
            // the line would never try again.
            if queue
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
        let found = p.rgba.is_some();
        // The `None` GOES INTO THE MAP. It is the answer "it does not exist", and
        // it is what stops the request from coming back on every frame.
        self.icons.insert(p.icon, p.rgba);
        // It only redraws when there is something new to show, and only with the
        // list open: an icon that arrives after the view closes must not cost a
        // frame.
        if found && self.apps.is_some() {
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

    /// One key for the transfer panel, and what it asked for.
    ///
    /// The view never opens a socket — it says what to send and this sends it.
    /// A failure to reach the queue lands on the message line rather than in
    /// the log: the panel is on screen, and a key that did nothing needs to say
    /// why where the person is looking.
    fn transfers_key(&mut self, key: ronoctua_keymap::Key) {
        let step = self
            .transfers
            .as_mut()
            .map_or(crate::transfers::Step::Nothing, |v| v.key(key));
        match step {
            crate::transfers::Step::Nothing => {}
            crate::transfers::Step::Changed => {
                self.follow_cursor();
                self.apply_geometry(KeyboardInteractivity::Exclusive);
                self.draw_and_paint();
            }
            crate::transfers::Step::Closed => {
                self.transfers = None;
                self.first_page();
                self.apply_geometry(self.interactivity());
                self.draw_and_paint();
            }
            crate::transfers::Step::Send(ask) => {
                self.tell_the_queue(&ask);
                self.draw_and_paint();
            }
        }
    }

    /// Sends one ask to the queue, saying so on the message line if it cannot.
    ///
    /// It is `tell` and not `ask`: nothing here needs an answer, and waiting
    /// for one would put a socket round trip inside the frame that draws. What
    /// comes back is the LIST, through the watching thread, which is the only
    /// thing that has to be right.
    fn tell_the_queue(&mut self, ask: &ronoctua_transfers::Ask) {
        if let Err(e) = ronoctua_transfers::client::tell(ask) {
            tracing::warn!(error = %e, ?ask, "the transfer queue did not answer");
            let said = self.texts.t(ronoctua_i18n::key::TRANSFERS_OFFLINE);
            self.echo_message(said);
        }
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
            // PINNED, AND THE LAUNCHER STAYS OPEN. Pinning is not opening.
            //
            // The preferences are read and written HERE rather than through a
            // `files::View` that may not exist: `LEADER a` is reachable without
            // the manager ever having been opened, and refusing to pin until it
            // has been would be a rule with nothing on screen to state it. The
            // manager notices on its own — it re-reads the file when its mtime
            // moves, and so does the window, which is another process entirely.
            Action::Pin(e) => {
                use ronoctua_files::sidebar::{Item, Target, store};
                // THE DIRECTORY IS FOUND HERE, once, and handed in — the
                // crate has no function that finds it on its own any more.
                // See `sidebar::store` for the defect that removed them.
                let Some(dir) = store::config_dir() else {
                    return;
                };
                let mut prefs = store::read_from(&dir);
                let id = format!("app:{}", e.id);
                if prefs.added.iter().any(|i| i.id == id) {
                    let m = self.texts.t_with(
                        ronoctua_i18n::key::FILES_SIDEBAR_PINNED,
                        &[("name", e.name.as_str())],
                    );
                    self.echo_message(m);
                    self.draw_and_paint();
                    return;
                }
                prefs.added.push(Item {
                    id,
                    // THE ID, and never the command: a store path names a build
                    // that the next collection removes, and an id survives every
                    // rebuild. The NAME is stored because it is the one label
                    // that cannot be derived from the target.
                    target: Target::Application(e.id.clone()),
                    label: Some(e.name.clone()),
                });
                let m = match store::write_to(&dir, &prefs) {
                    Ok(()) => self.texts.t_with(
                        ronoctua_i18n::key::FILES_SIDEBAR_PINNED,
                        &[("name", e.name.as_str())],
                    ),
                    Err(err) => self.texts.t_with(
                        ronoctua_i18n::key::FILES_SIDEBAR_UNWRITABLE,
                        &[("why", err.to_string().as_str())],
                    ),
                };
                self.echo_message(m);
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
            tracing::info!(app = %e.id, program, "opening the application");
            self.open_foreign(program, args, e.working_directory.as_deref());
            return;
        }
        let Ok(me) = std::env::current_exe() else {
            tracing::error!("I do not know my own binary");
            return;
        };
        let mut inside = vec!["terminal".to_owned(), program.clone()];
        inside.extend_from_slice(args);
        tracing::info!(app = %e.id, program, "opening the application IN the terminal");
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
    fn spawn_it(&mut self, scope: &str, direct: &[String], cmd: &[String]) {
        let Some(sender) = self.launch_sender.clone() else {
            // With no channel there is no way of reporting, and launching blind would
            // hide the error. It only happens outside the event loop (tests).
            tracing::error!("no launch channel; buffer not opened");
            return;
        };
        let (scope, direct, cmd) = (scope.to_owned(), direct.to_vec(), cmd.to_vec());
        // THE THREAD'S NAME shows up in `perf` and in `gdb`. An anonymous thread
        // stuck in `wait4` for hours is exactly what nobody identifies.
        let born = std::thread::Builder::new()
            .name("ronoctua-lancar".to_owned())
            .spawn(move || {
                let verdict = crate::foreign::launch(&scope, &direct, &cmd);
                // A closed channel = the footer left. Nothing to report.
                let _ = sender.send(verdict);
            });
        if let Err(e) = born {
            tracing::error!(error = %e, "could not create the launch thread");
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
            tracing::debug!(error = %e, "no scan watcher; the list only rereads on request");
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
                tracing::info!(scope, program, "foreign buffer opened");
            }
            Launch::Orphan { program, reason } => {
                tracing::warn!(program, reason, "opened WITHOUT a cgroup of its own");
            }
            Launch::Failed { program, error } => {
                tracing::error!(error, program, "could not open");
                let m = self.texts.t_with(
                    ronoctua_i18n::key::ECHO_NO_OPENED,
                    &[("program", program.as_str())],
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
        self.state_pending = false;
        self.state_menu.clear();

        let Some(pid) = self.niri.focused().and_then(|w| w.pid) else {
            return;
        };
        let Ok(pid) = u32::try_from(pid) else { return };
        match crate::leader::buffer_keymap(pid) {
            Ok(bindings) if !bindings.is_empty() => {
                tracing::debug!(
                    pid,
                    bindings = bindings.len(),
                    "the focused buffer's keymap"
                );
                self.buffer_pid = Some(pid);
                self.buffer_bindings = bindings;
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "could not read the buffer's keymap"),
        }

        // THE STATES, from the same socket and in the same breath.
        //
        // `SPC b s` only exists with MORE THAN ONE: a submenu offering the one
        // state the buffer is already in is a key that teaches there is a choice
        // and then makes none — the same reason a window that publishes no
        // keymap has no `SPC m` row at all.
        if let Ok(Some((now, listed))) = crate::leader::buffer_states(pid) {
            self.states.insert(pid, now);
            self.buffer_pid = Some(pid);
            if listed.len() > 1 {
                self.state_menu = listed;
            }
        }
    }

    /// The which-key rows for the focused buffer's states.
    ///
    /// The label is the SAME key the modeline draws — `state-<id>` — so the list
    /// and the segment can never disagree about what a state is called, in
    /// either language.
    fn state_inputs(&self) -> Vec<ronoctua_keymap::Entry> {
        self.state_menu
            .iter()
            .filter_map(|(id, letter)| {
                let typed = letter.to_string();
                let sym = ronoctua_keymap::Key::parse(&typed).ok()?;
                Some(ronoctua_keymap::Entry {
                    key: typed,
                    sym,
                    label: format!("state-{id}"),
                    command: Some(id.clone()),
                    is_group: false,
                })
            })
            .collect()
    }

    /// Handles a key inside `SPC b s`, and says whether it did.
    ///
    /// Mirrors [`Self::buffer_key`], and for the same reason: the rows are built
    /// at runtime from what the focused window published, so they cannot live in
    /// `ronoctua_keymap::Tree`, whose bindings are `&'static str`.
    fn state_key(&mut self, key: ronoctua_keymap::Key) -> bool {
        let typed = ronoctua_keymap::notation(&[key]);

        if !self.state_pending {
            // Opening it: exactly `b` typed so far, `s` now, and a buffer with
            // more than one state in focus.
            let after_b = ronoctua_keymap::notation(self.leader.typed()) == "b";
            if !(after_b && typed == "s" && !self.state_menu.is_empty()) {
                return false;
            }
            self.state_pending = true;
            self.inputs = self.state_inputs();
            self.first_page();
            self.echo = None;
            self.apply_geometry(KeyboardInteractivity::Exclusive);
            return true;
        }

        self.state_pending = false;
        // `ESC` LEAVES, as it does at every level of the leader — section 7.4
        // wants an exit that works without the person knowing where they are.
        if typed == "ESC" {
            self.close_leader();
            return true;
        }
        let picked = self
            .state_menu
            .iter()
            .find(|(_, letter)| letter.to_string() == typed)
            .map(|(id, _)| id.clone());
        self.close_leader();
        match (picked, self.buffer_pid) {
            (Some(id), Some(pid)) => {
                match crate::leader::buffer_state(pid, &format!("state {id}")) {
                    Ok(()) => tracing::info!(state = %id, "state chosen by the leader"),
                    Err(e) => {
                        tracing::warn!(error = %e, state = %id, "the buffer refused the state")
                    }
                }
            }
            // A key that is not one of the letters ends the sequence, like any
            // other key that leads nowhere.
            _ => tracing::debug!(key = %typed, "a key outside the buffer's states"),
        }
        true
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
                Some(Ok(())) => tracing::info!(command = %id, "command of the focused buffer"),
                Some(Err(e)) => tracing::warn!(error = %e, command = %id, "the buffer refused"),
                None => tracing::warn!(command = %id, "no buffer to take the command"),
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
        let mut n = ronoctua_notify::Notification::fresh_one(
            0,
            self.texts.t(ronoctua_i18n::key::NOTIF_OOM_APP),
            None,
            self.texts
                .t(ronoctua_i18n::key::NOTIF_MEMORY_KILLED_SUMMARY),
            body,
            ronoctua_notify::Urgency::Critical,
        );
        // THE TAG IS HOW THE CATALOGUE RECOGNISES IT.
        //
        // This used to be the one notification in the system with a code path
        // of its own: its own urgency, its own never-expires, its own three
        // text keys. Now it is a row in `assets/notifications.nix` like the
        // others, and what says which row is the tag — the same mechanism a
        // notification arriving from the bus goes through.
        n.tags = vec![MEMORY_KILLED.to_owned()];
        n
    }

    /// Receives a notification: decide, rewrite if it is ours, queue, record.
    ///
    /// # Why the shell decides nothing here
    ///
    /// Every question about how it appears — whether at all, for how long, how
    /// loudly, with which glyph — is answered by `Book::decide`, out of
    /// `assets/notifications.nix` and the person's own filters. What is left
    /// here is the two things only the shell can do: put OUR sentence on a
    /// notification that arrived in somebody else's language, and hand it to
    /// the queue.
    fn notification_of(&mut self, e: ronoctua_notify::Event) {
        let now = std::time::Instant::now();
        match e {
            ronoctua_notify::Event::New(n) => self.arrived(*n, now),
            ronoctua_notify::Event::Close(id) => {
                if self.notifications.dismiss(id, now) {
                    ronoctua_notify::dbus::notification_was_closed(
                        id,
                        ronoctua_notify::dbus::Closed::ByRequest,
                    );
                }
            }
            // THE MACHINE CLOSED AN APPLICATION SO IT WOULD NOT FREEZE.
            //
            // It goes through the same door as everything else. What makes it
            // loud and long is the catalogue's `memory-killed` row, and not a
            // branch here — so the day the owner wants it quieter, the change
            // is one line of a table.
            ronoctua_notify::Event::OomKilled { cgroup, reason } => {
                let n = self.oom_notification(&cgroup, &reason);
                self.arrived(n, now);
            }
        }
        self.after_the_queue_moved();
    }

    /// One notification, from arrival to the band.
    fn arrived(&mut self, mut n: ronoctua_notify::Notification, now: std::time::Instant) {
        let p = self.book.decide(&n);
        // OUR SENTENCE, on a notification that arrived in English.
        //
        // The compositor's screenshot is the case this exists for: upstream
        // sends "Screenshot captured", it is not translated and never will be,
        // and invariant 8 wants both languages. The entry says `rewrite` and
        // the text comes from our keys.
        if let Some(e) = p.catalogue.as_deref().and_then(|id| self.book.entry(id)) {
            if e.rewrite {
                n.summary = self.texts.t(&e.summary_key());
                n.body = self.texts.t(&e.body_key());
            }
            // The catalogue's actions REPLACE whatever came on the wire: ours
            // know what to run, and a foreign app's would need a bus round trip
            // for a notification the compositor is not listening about.
            if !e.actions.is_empty() {
                n.actions = usable_actions(&e.actions, &n);
            }
            n.tags.extend(e.tags.iter().cloned());
            n.tags.dedup();
        }
        tracing::info!(
            id = n.id,
            app = %n.app,
            catalogue = p.catalogue.as_deref().unwrap_or("-"),
            rule = p.by.as_deref().unwrap_or("-"),
            show = p.show.id(),
            life = p.life.id(),
            level = p.level,
            "notification decided"
        );
        self.notifications.enqueue(n, p, now);
    }

    /// The queue moved: log it, save the history, resize and paint.
    fn after_the_queue_moved(&mut self) {
        // The queue's state on ONE greppable line. The modeline's segment also
        // shows it, but it starts with a PUA glyph — a grep for `text= 2` does
        // not match, because between the `=` and the space there is a glyph.
        tracing::info!(
            showing = self.notifications.showing().map(|s| s.notification.id),
            pending = self.notifications.pending(),
            unread = self.notifications.unread(),
            band = self.band_rows(),
            loud = self.notifications.has_loud(),
            "notification queue"
        );
        self.save_history();
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// Writes the history to disk, pruned to fifteen days.
    ///
    /// A failure is logged and swallowed: a read-only home or a full disk is not
    /// a reason for the desktop to stop working, and the alternative — the shell
    /// coming down over a file it writes for convenience — is worse than losing
    /// the record.
    fn save_history(&mut self) {
        if let Err(e) = self.notes.save_history(self.notifications.history_mut()) {
            tracing::warn!(error = %e, "could not write the notification history");
        }
    }

    /// Recomputes the layout and paints. The pair shows up in every event source.
    fn draw_and_paint(&mut self) {
        // WHAT THE PANELS NEED, asked ONCE PER FRAME and in one place.
        //
        // Every other arrangement was a list of the ways the cursor can move —
        // a key, a click, a re-read, a directory entered — and the one that got
        // forgotten would be a panel showing the previous file with nothing
        // saying why. `wants_look` answers `None` when the answer is already in
        // hand, so this costs a comparison of two strings.
        self.look_again();
        self.draw();
        self.paint();
    }

    /// Asks the inspector about the file under the manager's cursor.
    ///
    /// A no-op when there is no manager, when neither panel is on, or when the
    /// same file has already been asked about — all three decided by
    /// `files::View::wants_look`, which is the one place that knows.
    fn look_again(&mut self) {
        if let Some(queue) = self.files_looking.as_ref()
            && let Some(view) = self.files.as_mut()
            && let Some(ask) = view.wants_look(u32::from(tokens::space::FILES_PANELPIXELS))
        {
            let _ = queue.send(ask);
        }
        self.follow_the_motion();
        self.follow_the_turn();
    }

    /// Starts, or stops, the player that follows a moving file.
    ///
    /// The window door's twin, and asked in the same place for the same
    /// reason: the cursor can move a dozen ways, and enumerating them is how a
    /// case gets missed.
    fn follow_the_motion(&mut self) {
        let Some(player) = self.files_playing.as_mut() else {
            return;
        };
        let Some(view) = self.files.as_mut() else {
            return;
        };
        // ONE QUESTION, ONE ANSWER, PASSED STRAIGHT ON. The view decides what
        // should be running by comparing values; this door does not know what
        // changed and does not need to.
        if let Some(ask) = view.wants_motion() {
            player.tell(ask);
        }
    }

    /// One key, however it arrived — pressed, or repeated by the clock.
    ///
    /// One body and not two: SCTK delivers a repeat through the CALLBACK given
    /// to `get_keyboard_with_repeat` and not through `repeat_key`, and a repeat
    /// that took a different path is a key that does something slightly
    /// different the second time round.
    fn typed(&mut self, event: KeyEvent) {
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

        // THE TRANSFER PANEL intercepts for the same reason: with it open the
        // whole keyboard is its, and `C-c` gives a transfer up rather than
        // reaching whatever `C-c` would otherwise mean.
        if self.transfers.is_some() {
            self.transfers_key(key);
            return;
        }

        // THE NOTIFICATION MANAGER intercepts for the same reason as the other
        // modals: with it open the whole keyboard is its, and `r` marks
        // everything read rather than firing `SPC r`.
        if self.notifications_open {
            self.manager_key(key);
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

        // THE FILE MANAGER intercepts before the leader, and before the rest of
        // the modals for the same reason `view()` ranks it first: it holds a
        // directory somebody navigated to, a set of marks and possibly an armed
        // confirmation, and a key that fell through to the keymap would end the
        // sequence and take all of it with it.
        //
        // It is intercepted here and not through a table of its own because its
        // keys are NOT commands of the registry — they navigate a listing. The
        // table that a check reads is `files::OF_THE_VIEW`, and the unit test
        // beside it presses every key in it.
        if self.files.is_some() {
            let step = self
                .files
                .as_mut()
                .map_or(crate::files::Step::Nothing, |v| v.key(key));
            self.apply_files_step(step);
            return;
        }

        // THE MANUAL intercepts before everything else: with it open the arrows
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
                    tracing::info!(command = id, "command run by the finder");
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
                    tracing::info!(command = id, argument = %arg, "a command with an argument");
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
        // THE STATE SUBMENU, before the buffer's keymap and before the tree.
        //
        // Before the tree because `b s` is not in it — the rows come from what
        // the focused window declared. Before `buffer_key` because that one only
        // opens at the ROOT of the sequence, and this one opens after `b`; the
        // order costs nothing and keeps each check about its own level.
        if self.state_key(key) {
            self.draw_and_paint();
            return;
        }
        if self.buffer_key(key) {
            self.draw_and_paint();
            return;
        }

        let effect = self.leader.key(&self.tree, key);
        self.apply_leader_effect_with(effect, Some(&attempt), key.sym);
        self.draw_and_paint();
    }

    /// Starts, or stops, the thread that draws a shape.
    ///
    /// The twin of [`Footer::follow_the_motion`], asked in the same places and
    /// for the same reason: the pose changes from a key, from the wheel, from
    /// a drag and from the region being resized, and enumerating those is how
    /// one of them gets missed.
    fn follow_the_turn(&mut self) {
        let Some(stage) = self.files_turning.as_mut() else {
            return;
        };
        let Some(view) = self.files.as_mut() else {
            return;
        };
        if let Some(ask) = view.wants_turn() {
            stage.tell(ask);
        }
    }

    /// Owes a paint, to happen once the event queue is empty.
    ///
    /// For the bursts — pointer motion, the wheel, a held-down arrow — where
    /// painting per event means blocking per event. See `Footer::owes_paint`.
    const fn ask_paint(&mut self) {
        self.owes_paint = true;
    }

    /// Activates the leader: asks for the keyboard and expands.
    ///
    /// The keyboard request is `Exclusive` on purpose. `OnDemand` would give
    /// focus only on a click, and the footer has nothing clickable.
    fn activate_leader(&mut self) {
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
        self.leader.activate();
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
        tracing::info!(inputs = self.inputs.len(), "leader active");
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
            tracing::debug!(bytes = bytes.len(), "the copied image did not decode");
            return;
        };
        tracing::debug!(
            width = r.width,
            height = r.height,
            ms = start.elapsed().as_secs_f64() * 1000.0,
            "preview decoded"
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
    /// `yes` reconnects with `psk-flags=1` and seals the password in the TPM;
    /// any other answer leaves it as it is — `psk-flags=2`, nothing on disk.
    ///
    /// The `(ssid, password)` pair comes out of the state with `take()`
    /// **before** anything can fail: a password left behind after an error
    /// would be a password in memory with no owner and no deadline.
    fn answer_remember(&mut self, yes: bool) {
        let Some((ssid, password)) = self.network_remember.take() else {
            return;
        };
        let m = if yes {
            match self
                .network
                .as_mut()
                .map(|v| v.connect(&ssid, Some(&password), true))
            {
                Some(Ok(())) => self.texts.t_with(
                    ronoctua_i18n::key::ECHO_NETWORK_STORED,
                    &[("network", &ssid)],
                ),
                Some(Err(e)) => self.texts.t_with(
                    ronoctua_i18n::key::ECHO_NETWORK_STORE_FAILED,
                    &[("reason", &e.to_string())],
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

    /// Opens or closes the notification manager.
    ///
    /// It uses THE surface, in the expanded region — below the modeline +
    /// minibuffer pair. There is no second surface anywhere in the system
    /// (invariant 14).
    ///
    /// # Why it takes the keyboard now, when the history did not
    ///
    /// The history was passive: it was a list one read, so it gave the keys to
    /// whoever wanted them. The manager is not read, it is USED — the arrows
    /// move a cursor, `TAB` changes pane, letters run actions — and a view with
    /// a cursor nobody can move is a view that only the mouse reaches, which is
    /// the one thing this footer refuses.
    fn toggle_manager(&mut self) {
        self.notifications_open = !self.notifications_open;
        if self.notifications_open {
            self.manager = crate::notifications::View::fresh();
        }
        self.first_page();
        tracing::info!(
            open = self.notifications_open,
            unread = self.notifications.unread(),
            lines = self.expansion_len(),
            pages = self.pages(),
            "notification manager"
        );
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// Sends one press to the manager, if it is open.
    ///
    /// The door the registry's commands come in by — see the `notif-read` arm
    /// of `run_command` for why they go through `View::key` rather than
    /// reaching into the list.
    fn manager_press(&mut self, press: crate::notifications::Press) {
        if !self.notifications_open {
            return;
        }
        let records = self.notifications.history().to_vec();
        let errand = {
            let lists = crate::notifications::Lists::of(&records, &self.book);
            self.manager.key(press, &lists, &self.book)
        };
        self.manager_errand(errand);
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// One key, for the notification manager.
    ///
    /// The translation from `Sym` to [`crate::notifications::Press`] is the
    /// ONLY part of the manager that needs a seat; everything below it is
    /// arithmetic over a list, and that is what lets the whole thing be driven
    /// by a test with no compositor.
    fn manager_key(&mut self, key: ronoctua_keymap::Key) {
        use crate::notifications::Press;
        use ronoctua_keymap::Sym;
        let press = match key.sym {
            Sym::Up => Press::Up,
            Sym::Down => Press::Down,
            Sym::Left => Press::Left,
            Sym::Right => Press::Right,
            Sym::Tab => Press::Tab,
            Sym::Enter => Press::Enter,
            Sym::Space => Press::Space,
            Sym::Esc => Press::Escape,
            Sym::Char(c) => Press::Letter(c),
            _ => return,
        };
        let records = self.notifications.history().to_vec();
        let lists = crate::notifications::Lists::of(&records, &self.book);
        let errand = self.manager.key(press, &lists, &self.book);
        drop(lists);
        self.manager_errand(errand);
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// Carries out what the manager asked for.
    fn manager_errand(&mut self, errand: crate::notifications::Errand) {
        use crate::notifications::Errand;
        match errand {
            Errand::Nothing => {}
            Errand::Read(id) => {
                if self.notifications.mark_read(id) {
                    // The one on the band goes too: marking it read and leaving
                    // it on screen would be two states for one thing.
                    if self
                        .notifications
                        .showing()
                        .is_some_and(|s| s.notification.id == id)
                    {
                        self.notifications
                            .dismiss_current(std::time::Instant::now());
                    }
                    ronoctua_notify::dbus::notification_was_closed(
                        id,
                        ronoctua_notify::dbus::Closed::ByHand,
                    );
                }
                self.after_the_queue_moved();
            }
            Errand::ReadAll => {
                self.notifications.mark_all_read();
                self.after_the_queue_moved();
            }
            Errand::Forget(id) => {
                if self.notifications.forget(id) {
                    tracing::info!(id, "notification forgotten");
                }
                self.after_the_queue_moved();
            }
            Errand::Act(id, which) => self.run_notification_action(id, which),
            Errand::Adjusted(id, verdict) => {
                match verdict {
                    Some(v) => self.book.adjustments.insert(id.clone(), v),
                    None => self.book.adjustments.remove(&id),
                };
                let prefs = ronoctua_notify::Preferences {
                    rules: self.book.mine.clone(),
                    adjustments: self.book.adjustments.clone(),
                };
                if let Err(e) = self.notes.save_preferences(&prefs) {
                    tracing::warn!(error = %e, "could not write the preferences");
                    self.echo_message(self.texts.t(ronoctua_i18n::key::ECHO_COPY_FAILED));
                }
                tracing::info!(id = %id, "notification preference saved");
                self.draw_and_paint();
            }
            Errand::Close => self.toggle_manager(),
        }
    }

    /// Runs action `which` of the notification with this id.
    ///
    /// # Why it looks the notification up again
    ///
    /// Because the manager holds no records — it holds a cursor. What it hands
    /// over is an id and an index, and the record is found here, where the
    /// queue is. A manager carrying its own copy is the second copy that goes
    /// stale, and this file has already paid for one.
    fn run_notification_action(&mut self, id: u32, which: usize) {
        let Some(record) = self
            .notifications
            .history()
            .iter()
            .find(|r| r.notification.id == id)
        else {
            return;
        };
        let Some(action) = record.notification.actions.get(which).cloned() else {
            return;
        };
        let file = record.notification.file();
        match &action.doing {
            // A FOREIGN APP'S BUTTON goes back to the bus, which is what the
            // sender is waiting for.
            ronoctua_notify::Doing::Invoke => {
                ronoctua_notify::dbus::action_was_invoked(id, &action.id);
            }
            ronoctua_notify::Doing::Command(cmd) => self.run_command(&cmd.clone()),
            ronoctua_notify::Doing::CopyPath => {
                let Some(f) = file else { return };
                let text = f.to_string_lossy().into_owned();
                match crate::buffer::copy_to_clipboard(&text) {
                    Ok(()) => self.echo_message(self.texts.t(ronoctua_i18n::key::ECHO_COPIED)),
                    Err(e) => {
                        tracing::error!(error = %e, "copying the path FAILED");
                        self.echo_message(self.texts.t(ronoctua_i18n::key::ECHO_COPY_FAILED));
                    }
                }
            }
            ronoctua_notify::Doing::Run { program, args } => {
                let Some(path) = crate::actions::notification_program(*program) else {
                    // It should not get here: an action whose program is
                    // missing is not offered — see `usable_actions`. The guard
                    // is for the day the profile changes under a running shell.
                    tracing::warn!(
                        program = program.id(),
                        "the action's program is not installed"
                    );
                    return;
                };
                let mut argv: Vec<String> = Vec::new();
                for a in args {
                    match a {
                        ronoctua_notify::action::Arg::Literal(s) => argv.push(s.clone()),
                        ronoctua_notify::action::Arg::File => {
                            let Some(f) = &file else { return };
                            argv.push(f.to_string_lossy().into_owned());
                        }
                        ronoctua_notify::action::Arg::Folder => {
                            let Some(f) = file.as_ref().and_then(|f| f.parent()) else {
                                return;
                            };
                            argv.push(f.to_string_lossy().into_owned());
                        }
                    }
                }
                tracing::info!(program = %path, ?argv, "a notification action");
                self.open_foreign(&path, &argv, None);
            }
        }
        // Acting on it is having seen it.
        self.notifications.mark_read(id);
        self.after_the_queue_moved();
    }

    /// Puts a message in the echo area.
    /// Puts whatever was last copied into the queue.
    ///
    /// # Why the clipboard and not a field to type into
    ///
    /// Because an address is never typed. It is copied — from the browser's own
    /// bar, from a chat window, from a page — and the gesture that follows is
    /// already `Ctrl+C`. A field would ask the person to paste into it, which
    /// is the same paste with one more step and a syntax to get wrong.
    ///
    /// The [`Wish`] says what to do with a page and whose session to use. A
    /// magnet ignores both: it is not a page, and a swarm has no account.
    fn queue_the_clipboard(&mut self, wish: Wish) {
        let latest = self
            .clipboard
            .at(0)
            .and_then(|e| e.text())
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(ToOwned::to_owned);
        let Some(address) = latest else {
            self.echo_message(self.texts.t(ronoctua_i18n::key::TRANSFERS_NOTHING_COPIED));
            return;
        };
        // A SANITY CHECK AND NOT A PARSER: what decides the engine is
        // `where_to::work_for`, in the queue's crate, where every other door
        // asks the same question. This only refuses what is plainly not an
        // address, so that copying a paragraph and pressing the key gives a
        // sentence rather than a failed job.
        let plausible = address.contains("://") || address.starts_with("magnet:");
        if !plausible || address.contains(char::is_whitespace) {
            self.echo_message(self.texts.t(ronoctua_i18n::key::TRANSFERS_NOT_AN_ADDRESS));
            return;
        }
        let work = ronoctua_transfers::where_to::work_for(
            &address,
            ronoctua_transfers::where_to::landing(),
            wish,
        );
        match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
            Ok(()) => {
                // WHICH OF THE TWO HAPPENED, said at the moment it happened.
                // The row carries it afterwards, but the line that matters is
                // this one: it is the only chance to notice that this download
                // was not anonymous before it has already gone out.
                let message = self.texts.t(if wish.identity == Identity::Mine {
                    ronoctua_i18n::key::TRANSFERS_QUEUED_AS_ME
                } else {
                    ronoctua_i18n::key::TRANSFERS_QUEUED
                });
                self.echo_message(message);
            }
            Err(e) => {
                tracing::error!(error = %e, "the queue refused the address");
                let message = self.texts.t(ronoctua_i18n::key::TRANSFERS_OFFLINE);
                self.echo_message(message);
            }
        }
    }

    fn echo_message(&mut self, text: String) {
        tracing::info!(echo = %text, "echo area");
        self.echo = Some((
            text,
            std::time::Instant::now() + std::time::Duration::from_secs(ECHO_SECONDS),
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
        tracing::info!("leader closed");
        // IT GIVES THE KEYBOARD BACK ONLY IF NOBODY ELSE WANTS IT.
        //
        // The sequence that OPENS the `M-x` ends here: running the command
        // ends the leader. Giving it back unconditionally took the keyboard
        // away from the prompt that had just asked for it.
        self.apply_geometry(self.interactivity());
    }

    /// Which view is in the expanded region right now.
    ///
    /// The count used to be written into this sentence, and it rotted: this
    /// line said EIGHT, the network's comment below said seven, and
    /// [`ViewState`] has nine of them plus `NoneOf`. A number in prose is a
    /// second copy of something the enum already says exactly, so there is no
    /// number here any more.
    ///
    /// **The precedence order is written HERE, and only here.** Whoever is in
    /// the middle of something wins: the four modals captured the whole
    /// keyboard, then comes the leader (a sequence under way), and the history
    /// is passive and yields to all of them. See [`ViewState`].
    const fn view(&self) -> ViewState {
        if self.clipboard_open {
            ViewState::Clipboard
        } else if self.transfers.is_some() {
            ViewState::Transfers
        } else if self.manual {
            ViewState::Manual
        } else if self.apps.is_some() {
            ViewState::Applications
        } else if self.network.is_some() {
            ViewState::Network
        } else if self.files.is_some() {
            // ABOVE THE LOCATOR AND THE LEADER, because it holds state a
            // person built by hand: a directory navigated to, a set of marks,
            // an armed confirmation, a question half answered. Every other view
            // can be reopened in a keystroke.
            //
            // It does not have to yield to the locator, and that is the point
            // of the picker living inside it: asking which owner, which group,
            // which permission draws in the manager's own region and answers
            // there. Two pickers would be two behaviours for one gesture.
            ViewState::Files
        } else if self.mx.active() {
            ViewState::Commands
        } else if self.leader.active_one() {
            ViewState::Leader
        } else if self.notifications_open {
            ViewState::Notifications
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

    /// How many lines the notification band takes right now.
    ///
    /// Zero at rest, one for a brief notification, up to three for a full one.
    /// The number comes from [`crate::band::rows`] and NOT from the width, so
    /// the footer's height stays the same on every output — see
    /// [`Footer::apply_geometry`] for why that matters.
    fn band_rows(&self) -> u16 {
        crate::band::rows(self.notifications.showing()).min(BAND_CEILING)
    }

    /// A sequence as the person would type it, leader included.
    ///
    /// The trigger's own notation comes from the ENVIRONMENT (`crate::leader`):
    /// in the nested loop it is not `SPC`, and a line that said `SPC` there
    /// would be teaching a key that does not exist.
    fn brain_keys(&self, sequence: &str) -> String {
        format!("{} {sequence}", self.leader_label)
    }

    /// What the brain has announced, when it has announced anything.
    fn announced(&self) -> Option<&ronoctua_brain::Announcement> {
        self.brain.as_ref()?.announcement.as_ref()
    }

    /// How many lines the brain's line takes right now.
    ///
    /// One while a countdown is running, zero otherwise — and never a function
    /// of the width, for the reason [`crate::brain::rows`] gives.
    /// How many lines the brain's line takes: one, or none.
    ///
    /// `brain_rows` AND NOT `brain_line`, which is what it was called. Its
    /// twin `brain_line_at` returns the row the line sits ON, and a pair where
    /// one name reads as an index and returns a count is the exact confusion
    /// `first_of_expansion` already carries a paragraph about. `band_rows` next
    /// door answers the same question about the band, and now they read alike.
    fn brain_rows(&self) -> u16 {
        crate::brain::rows(self.announced())
    }

    /// Which line of the surface the modeline is on.
    ///
    /// It used to be zero, and a constant. The band grows ABOVE it, so it moves
    /// — but the surface is anchored at the bottom, so the modeline does not
    /// move on the SCREEN. That is the whole point of the band being up there
    /// rather than on the modeline itself.
    fn modeline_line(&self) -> u16 {
        self.band_rows()
    }

    /// Which line of the surface the minibuffer is on. Glued under the modeline.
    fn minibuffer_line(&self) -> u16 {
        self.band_rows() + 1
    }

    /// Which line of the surface the brain's is on, when it is on one.
    ///
    /// GLUED UNDER THE MINIBUFFER, and not the last line of the surface. Three
    /// reasons, and `crate::brain`'s header argues all three — the one that
    /// decides it is that a truly-last line would live at
    /// `first_of_expansion() + expansion_rows()` and would have to be worked
    /// out again in the painter, in the hit test and in the background quad,
    /// which is exactly the defect [`Self::first_of_expansion`] exists to
    /// remove.
    fn brain_line_at(&self) -> u16 {
        self.band_rows() + FOOTER_LINES
    }

    /// The first line of the expanded region.
    ///
    /// The number used to be a constant and appeared in the drawing and again
    /// where the pointer works out which item it is over — two calculations of
    /// the same thing, and the one that gets it wrong is the one nobody looks
    /// at: the cursor describes the line above.
    fn first_of_expansion(&self) -> u16 {
        self.band_rows() + FOOTER_LINES + self.brain_rows()
    }

    /// The height wanted right now, in px.
    ///
    /// Two lines at rest; plus the notification band above, plus one per
    /// which-key entry below. The list stays BELOW the minibuffer — it is a
    /// vertico, not a menu that pops up (see `CLAUDE.md`).
    fn wanted_height(&self) -> u32 {
        // Which-key, the manager and `M-x` use THE SAME expanded area, and
        // never two at the same time: they are uses of one surface (invariant
        // 14), not several surfaces.
        //
        // The precedence order is that of whoever is in the middle of
        // something: the `M-x` captured the whole keyboard, so it wins; then
        // the leader, which is a sequence under way; the manager is passive and
        // yields to both.
        //
        // The band is added and not ordered against them: it is not a use of
        // the expanded region, it is a region of its own, above the pair.
        let lines = u32::from(FOOTER_LINES + self.band_rows() + self.brain_rows())
            + u32::try_from(self.expansion_rows()).unwrap_or(0);
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
        let smaller = self
            .screens
            .iter()
            .map(|t| t.output_height)
            .filter(|h| *h > 0)
            .min();
        // WHAT IS ACTUALLY FIXED, and it used to be a literal `2`.
        //
        // The percentage belongs to the WHOLE footer, so whatever is not the
        // expansion has to come off it — and until the brain's line arrived the
        // fixed part was exactly the modeline and the minibuffer. It is not any
        // more: a countdown on screen is a third line the footer is spending.
        //
        // THE NOTIFICATION BAND IS STILL NOT IN THIS TERM, and that is a
        // pre-existing looseness rather than a decision made here: up to three
        // lines of band are not counted against the ceiling. Whether it should
        // join is a separate question — the band is transient in a way these
        // are not — and it is flagged rather than folded in silently.
        expansion_ceiling(smaller, usize::from(FOOTER_LINES + self.brain_rows()))
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

    /// How many cells wide the region is.
    ///
    /// From the first CONFIGURED screen and not the first of the list: an
    /// unconfigured one is at zero width, and everything would then fit in one
    /// column — the same reason `grid()` picks it that way.
    fn screen_columns(&self) -> u16 {
        self.metric().map_or(0, crate::chrome::Frame::columns)
    }

    /// This door's measurements — see `chrome::Frame`, the one place a token
    /// becomes a pixel. `None` before a screen has been configured.
    ///
    /// Built rather than stored: the surface's width and height are the only
    /// facts, and a kept copy of what they imply is a second home for them.
    #[allow(clippy::cast_precision_loss)]
    fn metric(&self) -> Option<crate::chrome::Frame> {
        let screen = self.screens.iter().find(|t| t.configured && t.width > 0)?;
        Some(crate::chrome::Frame::of(
            screen.width as f32,
            screen.height as f32,
            self.first_of_expansion(),
        ))
    }

    /// The expanded region's grid right now.
    ///
    /// On scrolling it is a single column — a list. On pagination it is columns
    /// side by side, filling the width before the height.
    fn grid(&self) -> Grid {
        // THE COUNT FIRST, and the ROWS only for whoever measures them.
        //
        // The clipboard and the file manager take a single column, so all they
        // want from the list is how long it is — and building it to find out is
        // what made every frame pay for the whole directory. See
        // `expansion_len`.
        let total = self.expansion_len();
        // The width comes from the first CONFIGURED screen, and not from the
        // first of the list: `first()` could be at zero width, and then
        // everything would fit in one column.
        let width_px = self
            .screens
            .iter()
            .find(|t| t.configured && t.width > 0)
            .map_or(0, |t| t.width);
        let columns = tokens::metrics::columns(width_px as f32, tokens::font::SIZE_BASE);
        // THE CLIPBOARD USES ONLY THE LEFT COLUMN.
        //
        // Its list is the left panel; on the right sits the content, which is
        // drawn separately. Letting the grid spread into columns — as it does on
        // the shortcut list — would put the history on top of the content.
        if self.clipboard_open {
            return Grid {
                columns: 1,
                lines: self.expansion_ceiling().min(total.max(1)),
                width: list_columns(columns),
                colon: 0,
                icon: 0,
            };
        }
        // THE TRANSFER PANEL, for the same reason as the file manager just
        // below: its row already resolved its own columns against the screen
        // (`transfers::row_of`), so letting the grid spread it would lay the
        // same row out twice, with two different answers, and draw one over the
        // other.
        if matches!(self.view(), ViewState::Transfers) {
            return Grid {
                columns: 1,
                lines: self.expansion_ceiling().min(total.max(1)),
                width: columns,
                colon: 0,
                icon: 0,
            };
        }
        // THE FILE MANAGER TAKES THE WHOLE WIDTH, in one column.
        //
        // Its row already resolved its own columns against the screen — see
        // `files::layout_for` — so letting the grid spread it into columns
        // would lay the same row out twice, with two different answers, and
        // draw one over the other.
        if self.files.is_some() {
            // AND THE STRIP COMES OFF THE CEILING, not off the list's own
            // count: the four lines it takes are four the listing does not get,
            // and `per_page` is what pages and scrolls. At least one line left
            // for the list — a region that was all strip would be a preview of
            // a file nothing on screen names.
            let room = self
                .expansion_ceiling()
                .saturating_sub(self.files_strip())
                .max(1);
            return Grid {
                columns: 1,
                lines: room.min(total.max(1)),
                width: columns,
                colon: 0,
                icon: 0,
            };
        }
        let lines = self.expansion();
        let widths: Vec<Measure> = lines.iter().map(ExpansionRow::measurement).collect();
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
        if self.expansion_len() == 0 {
            return 0;
        }
        // THE HEIGHT, and this is the one caller that wants it: what the region
        // OCCUPIES is the list plus the strip pinned under it. See
        // `expansion_len` for the audit that puts every other caller on the
        // other side of the line.
        self.grid().lines + self.files_strip()
    }

    /// How many pages the current content has.
    fn pages(&self) -> usize {
        self.expansion_len().div_ceil(self.grid().per_page()).max(1)
    }

    /// The first visible line, corrected if the content shrank.
    ///
    /// Corrected on READING, and not stored corrected: the content changes
    /// underneath the offset (a notification expires, the filter narrows), and
    /// a stored value would be left pointing past the end with nothing warning
    /// it. The symptom would be a blank region.
    fn current_offset(&self) -> usize {
        // THE MANAGER'S OFFSET IS THE MANAGER'S. This door keeps one number
        // for the eight views that have no state of their own; the file
        // manager has three sections that scroll and keeps all three, so
        // asking it is the only way the two cannot disagree.
        //
        // They did disagree. The footer moved its copy on a wheel event and
        // pushed it into the view before the next draw, so a gesture with no
        // frame between the two arrived late — and a drag on the listing's bar
        // moved a number the view never read.
        if let Some(v) = self.files.as_ref() {
            return v.list_offset();
        }
        // The LAST offset goes by PAGE when it paginates: with a grid of three
        // columns, stopping at `per_page` from the end would leave two empty
        // columns on the last turn.
        let per_page = self.grid().per_page();
        let last = last_offset(self.expansion_len(), per_page);
        let step = self.scroll_step().max(1);
        (self.offset - self.offset % step).min(last)
    }

    /// Moves the region's first visible row, wherever that number lives.
    ///
    /// The counterpart of [`Self::current_offset`], and the only writer: with
    /// the manager open it is the view's, otherwise this door's. Every place
    /// that scrolls goes through here, which is what makes "one offset per
    /// list" a property of the code rather than a habit.
    fn scroll_offset_to(&mut self, to: usize) {
        if let Some(v) = self.files.as_mut() {
            v.scroll_listing_to(to);
            return;
        }
        self.offset = to;
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
    /// How many lines the expansion has, WITHOUT building them.
    ///
    /// The height, the page count, the offset and the scrollbar all want the
    /// count and nothing else, and each of them used to get it by building the
    /// whole list — several full builds per frame. On a listing of ten thousand
    /// files that is tens of milliseconds of arithmetic per pointer movement.
    /// See `files::View::rows_from` for the measurements.
    ///
    /// Only the file manager can answer cheaply, and only it needs to: every
    /// other expansion is a keymap page, a clipboard history or a command list
    /// — dozens of rows, built in microseconds.
    ///
    /// # This number is the SCROLL, and never the HEIGHT
    ///
    /// The manager's bottom strip is pinned under the list and does not move
    /// with it, so it is not in here — a strip that scrolled would appear only
    /// on the last page. Every caller was read and classified, and the split is
    /// written out rather than left to be inferred:
    ///
    /// | caller | wants | reads |
    /// |---|---|---|
    /// | `grid()` | height | this, minus the strip, for `lines` |
    /// | `expansion_rows()` | height | `grid().lines` PLUS the strip |
    /// | `pages()`, `current_offset()`, `last_offset` | scroll | this |
    /// | `drag_to()` | scroll | this |
    /// | `row_at()`, `visible_row()` | both | this, and the strip separately |
    /// | the scrollbar (`bar_showing`, its height) | scroll | this |
    ///
    /// The strip's own count is [`Footer::files_strip`].
    fn expansion_len(&self) -> usize {
        if let Some(files) = &self.files {
            // THE MODAL LIST IS THE REGION, when one is up. The menu, the
            // picker and the detail sheet COVER the listing, so the height is
            // theirs and neither `row_count` nor `region_len` will answer —
            // both say `None` there, on purpose.
            //
            // Falling through to the bottom of this function with that `None`
            // is what killed the shell: `expansion()` with the manager open
            // asks `expansion_window`, which asked `expansion` back. See
            // `files::View::modal_len`.
            if let Some(n) = files.modal_len(self.screen_columns(), &self.texts) {
                return n.max(1);
            }
            // NO `n == 0` CASE: `row_count` answers at least one, because
            // "nothing here" is a sentence that has to be somewhere and it is
            // now a field of the listing. That `.max(1)` moved INTO the view,
            // where the window door can reach it too.
            //
            // THE CHROME COMES FROM THE VIEW, which is the one place that
            // decides how tall it is — see `View::chrome_lines`. It used to be
            // a literal `2` here and another one below, and the doors could
            // drift apart from each other and from the drag.
            //
            // THROUGH `region_len`, which adds the chrome to the LISTING and
            // not to whichever column is tallest — see the note there.
            return files
                .region_len(files.chrome_lines(), self.screen_columns())
                .unwrap_or(1);
        }
        // The panel counts without building: `expansion()` would clone every
        // job to answer "how many", and this runs on every pointer motion —
        // which is the half of the hover lag `rows_from` was written to end.
        //
        // Through `view()`, for the reason `expansion()` gives: a height taken
        // from one view and rows from another is a region with a gap in it.
        if matches!(self.view(), ViewState::Transfers)
            && let Some(view) = &self.transfers
        {
            return view.row_count().max(1);
        }
        self.expansion().len()
    }

    /// The file manager's two lines that are NOT the listing.
    ///
    /// They cost two of the twelve, and each buys an answer the rows do not give.
    ///
    /// THE PATH, because until now this door never said where it was. The
    /// window door has shown it from the first day; the footer showed the rows
    /// and nothing else, and "where am I" is the first question a listing
    /// raises. It is also where what is typed goes — see `files::View::crumbs`
    /// for why the query belongs at the end of the path and not on the message
    /// line.
    ///
    /// THE HEADER, because without it the columns are unlabelled and the sort is
    /// invisible — and it is where the sort is clicked, which is the gesture
    /// every table in the world has.
    ///
    /// Neither is drawn while the menu, the picker or the detail sheet is up:
    /// none of those has columns or a directory, and chrome over them would
    /// name nothing.
    ///
    /// ONE function, because `expansion` and `expansion_window` both need it and
    /// two copies would drift — which is the same reason `expansion` itself
    /// exists as a single list feeding both the height and the drawing.
    /// How many lines the manager's bottom strip takes — and zero when there
    /// is none.
    ///
    /// It is a HEIGHT and never a scroll: the strip is pinned under the last
    /// line of the list, so it costs the listing rows rather than following it.
    /// See `expansion_len` for the audit of who reads which.
    fn files_strip(&self) -> usize {
        self.files
            .as_ref()
            .map_or(0, |v| usize::from(v.panes(self.screen_columns()).strip))
    }

    /// The strip's own rows, built by the same view that builds the listing.
    fn files_strip_rows(&self) -> Vec<ExpansionRow> {
        let Some(view) = self.files.as_ref() else {
            return Vec::new();
        };
        view.strip_rows(self.screen_columns(), &self.texts)
            .into_iter()
            .map(|r| ExpansionRow {
                // THE SAME RULE AS A LINE OF THE LISTING: a row with nothing in
                // it is not a target. The strip is four lines and the info
                // panel may have two facts.
                click: (!r.fields.is_empty()).then_some(Click::FilesStrip),
                // AND THE TRANSPORT'S ANCHOR TRAVELS WITH IT — see
                // `files::Row::foot`: the strip's last line is the one the
                // remainder must fall above rather than below.
                foot: r.foot,
                fields: r.fields,
                ..ExpansionRow::empty_one()
            })
            .collect()
    }

    fn files_chrome(
        &self,
        files: &crate::files::View,
        width: u16,
        plain: bool,
    ) -> Vec<ExpansionRow> {
        if !plain {
            return Vec::new();
        }
        vec![
            ExpansionRow {
                click: Some(Click::FilesPath),
                fields: files.crumbs(width),
                foot: false,
                ..ExpansionRow::empty_one()
            },
            ExpansionRow {
                click: Some(Click::Files),
                fields: files.header(width, &self.texts).fields,
                foot: false,
                ..ExpansionRow::empty_one()
            },
        ]
    }

    /// The expansion's lines from `skip`, at most `take` of them.
    ///
    /// The drawing wants one page and the height wants a count; building the
    /// whole directory for either is what made the hover trail the pointer.
    fn expansion_window(&self, skip: usize, take: usize) -> Vec<ExpansionRow> {
        let Some(files) = &self.files else {
            // Every other view's list is short: build it and cut it.
            return self.expansion().into_iter().skip(skip).take(take).collect();
        };
        // AND THE MANAGER IS BUILT HERE, WHATEVER STATE IT IS IN — the modal
        // branch below is a few lines down, and it is the whole reason this
        // function knows about `plain`.
        //
        // There used to be two guards above it, both asking `row_count` and
        // both handing the manager back to `expansion()` when it said `None`.
        // `expansion()` with the manager open asks THIS function, so the pair
        // was an unconditional infinite recursion — and `row_count` says `None`
        // for exactly the three states the modal branch exists to draw. The
        // footer's file manager died of a stack overflow the moment `C-m`, the
        // right button or a permission question opened one of them.
        let width = self.screen_columns();
        let plain = files.plain();
        // THE HEADER IS LINE ZERO, so it belongs to the window only when the
        // window starts at the top — and it costs one of the slots.
        // THE TWO LINES THAT ARE NOT THE LISTING, and they cost two of the twelve.
        //
        // THE PATH, because until now this door never said where it was. The
        // window door has shown it from the first day; the footer showed the
        // rows and nothing else, and "where am I" is the question a listing
        // raises first. It is also where the query goes now — see
        // `files::View::crumbs`.
        //
        // THE HEADER, because without it the columns are unlabelled and the
        // sort is invisible — and it is where the sort is clicked, which is the
        // gesture every table in the world has.
        //
        // Neither is drawn while the menu, the picker or the detail sheet is on
        // screen: none of those has columns, and a header over them would name
        // nothing.
        // SECTION BY SECTION, and never a line composed of four columns.
        //
        // This door draws through `ExpansionRow`, which is one row per line —
        // so the sections are laid onto those lines here, each at its own
        // `top`, each from its own content. What does NOT happen is a section
        // deciding another's line: a column's length, its offset and its top
        // stay inside it, and the only thing they share is the `y` they are
        // drawn at.
        // A MODAL VIEW IS ONE LIST, and not four columns — and it comes down
        // the SAME path as the other four now.
        //
        // It used to have a branch of its own here, carrying `modal_rows`
        // straight past the sections. That branch is what left the menu, the
        // picker and the detail sheet without a scrollbar, without a thumb to
        // drag, without a wheel that stopped where the bar did and without a
        // hover band: all four of those are `Section`'s, and a list that never
        // became a section had none of them. `View::sections_of` builds the
        // modal as the LISTING's section now — see the note there.
        let chrome = self.files_chrome(files, width, plain);
        let bare: Vec<crate::files::Row> = chrome
            .iter()
            .map(|r| crate::files::Row {
                fields: r.fields.clone(),
                marked: false,
                foot: r.foot,
                focused: false,
            })
            .collect();
        let clicks: Vec<Option<Click>> = chrome.iter().map(|r| r.click).collect();
        let sections = files.sections(&bare, take, width, &self.texts);
        // THE REGION IS WHAT SITS ABOVE THE STRIP — the strip has lines of its
        // own and both doors draw it through `View::strip_rows`.
        let region: Vec<&crate::files::Section> = sections.iter().filter(|s| s.top == 0).collect();
        let lines = region.iter().map(|s| s.content.len()).max().unwrap_or(0);
        (0..take.min(lines))
            .map(|n| {
                let mut fields = Vec::new();
                let mut listing_has_something = false;
                for sec in &region {
                    let Some(line) = sec.content.get(n) else {
                        continue;
                    };
                    if matches!(sec.pane, crate::files::Pane::Listing) {
                        listing_has_something = !line.fields.is_empty();
                    }
                    fields.extend(line.fields.iter().cloned());
                }
                ExpansionRow {
                    // THE CHROME'S OWN CLICK on its own lines — the path opens
                    // the parent and the header sorts — and the listing's
                    // below them.
                    click: clicks
                        .get(n)
                        .copied()
                        .flatten()
                        .or_else(|| listing_has_something.then_some(Click::Files)),
                    fields,
                    ..ExpansionRow::empty_one()
                }
            })
            .collect()
    }

    fn expansion(&self) -> Vec<ExpansionRow> {
        // THE FILE MANAGER FIRST, matching `view()`'s own order — the one place
        // the precedence is written. A branch here that disagreed with that
        // function is exactly the divergence `ViewState` exists to prevent.
        if self.files.is_some() {
            // THROUGH THE SAME BUILDER THE DRAWING USES.
            //
            // This branch had a second copy of it — chrome chained in front of
            // a flat list of rows — and a second copy is a second answer the
            // day one of them is edited. `expansion_window` lays the SECTIONS
            // onto the lines, and asking it for the whole region is the same
            // question with no ceiling.
            //
            // AND THE HEIGHT COMES FROM `expansion_len`, which is the one place
            // the manager's is decided — modal list included. This asked
            // `region_len` on its own and took `0` for its `None`, so anything
            // modal asked for a region of no lines.
            return self.expansion_window(0, self.expansion_len());
        }
        // THE TRANSFER PANEL. One row per job, drawn by `transfers::View` with
        // the file manager's own `Row` and `Field` — see that module for why it
        // does not have a second row type of its own.
        //
        // IT ASKS `view()`, and does not test its own field. The two are not
        // the same question: the leader arrives from the compositor whatever is
        // on screen, so `LEADER c c` opens the clipboard **on top of** an open
        // panel — and then the keyboard belongs to one of them and the region
        // would have drawn the other. That is the exact divergence `ViewState`
        // exists to prevent, and it is why the precedence lives in one place.
        if let Some(view) = self
            .transfers
            .as_ref()
            .filter(|_| matches!(self.view(), ViewState::Transfers))
        {
            let rows = view.rows(self.screen_columns(), &self.texts);
            if rows.is_empty() {
                // "Nothing under way" is still a line. An empty region would
                // leave the person unable to tell an empty queue from a panel
                // that failed to open.
                return vec![ExpansionRow {
                    text: self.texts.t(ronoctua_i18n::key::TRANSFERS_EMPTY),
                    text_color: tokens::color::TEXT_DIM,
                    ..ExpansionRow::empty_one()
                }];
            }
            return rows
                .into_iter()
                .enumerate()
                .map(|(n, r)| ExpansionRow {
                    click: Some(Click::Transfers(n)),
                    fields: r.fields,
                    foot: false,
                    ..ExpansionRow::empty_one()
                })
                .collect();
        }
        // THE CLIPBOARD beats everything else: like the manual and the `M-x`, it
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
                    fields: Vec::new(),
                    foot: false,
                })
                .collect();
        }
        // THE NETWORK. It comes after the finder and before the `M-x` — the
        // precedence of the views lives only in `Footer::view()`, and this
        // order here only follows it.
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
                        .t_with(ronoctua_i18n::key::NETWORK_REMEMBER, &[("network", ssid)]),
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
                    fields: Vec::new(),
                    foot: false,
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
        if self.notifications_open {
            return self.manager_rows();
        }
        Vec::new()
    }

    /// The manager's rows, as the expanded region draws them.
    ///
    /// Its own `fields` and not the key/text pair: it lays out two columns of
    /// its own, and passing it through the key/text machinery would lay the
    /// same row out a second time with a different answer. It is the shape the
    /// file manager's listing already uses, for the same reason.
    fn manager_rows(&self) -> Vec<ExpansionRow> {
        let width = self
            .screens
            .iter()
            .find(|t| t.configured && t.width > 0)
            .map_or(80, |t| {
                tokens::metrics::columns(t.width as f32, tokens::font::SIZE_BASE)
            });
        let records = self.notifications.history();
        let lists = crate::notifications::Lists::of(records, &self.book);
        self.manager
            .rows(&lists, &self.book, width, &self.texts)
            .into_iter()
            .enumerate()
            .map(|(n, r)| ExpansionRow {
                // THE HEADING IS NOT A ROW to click on: it names the column, it
                // is not in it. Giving it a click would put the cursor on the
                // first item while the pointer was on the title.
                click: (n > 0).then(|| Click::Notifications(n - 1)),
                fields: r.fields,
                foot: false,
                ..ExpansionRow::empty_one()
            })
            .collect()
    }

    /// The region, as a list that may not fit.
    ///
    /// The eight views that are not the manager share one list and one offset,
    /// so there is one of these. The manager keeps four of its own — see
    /// `files::Section::scroll`.
    fn region_scroll(&self) -> ronoctua_design::Scroll {
        ronoctua_design::Scroll {
            total: self.expansion_len(),
            lines: self.grid().lines,
            offset: self.current_offset(),
            top: 0,
        }
    }

    /// THE GENERIC BAR'S TRACK — its column and its band — as ONE rectangle.
    ///
    /// # Why a rectangle and not a column
    ///
    /// Because `on_thumb` tested the COLUMN alone, and the bar's column runs
    /// the whole height of the surface: pressing the last cell of the modeline
    /// — or of the minibuffer, or of the notification band — started a drag of
    /// a list that is nowhere near the pointer. The manager's own bars have
    /// been a rectangle tested in both axes since `Section::bar_box`; this one
    /// had not caught up.
    ///
    /// Pressing the track ABOVE or BELOW the thumb still jumps: that is this
    /// door's documented behaviour, and the rectangle is the track rather than
    /// the thumb for exactly that reason.
    ///
    /// `None` when there is nothing to scroll.
    fn region_track(&self, width: f32, height: f32) -> Option<(f32, f32, f32, f32)> {
        if !self.has_bar() {
            return None;
        }
        let f = crate::chrome::Frame::of(width, height, self.first_of_expansion());
        #[allow(clippy::cast_possible_truncation)]
        let column = self.bar_x(f64::from(width)) as f32;
        Some(region_track_of(
            f,
            self.region_scroll(),
            self.first_of_expansion(),
            column,
        ))
    }

    /// The generic bar's THUMB, as a rectangle.
    ///
    /// The track says where the bar may be pressed; this says where the handle
    /// is drawn. Both come out of the same [`ronoctua_design::Scroll`], which
    /// is what stopped them being two arithmetics.
    fn region_bar(&self, width: f32, height: f32) -> Option<(f32, f32, f32, f32)> {
        let (x, top, cell, _) = self.region_track(width, height)?;
        let f = crate::chrome::Frame::of(width, height, self.first_of_expansion());
        self.region_scroll()
            .bar(x + cell, top, f.bottom(), cell, f.line())
    }

    /// The first configured screen's size, which is the one the pointer is on.
    fn pointer_screen(&self) -> Option<(f32, f32)> {
        #[allow(clippy::cast_precision_loss)]
        self.screens
            .iter()
            .find(|t| t.configured && t.width > 0)
            .map(|t| (t.width as f32, t.height as f32))
    }

    /// Is the pointer on the scrollbar's track?
    fn on_thumb(&self, x: f64, y: f64) -> bool {
        let Some((w, h)) = self.pointer_screen() else {
            return false;
        };
        let Some((bx, by, bw, bh)) = self.region_track(w, h) else {
            return false;
        };
        x >= f64::from(bx) && x < f64::from(bx + bw) && y >= f64::from(by) && y < f64::from(by + bh)
    }

    /// WHICH COLUMN THE EXPANSION'S SCROLLBAR SITS IN, in pixels.
    ///
    /// The surface's last cell for eight of the nine views, and it was written
    /// out as `width - cell` at both of the two sites that need it — the one
    /// that PAINTS the thumb and the one that decides whether the pointer is on
    /// it. Two copies of one number is how a bar comes to be drawn where it
    /// cannot be grabbed, so they are one call now.
    ///
    /// The ninth view is the file manager, which has FOUR sections. The bar
    /// belongs to the LISTING, and with the panels on the right the surface's
    /// last cell is inside the info panel: a thumb there says "the listing is
    /// cut" from a column that is not the listing, and drags a list it is not
    /// on top of. `Panes::listing_bar` is where that cell is decided, once, for
    /// both doors.
    #[allow(clippy::cast_possible_truncation)]
    fn bar_x(&self, width: f64) -> f64 {
        let cell = f64::from(tokens::metrics::cell_width(tokens::font::SIZE_BASE));
        let last = width - cell;
        let columns = tokens::metrics::columns(width as f32, tokens::font::SIZE_BASE);
        self.files
            .as_ref()
            .and_then(|v| v.panes(columns).listing_bar(columns))
            .map_or(last, |at| f64::from(crate::chrome::FIELDS_LEAD + at) * cell)
    }

    /// Puts the top of what is seen where the pointer is, dragging the bar.
    ///
    /// The handle follows the cursor: the fraction of the region's height
    /// becomes the fraction of the list. There is no correction for "where
    /// inside the handle it was grabbed" — with a handle of one or two lines,
    /// it is worth less than the line of code that would implement it.
    fn drag_to(&mut self, y: f64) {
        let Some((w, h)) = self.pointer_screen() else {
            return;
        };
        // THE TRACK THE THUMB WAS DRAWN IN, asked of the same function.
        //
        // It was `ceiling * line_height` here and `grid.lines * line_height`
        // where the thumb is painted — two answers to "how long is this
        // track", which is how a bar comes to be grabbed where it is not
        // drawn.
        let Some((_, top, _, track)) = self.region_track(w, h) else {
            return;
        };
        let line_height = tokens::metrics::line_height(tokens::font::SIZE_BASE);
        let ceiling = self.expansion_ceiling();
        let total = self.expansion_len();
        // `Scroll::offset_at`, the inverse of what was DRAWN.
        //
        // `total * fraction` is not that inverse: the thumb has a length, so
        // the offsets are spread over `track - thumb`. Written that way the
        // bottom of the track was not the last page, and the pointer and the
        // thumb drifted apart as the list grew.
        #[allow(clippy::cast_possible_truncation)]
        let at = (y as f32) - top;
        let to = self.region_scroll().offset_at(at, track, line_height);
        self.scroll_offset_to(to.min(last_offset(total, ceiling)));
    }

    /// Which line of the page is at height `y` of the surface.
    ///
    /// `None` when the pointer is over the modeline, over the minibuffer, or
    /// past the last line — there is no item to point at there, and inventing
    /// one would make the message area describe something that is not under
    /// the cursor.
    /// The two rooms this door gives the manager's sections.
    fn files_rooms(&self) -> crate::files::Room {
        let lines = self.grid().lines;
        crate::files::Room {
            list: lines,
            panel: if self.files_strip() > 0 {
                self.files_strip()
            } else {
                lines
            },
        }
    }

    /// Drags one section's bar to where the pointer is.
    fn drag_section_to(
        &mut self,
        pane: crate::files::Pane,
        axis: crate::files::Axis,
        x: f64,
        y: f64,
    ) {
        // ALONG THE TRACK THAT WAS GRABBED. The sideways bar's is the panel's
        // own pixels, and it answers first because the pane it belongs to also
        // has one going down.
        // AND THE FILM'S AXIS RUNS ACROSS TOO, over the track's own pixels
        // rather than the panel's — `files::track_px` is the one answer, asked
        // here and by the rectangle that was drawn.
        if matches!(axis, crate::files::Axis::Across | crate::files::Axis::Time) {
            let columns = self.screen_columns();
            let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
            let edge = self
                .screens
                .iter()
                .find(|t| t.configured && t.width > 0)
                .map_or(0.0, |t| {
                    f32::from(u16::try_from(t.width).unwrap_or(u16::MAX))
                });
            let rooms = self.files_rooms();
            let panes = self.files.as_ref().map(|v| v.panes(columns));
            let Some(panes) = panes else { return };
            let Some((left, wide)) = crate::files::section_px(
                panes,
                pane,
                crate::chrome::FIELDS_LEAD,
                columns,
                cell,
                edge,
            ) else {
                return;
            };
            if wide <= 0.0 {
                return;
            }
            let Some((left, wide)) = (if matches!(axis, crate::files::Axis::Time) {
                // THE CLOCK'S WIDTH IS THE SECTION'S — it decides where the
                // track ends, so grabbing it by any other arithmetic would read
                // a position the cursor was never drawn at.
                let clock = self.files.as_ref().map_or(0, |v| {
                    let chrome = v.chrome_rows(columns, &self.texts);
                    v.sections(&chrome, self.grid().lines, columns, &self.texts)
                        .iter()
                        .find(|s| s.pane == pane)
                        .map_or(0, |s| s.clock)
                });
                crate::files::track_px(left, wide, cell, clock)
            } else {
                Some((left, wide))
            }) else {
                return;
            };
            if let Some(v) = self.files.as_mut()
                && v.drag_to(
                    pane,
                    axis,
                    (x - f64::from(left)) / f64::from(wide),
                    columns,
                    rooms,
                )
            {
                self.owes_paint = true;
            }
            return;
        }
        let line_height = f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE));
        let top = f64::from(self.first_of_expansion()) * line_height;
        let rooms = self.files_rooms();
        let columns = self.screen_columns();
        // THE SECTION'S OWN BAND, and not the region's height for all four.
        //
        // The window door had this defect and the owner watched it: given the
        // whole surface, the listing's thumb ran down into the facts' box. The
        // strip's rule is where the listing and the sidebar end; the panels
        // down in the strip end at the surface. `Scroll::track` decides which,
        // and it is the same call the thumb was drawn with.
        #[allow(clippy::cast_precision_loss)]
        let tall = {
            let lines = match pane {
                crate::files::Pane::Info => rooms.panel,
                _ => rooms.list,
            };
            let bar = ronoctua_design::Scroll {
                total: usize::MAX,
                lines,
                offset: 0,
                top: 0,
            };
            let bottom = self
                .screens
                .iter()
                .find(|t| t.configured && t.height > 0)
                .map_or(0.0, |t| t.height as f32);
            f64::from(bar.track(top as f32, bottom, line_height as f32)).max(1.0)
        };
        if let Some(v) = self.files.as_mut()
            && v.drag_to(pane, axis, (y - top) / tall, columns, rooms)
        {
            // OWED, not drawn: see `Footer::owes_paint`. A drag sends a
            // `Motion` per pixel of pointer travel, and presenting waits for
            // the vblank — a hundred of them is a hundred frames, and the
            // listing is SEEN walking to where the thumb already is.
            self.owes_paint = true;
        }
    }

    /// EVERY SECTION'S SCROLLBAR, as rectangles — drawn here and tested for
    /// there. See `files::Section::bar_box`.
    fn files_bars(
        &self,
        columns: u16,
        cell: f32,
        line_height: f32,
        edge: f32,
    ) -> Vec<crate::files::Bar> {
        let Some(v) = self.files.as_ref() else {
            return Vec::new();
        };
        let panes = v.panes(columns);
        let top = f32::from(self.first_of_expansion()) * line_height;
        // THE SURFACE'S OWN BOTTOM, for the same reason as the window's — see
        // `files::Section::bar_box`.
        let bottom = self.metric().map_or(top, crate::chrome::Frame::bottom);
        let chrome = v.chrome_rows(columns, &self.texts);
        v.sections(&chrome, self.grid().lines, columns, &self.texts)
            .iter()
            .filter_map(|sec| {
                let (x, wide) = crate::files::section_px(
                    panes,
                    sec.pane,
                    crate::chrome::FIELDS_LEAD,
                    columns,
                    cell,
                    edge,
                )?;
                // BOTH OF A SECTION'S BARS. See the window door's twin: a
                // pane may have one going down and one going across, and a
                // pointer holding one is not holding the other.
                let down = sec
                    .bar_box(x + wide, top, bottom, cell, line_height)
                    .map(|r| (sec.pane, crate::files::Axis::Down, r));
                let across = sec
                    .wide_bar_box(x, top, bottom, wide, cell, line_height)
                    .map(|r| (sec.pane, crate::files::Axis::Across, r));
                // AND THE FILM'S — see the window door's twin.
                // AND THE FILM'S TRACK — see the window door's twin.
                let time = sec
                    .time_track_box(crate::files::Foot {
                        left: x,
                        wide,
                        cell,
                        line_px: line_height,
                        bottom,
                    })
                    .map(|r| (sec.pane, crate::files::Axis::Time, r));
                Some([down, across, time])
            })
            .flatten()
            .flatten()
            .collect()
    }

    /// The film's controls, as rectangles that go ON the picture.
    ///
    /// Three of them and in this order: the half-transparent band, the
    /// timeline's track, and the cursor on it. They are painted after the
    /// pictures — see the `draw_range` pair in `paint` — because the film keeps
    /// the whole panel and anything drawn with the other backgrounds would be
    /// underneath it.
    ///
    /// `Section::time_bar_box` is where the cursor's rectangle lives, which is
    /// what keeps this in step with what the drag reads back.
    fn files_transport(
        &self,
        columns: u16,
        cell: f32,
        line_height: f32,
        edge: f32,
    ) -> Vec<ronoctua_render::quad::Quad> {
        let Some(v) = self.files.as_ref() else {
            return Vec::new();
        };
        if !v.transport_showing() {
            return Vec::new();
        }
        let panes = v.panes(columns);
        let top = f32::from(self.first_of_expansion()) * line_height;
        let chrome = v.chrome_rows(columns, &self.texts);
        let mut out = Vec::new();
        for sec in &v.sections(&chrome, self.grid().lines, columns, &self.texts) {
            let Some((_x, _wide)) = crate::files::section_px(
                panes,
                sec.pane,
                crate::chrome::FIELDS_LEAD,
                columns,
                cell,
                edge,
            ) else {
                continue;
            };
            let mut band = |rect: (f32, f32, f32, f32), color| {
                out.push(ronoctua_render::quad::Quad {
                    x: rect.0,
                    y: rect.1,
                    width: rect.2,
                    height: rect.3,
                    color,
                });
            };
            // EVERY PART OF THE TRANSPORT FROM THE SAME FOOT — see
            // `files::transport_top`, and the window door's twin.
            let foot = crate::files::Foot {
                left: 0.0,
                wide: 0.0,
                cell,
                line_px: line_height,
                bottom: self.metric().map_or(top, crate::chrome::Frame::bottom),
            };
            // THE FIELDS' OWN CELLS, and not the section's rails — see
            // `Section::fields_px`: the glyphs and the clock sit at `from`..`to`
            // and the rails are wider, which is the gap the owner photographed
            // between the timeline and the numbers.
            let (fx, fwide) = sec.fields_px(crate::chrome::FIELDS_LEAD, cell);
            let foot = crate::files::Foot {
                left: fx,
                wide: fwide,
                ..foot
            };
            if let Some(r) = crate::files::transport_band(sec, foot) {
                band(r, tokens::color::SURFACE_TRANSPORT);
            }
            if let Some(r) = sec.time_track_box(foot) {
                band(r, tokens::color::RULE_STRONG);
            }
            if let Some(r) = sec.time_bar_box(foot) {
                band(r, tokens::color::TEXT_PRIMARY);
            }
        }
        out
    }

    /// Which section's bar a POINT is on — the same rectangles that were drawn.
    fn files_bar_at_point(
        &self,
        x: f64,
        y: f64,
    ) -> Option<(crate::files::Pane, crate::files::Axis)> {
        let width = self
            .screens
            .iter()
            .find(|t| t.configured && t.width > 0)
            .map(|t| f32::from(u16::try_from(t.width).unwrap_or(u16::MAX)))?;
        let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
        let line_height = tokens::metrics::line_height(tokens::font::SIZE_BASE);
        self.files_bars(self.screen_columns(), cell, line_height, width)
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

    /// WHAT IS UNDER THE POINTER in the manager, asked of the view.
    ///
    /// The same one question the window door asks — see `files::View::hit`.
    /// This door used to answer it in four places (the path's crumbs, the
    /// header, the listing's row and the strip's), each with its own idea of
    /// which list a `y` belonged to, and the owner found every one of them.
    fn files_target(&self) -> Option<crate::files::Target> {
        let view = self.files.as_ref()?;
        let line = self.row_under_pointer?;
        let cell = self.cell_under_pointer.unwrap_or(0);
        view.hit(
            self.screen_columns(),
            self.grid().lines,
            line,
            cell,
            &self.texts,
        )
    }

    fn row_at(&self, x: f64, y: f64) -> Option<usize> {
        let mut grid = self.grid();
        // THE STRIP IS UNDER THE POINTER TOO. It is not in `expansion_len` —
        // it does not scroll — so both the grid's height and the count have to
        // learn about it here, in the one place that asks "what is under the
        // cursor". Extending the grid is right because the manager's grid is a
        // SINGLE column: `item_at` then walks straight down.
        let strip = self.files_strip();
        let how_many = self
            .expansion_len()
            .saturating_sub(self.current_offset())
            .min(grid.lines)
            + strip;
        grid.lines += strip;
        item_at(
            x,
            y,
            f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE)),
            f64::from(tokens::metrics::cell_width(tokens::font::SIZE_BASE)),
            self.first_of_expansion(),
            grid,
            // THE COUNT, AND NOT THE LIST. `row_at` runs on EVERY pointer
            // motion — this line alone was rebuilding a row for every entry in
            // the directory, per pixel of movement, which is the half of the
            // hover lag that survived `rows_from`. See `expansion_len`.
            how_many,
        )
    }

    /// Opens the file manager on the footer's expanded region.
    ///
    /// It starts where `RONOCTUA_FILES_START` says, relative to home — a NixOS
    /// option and not a literal, because a directory somebody keeps their work
    /// in is a machine's preference (invariant 13). A name that is not there
    /// lands in the home rather than on an error.
    fn open_files(&mut self) {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let preferred = std::env::var("RONOCTUA_FILES_START").ok();
        let start = ronoctua_files::listing::start_at(home.as_deref(), preferred.as_deref());
        tracing::info!(dir = %start.display(), "file manager opened");
        // The trash's root, resolved ONCE and handed over — the same rule as
        // the roster and the zone. `$XDG_DATA_HOME` does not move while a
        // listing is up, and reading it per menu would be an env lookup inside
        // the frame that draws.
        let data = std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from);
        let bin = ronoctua_files::trash::Bin::of_home(data.as_deref(), home.as_deref());
        let store = ronoctua_files::sidebar::store::config_dir();
        let side = crate::files::sidebar_of(home.as_deref(), bin.as_ref(), store.clone());
        // THE DIRECTORY IS WATCHED LAZILY TOO, and only once, for the reasons
        // written against `disks_watching` just below. The footer's one-second
        // tick already noticed a directory whose mtime had moved; what this adds
        // is everything that leaves the mtime alone — a file rewritten in place,
        // a permission changed, and anything at all on a `vfat` stick, whose
        // stamps land in two-second buckets.
        if self.files_notice.is_none()
            && let Some(sender) = self.files_sender.clone()
        {
            match ronoctua_files::notice::Notice::watching(store.as_deref(), move || {
                sender.send(crate::files::Report::Stirred).is_ok()
            }) {
                Ok(n) => self.files_notice = Some(n),
                // NOT FATAL and not worth a sentence: unlike the window, this
                // door keeps its one-second `stale()`, so what is lost here is
                // promptness and the two cases mtime cannot see — not the
                // refreshing itself.
                Err(e) => {
                    tracing::warn!(error = %e, "no directory watcher; the one-second stat is on its own");
                }
            }
        }
        self.files = Some(crate::files::View::at(
            &start,
            self.roster.clone(),
            self.zone.clone(),
            bin,
            self.catalogue.clone(),
            self.texts.clone(),
            side,
        ));
        // THE DISKS ARE WATCHED LAZILY, and only once. Raising the thread at
        // startup would open a bus connection for a view nobody has asked for;
        // raising it per open would leave one running per open. The same shape
        // the launcher's rescan already uses.
        if !self.disks_watching {
            self.disks_watching = true;
            let Some(sender) = self.files_sender.clone() else {
                return;
            };
            if let Err(e) = ronoctua_disks::watch(move |rack| {
                let _ = sender.send(crate::files::Report::Racked(Box::new(rack)));
            }) {
                // NOT FATAL, and not silent either: the sidebar reads once when
                // it opens and simply does not refresh by itself. A stick
                // plugged in that never appears, with nothing said, is the
                // silent fallback this project refuses.
                tracing::warn!(error = %e, "no disk watcher");
                let m = self.texts.t_with(
                    ronoctua_i18n::key::FILES_DISKS_NOT_WATCHING,
                    &[("why", e.to_string().as_str())],
                );
                self.echo_message(m);
            }
        }
        // AND THE WATCH IS POINTED AT WHERE THE VIEW OPENED, here and not only
        // from the next keypress. Leaving it out left the arming to whatever
        // happened to arrive first — in practice the disks thread's `Racked`,
        // which ends in `files_reported` and therefore does point it. That is
        // luck, and on a machine with no udisks2 it is a manager that refreshes
        // only after the first key.
        self.follow_the_place();
        self.first_page();
        self.apply_geometry(KeyboardInteractivity::Exclusive);
    }

    /// The right button: focus that row, then open the action menu.
    ///
    /// Focusing first is what makes the gesture mean what it looks like —
    /// right-clicking a row people have not moved to is the common case, and a
    /// menu that acted on the cursor's row instead would act on something else
    /// entirely.
    ///
    /// THE SAME RESOLUTION THE LEFT BUTTON USES, and the same one the window
    /// door uses: `files_target()`, which is `files::View::hit`. `None` is an
    /// answer and not a failure — the empty directory has no field anywhere and
    /// still has a menu, about the directory itself.
    fn open_files_menu(&mut self) {
        let target = self.files_target();
        let step = self
            .files
            .as_mut()
            .map_or(crate::files::Step::Nothing, |v| v.menu_at(target.as_ref()));
        self.apply_files_step(step);
    }

    /// Carries out what the view asked for.
    ///
    /// Whatever costs more than a syscall goes to a NAMED thread and comes back
    /// through `files_sender`. It is the lesson `systemd-run --scope` already
    /// taught this project: waiting inside the loop that draws freezes the
    /// keyboard, the painting and the compositor's events for as long as the
    /// work takes — and copying a directory takes as long as it takes.
    fn apply_files_step(&mut self, step: crate::files::Step) {
        use crate::files::Step;
        match step {
            Step::Nothing => {}
            Step::Changed => {
                // AND THE REGION FOLLOWS THE CURSOR, as every other list does.
                // Without it the cursor walks past the twelfth line and off the
                // bottom, which looks exactly like a key that stopped working —
                // the same shape as the focus not being drawn at all.
                self.follow_cursor();
                self.apply_geometry(self.interactivity());
                // OWED, not painted here: a held-down arrow is a burst too, and
                // it blocked per key for the same reason the pointer did.
                self.follow_the_place();
                self.ask_paint();
            }
            Step::Closed => {
                self.files = None;
                self.rest_the_watch();
                self.apply_geometry(self.interactivity());
                self.draw_and_paint();
            }
            Step::Say(e) => {
                let text = self.texts.t_with(e.key, &e.borrowed());
                self.echo_message(text);
                self.draw_and_paint();
            }
            Step::Do(doing) => {
                self.do_about_files(doing);
                self.follow_the_place();
            }
        }
    }

    /// Points the watch at whatever directory the view is on now.
    ///
    /// Called after anything that could have navigated, rather than at the one
    /// place that navigates: `View::enter` is inside the view, which knows
    /// nothing about watches and should not start. When the place has not moved
    /// this is a `PathBuf` compare and nothing else, which is what makes calling
    /// it after every keypress honest.
    fn follow_the_place(&mut self) {
        if let (Some(n), Some(v)) = (self.files_notice.as_mut(), self.files.as_ref()) {
            n.look_at(v.where_());
        }
    }

    /// Stops watching the directory, keeping the thread and the sidebar.
    ///
    /// BOTH places the view goes away call this, and the second one is easy to
    /// miss: losing keyboard focus drops the manager without anything having
    /// been closed. Without it the watch would stay on the last directory for
    /// the rest of the session, waking a footer that has no view to re-read.
    fn rest_the_watch(&mut self) {
        if let Some(n) = self.files_notice.as_mut() {
            n.rest();
        }
    }

    /// Carries out one operation.
    ///
    /// A THIN WRAPPER over `files::carry_out`, which is the same code the
    /// window runs. What is left here is the one thing the two doors do
    /// differently: starting a process. The footer has `open_foreign`, with its
    /// scope and its Xwayland satellite; the window reaches `foreign` a step
    /// lower down. Everything else — the syscalls, the threads, the staged
    /// request — is shared, because two copies of it is how the two doors would
    /// come to behave differently.
    fn do_about_files(&mut self, doing: crate::files::Doing) {
        use crate::files::{Doing, Errand};
        let me = std::env::current_exe().ok();
        // The four that need a process, and the clipboard, are the door's
        // business; the rest goes straight through.
        match doing {
            Doing::Open(path) => return self.open_the_file(&path),
            Doing::OpenWith(path, which) => {
                let Some(entry) = self.catalogue.iter().find(|e| e.id == which).cloned() else {
                    return;
                };
                let (program, args) = crate::apps::command_for(&entry, &path);
                self.open_foreign(&program, &args, entry.working_directory.as_deref());
                return;
            }
            // AN APPLICATION PINNED TO THE SIDEBAR, by its id. Through the
            // launcher's own `command_for` and never a second reader of a
            // `.desktop` file's `Exec` — the format has quoting rules and field
            // codes, and two parsers of it is two places to get them wrong.
            //
            // An id the catalogue no longer has is an application uninstalled
            // since the row was pinned, and it SAYS so: a row pressed that does
            // nothing is the silent failure this project refuses.
            // FORMATTING IS A BUFFER, and this door only opens it. The
            // questions — which filesystem, what name, are you sure — belong
            // on a surface with room for them and with its own state, not on a
            // menu row; and the work belongs to the host, which re-checks that
            // the volume is one the running system does not need.
            Doing::Format(id) => {
                self.open_buffer_about("RONOCTUA_BUFFER_FORMAT", Some(&id));
                return;
            }
            Doing::Launch(which) => {
                let Some(entry) = self.catalogue.iter().find(|e| e.id == which).cloned() else {
                    let m = self.texts.t_with(
                        ronoctua_i18n::key::FILES_SIDEBAR_GONE,
                        &[("name", which.as_str())],
                    );
                    self.echo_message(m);
                    return;
                };
                let (program, args) = crate::apps::command_for(&entry, std::path::Path::new(""));
                self.open_foreign(&program, &args, entry.working_directory.as_deref());
                return;
            }
            Doing::Terminal(dir) => {
                if let Some(me) = me {
                    self.open_foreign(
                        &me.to_string_lossy(),
                        &["terminal".to_owned()],
                        Some(&dir.to_string_lossy()),
                    );
                }
                return;
            }
            Doing::RootTerminal(dir) => {
                let (Some(me), Some(pkexec), Some(shell)) = (
                    me,
                    option_env!("RONOCTUA_PKEXEC"),
                    option_env!("RONOCTUA_RESCUE_SHELL"),
                ) else {
                    let m = self.texts.t(ronoctua_i18n::key::FILES_NO_PKEXEC);
                    self.echo_message(m);
                    return;
                };
                // `--keep-cwd`, or the root shell lands in `/` and the gesture
                // stops meaning "here". polkit is what asks; this process never
                // sees a password.
                self.open_foreign(
                    &me.to_string_lossy(),
                    &[
                        "terminal".to_owned(),
                        pkexec.to_owned(),
                        "--keep-cwd".to_owned(),
                        shell.to_owned(),
                    ],
                    Some(&dir.to_string_lossy()),
                );
                return;
            }
            Doing::Copy(text) => {
                let ok = crate::buffer::copy_to_clipboard(&text).is_ok();
                let m = self.texts.t(if ok {
                    ronoctua_i18n::key::FILES_COPIED
                } else {
                    ronoctua_i18n::key::FILES_NOT_COPIED
                });
                self.echo_message(m);
                self.draw_and_paint();
                return;
            }
            _ => {}
        }
        let Some(sender) = self.files_sender.clone() else {
            return;
        };
        let clip = self
            .files
            .as_ref()
            .map(|v| v.clip().clone())
            .unwrap_or_default();
        if matches!(doing, Doing::Paste(_))
            && let Some(v) = self.files.as_mut()
        {
            v.clip_used();
        }
        match crate::files::carry_out(doing, &sender, &clip) {
            // A syscall that answered already: re-read now, so the new value is
            // on screen before the hand leaves the key. What went to a thread
            // re-reads when its report arrives instead.
            Errand::Done => {
                if let Some(v) = self.files.as_mut() {
                    v.reread();
                }
            }
            Errand::Say(e) => {
                let text = self.texts.t_with(e.key, &e.borrowed());
                self.echo_message(text);
            }
        }
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// Opens a file with whatever handles its type.
    ///
    /// Text goes to the editor buffer, which is ours and always there. Anything
    /// else goes to the application that DECLARES the type, out of the
    /// catalogue swept at startup — no directory is walked here, and no
    /// `xdg-open` is spawned to walk one for us.
    fn open_the_file(&mut self, path: &std::path::Path) {
        let of_type = ronoctua_files::mime::of_path(path, ronoctua_files::Kind::File, true);
        if of_type.starts_with("text/") || of_type == ronoctua_files::mime::UNKNOWN {
            self.open_editor(path);
            return;
        }
        let Some(entry) = crate::apps::opens(&self.catalogue, of_type).cloned() else {
            let m = self.texts.t_with(
                ronoctua_i18n::key::FILES_NOTHING_OPENS,
                &[("type", of_type)],
            );
            self.echo_message(m);
            self.draw_and_paint();
            return;
        };
        let (program, args) = crate::apps::command_for(&entry, path);
        self.open_foreign(&program, &args, entry.working_directory.as_deref());
    }

    /// What a thread came back with.
    fn files_reported(&mut self, report: crate::files::Report) {
        match report {
            crate::files::Report::Done => {
                if let Some(v) = self.files.as_mut() {
                    v.reread();
                }
            }
            crate::files::Report::Measured(name, bytes) => {
                if let Some(v) = self.files.as_mut() {
                    v.measured(&name, bytes);
                }
            }
            crate::files::Report::Failed(why) => self.echo_message(why),
            // A REFUSAL WITH A NAME, turned into a sentence here — where the
            // language bundle is. `AlreadyMounted` is the odd one: something
            // else mounted it between the reading and the press, so the honest
            // thing is to re-read and let the row become what it now is.
            crate::files::Report::Refused(which, why) => {
                if matches!(which, ronoctua_disks::Refusal::AlreadyMounted) {
                    if let Some(v) = self.files.as_mut() {
                        v.reread();
                    }
                } else if let Some(v) = self.files.as_ref() {
                    let e = v.refusal_said(which, &why);
                    let m = self.texts.t_with(e.key, &e.borrowed());
                    self.echo_message(m);
                }
            }
            crate::files::Report::Mounted(at) => {
                if let Some(v) = self.files.as_mut() {
                    v.mounted(&at);
                }
            }
            // WHAT IS INSIDE THE FILE. A stale answer is dropped inside
            // `View::looked` and not here: the cursor moves while a child
            // process runs, and which answer is still the right one is the
            // view's question, not this door's.
            crate::files::Report::Inspected(looked) => {
                if let Some(v) = self.files.as_mut() {
                    v.looked(*looked);
                }
                // AND THE PLAYER IS ASKED AGAIN — see the window door's twin,
                // and the defect written out there.
                self.follow_the_motion();
                self.follow_the_turn();
            }
            crate::files::Report::Racked(rack) => {
                // ONLY WHEN THERE IS A VIEW TO PUT IT IN. The thread goes on
                // reading whatever is on screen — that is the point, a stick
                // plugged in before `LEADER .` has to be there when it opens —
                // but repainting for a region nobody is looking at is work
                // spent on nothing.
                let Some(v) = self.files.as_mut() else {
                    self.rack = Some(rack);
                    return;
                };
                v.racked(&rack);
                self.rack = Some(rack);
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
                if let Some(v) = self.files.as_mut() {
                    v.reread();
                    if v.sidebar_stale() {
                        v.reread_sidebar();
                    }
                }
            }
        }
        // A MOUNT MOVES THE VIEW. `Report::Mounted` goes into the directory it
        // just mounted, so the watch has to follow from here too and not only
        // from the key that was pressed.
        self.follow_the_place();
        // THE HEIGHT FOLLOWS THE ROW COUNT, up to the region's ceiling — the
        // same pair the one-second tick uses. Painting a longer listing into a
        // surface still sized for the shorter one is how rows go missing with
        // nothing having gone wrong.
        self.apply_geometry(self.interactivity());
        self.draw_and_paint();
    }

    /// Which CELL of the region the pointer is over, from the left edge.
    ///
    /// Separate from `row_at` because it answers a different question and one
    /// view needs both: the row says which entry, the cell says which of its
    /// columns — and for the permission block, which of the ten bits.
    fn cell_at(&self, x: f64) -> Option<u16> {
        // THROUGH `chrome`, and not through the token directly: it is what
        // says how wide a cell is on every surface this shell draws, and it is
        // what the two-door test compares. Reaching past it here would be a
        // second grid nobody named.
        let cell_px = f64::from(crate::chrome::cell_width());
        if cell_px <= 0.0 || x < 0.0 {
            return None;
        }
        // THE LEAD COMES OFF, because the drawing put it on.
        //
        // The answer is a cell of the ROW, and the row is drawn shifted by
        // `chrome::FIELDS_LEAD`. Reading the screen's cell straight would
        // describe the field to the left of the one under the finger, all the
        // way across — the shape of defect this file has already paid for
        // twice, where the drawing and the hit test each computed the same
        // number their own way.
        u16::try_from((x / cell_px) as u64)
            .ok()
            .map(|c| c.saturating_sub(crate::chrome::FIELDS_LEAD))
    }

    /// The visible line at index `i` on the current page.
    ///
    /// PAST THE LIST IS THE STRIP, when the manager has one. The two are
    /// numbered as one column of lines because that is what the pointer sees —
    /// `row_at` counts from the top of the region and does not know where the
    /// list stops — and a strip that answered `None` here would be a preview
    /// one cannot click, which is the mouse reaching less than the keyboard.
    ///
    /// # WITH THE MANAGER OPEN, ONLY THE `Click` IS TRUE, AND ONLY AS A LABEL
    ///
    /// A window of ONE line of a plain listing is the listing's CHROME — the
    /// columns are laid onto the region's lines, and one line of region is line
    /// zero of every column. So the fields that come back here are the path
    /// line's, whatever `i` says, and the row is the manager's only in the
    /// sense that it says so.
    ///
    /// That is enough for the one thing this is asked for: `click_at` treats
    /// the three `Files*` variants alike and resolves the point again through
    /// `files::View::hit`. It is NOT enough for anything that reads a number
    /// out of the answer, and the secondary button did exactly that until the
    /// owner reported it dead in this door — see `Click::Files`.
    fn visible_row(&self, i: usize) -> Option<ExpansionRow> {
        let lines = self.grid().lines;
        if self.files_strip() > 0 && i >= lines {
            return self.files_strip_rows().into_iter().nth(i - lines);
        }
        self.expansion_window(self.current_offset() + i, 1)
            .into_iter()
            .next()
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
            Some(Click::Transfers(n)) => {
                if let Some(v) = self.transfers.as_mut() {
                    v.focus_row(n);
                    self.draw_and_paint();
                }
            }
            Some(Click::Notifications(n)) => {
                // WHICH CELL decides whether this was a target or just aiming —
                // the row alone cannot say, because the row has two of them.
                let cell = self.cell_under_pointer;
                let width = self.screen_columns();
                let records = self.notifications.history().to_vec();
                let errand = {
                    let lists = crate::notifications::Lists::of(&records, &self.book);
                    self.manager.clicked(n, cell, &lists, width)
                };
                self.manager_errand(errand);
                self.draw_and_paint();
            }
            Some(Click::Network(n)) => {
                if let Some(v) = self.network.as_mut() {
                    v.focus_on(n);
                    self.draw_and_paint();
                }
            }
            // EVERYTHING THE MANAGER OWNS, through ONE resolution.
            //
            // The path's crumbs, the header, a listing row, a sidebar place, a
            // fact in the panels and a cell of the bottom strip were six
            // branches with six ideas of which list a point belonged to. They
            // are one now: `files::View::hit` says the section, the row inside
            // it and the field, and this door presses the key that goes with
            // it. Nothing here decides anything the keyboard cannot reach.
            Some(Click::FilesPath | Click::Files | Click::FilesStrip) => {
                let Some(target) = self.files_target() else {
                    return;
                };
                let Some(view) = self.files.as_mut() else {
                    return;
                };
                let step = view.click(target.row, &target.hit);
                self.apply_files_step(step);
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
        // NOT WHERE THE MANAGER IS, and this is the whole of it: the manager
        // draws a bar PER SECTION (`files_bars`), and this one draws one for
        // the region as if the region were a single list.
        //
        // Both were drawn. The owner photographed the result twice — two bars
        // in the same column, the outer one moving with a total that is not
        // the listing's, so it stood still while the listing scrolled and
        // jumped while it did not. It was never a bar with a bug: it was a
        // second bar, and this is the line that says there is only one.
        //
        // It goes for a menu and a sheet too, which do not scroll at all: the
        // condition is the door, not the state.
        if self.files.is_some() {
            return false;
        }
        // ONLY WHAT SCROLLS HAS A BAR. On a paginated list it would be a
        // continuous control for content that moves in jumps — and what says
        // where you are there is the counter.
        self.navigation() == Navigation::Scroll && self.expansion_len() > self.grid().per_page()
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
        let total = self.expansion_len();
        let margin = SCROLL_MARGIN.min(ceiling.saturating_sub(1) / 2);

        let offset = self.current_offset();
        let fresh = if cursor < offset + margin {
            // Going up: the cursor does not touch the top of the region.
            cursor.saturating_sub(margin)
        } else if cursor + margin >= offset + ceiling {
            // Going down: it stops `margin` lines from the end.
            (cursor + margin + 1).saturating_sub(ceiling)
        } else {
            offset
        };
        self.scroll_offset_to(fresh.min(last_offset(total, ceiling)));
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
            // The manager's cursor, so the region scrolls with it — the same
            // `scrolloff` the locator uses. A listing that does not follow its
            // own cursor is a listing where holding `j` walks off the bottom.
            //
            // ASKED OF THE VIEW, because the answer is the VISUAL position and
            // the view is what knows which list is on screen. This door used to
            // work it out itself — the listing's cursor plus a header it tested
            // for — and with a menu up that is the wrong cursor entirely: the
            // region followed a listing nobody could see while the chosen row
            // walked off the bottom of a seventeen-row menu.
            ViewState::Files => self
                .files
                .as_ref()
                .and_then(crate::files::View::visual_cursor),
            // The transfer panel's, for the same reason and with no adjustment:
            // it draws no chrome above the list, so its row IS its visual line.
            ViewState::Transfers => self
                .transfers
                .as_ref()
                .and_then(crate::transfers::View::focused_row),
            // THE MANAGER'S, plus the heading it draws on the first line — the
            // same adjustment the file manager makes for its own header. Without
            // it the region scrolls to one row above the focused one, and with
            // no arm here at all the region does not follow the cursor: holding
            // the arrow walks the cursor off the bottom of a list that never
            // moves.
            ViewState::Notifications => Some(self.manager.cursor() + 1),
            _ => None,
        }
    }

    /// Goes back to the top of the region.
    ///
    /// Called where the CONTENT changes — new key, new filter, new list.
    /// Without this, narrowing the list while scrolled down would show an
    /// empty region, and the key would look as if it had not worked.
    fn first_page(&mut self) {
        self.scroll_offset_to(0);
    }

    /// Scrolls the region by LINES, without leaving the extremes.
    ///
    /// Gives back `true` if it changed — redrawing with no change spends a
    /// frame to paint the same thing. It does not wrap around on purpose:
    /// going back to the start after the end makes the eye lose its place.
    fn scroll_by(&mut self, lines: i32) -> bool {
        let last = last_offset(self.expansion_len(), self.grid().per_page());
        let current = self.current_offset();
        let fresh = scrolling(current, last, lines);
        self.scroll_offset_to(fresh);
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
        let exclusive = height as i32;
        for t in &self.screens {
            t.layer.set_size(0, height);
            t.layer.set_exclusive_zone(exclusive);
            t.layer.set_keyboard_interactivity(keyboard);
            t.layer.commit();
        }
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "flushing the connection failed");
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
            // AND `ISO_Left_Tab` IS THE SAME KEY. It is what xkb produces for
            // Shift+Tab, and without this case it fell through to `key_char()`,
            // came back `None`, and the key was swallowed in silence — which is
            // the first of the three causes that show the same screen.
            Keysym::Tab | Keysym::ISO_Left_Tab => Sym::Tab,
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

        let mut mods = self.inheritance.discount(self.mods);
        // SHIFT ENTERS FROM THE KEYSYM AND FROM NOWHERE ELSE.
        //
        // `discount` refuses the modifier itself, and its own note says why:
        // xkb has already applied Shift when it produced the character, so
        // counting both would turn `F` into `S-F` and never match. This is that
        // same rule and not an exception to it — `ISO_Left_Tab` IS the applied
        // form of Shift+Tab, exactly as `F` is of Shift+f — so reading it here
        // is reading what xkb decided rather than guessing at a modifier.
        //
        // AND FROM THE MODIFIER FOR EVERY KEY THAT IS NOT A CHARACTER, because
        // there xkb has nothing to apply it TO: Shift+Left is `Left`, and the
        // modifier is the only place that shift exists. The paragraph above was
        // written when `S-TAB` was the only shifted binding, and what it
        // actually says is "shift is not counted twice" — which an arrow cannot
        // do. Without this, `S-<left>` arrived as a bare `<left>` and went up a
        // directory instead of turning the shape in the preview.
        mods.shift =
            event.keysym == Keysym::ISO_Left_Tab || (mods.shift && !matches!(sym, Sym::Char(_)));
        let key = Key { mods, sym };
        // THE KEY AS IT ENDED UP, in the log.
        //
        // Without this, "I pressed it and nothing happened" does not tell "the
        // key did not arrive" from "it arrived with a stuck modifier and did not
        // match" — and the two are fixed in opposite places. It took four
        // occurrences for this line to exist.
        tracing::debug!(?key, inherited = ?self.inheritance, "the key after the discount");
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
        self.open_buffer_about(variable, None);
    }

    /// The same, with something for the buffer to be ABOUT.
    ///
    /// It travels as the third argument, in the slot `host_it` already keeps
    /// for the file an editor buffer opens — one mechanism, and the buffer
    /// receives it as data it did not go looking for. A buffer has no
    /// filesystem and no bus (invariant 11); what it can be given is a name.
    fn open_buffer_about(&mut self, variable: &str, about: Option<&str>) {
        match (std::env::current_exe(), buffer_component(variable)) {
            (Ok(me), Some(w)) => {
                let mut args = vec!["buffer".to_owned(), w];
                args.extend(about.map(ToOwned::to_owned));
                self.open_foreign(&me.to_string_lossy(), &args, None)
            }
            (_, None) => {
                tracing::error!(
                    variable,
                    "the component's variable is not in the environment"
                );
                let m = self.texts.t(ronoctua_i18n::key::ECHO_NO_BUFFER);
                self.echo_message(m);
            }
            (Err(e), _) => tracing::error!(error = %e, "I do not know my own binary"),
        }
    }

    /// Opens the rescue console in a floating terminal window.
    ///
    /// # Why the binary comes from NEXT DOOR and not from a variable
    ///
    /// `ronoctua-rescue` and this binary are built by the same derivation —
    /// there is one workspace (`nix/rust.nix`). A `RONOCTUA_RESCUE` holding its
    /// store path would be this derivation referencing itself, which Nix cannot
    /// express. What ships them together is that they are the same `bin/`, and
    /// asking the filesystem is the honest way to say so.
    fn open_rescue(&mut self) {
        let beside = std::env::current_exe()
            .ok()
            .and_then(|me| Some(me.parent()?.join("ronoctua-rescue")))
            .filter(|p| p.exists());
        match (std::env::current_exe(), beside) {
            (Ok(me), Some(rescue)) => self.open_foreign(
                &me.to_string_lossy(),
                &["rescue".to_owned(), rescue.to_string_lossy().into_owned()],
                None,
            ),
            (_, None) => {
                tracing::error!("the rescue binary is not beside this one");
                let m = self.texts.t(ronoctua_i18n::key::ECHO_NO_BUFFER);
                self.echo_message(m);
            }
            (Err(e), _) => tracing::error!(error = %e, "I do not know my own binary"),
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
                tracing::error!(variable, "the watcher's variable was not set at build time");
                let m = self
                    .texts
                    .t_with(ronoctua_i18n::key::ECHO_NO_OPENED, &[("program", variable)]);
                self.echo_message(m);
            }
            (Err(e), _) => tracing::error!(error = %e, "I do not know my own binary"),
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
            tracing::error!("null Wayland pointers; no graphics context");
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
                                tracing::error!(error = %e, "the rectangle renderer failed")
                            }
                        }
                        match ronoctua_render::image::Images::new(&g.device, g.config.format) {
                            Ok(im) => self.screens[i].images = Some(im),
                            // Without it the whole footer carries on: what is lost is
                            // the preview of the copied image, and the history's text
                            // stays on screen.
                            Err(e) => tracing::error!(error = %e, "the image renderer failed"),
                        }
                        self.screens[i].text = Some(t);
                        // "first frame in 200 ms" (5.2), counted from the start of
                        // the process. Only the FIRST screen marks it: with two
                        // monitors, the second would measure the time up to a frame
                        // that is not the first.
                        let mut g = g;
                        if i == 0 {
                            g.mark_as("first-frame", ronoctua_render::start());
                        }
                        self.screens[i].gpu = Some(g);
                        tracing::info!(screen = i, "graphics context and renderer ready");
                    }
                    Err(e) => tracing::error!(error = %e, "the text renderer failed"),
                }
            }
            // NOT fatal: with no GPU the footer still resolves the layout and logs.
            // Failing here would bring the whole session down because of a driver.
            Err(e) => {
                tracing::error!(error = %e, "no graphics context; the footer will not paint")
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
        // THE NOTIFICATION LEFT THE MINIBUFFER'S LINE.
        //
        // It used to take the message region — fourth in the precedence below —
        // and that was the whole of section 4.2's "the modeline has one message
        // region". It has a region of its own now, above the modeline, which is
        // what the owner asked for and what lets a notification carry a body
        // and a row of actions without stealing the line one types on.
        //
        // What stays here is the ECHO, which is the shell answering the person
        // — a different thing that happens to have used the same line.

        // THE PENDING SEQUENCE on the minibuffer's line, as in Doom.
        //
        // It already showed up in the modeline's mode indicator. That is not
        // enough: the indicator is a narrow segment that ELIDES, and the place
        // one looks at while typing a chord is the line below — the echo area.
        let pending = self
            .leader
            .active_one()
            .then(|| self.leader.prefix(&self.leader_label));
        let echo = self.echo.as_ref().map(|(t, _)| t.clone());

        // THE PREVIEW COMES OUT HERE, before the loop's mutable borrows. It is a
        // REFERENCE: cloning 33 MB of RGBA per frame would be worse than having
        // no preview.
        let image_ready = self.clip_image.as_ref().map(|(_, r)| r);
        let version = self.clip_version;
        // THE ALREADY DECODED PIXMAPS, by reference and for the SAME reason: each
        // one is small, but cloning twelve per frame would be paying for the
        // decoding again in memory.
        let ready_icons = &self.icons;
        // AND THE MANAGER'S PREVIEW, by reference and for the same reason: it
        // is up to a mebibyte, and the upload happens inside the loop where
        // `self` is already borrowed mutably by `screens.get_mut`.
        let files_pixels = self
            .files
            .as_ref()
            .and_then(crate::files::View::preview_pixels);
        // AND THE PICTURE THAT REDRAWS ITSELF — a film's frame, or a shape at
        // the angle it is being turned to — which replaces it while it lasts.
        let files_frame = self.files.as_ref().and_then(crate::files::View::live_frame);

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
        }
        // AND THE FILE MANAGER'S HINT — but never its query.
        //
        // The query lives at the END OF THE PATH now (`files::View::crumbs`):
        // the two read as one sentence there, and this line goes back to its own
        // job, which is what the pointer is resting on and what the last action
        // said. While the query was here, hovering a permission bit replaced
        // what was being typed, or was replaced by it, depending on the order.
        //
        // The hint stays, because typing is not a key and so has no line in the
        // manual — and it goes away the moment there is something typed, since
        // by then the path itself is showing it.
        .or_else(|| {
            let v = self.files.as_ref()?;
            if !v.plain() || v.filtering() {
                return None;
            }
            Some(crate::files::hint_of(
                &self.texts,
                v.split(self.screen_columns()) > 0,
            ))
        })
        // AND THE TRANSFER PANEL'S, for the same reason: its keys are chords
        // that nothing on the row spells out, and a panel whose only way to
        // find `C-c` is the manual is a panel where nobody ever stops
        // anything.
        .or_else(|| self.transfers.as_ref().map(|v| v.hint(&self.texts)));

        // THE VISIBLE PAGE of the expanded region.
        //
        // The whole list comes out of `expansion()`; here it is cut ONCE, and the
        // same cut holds for the height (`expansion_rows`). Cutting in two
        // places is what once gave a huge gap in the footer.
        let grid = self.grid();
        let navigation = self.navigation();
        let pages = self.pages();
        let offset = self.current_offset();
        // ONLY THE PAGE THAT IS DRAWN gets built. See `expansion_window`.
        let all_of = self.expansion_window(offset, grid.per_page());
        // AND THE MANAGER'S STRIP UNDER IT.
        //
        // It is NOT part of the list: it does not scroll, it is pinned below
        // the last line, and it has a line numbering of its own. What follows
        // makes the INDEX of `visible` the SCREEN LINE, padding a short listing
        // with a blank — because the pointer counts lines from the top of the
        // region and does not know where the listing stopped. Without the
        // padding a two-file directory would put the strip's first row where
        // the pointer thinks the third file is.
        let strip_of = self.files_strip_rows();
        let blank = ExpansionRow::empty_one();
        let visible: Vec<&ExpansionRow> = if strip_of.is_empty() {
            all_of.iter().collect()
        } else {
            let mut v: Vec<&ExpansionRow> = Vec::with_capacity(grid.lines + strip_of.len());
            v.extend(all_of.iter());
            while v.len() < grid.lines {
                v.push(&blank);
            }
            v.extend(strip_of.iter());
            v
        };
        // WHERE LINE `i` LANDS. The grid answers for the list; past it lies the
        // strip, which is not in the grid at all — one column, pinned, and the
        // index IS the line.
        let strip_from = grid.lines;
        let has_strip = !strip_of.is_empty();
        let place_of = move |i: usize| -> (u16, u16) {
            if has_strip && i >= strip_from {
                (0, u16::try_from(i).unwrap_or(0))
            } else {
                grid.position(i)
            }
        };
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
        let indicator = (navigation == Navigation::Page && pages > 1).then(|| {
            format!(
                "{}  {}/{pages}",
                self.texts.t(ronoctua_i18n::key::EXPANSION_PAGE),
                offset / grid.per_page().max(1) + 1
            )
        });
        // THE DESCRIPTION OF THE ITEM UNDER THE CURSOR.
        //
        // The list shows the COMMAND; the whole sentence shows up here, on the
        // message line, when the pointer passes over it. It is Doom's split: the
        // list is to be swept with the eye, and the description only when it is
        // asked for — and pointing with the mouse is the question.
        let sob_o_cursor = self
            // THE BAND'S CONTROL WINS, and it can: no two of these can be under
            // the pointer at once, because each is a different `y`. The order
            // is the screen's, from the top down — the band, then the modeline,
            // then the region.
            .band_hint
            .clone()
            .or_else(|| self.segment_hint())
            // AND THE BRAIN'S LINE, next down the screen: it sits under the
            // minibuffer and above the region, and this chain is the screen's
            // own order from the top.
            .or_else(|| self.brain_hint.clone())
            // AND WHAT THE MANAGER'S FOCUSED ROW HAS TO ADD — today, the reason
            // a switch does nothing. It used to be drawn as a third column on
            // the row and the owner asked for it gone; the honesty had to stay,
            // so it moved to the line this footer already explains things on.
            .or_else(|| {
                self.notifications_open
                    .then(|| self.manager.hint(&self.texts))
                    .flatten()
            })
            .or_else(|| {
                self.row_under_pointer
                    .and_then(|i| visible.get(i))
                    .and_then(|l| l.description.clone())
            })
            // AND A MENU ROW UNDER THE CURSOR SAYS WHAT IT ACTUALLY DOES.
            //
            // ASKED FOR FROM THE SCREEN, and the request named the gap exactly:
            // the rows are written for somebody who is starting out, and
            // somebody who already knows what they want has nowhere to read
            // which call, which options, what can be undone. So each action
            // carries a second sentence, and it arrives by MOVING THE CURSOR —
            // not by pointing, which is what every other explanation on this
            // line has required and which the owner's own rule forbids being
            // the only way ("whatever the mouse can do the keyboard can do").
            //
            // Above the pointer's own answer in this chain, because a menu
            // covers the listing: with one open, what is under the pointer is
            // the row the pointer is on and what is under the CURSOR is the
            // choice about to be made.
            .or_else(|| {
                let e = self.files.as_ref()?.cursor_explains()?;
                Some(self.texts.t_with(e.key, &e.borrowed()))
            })
            // THE FILE MANAGER'S FIELDS ANSWER PER CELL, and this is the whole
            // of the "tooltip" the owner asked for: pointing at the `w` of a
            // group says, on this line, what that bit means for THIS entry —
            // and on a directory it says something different, because there `x`
            // is "may enter" and not "execute".
            //
            // Nothing floats. It is the same place which-key already puts a
            // description, which is what keeps invariant 14 intact.
            .or_else(|| {
                // THROUGH `files::View::hit`, which resolves the SECTION first.
                // Read off the flattened line, as it was, a pointer resting in
                // an empty part of the facts described a file far down the
                // listing — the owner photographed exactly that.
                let e = self.files_target()?.explain?;
                Some(self.texts.t_with(e.key, &e.borrowed()))
            })
            // AND WHAT WENT WRONG WITH A FILM, which nothing else on screen
            // could say: **refusal is a value, not an absence**. A panel with
            // no sound and a panel whose film went silent look the same, and
            // only one of them has a sentence.
            //
            // BELOW the pointer's answer, which is this line's own precedence:
            // pointing at something is a question, and a question asked now
            // beats a state from before.
            .or_else(|| {
                let v = self.files.as_ref()?;
                // AND WHAT WENT WRONG WITH A SHAPE, on the same line and for
                // the same reason the film's sentence is here.
                let e = v.film_trouble().or_else(|| v.shape_said())?;
                Some(self.texts.t_with(e.key, &e.borrowed()))
            })
            // AND A TRANSFER'S ROW SAYS WHAT WENT WRONG.
            //
            // A row that says only "failed" sends somebody to the journal —
            // which on this system means opening a terminal to find out why a
            // copy did not happen. The daemon's own words are already in the
            // record; this is where they reach the screen.
            .or_else(|| {
                // THE VISIBLE ROW IS NOT THE LIST'S ROW, and this is the third
                // time that distinction has cost something in this file. With
                // the region scrolled, `row_under_pointer` is a line of the
                // PAGE; indexing the job list with it names a different
                // transfer, and the sentence would describe one row while the
                // pointer sat on another.
                //
                // The `click` the row already carries is the list's index, put
                // there by `expansion()`. Reading it back is the only
                // arrangement in which the two cannot drift.
                let row = self.row_under_pointer?;
                let view = self.transfers.as_ref()?;
                let Some(Click::Transfers(n)) = visible.get(row)?.click else {
                    return None;
                };
                Some(crate::transfers::under_the_pointer(
                    view.at(n)?,
                    &self.texts,
                ))
            });
        // WHERE THE POINTER IS, marked by a BACKGROUND — and, since the owner
        // reported it from the screen, where the FOCUS is by another.
        //
        // # Why they are two backgrounds now, and were one
        //
        // This said that the focus needs no background because it is told apart
        // by the text's COLOUR, and that giving both one would leave "which is
        // selected?" unanswered while the pointer rested elsewhere. The first
        // half was not true of the manager: the colour reaches ONE field of
        // seven (the name, through `files::colour_of`), a folder is already
        // that colour, and the action menu's rows carry no accent at all — so
        // in the menu the cursor was invisible outright. *"Ao trocar de item
        // com as setas nenhuma alteração/indicador visual é feita"*, in both
        // doors.
        //
        // The second half was a real objection and it is answered rather than
        // dropped: the two bands are two TOKENS, `surface.selection` for the
        // focus and the weaker `surface.hover` for the pointer, so two lit rows
        // still say which is which. And when they are the same row only the
        // focus is drawn — the stronger statement of the two, and painting the
        // weaker over it would dim the very row one is standing on.
        //
        // # Only what is CLICKABLE
        //
        // The background says "this responds to the click". In the manual, which
        // is only read, a lit line would promise an action that does not exist.
        //
        // AND NEVER OVER A PICTURE. The band says which ROW of a list the
        // pointer is on, and a preview showing a photograph or a film is not a
        // list — the owner reported it lighting *"o background da linha no
        // preview de vídeos e imagens"*, where it says nothing and covers part
        // of what one came to look at. `View::band_belongs` is the one place
        // that decides, so both doors answer the same.
        //
        // ASKED OF THE CELL, which is the same input the band's own rectangle
        // comes from — see `View::band_belongs_at`. Asked of the resolved
        // target instead, it answered "draw it" wherever the target came back
        // `None` while `band_of` went on banding the cell, and the owner would
        // have gone on seeing the stripe.
        let over_a_picture = self.files.as_ref().is_some_and(|v| {
            let columns = self.screen_columns();
            let in_strip = self
                .row_under_pointer
                .is_some_and(|line| line >= self.grid().lines);
            !v.band_belongs_at(self.cell_under_pointer.unwrap_or(0), columns, in_strip)
        });
        // WHERE THE KEYBOARD IS, in the manager and nowhere else.
        //
        // The seven other views of this region have a focus one can see: the
        // finder and the application list accent the focused item's own text,
        // and the manual has no focused item at all. The manager is the one
        // where the signal did not survive — one field of seven in the listing,
        // nothing whatsoever in the menu — and it is the one the owner
        // reported. Widening this to lists nobody complained about would be
        // repainting seven things to fix one.
        //
        // THE LINE COMES FROM THE SECTIONS — see `files::View::focus_line`. A
        // row is not a line, and the two bands have to agree about the
        // conversion or the same row would light in two places.
        //
        // AND THE CELL COMES FROM THE PANE THAT HAS THE KEYBOARD, not from the
        // pointer: `band_of` decides which side of the sidebar's rule to light
        // from a cell, and the cell it is normally given is where the mouse is.
        // A focus band placed that way would jump sides when the hand moved.
        let focused_band = self.files.as_ref().and_then(|v| {
            let columns = self.screen_columns();
            let (pane, line) = v.focus_line(columns, grid.lines)?;
            let cell = match pane {
                crate::files::Pane::Sidebar => 0,
                _ => v.split(columns),
            };
            Some((cell, line))
        });
        let focus_cell = focused_band.map(|(cell, _)| cell);
        let focused_row = focused_band
            .filter(|(_, line)| visible.get(*line).is_some_and(|l| l.click.is_some()))
            .map(|(_, line)| place_of(line));
        let under_the_pointer = self
            .row_under_pointer
            .filter(|_| !over_a_picture)
            .filter(|i| visible.get(*i).is_some_and(|l| l.click.is_some()))
            .map(place_of)
            // ONE BAND WHERE IT IS ONE ROW. Painting the weaker colour over the
            // stronger would dim the very row one is standing on, which is the
            // opposite of what either band is for.
            .filter(|p| Some(*p) != focused_row);
        // Does the footer have the keyboard? It is what decides whether the
        // accent stays on the window (the compositor's border) or comes to the
        // buffer's name.
        let with_keyboard = self.grabbing();
        // ALWAYS, WHEN THE LIST WAS CUT — and not only once the pointer has
        // found it. It used to read `self.under_pointer && self.has_bar()`,
        // which made the one thing on screen that says "there is more below"
        // depend on already suspecting there was. The manager's per-section
        // bars have never hidden; this is the same rule for the other eight
        // views of the footer.
        let bar_showing = self.has_bar();
        let first_paint = self.frames_painted < 3;
        let mut painted = false;

        for index in 0..self.screens.len() {
            let width = self.screens[index].width;
            if width == 0 {
                continue;
            }
            // THE SURFACE'S BOTTOM, in pixels. A divider that stops at
            // `lines * line_height` stops short — the line height is
            // fractional — and that is the gap the owner photographed under
            // the panels' rule.
            #[allow(clippy::cast_precision_loss)]
            let bottom = self.screens[index].height as f32;
            // PER SCREEN: a narrow monitor elides what the wide one shows.
            let columns = tokens::metrics::columns(width as f32, tokens::font::SIZE_BASE);
            let placed = layout::resolve(columns, &segments);

            // THE ORDER OF THE FOUR REGIONS, from top to bottom:
            //
            //   lines 0..b     band       — the notification, 0 to 3 lines
            //   line  b        modeline   — the buffer's name
            //   line  b+1      minibuffer — GLUED to it
            //   line  b+2      brain      — the countdown, 0 or 1 line
            //   lines b+3..n   expansion  — which-key, `M-x`, the manager
            //
            // The footer's two lines stay TOGETHER; the list grows BELOW them
            // and the notification appears ABOVE. The owner's decision, and the
            // third position the expansion has taken in this project: it has
            // been above the modeline (and the buffer's name jumped on every
            // key) and between the two (and the two lines that are read
            // together were separated by twenty of list). Gluing the pair, what
            // is read as one thing appears as one thing, and what grows grows
            // away from it.
            //
            // The surface is anchored at the BOTTOM, and that is what makes the
            // band free: growing upwards does not move the modeline on the
            // screen, it eats into the content. Which is also why the expansion
            // has a CEILING (`expansion_ceiling`) and the band has one of three.
            let first_of_expansion = self.first_of_expansion();
            let modeline_line = self.modeline_line();
            let mini_line = self.minibuffer_line();
            let brain_line = self.brain_line_at();

            // WHETHER THE STATE IS THE BASE ONE decides its colour, and it is
            // asked ONCE per frame rather than per segment: the answer is the
            // same for every span on the line, and it costs a map lookup.
            // `None` counts as the base: with no state there is no segment to
            // colour, and the quiet token is the answer that changes nothing.
            let state_is_base = self
                .state_now()
                .is_none_or(|s| s == ronoctua_protocol::BASE_STATE);
            self.modeline_placed = placed
                .iter()
                .map(|c| (c.id, c.column, ronoctua_render::layout::width(&c.text)))
                .collect();
            let mut spans: Vec<ronoctua_render::Span> = placed
                .iter()
                .flat_map(|c| {
                    glyph_and_text(
                        &c.text,
                        c.column,
                        modeline_line,
                        segment_color(c.id, with_keyboard, state_is_base),
                        c.edge,
                    )
                })
                .collect();

            // THE NOTIFICATION BAND, above everything.
            //
            // It is the footer's own surface growing UPWARDS, exactly as the
            // expansion grows downwards — one surface, no popup, no card
            // (invariants 14 and 15). The layout is `crate::band`, which knows
            // nothing about Wayland; here it becomes runs.
            //
            // Remembered in `band_placed` for the pointer, for the same reason
            // the modeline's segments are: what the cursor is over has to be
            // what was DRAWN.
            let band = crate::band::lines(
                self.notifications.showing(),
                columns,
                self.notifications.fraction_left(std::time::Instant::now()),
                self.notifications.position(),
                self.blink_on,
                &crate::band::Words { texts: &self.texts },
            );
            for (row, line) in band.iter().enumerate() {
                for piece in &line.pieces {
                    spans.extend(glyph_and_text(
                        &piece.text,
                        piece.at,
                        u16::try_from(row).unwrap_or(0),
                        piece.color,
                        // The band's pieces are laid out from the LEFT.
                        false,
                    ));
                }
            }
            self.band_placed = band;

            // THE BRAIN'S LINE, under the minibuffer.
            //
            // It appears without anybody having asked for it, so it says whose
            // it is before it says anything else — and it says it in a colour
            // of its own, which is the only thing separating it from the pair
            // above (no rule, no box, no card: invariants 14 and 15).
            //
            // Remembered in `brain_placed` for the pointer, for the reason the
            // band's is: what the cursor is over has to be what was DRAWN.
            let brain = crate::brain::lines(
                self.announced(),
                columns,
                ronoctua_brain::Clock::now(&ronoctua_brain::moment::Wall),
                &crate::brain::Words {
                    texts: &self.texts,
                    abort_keys: &self.brain_keys(crate::leader::BRAIN_ABORT_KEYS),
                    proceed_keys: &self.brain_keys(crate::leader::BRAIN_PROCEED_KEYS),
                },
            );
            if let Some(l) = &brain {
                for piece in &l.pieces {
                    spans.extend(glyph_and_text(
                        &piece.text,
                        piece.at,
                        brain_line,
                        piece.color,
                        false,
                    ));
                }
            }
            self.brain_placed = brain;

            // THE PRECEDENCE of the minibuffer's line, from top to bottom:
            //
            //   0. item under the cursor — asking with the mouse wins, because it
            //      is a deliberate and ephemeral gesture: it goes away when the
            //      pointer leaves, and the pending sequence comes back.
            //   1. `M-x` open           — the user is typing there.
            //   2. leader active        — the pending sequence, as in Doom.
            //   3. echo                 — the shell's last message.
            //   4. at rest              — empty.
            //
            // THE NOTIFICATION IS NOT ON THIS LIST ANY MORE, and it used to be
            // fourth. It has a band of its own now, above the modeline — which
            // also settles the thing that made it fourth: a notification
            // arriving mid-chord used to wipe the sequence the person was
            // building off the screen. Now the two are in different places and
            // neither has to wait for the other.
            // THE ECHO COMES FIRST, and it used to come fourth.
            //
            // It is what the shell has to SAY — the refusal of an action that
            // was just asked for — and it EXPIRES. Behind the pointer's
            // description it was unreachable in the one situation it exists
            // for: the hand is on the mouse, the pointer is resting on the row
            // that was just clicked, and the tooltip owns the line for as long
            // as it stays there. The message came and went underneath it.
            //
            // That is how "changing the owner did nothing" gets reported about an
            // action that answered perfectly well — on a line something else
            // was covering. A description of what is under the pointer can wait
            // three seconds; a refusal cannot, because by then it is gone.
            let (mini, mini_color) = if let Some(e) = &echo {
                (e.clone(), tokens::color::TEXT_SECONDARY)
            } else if let Some(d) = &sob_o_cursor {
                (d.clone(), tokens::color::TEXT_SECONDARY)
            } else if let Some(p) = &prompt_mx {
                (p.clone(), tokens::color::TEXT_PRIMARY)
            } else if let Some(p) = &pending {
                (p.clone(), tokens::color::STATE_ACCENT)
            } else {
                (mini_repouso.clone(), tokens::color::TEXT_PRIMARY)
            };
            if !mini.is_empty() {
                spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::Grid,
                    text: mini,
                    column: tokens::space::MODELINE_EDGELEFT,
                    line: mini_line,
                    color: mini_color,
                    edge: false,
                    foot: false,
                });
            }

            // THE PAGE INDICATOR, flush against the right edge of the minibuffer's
            // line. There is no bar and no box: it is text on the line that already
            // exists (invariants 14 and 15).
            if let Some(ind) = &indicator {
                let indicator_width = layout::width(ind);
                if indicator_width < columns {
                    spans.push(ronoctua_render::Span {
                        size: ronoctua_render::Size::Grid,
                        text: ind.clone(),
                        column: columns - indicator_width,
                        line: mini_line,
                        color: tokens::color::TEXT_DIM,
                        edge: false,
                        foot: false,
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
                let (base, grid_y) = place_of(i);
                let y = first_of_expansion + grid_y;
                // A FILE MANAGER ROW DRAWS ITSELF, field by field.
                //
                // It resolved its own columns against the screen's width before
                // this loop existed — see `files::layout_for` — and each field
                // carries the cell it starts at. Passing it through the
                // key/text machinery would lay the same row out a second time,
                // with a different answer, and draw one over the other.
                if !line.fields.is_empty() {
                    // THE FIELD UNDER THE POINTER LIGHTS UP.
                    //
                    // The row already has a background; this marks WHICH of its
                    // fields the explanation on the message line is about. In a
                    // block of ten characters that look like one word, a
                    // sentence with nothing pointing at its subject is a
                    // sentence about an unknown bit.
                    //
                    // A COLOUR and not a box: invariant 15, and the same choice
                    // the focused row already makes.
                    let on_this_row = self.row_under_pointer == Some(i);
                    for f in &line.fields {
                        let under =
                            on_this_row && self.cell_under_pointer.is_some_and(|c| f.covers(c));
                        spans.push(ronoctua_render::Span {
                            size: ronoctua_render::Size::Grid,
                            text: f.text.clone(),
                            // AND THE CONTENT'S LEAD, which this path did not
                            // pay and every other one did. It is the whole of
                            // "the same view is glued to the edge here and not
                            // there": a field row begins at its own gutter —
                            // the mark's cell — and a surface owes the
                            // difference up to `chrome::INSET`. Both windows
                            // already worked that out for themselves; this was
                            // the third site, and it had no such line at all.
                            column: base + crate::chrome::FIELDS_LEAD + f.at,
                            line: y,
                            color: if under {
                                tokens::color::STATE_ACCENT
                            } else {
                                f.color
                            },
                            edge: false,
                            // AND THE TRANSPORT'S ROW IS ANCHORED TO THE FOOT,
                            // the only row in this manager that is — see
                            // `files::Row::foot`.
                            foot: line.foot,
                        });
                    }
                    continue;
                }
                let colon = base + grid.colon;
                let with_key = !line.key.is_empty();
                // THE ICON BEFORE EVERYTHING, in the margin. It is drawn even
                // when the rest of the line is elided: it is the fastest answer
                // to "which program is this".
                if let Some(g) = line.icon {
                    spans.push(ronoctua_render::Span {
                        size: ronoctua_render::Size::Grid,
                        text: g.to_string(),
                        column: base + crate::chrome::INSET,
                        line: y,
                        color: line.key_color,
                        edge: false,
                        foot: false,
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
                    for (text, relative_column, matched) in runs_of(&key, &line.highlights) {
                        spans.push(ronoctua_render::Span {
                            size: ronoctua_render::Size::Grid,
                            text,
                            column: column + relative_column,
                            line: y,
                            color: if matched {
                                tokens::color::STATE_ACCENT
                            } else {
                                line.key_color
                            },
                            edge: false,
                            foot: false,
                        });
                    }
                    if line.separator {
                        spans.push(ronoctua_render::Span {
                            size: ronoctua_render::Size::Grid,
                            text: SEPARATOR_OF.to_owned(),
                            column: colon,
                            line: y,
                            // DISCREET: it separates, it does not inform. In the tone
                            // the modeline gives to what dies first in the elision.
                            color: tokens::color::TEXT_DIM,
                            edge: false,
                            foot: false,
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
                    size: ronoctua_render::Size::Grid,
                    text: layout::elide_end(&line.text, grid.fits_in_text(with_key, base, columns)),
                    column: base + start,
                    line: y,
                    color: line.text_color,
                    edge: false,
                    foot: false,
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

            // THE FILE MANAGER'S PICTURE, fitted into the preview panel's box.
            //
            // A textured quad on the SAME surface — no window and no popup
            // (invariant 14), no border, radius or shadow (invariant 15) — and
            // never stretched: a deformed photograph is worse than a small one,
            // because whoever looks does not know whether what is skewed is the
            // image or the panel.
            let mut file_preview: Option<(String, ronoctua_render::image::Area)> = None;
            if let Some(v) = self.files.as_ref() {
                let panes = v.panes(columns);
                // WHERE THE PANELS' OWN ROWS BEGIN, and it is NOT the top of
                // the region.
                //
                // # The defect this line exists for
                //
                // The picture was fitted into a box that started at the first
                // line of the EXPANSION and ran the region's whole height. The
                // panels' fields do not: they ride the listing's rows, and this
                // door puts two lines of its own above those — the path and the
                // header. So the box was two lines taller than the panel at the
                // top and centred inside it, which put the picture a line above
                // where the panel begins and left the difference underneath.
                // Photographed and reported exactly that way: *"the preview's
                // picture is touching the top line and has a lot of extra space
                // underneath"*.
                //
                // AND THE BOX IS MEASURED FROM THE PICTURE THAT IS DRAWN.
                //
                // It used to ask `preview_pixels` — the STILL frame — and then
                // draw whatever the player had sent into the rectangle that
                // answer produced. The two are different shapes, so every film
                // came out stretched to its poster's proportions; under the old
                // height guess that was ninefold, and it is what *"the frames
                // look rotated"* was. `preview_now` is the one answer, and
                // the picture that is measured is the picture that is drawn.
                //
                // AND THE BOX IS MEASURED WHETHER OR NOT THERE IS ANYTHING IN
                // IT YET — see the window door's twin. Measured inside the
                // `preview_key` guard it DEADLOCKED: a film has no still any
                // more, so nothing is drawn until the player produces a frame,
                // and the player is not started until the box is known.
                if let Some((at, room)) = panes.preview_box(columns) {
                    let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
                    #[allow(clippy::cast_possible_truncation)]
                    let (top, tall) = crate::files::panel_band(
                        first_of_expansion,
                        grid.lines as u16,
                        panes.strip,
                    );
                    #[allow(clippy::cast_precision_loss)]
                    let measured = crate::files::panel_px(
                        panes,
                        crate::files::Panel::Preview,
                        crate::chrome::FIELDS_LEAD,
                        columns,
                        cell,
                        width as f32,
                    );
                    if let Some((_, box_wide)) = measured {
                        let air = f32::from(tokens::space::FILES_PICTUREAIR);
                        v.measured_preview(
                            box_wide - air - air,
                            f32::from(tall) * line_height - air - air,
                        );
                    }
                    // AND THE DRAWING IS A GUARD AND NOT A `continue`: the
                    // nearest loop here is the one walking the region's visible
                    // LINES, so bailing out of it would drop a line of text
                    // whenever the preview had nothing to show.
                    if let (Some(key), Some(rgba)) = (v.preview_key(), v.preview_now()) {
                        // AND THE AIR ON ALL FOUR SIDES IS THE VIEW'S, not this
                        // door's. `fit_into` centres and that was never the whole
                        // promise: whichever dimension is the tight one comes out
                        // exactly as long as the box, so the picture was centred
                        // AND touching the rule on its right. See
                        // `files::picture_in`.
                        // THE REGION'S RAILS, not the text's — see
                        // `files::panel_px`.
                        #[allow(clippy::cast_precision_loss)]
                        let (box_x, box_wide) = crate::files::panel_px(
                            panes,
                            crate::files::Panel::Preview,
                            crate::chrome::FIELDS_LEAD,
                            columns,
                            cell,
                            width as f32,
                        )
                        .unwrap_or((
                            f32::from(crate::chrome::FIELDS_LEAD + at) * cell,
                            f32::from(room) * cell,
                        ));
                        file_preview = Some((
                            key.to_owned(),
                            crate::files::picture_in(
                                ronoctua_render::image::Area {
                                    x: box_x,
                                    y: f32::from(top) * line_height,
                                    width: box_wide,
                                    // AND THE TRANSPORT TAKES NOTHING OFF IT — see
                                    // the window door's twin: the controls are
                                    // painted over the film's last line, so the
                                    // picture keeps the whole panel.
                                    height: f32::from(tall) * line_height,
                                },
                                rgba.width,
                                rgba.height,
                            ),
                        ));
                    }
                }
            }

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
                let (base, grid_y) = grid.position(i);
                pixmaps.push((
                    key.clone(),
                    ronoctua_render::image::Area {
                        x: f32::from(base + crate::chrome::INSET) * cell,
                        // CENTERED ON THE LINE, and not flush with the top: the
                        // font's box is `1.45 at` and the drawing is `1.00`, so
                        // without centering the icon would touch the line above.
                        y: f32::from(first_of_expansion + grid_y) * line_height
                            + (line_height - side) / 2.0,
                        width: side,
                        height: side,
                    },
                ));
            }
            if self.clipboard_open {
                let start = list_columns(columns).saturating_add(2);
                let panel = columns.saturating_sub(start);
                let end = first_of_expansion + u16::try_from(grid.lines).unwrap_or(0);
                let chosen = self.clipboard.at(self.clip_cursor);

                // THE LAST THREE LINES are fixed: what the item is, ClipCascade's
                // state, and the keys. They stay at the bottom because they are
                // reference — what is read is the content, and it stays at the top.
                let panel_footer = 3u16;
                let alto = end
                    .saturating_sub(first_of_expansion)
                    .saturating_sub(panel_footer);

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
                    let y0 = f32::from(first_of_expansion) * line_height + modulo;
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
                            y: f32::from(first_of_expansion) * line_height,
                            width: f32::from(panel) * cell,
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
                    let tongue = ronoctua_syntax::guess(t);
                    for (n, l) in wrap_text(t, panel, usize::from(alto))
                        .into_iter()
                        .enumerate()
                    {
                        let line = first_of_expansion + u16::try_from(n).unwrap_or(0);
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
                        size: ronoctua_render::Size::Grid,
                        text: layout::elide_end(&sheet, panel),
                        line: end.saturating_sub(3),
                        column: start,
                        color: tokens::color::TEXT_DIM,
                        edge: false,
                        foot: false,
                    });
                }

                // CLIPCASCADE, telling the truth. The integration does not exist
                // yet (docs/PENDING.md), and an indicator that pretended
                // "connected" would be the façade this project undoes.
                spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::Grid,
                    text: layout::elide_end(
                        &format!(
                            "{}: {}",
                            self.texts.t("clipboard-clipcascade"),
                            self.texts.t("clipboard-clipcascade-missing")
                        ),
                        panel,
                    ),
                    line: end.saturating_sub(2),
                    column: start,
                    color: tokens::color::STATE_WARNING,
                    edge: false,
                    foot: false,
                });

                // THE ACTIONS, with each one's key. They come out of the SAME
                // table the orphan check reads — a legend written by hand would be
                // the second copy that lies the day a key changes.
                let actions = crate::leader::OF_CLIPBOARD_VIEW
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
                    size: ronoctua_render::Size::Grid,
                    text: layout::elide_end(&actions, panel),
                    line: end.saturating_sub(1),
                    column: start,
                    color: tokens::color::TEXT_DIM,
                    edge: false,
                    foot: false,
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
            //
            // AND THE FILM'S CONTROLS, WORKED OUT HERE AND APPENDED LAST.
            //
            // Here because `screen.quads` is borrowed mutably further down and
            // this reads `self`; appended last because they are painted AFTER
            // the pictures, and the order in the list is what says so.
            #[allow(clippy::cast_precision_loss)]
            let transport_now = self.files_transport(columns, cell, line_height, width as f32);
            let mut backgrounds = vec![ronoctua_render::quad::Quad {
                x: 0.0,
                y: f32::from(modeline_line) * line_height,
                width: width as f32,
                height: line_height,
                color: tokens::color::MODELINE_BACKGROUND,
            }];

            // AND THE BAND'S, one tone above the modeline's.
            //
            // The three backgrounds — content, modeline, band — are a ramp the
            // eye reads from bottom to top as "what you are working on, the
            // footer, and the thing that just arrived". It is a change of
            // colour and not a box: no border, no radius, no shadow.
            if modeline_line > 0 {
                backgrounds.push(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y: 0.0,
                    width: width as f32,
                    height: f32::from(modeline_line) * line_height,
                    color: tokens::color::NOTIFICATION_BACKGROUND,
                });
            }

            // AND THE BRAIN'S, which is a step SIDEWAYS on that ramp and not
            // another rung up it: the same luminance neighbourhood as the
            // modeline, with a blue cast, because it is not one more layer of
            // chrome — it is a different KIND of line. `colors.nix` argues the
            // hue where the token lives, and it is the only background in that
            // file that carries one.
            if self.brain_rows() > 0 {
                backgrounds.push(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y: f32::from(brain_line) * line_height,
                    width: width as f32,
                    height: line_height,
                    color: tokens::color::BRAIN_BACKGROUND,
                });
            }

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
            // THE FOCUS FIRST, and the pointer over it: they are never the
            // same row (`under_the_pointer` filters it out), so the order only
            // decides which is drawn under the scrollbar — and both are.
            if let Some((base, grid_y)) = focused_row {
                let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
                #[allow(clippy::cast_precision_loss)]
                let (band_at, band_wide) = self.files.as_ref().map_or_else(
                    || (f32::from(base) * cell, f32::from(grid.width) * cell),
                    |v| {
                        crate::files::band_of(
                            v.panes(columns),
                            crate::chrome::FIELDS_LEAD,
                            grid.width,
                            focus_cell,
                            cell,
                            width as f32,
                        )
                    },
                );
                backgrounds.push(ronoctua_render::quad::Quad {
                    x: band_at,
                    y: f32::from(first_of_expansion + grid_y) * line_height,
                    width: band_wide,
                    height: line_height,
                    color: tokens::color::SURFACE_SELECTION,
                });
            }
            if let Some((base, grid_y)) = under_the_pointer {
                let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
                // AND IT STOPS AT THE SIDEBAR'S EDGE.
                //
                // The manager's row is ONE grid column holding two lists, so a
                // band the width of the column would light the sidebar row
                // beside whatever the pointer is actually on — two rows lit,
                // neither of them the answer to "what am I about to click".
                // Which side is decided by where the pointer IS.
                //
                // AND IT STOPS AT THE PANELS' EDGE TOO, on the other side: a
                // band running under the preview would light the picture, and
                // the pointer is not on the picture — it is on a row of the
                // listing beside it.
                //
                // BOTH EDGES ARE THE RULES' OWN PIXELS NOW, and that is the
                // defect the owner photographed: this door started the
                // listing's band one CELL past the rule — `lead + split + 1` —
                // so a lit row had a strip of dark background between it and
                // the line beside it. `files::band_of` is the one place that
                // arithmetic lives, because the two doors had written it out
                // separately and had three different answers between them.
                #[allow(clippy::cast_precision_loss)]
                let (band_at, band_wide) = self.files.as_ref().map_or_else(
                    || (f32::from(base) * cell, f32::from(grid.width) * cell),
                    |v| {
                        crate::files::band_of(
                            v.panes(columns),
                            crate::chrome::FIELDS_LEAD,
                            grid.width,
                            self.cell_under_pointer,
                            cell,
                            width as f32,
                        )
                    },
                );
                backgrounds.push(ronoctua_render::quad::Quad {
                    x: band_at,
                    y: f32::from(first_of_expansion + grid_y) * line_height,
                    // THE WIDTH OF THE GRID'S COLUMN, and not that of the screen:
                    // with the shortcut list in three columns, a full-screen band
                    // would light the three neighboring lines together.
                    width: band_wide,
                    height: line_height,
                    color: tokens::color::SURFACE_HOVER,
                });
            }

            // THE VERTICAL RULE between the clipboard's two columns, and the
            // QR's modules. Both are quads — the same kind of thing as the
            // modeline's background, inside the SAME surface.
            if self.clipboard_open {
                let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
                let x = f32::from(list_columns(columns).saturating_add(1)) * cell;
                backgrounds.push(crate::chrome::divider(
                    x,
                    f32::from(first_of_expansion) * line_height,
                    bottom,
                ));
                backgrounds.extend(qr_quads);
            }

            // AND THE ONE BETWEEN THE SIDEBAR AND THE LISTING, which is the
            // same kind of thing: a one-pixel rule inside the same surface.
            // Separation by whitespace and a rule, never by a container
            // (invariant 15) — there is no panel here, only a line.
            if let Some(v) = self.files.as_ref() {
                let cell = tokens::metrics::cell_width(tokens::font::SIZE_BASE);
                let panes = v.panes(columns);
                // AND IT PAYS THE FIELDS' LEAD, which it did not.
                //
                // A row is drawn at `base + chrome::FIELDS_LEAD + f.at` and
                // this rule was drawn at `split * cell`: one cell to the LEFT
                // of the boundary it marks, so the sidebar's last letter sat on
                // top of it. The two numbers have to be the same number, and
                // the panels' rules just below pay it too.
                //
                // AND THE LINE SITS IN THE MIDDLE OF THE CELL RESERVED FOR IT.
                // `files::rule_x` argues it: drawn at the cell's left edge, the
                // whole cell became air on the listing's side and none at all
                // on the sidebar's.
                let lead = crate::chrome::FIELDS_LEAD;
                // WHERE THE LISTING'S REGION ENDS: the strip's rule when the
                // panels are down there, and the SURFACE'S bottom when they
                // are not.
                //
                // A BOUNDARY AND AN EDGE ARE DIFFERENT QUESTIONS, and this is
                // the boundary. The strip's rule sits ON a line, so the pixel
                // it starts at is that line's — a count times a line height,
                // and correct. What may never be written that way is an EDGE:
                // the surface's bottom is `floor(height / line)` lines plus a
                // leftover, and the product falls short of it. That is what
                // `list_bottom` asks `bottom` for below, and what
                // `ronoctua_design::Scroll::track` decides for every band.
                #[allow(clippy::cast_precision_loss)]
                let strip_top =
                    f32::from(first_of_expansion) * line_height + grid.lines as f32 * line_height;
                let list_bottom = if panes.strip > 0 { strip_top } else { bottom };
                if panes.sidebar > 0 {
                    backgrounds.push(crate::chrome::divider(
                        crate::files::rule_x(panes.sidebar, lead, cell),
                        f32::from(first_of_expansion) * line_height,
                        list_bottom,
                    ));
                }
                // THE PANELS' RULES: one before the first when they are on the
                // right, one between them wherever they are. Same kind of
                // thing, same surface, no container (invariant 15).
                let (rule_from, rule_to) = if panes.right > 0 {
                    (f32::from(first_of_expansion) * line_height, list_bottom)
                } else {
                    (strip_top, bottom)
                };
                for at in panes.rules(columns) {
                    backgrounds.push(crate::chrome::divider(
                        crate::files::rule_x(at, lead, cell),
                        rule_from,
                        rule_to,
                    ));
                }
                // AND THE LINE THAT SAYS WHERE THE LISTING ENDS, when the
                // panels are down at the bottom. Without it the strip's first
                // row reads as one more entry that happens to say something
                // else.
                if panes.strip > 0 {
                    #[allow(clippy::cast_precision_loss)]
                    backgrounds.push(crate::chrome::rule(width as f32, strip_top));
                }
                // AND THE INFO COLUMN'S OWN BAR.
                //
                // The panel no longer stretches the region to fit its facts —
                // see `files::View::panel_lines` — so on a photograph it has
                // more to say than it has lines, and the owner asked for a bar
                // of its own to say where one is in them.
                //
                // It is the LISTING's bar at another edge, and deliberately the
                // same rectangle: one cell wide, `rule.strong`, a thumb
                // proportional to what is shown and floored at one line. The
                // arithmetic is the view's, in cells and lines, because the two
                // doors draw at different font sizes.
                //
                // **It is NOT hidden behind the pointer**, and that is the one
                // place it departs from the listing's. Down there the bar is
                // second-order — the keyboard scrolls and the region is
                // obviously a list — and here it is the only thing on screen
                // that says the panel has been cut. A list cut with nothing
                // saying so is this file's oldest complaint.
                // EVERY SECTION'S BAR, from the section itself — see
                // `files::Section::bar_box`. They were three rectangles worked
                // out separately here, and the pointer test was a fourth
                // opinion; the owner found each place they disagreed, and one
                // of them put the facts' thumb off the side of the screen
                // whenever the window was not full width.
                //
                // THE FILM'S OWN ARE NOT HERE. They go over the picture — see
                // the overlay below — and a rectangle pushed here would be
                // painted before it and hidden by it.
                #[allow(clippy::cast_precision_loss)]
                for (_, axis, rect) in self.files_bars(columns, cell, line_height, width as f32) {
                    if matches!(axis, crate::files::Axis::Time) {
                        continue;
                    }
                    backgrounds.push(ronoctua_render::quad::Quad {
                        x: rect.0,
                        y: rect.1,
                        width: rect.2,
                        height: rect.3,
                        color: tokens::color::RULE_STRONG,
                    });
                }
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
            if bar_showing {
                // ONE RECTANGLE, FROM `ronoctua_design::Scroll`.
                //
                // Everything about this bar used to be written out here, and
                // wrongly in the same way the manager's was before it moved:
                // the position was `offset / total` and the travel the WHOLE
                // track, so the last page put the thumb's top at the bottom
                // and, clamped, half way down. The owner reported that with
                // the mouse in the manager; the other eight views had it too,
                // and nobody had ever dragged one.
                //
                // The pointer test asks `region_track` and the drag asks
                // `offset_at`, which is this call's inverse. Nine views, one
                // rectangle; what still differs is how many bands a view has.
                #[allow(clippy::cast_precision_loss)]
                if let Some((bx, by, bw, bh)) = self.region_bar(width as f32, bottom) {
                    backgrounds.push(ronoctua_render::quad::Quad {
                        x: bx,
                        y: by,
                        width: bw,
                        height: bh,
                        color: tokens::color::RULE_STRONG,
                    });
                }
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
                if let (Some(area), Some(rgba)) = (preview, image_ready) {
                    let key = format!("clip:{version}");
                    if im.missing(&key) {
                        im.upload_it(&gpu.device, &gpu.queue, &key, rgba);
                    }
                    batch.push((key, area));
                }
                // AND THE MANAGER'S, in the same batch and by the same rule.
                //
                // The key is `Stamp::key()` — path, mtime, size and the step it
                // was decoded at — which is what keeps it out of the way of the
                // clipboard's `clip:<version>` and the icons' bare `Icon=` name
                // in this one map.
                if let Some((key, area)) = file_preview {
                    // A MOVING FILE REPLACES THE TEXTURE UNDER THE SAME KEY,
                    // every frame — which is what keeps an animation to ONE of
                    // the thirty-nine slots. A key per frame would fill the
                    // cache with one file's film and evict every icon.
                    if let Some(frame) = files_frame {
                        im.upload_it(&gpu.device, &gpu.queue, &key, frame);
                    } else if im.missing(&key)
                        && let Some(rgba) = files_pixels
                    {
                        im.upload_it(&gpu.device, &gpu.queue, &key, rgba);
                    }
                    batch.push((key, area));
                }
                for (key, _) in &pixmaps {
                    if im.missing(key)
                        && let Some(Some(rgba)) = ready_icons.get(key)
                    {
                        im.upload_it(&gpu.device, &gpu.queue, key, rgba);
                    }
                }
                im.prepare(&gpu.queue, gpu.config.width, gpu.config.height, &batch);
            }
            // THE FILM'S CONTROLS, LAST AND ON TOP.
            //
            // The half-transparent band that makes the glyphs legible against
            // any frame, the timeline's track, and the cursor on it. All three
            // arrive and leave with the pointer, and all three are painted
            // AFTER the pictures — the picture keeps the whole panel now, so a
            // rectangle drawn with the other backgrounds would be under it.
            #[allow(clippy::cast_possible_truncation)]
            let under = backgrounds.len() as u32;
            backgrounds.extend(transport_now);
            if let Some(q) = screen.quads.as_mut() {
                q.prepare(
                    &gpu.device,
                    &gpu.queue,
                    gpu.config.width,
                    gpu.config.height,
                    &backgrounds,
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
                tracing::error!(error = %e, screen = index, "preparing the text failed");
                continue;
            }
            let shaping_of = t.elapsed();

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
                    q.draw_range(pass, 0, under);
                }
                if let Some(im) = images {
                    im.draw(pass);
                }
                // AND THE FEW THAT GO ON TOP OF IT: the film's transport, which
                // is drawn ON the film by design.
                if let Some(q) = quads {
                    q.draw_range(pass, under, u32::MAX);
                }
                text.draw(ronoctua_render::Atlas::Pinned, pass)
            }) {
                tracing::error!(error = %e, screen = index, "drawing the frame failed");
                continue;
            }
            painted = true;
            // Only the first frames, and only of the first screen: after that they
            // become noise on every 1 s tick.
            if first_paint && index == 0 {
                tracing::info!(
                    shaping_ms = shaping_of.as_secs_f64() * 1000.0,
                    desenho_ms = t.elapsed().as_secs_f64() * 1000.0,
                    "start-up: painting"
                );
            }
        }

        if painted && self.frames_painted < 3 {
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
            tracing::warn!(error = %e, "flushing the connection failed");
        }
    }
}

/// How far the expansion may grow, in lines.
///
/// `screen_height` is `None` when no output has announced a size yet; then
/// only the absolute ceiling holds, which is the more conservative of the two.
///
/// # Why a pure function
///
/// For the same reason as `screen_difference`: the rule is arithmetic, and
/// what wraps it — `wl_output`, `LayerSurface` — does not exist outside a
/// compositor. Separated, it is testable on screens this computer does not have.
/// The generic bar's track — its column and its band — as one rectangle.
///
/// Free, and that is the point: `Footer` owns a Wayland connection, a `Gpu` and
/// a keymap, so nothing about it can be stood up in a test. This defect is one
/// of POSITION, and position is arithmetic — the same reason
/// [`expansion_ceiling`] below is a free function.
///
/// The band starts at the expansion's first LINE and never at the surface's
/// top: the modeline, the minibuffer, the brain's line and the notification
/// band are above it, and the bar's column runs past all four. Testing the
/// column alone is what made the modeline's last cell a handle for a list
/// nowhere near it.
fn region_track_of(
    f: crate::chrome::Frame,
    bar: ronoctua_design::Scroll,
    first: u16,
    column_x: f32,
) -> (f32, f32, f32, f32) {
    let top = f.y_of(first);
    (
        column_x,
        top,
        f.cell(),
        bar.track(top, f.bottom(), f.line()),
    )
}

fn expansion_ceiling(screen_height: Option<u32>, fixed: usize) -> usize {
    let line_height = tokens::metrics::line_height(tokens::font::SIZE_BASE);
    let per_screen = screen_height.map_or(usize::MAX, |h| {
        let px = f64::from(h) * f64::from(tokens::space::EXPANSION_MAXFOOTERPCT) / 100.0;
        // MINUS WHAT IS FIXED: the percentage belongs to the whole footer, and
        // the modeline, the minibuffer and — while a countdown runs — the
        // brain's line are in it. Counting only the expansion, the footer went
        // past the limit the token itself names.
        //
        // It was the literal `2` for as long as two lines were the whole of the
        // fixed part. A number that has to be right in two places is a number
        // that will be wrong in one of them, which is why it is a PARAMETER
        // now: the caller counts what it is actually drawing.
        ((px / f64::from(line_height)) as usize).saturating_sub(fixed)
    });
    // Never zero: an expansion of zero lines is the list not opening, and the
    // user would have no way of telling that from the key not having caught.
    per_screen
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
/// `None` over the notification band, over the modeline, over the minibuffer
/// and past the last line: there is no item there, and inventing one would make
/// the message area describe something that is not under the cursor.
///
/// # Why the first line is a PARAMETER
///
/// Because it moves. The band above the modeline is one to three lines, or
/// none, and it changes while the pointer sits still. It used to be a constant,
/// and a constant here would make every row describe the one above it for as
/// long as a notification was on screen.
fn row_at_height(y: f64, line_height: f64, first: u16, how_many: usize) -> Option<usize> {
    if y < 0.0 || line_height <= 0.0 {
        return None;
    }
    let line = (y / line_height) as usize;
    let i = line.checked_sub(usize::from(first))?;
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
    first: u16,
    grid: Grid,
    how_many: usize,
) -> Option<usize> {
    let line = row_at_height(y, line_height, first, grid.lines)?;
    if cell_width_px <= 0.0 || x < 0.0 {
        return None;
    }
    let cell = (x / cell_width_px) as usize;
    let column = cell / usize::from(grid.width).max(1);
    if column >= grid.columns {
        return None;
    }
    let i = column * grid.lines + line;
    (i < how_many).then_some(i)
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
fn screen_difference<T: PartialEq>(screens: &[T], outputs: &[T]) -> (Vec<usize>, Vec<usize>) {
    let to_create = outputs
        .iter()
        .enumerate()
        .filter(|(_, s)| !screens.contains(s))
        .map(|(i, _)| i)
        .collect();
    let remove = screens
        .iter()
        .enumerate()
        .filter(|(_, t)| !outputs.contains(t))
        .map(|(i, _)| i)
        .collect();
    (to_create, remove)
}

/// The actions of an entry, minus the ones this machine cannot carry out.
///
/// An action whose program is not installed DISAPPEARS instead of failing when
/// clicked, and so does one that needs a file when there is none. An offer that
/// does nothing is worse than an offer that was never made — and `swappy` lives
/// in the `full` profile only, so this is the ordinary case on a minimal
/// install and not an edge.
///
/// # Why it is a free function
///
/// Because it was a method, it needed a `Footer`, and a `Footer` needs a
/// compositor — so the only way to find out what it decided was to build a VM
/// and take a photograph. It decided to drop everything, twice, for two
/// different reasons, and both times the symptom was a band two lines tall
/// where three were expected.
fn usable_actions(
    actions: &[ronoctua_notify::Action],
    n: &ronoctua_notify::Notification,
) -> Vec<ronoctua_notify::Action> {
    let file = n.file();
    actions
        .iter()
        .filter(|a| match &a.doing {
            ronoctua_notify::Doing::Run { program, args } => {
                crate::actions::notification_program(*program).is_some()
                    && (file.is_some()
                        || !args.iter().any(|x| {
                            matches!(
                                x,
                                ronoctua_notify::action::Arg::File
                                    | ronoctua_notify::action::Arg::Folder
                            )
                        }))
            }
            ronoctua_notify::Doing::CopyPath => file.is_some(),
            ronoctua_notify::Doing::Command(_) | ronoctua_notify::Doing::Invoke => true,
        })
        .cloned()
        .collect()
}

/// One piece of chrome text becomes ONE OR TWO runs.
///
/// # Why the glyph cannot be in the same run as the text
///
/// Because it is drawn BIGGER. In the real face the ink of `cod-bell` is 600
/// units tall against 751 for the digit `0`, so a bell at the text's size comes
/// out a fifth shorter than the number beside it and reads as a different,
/// smaller thing — which is exactly how it was reported from the screen. The
/// fix is `font.size.icon`, and a size belongs to a run.
///
/// The glyph is centred on its cell ([`ronoctua_render::Size::OnTheCell`]) and
/// the text beside it stays on the grid, so the LAYOUT never learns that two
/// sizes exist: it still counts one cluster per cell, and the arithmetic stays
/// integer.
///
/// Text with no leading glyph comes back as a single run, unchanged.
fn glyph_and_text(
    text: &str,
    column: u16,
    line: u16,
    color: tokens::Color,
    edge: bool,
) -> Vec<ronoctua_render::Span> {
    let Some((glyph, rest)) = ronoctua_render::layout::leading_glyph(text) else {
        return vec![ronoctua_render::Span {
            size: ronoctua_render::Size::Grid,
            text: text.to_owned(),
            column,
            line,
            color,
            edge,
            foot: false,
        }];
    };
    let mut out = vec![ronoctua_render::Span {
        size: ronoctua_render::Size::OnTheCell(tokens::font::SIZE_ICON),
        text: glyph.to_owned(),
        column,
        line,
        color,
        edge,
        foot: false,
    }];
    // An empty rest is the elided case — one cell left, and it went to the
    // glyph. An empty run would cost a `glyphon::Buffer` to draw nothing.
    if !rest.is_empty() {
        out.push(ronoctua_render::Span {
            size: ronoctua_render::Size::Grid,
            text: rest.to_owned(),
            column: column + 1,
            line,
            color,
            edge,
            foot: false,
        });
    }
    out
}

/// Each segment's color — emphasis by COLOR AND WEIGHT, never by a container
/// (section 5.1).
fn segment_color(id: &str, footer_with_keyboard: bool, state_is_base: bool) -> tokens::Color {
    match id {
        // THE KIND OF BUFFER. It used to carry the accent, from when it was the
        // only thing next to the name; now the state sits beside it saying what
        // is HAPPENING, and between "this is an icon browser" and "it is showing
        // a sheet" the second is the one that changed since you last looked.
        //
        // `secondary` AND NOT `dim`, and the VM is what said so: at `dim` the
        // OCR could not read `foot` off the screen any more — 0x6E6656 on the
        // modeline's 0x1E1E1E. A test reading it badly is a person reading it
        // badly. `dim` is for a segment on its way out of the elision, and the
        // mode is not on its way anywhere.
        "mode" => tokens::color::TEXT_SECONDARY,
        // THE STATE. Two colours and not one per state: what a reader has to
        // learn is "quiet means the usual, green means not", and that holds
        // whichever buffer they are in. See `assets/tokens/colors.nix`.
        "state" if state_is_base => tokens::color::MODELINE_STATE_NORMAL,
        "state" => tokens::color::MODELINE_STATE_ALTERNATE,
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
        "buffer" if footer_with_keyboard => tokens::color::STATE_ACCENT,
        "buffer" => tokens::color::TEXT_PRIMARY,
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
            tracing::warn!(key = next, "the buffer published a key that does not exist");
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

    /// THE BAR'S TRACK STARTS AT THE EXPANSION, and not at the top of the
    /// surface.
    ///
    /// The pointer test used to be the COLUMN alone, and that column runs the
    /// whole height: the notification band, the modeline, the minibuffer and
    /// the brain's line are all above the expansion and all have a last cell.
    /// Pressing any of them started a drag of a list nowhere near the pointer.
    #[test]
    fn the_bars_track_is_the_expansion_and_nothing_above_it() {
        // A surface that is not a whole number of lines, which is every real
        // one.
        for first in 1..=5u16 {
            let f = crate::chrome::Frame::of(900.0, 317.4, first);
            // THE EXPANSION IS WHAT IS LEFT UNDER THE FOUR LINES ABOVE IT.
            // `Footer::region_scroll` reads `grid().lines`, which is exactly
            // that; a fixture that put fewer rows there would be asking about
            // a band this door never draws.
            let bar = ronoctua_design::Scroll {
                total: 200,
                lines: usize::from(f.lines() - first),
                offset: 0,
                top: 0,
            };
            let (x, y, w, h) = super::region_track_of(f, bar, first, 888.0);
            assert!(
                (y - f.y_of(first)).abs() < 0.001,
                "first={first}: the track starts at {y} and the region at {}",
                f.y_of(first)
            );
            assert!(
                y > 0.0,
                "first={first}: the track reached the surface's top"
            );
            // AND IT ENDS AT THE SURFACE. The expansion is the last thing this
            // door draws, so the leftover under the last whole line is its.
            assert!(
                (y + h - f.bottom()).abs() < 0.001,
                "first={first}: the track ends at {} and the surface at {}",
                y + h,
                f.bottom()
            );
            assert_eq!(x, 888.0, "the column is the caller's");
            assert!((w - f.cell()).abs() < 0.001, "the bar is one cell wide");

            // AND A POINT ON THE MODELINE IS NOT ON IT.
            let on_the_line_above = f.y_of(first - 1) + 1.0;
            assert!(
                on_the_line_above < y,
                "first={first}: line {} is inside the track",
                first - 1
            );
        }
    }
    /// The focused buffer's keymap narrows like the rest of the which-key does.
    ///
    /// One row per distinct next key, a group when the sequence continues, a
    /// command when it ends — and NOTHING when the prefix does not match, which
    /// is what makes a key the buffer does not answer to end the sequence
    /// instead of opening an empty screen.
    #[test]
    fn the_buffer_keymap_narrows_by_prefix() {
        let bindings = vec![
            ("m f s".to_owned(), "memory-refresh".to_owned()),
            ("m f r".to_owned(), "memory-scan".to_owned()),
            ("m q".to_owned(), "memory-close".to_owned()),
        ];

        let root = super::narrow_buffer_keymap(&bindings, &["m"]);
        assert_eq!(root.len(), 2, "two distinct keys after `m`: {root:?}");
        let f = root.iter().find(|e| e.key == "f").expect("no `f`");
        assert!(f.is_group, "`m f` carries on, so it is a group");
        assert!(f.command.is_none(), "a group runs nothing");
        let q = root.iter().find(|e| e.key == "q").expect("no `q`");
        assert!(!q.is_group);
        assert_eq!(q.command.as_deref(), Some("memory-close"));
        assert_eq!(
            q.label, "cmd-memory-close",
            "the label is `cmd-<id>` (section 7.9)"
        );

        let under_f = super::narrow_buffer_keymap(&bindings, &["m", "f"]);
        assert_eq!(under_f.len(), 2);
        assert!(
            under_f.iter().all(|e| !e.is_group),
            "`m f s` e `m f r` terminam"
        );

        assert!(
            super::narrow_buffer_keymap(&bindings, &["m", "z"]).is_empty(),
            "a key the buffer does not answer must open no screen at all"
        );
    }

    /// A key the buffer publishes and this keymap cannot spell is DROPPED, and
    /// does not take the rest of the list with it.
    #[test]
    fn an_unspellable_key_does_not_lose_the_others() {
        let bindings = vec![
            ("m no-such-key".to_owned(), "x".to_owned()),
            ("m s".to_owned(), "memory-refresh".to_owned()),
        ];
        let rows = super::narrow_buffer_keymap(&bindings, &["m"]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "s");
    }

    use super::{
        BAND_CEILING, FOOTER_LINES, Footer, Grid, Inheritance, Measure, expansion_ceiling,
        grid_for, item_at, last_offset, navigation_step, row_at_height, runs_of, screen_difference,
        scrolling, tokens, usable_actions,
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
        let tall = AxisScroll {
            value120: 60,
            ..AxisScroll::default()
        };
        assert!((axis_detents(&tall, line) - 0.5).abs() < 1e-9);

        // THE OLD ONE, from an older compositor: an integer per detent.
        let old = AxisScroll {
            discrete: -2,
            ..AxisScroll::default()
        };
        assert!((axis_detents(&old, line) + 2.0).abs() < 1e-9);

        // THE CONTINUOUS one from the touchpad: finger pixels, converted by the
        // LINE HEIGHT. Three lines of finger = one detent.
        let finger = AxisScroll {
            absolute: line * 3.0,
            ..AxisScroll::default()
        };
        assert!((axis_detents(&finger, line) - 1.0).abs() < 1e-9);

        // AND A SMALL EVENT IS NOT ZERO. Truncating here would make the touchpad
        // never scroll: each of its events is worth a fraction, and what adds
        // them up is the accumulator.
        let crumb = AxisScroll {
            absolute: 2.0,
            ..AxisScroll::default()
        };
        assert!(axis_detents(&crumb, line) > 0.0);

        // With no line height there is no converting: zero, and not infinity.
        assert_eq!(axis_detents(&finger, 0.0), 0.0);
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
            "the Alt that opened the leader must not enter the sequence"
        );

        // AND WHEN THE ALT IS RELEASED, THE DISCOUNT GOES WITH IT.
        //
        // This is the half that did not exist, and it is what makes a `M-…`
        // binding possible AT ALL: the file manager's view is opened with
        // `LEADER .`, which in the nested session holds Ctrl and Alt, and
        // without this the discount lasted until the focus changed. `M-s`
        // inside the list would arrive as a bare `s` and be typed into the
        // query instead of sorting.
        h.on_announcement(Modifiers::default());
        assert!(
            h.discount(alt).alt,
            "an Alt released and pressed again HAS to count"
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
        assert!(!m.ctrl, "the inherited Ctrl goes");
        assert!(m.alt, "an Alt pressed afterwards STAYS");
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
            "without recapture, a stuck Alt joins every key — and nothing matches"
        );

        // WITH THE RECAPTURE ON STARTING THE SEQUENCE, it goes away.
        h.on_sequence_start(alt);
        assert!(
            !h.discount(alt).alt,
            "what was already held when the sequence began is not part of it"
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
        let (tree, _) = crate::leader::default_of().expect("the default keymap has to close");

        assert!(
            matches!(
                tree.resolve(&[Key::simple(Sym::Char('I'))]),
                Resolution::Nothing
            ),
            "if `I` becomes a shortcut, this test has lost its point"
        );

        let c = Footer::without_caps_lock('I', true);
        assert!(
            matches!(
                tree.resolve(&[Key::simple(Sym::Char(c))]),
                Resolution::Prefix(_)
            ),
            "with Caps Lock on, `LEADER i` has to open the icon group"
        );
    }

    #[test]
    fn each_output_gets_a_screen() {
        // The case the nested backend cannot reach: two monitors. The decision
        // of section 9.2 is one surface PER OUTPUT.
        let (to_create, remove) = screen_difference::<u32>(&[], &[1, 2]);
        assert_eq!(to_create, vec![0, 1], "both outputs need a screen");
        assert!(remove.is_empty());
    }

    #[test]
    fn an_output_already_covered_gets_no_second_screen() {
        // `new_output` and `update_output` call the same routine, and SCTK fires
        // both. Without this guard the same monitor would get two stacked
        // surfaces, and the exclusive zone would double.
        let (to_create, remove) = screen_difference(&[1, 2], &[1, 2]);
        assert!(to_create.is_empty());
        assert!(remove.is_empty());
    }

    #[test]
    fn hotplug_removes_the_screen_of_the_monitor_that_left() {
        let (to_create, remove) = screen_difference(&[1, 2], &[1]);
        assert!(to_create.is_empty());
        assert_eq!(remove, vec![1], "the screen of output 2 has to go");
    }

    #[test]
    fn swapping_monitors_creates_and_removes_in_the_same_pass() {
        // Unplugging one and plugging another between two events is common on a
        // dock. Both things have to happen, not only the first.
        let (to_create, remove) = screen_difference(&[1], &[2]);
        assert_eq!(to_create, vec![0]);
        assert_eq!(remove, vec![0]);
    }

    #[test]
    fn the_expansion_never_takes_the_screen() {
        // INVARIANT 14 as a number: the footer has to go on being a footer. On a
        // short screen, the absolute ceiling of 12 lines on its own would go past
        // half of it — and the list would stop having content behind it.
        let line_height = f64::from(tokens::metrics::line_height(tokens::font::SIZE_BASE));
        for height in [600_u32, 720, 800, 900, 1080, 1440, 2160] {
            let lines = expansion_ceiling(Some(height), usize::from(FOOTER_LINES));
            // THE WHOLE FOOTER: the two fixed lines count for whoever is looking.
            let px = (lines + usize::from(FOOTER_LINES)) as f64 * line_height;
            let limit =
                f64::from(height) * f64::from(tokens::space::EXPANSION_MAXFOOTERPCT) / 100.0;
            assert!(
                px <= limit,
                "at {height} px of screen the footer takes {px} px, more than the \
                 {limit} px the token allows"
            );
            assert!(lines >= 1, "an expansion of zero lines at {height} px");
        }
    }

    #[test]
    fn the_absolute_ceiling_holds_on_the_big_screen_and_the_percentage_on_the_small_one() {
        // Both limits exist because each one fails on its own: on a 2160 px
        // screen, 40% would give 33 lines (the footer would become the screen);
        // on a 600 px one, 12 lines would give more than half.
        let fixed = usize::from(FOOTER_LINES);
        assert_eq!(
            expansion_ceiling(Some(2160), fixed),
            usize::from(tokens::space::EXPANSION_MAXLINES),
            "on a big screen the absolute ceiling is what limits"
        );
        assert!(
            expansion_ceiling(Some(600), fixed) < usize::from(tokens::space::EXPANSION_MAXLINES),
            "on a small screen the percentage is what limits"
        );
        // Node output yet: the absolute one holds, which is the more conservative.
        assert_eq!(
            expansion_ceiling(None, fixed),
            usize::from(tokens::space::EXPANSION_MAXLINES)
        );
        // AND A THIRD FIXED LINE TAKES ONE OFF THE LIST, where the screen is
        // what limits. With the brain's line on, the footer is spending one
        // more row and the expansion is what gives it back — the literal `2`
        // could not say that.
        assert!(
            expansion_ceiling(Some(600), fixed + 1) < expansion_ceiling(Some(600), fixed),
            "the brain's line did not come out of the expansion's ceiling"
        );
    }

    #[test]
    fn the_scrolling_stops_at_the_ends_and_does_not_wrap() {
        // Wrapping around makes the eye lose its place: reaching the end and going
        // back to the top is indistinguishable from the list having changed.
        let last = last_offset(30, 12); // 18
        assert_eq!(last, 18);
        assert_eq!(scrolling(0, last, -1), 0, "it came up from the top");
        assert_eq!(scrolling(last, last, 1), last, "it went past the end");
        assert_eq!(scrolling(0, last, 12), 12);
        assert_eq!(
            scrolling(12, last, 12),
            18,
            "the last page stops at the end"
        );
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
        assert_eq!(g.colon, 5 + crate::chrome::INSET + 1);
        for width in [1_u16, 3, 5] {
            let start = g.colon - (width + 1);
            assert_eq!(start + width + 1, g.colon);
        }
        // And the widest one starts exactly at the margin — with no dead gap.
        assert_eq!(g.colon - (5 + 1), crate::chrome::INSET);
    }

    #[test]
    fn the_shortcut_list_uses_the_width_before_the_height() {
        // DOOM'S ARRANGEMENT. Ten shortcuts in a region of twelve lines would fit
        // in a single column — a narrow strip with the whole screen empty beside
        // it, and still paginating once they went past twelve. Using the columns
        // that fit, the same ten become a few lines.
        let items: Vec<Measure> = std::iter::repeat_n(measurement(3, 8), 10).collect();
        let g = grid_for(&items, 200, 12, true);
        assert!(
            g.columns > 1,
            "it stayed in a single column on a wide screen"
        );
        assert!(
            g.lines < 10,
            "it used {} lines for 10 items: it did not use the width",
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
        assert_eq!(g.lines, 12, "the list should fill the ceiling");
    }

    #[test]
    fn the_grid_does_not_overflow_the_height_ceiling() {
        // Invariant 14 as a number: even with many items and a narrow screen, the
        // region stops at the ceiling.
        let items: Vec<Measure> = std::iter::repeat_n(measurement(3, 8), 500).collect();
        for width in [40_u16, 80, 200, 400] {
            let g = grid_for(&items, width, 12, true);
            assert!(g.lines <= 12, "{width} columns gave {} lines", g.lines);
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
        for screen_columns in [40_u16, 80, 120, 176, 300] {
            let g = grid_for(&items, screen_columns, 12, true);
            for i in 0..g.per_page() {
                let (base, _) = g.position(i);
                for with_key in [true, false] {
                    let end = base
                        + g.text_start(with_key)
                        + g.fits_in_text(with_key, base, screen_columns);
                    assert!(
                        end <= screen_columns,
                        "at {screen_columns} columns, item {i} ends at {end}"
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
        assert!(
            g.columns > 1,
            "a wide screen should give more than one column"
        );
        for with_key in [true, false] {
            let used = g.text_start(with_key) + g.fits_in_text(with_key, 0, 300);
            assert!(
                used <= g.width,
                "the text uses {used} of a column of {}",
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
        assert!(g.width > 0, "a width of zero divides by zero further on");
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
        // With no band, the expansion starts right after the footer's two lines.
        let y0 = f64::from(FOOTER_LINES) * h;
        // First column, first row.
        assert_eq!(item_at(0.0, y0, h, c, FOOTER_LINES, g, 12), Some(0));
        // First column, third row.
        assert_eq!(
            item_at(0.0, y0 + 2.0 * h, h, c, FOOTER_LINES, g, 12),
            Some(2)
        );
        // SECOND column, first row: the fifth item, and not the first.
        assert_eq!(item_at(20.0 * c, y0, h, c, FOOTER_LINES, g, 12), Some(4));
        // Third column, last row.
        assert_eq!(
            item_at(40.0 * c, y0 + 3.0 * h, h, c, FOOTER_LINES, g, 12),
            Some(11)
        );
        // Past the last column: there is no item there.
        assert_eq!(item_at(80.0 * c, y0, h, c, FOOTER_LINES, g, 12), None);
        // And past the end of the list, even inside the grid.
        assert_eq!(
            item_at(40.0 * c, y0 + 3.0 * h, h, c, FOOTER_LINES, g, 5),
            None
        );
    }

    #[test]
    fn the_cursor_only_points_at_a_row_that_exists() {
        // The modeline and the minibuffer are NOT items. Giving back `0` for them
        // would make the message area describe the list's first line while the
        // pointer is over the buffer's name.
        let h = 26.0;
        let n = 4;
        assert_eq!(
            row_at_height(0.0, h, FOOTER_LINES, n),
            None,
            "the modeline became an item"
        );
        assert_eq!(
            row_at_height(h + 1.0, h, FOOTER_LINES, n),
            None,
            "the minibuffer became an item"
        );
        assert_eq!(
            row_at_height(f64::from(FOOTER_LINES) * h, h, FOOTER_LINES, n),
            Some(0)
        );
        assert_eq!(
            row_at_height((f64::from(FOOTER_LINES) + 3.0) * h, h, FOOTER_LINES, n),
            Some(3)
        );
        // Past the list's last line: there is no item there.
        assert_eq!(
            row_at_height((f64::from(FOOTER_LINES) + 4.0) * h, h, FOOTER_LINES, n),
            None
        );
        // And with the region closed, nothing is pointable.
        assert_eq!(row_at_height(100.0, h, FOOTER_LINES, 0), None);
    }

    #[test]
    fn an_action_that_needs_a_file_only_survives_with_one() {
        // THE DEFECT THIS EXISTS FOR: the whole row of actions vanished, and
        // the only symptom was a band two lines tall where three were expected.
        // Nothing failed, nothing logged, and finding out cost a VM and a
        // photograph — because this was a method and a method needs a `Footer`.
        use ronoctua_notify::action::{Arg, Doing, Program};
        let actions = vec![
            ronoctua_notify::Action {
                id: "open".into(),
                key: Some('a'),
                label: ronoctua_notify::Label::Key("k".into()),
                doing: Doing::Run {
                    program: Program::Open,
                    args: vec![Arg::File],
                },
            },
            ronoctua_notify::Action {
                id: "copy-path".into(),
                key: Some('c'),
                label: ronoctua_notify::Label::Key("k".into()),
                doing: Doing::CopyPath,
            },
            ronoctua_notify::Action {
                id: "panel".into(),
                key: Some('m'),
                label: ronoctua_notify::Label::Key("k".into()),
                doing: Doing::Command("memory-open".into()),
            },
        ];
        let with_file = ronoctua_notify::Notification::fresh_one(
            1,
            "niri".into(),
            Some("file:///home/x/a%20b.png".into()),
            "s".into(),
            String::new(),
            ronoctua_notify::Urgency::Normal,
        );
        let kept: Vec<String> = usable_actions(&actions, &with_file)
            .iter()
            .map(|a| a.id.clone())
            .collect();
        // `open` only survives where `xdg-open` was baked in, which is every
        // build made through the flake and not every `cargo test`. The two that
        // do not depend on a program have to be there in both.
        assert!(
            kept.iter().any(|k| k == "copy-path"),
            "copy-path is missing: {kept:?}"
        );
        assert!(
            kept.iter().any(|k| k == "panel"),
            "our own command is missing: {kept:?}"
        );

        // WITHOUT A FILE, the two that need one go and the command stays.
        let bare = ronoctua_notify::Notification::fresh_one(
            1,
            "app".into(),
            Some("dialog-information".into()),
            "s".into(),
            String::new(),
            ronoctua_notify::Urgency::Normal,
        );
        let kept: Vec<String> = usable_actions(&actions, &bare)
            .iter()
            .map(|a| a.id.clone())
            .collect();
        assert_eq!(kept, vec!["panel".to_owned()], "{kept:?}");
    }

    #[test]
    fn the_band_pushes_the_whole_list_down() {
        // THE DEFECT THIS EXISTS FOR: with the first line a constant, every row
        // of the expansion would describe the one above it for as long as a
        // notification was on screen — and nothing would look broken, the
        // cursor would simply be one line off.
        let h = 26.0;
        let n = 4;
        for band in 0..=BAND_CEILING {
            let first = FOOTER_LINES + band;
            // The band's own lines are not items.
            for line in 0..first {
                assert_eq!(
                    row_at_height(f64::from(line) * h + 1.0, h, first, n),
                    None,
                    "with a band of {band} lines, line {line} became an item"
                );
            }
            assert_eq!(row_at_height(f64::from(first) * h, h, first, n), Some(0));
        }
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
        let c = runs_of("window", &[0, 1]);
        assert_eq!(
            c,
            vec![("wi".to_owned(), 0, true), ("ndow".to_owned(), 2, false)]
        );

        // The columns are the CLUSTER's index: one cell each, the same ruler as
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
                "{sym:?} should do nothing with CtrlRight held"
            );
        }
    }
}
