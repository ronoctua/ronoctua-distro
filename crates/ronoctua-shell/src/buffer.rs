//! A native buffer hosted in an `xdg_toplevel` of its own — section 7.5.
//!
//! # Why an `xdg_toplevel`, and not a region of our surface
//!
//! Invariant 4 is explicit: **the shell never reimplements window
//! layout**. A native buffer is a WINDOW like any other, and what
//! decides where it goes is niri. Drawing buffers inside a fullscreen
//! surface of ours would throw the compositor away — and section 7.5 forbids that
//! by name.
//!
//! The practical consequence: a native buffer appears in niri's window
//! column, stacks, resizes and closes like everything else. There is nothing special
//! about it from the compositor's point of view, and that is how it should be.
//!
//! # The cycle
//!
//! ```text
//! .wasm component  ->  instance  ->  render(ctx)  ->  DisplayList
//!                                                            |
//!                   tokens and i18n resolved HERE  <---------+
//!                                                            |
//!                                             wgpu on the xdg_toplevel
//! ```
//!
//! The component sees no token, no language and no Wayland (invariant 11).
//! What resolves all three is this file.

use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::calloop::EventLoop,
    reexports::calloop::channel as calloop_channel,
    // The clock, for the buffer that samples the system. It reaches here
    // through the toolkit's re-export, like everything else from calloop —
    // depending on the crate directly would risk two versions of it.
    reexports::calloop::timer,
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

// THE BRIDGE MOVED OUT, and this is the only copy: see `paint`. The
// terminal already drew display lists through it, and the offscreen tool
// that writes a buffer to a PNG needs the same conversion — a second one
// would prove things about itself.
use crate::paint::{cell, quads_of, spans_of, token_color};

/// A vector drawing request, with the area already in pixels.
struct VectorDrawing {
    /// The cache key — see [`svg_key`], which says what is in it and why.
    ///
    /// NOT the SVG's string, though it used to be and this line used to say
    /// so. The document is what is DRAWN; the key has to carry everything the
    /// raster depends on, which is also the size and the ink.
    key: String,
    /// Where it fits, already fitted without stretching.
    area: ronoctua_render::image::Area,
}

/// The pixmap of a [`ronoctua_protocol::Command::Svg`], ready for the GPU.
///
/// # Why it is a function, and not three lines inside the loop
///
/// Because it carries **two decisions the screen does not give away**, and both have
/// already been wrong:
///
/// - **`Natural`, and not `Fill`.** The rectangle is the CEILING, and the drawing comes out
///   at the size the document itself declares. Filling, a `width="24"`
///   and a `width="512"` give the same image — Lucide's size adjustment had
///   no visible effect at all, and the preview confidently showed a size
///   that was not the chosen one.
/// - **The `currentColor` resolved** before rasterising, otherwise the drawing comes out
///   black over the buffer's background.
///
/// Inside the loop, neither of the two could be exercised: `vector_ones`
/// needs a Wayland window. Here they are testable, and it is the only way
/// for the wrong choice to break something before the screen does.
fn rasterize_preview(
    svg: &str,
    color: tokens::Color,
    side: u32,
) -> Option<ronoctua_render::image::Rgba> {
    let document = resolver_current_color(svg, color);
    let r = ronoctua_notify::icon::from_svg(
        document.as_bytes(),
        side,
        ronoctua_notify::icon::Fit::Natural,
    )?;
    Some(ronoctua_render::image::Rgba {
        width: r.width,
        height: r.height,
        pixels: r.pixels,
    })
}

/// The cache key of a rasterised SVG: the size, the theme's position, then the
/// document.
///
/// The document alone was the key, and it was exact for the drawing — the
/// icon and the five adjustments are all in it. What it did not carry was the
/// ink: `currentColor` is resolved to the CURRENT theme's value below, and a
/// key without the theme kept the old ink on the GPU after a switch, because
/// the upload only happens for a key the cache is missing.
///
/// AND IT DID NOT CARRY THE SIZE EITHER, which the day `pixmap_side` and the
/// cell stopped naming the anchor made a second way for the same defect to
/// happen: the raster is `side` pixels square, `side` comes from the cell,
/// and the cell asks `text::size()`. The same document at 200% is a different
/// picture, and a cache that could not tell them apart kept the small one —
/// which `Area::centre` then draws at its natural size inside a doubled box.
///
/// AND THE TOKEN IS THE OTHER HALF OF THE INK. The theme was put in on the
/// argument that it is what makes the key carry the colour, and that is only
/// half true: the ink is `theme x token`, and `resolver_current_color` takes
/// BOTH. Two `Command::Svg` with the same document and different tokens — one
/// drawing `TextPrimary` in a row and `StateError` in a warning, or the same
/// drawing selected and unselected — spell one key, and the second is served
/// the first's raster in the first's colour. No later change can repair it,
/// because the key never moves. Latent while the one buffer that draws an SVG
/// emits exactly one with a fixed token, and the WIT permits any number.
fn svg_key(side: u32, theme_position: usize, token: tokens::Color, svg: &str) -> String {
    // The token is a ROLE, and `name` is what identifies it — `text.dim`,
    // not the bytes it happens to resolve to in this theme. Two roles that
    // share bytes in one theme are still two roles, which is what `Color`'s
    // own equality means, and a key built from the VALUE would merge them.
    format!("{side}:{theme_position}:{}:{svg}", token.name())
}

/// The SVG's `currentColor` resolved to a value.
///
/// # Why the substitution belongs to the host
///
/// Because the colour crosses the boundary as a **token**, never as a value
/// (invariant 11). And because `currentColor` with no context is **black**: over the
/// buffer's background, the drawing would come out invisible — which is indistinguishable from "this
/// primitive is not implemented yet", the worst possible result.
///
/// When the person has typed a colour of their own, there is no `currentColor` in the
/// document and nothing is replaced: there the colour is **content**, and the drawing comes out
/// as the artefact would.
fn resolver_current_color(svg: &str, color: tokens::Color) -> String {
    // `#RRGGBB`, which is what SVG understands. The token's alpha does not travel: an
    // accidentally semitransparent icon would look like a rendering error.
    let [r, g, b, _] = color.to_rgba8();
    let hex = format!("#{r:02X}{g:02X}{b:02X}");
    svg.replace("currentColor", &hex)
}

/// The state of the window hosting a buffer.
struct Host {
    conn: Connection,
    /// The loop, for the one thing that needs it here: a keyboard that
    /// REPEATS. SCTK only sends repeat events for a keyboard made with
    /// `get_keyboard_with_repeat`, and that wants a `LoopHandle` — without it
    /// the `repeat_key` below is dead code, and holding the arrow in a buffer
    /// moved exactly one row while the comment beside it said otherwise.
    handle: smithay_client_toolkit::reexports::calloop::LoopHandle<'static, Self>,
    /// The watch on `theme.json` — the file is the broadcast (see `theme.rs`).
    /// The component is told nothing: it never had a value (invariant 11).
    _theme_watch: Option<ronoctua_files::notice::Notice>,
    /// `display.json`, for the size of the text.
    _size_watch: Option<ronoctua_files::notice::Notice>,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    /// The pointer, so the click reaches the buffer's items.
    pointer: Option<wl_pointer::WlPointer>,
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
    quit: bool,
    /// The file this buffer was opened with, when there is one.
    ///
    /// It is the HOST that holds it, and not the component: `Effect::Save`
    /// carries only the text, and the buffer never learns a path. That is what
    /// keeps invariant 11 intact while a file editor exists — the same shape as
    /// `Effect::Copy`, where the buffer hands text over and the host decides
    /// where it goes.
    file: Option<std::path::PathBuf>,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    /// The image pipeline, for [`ronoctua_protocol::Command::Svg`].
    ///
    /// The buffer window only knew how to draw text. It is born along with
    /// `text`, in the same `assemble_gpu`, and stays `None` while there is no GPU —
    /// for the same reason as it.
    images: Option<ronoctua_render::image::Images>,
    /// The flat-rectangle pipeline, for `Command::Selection`.
    ///
    /// It had been missing since the primitive existed: the host answered
    /// `Vec::new()` for it, in silence. A buffer that used it drew nothing and
    /// never found out — which is worse than a primitive that does not exist,
    /// because the WIT says it does.
    quads: Option<ronoctua_render::quad::Quads>,
    list: ronoctua_protocol::DisplayList,
    /// The already-rasterised vector drawings, by [`svg_key`].
    ///
    /// THE KEY IS NOT THE DOCUMENT, and the sentence that stood here — "the
    /// key is the whole document, and it is exact: it carries the icon and the
    /// five adjustments" — is the exact reasoning that let the theme, then the
    /// size, then the colour token fall out of it, one at a time. It is exact
    /// about the DRAWING and says nothing about the raster, which also depends
    /// on how big it is and what colour it is being drawn in.
    ///
    /// Nothing evicts from this map; a theme change clears it whole. With the
    /// size and the token in the key it now holds one pixmap per (document,
    /// size, token) for the process's life — bounded in practice by a glyph
    /// browser showing one icon at a time, and unlike the GPU's `Images` it
    /// has no ceiling of its own. Worth a `dump()` of its own if a buffer ever
    /// draws many.
    rasterized: std::collections::HashMap<String, ronoctua_render::image::Rgba>,
    /// What the descriptor ASKED FOR. See [`ronoctua_protocol::DisplayList::validate_it`].
    capabilities: Vec<ronoctua_protocol::Capability>,
    /// The buffer's OWN keymap, straight from the descriptor.
    ///
    /// # Why the buffer's host resolves it, and not the footer
    ///
    /// A buffer is a window of its own (invariant 4), and while it has the
    /// focus it is THIS process that holds the keyboard — the footer only gets
    /// it back when the leader is pressed. A key typed in here never reaches the
    /// footer, so a keymap resolved over there would never fire.
    ///
    /// It stayed declared and unreachable until an editor needed to SAVE, which
    /// is when "declares and nothing consumes it" stopped being tolerable: the
    /// buffer would open, edit, and lose the work.
    keymap: Vec<ronoctua_protocol::Binding>,
    /// The sequence typed so far, waiting to complete a binding.
    pending: Vec<char>,
    texts: ronoctua_i18n::Texts,
    /// The LIVE component.
    ///
    /// The host used to call `render` once and stop: no key reached
    /// the buffer, and the glyph browser was an image — there was no way to
    /// search, scroll or choose. The standalone `Runtime::render` on top of that
    /// creates a new store and clears the state on every call (see AGENTS.md).
    instance: ronoctua_runtime::Instance,
    /// Kept because `Instance` does not return them, and dropping them
    /// would take the component down.
    _runtime: ronoctua_runtime::Runtime,
    /// What a buffer with [`ronoctua_protocol::Capability::System`] is told,
    /// and what it asked for. `None` for every other buffer — which is the
    /// whole of the cost this capability imposes on the ones that never use it.
    system: Option<System>,
    /// The same, for [`ronoctua_protocol::Capability::Transfers`]: a
    /// subscription to the queue's socket, and the ids it may act on.
    queue: Option<Queue>,
    /// The same, for [`ronoctua_protocol::Capability::Disks`]: a subscription
    /// to udisks, and the devices it may write over.
    disks: Option<Disks>,
    /// The same, for [`ronoctua_protocol::Capability::Brain`]: a subscription
    /// to the daemon, the tasks it may name and the preferences it may change.
    brain: Option<Brain>,
    /// The same, for [`ronoctua_protocol::Capability::Keyboard`]: what the
    /// kernel reports and what the person decided, and the gate for both.
    keyboards: Option<Keyboards>,
    /// The states the descriptor declared, in order. The first is the base.
    ///
    /// Never empty: [`ronoctua_protocol::Descriptor::validate`] refuses an empty
    /// list at load, which is what lets every reader here index without a guard.
    states: Vec<ronoctua_protocol::BufferState>,
    /// Which state is in effect, as an index into [`Self::states`].
    ///
    /// SHARED with the socket thread, which answers `states` to the footer and
    /// has no other way to know. An index and not a string because that makes
    /// it an atomic — a lock held across a socket read is a lock the frame can
    /// wait on.
    current: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// `Esc`: whether it is down, and whether it has been used as a modifier.
    escape: Escape,
}

/// `Esc` held, and what has happened while it was.
///
/// # Why a tap and a hold, and why it is resolved HERE
///
/// `Esc` had one meaning — cancel — and the state machine needs two: go back to
/// the base state, and choose among the states. Every other key in this system
/// that has two meanings gets them from `keyd`, which tells a tap from a hold
/// before the compositor sees anything. That is not available for this one: a
/// `keyd` layer on `Esc` would change `Esc` for the terminal, for `vim` and for
/// every page in the browser, to give a shortcut to six windows.
///
/// So it is resolved in the client that already has the keyboard. While a
/// buffer of ours has the focus, THIS process receives the keys; while anything
/// else does, none of this exists. That is the whole reason the feature can have
/// `Esc` at all.
///
/// The cost is that the tap acts on RELEASE. For a tap that is the time between
/// pressing and letting go, and it is the same contract the leader already
/// teaches on `Super`.
#[derive(Debug, Default, Clone, Copy)]
struct Escape {
    /// It is down right now.
    down: bool,
    /// Something was pressed while it was down, so letting go is not a tap.
    used: bool,
}

/// What the `Esc` layer decided about a key.
///
/// It exists so the decision is a PURE FUNCTION and can be tested without a
/// compositor: `Host` needs a Wayland connection, a GPU and a live wasm
/// component to exist at all, and the three cases below are the whole of the
/// behaviour anybody would want to check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EscapeAct {
    /// Not this layer's: the key goes on to the rest of the handler.
    NotMine,
    /// Eaten, and nothing else happens.
    Swallow,
    /// Rotate the state; `true` is forward.
    Rotate(bool),
    /// A tap finished: back to the base state, and `Cancel` as before.
    Tap,
}

impl Escape {
    /// A key going down.
    fn press(&mut self, sym: Keysym) -> EscapeAct {
        if sym == Keysym::Escape {
            // A REPEAT IS NOT A SECOND PRESS. Holding the key makes the
            // compositor send more of them, and treating one as a fresh press
            // would clear `used` — so `Esc`+arrow, held a moment too long, would
            // rotate and then cancel on release.
            if !self.down {
                *self = Self {
                    down: true,
                    used: false,
                };
            }
            return EscapeAct::Swallow;
        }
        if !self.down {
            return EscapeAct::NotMine;
        }
        // WHILE IT IS HELD, THIS BUFFER IS CHOOSING A STATE and nothing else
        // reaches the component. Swallowing the keys that are not arrows is what
        // makes the gesture safe: without it, letting go after `Esc`+letter
        // would cancel AND the letter would have gone into the search on the way
        // past — one gesture doing two things nobody asked for.
        self.used = true;
        match sym {
            Keysym::Right | Keysym::Down => EscapeAct::Rotate(true),
            Keysym::Left | Keysym::Up => EscapeAct::Rotate(false),
            _ => EscapeAct::Swallow,
        }
    }

    /// A key coming up.
    fn release(&mut self, sym: Keysym) -> EscapeAct {
        if sym != Keysym::Escape {
            return EscapeAct::NotMine;
        }
        // `down` IS CHECKED, and it is not paranoia: a key held while the focus
        // ARRIVES gives this window the release without ever giving it the
        // press. Without the check that release would cancel something nobody
        // asked to cancel — the same trap `AGENTS.md` records for the leader,
        // where a modifier held across a focus change has to be discounted.
        let was = std::mem::take(self);
        if was.down && !was.used {
            EscapeAct::Tap
        } else {
            EscapeAct::Swallow
        }
    }
}

/// The queue half of the host: one subscription, and the id check.
///
/// It owns no list. The unit `ronoctua-transfers` owns it, this process is
/// merely subscribed, and a copy of forty gibibytes carries on whether this
/// window is open or not.
struct Queue {
    /// The ids of the last rows delivered.
    ///
    /// THE GATE, and it is the same discipline `stop_consumer` uses for an
    /// index: the buffer names an id, and the host checks it against what it
    /// actually sent before passing it on. Without it a component could name
    /// any number and cancel a job that is not even on its screen.
    known: Vec<u64>,
}

impl Queue {
    const fn fresh() -> Self {
        Self { known: Vec::new() }
    }
}

/// What a buffer with [`ronoctua_protocol::Capability::Disks`] may act on.
struct Disks {
    /// The ids of the last devices delivered, and whether each may be WRITTEN.
    ///
    /// A pair and not a set, and the second half is the whole point: a volume
    /// the running system needs IS sent — somebody looking for their disk has
    /// to find it — and it is sent with `false`. The gate is therefore not
    /// "did I send this id" but "did I send it as a target", which is a
    /// different question and the one that matters.
    known: Vec<(String, bool)>,
    /// The last whole reading, kept for one thing: finding what else is mounted
    /// on the drive about to be wiped.
    ///
    /// It cannot be re-derived from `known`, which carries ids and no topology
    /// — and asking udisks again at the moment of formatting would be reading
    /// the machine a second time to answer a question already answered.
    rack: Option<ronoctua_disks::Rack>,
    /// Where a worker thread puts what became of a format.
    ///
    /// `None` until the loop is running, because the channel cannot exist
    /// before there is a loop to hang it on.
    ///
    /// IT CARRIES THE FACT AND NOT THE SENTENCE. `Texts` holds a parsed Fluent
    /// bundle and is not `Send`, so a thread cannot word its own refusal — and
    /// it should not: the wording belongs where the bundle is, which is the
    /// same split `files::Report::Refused` already makes.
    said: Option<calloop_channel::Sender<Verdict>>,
}

/// What a format came back as.
enum Verdict {
    /// It was written.
    Done,
    /// udisks refused, by NAME — and its own prose alongside, because
    /// "could not format" alone does not say "something has a file open on it".
    ///
    /// # The disk's name travels WITH the refusal
    ///
    /// It was a field on the host at first, set before the thread started. That
    /// is a place two formats can collide: the second overwrites it, and the
    /// first comes back to be worded with the wrong disk in the sentence. It
    /// also went stale by construction — the format may be what changed the
    /// label. Carried here, the name is the one the person pressed.
    Refused {
        which: ronoctua_disks::Refusal,
        why: String,
        name: String,
    },
}

impl Disks {
    const fn fresh() -> Self {
        Self {
            known: Vec::new(),
            rack: None,
            said: None,
        }
    }
}

/// What a buffer with [`ronoctua_protocol::Capability::Keyboard`] may act on.
///
/// # The gate, and why it is three lists
///
/// The buffer receives the keyboards, the keys and the choices FROM here, and
/// hands three strings back. So the question at the moment of acting is not
/// "is this a keyboard" but "is this one of the ones I sent" — and the same
/// for the key and for what it becomes. A component that invented any of the
/// three is refused, which is the discipline `Disks::known` follows one
/// capability over.
///
/// It matters more here than it looks: what is written is the file a daemon
/// between the keyboard and everything else reads.
struct Keyboards {
    /// The ids of the last keyboards delivered.
    boards: Vec<String>,
    /// The keyd names of the keys offered, and of what they can become.
    keys: Vec<String>,
    choices: Vec<String>,
    /// Why the last write did not go through, already a text key. Empty when
    /// it did.
    trouble: String,
}

impl Keyboards {
    const fn fresh() -> Self {
        Self {
            boards: Vec::new(),
            keys: Vec::new(),
            choices: Vec::new(),
            trouble: String::new(),
        }
    }
}

/// What a buffer with [`ronoctua_protocol::Capability::Brain`] may act on.
///
/// It owns no schedule. The unit `ronoctua-brain` owns it, this process is
/// merely subscribed, and a rebuild in flight carries on whether this window is
/// open or not — the same relation [`Queue`] has to the transfer unit.
struct Brain {
    /// The ids of the last rows delivered, in the order they were sent.
    ///
    /// THE GATE, and it is the same discipline `stop_consumer` and `job_demand`
    /// use: the component names a 1-BASED INDEX and the host resolves it
    /// against the list it actually sent. `Demand::target`'s own doc says so —
    /// an index, never a name the buffer built — and an index checked against a
    /// stale list is the wrong task.
    tasks: Vec<String>,
    /// The preferences as the daemon last reported them.
    ///
    /// Kept because `Ask::Tune` carries the WHOLE record and the component
    /// sends ONE knob: the change is applied on top of what the brain last
    /// said, so two screens changing two different fields do not undo each
    /// other. It is also why the file the daemon writes never grows a field
    /// this side invented.
    prefs: ronoctua_brain::Prefs,
    /// The machine's zone, read once.
    ///
    /// The component has none (invariant 11), so the dates are formatted here.
    /// Once and not per push: reading `/etc/localtime` on every change would be
    /// a file opened for every beat of a countdown.
    zone: ronoctua_files::when::Zone,
    /// The last mind delivered, so an unchanged one costs a comparison.
    ///
    /// # Why the host defends itself here
    ///
    /// Because the window has to answer the compositor whatever the thing on
    /// the other end of the socket is doing, and it did not. A defect in the
    /// daemon's own loop made it publish hundreds of thousands of times a
    /// second, and each one was taken at face value: the record converted for
    /// the WIT, the component called, the screen redrawn. The window stopped
    /// resizing when the minibuffer opened and took seconds to close — which
    /// is not a slow buffer, it is an event loop with no time left for
    /// Wayland.
    ///
    /// That loop is fixed. This is the half that means it cannot happen again
    /// from ANY producer: an unchanged mind ends here, at one comparison, and
    /// never reaches the conversion, the component or a frame. It is the rule
    /// `buffers/memory` already states about its survey — the numbers arrive on
    /// a clock, and the screen moves when they CHANGED.
    last: Option<ronoctua_brain::Mind>,
}

impl Brain {
    fn fresh() -> Self {
        Self {
            tasks: Vec::new(),
            prefs: ronoctua_brain::Prefs::default(),
            zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
            last: None,
        }
    }
}

/// The system half of the host: sampling, scanning and the policy.
///
/// It only exists for a buffer that declared the capability. Everything in it
/// costs something — a clock, a thread, a process spawned to read systemd —
/// and none of it should exist for a glyph browser.
struct System {
    /// Where `/proc` and the cgroups are. A field, and not a constant, because
    /// that is what makes the parsers testable against a tree a test wrote.
    places: ronoctua_memory::Places,
    /// The policy, as systemd last reported it.
    ///
    /// Cached on purpose: reading it spawns `systemctl` and `journalctl`, which
    /// is fine for a key press and not for a clock. It is refreshed when the
    /// buffer asks and after every change.
    guard: ronoctua_protocol::Guard,
    /// The last scan's result, kept so a redraw does not re-scan.
    consumers: Vec<ronoctua_protocol::Consumer>,
    /// A scan in flight, and how far it got, in per mille.
    scanning: bool,
    progress: u16,
    /// Which scan produced the list on screen.
    ///
    /// Kept so that closing something re-runs THAT scan: the list is stale the
    /// moment an entry leaves it, and re-scanning the other one would swap the
    /// screen's contents as a side effect of a key that meant "close this".
    last_scan: Option<Scan>,
    /// What tells a running scan to stop.
    ///
    /// Shared with the thread. Dropping it is not enough: a walk of a home
    /// directory would keep consuming I/O long after the screen moved on.
    cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// How the scan thread answers.
    reports: Option<calloop_channel::Sender<Scanned>>,
    /// This machine's RAM, from the last survey.
    ///
    /// The guard's ceiling comes back from systemd in BYTES, and turning it
    /// into the percentage the screen shows needs the total. Kept here so
    /// reading the policy does not have to read `/proc/meminfo` again.
    ram_total: u64,
}

/// What a scan thread sends back.
enum Scanned {
    /// How far it got.
    Step(u16),
    /// It finished, or it was stopped: what it had found either way.
    Done(Vec<ronoctua_protocol::Consumer>),
}

/// The host's input in the form the buffer understands.
///
/// # Why the buffer does not see a `Key`
///
/// Invariant 11 and §7.9: the component does not capture keys, it receives intent.
/// An arrow becomes `Up`, `Enter` becomes `Confirm` — that way a buffer does not need
/// to know about keysyms, modifiers or keyboard layout.
#[must_use]
fn input_of_key(sym: Keysym, utf8: Option<&str>) -> Option<ronoctua_protocol::Input> {
    use ronoctua_protocol::Input;
    Some(match sym {
        Keysym::Up => Input::Up,
        Keysym::Down => Input::Down,
        Keysym::Left => Input::Left,
        Keysym::Right => Input::Right,
        Keysym::Return | Keysym::KP_Enter => Input::Confirm,
        Keysym::Escape => Input::Cancel,
        Keysym::BackSpace => Input::Delete,
        // `ISO_Left_Tab` is what xkb delivers on `Shift+Tab`. Both go
        // to the same place: without this the second would fall into the `_` and enter
        // the search as a character.
        Keysym::Tab | Keysym::ISO_Left_Tab => Input::Toggle,
        _ => Input::Char(utf8?.chars().next()?),
    })
}

/// Puts a text on the system clipboard.
///
/// # Why a process, and not a `wl_data_device` of our own
///
/// Because whoever **owns** the selection has to stay alive to serve it
/// when someone pastes. The buffer's window dies when the browser is closed
/// — and copying a glyph **in order to** close the browser and paste is exactly the
/// use case. An owner that dies with it would hand over an empty clipboard, which
/// is worse than not copying: it looks as if it worked.
///
/// `wl-copy` leaves a minimal process serving the selection until someone
/// replaces it. It is the same choice as `niri msg action`: using the client that
/// already exists instead of reimplementing the other side's protocol.
///
/// # Errors
///
/// The binary was not put in the build, did not run, or exited with an error.
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    use std::io::Write as _;

    // A STORE PATH, never a name: the `PATH` of a systemd scope is
    // minimal, and the failure would show up far from the cause.
    let path = option_env!("RONOCTUA_WL_COPY").context(
        "RONOCTUA_WL_COPY was not set at build time.\n\
             It comes from `nix/rust.nix`; without it there is no way to copy.",
    )?;

    let mut cmd = std::process::Command::new(path);
    // ITS PATH, and not ours. See `RONOCTUA_WL_COPY_PATH` in
    // `nix/rust.nix`: `wl-copy` does `exec cat`, and under a systemd
    // unit PATH is minimal. Without this the copy fails with
    // `exec cat: Node such file or directory` — and failing silently here
    // would be going back to the defect this whole path fixes.
    if let Some(p) = option_env!("RONOCTUA_WL_COPY_PATH") {
        cmd.env("PATH", p);
    }
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        // INHERITED, and not a pipe of ours.
        //
        // Why this matters: `wl-copy` FORKS a child that stays serving
        // the selection, and that child inherits the pipes. `wait_with_output()` reads the
        // pipes to EOF — and the EOF only comes when the selection's server dies,
        // that is, never. The buffer's event loop stayed stuck there, and
        // on screen that appeared as "the buffer stopped responding after
        // copying". By inheriting, `wl-copy`'s error message lands in OUR
        // journal, which is where it is any use.
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .with_context(|| format!("could not run {path}"))?;

    child
        .stdin
        .take()
        .context("wl-copy did not open stdin")?
        .write_all(text.as_bytes())
        .context("could not write to wl-copy")?;

    // WAIT FOR THE PARENT, and only it. `wl-copy` forks: the parent exits as soon as the
    // selection is set, and the child stays serving. Without waiting, an error
    // (a compositor with no `wl_data_device`, for example) would come out as success, and
    // we would be back to the defect this fixes.
    //
    // `wait()`, and NOT `wait_with_output()`: see the `stderr` comment.
    let status = child.wait().context("wl-copy did not finish")?;
    if !status.success() {
        anyhow::bail!("wl-copy exited with {status}; the reason is in the log above");
    }
    Ok(())
}

/// Reads what is on the system clipboard.
///
/// [`copy_to_clipboard`]'s counterpart. What reads is the host, and not the component: the buffer does not
/// touch the clipboard as it does not touch a file (invariant 11).
///
/// # Errors
///
/// The binary was not put in the build, did not run, or exited with an error.
fn paste_text() -> Result<String> {
    let path = option_env!("RONOCTUA_WL_PASTE").context(
        "RONOCTUA_WL_PASTE was not set at build time.\n\
         It comes from `nix/rust.nix`; without it there is no way to paste.",
    )?;

    let mut cmd = std::process::Command::new(path);
    // THE SAME PATH as `wl-copy`'s, and for the same reason: under a systemd
    // unit it is minimal, and these tools spawn by NAME.
    if let Some(p) = option_env!("RONOCTUA_WL_COPY_PATH") {
        cmd.env("PATH", p);
    }
    let output = cmd
        // `--no-newline`: `wl-paste` adds a `\n` that was not in
        // what was copied, and it would become one more character in the search.
        .arg("--no-newline")
        .output()
        .with_context(|| format!("could not run {path}"))?;

    // AN EMPTY CLIPBOARD IS NOT AN ERROR. `wl-paste` exits 1 when there is no selection, and
    // treating that as a failure would fill the log with errors every time someone
    // pressed `Ctrl+V` without having copied anything.
    if !output.status.success() {
        // THE REASON IN THE LOG, even though it is a normal case. Without it, "empty clipboard"
        // and "wl-paste failed" become the same line — and it was exactly that
        // ambiguity that made `Ctrl+V` look as if it did not exist.
        tracing::debug!(
            status = %output.status,
            error = %String::from_utf8_lossy(&output.stderr).trim(),
            "wl-paste gave back no text"
        );
        return Ok(String::new());
    }
    // HERE THE OUTPUT IS READ TO THE END, and it can be: `wl-paste` does NOT leave a child
    // serving anything — unlike `wl-copy`, where that froze the loop.
    String::from_utf8(output.stdout).context("what was on the clipboard is not UTF-8 text")
}

/// Opens an `xdg_toplevel` and draws a component in it — section 7.5.
///
/// # Errors
///
/// There is no compositor, the component does not load, or the `render` fails.
pub fn host_it(path: &std::path::Path, file: Option<&std::path::Path>) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, events) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<Host> = events.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;

    // THE INSTANCE LIVES. Loaded once, it keeps the component's
    // state between keys — search, cursor, scroll.
    let wasm = std::fs::read(path).with_context(|| format!("could not read {}", path.display()))?;
    let runtime = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = runtime.precompile(&wasm)?;
    let component = runtime.load(&cwasm)?;
    let mut instance = runtime.open(&component)?;

    // The descriptor BEFORE any render: it is what declares the keymap and
    // the capabilities, and §7.9 wants the refusal at load, not afterwards.
    let d = instance.init()?;
    if let Err(e) = d.validate() {
        anyhow::bail!("the buffer's descriptor does not pass the section 7.9 guard: {e}");
    }
    tracing::info!(name = d.name, mode = d.mode, "component loaded");

    // THE TITLE AND THE `app_id` COME FROM THE DESCRIPTOR.
    //
    // They used to be a fixed `ronoctua: buffer` and `br.ronoctua.buffer`, and the modeline —
    // which reads the title and app id from the compositor — showed `ronoctua: buffer
    // buffer` for any component. It said the same thing twice and did not
    // say WHICH buffer.
    //
    // `*name*` is Emacs's convention for a buffer that is not a file, the same one
    // as the `*scratch*` the modeline already uses in the empty state.
    // THE ID IS ENGLISH AND THE LABEL COMES FROM THE i18n, and the two used to be
    // the same string. That is how `glifos` reached an English screen: the
    // descriptor's `name` WAS the label, drawn raw, so a Portuguese id was a
    // Portuguese window title in every language (invariant 18 and invariant 8 at
    // the same time). Now the buffer publishes `glyphs` and this is what turns
    // it into what the person reads.
    //
    // The `app_id` is NOT translated, and that is deliberate: niri's window
    // rules match on it, and a rule that only fires in one language is a rule
    // that fails on the machine of whoever changed `LANG`.
    let texts = ronoctua_i18n::Texts::from_env();
    let title = texts.t(&format!("buffer-{}", d.name));

    // THE PHRASES, BEFORE THE FIRST FRAME.
    //
    // A key a buffer only SHOWS is resolved when this host draws it, and the
    // buffer never holds what came out. This is the other case: text the buffer
    // holds, parses or writes back — its search field — which as data opened
    // `familia:emoji` in an en-GB session and as a key could not have been read
    // back by whoever drew it. See `ronoctua_protocol::Phrase`.
    //
    // HERE, and not after the `Host` is built, because the render below is what
    // `list.validate` reads: seeding afterwards would check a frame nobody sees
    // and leave the one they do see un-seeded.
    //
    // Once, and never again: the language is read from the environment at
    // start-up and nothing in this system changes it while a session runs
    // (`surface.rs`, `texts`). A buffer that had to be told twice would need to
    // know it had been told once.
    if !d.phrases.is_empty() {
        let said = d
            .phrases
            .iter()
            .map(|k| ronoctua_protocol::Phrase {
                key: k.clone(),
                // THE RESERVED KEY IS ANSWERED WITH THE TAG, not with a
                // translation. `Texts::t` would give back `[-language]` — it
                // is not a message and by construction cannot be one, because
                // Fluent reserves the leading `-` for terms and the generator
                // refuses them as keys. See `Phrases::LANGUAGE`.
                text: if k == ronoctua_design::Phrases::LANGUAGE {
                    texts.language().to_owned()
                } else {
                    texts.t(k)
                },
            })
            .collect();
        instance.event(&ronoctua_protocol::Input::Phrases(said))?;
        tracing::info!(phrases = d.phrases.len(), "phrases handed over");
    }

    let surface_of = compositor.create_surface(&qh);
    // `RequestServer`: decoration belongs to the COMPOSITOR, never to us. Drawing a
    // border or a title bar here would be reimplementing window layout, and
    // invariant 4 forbids it. niri runs with `prefer-no-csd`, so in
    // practice there is no decoration at all — which is what section 3 wants.
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    window.set_title(format!("*{title}*"));
    window.set_app_id(format!("br.ronoctua.{}", d.mode));
    window.commit();

    // The initial columns and rows are a guess; the `configure` brings the actual
    // size and the `render` runs again with it.
    let list = instance.render(80, 24, Host::heights())?;
    tracing::info!(
        commands = list.commands.len(),
        height = list.height(),
        "component rendered"
    );

    // THE CAPABILITY GATE, enforced HERE.
    //
    // The WIT has always promised that "the host refuses whoever uses without having asked",
    // and until now it was only a promise: `validate_it` checked the keymap's prefix and
    // nothing else. The first `render` happens BEFORE the window exists, so
    // a buffer that draws a gated primitive without having asked for it dies at
    // startup — which is where §7.9 wants the refusal.
    if let Err(e) = list.validate(&d.capabilities) {
        anyhow::bail!(
            "the buffer `{}` uses a primitive with no capability: {e}",
            d.name
        );
    }

    // THE SYSTEM HALF EXISTS ONLY FOR WHOEVER ASKED FOR IT.
    //
    // Not a runtime check inside the tick, but the absence of the tick: a
    // buffer without the capability has no clock, no thread and no `systemctl`
    // — the cost of this surface is paid only by the buffer that uses it.
    let system = d
        .capabilities
        .contains(&ronoctua_protocol::Capability::System)
        .then(System::fresh);
    // And the same for the queue: no capability, no thread and no socket.
    let queue = d
        .capabilities
        .contains(&ronoctua_protocol::Capability::Transfers)
        .then(Queue::fresh);
    // And the same for the disks: no capability, no udisks thread and no bus
    // connection. THE HEAVIEST OF THE THREE, and the one whose absence is worth
    // the most: a buffer that never asked has no way to reach a disk at all,
    // rather than a check it might get past.
    let disks = d
        .capabilities
        .contains(&ronoctua_protocol::Capability::Disks)
        .then(Disks::fresh);
    // And the same for the brain: no capability, no thread, no socket and no
    // `systemctl`. The one that reaches furthest of the four — it ends with the
    // machine rebuilding its own operating system — and the buffer that never
    // asked has no way to reach it at all, rather than a check it might get
    // past.
    let brain = d
        .capabilities
        .contains(&ronoctua_protocol::Capability::Brain)
        .then(Brain::fresh);
    // And the same for the keyboards: no capability, no reading of the kernel
    // and no writing of the daemon's file. It reaches further than it looks —
    // what it writes sits between the keyboard and everything else — and a
    // buffer that never asked cannot get near it.
    let keyboards = d
        .capabilities
        .contains(&ronoctua_protocol::Capability::Keyboard)
        .then(Keyboards::fresh);

    let current = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    // THE LOOP FIRST, because the state carries its handle: a keyboard that
    // REPEATS is made with `get_keyboard_with_repeat`, and that wants one.
    let mut cycle: EventLoop<Host> = EventLoop::try_new().context("calloop")?;
    let mut state = Host {
        handle: cycle.handle(),
        system,
        queue,
        disks,
        brain,
        keyboards,
        states: d.states.clone(),
        current: std::sync::Arc::clone(&current),
        escape: Escape::default(),
        keymap: d.keymap.clone(),
        pending: Vec::new(),
        file: file.map(std::path::Path::to_path_buf),
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
        asked: (0, 0),
        // ONE TO ONE UNTIL A SCREEN SAYS OTHERWISE — see `crate::hidpi`.
        scale: 1,
        scale_told: 1,
        quit: false,
        gpu: None,
        text: None,
        images: None,
        quads: None,
        list,
        rasterized: std::collections::HashMap::new(),
        _theme_watch: None,
        _size_watch: None,
        capabilities: d.capabilities,
        // THE SAME BUNDLE that resolved the title and the phrases above. It
        // used to be a second `from_env()` in the same process — two readings
        // of one environment, which is the shape a divergence hides in.
        texts,
        instance,
        _runtime: runtime,
    };

    WaylandSource::new(conn, events)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;

    // THE THEME'S FILE, watched. The component keeps drawing NAMES; what this
    // host must forget is the rasterised SVGs, whose `currentColor` was the
    // old theme's value.
    let (theme_sender, theme_changed) = calloop_channel::channel::<()>();
    state._theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    cycle
        .handle()
        .insert_source(theme_changed, |event, (), state: &mut Host| {
            if let calloop_channel::Event::Msg(()) = event
                && crate::theme::reload()
            {
                // The new theme is a new key (`svg_key`), so the next paint
                // rasterises and uploads afresh; clearing only drops the old
                // theme's CPU copies.
                state.rasterized.clear();
                state.paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the theme channel: {e}"))?;

    // AND THE SIZE OF THE TEXT, on the same footing. A buffer's body is what
    // a person reads; a host that kept the old size would put the content at
    // one size inside a footer at another.
    let (size_sender, size_changed) = calloop_channel::channel::<()>();
    state._size_watch = crate::toolkits::watch(move || size_sender.send(()).is_ok());
    cycle
        .handle()
        .insert_source(size_changed, |event, (), state: &mut Host| {
            if let calloop_channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                if let Some(t) = state.text.as_mut() {
                    t.draw_at(ronoctua_tokens::text::size());
                }
                // THE COMPONENT HAS TO BE ASKED AGAIN — and `paint` does
                // not ask it, it draws the list it already has. `render` is
                // what asks, and `request_redraw` is what calls it.
                //
                // It matters most HERE, of all ten doors: this is the one
                // whose content comes from another process. It is told how
                // many cells it has, and 100% to 200% on a 1280 px window
                // takes that from 118 columns to 59 — so its elisions, its
                // column positions and the cells it reserved by scale are all
                // for a grid that no longer exists. Invariant 11 is what
                // makes it invisible from this side.
                state.request_redraw();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    // THE KEYMAP AND THE STATES GO OUT, AND ORDERS COME BACK IN.
    //
    // The socket thread cannot touch the component: the state lives in the
    // `Instance` and it is not `Send`. So it puts the order on a calloop channel
    // and the LOOP acts on it, on the same thread as every other input — which
    // is the arrangement the footer already uses for its control socket.
    let (orders, channel) = calloop_channel::channel::<FromSocket>();
    let published = publish_keymap(d.keymap, d.states, current, orders);
    if cycle
        .handle()
        .insert_source(channel, |event, (), state| {
            if let calloop_channel::Event::Msg(order) = event {
                match order {
                    FromSocket::Command(id) => {
                        state.deliver(&ronoctua_protocol::Input::Command(id));
                    }
                    FromSocket::Named(id) => match state.index_of(&id) {
                        Some(n) => state.enter_state(n),
                        // The footer only ever sends back an id it was just
                        // told, so this is a bug on the other side rather than
                        // a person's mistake — and guessing at the nearest
                        // state would hide it.
                        None => tracing::warn!(state = %id, "a state this buffer does not declare"),
                    },
                    FromSocket::Next => state.rotate_state(true),
                    FromSocket::Previous => state.rotate_state(false),
                }
            }
        })
        .is_err()
    {
        tracing::warn!("could not hang the keymap's channel");
    }

    // AND THE FOOTER IS TOLD WHERE THIS BUFFER STARTS.
    //
    // Without this the modeline shows the state only from the first change on:
    // a window that opens and is not touched would have a mode and no state,
    // which reads as "this buffer has none" rather than "nobody asked yet".
    state.tell_the_footer();

    // THE CLOCK, and the thread that answers a scan — only for the buffer that
    // declared the capability.
    if state.system.is_some() {
        // The scan's channel FIRST: the tick may start one, and a report with
        // nowhere to arrive would be a scan that runs and never shows.
        let (reports, arrivals) = calloop_channel::channel::<Scanned>();
        if cycle
            .handle()
            .insert_source(arrivals, |event, (), state: &mut Host| {
                if let calloop_channel::Event::Msg(m) = event {
                    state.scanned(m);
                }
            })
            .is_ok()
        {
            if let Some(s) = state.system.as_mut() {
                s.reports = Some(reports);
            }
        } else {
            tracing::warn!("could not hang the scan's channel");
        }

        // ONE SECOND, and the survey it carries is only pseudo-file reads —
        // microseconds. What costs (systemd, the journal, a walk of the disk)
        // is deliberately NOT here: the tick is the cheap half by construction,
        // which is what lets it run inside the loop that also draws.
        let tick = timer::Timer::from_duration(std::time::Duration::from_secs(1));
        if cycle
            .handle()
            .insert_source(tick, |_, (), state: &mut Host| {
                state.sample();
                timer::TimeoutAction::ToDuration(std::time::Duration::from_secs(1))
            })
            .is_err()
        {
            tracing::warn!("could not hang the sampling clock");
        }
        // The first survey does not wait a second: the screen opens with
        // numbers, and not with a blank that fills in later.
        //
        // SAMPLE FIRST, AND THE ORDER IS NOT STYLE: the guard's ceiling comes
        // back from systemd in bytes and is shown as a fraction of RAM, so
        // reading it before anything knows how much RAM there is prints `0.0%`
        // — a wrong number on the first screen, which is the one screen
        // everybody sees.
        state.sample();
        state.refresh_guard();
        state.sample();
    }

    // THE QUEUE'S SUBSCRIPTION, and it is NOT a clock.
    //
    // A socket round trip has no business inside a one-second timer: the unit
    // already knows when something changed, and `client::watch` blocks on the
    // socket until it does. So it is a thread that blocks and a channel that
    // wakes the loop — the same shape the scan uses, and the same one the
    // footer already uses for this very list.
    if state.queue.is_some() {
        let (send, arrivals) = calloop_channel::channel::<Vec<ronoctua_transfers::Job>>();
        if cycle
            .handle()
            .insert_source(arrivals, |event, (), state: &mut Host| {
                if let calloop_channel::Event::Msg(jobs) = event {
                    state.queue_arrived(&jobs);
                }
            })
            .is_ok()
        {
            // `listen` starts the unit if it is not up, so an empty screen here
            // means an empty queue and never a queue nobody asked.
            if !crate::transfers::listen(send) {
                tracing::warn!("no transfer queue; the buffer opens empty");
            }
        } else {
            tracing::warn!("could not hang the queue's channel");
        }
    }

    // THE DISKS' SUBSCRIPTION, and it is not a clock either.
    //
    // udisks says when something moved — a stick in, a partition mounted, a
    // filesystem written. Polling it on a timer would ask a bus twenty times a
    // second for an answer that changes twice an hour, and would still be late
    // the one time it mattered.
    if state.disks.is_some() {
        let (send, readings) = calloop_channel::channel::<ronoctua_disks::Rack>();
        if cycle
            .handle()
            .insert_source(readings, |event, (), state: &mut Host| {
                if let calloop_channel::Event::Msg(rack) = event {
                    state.rack_arrived(rack);
                }
            })
            .is_ok()
        {
            if let Err(e) = ronoctua_disks::watch(move |rack| {
                let _ = send.send(rack);
            }) {
                // NOT FATAL, and said out loud. The screen opens empty and
                // stays empty, which is wrong — but a buffer that refused to
                // start would say even less about why.
                tracing::error!(error = %e, "no udisks2; the buffer opens with no disks");
            }
        } else {
            tracing::warn!("could not hang the disks' channel");
        }

        // AND THE WAY BACK, which is a second channel and not the same one.
        //
        // What comes down this one is the VERDICT on a format: a fresh reading
        // says the disk changed, and says nothing at all when udisks refused.
        // Without it a refusal is a log line, and on screen the buffer would go
        // on showing the question it already asked.
        let (said, verdicts) = calloop_channel::channel::<Verdict>();
        if cycle
            .handle()
            .insert_source(verdicts, |event, (), state: &mut Host| {
                if let calloop_channel::Event::Msg(verdict) = event {
                    state.format_said(&verdict);
                }
            })
            .is_ok()
        {
            if let Some(d) = state.disks.as_mut() {
                d.said = Some(said);
            }
        } else {
            tracing::warn!("could not hang the format verdict's channel");
        }
    }

    // THE BRAIN'S SUBSCRIPTION, and it is not a clock either.
    //
    // The daemon knows when something changed — a moment came, a run ended, a
    // countdown started — and `client::watch` blocks on the socket until it
    // does. A one-second timer asking for the schedule would be a socket round
    // trip inside the loop that draws, for an answer that changes twice a week.
    //
    // It is the SLOW half of that crate's two connects on purpose: it starts
    // the daemon when nobody is home and reconnects when it goes away, which is
    // a sleep and a retry — and a sleep in this loop is a frozen window.
    if state.brain.is_some() {
        let (send, minds) = calloop_channel::channel::<ronoctua_brain::Mind>();
        if cycle
            .handle()
            .insert_source(minds, |event, (), state: &mut Host| {
                if let calloop_channel::Event::Msg(mind) = event {
                    state.mind_arrived(&mind);
                }
            })
            .is_ok()
        {
            if !crate::brain::listen(send) {
                tracing::warn!("no brain; the buffer opens empty");
            }
        } else {
            tracing::warn!("could not hang the brain's channel");
        }
    }

    // THE KEYBOARDS ARRIVE ONCE, AND AGAIN AFTER EVERY CHANGE.
    //
    // No thread and no clock: `/proc/bus/input/devices` is three kilobytes
    // read at the moment somebody opens the window, and a keyboard plugged in
    // while it is open is a case that costs a `udev` socket and a thread to
    // catch. The buffer is re-sent the whole picture after every remap — which
    // is when the numbers on screen actually change — and the header of
    // `keyboards_arrived` says so.
    if state.keyboards.is_some() {
        state.keyboards_arrived();
    }

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |e| {
            if e.quit {
                signal.stop();
            }
        })
        .context("event loop")?;
    // The socket outlives nothing: a stale one in `XDG_RUNTIME_DIR` would make
    // the footer connect to a buffer that is not there and wait.
    if let Some(path) = published {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

/// The work itself, off the loop that draws.
///
/// A free function and not a method because it runs on another thread: every
/// decision it needed has already been made, and it carries the answers rather
/// than the `Host` they came from.
fn run_format(
    device: &str,
    filesystem: &'static str,
    label: &str,
    whole: bool,
    unmount_first: &[String],
    name: &str,
) -> Verdict {
    let refused = |which, why: String| Verdict::Refused {
        which,
        why,
        name: name.to_owned(),
    };

    let manager = match ronoctua_disks::Manager::fresh() {
        Ok(m) => m,
        Err(e) => return refused(ronoctua_disks::Refusal::Other, e.to_string()),
    };

    // THE FIRST REFUSAL STOPS THE REST. Carrying on after one partition
    // refused to come down would format a disk with a filesystem still mounted
    // from it — which is the write-half-done this repository already refuses
    // once, in `Doing::Eject`.
    for at in unmount_first {
        if let Err(e) = manager.unmount(at) {
            return refused(ronoctua_disks::refusal(&e), e.to_string());
        }
    }

    match manager.format(device, filesystem, label, whole) {
        Ok(()) => {
            tracing::info!(device, filesystem, whole, "formatted");
            Verdict::Done
        }
        Err(e) => {
            tracing::error!(device, error = %e, "formatting FAILED");
            refused(ronoctua_disks::refusal(&e), e.to_string())
        }
    }
}

/// PUBLISHES THIS BUFFER'S KEYMAP, and takes commands back.
///
/// # Why this exists at all
///
/// The buffer's host resolves its own keymap, and it has to: a buffer is a
/// window of its own (invariant 4), so while it has the focus it is THIS process
/// that holds the keyboard. The footer only knows the windows the compositor
/// tells it about — id, title, `app_id` — and none of that is a key binding.
///
/// So `SPC m` in the footer had nothing to list. This socket is the answer, and
/// it carries BOTH directions on purpose: a list of keys with no way to run them
/// would teach shortcuts that do nothing, which is worse than not listing.
///
/// # The protocol, which is the footer's own
///
/// `keymap` answers one line per binding, `<sequence> <id>`. `command <id>` puts
/// the id on the channel and answers `ok`.
///
/// `states` answers the CURRENT id on the first line and then one line per
/// declared state, `<id> <letter>`. `state <id>`, `state-next` and `state-prev`
/// put the change on the same channel and answer `ok`. They are here and not on
/// a socket of their own for the reason the file's header already gives: a
/// second grammar in the same directory is two things to keep in step.
///
/// Anything else answers `?`.
///
/// # It never brings the buffer down
///
/// A buffer that cannot open its socket is a buffer whose keymap the footer
/// cannot list — it is not a buffer that fails to open. The editor still edits.
/// What the socket asks the buffer's loop to do.
///
/// It used to be a bare `String` — a command id — and widening it is what let
/// the state verbs share the one socket that already exists instead of opening
/// a second one next to it.
enum FromSocket {
    /// Run one of the ids this buffer declared.
    Command(String),
    /// Go to this state, by the id this buffer declared.
    Named(String),
    /// The next state, wrapping round.
    Next,
    /// The previous one.
    Previous,
}

fn publish_keymap(
    keymap: Vec<ronoctua_protocol::Binding>,
    states: Vec<ronoctua_protocol::BufferState>,
    current: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    orders: calloop_channel::Sender<FromSocket>,
) -> Option<std::path::PathBuf> {
    use std::io::{BufRead as _, Write as _};

    let path = match crate::leader::buffer_socket_path(std::process::id()) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "no buffer socket; SPC m will not list it");
            return None;
        }
    };
    // An orphan socket from a dead process of the same PID would block the bind.
    let _ = std::fs::remove_file(&path);
    let listener = match std::os::unix::net::UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "could not open the buffer's socket");
            return None;
        }
    };
    tracing::info!(path = %path.display(), bindings = keymap.len(), "keymap published");

    let answer = keymap
        .iter()
        .map(|b| format!("{} {}", b.keys, b.command))
        .collect::<Vec<_>>()
        .join("\n");
    let known: std::collections::HashSet<String> = keymap.into_iter().map(|b| b.command).collect();
    // The LIST does not change after `init`, so the thread owns a copy of it and
    // reads only the index from the shared cell. Sharing the list too would be a
    // lock the frame can wait on, for something that is immutable.
    let listed = states
        .iter()
        .map(|s| format!("{} {}", s.id, s.letter))
        .collect::<Vec<_>>()
        .join("\n");
    let ids: Vec<String> = states.iter().map(|s| s.id.clone()).collect();

    let built = std::thread::Builder::new()
        .name("keymap".into())
        .spawn(move || {
            for connection in listener.incoming() {
                let Ok(mut stream) = connection else { continue };
                let mut line = String::new();
                if std::io::BufReader::new(match stream.try_clone() {
                    Ok(c) => c,
                    Err(_) => continue,
                })
                .read_line(&mut line)
                .is_err()
                {
                    continue;
                }
                let reply = match line.trim().split_once(' ') {
                    None if line.trim() == "keymap" => answer.clone(),
                    // THE CURRENT STATE FIRST, then the list. The footer asks
                    // this on every change of focus, because a buffer that
                    // changed state while it was NOT focused has nothing to push
                    // — and a footer that restarted has nothing at all.
                    None if line.trim() == "states" => {
                        let n = current.load(std::sync::atomic::Ordering::Relaxed);
                        let now = ids.get(n).cloned().unwrap_or_default();
                        if listed.is_empty() {
                            now
                        } else {
                            format!("{now}\n{listed}")
                        }
                    }
                    None if line.trim() == "state-next" => {
                        if orders.send(FromSocket::Next).is_err() {
                            return;
                        }
                        "ok".into()
                    }
                    None if line.trim() == "state-prev" => {
                        if orders.send(FromSocket::Previous).is_err() {
                            return;
                        }
                        "ok".into()
                    }
                    // ONLY A STATE THIS BUFFER DECLARED, for the same reason as
                    // `command` below: the socket is reachable by anything
                    // running as this user, and an id from outside would be a
                    // way to drive the component that the feature does not need.
                    Some(("state", id)) if ids.iter().any(|k| k == id) => {
                        if orders.send(FromSocket::Named(id.to_owned())).is_err() {
                            return;
                        }
                        "ok".into()
                    }
                    // ONLY A COMMAND THIS BUFFER DECLARED.
                    //
                    // The socket is in `XDG_RUNTIME_DIR` and so is reachable by
                    // anything running as this user. Forwarding an arbitrary id
                    // would make it a way to drive the component from outside,
                    // which is a wider door than the feature needs: the footer
                    // only ever sends back what it was just told.
                    Some(("command", id)) if known.contains(id) => {
                        if orders.send(FromSocket::Command(id.to_owned())).is_err() {
                            // The loop is gone; so is the reason to listen.
                            return;
                        }
                        "ok".into()
                    }
                    _ => {
                        tracing::warn!(
                            order = line.trim(),
                            "an unknown order on the buffer's socket"
                        );
                        "?".into()
                    }
                };
                let _ = writeln!(stream, "{reply}");
            }
        });
    if let Err(e) = built {
        tracing::warn!(error = %e, "could not start the keymap thread");
        let _ = std::fs::remove_file(&path);
        return None;
    }
    Some(path)
}

impl Host {
    /// Feeds a character to the buffer's local keymap.
    ///
    /// Returns `Some(Input::Command)` when a sequence COMPLETES, and `None`
    /// when the character does not finish one. A partial sequence is swallowed.
    ///
    /// It is the CALLER that decides when a sequence may start: the first key
    /// comes with `Ctrl`, and only then do the plain letters that follow reach
    /// here. Letting any letter open a sequence is what made the editor eat
    /// every `m`.
    fn local_key(&mut self, c: char) -> Option<ronoctua_protocol::Input> {
        let mut pending = std::mem::take(&mut self.pending);
        let hit = advance_sequence(&self.keymap, &mut pending, c);
        self.pending = pending;
        if let Some(command) = hit {
            tracing::info!(command, "the buffer's keymap");
            return Some(ronoctua_protocol::Input::Command(command));
        }
        None
    }

    /// Rasterises the list's vector drawings and says where each one fits.
    ///
    /// # Why here, and not on every frame
    ///
    /// Because `paint` only runs when the component asks (`Effect::Redraw`),
    /// that is **per key**, and the cache keyed by the SVG's string absorbs the keys
    /// that do not change the drawing. One rasterisation per frame would put resvg in the
    /// frame budget just to redraw the same icon.
    ///
    /// What does **not** rasterise is left out, and the area is simply not
    /// drawn: a half-typed colour (`#8ab`) makes the preview disappear
    /// while one types, which is true information. Inventing a drawing
    /// in its place would be worse.
    fn vector_ones(&mut self) -> Vec<VectorDrawing> {
        use ronoctua_protocol::Command;
        // THE SAME cell as the click's and the text's — see [`crate::paint::cell`].
        // A second calculation here would make the drawing land in one place and the click find it
        // in another, with the error growing with the distance from the origin.
        let (cell_px, line_height) = cell();
        let mut out = Vec::new();
        for c in &self.list.commands {
            let Command::Svg {
                line,
                column,
                lines,
                columns,
                svg,
                color,
            } = c
            else {
                continue;
            };
            let Some(token) = token_color(color.as_str()) else {
                tracing::error!(token = color.as_str(), "an invalid colour in an svg");
                continue;
            };
            // THE SIDE IS COMPUTED FIRST, because it is part of the key now —
            // see `svg_key`.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let side = (f32::from(*columns) * cell_px)
                .min(f32::from(*lines) * line_height)
                .max(1.0) as u32;
            let key = svg_key(side, tokens::theme::current().position(), token, svg);
            if !self.rasterized.contains_key(&key) {
                // THE SIDE IN PIXELS comes from the rectangle of CELLS the buffer
                // asked for — it does not know pixels.
                match rasterize_preview(svg, token, side) {
                    Some(r) => {
                        self.rasterized.insert(key.clone(), r);
                    }
                    None => {
                        tracing::debug!(bytes = svg.len(), "the buffer's svg did not rasterise");
                        continue;
                    }
                }
            }
            let Some(r) = self.rasterized.get(&key) else {
                continue;
            };
            // THE MARGIN COMES IN HERE, as it does in `with_margin` for the text: the
            // component counts from zero and does not know where its window
            // is (invariant 11). Without it the drawing would come out shifted from the
            // text beside it, and only diagonally — the hardest kind to detect.
            let x = f32::from(column.saturating_add(tokens::space::CONTENT_INSET)) * cell_px;
            let y = f32::from(line.saturating_add(tokens::space::CONTENT_INSETY)) * line_height;
            out.push(VectorDrawing {
                area: ronoctua_render::image::Area {
                    x,
                    y,
                    width: f32::from(*columns) * cell_px,
                    height: f32::from(*lines) * line_height,
                }
                // CENTRE, and do not fit: `fit_into` would grow to the edge and would
                // undo the rasterisation at the natural size, which is what makes
                // the size adjustment show.
                .centre(r.width, r.height),
                key,
            });
        }
        out
    }

    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let (columns, _) = self.context_of();
        let spans = spans_of(&self.list, &self.texts, columns);
        let vector_ones = self.vector_ones();
        let selections = self.selections();
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
        if let Some(im) = self.images.as_mut() {
            let batch: Vec<(String, ronoctua_render::image::Area)> = vector_ones
                .into_iter()
                .map(|v| {
                    if im.missing(&v.key)
                        && let Some(r) = self.rasterized.get(&v.key)
                    {
                        im.upload_it(&gpu.device, &gpu.queue, &v.key, r);
                    }
                    (v.key, v.area)
                })
                .collect();
            im.prepare(&gpu.queue, logical_w, logical_h, &batch);
        }
        if let Some(q) = self.quads.as_mut() {
            q.prepare(&gpu.device, &gpu.queue, logical_w, logical_h, &selections);
        }
        let images = self.images.as_ref();
        let quads = self.quads.as_ref();
        if let Err(e) = gpu.frame(
            || crate::hidpi::told_before_present(scale, told, &surface),
            |pass| {
                // THE SELECTION FIRST OF ALL: it is a background, and anything drawn
                // after it stays on top. Drawing it last would hide the very text it
                // exists to highlight.
                if let Some(q) = quads {
                    q.draw(pass);
                }
                // THE DRAWING BELOW THE TEXT, as in the footer: what is written
                // over it has to stay legible.
                if let Some(im) = images {
                    im.draw(pass);
                }
                text.draw(ronoctua_render::Atlas::Pinned, pass)
            },
        ) {
            tracing::error!(error = %e, "drawing the frame failed");
            return;
        }
        tracing::info!(spans = spans.len(), "buffer drawn on the xdg_toplevel");
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "the flush failed");
        }
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
        match gpu {
            Ok(mut g) => {
                // AND THE FIRST BUFFER IS DIVISIBLE TOO. `Gpu::new` clamps at
                // the GPU's ceiling like `resize` does, and the first frame
                // sends the scale — see `crate::hidpi::resized`.
                let (shown_w, shown_h) = crate::hidpi::resized(&mut g, self.asked, self.scale);
                self.width = shown_w;
                self.height = shown_h;
                // AND THE COMPONENT IS ASKED AGAIN IF THE FIRST BUFFER CAME
                // BACK SMALLER. `configure` rendered against the size the
                // compositor gave, before there was a GPU to clamp anything —
                // see `scale_factor_changed` for the whole of why a repaint is
                // not enough here.
                if (shown_w, shown_h) != self.asked {
                    self.re_render();
                }
                match ronoctua_render::Text::new(
                    &g.device,
                    &g.queue,
                    g.config.format,
                    self.scale,
                    // THE SYSTEM'S ONE SIZE. This used to be `SIZE_LARGE`, on the
                    // argument that "the footer is peripheral reference, the
                    // buffer is what one reads". The owner asked for one measure
                    // instead, from a photograph of the file manager's two doors:
                    // see `font.size.base` and `ronoctua_tokens::text::size`.
                    tokens::text::size(),
                ) {
                    Ok(t) => {
                        // THE IMAGE PIPELINE is born along with the text one, and its
                        // failure does NOT take the buffer down: without it what is lost
                        // is the vector preview, and the text goes on drawing.
                        match ronoctua_render::image::Images::new(&g.device, g.config.format) {
                            Ok(im) => self.images = Some(im),
                            Err(e) => tracing::error!(error = %e, "the image pipeline failed"),
                        }
                        // THE QUAD PIPELINE, for `Command::Selection`. Same rule as
                        // the image one: its failure costs the highlight, not the
                        // buffer.
                        match ronoctua_render::quad::Quads::new(&g.device, g.config.format) {
                            Ok(q) => self.quads = Some(q),
                            Err(e) => tracing::error!(error = %e, "the quad pipeline failed"),
                        }
                        self.text = Some(t);
                        self.gpu = Some(g);
                        tracing::info!("the buffer's graphics context is ready");
                    }
                    Err(e) => tracing::error!(error = %e, "the renderer failed"),
                }
            }
            Err(e) => tracing::error!(error = %e, "no graphics context"),
        }
    }
}

impl Host {
    /// The cell, in pixels: width and height.
    ///
    /// ONE place only, because it is used by two calculations that **have** to agree:
    /// how many cells fit in the window ([`Self::context_of`]) and which cell
    /// a pixel landed in ([`Self::cell_at`]). Diverging, the click hits one
    /// glyph and the drawing shows another — and the error grows with the distance from the
    /// origin, which is the worst kind of error to reproduce.
    /// The focused rows, as rectangles.
    ///
    /// It uses the SAME cell as the text and the click ([`Self::cell`]): a second
    /// calculation would paint the row in one place while the click found it in
    /// another, with the error growing away from the origin.
    /// The focused rows' bands, for THIS window's width.
    ///
    /// The window is what knows how wide it is; everything else about the band
    /// is in `paint`, next to the conversion that draws the text over it.
    fn selections(&self) -> Vec<ronoctua_render::quad::Quad> {
        quads_of(&self.list, self.width as f32)
    }

    /// Which cell OF THE BUFFER a point in the window landed in.
    ///
    /// `None` when the point is in the margin: there is no content there, and returning
    /// cell 0 would make clicking the border select the first item.
    ///
    /// The origin is the **buffer's** (0,0) cell, and not the window's corner — it is the
    /// same origin `spans_of` uses when drawing, and that is why the margin
    /// comes in here as a subtraction.
    fn cell_at(x: f64, y: f64) -> Option<(u16, u16)> {
        let (width, height) = cell();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let column = (x / f64::from(width)) as u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = (y / f64::from(height)) as u32;
        Some((
            u16::try_from(column.checked_sub(u32::from(tokens::space::CONTENT_INSET))?).ok()?,
            u16::try_from(line.checked_sub(u32::from(tokens::space::CONTENT_INSETY))?).ok()?,
        ))
    }

    /// How many columns and rows fit in the window right now.
    ///
    /// Integer arithmetic over the cell, as in the footer: measuring text
    /// here would trade a calculation for a shaping on every `configure`.
    fn context_of(&self) -> (u16, u16) {
        // THE GRID IS ALWAYS THE NORMAL SIZE'S. A `large` run occupies
        // more than one cell, and what spaces it is whatever draws — that is how
        // the browser's grid came to move two rows at a time.
        let (cell_width_px, cell_height_px) = cell();
        // It subtracts the margins on BOTH sides: the content fits in what is left,
        // and not in what the window measures.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let columns = ((self.width as f32 / cell_width_px) as u32)
            .saturating_sub(u32::from(tokens::space::CONTENT_INSET) * 2)
            .clamp(1, u32::from(u16::MAX)) as u16;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let lines = ((self.height as f32 / cell_height_px) as u32)
            .saturating_sub(u32::from(tokens::space::CONTENT_INSETY) * 2)
            .clamp(1, u32::from(u16::MAX)) as u16;
        (columns, lines)
    }

    /// Each scale's heights, in cells.
    ///
    /// Computed from the SAME tokens `scale_in_px` uses: if the two calculations
    /// diverge, the buffer reserves one number of cells and the run occupies
    /// another — which was exactly the clipped-glyph defect.
    fn heights() -> ronoctua_protocol::Heights {
        crate::paint::heights()
    }

    /// Asks the component for a new frame and redraws.
    fn request_redraw(&mut self) {
        self.re_render();
        self.paint();
    }

    /// Asks the component for a new frame and keeps it, without drawing.
    ///
    /// SPLIT OUT FOR `assemble_gpu`, which runs INSIDE `paint`: asking there
    /// and then painting would paint twice on the first frame, and the first
    /// frame is what the 200 ms budget is measured over.
    fn re_render(&mut self) {
        let (columns, lines) = self.context_of();
        match self.instance.render(columns, lines, Self::heights()) {
            // THE GATE HOLDS ON EVERY FRAME, and not only the first.
            //
            // A component can emit the gated primitive only after
            // a key — the sheet's preview is exactly that case: it does not
            // exist in the grid. Checking once would be checking the frame in which
            // it does not appear. The whole list is discarded, and not only the
            // command: drawing half of what the buffer asked for is worse than not
            // drawing, because it looks as if it works.
            Ok(l) => match l.validate(&self.capabilities) {
                Ok(()) => self.list = l,
                Err(e) => {
                    tracing::error!(error = %e, "frame refused: a primitive with no capability");
                }
            },
            Err(e) => {
                tracing::error!(error = %e, "the component's render failed");
            }
        }
    }

    /// Delivers a key to the component and runs what it ASKS for.
    /// The index of a declared state, by id.
    fn index_of(&self, id: &str) -> Option<usize> {
        self.states.iter().position(|s| s.id == id)
    }

    /// The state in effect right now.
    fn now(&self) -> usize {
        self.current.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Asks the component to go to state `n`.
    ///
    /// # It does NOT record the state here, and that is the whole point
    ///
    /// The first version stored the index and told the footer BEFORE delivering,
    /// which reads as the natural order and is wrong: **a component is allowed
    /// to refuse**. `sheet` with nothing under the cursor is a screen of empty
    /// fields, and the icon browsers say no to it — so the host would have
    /// announced a state the screen was never in, and the modeline would have
    /// gone on naming it until the next change.
    ///
    /// What records the state is `effect::state-changed`, arriving back through
    /// [`Self::deliver`]. So the host's idea of where the buffer is can only
    /// ever be something the buffer confirmed — one source of truth, and it is
    /// the one that draws.
    ///
    /// Idempotent, for the same reason it always was: `Esc` at the base state
    /// arrives here and has to cost nothing.
    fn enter_state(&mut self, n: usize) {
        let Some(going) = self.states.get(n) else {
            return;
        };
        if n == self.now() {
            return;
        }
        self.deliver(&ronoctua_protocol::Input::EnterState(going.id.clone()));
    }

    /// The next state, or the previous one, wrapping round.
    ///
    /// WRAPPING and not stopping at the end: with two states — which is what
    /// most buffers have — a rotation that stopped would make one of the two
    /// arrows do nothing half the time, and the person would learn to distrust
    /// both.
    fn rotate_state(&mut self, forward: bool) {
        let total = self.states.len();
        if total < 2 {
            return;
        }
        let now = self.now();
        let going = if forward {
            (now + 1) % total
        } else {
            (now + total - 1) % total
        };
        self.enter_state(going);
    }

    /// Tells the footer which state this window is in.
    ///
    /// # Why it pushes, when the footer can also ask
    ///
    /// Because the footer only asks when the FOCUS changes, and most state
    /// changes happen without one — pressing a key in a window that is already
    /// focused. Asking per frame instead would put a socket round trip inside
    /// the frame budget for something that changes when a person presses a key.
    ///
    /// Failure is silent, and it has to be: there is no footer while the buffer
    /// runs under `ronoctua-shell buffer` on its own, which is how every VM test
    /// and the whole of `nix run .#dev`'s single-buffer mode work. A window that
    /// refused to run without a modeline would be a buffer that needs chrome.
    fn tell_the_footer(&self) {
        let Some(id) = self.states.get(self.now()).map(|s| s.id.as_str()) else {
            return;
        };
        let order = format!("state {} {id}", std::process::id());
        if let Err(e) = crate::leader::send_it(&order) {
            tracing::debug!(error = %e, "no footer to tell the state to");
        }
    }

    fn deliver(&mut self, input_of: &ronoctua_protocol::Input) {
        let effects = match self.instance.event(input_of) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(error = %e, "the component's event failed");
                return;
            }
        };
        // INPUT AND EFFECTS IN THE LOG, always.
        //
        // Without this, "the key did nothing" does not tell "it did not arrive" from
        // "it arrived and the component returned an empty list" — and the two are
        // fixed in opposite places. It was that doubt that cost a whole
        // round of VM testing.
        tracing::debug!(input = ?input_of, effects = effects.len(), "input delivered");
        let mut verdict = None;
        for effect in effects {
            match effect {
                ronoctua_protocol::Effect::Redraw => self.request_redraw(),
                // The component does NOT touch the clipboard: it asks,
                // and the host does it (invariant 11).
                ronoctua_protocol::Effect::Copy(t) => {
                    let ok = match copy_to_clipboard(&t) {
                        Ok(()) => {
                            tracing::info!(bytes = t.len(), "copied");
                            true
                        }
                        // LOUD, and not a `debug`. This here WAS a
                        // `tracing::info!` that copied nothing, and nobody
                        // noticed because copying silently and not copying
                        // silently are indistinguishable.
                        Err(e) => {
                            tracing::error!(error = %e, "copying FAILED");
                            false
                        }
                    };
                    // THE VERDICT GOES BACK TO THE COMPONENT, so it can
                    // say on screen what happened. What knows whether it worked is
                    // this side; without the way back, the buffer could only show
                    // an optimistic "copied!" — which lies precisely when it
                    // matters. Outside the loop so as not to re-enter `event` in the
                    // middle of the effect list.
                    verdict = Some(ok);
                }
                ronoctua_protocol::Effect::Command(id) => {
                    tracing::info!(command = %id, "command asked for by the buffer");
                }
                // WHAT ACTS ON THE MACHINE, and the gate in front of it.
                //
                // The check is here and not inside `ask` so that the refusal is
                // one line for every request there will ever be — and so that
                // it says WHICH capability was missing, instead of the request
                // quietly doing nothing.
                ronoctua_protocol::Effect::Ask(demand) => {
                    if let Some(needed) = ronoctua_protocol::Effect::Ask(demand).requires()
                        && !self.capabilities.contains(&needed)
                    {
                        tracing::error!(
                            request = ?demand.what,
                            capability = ?needed,
                            "request refused: the buffer did not declare the capability"
                        );
                        continue;
                    }
                    self.ask(demand);
                }
                // THE HEAVIEST EFFECT THERE IS, and the gate reads like the
                // one above it for a reason: the capability is checked here,
                // once, and says WHICH one was missing. What it does NOT check
                // here is the target — that is `format`'s job, because the
                // answer needs the rack and the rack is not in this scope.
                ronoctua_protocol::Effect::Format(plan) => {
                    if !self
                        .capabilities
                        .contains(&ronoctua_protocol::Capability::Disks)
                    {
                        tracing::error!(
                            capability = ?ronoctua_protocol::Capability::Disks,
                            "format refused: the buffer did not declare the capability"
                        );
                        continue;
                    }
                    self.format(&plan);
                }
                // A KEY ON ONE KEYBOARD, and the gate is the same shape for the
                // same reason — what this writes is read by a daemon that sits
                // between the keyboard and everything else. The capability is
                // checked here, once; whether the three strings NAME anything
                // is `remap`'s job, because that answer needs the lists that
                // were sent and they are not in this scope.
                ronoctua_protocol::Effect::Remap(what) => {
                    if !self
                        .capabilities
                        .contains(&ronoctua_protocol::Capability::Keyboard)
                    {
                        tracing::error!(
                            capability = ?ronoctua_protocol::Capability::Keyboard,
                            "remap refused: the buffer did not declare the capability"
                        );
                        continue;
                    }
                    self.remap(&what);
                }
                // A PREFERENCE, and the gate in front of it reads like the two
                // above for the same reason: what is being remembered here is
                // when the machine rebuilds its own operating system, so the
                // capability is checked once, here, and the refusal says WHICH
                // one was missing.
                //
                // The buffer touches no disk (invariant 11): it says what it
                // wants remembered, the host turns it into an `Ask::Tune`, and
                // the DAEMON writes the file — the same arrangement `Save` and
                // `Copy` already have.
                ronoctua_protocol::Effect::Tune(setting) => {
                    if !self
                        .capabilities
                        .contains(&ronoctua_protocol::Capability::Brain)
                    {
                        tracing::error!(
                            capability = ?ronoctua_protocol::Capability::Brain,
                            "preference refused: the buffer did not declare the capability"
                        );
                        continue;
                    }
                    self.tune(&setting);
                }
                ronoctua_protocol::Effect::Close => self.quit = true,
                // THE COMPONENT SAYS WHERE IT IS, and this is the ONLY place the
                // host writes that down — whether the change came from a key
                // pressed here or from the person typing a letter over there.
                // Nobody presses a key meaning "open the sheet"; they press
                // `Enter` on a row, and without this the host would never know.
                //
                // `enter_state` deliberately records nothing: a component may
                // REFUSE — `sheet` with nothing under the cursor does — and a
                // host that had already announced the state would leave the
                // modeline naming a screen that never came up.
                //
                // It does not call `enter_state` back, either: that would send
                // the id to the component that just announced it, and a buffer
                // whose `EnterState` sets the screen it is already on would
                // answer with another `StateChanged`. It would terminate, and it
                // would spend a round trip through wasm per keystroke.
                ronoctua_protocol::Effect::StateChanged(id) => match self.index_of(&id) {
                    Some(n) => {
                        if n != self.now() {
                            self.current.store(n, std::sync::atomic::Ordering::Relaxed);
                            tracing::info!(state = %id, "the buffer's state");
                            self.tell_the_footer();
                        }
                    }
                    // An id outside the descriptor would put a word on the
                    // modeline that no key can return from — refused, and said
                    // out loud, because it is a defect in the component.
                    None => tracing::error!(
                        state = %id,
                        "the component reported a state it did not declare"
                    ),
                },
                // THE BUFFER DOES NOT TOUCH THE DISK: it asks, and the host
                // writes (invariant 11) — the same arrangement as `Copy`.
                ronoctua_protocol::Effect::Save(text) => {
                    let ok = match &self.file {
                        Some(path) => match std::fs::write(path, &text) {
                            Ok(()) => {
                                tracing::info!(bytes = text.len(), path = %path.display(), "saved");
                                true
                            }
                            // LOUD, for the same reason as `Copy`: saving in
                            // silence and failing in silence look identical, and
                            // the one that loses work is the second.
                            Err(e) => {
                                tracing::error!(error = %e, path = %path.display(), "saving FAILED");
                                false
                            }
                        },
                        None => {
                            tracing::error!(
                                "the buffer asked to save and was not opened with a file"
                            );
                            false
                        }
                    };
                    // The verdict goes back, so the buffer can say on screen
                    // what happened instead of showing an optimistic "saved!".
                    verdict = Some(ok);
                }
            }
        }
        if let Some(ok) = verdict {
            self.deliver(&ronoctua_protocol::Input::Copied(ok));
        }
    }
}

impl System {
    /// The system half, before anything has been read.
    fn fresh() -> Self {
        Self {
            places: ronoctua_memory::Places::default(),
            guard: ronoctua_protocol::Guard::default(),
            consumers: Vec::new(),
            scanning: false,
            progress: 0,
            last_scan: None,
            cancel: None,
            reports: None,
            ram_total: 0,
        }
    }
}

/// The system half of the host: the clock, the scans and the policy.
impl Host {
    /// Reads the cheap numbers and hands them to the component.
    ///
    /// Everything here is a pseudo-file: it costs microseconds and never
    /// touches the disk, which is why a clock may call it. The policy and the
    /// consumers are NOT read here — they are what the last answer left behind.
    fn sample(&mut self) {
        let Some(s) = self.system.as_mut() else {
            return;
        };
        let mut survey = ronoctua_memory::survey(&s.places);
        // The machine's RAM, kept for whoever turns systemd's bytes back into a
        // percentage. Read from the survey so there is one reading, not two.
        if let Some(total) = survey
            .readings
            .iter()
            .find(|r| r.of == ronoctua_protocol::Gauge::RamTotal)
            .map(|r| r.bytes)
        {
            s.ram_total = total;
        }
        survey.guard = s.guard;
        survey.consumers.clone_from(&s.consumers);
        survey.scanning = s.scanning;
        survey.progress = s.progress;
        self.deliver(&ronoctua_protocol::Input::Survey(survey));
    }

    /// Asks systemd what is in force. Spawns processes — never on the clock.
    fn refresh_guard(&mut self) {
        let Some(s) = self.system.as_mut() else {
            return;
        };
        s.guard = ronoctua_memory::guard::state(s.ram_total);
    }

    /// A scan's progress or its result.
    fn scanned(&mut self, m: Scanned) {
        {
            let Some(s) = self.system.as_mut() else {
                return;
            };
            match m {
                Scanned::Step(p) => s.progress = p,
                Scanned::Done(list) => {
                    s.consumers = list;
                    s.scanning = false;
                    s.progress = 1000;
                    s.cancel = None;
                }
            }
        }
        // The screen only moves when the survey goes back in — the component
        // holds no state the host can write to.
        self.sample();
    }

    /// Does what the buffer asked.
    ///
    /// Every arm that changes the machine reports its verdict the same way a
    /// copy or a save does: through the log, and through the next survey, which
    /// carries the state as systemd reports it — never as we requested it.
    fn ask(&mut self, demand: ronoctua_protocol::Demand) {
        use ronoctua_protocol::Request as R;
        match demand.what {
            R::Refresh => {
                self.refresh_guard();
                self.sample();
            }
            R::ScanRam => self.start_scan(Scan::Ram),
            R::ScanDisk => self.start_scan(Scan::Disk),
            R::CancelScan => self.cancel_scan(),
            R::GuardTolerant => self.switch_guard(ronoctua_protocol::GuardMode::Tolerant),
            R::GuardDefault => self.switch_guard(ronoctua_protocol::GuardMode::Strict),
            R::GuardPersist => {
                let mode = self
                    .system
                    .as_ref()
                    .map_or(ronoctua_protocol::GuardMode::Unknown, |s| s.guard.mode);
                match ronoctua_memory::guard::persist(mode) {
                    Ok(()) => tracing::info!(?mode, "profile written into host.nix"),
                    Err(e) => tracing::error!(error = %e, "saving the profile FAILED"),
                }
                self.refresh_guard();
                self.sample();
            }
            R::StopConsumer => self.stop_consumer(demand.target),
            R::PauseJob | R::ResumeJob | R::CancelJob | R::ForgetJob => {
                self.job_demand(demand.what, demand.target);
            }
            R::SweepJobs => Self::tell_queue(&ronoctua_transfers::Ask::Sweep),
            R::SweepStore | R::SweepGenerations | R::SweepJournal => {
                match ronoctua_memory::scan::sweep(demand.what) {
                    Ok(()) => tracing::info!(what = ?demand.what, "the sweep is done"),
                    Err(e) => tracing::error!(error = %e, what = ?demand.what, "the sweep FAILED"),
                }
                // The freed space only shows up on the next scan; the readings
                // that come from `statvfs` update on their own.
                self.sample();
            }
            // THE BRAIN'S FIVE. Three name a task and two do not, and the
            // difference is in the target: `Proceed` and `Postpone` act on
            // whatever the daemon has ANNOUNCED, which the daemon knows and the
            // screen does not have to name.
            R::RunTask | R::RehearseTask | R::AbortTask => {
                self.task_demand(demand.what, demand.target);
            }
            R::Proceed => Self::tell_brain(&ronoctua_brain::Ask::Proceed),
            R::Postpone => Self::tell_brain(&ronoctua_brain::Ask::Postpone),
        }
    }

    /// Passes one of the buffer's task demands on to the brain, once the index
    /// checks out.
    ///
    /// # Why the index is resolved against the LAST list sent
    ///
    /// Because that is the only list the component could have been pointing at.
    /// `Demand::target` is a 1-based index and never a name — the same
    /// discipline `stop_consumer` and `job_demand` follow — and an index
    /// checked against a fresh reading would act on whatever moved into that
    /// row between the drawing and the key.
    fn task_demand(&mut self, what: ronoctua_protocol::Request, target: u64) {
        use ronoctua_protocol::Request as R;
        let Some(b) = self.brain.as_ref() else {
            return;
        };
        // 1-BASED, and `0` is "none" — the same convention the survey's
        // consumers use. `checked_sub` and not `- 1`, because a zero here is an
        // ordinary thing for a buffer to send and must not be an underflow.
        let Some(task) = target
            .checked_sub(1)
            .and_then(|n| usize::try_from(n).ok())
            .and_then(|n| b.tasks.get(n))
            .cloned()
        else {
            // NOT an error the person can act on: the list changed between the
            // draw and the key, which is ordinary. The next mind already shows
            // it.
            tracing::warn!(target, "a request for a task that is not in the list");
            return;
        };
        let ask = match what {
            R::RunTask => ronoctua_brain::Ask::Run {
                task,
                rehearsal: false,
            },
            R::RehearseTask => ronoctua_brain::Ask::Run {
                task,
                rehearsal: true,
            },
            R::AbortTask => ronoctua_brain::Ask::Abort { task },
            // Unreachable by construction: `ask` only routes those three here.
            _ => return,
        };
        Self::tell_brain(&ask);
    }

    /// One preference the person changed, applied on top of what the daemon
    /// last said and sent back whole.
    ///
    /// # Why the whole record goes back
    ///
    /// Because that is what `Ask::Tune` carries, and the reason it does is on
    /// the other side: one shape to validate instead of one message per
    /// preference. What this function must not do is BUILD one — the base is
    /// the brain's own last answer, so a field this screen never touched comes
    /// back exactly as it was.
    fn tune(&mut self, setting: &ronoctua_protocol::Setting) {
        use ronoctua_protocol::Knob;
        let Some(b) = self.brain.as_mut() else {
            return;
        };
        // THE TASK IS CHECKED against the list actually sent, exactly as an
        // index is: a preference written for an id nothing answers to would sit
        // in the person's file for ever, switching nothing off.
        if !b.tasks.contains(&setting.task) {
            tracing::warn!(task = %setting.task, "a preference for a task that is not in the list");
            return;
        }
        let mut prefs = b.prefs.clone();
        {
            let mine = prefs.mine(&setting.task);
            match &setting.knob {
                Knob::IntervalDays(days) => mine.interval_days = *days,
                Knob::PartOfDay(part) => mine.part_of_day = crate::brain::part_back(*part),
                Knob::Enabled(on) => mine.enabled = *on,
                // A SET OF EXCEPTIONS, and not a list of what runs: the order
                // and the contents belong to the task, so a step added by a
                // later version is on by default instead of missing from
                // everybody's file. `on` is the state the screen wants, and not
                // a toggle — a toggle sent twice by a key that repeated would
                // land back where it started with nobody the wiser.
                Knob::Step(switch) => {
                    if switch.on {
                        mine.steps_off.remove(&switch.id);
                    } else {
                        mine.steps_off.insert(switch.id.clone());
                    }
                }
                Knob::ResetSteps => mine.steps_off.clear(),
            }
        }
        Self::tell_brain(&ronoctua_brain::Ask::Tune {
            prefs: Box::new(prefs),
        });
    }

    /// Says it to the daemon, and says so in the log when it cannot.
    ///
    /// `tell` and not `ask`, for the reason [`Self::tell_queue`] gives: none of
    /// these needs an answer, and waiting for one that never comes would freeze
    /// this window's loop. What comes back arrives on the watch stream, which
    /// is where the screen reads it anyway.
    fn tell_brain(what: &ronoctua_brain::Ask) {
        if let Err(e) = crate::brain::tell(what) {
            tracing::error!(error = %e, ?what, "the brain did not take the request");
        }
    }

    /// A fresh mind arrived from the daemon.
    ///
    /// The whole thing, every time, and never a delta — the reason is the
    /// daemon's own: a delta needs both sides to agree on what they last saw,
    /// and what is being agreed about here is when the machine will rebuild
    /// itself.
    fn mind_arrived(&mut self, mind: &ronoctua_brain::Mind) {
        let over = {
            let Some(b) = self.brain.as_mut() else {
                return;
            };
            // NOTHING CHANGED, NOTHING HAPPENS — see `Brain::last`. Everything
            // below this line costs: the conversion formats two dates, the
            // delivery calls into the component, and the component's answer is
            // a frame. A producer that repeats itself must not be able to spend
            // any of that.
            if b.last.as_ref() == Some(mind) {
                return;
            }
            b.last = Some(mind.clone());
            b.tasks.clear();
            b.tasks.extend(mind.tasks.iter().map(|t| t.id.clone()));
            b.prefs.clone_from(&mind.prefs);
            crate::brain::mind_of(mind, &b.zone)
        };
        self.deliver(&ronoctua_protocol::Input::Mind(over));
    }

    /// A fresh queue arrived from the unit.
    ///
    /// The whole list, every time, and never a delta: the buffer holds no
    /// state that could go out of step, and the list is short enough that
    /// sending it whole costs less than being wrong about it.
    fn queue_arrived(&mut self, jobs: &[ronoctua_transfers::Job]) {
        let queue = crate::transfers::queue_of(jobs);
        if let Some(q) = self.queue.as_mut() {
            q.known.clear();
            q.known.extend(queue.jobs.iter().map(|j| j.id));
        }
        self.deliver(&ronoctua_protocol::Input::Queue(queue));
    }

    /// A fresh reading of the disks arrived.
    ///
    /// The whole rack, every time, for the same reason the queue is: the buffer
    /// holds nothing that could go out of step, and a machine has a dozen block
    /// devices — sending them all costs less than being wrong about which one
    /// changed.
    fn rack_arrived(&mut self, rack: ronoctua_disks::Rack) {
        let mut over = crate::disks::rack_of(&rack, ronoctua_files::who::my_uid(), &self.texts);

        // WHAT THIS BUFFER WAS OPENED ABOUT GOES FIRST.
        //
        // The sidebar's format row opens this window naming a device, and that
        // device is what the person was looking at when they pressed. The
        // buffer puts the cursor on the first row it is given, so ORDERING is
        // all it takes — and ordering is already the host's business here: the
        // WIT says the list arrives cut and decided, and "most relevant first"
        // is the ordinary meaning of that.
        //
        // Not a field on the record, and not a second `input` case: either
        // would be a way for the host to move a cursor the buffer owns, and a
        // cursor with two owners is one that jumps under somebody's hand while
        // they are choosing which disk to erase.
        if let Some(about) = self.file.as_ref().map(|p| p.to_string_lossy().into_owned())
            && let Some(at) = over.devices.iter().position(|d| d.id == about)
        {
            let chosen = over.devices.remove(at);
            over.devices.insert(0, chosen);
        }
        if let Some(d) = self.disks.as_mut() {
            d.known.clear();
            d.known.extend(
                over.devices
                    .iter()
                    .map(|dev| (dev.id.clone(), dev.needed.is_empty())),
            );
            d.rack = Some(rack);
        }
        self.deliver(&ronoctua_protocol::Input::Rack(over));
    }

    /// Reads the keyboards and what was decided, and sends the whole picture.
    ///
    /// # Why it reads the kernel every time
    ///
    /// `/proc/bus/input/devices` is three kilobytes and a parse. A keyboard
    /// arrives when somebody plugs it in and leaves when the battery dies, and
    /// there is no signal for either — udev would be a socket and a thread for
    /// a list nobody looks at while it changes. Reading it at every delivery
    /// costs less than being wrong about which keyboards exist.
    fn keyboards_arrived(&mut self) {
        if self.keyboards.is_none() {
            return;
        }
        let seen = ronoctua_keyboards::seen().unwrap_or_else(|e| {
            tracing::warn!(error = %e, "the kernel did not say what keyboards there are");
            Vec::new()
        });
        let prefs = ronoctua_keyboards::store().read();
        // WHAT THIS WINDOW WAS OPENED ABOUT GOES FIRST.
        //
        // The row on a Bluetooth device's page opens this naming a keyboard,
        // and that keyboard is what somebody was looking at when they pressed.
        // The buffer puts its cursor on the first row it is given, so ORDERING
        // is all it takes — and ordering is the host's business here, the same
        // arrangement the disks buffer already has. Not a field on the record:
        // that would be a way for the host to move a cursor the buffer owns.
        let mut ordered = seen;
        if let Some(about) = self.file.as_ref().map(|p| p.to_string_lossy().into_owned())
            && let Some(at) = ordered.iter().position(|k| k.id == about)
        {
            let chosen = ordered.remove(at);
            ordered.insert(0, chosen);
        }
        let word = |(keyd, text): (&str, &str), holds: bool| ronoctua_protocol::Word {
            keyd: keyd.to_owned(),
            text: text.to_owned(),
            holds,
        };
        let over = ronoctua_protocol::Boards {
            keys: ronoctua_keyboards::KEYS
                .iter()
                .map(|(k, t)| word((k, t), false))
                .collect(),
            choices: ronoctua_keyboards::CHOICES
                .iter()
                .map(|(k, t, holds)| word((k, t), *holds))
                .collect(),
            boards: ordered
                .iter()
                .map(|k| ronoctua_protocol::Board {
                    id: k.id.clone(),
                    name: k.name.clone(),
                    built_in: k.built_in,
                    decided: prefs.of(&k.id).keys.into_iter().collect(),
                })
                .collect(),
            // NO PLACE TO WRITE IS SAID AT ONCE, and not after a key.
            //
            // The directory belongs to the installed system; in the nested
            // loop the host is somebody else's distro and there is none. The
            // screen says so when it opens rather than blaming keyd on the
            // first arrow — and that loop is where most of the work happens.
            trouble: if ronoctua_keyboards::somewhere_to_write(ronoctua_keyboards::DIRECTORY) {
                self.keyboards
                    .as_ref()
                    .map(|k| k.trouble.clone())
                    .unwrap_or_default()
            } else {
                ronoctua_i18n::key::KEYBOARD_TROUBLE_NO_PLACE.to_owned()
            },
            // WHICH KEY THE LEADER IS MADE OF, so the screen can say so before
            // somebody moves it. Every rendered file claims that key, and a
            // rule written after it wins — which is the one remap that costs
            // the key opening everything else.
            leader: ronoctua_keyboards::LEADER.0.to_owned(),
        };
        if let Some(k) = self.keyboards.as_mut() {
            k.boards = over.boards.iter().map(|b| b.id.clone()).collect();
            k.keys = over.keys.iter().map(|w| w.keyd.clone()).collect();
            k.choices = over.choices.iter().map(|w| w.keyd.clone()).collect();
        }
        self.deliver(&ronoctua_protocol::Input::Keyboards(over));
    }

    /// Makes a key on one keyboard mean something else.
    ///
    /// # The three questions, and why they are all asked HERE
    ///
    /// Did I send this keyboard; did I send this key; did I send this meaning.
    /// The buffer received all three from `keyboards_arrived` and hands them
    /// back, so anything else is a component that made one up — and what is
    /// about to be written is read by the daemon that owns the keyboard.
    ///
    /// # And the file is validated before it is installed
    ///
    /// `ronoctua_keyboards::install` renders into a scratch directory, asks
    /// `keyd check` — the daemon's own parser — and only then writes. A keyd
    /// that refuses its configuration is a keyboard with no leader on a
    /// machine somebody is holding.
    fn remap(&mut self, what: &ronoctua_protocol::Remap) {
        let Some(gate) = self.keyboards.as_ref() else {
            return;
        };
        if !gate.boards.contains(&what.device) {
            tracing::error!(
                device = what.device,
                "remap refused: no such keyboard was sent"
            );
            return;
        }
        if !gate.keys.contains(&what.key) {
            tracing::error!(key = what.key, "remap refused: that key was not offered");
            return;
        }
        if !gate.choices.contains(&what.becomes) {
            tracing::error!(
                becomes = what.becomes,
                "remap refused: that meaning was not offered"
            );
            return;
        }

        if !ronoctua_keyboards::somewhere_to_write(ronoctua_keyboards::DIRECTORY) {
            tracing::warn!(
                directory = ronoctua_keyboards::DIRECTORY,
                "no place for the keyboard rules on this machine"
            );
            self.trouble(ronoctua_i18n::key::KEYBOARD_TROUBLE_NO_PLACE);
            return;
        }
        let store = ronoctua_keyboards::store();
        // `parsed` AND NOT `read`: this writes the whole value back, and a
        // reader's default over a file that will not parse is how somebody
        // loses every keyboard they had configured. The rule is written out in
        // `ronoctua_store`.
        let Some(mut prefs) = store.parsed() else {
            tracing::error!("the keyboards file will not parse; refusing to write over it");
            self.trouble(ronoctua_i18n::key::KEYBOARD_TROUBLE_FILE);
            return;
        };
        prefs.set(&what.device, &what.key, &what.becomes);
        let files = ronoctua_keyboards::render(&prefs);
        if let Err(e) = ronoctua_keyboards::install(ronoctua_keyboards::DIRECTORY, &files) {
            tracing::error!(error = %e, "keyd would not take the rules");
            self.trouble(ronoctua_i18n::key::KEYBOARD_TROUBLE_REFUSED);
            return;
        }
        if let Err(e) = store.write(&prefs) {
            tracing::error!(error = %e, "the keyboards file was not written");
            self.trouble(ronoctua_i18n::key::KEYBOARD_TROUBLE_FILE);
            return;
        }
        // AND THE DAEMON IS TOLD, through the polkit rule that already exists
        // for `keyboard-restore`: one verb, one unit, one user.
        if let Err(e) = crate::keyboard::restore_it() {
            tracing::error!(error = %e, "keyd did not restart");
            self.trouble(ronoctua_i18n::key::KEYBOARD_TROUBLE_RESTART);
            return;
        }
        if let Some(k) = self.keyboards.as_mut() {
            k.trouble.clear();
        }
        self.keyboards_arrived();
    }

    /// Remembers why a write did not go through, and says so on the next
    /// delivery — the buffer draws it, because a refusal nobody sees is a key
    /// that did nothing.
    fn trouble(&mut self, key: &str) {
        if let Some(k) = self.keyboards.as_mut() {
            k.trouble = key.to_owned();
        }
        self.keyboards_arrived();
    }

    /// Writes a filesystem, once the target checks out.
    ///
    /// # The three questions, and why they are all asked HERE
    ///
    /// Did I send this id; did I send it as a TARGET; and is it still one. The
    /// buffer asked the person twice before getting this far, and none of that
    /// counts: the buffer is a separate binary and a guard that lives inside
    /// the thing being guarded holds until somebody writes a second one.
    fn format(&mut self, plan: &ronoctua_protocol::Plan) {
        let Some(d) = self.disks.as_ref() else {
            return;
        };
        // ONE LOOKUP ANSWERS BOTH: an id that was never sent is absent, and one
        // sent as un-writable is present and `false`. Two different mistakes,
        // and neither is a person's — so they are logged and not shown.
        match d.known.iter().find(|(id, _)| id == &plan.device) {
            None => {
                tracing::error!(
                    device = %plan.device,
                    "format refused: a device that was not sent"
                );
                return;
            }
            Some((_, false)) => {
                tracing::error!(
                    device = %plan.device,
                "format refused: the running system needs this one"
                );
                return;
            }
            Some((_, true)) => {}
        }

        // AND THE JUDGEMENT IS MADE AGAIN, against the volume rather than
        // against the string. `known` was written when the rack was sent, and
        // between then and now somebody may have mounted the disk over `/mnt`
        // or the administrator may have edited `fstab`. The reading is the same
        // one the sidebar reads, and it is the last word.
        let rack = d.rack.as_ref();
        let target = rack.and_then(|r| r.volumes.iter().find(|v| v.at == plan.device));
        let Some(target) = target else {
            tracing::error!(device = %plan.device, "format refused: it went away between readings");
            return;
        };
        if let Some(why) = ronoctua_disks::needed(target) {
            tracing::error!(
                device = %plan.device,
                why = ?why,
                "format refused: the running system started needing this one"
            );
            return;
        }

        // WHAT HAS TO COME DOWN FIRST, resolved here where the topology is.
        //
        // udisks refuses to write over a mounted filesystem, and it is right
        // to. For a partition that is the partition itself; for a whole disk it
        // is every partition ON it, which is the case that would otherwise fail
        // with `DeviceBusy` on the most ordinary gesture there is — a stick
        // somebody just clicked into, being prepared.
        //
        // Resolved through the DRIVE and never by trimming a digit off the
        // device node: `/dev/nvme0n1` ends in one and is a disk.
        let mut first = Vec::new();
        if let Some(r) = rack {
            if plan.whole_disk {
                let drive = ronoctua_disks::attached_to(target, &r.volumes).map(str::to_owned);
                first.extend(
                    r.volumes
                        .iter()
                        .filter(|v| v.mounted())
                        .filter(|v| {
                            drive.is_some()
                                && ronoctua_disks::attached_to(v, &r.volumes) == drive.as_deref()
                        })
                        .map(|v| v.at.clone()),
                );
            } else if target.mounted() {
                first.push(target.at.clone());
            }
        }

        let Some(said) = d.said.clone() else {
            tracing::error!("no verdict channel; formatting was not asked for");
            return;
        };
        let device = plan.device.clone();
        let filesystem = plan.recipe.filesystem();
        let label = plan.label.clone();
        let whole = plan.whole_disk;
        // THE NAME IS TAKEN NOW and travels with the answer. By the time a
        // refusal is worded the disk may carry a different label — the format
        // may be what changed it — and a message naming what it is NOW would be
        // about something the person never pressed.
        let name = target.name().to_owned();

        // A THREAD, and for the reason mounting already goes to one: mkfs on a
        // sixty-gigabyte stick is seconds of work, and waiting for it inside
        // the loop that draws freezes the screen for all of them.
        if let Err(e) = std::thread::Builder::new()
            .name("ronoctua-format".to_owned())
            .spawn(move || {
                let _ = said.send(run_format(
                    &device, filesystem, &label, whole, &first, &name,
                ));
            })
        {
            tracing::error!(error = %e, "could not start the format thread");
        }
    }

    /// A format came back; the buffer is told, in words.
    fn format_said(&mut self, verdict: &Verdict) {
        let outcome = match verdict {
            Verdict::Done => ronoctua_protocol::Outcome {
                done: true,
                trouble: String::new(),
            },
            Verdict::Refused { which, why, name } => {
                use ronoctua_disks::Refusal;
                let key = match which {
                    Refusal::Busy => ronoctua_i18n::key::FORMAT_REFUSED_BUSY,
                    Refusal::NeedsAdmin => ronoctua_i18n::key::FORMAT_REFUSED_ADMIN,
                    _ => ronoctua_i18n::key::FORMAT_REFUSED_OTHER,
                };
                ronoctua_protocol::Outcome {
                    done: false,
                    trouble: self.texts.t_with(key, &[("name", name), ("why", why)]),
                }
            }
        };
        self.deliver(&ronoctua_protocol::Input::Formatted(outcome));
    }

    /// Passes one of the buffer's demands on to the unit, once the id checks out.
    fn job_demand(&mut self, what: ronoctua_protocol::Request, target: u64) {
        use ronoctua_protocol::Request as R;
        let Some(q) = self.queue.as_ref() else {
            return;
        };
        if !q.known.contains(&target) {
            // NOT an error the buffer can act on, and not a panic either: the
            // job finished between the draw and the key, which is ordinary. The
            // next ledger will already show it gone.
            tracing::warn!(id = target, "a request for a job that is not in the list");
            return;
        }
        let ask = match what {
            R::PauseJob => ronoctua_transfers::Ask::Pause { id: target },
            R::ResumeJob => ronoctua_transfers::Ask::Resume { id: target },
            R::CancelJob => ronoctua_transfers::Ask::Cancel { id: target },
            R::ForgetJob => ronoctua_transfers::Ask::Forget { id: target },
            // Unreachable by construction: `ask` only routes those four here.
            _ => return,
        };
        Self::tell_queue(&ask);
    }

    /// Says it to the unit, and says so in the log when it cannot.
    ///
    /// `tell` and not `ask`: none of these answer, and waiting for a reply that
    /// never comes would freeze this window's loop — which is the whole of the
    /// bug the footer's own split exists to avoid.
    fn tell_queue(what: &ronoctua_transfers::Ask) {
        if let Err(e) = ronoctua_transfers::client::tell(what) {
            tracing::error!(error = %e, ?what, "the queue did not take the request");
        }
    }

    /// Applies a profile for this session, and reads back what took.
    fn switch_guard(&mut self, mode: ronoctua_protocol::GuardMode) {
        let table = ronoctua_memory::guard::profiles();
        match table.of(mode) {
            Some(p) => match ronoctua_memory::guard::apply(p) {
                Ok(()) => tracing::info!(?mode, "profile applied for this session"),
                // THE MANUAL WARNS that not every property may be changed at
                // runtime. If this one may not, the screen has to say so — which
                // it does by the next survey carrying the UNCHANGED state.
                Err(e) => tracing::error!(error = %e, ?mode, "systemd refused the swap"),
            },
            None => tracing::error!(?mode, "the profile table does not have that profile"),
        }
        self.refresh_guard();
        self.sample();
    }

    /// Closes the consumer at a 1-based index of the last survey.
    fn stop_consumer(&mut self, target: u64) {
        let Some(s) = self.system.as_ref() else {
            return;
        };
        let Some(who) = usize::try_from(target)
            .ok()
            .and_then(|n| n.checked_sub(1))
            .and_then(|n| s.consumers.get(n))
            .cloned()
        else {
            tracing::error!(target, "an index outside the last sample's list");
            return;
        };
        let places = s.places.clone();
        match ronoctua_memory::scan::stop(&places, &who) {
            Ok(()) => tracing::info!(name = %who.name, "stopped"),
            // The refusal is the point of the message: the panel does not touch
            // what is outside `app.slice`, and saying so is how the person
            // learns the boundary instead of thinking the key is broken.
            Err(e) => tracing::error!(error = %e, name = %who.name, "shutting down FAILED"),
        }
        // The list is stale the moment something leaves it: scan again, so the
        // screen does not offer to close what is already gone — and the SAME
        // scan, or the screen would change subject on its own.
        if let Some(again) = self.system.as_ref().and_then(|s| s.last_scan) {
            self.start_scan(again);
        }
    }

    /// Starts a scan, cancelling whichever was running.
    ///
    /// Starting while one runs CANCELS it — which is what makes an explicit
    /// stop optional rather than required, and keeps two threads from walking
    /// the same disk.
    fn start_scan(&mut self, what: Scan) {
        self.cancel_scan();
        let Some(s) = self.system.as_mut() else {
            return;
        };
        let Some(reports) = s.reports.clone() else {
            tracing::error!("no scan channel; nothing to do");
            return;
        };
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        s.cancel = Some(std::sync::Arc::clone(&cancel));
        s.last_scan = Some(what);
        s.scanning = true;
        s.progress = 0;
        let places = s.places.clone();

        // A THREAD, because this is the half that takes seconds. The loop that
        // draws is single-threaded, and waiting here would freeze the window —
        // the same trap `systemd-run --scope` set on the footer.
        let built = std::thread::Builder::new()
            .name(format!("ronoctua-scan-{what:?}").to_lowercase())
            .spawn(move || {
                let found = match what {
                    Scan::Ram => ronoctua_memory::scan::ram(&places),
                    Scan::Disk => {
                        let mut all = ronoctua_memory::scan::disk_metadata();
                        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
                        if let Some(home) = home {
                            all.extend(ronoctua_memory::scan::disk(&home, &cancel, |p| {
                                let _ = reports.send(Scanned::Step(p));
                            }));
                        }
                        all.sort_by_key(|c| std::cmp::Reverse(c.bytes));
                        all
                    }
                };
                // A closed channel means the window is gone; there is nobody to
                // tell, and that is not an error.
                let _ = reports.send(Scanned::Done(found));
            });
        if let Err(e) = built {
            tracing::error!(error = %e, "could not start the scan thread");
            s.scanning = false;
        }
        self.sample();
    }

    /// Tells a running scan to stop, and forgets it.
    fn cancel_scan(&mut self) {
        let Some(s) = self.system.as_mut() else {
            return;
        };
        if let Some(c) = s.cancel.take() {
            // The flag, and not just dropping the channel: a walk of a home
            // directory would keep consuming I/O long after the screen moved on.
            c.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        s.scanning = false;
    }
}

/// Which scan.
#[derive(Debug, Clone, Copy)]
enum Scan {
    /// Who is holding RAM.
    Ram,
    /// Where the disk went.
    Disk,
}

impl SeatHandler for Host {
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
            // A KEYBOARD THAT REPEATS: holding a key here is meant to work,
            // and SCTK only repeats for one made this way.
            match self.seats.get_keyboard_with_repeat(
                qh,
                &seat,
                None,
                self.handle.clone(),
                Box::new(|state: &mut Self, _, event| state.typed(event)),
            ) {
                Ok(k) => self.keyboard = Some(k),
                Err(e) => tracing::warn!(error = %e, "no keyboard; the buffer takes no input"),
            }
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => self.pointer = Some(p),
                // With no pointer the buffer works entirely through the keyboard: the
                // click is a shortcut, not the only path.
                Err(e) => tracing::warn!(error = %e, "no pointer; the click will not work"),
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

impl Host {
    /// One key, from a press or from a REPEAT.
    ///
    /// SCTK delivers a repeat through the callback given to
    /// `get_keyboard_with_repeat`, which has no `Connection` and no
    /// `QueueHandle` to hand on — so the work cannot live in `press_key`, and
    /// while it did, `repeat_key` was dead code and holding a key did it
    /// once. The comment beside `repeat_key` said the opposite for a year.
    fn typed(&mut self, event: KeyEvent) {
        // `Esc` IS A KEY AND A MODIFIER, and which of the two is decided by what
        // happens before it comes back up. See [`Escape`].
        //
        // Nothing is delivered here: a tap only becomes `Cancel` on release,
        // because until then it might still be the start of a hold.
        match self.escape.press(event.keysym) {
            EscapeAct::NotMine => {}
            EscapeAct::Swallow => return,
            EscapeAct::Rotate(forward) => {
                self.rotate_state(forward);
                return;
            }
            // A press never ends a tap; only a release does.
            EscapeAct::Tap => return,
        }
        // `Ctrl+V` PASTES, and it is the only chord this host handles.
        //
        // It comes BEFORE the Ctrl guard below, otherwise it would fall into the `return` and
        // would do nothing — which is what happened: the search field ignored
        // `Ctrl+V` silently, and there was no way to tell that from "there was
        // nothing copied".
        //
        // What reads the clipboard is the HOST. The buffer does not read the
        // clipboard, as it does not read a file (invariant 11): it receives the text
        // already resolved.
        if self.mods.ctrl && event.keysym == Keysym::v {
            match paste_text() {
                Ok(t) if !t.is_empty() => self.deliver(&ronoctua_protocol::Input::Paste(t)),
                Ok(_) => tracing::debug!("the clipboard is empty"),
                Err(e) => tracing::error!(error = %e, "pasting FAILED"),
            }
            return;
        }
        // With Ctrl the key is not text: it is a chord, and a chord belongs to the buffer's
        // keymap, which the HOST dispatches as `Input::Command`. Sending the
        // character would make `C-c` become a `c` in the search.
        if self.mods.ctrl || self.mods.logo {
            // THE BUFFER'S OWN KEYMAP OPENS HERE, and only here.
            //
            // The FIRST key of a sequence is a CHORD; the ones after it are
            // plain letters. It cost the editor to learn why: with the first
            // key plain, a keymap of `m s` swallowed EVERY `m` typed into the
            // file — and when the next letter did not continue the sequence,
            // both were dropped. No word with an `m` could be written.
            //
            // A chord is not text and a letter is, and that is the whole line.
            // `Ctrl` and not `Logo`: the compositor takes `Super` for itself.
            if opens_sequence(self.mods.ctrl, self.mods.logo, self.pending.is_empty())
                && let Some(c) = event.keysym.key_char()
                && let Some(input) = self.local_key(c)
            {
                self.deliver(&input);
            }
            return;
        }
        // A sequence ALREADY under way: from the second key on it is a plain
        // letter, as in `SPC m s`. `local_key` swallows what continues the
        // sequence and drops what kills it — see `advance_sequence`.
        if !self.pending.is_empty()
            && let Some(c) = event.utf8.as_deref().and_then(|s| s.chars().next())
        {
            if let Some(input) = self.local_key(c) {
                self.deliver(&input);
            }
            return;
        }
        let Some(e) = input_of_key(event.keysym, event.utf8.as_deref()) else {
            return;
        };
        self.deliver(&e);
    }
}

impl KeyboardHandler for Host {
    /// Focus arrives — and whatever was held before it belongs to the OTHER
    /// window.
    ///
    /// `AGENTS.md` records this as a trap with a name: a modifier held across a
    /// focus change kills the keymap, and the fix is to discount what was
    /// already down both on gaining focus and on starting a sequence. Here it is
    /// `Esc`: alt-tabbing into a buffer while it is down would leave this window
    /// believing it is choosing a state, and every key would vanish.
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
        self.escape = Escape::default();
    }

    /// Focus leaves, and so does the release this window will never see.
    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        self.escape = Escape::default();
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
        // Repeating IS right here: holding the arrow scrolls the list, holding
        // backspace erases the search.
        //
        // EXCEPT WHILE `Esc` IS DOWN, and that covers both halves of the
        // gesture. `Esc` repeating would re-enter the press above on a key that
        // is already down; the arrow repeating would spin the states at the
        // keyboard's repeat rate, which is a rotation nobody can stop on the one
        // they wanted.
        if event.keysym == Keysym::Escape || self.escape.down {
            return;
        }
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
        if self.escape.release(event.keysym) == EscapeAct::Tap {
            // `Cancel` ALONE, and the state follows from it.
            //
            // The first version sent the buffer to the base state and THEN
            // delivered `Cancel`, on the reasoning that "`Esc` returns to
            // normal" should be guaranteed by the host rather than trusted to
            // six components. It cost a real regression: every one of those
            // components already walks back ONE LAYER per `Esc` — the sheet to
            // the grid, then the grid clearing the search — and jumping to the
            // base did both at once, so leaving a sheet also threw away the
            // filter the person had typed to find it.
            //
            // One layer per press is what Doom does and what the six already
            // did. The modeline stays truthful because the walk back reports
            // itself through `effect::state-changed`, which is the same path
            // every other change uses.
            self.deliver(&ronoctua_protocol::Input::Cancel);
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
        self.mods = mods;
    }
}

impl WindowHandler for Host {
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
        // A `None` size = "you choose". 640x400 is a reasonable guess for
        // a buffer that does not know its content yet.
        // WHAT THE COMPOSITOR SAID IS KEPT SEPARATELY, because what the
        // buffer can show is what comes back below — see `asked`. It is
        // taken from the CONFIGURE and from nothing else: built out of
        // the door's own fields it would be the size `resized` last
        // answered, which can only ever shrink.
        self.asked = (
            cfg.new_size.0.map_or(640, std::num::NonZeroU32::get),
            cfg.new_size.1.map_or(400, std::num::NonZeroU32::get),
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
        // IT RE-RENDERS with the actual size. The first `render` used a
        // guessed 80x24; without this the component would draw for a window
        // that is not its own — few lines on a large screen, exactly
        // what was seen in the glyph browser.
        self.request_redraw();
    }
}

impl CompositorHandler for Host {
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
            // AND THE COMPONENT IS ASKED AGAIN, not merely repainted. `paint`
            // draws the list it already has, and that list was rendered
            // against the previous grid — which the scale moves whenever the
            // buffer comes back clamped, because what this door keeps is what
            // the buffer can SHOW. The eight doors with a `recount` do that; this is the
            // one whose content comes from another process, and the argument
            // is already written beside `text_size_changed`.
            self.request_redraw();
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

impl OutputHandler for Host {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for Host {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

/// The left button from `linux/input-event-codes.h` (`BTN_LEFT`).
const LEFT_BUTTON: u32 = 0x110;

impl PointerHandler for Host {
    /// The click becomes a CELL and goes to the component.
    ///
    /// The left button only, and only on the press. The others do nothing on
    /// purpose: a context menu would be new chrome (invariant 14), and the middle
    /// one pasting text into a glyph browser would be a search typed
    /// by accident.
    ///
    /// There is no hover: highlighting the item under the mouse would ask for a frame per pixel
    /// of movement, and here the highlight already has an owner — it is the cursor, and it belongs to the
    /// component.
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for e in events {
            let PointerEventKind::Press { button, .. } = e.kind else {
                continue;
            };
            if button != LEFT_BUTTON {
                continue;
            }
            let Some((column, line)) = Self::cell_at(e.position.0, e.position.1) else {
                // A click on the margin. There is no content there, and sending cell 0
                // would make clicking the border select the first item.
                continue;
            };
            tracing::debug!(column, line, "click delivered to the component");
            self.deliver(&ronoctua_protocol::Input::Click { column, line });
        }
    }
}

smithay_client_toolkit::delegate_registry!(Host);
smithay_client_toolkit::delegate_dispatch2!(Host);

/// May a key OPEN a buffer sequence?
///
/// Pulled out so the rule can be tested without a compositor. It is ONE line,
/// and it is the line that decides whether the person can write the letter `m`
/// in a buffer whose keymap starts with `m`.
const fn opens_sequence(ctrl: bool, logo: bool, idle: bool) -> bool {
    // `logo` excluded and not ignored: `Super` belongs to the compositor, and a
    // buffer answering to it would steal a binding from niri.
    ctrl && !logo && idle
}

/// Resolves a character against a buffer's keymap.
///
/// Pulled out of [`Host::local_key`] so it can be tested WITHOUT a compositor:
/// the rest of that method is Wayland state, and this is the whole of the
/// decision.
fn advance_sequence(
    keymap: &[ronoctua_protocol::Binding],
    pending: &mut Vec<char>,
    c: char,
) -> Option<String> {
    if keymap.is_empty() {
        return None;
    }
    let mut wanted = pending.clone();
    wanted.push(c);
    let typed: String = wanted.iter().collect();

    if let Some(b) = keymap
        .iter()
        .find(|b| b.keys.split_whitespace().collect::<String>() == typed)
    {
        pending.clear();
        return Some(b.command.clone());
    }
    if keymap.iter().any(|b| {
        b.keys
            .split_whitespace()
            .collect::<String>()
            .starts_with(&typed)
    }) {
        *pending = wanted;
        return None;
    }
    // Neither: the sequence died. What was held back was never text — dropping
    // it is right, and re-injecting it would type `m` into the file a keystroke
    // after the person moved on.
    pending.clear();
    None
}

#[cfg(test)]
mod tests {
    use super::{Escape, EscapeAct};
    use smithay_client_toolkit::seat::keyboard::Keysym;

    /// A TAP of `Esc` is still `Esc`, and it acts when the key comes back up.
    #[test]
    fn a_tap_of_escape_is_a_tap() {
        let mut e = Escape::default();
        assert_eq!(e.press(Keysym::Escape), EscapeAct::Swallow);
        assert_eq!(e.release(Keysym::Escape), EscapeAct::Tap);
        // And it left nothing behind: the next key is not this layer's.
        assert_eq!(e.press(Keysym::a), EscapeAct::NotMine);
    }

    /// HELD, it is a modifier, and letting go is NOT a cancellation.
    #[test]
    fn escape_and_an_arrow_rotates_and_does_not_cancel() {
        let mut e = Escape::default();
        e.press(Keysym::Escape);
        assert_eq!(e.press(Keysym::Right), EscapeAct::Rotate(true));
        assert_eq!(e.press(Keysym::Left), EscapeAct::Rotate(false));
        // THE WHOLE POINT: the release after a rotation must not cancel. If it
        // did, choosing a state would clear the search on the way out.
        assert_eq!(e.release(Keysym::Escape), EscapeAct::Swallow);
    }

    /// Anything else pressed while it is held is EATEN, and still not a tap.
    #[test]
    fn a_letter_under_escape_reaches_nothing() {
        let mut e = Escape::default();
        e.press(Keysym::Escape);
        assert_eq!(e.press(Keysym::a), EscapeAct::Swallow);
        assert_eq!(e.release(Keysym::Escape), EscapeAct::Swallow);
    }

    /// A REPEAT IS NOT A SECOND PRESS.
    ///
    /// The defect it prevents: holding `Esc` a moment longer than the repeat
    /// delay after rotating would clear `used`, and letting go would cancel the
    /// search the person had just filtered with.
    #[test]
    fn repeating_escape_does_not_forget_the_rotation() {
        let mut e = Escape::default();
        e.press(Keysym::Escape);
        e.press(Keysym::Right);
        // The compositor repeats the key that is down.
        assert_eq!(e.press(Keysym::Escape), EscapeAct::Swallow);
        assert_eq!(e.release(Keysym::Escape), EscapeAct::Swallow);
    }

    /// A RELEASE WITH NO PRESS cancels nothing.
    ///
    /// It happens for real: Alt-Tabbing into a buffer while `Esc` is down gives
    /// this window the release and never the press. `AGENTS.md` records the same
    /// trap for the leader — a modifier held across a focus change has to be
    /// discounted, on gaining focus AND on starting a sequence.
    #[test]
    fn a_release_this_window_never_saw_pressed_does_nothing() {
        let mut e = Escape::default();
        assert_eq!(e.release(Keysym::Escape), EscapeAct::Swallow);
    }

    /// And a key that has nothing to do with it passes straight through.
    #[test]
    fn every_other_key_is_not_this_layers() {
        let mut e = Escape::default();
        assert_eq!(e.press(Keysym::Right), EscapeAct::NotMine);
        assert_eq!(e.release(Keysym::Right), EscapeAct::NotMine);
    }

    /// The defect this gate exists for: the editor swallowed every `m`.
    ///
    /// A plain letter must NEVER open a sequence. When it did, typing `m` was
    /// held back and, if the next letter did not continue `m s`, both were
    /// dropped — no word with an `m` could be written into the file.
    #[test]
    fn a_plain_letter_does_not_open_a_sequence() {
        assert!(!super::opens_sequence(false, false, true));
    }

    #[test]
    fn a_chord_opens_a_sequence() {
        assert!(super::opens_sequence(true, false, true));
    }

    /// `Super` is the compositor's. A buffer answering to it would steal a
    /// binding from niri.
    #[test]
    fn super_does_not_open_a_sequence() {
        assert!(!super::opens_sequence(true, true, true));
    }

    /// With a sequence under way the gate is CLOSED: from the second key on the
    /// letters are plain, and it is the caller that routes them.
    #[test]
    fn a_sequence_under_way_does_not_open_another() {
        assert!(!super::opens_sequence(true, false, false));
    }

    use super::*;

    /// The keymap the editor declares.
    fn buffer_keymap() -> Vec<ronoctua_protocol::Binding> {
        vec![ronoctua_protocol::Binding {
            keys: "m s".into(),
            command: "memory-refresh".into(),
        }]
    }

    #[test]
    fn the_sequence_only_fires_when_it_completes() {
        // It is what makes an editor SAVEABLE. The binding had been declared
        // since section 7.9 and nothing consumed it: the buffer would open,
        // edit, and lose the work.
        let km = buffer_keymap();
        let mut pending = Vec::new();
        assert_eq!(
            advance_sequence(&km, &mut pending, 'm'),
            None,
            "`m` on its own does not fire"
        );
        assert_eq!(pending, vec!['m'], "the prefix has to stay pending");
        assert_eq!(
            advance_sequence(&km, &mut pending, 's').as_deref(),
            Some("memory-refresh")
        );
        assert!(pending.is_empty(), "firing has to clear the sequence");
    }

    #[test]
    fn a_dead_sequence_does_not_leak_into_the_text() {
        // `m` then `x`: no binding. Re-injecting the `m` would type it into the
        // file one keystroke after the person had moved on — the class of bug
        // that makes an editor untrustworthy.
        let km = buffer_keymap();
        let mut pending = Vec::new();
        advance_sequence(&km, &mut pending, 'm');
        assert_eq!(advance_sequence(&km, &mut pending, 'x'), None);
        assert!(pending.is_empty(), "a dead sequence has to clear");
    }

    #[test]
    fn a_buffer_with_no_keymap_swallows_nothing() {
        // Every other buffer types text. If an empty keymap held a character
        // back, the search in the glyph browser would lose its first letter.
        let mut pending = Vec::new();
        assert_eq!(advance_sequence(&[], &mut pending, 'm'), None);
        assert!(pending.is_empty());
    }

    /// An SVG in the form Lucide publishes: `currentColor` and an open stroke.
    const LUCIDE: &str = concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" "#,
        r#"viewBox="0 0 24 24" fill="none" stroke="currentColor" "#,
        r#"stroke-width="2" stroke-linecap="round">"#,
        r#"<path d="M3 12h18" /><path d="M12 3v18" /></svg>"#
    );

    /// THE DEFECT: a black preview over a black background.
    ///
    /// `currentColor` **with no context is black**. Over the buffer's background the
    /// drawing would come out invisible — and invisible is indistinguishable from "this
    /// primitive is not implemented yet", which is the worst possible
    /// result: it looks as if nothing was done.
    #[test]
    fn the_svg_key_changes_with_the_theme() {
        let ink = tokens::color::TEXT_PRIMARY;
        // Two themes, one document: two keys, so the GPU cache cannot serve
        // the old theme's ink for the new one.
        assert_ne!(svg_key(16, 0, ink, LUCIDE), svg_key(16, 1, ink, LUCIDE));
        assert!(svg_key(16, 0, ink, LUCIDE).ends_with(LUCIDE));
        // AND THE SIZE, for the same reason: the raster is `side` pixels
        // square, and `side` moves with the size a person chose.
        assert_ne!(svg_key(16, 0, ink, LUCIDE), svg_key(32, 0, ink, LUCIDE));
        // AND THE TOKEN, which is the OTHER half of the ink.
        //
        // The theme was put in the key on the argument that it is what carries
        // the colour; the colour is `theme x token`, and this half was left
        // out. One document drawn `text.primary` in a row and `state.error` in
        // a warning spelled ONE key, and the second was served the first's
        // raster in the first's colour — for ever, since the key never moves.
        assert_ne!(
            svg_key(16, 0, tokens::color::TEXT_PRIMARY, LUCIDE),
            svg_key(16, 0, tokens::color::STATE_ERROR, LUCIDE),
        );
    }

    #[test]
    fn the_current_color_becomes_the_token_and_the_drawing_shows_up() {
        let resolved = resolver_current_color(LUCIDE, tokens::color::TEXT_PRIMARY);
        assert!(
            !resolved.contains("currentColor"),
            "the `currentColor` survived: {resolved}"
        );
        let [r, g, b, _] = tokens::color::TEXT_PRIMARY.to_rgba8();
        assert!(
            resolved.contains(&format!("#{r:02X}{g:02X}{b:02X}")),
            "the token's colour did not come through: {resolved}"
        );

        // AND IT DRAWS. Finding the string is not drawing: a document `usvg`
        // refuses returns `None` in the same place, and the area stays empty.
        let side = 48;
        let rgba = ronoctua_notify::icon::from_svg(
            resolved.as_bytes(),
            side,
            ronoctua_notify::icon::Fit::Fill,
        )
        .expect("Lucide's SVG did not rasterise");
        assert_eq!((rgba.width, rgba.height), (side, side));
        let opaque = rgba.pixels.chunks_exact(4).filter(|p| p[3] > 0).count();
        assert!(
            opaque > 100,
            "the two-line cross rasterised with {opaque} visible pixels"
        );
    }

    /// THE DEFECT, SEEN ON SCREEN: the preview did not show the size.
    ///
    /// Lucide's SVG carries `width`/`height` in pixels and a fixed `viewBox` of
    /// 24. Rasterising to **fill** the box, a `width="24"` and a
    /// `width="512"` produce the same image — the size adjustment had
    /// no visible effect at all, and the preview confidently showed a size that
    /// was not the chosen one. `Natural` is what makes the row's number and the
    /// drawing say the same thing.
    #[test]
    fn the_preview_draws_at_the_documents_size() {
        let at = |px: u32| {
            LUCIDE.replace(
                r#"width="24" height="24""#,
                &format!(r#"width="{px}" height="{px}""#),
            )
        };
        // THROUGH THE PRODUCTION PATH, and not calling the rasteriser directly: the
        // wrong choice lives at the CALL SITE, and a test that passed
        // `Natural` by hand would stay green with the preview filling.
        //
        // The ceiling is generous: what is proved here is that the drawing FOLLOWS the
        // document, and not that it was clipped.
        let ceiling = 200;
        let preview = |px: u32| {
            rasterize_preview(&at(px), tokens::color::TEXT_PRIMARY, ceiling)
                .unwrap_or_else(|| panic!("the {px}px SVG did not rasterise"))
        };
        for px in [16u32, 24, 48, 128] {
            let r = preview(px);
            assert_eq!(
                (r.width, r.height),
                (px, px),
                "the document asked for {px}px and came out {}x{}",
                r.width,
                r.height
            );
        }
        // ABOVE THE CEILING it shrinks, like any viewer — the row's number
        // still says the actual size.
        let large = preview(512);
        assert_eq!((large.width, large.height), (ceiling, ceiling));

        // And the APPLICATION icon still FILLS: there the file's size
        // means nothing, and what matters is filling the cell.
        for px in [16u32, 512] {
            let r = ronoctua_notify::icon::from_svg(
                at(px).as_bytes(),
                18,
                ronoctua_notify::icon::Fit::Fill,
            )
            .expect("it did not rasterise");
            assert_eq!((r.width, r.height), (18, 18));
        }
    }

    /// And the AREA does not enlarge what already came out at the right size.
    ///
    /// `fit_into` grows to the edge; using it here would undo the natural
    /// rasterisation and would give the defect back — 24 px and 512 px would go back to being the same
    /// image, one step later.
    #[test]
    fn the_preview_area_centres_without_scaling_up() {
        use ronoctua_render::image::Area;
        let bounds = Area {
            x: 100.0,
            y: 10.0,
            width: 200.0,
            height: 200.0,
        };
        let small = bounds.centre(24, 24);
        assert_eq!((small.width, small.height), (24.0, 24.0));
        assert!(
            (small.x - 188.0).abs() < 0.01 && (small.y - 98.0).abs() < 0.01,
            "it did not centre: {small:?}"
        );
        // What does NOT fit shrinks, and then `centre` and `fit_into` agree.
        assert_eq!(bounds.centre(400, 400), bounds.fit_into(400, 400));
    }

    /// And the person's colour passes through intact: there it is **content**, not chrome.
    #[test]
    fn a_typed_colour_is_not_replaced() {
        let mine = LUCIDE.replace("currentColor", "#8AB4F8");
        assert_eq!(
            resolver_current_color(&mine, tokens::color::TEXT_PRIMARY),
            mine,
            "the chosen colour was swapped for the token's"
        );
    }

    /// A broken SVG is `None`, and the area stays empty — which is what one sees when
    /// typing a half-finished colour (`#8ab`). Inventing a drawing would be worse.
    #[test]
    fn an_invalid_svg_does_not_draw_and_does_not_panic() {
        assert!(
            ronoctua_notify::icon::from_svg(b"i am not svg", 48, ronoctua_notify::icon::Fit::Fill)
                .is_none()
        );
        assert!(
            ronoctua_notify::icon::from_svg(LUCIDE.as_bytes(), 0, ronoctua_notify::icon::Fit::Fill)
                .is_none()
        );
    }
}
