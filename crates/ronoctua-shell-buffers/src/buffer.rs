//! A native buffer hosted in an `xdg_toplevel` of its own — section 7.5.
//!
//! # Why an `xdg_toplevel`, and not a region of our surface
//!
//! Invariant 4 is explicit: **the shell never reimplements window
//! layout**. A native buffer is a WINDOW like any other, and what
//! decides where it goes is the compositor. Drawing buffers inside a fullscreen
//! surface of ours would throw the compositor away — and section 7.5 forbids that
//! by name.
//!
//! The practical consequence: a native buffer appears in the compositor's window
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

use crate::door::{Canvas, Content, Door, Hand, Ui};
use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::channel as calloop_channel,
    // The clock, for the buffer that samples the system. It reaches here
    // through the toolkit's re-export, like everything else from calloop —
    // depending on the crate directly would risk two versions of it.
    reexports::calloop::timer,
    seat::{
        keyboard::{KeyEvent, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
};

// THE BRIDGE MOVED OUT, and this is the only copy: see `paint`. The
// terminal already drew display lists through it, and the offscreen tool
// that writes a buffer to a PNG needs the same conversion — a second one
// would prove things about itself.
use crate::clipboard_copy::copy_to_clipboard;
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

/// The side a `Command::Svg` is rasterised at, in PHYSICAL pixels: the
/// square its rectangle of cells holds, times the window's scale — so the
/// drawing is as sharp as the letters beside it, and a scale is a new key.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn svg_side_px(columns: u16, lines: u16, (cell_px, line_px): (f32, f32), by: u32) -> u32 {
    let logical = (f32::from(columns) * cell_px)
        .min(f32::from(lines) * line_px)
        .max(1.0) as u32;
    logical.saturating_mul(by.max(1))
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

/// A bar of the last frame, held for as long as it is being dragged.
///
/// Its numbers are COPIED at the press and not looked up again: the display
/// list is rebuilt on every frame — and this drag causes frames — so a bar
/// found by index would be a different bar a moment later.
///
/// It is `paint::Grab`, whole, and not a shape of this module's own: the
/// rectangle a press is tested against has to be the rectangle the thumb was
/// painted in, and the surest way to say that is to hold the same value. The
/// version of this that kept its own `(column, line)` could not match a bar in
/// the last column at all — see [`crate::paint::Grab`].
type DraggedBar = crate::paint::Grab;

/// The queue in the words a buffer is handed it (`ronoctua_protocol::Queue`).
///
/// # Why it is handed in
///
/// Saying a job needs the words of everything a job can be — an archive, a
/// seal, a program, the belongings — which are the work's (`ronoctua_shell_work::jobs`),
/// above the host of buffers: the host stands on the door alone, and whoever
/// opens the window hands the words in (the binary, `jobs::queue_of`). The
/// sudo door's `Gate::footer` takes its gesture the same way.
pub type QueueWords =
    fn(&[ronoctua_transfers::Job], &ronoctua_i18n::Texts) -> ronoctua_protocol::Queue;

/// The state of the window hosting a buffer.
struct Host {
    /// `Ctrl+V` was pressed: the door reads the clipboard OFF THE LOOP
    /// (`Ui::paste`) and hands the text to `Content::pasted`.
    paste_wanted: bool,
    /// Launcher entries a demand of the component asked to open — what an
    /// install made — started at the end of the turn by the door, off the
    /// loop (`Ui::launch_entry`), which says on THIS window what went wrong.
    to_launch: Vec<String>,
    /// A question to put on this window's line at the end of the turn
    /// (`Ui::ask`) — what a brain module's action asks before it is done: its
    /// id is the action's command, which the answer comes back to
    /// `Content::answered` under, and its words the key the module declared.
    to_ask: Option<crate::line::Line>,
    /// A sentence to put on this window's line at the end of the turn — what
    /// the brain answered to something this window asked of it, a refusal
    /// above all: said in the door that asked.
    to_say: Option<String>,
    /// The question on this window's line, as it was last DRAWN — under the
    /// component's lines, where the door's message goes — and where each of
    /// its two keys landed, for the pointer: a key drawn is a key a click
    /// presses (`chrome::hint_marks`, the same `laid` the drawing reads).
    question: Vec<crate::chrome::Piece>,
    /// Which of the question's two keys the pointer rests on — lit, as every
    /// shortcut under the pointer is, from the same marks a click reads.
    question_lit: Option<&'static str>,
    /// The door's message, wrapped to the window — drawn on the lines under
    /// the component's, which it is rendered without while there is one.
    ///
    /// A buffer draws every line of its window itself, and the door's
    /// sentences — a footer that is not there, a paste that failed, a text
    /// swept and copied, a program that could not be started — reached no
    /// line of it: they were said, and nobody could read them.
    said: Vec<String>,
    /// The seat the buffer was last told the keys are from — so
    /// `keyboard-seat` is sent when it CHANGES, and not before every key.
    typing_seat: Option<u16>,
    /// Whose keyboard the key being handled came from — the door says so on
    /// every key (`Ui::keyboard_seat`).
    key_seat: u16,
    /// The window's title and `app_id`, from the descriptor — see `host_it`.
    title: String,
    app_id: &'static str,
    /// The theme the rasterised drawings were made in: a theme chosen since
    /// drops the old theme's CPU copies (the key already tells them apart).
    theme_seen: usize,
    /// Whether more than one cursor was on this window at the last frame.
    many_pointers: bool,
    /// The modifiers held with the key being handled.
    mods: Modifiers,
    /// The window's size in LOGICAL pixels — what the compositor configured
    /// it to, what the columns are counted from, what the pointer arrives in.
    width: u32,
    /// See the field above.
    height: u32,
    /// A frame is owed, and it is drawn once the queue is empty.
    ///
    /// # Why the pointer does not draw where it is handled
    ///
    /// Because presenting blocks on the vertical blank. This door used to ask
    /// the component and paint INSIDE the handler, once per event — and a
    /// pointer sweep arrives as a burst of motion in one `pointer_frame`. So a
    /// single sweep cost one wasm call, one layout and one blocked `present`
    /// per CELL crossed, and every one of those frames drew a position the
    /// pointer had already left. The owner reported it in those words: the
    /// highlight trails the mouse, and worse the faster you move.
    ///
    /// It is the same defect the file manager had on its scrollbar and the
    /// footer had on its hover, and the cure is the one both of them already
    /// carry: a handler MARKS, and the `calloop` callback — the one place that
    /// knows the batch is finished — draws once, from the last event.
    ///
    /// The render is deferred with the paint, and deliberately: asking the
    /// component per event and drawing once would still cross the wall N
    /// times to produce one frame.
    owed: bool,
    quit: bool,
    /// The file this buffer was opened with, when there is one.
    ///
    /// It is the HOST that holds it, and not the component: `Effect::Save`
    /// carries only the text, and the buffer never learns a path. That is what
    /// keeps invariant 11 intact while a file editor exists — the same shape as
    /// `Effect::Copy`, where the buffer hands text over and the host decides
    /// where it goes.
    file: Option<std::path::PathBuf>,
    /// How the queue is said to the buffer — see [`QueueWords`].
    queue_words: QueueWords,
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
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
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

/// What this window keeps for ONE pointer — see `crate::seats`.
#[derive(Debug, Default)]
struct PerSeat {
    /// What the wheel has earned and not yet spent, in LINES.
    ///
    /// The fraction has to survive between events or a slow gesture moves
    /// nothing and then jumps — see [`crate::wheel::wheel_arrived`], which
    /// carries the whole of that argument and is the only place this is added
    /// to.
    wheel_left: f64,
    /// Which cell of the BUFFER the pointer is resting on.
    ///
    /// `None` when it is outside the window or on the margin. It is kept so an
    /// event crosses the wall only when the cell CHANGES — the pointer arrives
    /// in pixels and there are a few dozen of them to a cell.
    cell_under_pointer: Option<(u16, u16)>,
    /// Which bar of the last frame is being dragged, and where its block is.
    ///
    /// # Why the host answers a drag at all
    ///
    /// Because it is the only side that CAN. The thumb is a fraction of a
    /// track in pixels, and a component counts cells (invariant 11) — so the
    /// bar crosses as three numbers and the host resolves it, which is the
    /// whole argument `ronoctua_design::Scroll` is written on. The inverse
    /// belongs on the same side as the forward: `Scroll::offset_at`.
    ///
    /// The owner reported the missing half in one sentence — *"o clicar e
    /// arrastar a scrollbar com o mouse não está funcionando"* — and the wheel
    /// already crossed, so the shape was there.
    ///
    /// It holds the bar's own `Command::Bar` numbers, copied at the press: the
    /// list is rebuilt on every frame and the drag outlives a frame.
    dragging: Option<DraggedBar>,
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
    /// The launcher entry each finished install of those rows made — what a
    /// `launch-job` opens. The buffer is told only THAT a row opens
    /// something (`job.opens`); which entry is the host's to know.
    opens: Vec<(u64, String)>,
}

impl Queue {
    const fn fresh() -> Self {
        Self {
            known: Vec::new(),
            opens: Vec::new(),
        }
    }

    /// Takes a fresh list: its ids, and the entries its installs made.
    fn took(&mut self, jobs: &[ronoctua_transfers::Job]) {
        self.known.clear();
        self.known.extend(jobs.iter().map(|j| j.id));
        self.opens = jobs
            .iter()
            .filter_map(|j| crate::queue::opens(j).map(|d| (j.id, d.to_owned())))
            .collect();
    }

    /// The entry the row `id` opens, when it opens one.
    fn opening(&self, id: u64) -> Option<&str> {
        self.opens
            .iter()
            .find(|(of, _)| *of == id)
            .map(|(_, d)| d.as_str())
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
    /// same split `report::Report::Refused` already makes.
    said: Option<calloop_channel::Sender<Verdict>>,
    /// A plan that asked for ENCRYPTION, waiting for its passphrase — and the
    /// passphrase's first answer, waiting for the second.
    ///
    /// The passphrase is asked HERE, on this window's own line, twice: it is
    /// never a field of the plan and never crosses into the component
    /// (`buffers/format`), which only said yes.
    sealing: Option<(
        ronoctua_protocol::Plan,
        Option<ronoctua_consent::keys::Secret>,
    )>,
}

/// The line's ids for the passphrase of an encrypted format: asked, and asked
/// again. Their questions are `ask-<id>`.
const PASSPHRASE: &str = "format-encrypt-passphrase";
/// See [`PASSPHRASE`].
const PASSPHRASE_AGAIN: &str = "format-encrypt-again";

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
            sealing: None,
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
    /// The identities of the units delivered, the cursors offered, and the
    /// layouts — what a unit change may name.
    units: Vec<String>,
    cursors: Vec<u16>,
    /// How the cursors were numbered when they were sent: the screen draws
    /// 1, 2, 3… with no hole ([`ronoctua_pointers::Shown`]), and a number
    /// coming back is turned into a kept one by the SAME numbering.
    shown: Option<ronoctua_pointers::Shown>,
    layouts: Vec<(String, Vec<String>)>,
    /// The watch on the input devices' directory, kept for as long as the
    /// window is open: dropped with it, the watcher's thread leaves.
    _plugged: Option<ronoctua_files::notice::Notice>,
}

/// Where the kernel's input devices appear and go — `devtmpfs`, whose nodes
/// are made through `vfs_mknod` and so are announced to inotify like any
/// other file: a keyboard plugged in, a Bluetooth one pairing, a battery
/// dying, each is an `eventN` created or deleted here.
pub const INPUT_DEVICES: &str = "/dev/input";

/// Every keyboard unit, plugged or remembered, as the buffer draws it.
fn units_of(
    prefs: &ronoctua_keyboards::Preferences,
    shown: &ronoctua_pointers::Shown,
) -> Vec<ronoctua_protocol::UnitBoard> {
    let seen = ronoctua_keyboards::units().unwrap_or_default();
    let pointers = ronoctua_pointers::prefs::store().read();
    let remembered = || {
        pointers
            .devices
            .iter()
            .filter_map(|(id, d)| d.cursor.map(|c| (id.as_str(), c)))
    };
    let board = |identity: String,
                 name: String,
                 model: String,
                 built_in: bool,
                 plugged: bool,
                 paired: Option<u16>| {
        let own = prefs.units.get(&identity).cloned().unwrap_or_default();
        let decided = own
            .keys
            .clone()
            .unwrap_or_else(|| prefs.of(&model).keys)
            .into_iter()
            .collect();
        ronoctua_protocol::UnitBoard {
            leader: prefs.unit_leader(&identity),
            identity,
            name: if name.is_empty() {
                own.name.clone()
            } else {
                name
            },
            model,
            built_in,
            plugged,
            cursor: shown
                .label(own.cursor.or(paired).unwrap_or(ronoctua_pointers::FIRST))
                .unwrap_or(ronoctua_pointers::FIRST),
            layout: own.layout.unwrap_or_default(),
            variant: own.variant.unwrap_or_default(),
            repeat_rate: own.repeat_rate.unwrap_or(0),
            repeat_delay: own.repeat_delay.unwrap_or(0),
            decided,
        }
    };
    let mut out: Vec<_> = seen
        .iter()
        .map(|u| {
            board(
                u.identity(),
                u.name.clone(),
                u.id.clone(),
                u.built_in,
                true,
                ronoctua_pointers::keyboards::paired(u, remembered()),
            )
        })
        .collect();
    for (identity, own) in &prefs.units {
        if !out.iter().any(|b| &b.identity == identity) {
            out.push(board(
                identity.clone(),
                own.name.clone(),
                String::new(),
                false,
                false,
                None,
            ));
        }
    }
    out
}

impl Keyboards {
    const fn fresh() -> Self {
        Self {
            boards: Vec::new(),
            keys: Vec::new(),
            choices: Vec::new(),
            trouble: String::new(),
            units: Vec::new(),
            cursors: Vec::new(),
            shown: None,
            layouts: Vec::new(),
            _plugged: None,
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
    /// The notices of the last mind delivered, IN THE ORDER SENT
    /// (`mind_wire::ranked`) — what `dismiss-notice`, `snooze-notice` and
    /// `act-notice` index into, by the same discipline as `tasks`.
    notices: Vec<Noticed>,
    /// The module whose page the component asked to follow
    /// (`request::watch-screen`), by id — `None` when no page is open.
    watched: Option<String>,
    /// The `screen-generation` the brain was last asked for the watched
    /// module's page at: a mind whose generation moved asks again, and one
    /// that did not asks nothing. An ask nobody answered forgets it, so the
    /// next mind — the brain back — asks again; a REFUSAL keeps it, since the
    /// same page asked again is refused again.
    asked_at: Option<u64>,
    /// The generation of the page last DELIVERED to the component for the
    /// watched module: an older one arriving late is dropped here, by the
    /// rule the component drops it by (`screened`), so the two never hold
    /// different pages and an `act-row` index names the row that is drawn.
    shown_at: Option<u64>,
    /// The rows of the last module page delivered, counted across its
    /// sections — `(row id, action id, second action id)` — what an
    /// `act-row` (and an `act-row-also`) index names.
    screen_rows: Vec<(String, String, String)>,
    /// Where the page's answer comes back to the loop from its thread.
    screens: Option<calloop_channel::Sender<Paged>>,
    /// Where the brain's answer to a change comes back from its thread
    /// (`brainlink::put`).
    answers: Option<calloop_channel::Sender<crate::mind_wire::Answer>>,
    /// The action waiting for what its question asks, and the row it was
    /// asked on — sent when the line answers, dropped when it is let go.
    pending: Option<(crate::brainlink::Acting, Option<String>)>,
    /// Whether the window was opened ABOUT a module (`brain-open <id>`) and
    /// has not opened its page yet: the first mind does.
    about_owed: bool,
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
    /// The daemon's own log, as far as this window has followed it — at most
    /// the daemon's own ring, oldest first (`ronoctua_brain::tail`).
    log: std::collections::VecDeque<ronoctua_brain::tail::Line>,
}

/// One notice as the component was sent it: whose, which, and what it
/// offers — enough to answer an index without the mind in hand.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Noticed {
    module: String,
    id: String,
    controls: Vec<String>,
    dismiss: bool,
    snooze: Option<u16>,
}

/// What came of asking the brain for a module's page, from the thread that
/// asked.
#[derive(Debug)]
enum Paged {
    /// The page, at the generation the brain says it is.
    Screen {
        module: String,
        generation: u64,
        screen: ronoctua_brain::module::Screen,
    },
    /// No page, and why — at the generation that was asked for.
    Not {
        module: String,
        asked: u64,
        answer: crate::mind_wire::Answer,
    },
}

impl Brain {
    fn fresh() -> Self {
        Self {
            tasks: Vec::new(),
            notices: Vec::new(),
            watched: None,
            asked_at: None,
            shown_at: None,
            screen_rows: Vec::new(),
            screens: None,
            answers: None,
            pending: None,
            about_owed: true,
            zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
            last: None,
            log: std::collections::VecDeque::new(),
        }
    }

    /// The generation of the watched module's page in the last mind.
    fn generation_of(&self, module: &str) -> Option<u64> {
        self.last
            .as_ref()
            .and_then(|m| m.task(module))
            .map(|t| t.screen_generation)
    }

    /// What a page that arrived becomes: the record to deliver, or `None`
    /// when it is not the page followed or is OLDER than the one delivered
    /// last — the component's own rule (`screened`), so the two never hold
    /// different pages.
    fn page_arrived(
        &mut self,
        paged: &Paged,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<ronoctua_protocol::ModuleScreen> {
        let (module, generation) = match paged {
            Paged::Screen {
                module, generation, ..
            } => (module.as_str(), *generation),
            Paged::Not { module, asked, .. } => (module.as_str(), *asked),
        };
        if self.watched.as_deref() != Some(module) || self.shown_at.is_some_and(|s| s > generation)
        {
            return None;
        }
        self.shown_at = Some(generation);
        Some(match paged {
            Paged::Screen { screen, .. } => {
                self.screen_rows = screen
                    .sections
                    .iter()
                    .flat_map(|s| s.rows.iter())
                    .map(|r| {
                        (
                            r.id.clone(),
                            r.act.as_deref().unwrap_or_default().to_owned(),
                            r.also.as_deref().unwrap_or_default().to_owned(),
                        )
                    })
                    .collect();
                crate::mind_wire::screen_of(module, generation, screen, &self.zone, texts)
            }
            // NO PAGE IS A SENTENCE ON THE PAGE, never an empty one: the rows
            // go, and what the brain said takes their place. An ask NOBODY
            // ANSWERED is forgotten, so the next mind — the brain back — asks
            // again; a refusal is kept, since the same page asked again is
            // refused again.
            Paged::Not { answer, .. } => {
                if *answer == crate::mind_wire::Answer::Unreachable
                    && self.asked_at == Some(generation)
                {
                    self.asked_at = None;
                }
                self.screen_rows.clear();
                crate::mind_wire::refused_screen(
                    module,
                    generation,
                    answer.sentence(texts).unwrap_or_default(),
                )
            }
        })
    }

    /// One of a module's actions, as the last mind lists it.
    fn acting(&self, module: &str, action: &str) -> crate::brainlink::Acting {
        let asks = self
            .last
            .as_ref()
            .and_then(|m| m.task(module))
            .and_then(|t| t.actions.iter().find(|a| a.id == action))
            .and_then(|a| a.asks.clone());
        crate::brainlink::Acting {
            module: module.to_owned(),
            action: action.to_owned(),
            command: format!("{module}-{action}"),
            asks,
        }
    }

    /// Takes a batch of the daemon's log into what this window holds.
    ///
    /// A batch whose first line is not after the last one held is the RING,
    /// sent whole — on a reconnection, or from a daemon that restarted and
    /// numbers from one again — and it replaces what was held rather than
    /// being appended to it twice.
    fn took_lines(&mut self, lines: Vec<ronoctua_brain::tail::Line>) {
        let last = self.log.back().map_or(0, |l| l.seq);
        if lines.first().is_some_and(|l| l.seq <= last) {
            self.log.clear();
        }
        self.log.extend(lines);
        while self.log.len() > ronoctua_brain::tail::KEPT {
            self.log.pop_front();
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

/// Opens an `xdg_toplevel` and draws a component in it — section 7.5.
///
/// `queue_words` says the queue to a buffer that is handed it — see
/// [`QueueWords`].
///
/// # Errors
///
/// There is no compositor, the component does not load, or the `render` fails.
pub fn host_it(
    path: &std::path::Path,
    file: Option<&std::path::Path>,
    queue_words: QueueWords,
) -> Result<()> {
    // THE SOCKET THE KEYMAP IS PUBLISHED ON, removed when the window closes:
    // a stale one in `XDG_RUNTIME_DIR` would make the footer connect to a
    // buffer that is not there and wait.
    let published: std::cell::RefCell<Option<std::path::PathBuf>> = std::cell::RefCell::new(None);
    let ran = crate::door::run(|w| {
        // THE INSTANCE LIVES. Loaded once, it keeps the component's
        // state between keys — search, cursor, scroll.
        let wasm =
            std::fs::read(path).with_context(|| format!("could not read {}", path.display()))?;
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
        // The `app_id` is NOT translated, and that is deliberate: the compositor's window
        // rules match on it, and a rule that only fires in one language is a rule
        // that fails on the machine of whoever changed `LANG`.
        let texts = crate::language::texts();
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
        // (`ronoctua_shell_footer::surface`, `texts`). A buffer that had to be told
        // twice would need to know it had been told once.
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

        // The initial columns and rows are a guess; the `configure` brings the actual
        // size and the `render` runs again with it.
        let list = instance.render(
            ronoctua_tokens::space::TERMINAL_COLUMNS,
            ronoctua_tokens::space::TERMINAL_LINES,
            Host::heights(),
            ronoctua_shell_kit::header::hide_header(),
        )?;
        tracing::info!(
            commands = list.commands.len(),
            height = list.height(crate::paint::heights()),
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
        let mut host = Host {
            owed: false,
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
            queue_words,
            paste_wanted: false,
            to_launch: Vec::new(),
            to_ask: None,
            to_say: None,
            question: Vec::new(),
            question_lit: None,
            said: Vec::new(),
            typing_seat: None,
            key_seat: 0,
            title: format!("*{title}*"),
            // ONE STRING FOR THE LIFE OF THE PROCESS, leaked on purpose: the
            // `app_id` is the descriptor's and a door names its `app_id` as a
            // `&'static str`. The process hosts one buffer and exits with it.
            app_id: Box::leak(format!("br.ronoctua.{}", d.mode).into_boxed_str()),
            theme_seen: tokens::theme::palette(),
            many_pointers: false,
            mods: Modifiers::default(),
            width: 0,
            height: 0,
            quit: false,
            list,
            rasterized: std::collections::HashMap::new(),
            capabilities: d.capabilities.clone(),
            // THE SAME BUNDLE that resolved the title and the phrases above.
            texts: std::rc::Rc::new(texts),
            instance,
            _runtime: runtime,
        };

        // THE KEYMAP AND THE STATES GO OUT, AND ORDERS COME BACK IN.
        //
        // The socket thread cannot touch the component: the state lives in the
        // `Instance` and it is not `Send`. So it puts the order on a calloop channel
        // and the LOOP acts on it, on the same thread as every other input — which
        // is the arrangement the footer already uses for its control socket.
        let (orders, channel) = calloop_channel::channel::<FromSocket>();
        *published.borrow_mut() = publish_keymap(d.keymap.clone(), d.states, current, orders);
        if w.handle
            .insert_source(channel, |event, (), door: &mut Door<Host>| {
                if let calloop_channel::Event::Msg(order) = event {
                    match order {
                        FromSocket::Command(id) => {
                            door.content.deliver(&ronoctua_protocol::Input::Command(id));
                        }
                        FromSocket::Named(id) => match door.content.index_of(&id) {
                            Some(n) => door.content.enter_state(n),
                            // The footer only ever sends back an id it was just
                            // told, so this is a bug on the other side rather than
                            // a person's mistake — and guessing at the nearest
                            // state would hide it.
                            None => {
                                tracing::warn!(state = %id, "a state this buffer does not declare")
                            }
                        },
                        FromSocket::Next => door.content.rotate_state(true),
                        FromSocket::Previous => door.content.rotate_state(false),
                    }
                }
            })
            .is_err()
        {
            tracing::warn!("could not hang the keymap's channel");
        }

        // AND THE FOOTER IS TOLD WHERE THIS BUFFER STARTS.
        //
        // Without this the modeline shows the state only from the first change
        // on: a window that opens and is not touched would have a mode and no
        // state, which reads as "this buffer has none" rather than "nobody
        // asked yet".
        host.tell_the_footer();

        // THE CLOCK, and the thread that answers a scan — only for the buffer that
        // declared the capability.
        if host.system.is_some() {
            // The scan's channel FIRST: the tick may start one, and a report with
            // nowhere to arrive would be a scan that runs and never shows.
            let (reports, arrivals) = calloop_channel::channel::<Scanned>();
            if w.handle
                .insert_source(arrivals, |event, (), door: &mut Door<Host>| {
                    if let calloop_channel::Event::Msg(m) = event {
                        door.content.scanned(m);
                    }
                })
                .is_ok()
            {
                if let Some(s) = host.system.as_mut() {
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
            if w.handle
                .insert_source(tick, |_, (), door: &mut Door<Host>| {
                    door.content.sample();
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
            host.sample();
            host.refresh_guard();
            host.sample();
        }

        // THE QUEUE'S SUBSCRIPTION, and it is NOT a clock.
        //
        // A socket round trip has no business inside a one-second timer: the unit
        // already knows when something changed, and `client::watch` blocks on the
        // socket until it does. So it is a thread that blocks and a channel that
        // wakes the loop — the same shape the scan uses, and the same one the
        // footer already uses for this very list.
        if host.queue.is_some() {
            let (send, arrivals) = calloop_channel::channel::<Vec<ronoctua_transfers::Job>>();
            if w.handle
                .insert_source(arrivals, |event, (), door: &mut Door<Host>| {
                    if let calloop_channel::Event::Msg(jobs) = event {
                        door.content.queue_arrived(&jobs);
                    }
                })
                .is_ok()
            {
                // `listen` starts the unit if it is not up, so an empty screen here
                // means an empty queue and never a queue nobody asked.
                if !crate::queue::listen(send) {
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
        if host.disks.is_some() {
            let (send, readings) = calloop_channel::channel::<ronoctua_disks::Rack>();
            if w.handle
                .insert_source(readings, |event, (), door: &mut Door<Host>| {
                    if let calloop_channel::Event::Msg(rack) = event {
                        door.content.rack_arrived(rack);
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
            if w.handle
                .insert_source(verdicts, |event, (), door: &mut Door<Host>| {
                    if let calloop_channel::Event::Msg(verdict) = event {
                        door.content.format_said(&verdict);
                    }
                })
                .is_ok()
            {
                if let Some(d) = host.disks.as_mut() {
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
        if host.brain.is_some() {
            let (send, minds) = calloop_channel::channel::<ronoctua_brain::Mind>();
            if w.handle
                .insert_source(minds, |event, (), door: &mut Door<Host>| {
                    if let calloop_channel::Event::Msg(mind) = event {
                        door.content.mind_arrived(&mind);
                    }
                })
                .is_ok()
            {
                if !crate::brainlink::listen(send) {
                    tracing::warn!("no brain; the buffer opens empty");
                }
            } else {
                tracing::warn!("could not hang the brain's channel");
            }
            // AND ITS OWN LOG, on a second connection: the daemon's lines have
            // a bell of their own, so following them wakes nothing that
            // watches the mind (`ronoctua_brain::tail`).
            let (send, lines) = calloop_channel::channel::<Vec<ronoctua_brain::tail::Line>>();
            if w.handle
                .insert_source(lines, |event, (), door: &mut Door<Host>| {
                    if let calloop_channel::Event::Msg(lines) = event {
                        door.content.lines_arrived(lines);
                    }
                })
                .is_ok()
            {
                if !crate::brainlink::follow(send) {
                    tracing::warn!("no brain; its log stays empty");
                }
            } else {
                tracing::warn!("could not hang the brain log's channel");
            }
            // AND A MODULE'S OWN PAGE, when the component follows one: asked
            // of the brain on a thread (`Ask::Screen`) whenever its generation
            // moves, and handed back here — never a clock, and never a wait
            // inside the loop that draws.
            let (send, screens) = calloop_channel::channel::<Paged>();
            if w.handle
                .insert_source(screens, |event, (), door: &mut Door<Host>| {
                    if let calloop_channel::Event::Msg(paged) = event {
                        door.content.screen_arrived(paged);
                    }
                })
                .is_ok()
            {
                if let Some(b) = host.brain.as_mut() {
                    b.screens = Some(send);
                }
            } else {
                tracing::warn!("could not hang the brain page's channel");
            }
            // AND WHAT THE BRAIN ANSWERED to a change asked from here — said on
            // this window's line, a refusal above all (`brainlink::put`).
            let (send, answers) = calloop_channel::channel::<crate::mind_wire::Answer>();
            if w.handle
                .insert_source(answers, |event, (), door: &mut Door<Host>| {
                    if let calloop_channel::Event::Msg(answer) = event {
                        door.content.brain_answered(&answer);
                    }
                })
                .is_ok()
            {
                if let Some(b) = host.brain.as_mut() {
                    b.answers = Some(send);
                }
            } else {
                tracing::warn!("could not hang the brain's answers");
            }
        }

        // THE KEYBOARDS ARRIVE ONCE, AND AGAIN AFTER EVERY CHANGE — of the
        // remap, and of the MACHINE: a keyboard plugged in, or paired, while
        // the window is open appears without anything being touched.
        //
        // AN EVENT AND NOT A CLOCK: the input devices' directory is watched
        // (`INPUT_DEVICES`), on the watcher's own thread, and a burst — one
        // keyboard is several nodes — is one nudge. The answer is the same
        // three kilobytes of `/proc/bus/input/devices` the window opened with,
        // sent whole.
        if host.keyboards.is_some() {
            let (told, heard) = calloop_channel::channel::<()>();
            let hung = w
                .handle
                .insert_source(heard, |event, (), door: &mut Door<Host>| {
                    if let calloop_channel::Event::Msg(()) = event {
                        door.content.keyboards_arrived();
                        door.ui.ask_paint();
                    }
                })
                .is_ok();
            let watch = hung
                .then(|| {
                    ronoctua_files::notice::Notice::on_dirs(
                        &[std::path::PathBuf::from(INPUT_DEVICES)],
                        &[],
                        ronoctua_files::notice::Pace::Coalesced,
                        move || told.send(()).is_ok(),
                    )
                    .map_err(
                        |e| tracing::warn!(error = %e, "keyboards plugged later will not appear"),
                    )
                    .ok()
                })
                .flatten();
            if let Some(k) = host.keyboards.as_mut() {
                k._plugged = watch;
            }
            host.keyboards_arrived();
        }

        Ok(host)
    });
    if let Some(path) = published.borrow_mut().take() {
        let _ = std::fs::remove_file(path);
    }
    ran
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
    passphrase: Option<&ronoctua_consent::keys::Secret>,
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
        // `put_away` WITHOUT THE POWER: unmounted, and an encrypted disk's
        // container locked again after what came out of it.
        if let Err(e) = manager.put_away(at, true) {
            return refused(ronoctua_disks::refusal(&e), e.to_string());
        }
    }

    match manager.format(device, filesystem, label, whole, passphrase) {
        Ok(()) => {
            tracing::info!(
                device,
                filesystem,
                whole,
                encrypted = passphrase.is_some(),
                "formatted"
            );
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

    let path = match crate::ipc::buffer_socket_path(std::process::id()) {
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
    let tones: Vec<&'static str> = states
        .iter()
        .map(|s| crate::ipc::tone_word(s.tone))
        .collect();

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
                        // `<id> <tone>`: the tone rides on the same line.
                        let now = ids
                            .get(n)
                            .map(|id| {
                                format!("{id} {}", tones.get(n).copied().unwrap_or("standard"))
                            })
                            .unwrap_or_default();
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
    ///
    /// # Rasterised at the PHYSICAL side
    ///
    /// The drawing is laid out in logical cells and rasterised at that box
    /// times the window's scale, so it is as sharp as the letters beside it
    /// (`ronoctua_shell_kit::hidpi`); the key carries the physical side, so a change of
    /// scale asks a question the cache has not been asked.
    fn vector_ones(&mut self, scale: i32) -> Vec<VectorDrawing> {
        let by = u32::try_from(scale.max(1)).unwrap_or(1);
        use ronoctua_protocol::Command;
        // THE SAME cell as the click's and the text's — see [`crate::paint::cell`].
        // A second calculation here would make the drawing land in one place and the click find it
        // in another, with the error growing with the distance from the origin.
        let (cell_px, line_height) = cell();
        #[allow(clippy::cast_possible_truncation)]
        let rows = crate::paint::Rows::of(&self.list.commands, self.height as f32);
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
            let side = svg_side_px(*columns, *lines, (cell_px, line_height), by);
            let key = svg_key(side, tokens::theme::palette(), token, svg);
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
            // THE SAME ROWS THE INK AND THE GROUNDS USE: a drawing inside an
            // anchored block moves with it, or it is a picture sitting a
            // residue above the row that names it.
            let y = rows.top(*line);
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
                // IN LOGICAL PIXELS: the picture holds `by` of its pixels per
                // logical one.
                .centre((r.width / by).max(1), (r.height / by).max(1)),
                key,
            });
        }
        out
    }

    /// One frame: asks the component, then lays what it said onto the canvas.
    ///
    /// # Why the component is asked HERE
    ///
    /// Because a frame is drawn once the queue is empty, and a hundred motion
    /// events in one batch come out as one frame asked of the component once,
    /// from the LAST of them — see `Host::owed`. Asking per event and drawing
    /// once would still cross the wall N times to produce one frame.
    fn draw(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        self.width = ui.width;
        self.height = ui.height;
        // A QUESTION ON THE LINE TAKES THE MESSAGE'S PLACE: it is what the
        // keyboard is typing into, and the door routes every key to it.
        self.question = self.question_of(ui);
        self.said = if self.question.is_empty() {
            self.said_of(ui.message.as_deref())
        } else {
            Vec::new()
        };
        // A THEME CHOSEN SINCE drops the old theme's CPU copies. The key
        // already tells the two apart (`svg_key`), so this is memory and not
        // correctness.
        let theme = tokens::theme::palette();
        if theme != self.theme_seen {
            self.theme_seen = theme;
            self.rasterized.clear();
        }
        self.re_render();
        let (columns, lines) = self.context_of();
        // WHILE THE DOOR SAYS SOMETHING, the block the component keeps at its
        // foot is drawn where the component put it — above the message — and
        // not counted back from the window's bottom edge, which is where the
        // message is. Dropped from the list itself, so the drawing, the click
        // and the bar all read the one layout (`paint::Rows`).
        if !self.said.is_empty() {
            self.list
                .commands
                .retain(|c| !matches!(c, ronoctua_protocol::Command::Anchor { .. }));
        }
        canvas
            .spans
            .extend(spans_of(&self.list, &self.texts, columns, lines));
        canvas.spans.extend(said_spans(&self.said, lines));
        // THE KEY UNDER THE POINTER LIT, from the marks the click reads.
        canvas.spans.extend(crate::chrome::hint_spans(
            &crate::chrome::lit(&self.question, self.question_lit),
            ronoctua_render::Size::Px(tokens::text::size()),
            tokens::space::CONTENT_INSET,
            tokens::space::CONTENT_INSETY + lines,
            columns,
        ));
        // THE SELECTION FIRST OF ALL: it is a background, and anything drawn
        // after it stays on top. THE DRAWING BELOW THE TEXT, as in the footer:
        // what is written over it has to stay legible. The canvas draws in
        // that order by construction.
        canvas.under.extend(self.selections());
        // EVERY BAR THE COMPONENT ASKED FOR IS RECORDED, by the track the
        // host drew it in: its column is the bar's (`crate::bars`), and the
        // door then tells the component the pointer on it is on no row.
        #[allow(clippy::cast_precision_loss)]
        let rows = crate::paint::Rows::of(&self.list.commands, self.height as f32);
        for c in &self.list.commands {
            #[allow(clippy::cast_precision_loss)]
            if let Some(g) =
                crate::paint::bar_box(c, &rows, self.width as f32, &self.texts, columns)
            {
                canvas.bar(Some(crate::bars::Track::of((g.x, g.y, g.width, g.height))));
            }
        }
        for v in self.vector_ones(ui.scale) {
            let rasterized = &self.rasterized;
            if canvas.upload(&v.key, || rasterized.get(&v.key).cloned()) {
                canvas.pictures.push((v.key, v.area));
            }
        }
        tracing::info!(
            spans = canvas.spans.len(),
            "buffer drawn on the xdg_toplevel"
        );
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
    /// The window is what knows how wide and how TALL it is; everything else
    /// about the band is in `paint`, next to the conversion that draws the
    /// text over it. The height goes with it because a scrollbar's track ends
    /// at the surface's bottom edge — the number the compositor gave — and
    /// never at a count of lines times a line.
    fn selections(&self) -> Vec<ronoctua_render::quad::Quad> {
        let (columns, _) = self.context_of();
        let mut quads = quads_of(
            &self.list,
            self.width as f32,
            self.height as f32,
            &self.texts,
            columns,
        );
        // ONE CURSOR KEEPS THE NEUTRAL HOVER. A buffer paints every hover in
        // its cursor's colour (`Role::HoverOf`), because it cannot know how
        // many cursors this window has; the host does, and spends colour only
        // where it tells two hands apart — the rule of every other window
        // (`crate::seats::band_colour`).
        if !self.many_pointers {
            for q in &mut quads {
                if crate::seats::is_a_cursor_hover(q.color) {
                    q.color = tokens::color::SURFACE_HOVER;
                }
            }
        }
        quads
    }

    /// Which cell OF THE BUFFER a point in the window landed in.
    ///
    /// `None` when the point is in the margin: there is no content there, and returning
    /// cell 0 would make clicking the border select the first item.
    ///
    /// The origin is the **buffer's** (0,0) cell, and not the window's corner — it is the
    /// same origin `spans_of` uses when drawing, and that is why the margin
    /// comes in here as a subtraction.
    fn cell_at(&self, x: f64, y: f64) -> Option<(u16, u16)> {
        let (width, _) = cell();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let column = (x / f64::from(width)) as u32;
        // THE LINE COMES FROM THE FRAME'S OWN ARITHMETIC, because a buffer
        // whose window ends in a footer places that block from the BOTTOM edge
        // — see `paint::Rows`. A click there resolved by `y / line` alone
        // answers the line the footer would have been on had the residue not
        // moved it, which is a click landing one row off exactly where the
        // rows are fewest.
        #[allow(clippy::cast_possible_truncation)]
        let line =
            crate::paint::Rows::of(&self.list.commands, self.height as f32).line_at(y as f32)?;
        Some((
            u16::try_from(column.checked_sub(u32::from(tokens::space::CONTENT_INSET))?).ok()?,
            line,
        ))
    }

    /// The bar of the last frame under this POINT of the surface, if any.
    ///
    /// # Why pixels, and not the cell the pointer is in
    ///
    /// Because a bar in the last column is PAINTED past the last cell — that
    /// is what makes it touch the border, and it is deliberate — while the
    /// pointer's cell is `floor(x / cell) − margin`, which in that strip
    /// answers a column the component never drew in. The two numbers could
    /// not be equal, so `column == column` never matched and a press on the
    /// thumb fell through to the row behind it. Measured on the owner's
    /// screen: the bar was column 81 and the press reported 83.
    ///
    /// There is one placement now and both doors read it: see
    /// [`crate::paint::Grab`]. Nothing is found when the list FITS, which is
    /// `Scroll::cut`'s decision and nobody else's.
    fn bar_under(&self, x: f64, y: f64) -> Option<DraggedBar> {
        #[allow(clippy::cast_possible_truncation)]
        let (columns, _) = self.context_of();
        crate::paint::bar_grabbed(
            &self.list.commands,
            x as f32,
            y as f32,
            self.width as f32,
            self.height as f32,
            &self.texts,
            columns,
        )
    }

    /// Where a drag at this height puts the first row, in the component's rows.
    ///
    /// The inverse of the thumb, asked of the track the thumb was drawn in —
    /// which is the whole reason the rectangle is one answer. Written as a
    /// second sum here it would come back somewhere other than under the
    /// finger, and a drag IS the finger.
    #[allow(clippy::unused_self)]
    fn row_dragged_to(&self, bar: DraggedBar, y: f64) -> u32 {
        #[allow(clippy::cast_possible_truncation)]
        let at = bar.row_at(y as f32);
        u32::try_from(at).unwrap_or(u32::MAX)
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
        // THE DOOR'S MESSAGE TAKES ITS LINES FROM THE COMPONENT, which is told
        // a window that much shorter — nothing it draws is covered. And so
        // does a question, which is one line.
        let said = u16::try_from(self.said.len())
            .unwrap_or(u16::MAX)
            .saturating_add(u16::from(!self.question.is_empty()));
        (columns, lines.saturating_sub(said).max(1))
    }

    /// What of the question's line is at `(x, y)`: `None` off the line, and
    /// on it the key there, if any — the ONE answer the lit key and the click
    /// both read, off the pieces as they were drawn.
    fn question_at(&self, x: f64, y: f64) -> Option<Option<crate::chrome::Mark>> {
        if self.question.is_empty() || x < 0.0 || y < 0.0 {
            return None;
        }
        let (width, height) = cell();
        let (columns, lines) = self.context_of();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (column, line) = (
            (x / f64::from(width)) as u16,
            (y / f64::from(height)) as u16,
        );
        question_mark_at(
            &self.question,
            (column, line),
            tokens::space::CONTENT_INSETY + lines,
            columns,
        )
    }

    /// The question on the door's line as pieces this wide — none when
    /// nothing is being asked.
    fn question_of(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        let Some(line) = ui.asking.as_ref() else {
            return Vec::new();
        };
        let (cell_width_px, _) = cell();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let columns = ((self.width as f32 / cell_width_px) as u32)
            .saturating_sub(u32::from(tokens::space::CONTENT_INSET) * 2)
            .clamp(1, u32::from(u16::MAX)) as u16;
        crate::door::list::question_pieces(line, &ui.texts, columns)
    }

    /// A press on the question's line: its two keys, as the keyboard presses
    /// them — `RET` answers with what is typed, `ESC` lets it go. `true` when
    /// the press was the line's.
    ///
    /// Read off the SAME layout the drawing used (`chrome::hint_marks`, over
    /// the one `laid`), so the key lit is the key the click takes.
    fn question_pressed(&mut self, ui: &mut Ui, x: f64, y: f64) -> bool {
        let Some(mark) = self.question_at(x, y) else {
            return false;
        };
        let Some(crate::chrome::Act::Key(key)) = mark.and_then(|m| m.press) else {
            return true;
        };
        let Some(asked) = ui.asking.take() else {
            return true;
        };
        if ronoctua_keymap::Key::parse("RET").is_ok_and(|k| k == key) {
            self.answered(ui, asked.id, &asked.text);
        } else {
            self.unanswered(ui, asked.id);
        }
        true
    }

    /// The door's message, as the lines it is drawn on at this width — at most
    /// half the window, so the component always keeps the other half.
    fn said_of(&self, message: Option<&str>) -> Vec<String> {
        let Some(message) = message.filter(|m| !m.is_empty()) else {
            return Vec::new();
        };
        let (cell_width_px, cell_height_px) = cell();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let columns = ((self.width as f32 / cell_width_px) as u32)
            .saturating_sub(u32::from(tokens::space::CONTENT_INSET) * 2)
            .clamp(1, u32::from(u16::MAX)) as u16;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let lines = ((self.height as f32 / cell_height_px) as usize).max(2);
        let mut out = crate::chrome::note_lines(message, columns);
        out.truncate(lines / 2);
        out
    }

    /// Each scale's heights, in cells.
    ///
    /// Computed from the SAME tokens `scale_in_px` uses: if the two calculations
    /// diverge, the buffer reserves one number of cells and the run occupies
    /// another — which was exactly the clipped-glyph defect.
    fn heights() -> ronoctua_protocol::Heights {
        crate::paint::heights()
    }

    /// Asks for a frame. It is drawn when the event queue empties.
    ///
    /// See the `owed` field for why nothing here presents directly.
    const fn ask_paint(&mut self) {
        self.owed = true;
    }

    /// Asks the component for a new frame and keeps it, without drawing.
    ///
    fn re_render(&mut self) {
        let (columns, lines) = self.context_of();
        match self.instance.render(
            columns,
            lines,
            Self::heights(),
            ronoctua_shell_kit::header::hide_header(),
        ) {
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
        let Some(state) = self.states.get(self.now()) else {
            return;
        };
        let order = crate::door::state_order(std::process::id(), &state.id, state.tone);
        if let Err(e) = crate::ipc::send_it(&order) {
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
                ronoctua_protocol::Effect::Redraw => self.ask_paint(),
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
                // THE MODE AND A UNIT'S SETTINGS, behind the same capability:
                // what they reach is the daemon between every keyboard and
                // everything else.
                ronoctua_protocol::Effect::KeyboardMode(_)
                | ronoctua_protocol::Effect::UnitChange(_)
                    if !self
                        .capabilities
                        .contains(&ronoctua_protocol::Capability::Keyboard) =>
                {
                    tracing::error!(
                        capability = ?ronoctua_protocol::Capability::Keyboard,
                        "keyboard change refused: the buffer did not declare the capability"
                    );
                }
                ronoctua_protocol::Effect::KeyboardMode(to) => self.switch_mode(to),
                ronoctua_protocol::Effect::UnitChange(change) => self.unit_change(&change),
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
                            // AND IT REPAINTS, because a state is a SCREEN.
                            //
                            // This host repaints only on `Effect::Redraw`, and
                            // that is right for everything else — an empty
                            // effect list means nothing changed. But a state
                            // change is not nothing: the modeline names the
                            // screen, so announcing one without drawing it puts
                            // the name of a page on the chrome while the page
                            // underneath is still the last one.
                            //
                            // The owner read exactly that: `RET` on a task
                            // flipped the modeline to `task` and left the home
                            // page on screen until an arrow key — which
                            // repaints unconditionally — brought the right page
                            // up, already scrolled one row. The brain's guest
                            // had forgotten its `Redraw`; the guest is fixed
                            // too, and this is the half that keeps the NEXT
                            // buffer from falling in the same hole.
                            //
                            // No frame is wasted: the guard is `n != self.now()`
                            // already, so a buffer re-announcing the state it is
                            // on costs nothing.
                            self.ask_paint();
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
            // OPENING WHAT AN INSTALL MADE starts from this window, as the
            // software window starts it (`Ui::launch_entry`): the entry found
            // in the launcher's catalogue and started as the footer starts it,
            // and a refusal said here — never on the footer's line.
            R::LaunchJob => match self.queue.as_ref().and_then(|q| q.opening(demand.target)) {
                Some(desktop) => self.to_launch.push(desktop.to_owned()),
                None => tracing::warn!(id = demand.target, "a launch for a row that opens nothing"),
            },
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
            // AND WHAT THEY SAY is what the footer's line says for the same
            // two answers: both doors do the same.
            R::Proceed => self.put_brain(
                ronoctua_brain::Ask::Proceed,
                Some(crate::key::ECHO_BRAIN_PROCEEDING),
            ),
            R::Postpone => self.put_brain(
                ronoctua_brain::Ask::Postpone,
                Some(crate::key::ECHO_BRAIN_POSTPONED),
            ),
            // THE MODULES' SIX, each an index into a list this host sent.
            R::WatchScreen => self.watch_screen(demand.target),
            R::ActRow => self.act_row(demand.target, false),
            R::ActRowAlso => self.act_row(demand.target, true),
            R::ActPage => self.act_page(demand.target),
            R::DismissNotice | R::SnoozeNotice | R::ActNotice => {
                self.notice_demand(demand.what, demand.target);
            }
        }
    }

    /// Which module's page the component follows now: `0` stops, and any
    /// other target is a 1-based index into the tasks sent — asked of the
    /// brain at once, and again whenever its generation moves.
    fn watch_screen(&mut self, target: u64) {
        let Some(b) = self.brain.as_mut() else {
            return;
        };
        let module = index_of(target).and_then(|n| b.tasks.get(n)).cloned();
        if target != 0 && module.is_none() {
            tracing::warn!(target, "a page asked for a task that is not in the list");
        }
        b.watched = module;
        b.asked_at = None;
        b.shown_at = None;
        b.screen_rows.clear();
        self.ask_screen();
    }

    /// Asks the brain for the watched module's page, on a thread — when its
    /// generation moved since the last ask, and never otherwise.
    fn ask_screen(&mut self) {
        let Some(b) = self.brain.as_mut() else {
            return;
        };
        let Some(module) = b.watched.clone() else {
            return;
        };
        // A MODULE THE MIND NO LONGER LISTS has no page to ask for — and
        // asking on every mind for one would be a thread per beat.
        let Some(generation) = b.generation_of(&module) else {
            return;
        };
        if b.asked_at == Some(generation) {
            return;
        }
        let Some(send) = b.screens.clone() else {
            return;
        };
        b.asked_at = Some(generation);
        let built = std::thread::Builder::new()
            .name("brain-page".into())
            .spawn(move || {
                let asked = ronoctua_brain::Ask::Screen {
                    module: module.clone(),
                };
                let paged = match ronoctua_brain::client::ask(&asked) {
                    Ok(ronoctua_brain::Say::Screen {
                        module,
                        generation: at,
                        screen,
                    }) => Paged::Screen {
                        module,
                        generation: at,
                        screen,
                    },
                    // A MIND IS NOT A PAGE: the protocol drifted, and the page
                    // says the brain did not answer rather than draw nothing.
                    Ok(ronoctua_brain::Say::Mind { .. }) => {
                        tracing::warn!(%module, "the brain answered a page with a mind");
                        Paged::Not {
                            module,
                            asked: generation,
                            answer: crate::mind_wire::Answer::Unreachable,
                        }
                    }
                    other => {
                        let answer = crate::mind_wire::answer_from(other, None);
                        tracing::warn!(%module, ?answer, "no page from the brain");
                        Paged::Not {
                            module,
                            asked: generation,
                            answer,
                        }
                    }
                };
                let _ = send.send(paged);
            });
        if let Err(e) = built {
            tracing::error!(error = %e, "could not start the page's thread");
            // NOTHING WILL COME BACK, so nothing is waited for: the next mind
            // asks again.
            if let Some(b) = self.brain.as_mut() {
                b.asked_at = None;
            }
        }
    }

    /// A module's page arrived from its thread — or why none did. Delivered
    /// only while it is still the page followed, and never OLDER than the one
    /// delivered last: two asks can answer out of order, and a host holding
    /// one page's rows while the component draws another's would act on a row
    /// nobody sees.
    fn screen_arrived(&mut self, paged: Paged) {
        let Some(b) = self.brain.as_mut() else {
            return;
        };
        if let Some(over) = b.page_arrived(&paged, &self.texts) {
            self.deliver(&ronoctua_protocol::Input::ModuleScreen(Box::new(over)));
        }
    }

    /// The action on a row of the followed page, by its index across the
    /// page's sections — its press's, or its `also` (the second action).
    fn act_row(&mut self, target: u64, also: bool) {
        let Some(b) = self.brain.as_ref() else {
            return;
        };
        let (Some(module), Some((row, act, second))) = (
            b.watched.clone(),
            index_of(target).and_then(|n| b.screen_rows.get(n)).cloned(),
        ) else {
            tracing::warn!(target, "an action for a row that is not on the page");
            return;
        };
        let action = if also { second } else { act };
        if action.is_empty() {
            tracing::warn!(target, also, "a row with no such action was asked to act");
            return;
        }
        let acting = b.acting(&module, &action);
        self.act(acting, Some(row));
    }

    /// One of the followed module's own actions, by its index in its list.
    fn act_page(&mut self, target: u64) {
        let Some(b) = self.brain.as_ref() else {
            return;
        };
        let Some(module) = b.watched.clone() else {
            return;
        };
        let action = b
            .last
            .as_ref()
            .and_then(|m| m.task(&module))
            .and_then(|t| index_of(target).and_then(|n| t.actions.get(n)))
            .map(|a| a.id.clone());
        let Some(action) = action else {
            tracing::warn!(target, "an action that is not on the page");
            return;
        };
        let acting = b.acting(&module, &action);
        self.act(acting, None);
    }

    /// The notice at an index of the list sent: put away, put off, or its
    /// first control — each only when the notice offers it.
    fn notice_demand(&mut self, what: ronoctua_protocol::Request, target: u64) {
        use ronoctua_protocol::Request as R;
        let Some(b) = self.brain.as_ref() else {
            return;
        };
        let Some(n) = index_of(target).and_then(|i| b.notices.get(i)).cloned() else {
            tracing::warn!(target, "a request for a notice that is not in the list");
            return;
        };
        match what {
            R::DismissNotice if n.dismiss => self.put_brain(
                ronoctua_brain::Ask::Dismiss {
                    module: n.module,
                    notice: n.id,
                },
                Some(crate::key::ECHO_BRAIN_DISMISSED),
            ),
            R::SnoozeNotice => match n.snooze {
                Some(minutes) => self.put_brain(
                    ronoctua_brain::Ask::Snooze {
                        module: n.module,
                        notice: n.id,
                        minutes,
                    },
                    Some(crate::key::ECHO_BRAIN_SNOOZED),
                ),
                None => tracing::warn!(target, "a snooze the notice does not offer"),
            },
            R::ActNotice => match n.controls.first() {
                Some(action) => {
                    let acting = b.acting(&n.module, action);
                    self.act(acting, None);
                }
                None => tracing::warn!(target, "an action the notice does not offer"),
            },
            _ => tracing::warn!(target, ?what, "the notice does not offer that"),
        }
    }

    /// A module's action: sent at once, or — when it asks a question first —
    /// the question on this window's line, and sent with its answer.
    fn act(&mut self, acting: crate::brainlink::Acting, row: Option<String>) {
        if acting.asks.is_some() {
            // IN THE WORDS OF THE KEY ITS MODULE DECLARED, under the action's
            // command — the footer's line asks the same way.
            match crate::brainlink::question_of(&acting) {
                Some((id, asks)) => {
                    self.to_ask = Some(crate::line::Line::asking(id, asks));
                    if let Some(b) = self.brain.as_mut() {
                        b.pending = Some((acting, row));
                    }
                    self.owed = true;
                }
                None => {
                    tracing::warn!(command = %acting.command, "no question for the action");
                    self.to_say = Some(self.texts.t(crate::key::ECHO_BRAIN_NO_ACTION));
                    self.owed = true;
                }
            }
            return;
        }
        self.put_brain(
            act_ask(&acting, row, None),
            Some(crate::key::ECHO_BRAIN_TAKEN),
        );
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
        // WHAT IT DID IS ON THE PAGE — the row running, queued, stopped — so
        // only a refusal is said.
        self.put_brain(ask, None);
    }

    /// One preference the person changed, sent to the daemon KNOB BY KNOB.
    ///
    /// # Why one knob and not the whole record
    ///
    /// It was the whole record (`Ask::Tune`), built here on top of the
    /// preferences the daemon last reported — and built from the SHARED
    /// default whenever the task had no entry of its own (`Prefs::mine`). A
    /// watch switched off and on again from this page came back as a weekly
    /// run with a minute of countdown it never had. One knob is applied by the
    /// daemon, which is the one place that knows the module's own defaults,
    /// and a step's state needs no default here either: the daemon turns "on"
    /// into an exception against the step it declared.
    fn tune(&mut self, setting: &ronoctua_protocol::Setting) {
        use ronoctua_protocol::Knob;
        let Some(b) = self.brain.as_ref() else {
            return;
        };
        // THE TASK IS CHECKED against the list actually sent, exactly as an
        // index is: a preference written for an id nothing answers to would sit
        // in the person's file for ever, switching nothing off.
        if !b.tasks.contains(&setting.task) {
            tracing::warn!(task = %setting.task, "a preference for a task that is not in the list");
            return;
        }
        let knob = match &setting.knob {
            Knob::IntervalDays(days) => ronoctua_brain::Knob::IntervalDays(*days),
            Knob::PartOfDay(part) => {
                ronoctua_brain::Knob::PartOfDay(crate::mind_wire::part_back(*part))
            }
            Knob::Enabled(on) => ronoctua_brain::Knob::Enabled(*on),
            // `on` IS THE STATE THE SCREEN WANTS, and not a toggle — a toggle
            // sent twice by a key that repeated would land back where it
            // started with nobody the wiser.
            Knob::Step(switch) => ronoctua_brain::Knob::Step {
                id: switch.id.clone(),
                on: switch.on,
            },
            Knob::ResetSteps => ronoctua_brain::Knob::ResetSteps,
        };
        // THE PAGE SHOWS THE CHANGE, so only a refusal is said: a cadence a
        // task does not keep, a step it does not have.
        self.put_brain(
            ronoctua_brain::Ask::Set {
                task: setting.task.clone(),
                knob,
            },
            None,
        );
    }

    /// Says it to the daemon, and says so in the log when it cannot.
    ///
    /// `tell` and not `ask`, for the reason [`Self::tell_queue`] gives: none of
    /// these needs an answer, and waiting for one that never comes would freeze
    /// this window's loop. What comes back arrives on the watch stream, which
    /// is where the screen reads it anyway.
    /// Asks the brain for a change OFF THE LOOP (`brainlink::put`), and says what
    /// came of it on this window's line when it comes back — `taken` when it
    /// went well, and whatever the brain said when it did not.
    ///
    /// # Why every ask here waits for its answer
    ///
    /// Because the brain refuses — a notice no longer up, an action its module
    /// declined, a run of a module that only watches — and a request told
    /// into a socket nobody reads was a key that did nothing and said nothing.
    /// A refusal is a sentence, said in the door that asked.
    fn put_brain(&mut self, ask: ronoctua_brain::Ask, taken: Option<&'static str>) {
        let started = self
            .brain
            .as_ref()
            .and_then(|b| b.answers.clone())
            .is_some_and(|back| crate::brainlink::put(ask, taken, back));
        if !started {
            self.to_say = crate::mind_wire::Answer::Unreachable.sentence(&self.texts);
            self.owed = true;
        }
    }

    /// The brain answered something this window asked of it.
    fn brain_answered(&mut self, answer: &crate::mind_wire::Answer) {
        if let Some(said) = answer.sentence(&self.texts) {
            self.to_say = Some(said);
            self.owed = true;
        }
    }

    /// A fresh mind arrived from the daemon.
    ///
    /// The whole thing, every time, and never a delta — the reason is the
    /// daemon's own: a delta needs both sides to agree on what they last saw,
    /// and what is being agreed about here is when the machine will rebuild
    /// itself.
    fn mind_arrived(&mut self, mind: &ronoctua_brain::Mind) {
        let mut found = false;
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
            let mut over = crate::mind_wire::mind_of(mind, &b.zone, &self.texts);
            // WHAT THIS WINDOW WAS OPENED ABOUT GOES FIRST — a mark's click
            // names its module — the arrangement the disks and keyboards
            // buffers have: ordering is the host's business, and the index
            // gate below reads the order sent.
            if let Some(about) = self.file.as_ref().map(|p| p.to_string_lossy().into_owned())
                && let Some(at) = over.tasks.iter().position(|t| t.id == about)
            {
                let chosen = over.tasks.remove(at);
                over.tasks.insert(0, chosen);
                found = true;
            }
            b.tasks.clear();
            b.tasks.extend(over.tasks.iter().map(|t| t.id.clone()));
            b.notices = crate::mind_wire::ranked(mind)
                .into_iter()
                .map(|n| Noticed {
                    module: n.module.clone(),
                    id: n.what.id.to_string(),
                    controls: n.what.controls.iter().map(ToString::to_string).collect(),
                    dismiss: n.what.dismiss,
                    snooze: n.what.snooze.first().copied(),
                })
                .collect();
            over
        };
        self.deliver(&ronoctua_protocol::Input::Mind(Box::new(over)));
        // AND THE MODULE IT WAS OPENED ABOUT, on the first mind: its page,
        // which the component opens by the state it declared for a task's page
        // — and only when that module IS in the list: a page opened on the
        // first task in its place would be somebody else's page.
        // A MIND WITH NO MODULES YET is not the answer: it waits for one that
        // has them.
        let opens = self.brain.as_mut().is_some_and(|b| {
            if !b.about_owed || b.tasks.is_empty() {
                return false;
            }
            b.about_owed = false;
            if self.file.is_some() && !found {
                tracing::warn!(about = ?self.file, "the brain has no module by that name");
            }
            found
        });
        if opens {
            self.deliver(&ronoctua_protocol::Input::EnterState("task".to_owned()));
        }
        // A PAGE FOLLOWED WHOSE GENERATION MOVED is asked for again.
        self.ask_screen();
    }

    /// Lines of the daemon's own log arrived.
    ///
    /// The whole log this window holds goes to the component every time, for
    /// the reason the mind does: the buffer keeps nothing that can go out of
    /// step.
    fn lines_arrived(&mut self, lines: Vec<ronoctua_brain::tail::Line>) {
        let over = {
            let Some(b) = self.brain.as_mut() else {
                return;
            };
            b.took_lines(lines);
            crate::mind_wire::log_of(&b.log, &b.zone)
        };
        self.deliver(&ronoctua_protocol::Input::Log(over));
    }

    /// A fresh queue arrived from the unit.
    ///
    /// The whole list, every time, and never a delta: the buffer holds no
    /// state that could go out of step, and the list is short enough that
    /// sending it whole costs less than being wrong about it.
    fn queue_arrived(&mut self, jobs: &[ronoctua_transfers::Job]) {
        let queue = (self.queue_words)(jobs, &self.texts);
        if let Some(q) = self.queue.as_mut() {
            q.took(jobs);
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

    /// Tells the buffer whose keys come next, when that changed — the
    /// cursor-colour number of the seat whose keyboard spoke, as a hover's.
    /// It is what lets a buffer keep one selection per keyboard.
    fn say_whose_keys(&mut self) {
        let seat = self.key_seat;
        if self.typing_seat != Some(seat) {
            self.typing_seat = Some(seat);
            self.deliver(&ronoctua_protocol::Input::KeyboardSeat { seat });
        }
    }

    /// Reads the keyboards and what was decided, and sends the whole picture.
    ///
    /// # Why it reads the kernel every time
    ///
    /// `/proc/bus/input/devices` is three kilobytes and a parse. A keyboard
    /// arrives when somebody plugs it in and leaves when the battery dies;
    /// the watch on [`INPUT_DEVICES`] is what says so while the window is open,
    /// and every remap delivers again. Reading it at every delivery costs less
    /// than being wrong about which keyboards exist.
    fn keyboards_arrived(&mut self) {
        if self.keyboards.is_none() {
            return;
        }
        let seen = ronoctua_keyboards::seen().unwrap_or_else(|e| {
            tracing::warn!(error = %e, "the kernel did not say what keyboards there are");
            Vec::new()
        });
        let prefs = ronoctua_keyboards::store().read();
        let shown = ronoctua_pointers::shown_now();
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
                crate::key::KEYBOARD_TROUBLE_NO_PLACE.to_owned()
            },
            // WHICH KEY THE LEADER IS MADE OF, so the screen can say so before
            // somebody moves it. Every rendered file claims that key, and a
            // rule written after it wins — which is the one remap that costs
            // the key opening everything else.
            leader: ronoctua_keyboards::LEADER.0.to_owned(),
            mode: match prefs.mode {
                ronoctua_keyboards::Mode::Together => ronoctua_protocol::KeyboardMode::Together,
                ronoctua_keyboards::Mode::Independent => {
                    ronoctua_protocol::KeyboardMode::Independent
                }
            },
            units: units_of(&prefs, &shown),
            // EVERY CURSOR THERE IS, AND THE NEXT — as the screen numbers them.
            cursors: shown.labels(),
            layouts: ronoctua_keyboards::layouts()
                .into_iter()
                .map(|l| ronoctua_protocol::LayoutWord {
                    code: l.code,
                    name: l.name,
                    variants: l.variants,
                })
                .collect(),
        };
        if let Some(k) = self.keyboards.as_mut() {
            k.boards = over.boards.iter().map(|b| b.id.clone()).collect();
            k.keys = over.keys.iter().map(|w| w.keyd.clone()).collect();
            k.choices = over.choices.iter().map(|w| w.keyd.clone()).collect();
            k.units = over.units.iter().map(|u| u.identity.clone()).collect();
            k.cursors.clone_from(&over.cursors);
            k.shown = Some(shown);
            k.layouts = over
                .layouts
                .iter()
                .map(|l| (l.code.clone(), l.variants.clone()))
                .collect();
        }
        self.deliver(&ronoctua_protocol::Input::Keyboards(over));
    }

    /// Switches how the keyboards work together — AT ONCE, the owner's call.
    ///
    /// It asked first once, with a countdown that switched back on its own:
    /// the fear was a remapper that fails to start, which is a machine with no
    /// keyboard. That machine does not happen. The remapper's grab is released
    /// by the kernel the moment it dies, which hands every keyboard straight
    /// back to the compositor, and its unit calls keyd when it fails
    /// (`OnFailure=`) — so a question on every switch paid for nothing.
    fn switch_mode(&mut self, to: ronoctua_protocol::KeyboardMode) {
        let to = match to {
            ronoctua_protocol::KeyboardMode::Together => ronoctua_keyboards::Mode::Together,
            ronoctua_protocol::KeyboardMode::Independent => ronoctua_keyboards::Mode::Independent,
        };
        let store = ronoctua_keyboards::store();
        let Some(mut prefs) = store.parsed() else {
            self.trouble(crate::key::KEYBOARD_TROUBLE_FILE);
            return;
        };
        if prefs.mode == to {
            return;
        }
        if let Err(e) = crate::keyboard::switch_to(to) {
            tracing::error!(error = %e, "the keyboards did not switch");
            self.trouble(crate::key::KEYBOARD_TROUBLE_MODE);
            return;
        }
        prefs.mode = to;
        if let Err(e) = store.write(&prefs) {
            tracing::error!(error = %e, "the keyboards file was not written");
        }
        if let Some(k) = self.keyboards.as_mut() {
            k.trouble.clear();
        }
        self.keyboards_arrived();
    }

    /// One setting of one unit — checked against what was SENT, written, and
    /// handed to whoever reads it: the remapper (its rules) and the footer's
    /// seats (the file it watches).
    fn unit_change(&mut self, change: &ronoctua_protocol::UnitChange) {
        use ronoctua_protocol::UnitSetting;
        let Some(gate) = self.keyboards.as_ref() else {
            return;
        };
        if !gate.units.contains(&change.unit) {
            tracing::error!(
                unit = change.unit,
                "unit change refused: no such unit was sent"
            );
            return;
        }
        let gate_shown = gate.shown.clone();
        let offered = match &change.setting {
            UnitSetting::Cursor(n) => gate.cursors.contains(n),
            UnitSetting::Layout(l) => l.is_empty() || gate.layouts.iter().any(|(c, _)| c == l),
            UnitSetting::Variant(_) | UnitSetting::RepeatRate(_) | UnitSetting::RepeatDelay(_) => {
                true
            }
            UnitSetting::Leader(k) => gate.keys.contains(k),
            UnitSetting::Remap(k, v) => gate.keys.contains(k) && gate.choices.contains(v),
        };
        if !offered {
            tracing::error!(change = ?change, "unit change refused: not something that was offered");
            return;
        }
        let store = ronoctua_keyboards::store();
        let Some(mut prefs) = store.parsed() else {
            self.trouble(crate::key::KEYBOARD_TROUBLE_FILE);
            return;
        };
        let seen = ronoctua_keyboards::units().unwrap_or_default();
        let model_keys = seen
            .iter()
            .find(|u| u.identity() == change.unit)
            .map(|u| prefs.unit_keys(u))
            .unwrap_or_default();
        let name = seen
            .iter()
            .find(|u| u.identity() == change.unit)
            .map(|u| u.name.clone());
        let own = prefs.units.entry(change.unit.clone()).or_default();
        if let Some(name) = name {
            own.name = name;
        }
        match &change.setting {
            UnitSetting::Cursor(n) => {
                own.cursor = gate_shown.as_ref().and_then(|s| s.number(*n));
            }
            UnitSetting::Layout(l) => {
                own.layout = (!l.is_empty()).then(|| l.clone());
                own.variant = None;
            }
            UnitSetting::Variant(v) => own.variant = (!v.is_empty()).then(|| v.clone()),
            UnitSetting::Leader(k) => own.leader = Some(k.clone()),
            UnitSetting::RepeatRate(r) => own.repeat_rate = (*r > 0).then_some(*r),
            UnitSetting::RepeatDelay(d) => own.repeat_delay = (*d > 0).then_some(*d),
            UnitSetting::Remap(k, v) => {
                let keys = own.keys.get_or_insert(model_keys);
                if v.is_empty() {
                    keys.remove(k);
                } else {
                    keys.insert(k.clone(), v.clone());
                }
            }
        }
        if let Err(e) = store.write(&prefs) {
            tracing::error!(error = %e, "the keyboards file was not written");
            self.trouble(crate::key::KEYBOARD_TROUBLE_FILE);
            return;
        }
        if prefs.mode == ronoctua_keyboards::Mode::Independent
            && let Err(e) = crate::keyboard::render_keys()
        {
            tracing::error!(error = %e, "the remapper's rules were not written");
            self.trouble(crate::key::KEYBOARD_TROUBLE_FILE);
            return;
        }
        if let Some(k) = self.keyboards.as_mut() {
            k.trouble.clear();
        }
        self.keyboards_arrived();
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
            self.trouble(crate::key::KEYBOARD_TROUBLE_NO_PLACE);
            return;
        }
        let store = ronoctua_keyboards::store();
        // `parsed` AND NOT `read`: this writes the whole value back, and a
        // reader's default over a file that will not parse is how somebody
        // loses every keyboard they had configured. The rule is written out in
        // `ronoctua_store`.
        let Some(mut prefs) = store.parsed() else {
            tracing::error!("the keyboards file will not parse; refusing to write over it");
            self.trouble(crate::key::KEYBOARD_TROUBLE_FILE);
            return;
        };
        prefs.set(&what.device, &what.key, &what.becomes);
        let files = ronoctua_keyboards::render(&prefs);
        if let Err(e) = ronoctua_keyboards::install(ronoctua_keyboards::DIRECTORY, &files) {
            tracing::error!(error = %e, "keyd would not take the rules");
            self.trouble(crate::key::KEYBOARD_TROUBLE_REFUSED);
            return;
        }
        if let Err(e) = store.write(&prefs) {
            tracing::error!(error = %e, "the keyboards file was not written");
            self.trouble(crate::key::KEYBOARD_TROUBLE_FILE);
            return;
        }
        // AND THE DAEMON IS TOLD, through the polkit rule that already exists
        // for `keyboard-restore`: one verb, one unit, one user.
        if let Err(e) = crate::keyboard::restore_it() {
            tracing::error!(error = %e, "keyd did not restart");
            self.trouble(crate::key::KEYBOARD_TROUBLE_RESTART);
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
        self.format_with(plan, None);
    }

    /// The passphrase's two answers, for a plan that asked for encryption.
    ///
    /// The first is kept, the second compared with it; the plan is carried
    /// out only when they agree, and the buffer is told in words when they do
    /// not — a disk encrypted under a passphrase mistyped once is a disk
    /// nobody opens again.
    fn sealed(&mut self, id: &'static str, text: &str) -> bool {
        let Some(d) = self.disks.as_mut() else {
            return false;
        };
        let Some((plan, first)) = d.sealing.take() else {
            return false;
        };
        match (id, first) {
            (PASSPHRASE, None) => {
                // AN EMPTY ANSWER IS NO PASSPHRASE, and said as that: there
                // was no second one for it to differ from.
                if text.is_empty() {
                    let said = self.texts.t(crate::key::FORMAT_ENCRYPT_LET_GO);
                    self.deliver(&ronoctua_protocol::Input::Formatted(
                        ronoctua_protocol::Outcome {
                            done: false,
                            trouble: said,
                        },
                    ));
                    return true;
                }
                d.sealing = Some((plan, Some(ronoctua_consent::keys::Secret::new(text))));
                self.to_ask = Some(crate::line::Line::new(PASSPHRASE_AGAIN, true, None));
                self.owed = true;
                true
            }
            (PASSPHRASE_AGAIN, Some(first)) => {
                if first.expose() == text {
                    self.format_with(&plan, Some(first));
                } else {
                    let said = self.texts.t(crate::key::FORMAT_ENCRYPT_DIFFER);
                    self.deliver(&ronoctua_protocol::Input::Formatted(
                        ronoctua_protocol::Outcome {
                            done: false,
                            trouble: said,
                        },
                    ));
                }
                true
            }
            (_, first) => {
                d.sealing = Some((plan, first));
                false
            }
        }
    }

    /// [`Host::format`], with the passphrase an encrypted plan was answered
    /// with — or, for one that asked for encryption and has none yet, the
    /// question for it.
    fn format_with(
        &mut self,
        plan: &ronoctua_protocol::Plan,
        passphrase: Option<ronoctua_consent::keys::Secret>,
    ) {
        if plan.encrypt && passphrase.is_none() {
            if let Some(d) = self.disks.as_mut() {
                d.sealing = Some((plan.clone(), None));
                self.to_ask = Some(crate::line::Line::new(PASSPHRASE, true, None));
                self.owed = true;
            }
            return;
        }
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
                        // AND AN ENCRYPTED DISK OPEN ON IT: its container
                        // holds the partition, and udisks refuses to write
                        // under it until it is locked again.
                        .filter(|v| {
                            v.mounted()
                                || v.encrypted() == Some(ronoctua_disks::Encrypted::Unlocked)
                        })
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
                    &device,
                    filesystem,
                    &label,
                    whole,
                    &first,
                    &name,
                    passphrase.as_ref(),
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
                    Refusal::Busy => crate::key::FORMAT_REFUSED_BUSY,
                    Refusal::NeedsAdmin => crate::key::FORMAT_REFUSED_ADMIN,
                    _ => crate::key::FORMAT_REFUSED_OTHER,
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

impl Host {
    /// One key, from a press or from a REPEAT.
    ///
    /// SCTK delivers a repeat through the callback given to
    /// `get_keyboard_with_repeat`, which has no `Connection` and no
    /// `QueueHandle` to hand on — so the work cannot live in `press_key`, and
    /// while it did, `repeat_key` was dead code and holding a key did it
    /// once. The comment beside `repeat_key` said the opposite for a year.
    fn typed(&mut self, event: &KeyEvent) {
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
        //
        // UNLESS THE BUFFER BINDS `C-v` ITSELF — a local key `, v` is `C-v` in
        // this window (see below), and the buffer that declared it chose it.
        //
        // READ OFF THE LOOP: the program that copied answers when it likes,
        // and this window waited for it (`ronoctua_shell_door::pasting`).
        if self.mods.ctrl && event.keysym == Keysym::v && !binds_the_chord(&self.keymap, 'v') {
            self.paste_wanted = true;
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
            //
            // AND THE BUFFER'S GROUP IS NOT TYPED HERE. A buffer declares its
            // keys under `LOCAL_PREFIX` (`, r`), which is the group behind the
            // leader from the footer (`LEADER , r`); inside the buffer's own
            // window `C-r` IS `, r` — the host puts the group in front. It was
            // `C-,` and then `r`, a comma nobody should have to type, and
            // plain letters stay text, so a buffer may grow a field that is
            // typed into without a key of its own being in the way.
            if opens_sequence(self.mods.ctrl, self.mods.logo, self.pending.is_empty())
                && let Some(c) = event.keysym.key_char()
            {
                self.pending = ronoctua_protocol::LOCAL_PREFIX.chars().collect();
                if let Some(input) = self.local_key(c) {
                    self.deliver(&input);
                }
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

impl Host {
    /// The click becomes a CELL and goes to the component.
    ///
    /// The left button only, and only on the press. The others do nothing on
    /// purpose: a context menu would be new chrome (invariant 14), and the middle
    /// one pasting text into a glyph browser would be a search typed
    /// by accident.
    ///
    /// # The hover, and the argument that kept it out
    ///
    /// What used to be written here was that *"highlighting the item under the
    /// mouse would ask for a frame per pixel of movement, and here the
    /// highlight already has an owner — it is the cursor"*. Both halves were
    /// wrong, and the owner said so: a buffer with no hover cannot have an
    /// elaborate interface at all.
    ///
    /// The pointer moves in PIXELS and a buffer counts CELLS, so an event is
    /// sent only when the CELL changes — a traversal of a hundred-column
    /// window is a hundred events, not one per pixel of a thousand. That is
    /// what every other window in this shell has done since it was written
    /// (`files_window`, `network_window`, `power_window` all keep an
    /// `under_pointer` and aim on `Enter | Motion`); the buffer window was the
    /// one exception, and the exception was an assumption rather than a
    /// measurement.
    ///
    /// And the cursor is not the same highlight. The cursor is where the
    /// KEYBOARD is; the hover is where a click would land. Every view in the
    /// system draws both, in two different tones, because they answer two
    /// different questions.
    ///
    /// One pointer's frame, with its state already `here`.
    fn pointer_here(&mut self, ui: &mut Ui, hand: &mut Hand<PerSeat>, events: &[PointerEvent]) {
        self.many_pointers = ui.many_pointers;
        for e in events {
            // THE POINTER AT REST. `Leave` puts the highlight out; `Enter` and
            // `Motion` move it, and only across a cell boundary.
            match e.kind {
                PointerEventKind::Leave { .. } => {
                    if hand.extra.cell_under_pointer.take().is_some() {
                        let seat = ui.pointer_seat;
                        self.deliver(&ronoctua_protocol::Input::Unhover { seat });
                    }
                    if self.question_lit.take().is_some() {
                        ui.ask_paint();
                    }
                    // AND THE DRAG ENDS WITH THE POINTER. Wayland's implicit
                    // grab usually delivers the release, but a broken grab
                    // drops it — and a drag left standing makes the next
                    // ENTER, with no button held, throw the list to wherever
                    // the pointer came in. The five list windows all carry
                    // this line; see `audio_window.rs`.
                    hand.extra.dragging = None;
                    continue;
                }
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    // A BAR BEING DRAGGED TAKES THE MOTION, and the hover does
                    // not: the pointer is on the control, not on a row, and a
                    // highlight following it down the track would light every
                    // row the drag passed over.
                    if let Some(bar) = hand.extra.dragging {
                        let first = self.row_dragged_to(bar, e.position.1);
                        self.deliver(&ronoctua_protocol::Input::Scrolled {
                            column: bar.column,
                            line: bar.first_line,
                            first,
                        });
                        continue;
                    }
                    // THE QUESTION'S KEY UNDER THE POINTER LIGHTS, as every
                    // shortcut does: what the click would press.
                    let lit = self
                        .question_at(e.position.0, e.position.1)
                        .flatten()
                        .filter(|m| m.press.is_some())
                        .map(|m| m.id);
                    if lit != self.question_lit {
                        self.question_lit = lit;
                        ui.ask_paint();
                    }
                    // A POINTER ON A DRAWN BAR NEVER REACHES HERE: the door
                    // hands it over as leaving the rows (`crate::bars`).
                    let now = self.cell_at(e.position.0, e.position.1);
                    if now != hand.extra.cell_under_pointer {
                        hand.extra.cell_under_pointer = now;
                        // WHICH CURSOR, so a buffer lights one row per hand.
                        let seat = ui.pointer_seat;
                        match now {
                            Some((column, line)) => self
                                .deliver(&ronoctua_protocol::Input::Hover { column, line, seat }),
                            // THE MARGIN IS NOT THE WINDOW. A pointer resting
                            // on the inset is on no cell, and saying "cell
                            // zero" there would light the first row whenever
                            // somebody's hand crossed the border.
                            None => {
                                self.deliver(&ronoctua_protocol::Input::Unhover { seat });
                            }
                        }
                    }
                    continue;
                }
                _ => {}
            }
            // THE WHEEL, and it is the only pointer event besides the click
            // that crosses. It exists because `Command::Bar` does: a
            // scrollbar nobody can turn is a picture of a control.
            if let PointerEventKind::Axis {
                vertical,
                horizontal,
                ..
            } = &e.kind
            {
                // THE ONE ARITHMETIC, the same `ronoctua_shell_footer::surface` uses for the
                // footer and its five list windows. It keeps the leftover
                // fraction between events, which is what makes a slow gesture
                // move at all: five windows each wrote `(px / line) * 3`,
                // threw the fraction away, and the owner read the result off
                // the screen as a page that jumps.
                crate::wheel::wheel_arrived(&mut hand.extra.wheel_left, vertical, horizontal);
                let lines = crate::wheel::wheel_lines(&mut hand.extra.wheel_left);
                if lines == 0 {
                    continue;
                }
                // A SENTENCE THAT IS CUT TAKES THE WHEEL, and the host
                // answers it: a paragraph's total is the host's, because the
                // host is what wrapped it. See `paint::sentence_wheeled`.
                let (columns, _) = self.context_of();
                #[allow(clippy::cast_possible_truncation)]
                if let Some(said) = crate::paint::sentence_wheeled(
                    &self.list.commands,
                    e.position.0 as f32,
                    e.position.1 as f32,
                    self.height as f32,
                    &self.texts,
                    columns,
                    i16::try_from(lines).unwrap_or(if lines < 0 { i16::MIN } else { i16::MAX }),
                ) {
                    self.deliver(&ronoctua_protocol::Input::Scrolled {
                        column: said.column,
                        line: said.line,
                        first: said.first,
                    });
                    continue;
                }
                let Some((column, line)) = self.cell_at(e.position.0, e.position.1) else {
                    continue;
                };
                // CLAMPED RATHER THAN WRAPPED. A gesture that earned more
                // lines than an `i16` holds is a gesture nobody made, and the
                // buffer subtracts this from an offset — a wrap would send it
                // the other way.
                let lines =
                    i16::try_from(lines).unwrap_or(if lines < 0 { i16::MIN } else { i16::MAX });
                self.deliver(&ronoctua_protocol::Input::Wheel {
                    column,
                    line,
                    lines,
                });
                continue;
            }
            if let PointerEventKind::Release { button, .. } = e.kind
                && button == crate::door::LEFT_BUTTON
            {
                hand.extra.dragging = None;
                continue;
            }
            let PointerEventKind::Press { button, .. } = e.kind else {
                continue;
            };
            if button != crate::door::LEFT_BUTTON {
                continue;
            }
            // A PRESS ON THE QUESTION'S LINE is the line's: its keys are
            // controls, and the component's rows end above it.
            if self.question_pressed(ui, e.position.0, e.position.1) {
                continue;
            }
            // A PRESS ON A BAR IS A DRAG, and never a click on the row behind
            // it. It is asked FIRST for that reason: the bar is drawn over the
            // block's own rows, and whichever question is asked second answers
            // about something the pointer is not on.
            //
            // AND IT IS ASKED IN PIXELS, before the cell — the conversion to a
            // cell is what loses a bar drawn in the right margin, which is
            // every bar in the last column. See `Host::bar_under`.
            if let Some(bar) = self.bar_under(e.position.0, e.position.1) {
                hand.extra.dragging = Some(bar);
                let first = self.row_dragged_to(bar, e.position.1);
                tracing::debug!(
                    column = bar.column,
                    line = bar.first_line,
                    first,
                    "a bar grabbed"
                );
                // THE BAR'S OWN CELL, never the pointer's: what the component
                // does with this is route it to one of its lists, and the
                // pointer may be past the last column when it does it. The
                // motion below has always sent the bar's; the press sent the
                // pointer's, which is a second way to answer one question.
                self.deliver(&ronoctua_protocol::Input::Scrolled {
                    column: bar.column,
                    line: bar.first_line,
                    first,
                });
                continue;
            }
            let Some((column, line)) = self.cell_at(e.position.0, e.position.1) else {
                // A click on the margin. There is no content there, and sending cell 0
                // would make clicking the border select the first item.
                continue;
            };
            tracing::debug!(column, line, "click delivered to the component");
            self.deliver(&ronoctua_protocol::Input::Click { column, line });
        }
    }
}

impl Content for Host {
    type Hand = PerSeat;

    fn app_id(&self) -> &'static str {
        self.app_id
    }

    // THE TITLE AND THE `app_id` COME FROM THE DESCRIPTOR — see `host_it`.
    fn title(&self, _: &ronoctua_i18n::Texts) -> String {
        self.title.clone()
    }

    /// No margin of the door's: a buffer counts from its own inset
    /// (`space.content.inset`), and `context_of` is what knows it.
    fn margin(&self) -> u16 {
        0
    }

    /// `Esc` and the right `Ctrl` are the BUFFER's — it has no header line of
    /// the door's, and `Esc` is its key and its modifier (see [`Escape`]).
    fn header_keys(&self) -> bool {
        false
    }

    /// A BUFFER BINDS ITS OWN per-PID socket (`publish_keymap`), answering
    /// `keymap`, `states` and `command` — so the base door must NOT bind the
    /// same path (`Content::serves_own_socket`).
    fn serves_own_socket(&self) -> bool {
        true
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        self.mods = mods;
        self.key_seat = ui.keyboard_seat;
        // REPEATING IS RIGHT HERE — holding the arrow scrolls the list, holding
        // backspace erases the search — EXCEPT WHILE `Esc` IS DOWN, and that
        // covers both halves of the gesture. `Esc` repeating would re-enter
        // the press on a key that is already down; the arrow repeating would
        // spin the states at the keyboard's repeat rate, which is a rotation
        // nobody can stop on the one they wanted.
        if ui.repeating && (event.keysym == Keysym::Escape || self.escape.down) {
            return;
        }
        // THE DOOR'S MESSAGE WAS READ: the next key gives its lines back to
        // the component, as the next key clears every window's line.
        if ui.message.take().is_some() {
            ui.ask_paint();
        }
        self.say_whose_keys();
        self.typed(event);
        if std::mem::take(&mut self.paste_wanted) {
            ui.paste();
        }
        self.hand_over(ui);
    }

    /// The clipboard's text, read for `Ctrl+V` — the component receives it
    /// resolved, and never touches the clipboard itself (invariant 11).
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        if text.is_empty() {
            tracing::debug!("the clipboard is empty");
        } else {
            self.deliver(&ronoctua_protocol::Input::Paste(text.to_owned()));
        }
        self.hand_over(ui);
    }

    /// A BUFFER TYPES TEXT — an editor, a search — and there the Lock is
    /// capitals. Its chords are still read without it (`keys::as_read`).
    fn types_capitals(&self) -> bool {
        true
    }

    fn released(&mut self, ui: &mut Ui, event: &KeyEvent) {
        self.key_seat = ui.keyboard_seat;
        if self.escape.release(event.keysym) == EscapeAct::Tap {
            // `Cancel` ALONE, and the state follows from it: every component
            // walks back ONE LAYER per `Esc`, and jumping to the base would
            // have thrown away the filter typed to find a sheet. The modeline
            // stays truthful because the walk back reports itself through
            // `effect::state-changed`.
            self.deliver(&ronoctua_protocol::Input::Cancel);
        }
        self.hand_over(ui);
    }

    /// FOCUS ARRIVES — and whatever was held before it belongs to the OTHER
    /// window. `AGENTS.md` records this as a trap with a name: here it is
    /// `Esc`, and alt-tabbing in while it is down would leave this window
    /// choosing a state with every key vanishing.
    fn keyboard_entered(&mut self, _: &mut Ui) {
        self.escape = Escape::default();
    }

    /// Focus leaves, and so does the release this window will never see.
    fn keyboard_left(&mut self, ui: &mut Ui) {
        self.escape = Escape::default();
        // THAT SEAT'S KEYBOARD LEFT, and the buffer is told which: another
        // keyboard still typing here keeps its field.
        let seat = ui.keyboard_seat;
        if self.typing_seat == Some(seat) {
            self.typing_seat = None;
        }
        self.deliver(&ronoctua_protocol::Input::KeyboardLeft { seat });
        self.hand_over(ui);
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<PerSeat>, events: &[PointerEvent]) {
        // AND SO DOES A PRESS — after it lands, in the layout it was aimed at.
        let pressed = events
            .iter()
            .any(|e| matches!(e.kind, PointerEventKind::Press { .. }));
        self.pointer_here(ui, hand, events);
        if pressed && ui.message.take().is_some() {
            ui.ask_paint();
        }
        self.hand_over(ui);
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        self.draw(ui, canvas);
    }

    /// IT RE-RENDERS with the actual size. The first `render` used a guessed
    /// 80x24; without this the component would draw for a window that is not
    /// its own — few lines on a large screen, exactly what was seen in the
    /// glyph browser. It matters most HERE, of all the doors: this is the one
    /// whose content comes from another process, and it is told how many
    /// cells it has.
    fn resized(&mut self, ui: &mut Ui) {
        self.width = ui.width;
        self.height = ui.height;
        ui.ask_paint();
    }

    fn language_changed(&mut self, ui: &mut Ui) {
        self.texts = ui.texts.clone();
    }

    /// What was typed for a module's question: the action it was asked for is
    /// sent with it — and only that action, under the id it was asked.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        // THE PASSPHRASE OF AN ENCRYPTED FORMAT, which never reaches the
        // component: asked here, compared here, written into the call here.
        if self.sealed(id, text) {
            ui.ask_paint();
            return;
        }
        let pending = self.brain.as_mut().and_then(|b| b.pending.take());
        match pending {
            Some((acting, row)) if crate::brainlink::question_id(&acting.command) == Some(id) => {
                self.put_brain(
                    act_ask(&acting, row, Some(text)),
                    Some(crate::key::ECHO_BRAIN_TAKEN),
                );
            }
            other => {
                tracing::warn!(id, "an answer for a question nothing asked");
                if let Some(b) = self.brain.as_mut() {
                    b.pending = other;
                }
            }
        }
        ui.ask_paint();
    }

    /// The question was let go: nothing is sent.
    fn unanswered(&mut self, ui: &mut Ui, id: &'static str) {
        // AN ENCRYPTED FORMAT WHOSE PASSPHRASE WAS LET GO writes nothing, and
        // the buffer — waiting on a disk being written — is told so.
        if matches!(id, PASSPHRASE | PASSPHRASE_AGAIN)
            && let Some(d) = self.disks.as_mut()
            && d.sealing.take().is_some()
        {
            let said = self.texts.t(crate::key::FORMAT_ENCRYPT_LET_GO);
            self.deliver(&ronoctua_protocol::Input::Formatted(
                ronoctua_protocol::Outcome {
                    done: false,
                    trouble: said,
                },
            ));
            ui.ask_paint();
            return;
        }
        if let Some(b) = self.brain.as_mut()
            && b.pending
                .as_ref()
                .is_some_and(|(a, _)| crate::brainlink::question_id(&a.command) == Some(id))
        {
            b.pending = None;
        }
        ui.ask_paint();
    }

    /// What the component asked for between frames — a redraw, a close —
    /// reaches the door here, once per turn of the loop, from whatever
    /// delivered it: a key, a daemon's answer, the clock.
    fn each_turn(&mut self, ui: &mut Ui) {
        self.hand_over(ui);
    }
}

impl Host {
    /// Hands the door what the component asked for: a frame, or the end.
    fn hand_over(&mut self, ui: &mut Ui) {
        if std::mem::take(&mut self.owed) {
            ui.ask_paint();
        }
        for desktop in std::mem::take(&mut self.to_launch) {
            ui.launch_entry(&desktop);
        }
        // A MODULE'S QUESTION, on this window's own line — the door routes
        // every key to it while it stands, and the answer comes back to
        // `answered`.
        if let Some(line) = self.to_ask.take() {
            ui.ask(line);
        }
        // AND WHAT THE BRAIN ANSWERED, on the line the door's messages take —
        // once the line is free: a question standing there is not unseated,
        // and the answer waits for it to be answered or let go.
        if ui.asking.is_none()
            && let Some(said) = self.to_say.take()
        {
            ui.say(said);
            ui.ask_paint();
        }
        if self.quit {
            ui.quit = true;
        }
    }
}

/// What of a question's line drawn on `row` is at the cell `(column, line)`:
/// `None` off the line, and on it the key there, if any — read off the SAME
/// marks the drawing lays (`chrome::hint_marks`), so the key the pointer
/// lights is the key its click presses.
fn question_mark_at(
    question: &[crate::chrome::Piece],
    (column, line): (u16, u16),
    row: u16,
    columns: u16,
) -> Option<Option<crate::chrome::Mark>> {
    if question.is_empty() || line != row {
        return None;
    }
    Some(
        crate::chrome::hint_marks(question, tokens::space::CONTENT_INSET, row, columns)
            .into_iter()
            .find(|m| column >= m.at && column < m.at + m.cells),
    )
}

/// A 1-based index into a list the host sent, as a position: `0` is "none",
/// and `checked_sub` — not `- 1` — because a zero is an ordinary thing for a
/// buffer to send.
fn index_of(target: u64) -> Option<usize> {
    target.checked_sub(1).and_then(|n| usize::try_from(n).ok())
}

/// The ask that does a module's action, on the row it was asked on and with
/// what was typed for it.
fn act_ask(
    acting: &crate::brainlink::Acting,
    row: Option<String>,
    answer: Option<&str>,
) -> ronoctua_brain::Ask {
    ronoctua_brain::Ask::Act {
        module: acting.module.clone(),
        action: acting.action.clone(),
        row,
        answer: answer.map(ToOwned::to_owned),
    }
}

/// The door's message, one span per line, on the lines under the
/// component's `lines` — in the register every window's line says a sentence
/// in (`chrome::Piece::said`).
fn said_spans(said: &[String], lines: u16) -> Vec<ronoctua_render::Span> {
    let pieces: Vec<Vec<crate::chrome::Piece>> = said
        .iter()
        .map(|l| vec![crate::chrome::Piece::said(l.clone())])
        .collect();
    pieces
        .iter()
        .zip(0u16..)
        .flat_map(|(line, n)| {
            let room = ronoctua_render::layout::width(&line[0].text);
            crate::chrome::hint_spans(
                line,
                ronoctua_render::Size::Px(tokens::text::size()),
                tokens::space::CONTENT_INSET,
                tokens::space::CONTENT_INSETY + lines + n,
                room,
            )
        })
        .collect()
}

/// Whether the buffer binds `C-<c>` — a local key whose first letter after
/// the group is `c` (`, c`, or `, c x` that it opens).
fn binds_the_chord(keymap: &[ronoctua_protocol::Binding], c: char) -> bool {
    let first = c.to_string();
    keymap
        .iter()
        .any(|b| b.keys.split_whitespace().nth(1) == Some(first.as_str()))
}

/// May a key OPEN a buffer sequence?
///
/// Pulled out so the rule can be tested without a compositor. It is ONE line,
/// and it is the line that decides whether the person can write the letter `m`
/// in a buffer whose keymap starts with `m`.
const fn opens_sequence(ctrl: bool, logo: bool, idle: bool) -> bool {
    // `logo` excluded and not ignored: `Super` belongs to the compositor, and a
    // buffer answering to it would steal a binding from the compositor.
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
    /// FAILS IF: a buffer's drawing is rasterised at its LOGICAL side on a
    /// screen of scale 2 — stretched, beside letters drawn at the scale.
    #[test]
    fn a_drawing_is_rasterised_at_the_screens_pixels() {
        let at_one = super::svg_side_px(4, 2, (10.8, 26.1), 1);
        let at_two = super::svg_side_px(4, 2, (10.8, 26.1), 2);
        assert_eq!(at_one, 43, "four cells of 10.8 px, the tighter side");
        assert_eq!(at_two, 2 * at_one);
        assert_eq!(
            super::svg_side_px(4, 2, (10.8, 26.1), 0),
            at_one,
            "scale 0 is 1"
        );
    }

    use super::{Brain, Escape, EscapeAct};
    use crate::paint::{bar_grabbed, cell, quads_of};
    use smithay_client_toolkit::seat::keyboard::Keysym;

    fn lines(from: u64, many: u64) -> Vec<ronoctua_brain::tail::Line> {
        (from..from + many)
            .map(|seq| ronoctua_brain::tail::Line {
                seq,
                at_ms: 1_700_000_000_000 + i64::try_from(seq).unwrap_or(0) * 1000,
                level: ronoctua_brain::tail::Level::Info,
                text: format!("line {seq}"),
            })
            .collect()
    }

    /// FAILS IF: an index a component hands back is read as anything but
    /// 1-based with `0` for none, or an action is asked for with anything but
    /// its module, its row and what was typed — or a module's action is found
    /// with a question it did not declare.
    #[test]
    fn a_module_action_is_asked_as_the_mind_declared_it() {
        assert_eq!(super::index_of(0), None);
        assert_eq!(super::index_of(1), Some(0));
        assert_eq!(super::index_of(3), Some(2));
        let mut b = Brain::fresh();
        let mut mind = ronoctua_brain::Mind::default();
        mind.tasks.push(ronoctua_brain::TaskState {
            kind: ronoctua_brain::Kind::Watch,
            id: "fixture".to_owned(),
            standing: ronoctua_brain::Standing::Idle,
            enabled: true,
            interval_days: 7,
            part_of_day: ronoctua_brain::PartOfDay::Any,
            next: 0,
            hold: None,
            why_not: String::new(),
            history: Vec::new(),
            last: None,
            steps: Vec::new(),
            calendar: false,
            name_key: String::new(),
            does_key: String::new(),
            why_key: String::new(),
            screen_generation: 7,
            actions: vec![ronoctua_brain::mind::ActionState {
                id: "when".to_owned(),
                label: "cmd-fixture-when".to_owned(),
                row: true,
                asks: Some("ask-fixture-when".to_owned()),
            }],
        });
        b.last = Some(mind);
        assert_eq!(b.generation_of("fixture"), Some(7));
        let acting = b.acting("fixture", "when");
        assert_eq!(acting.command, "fixture-when");
        assert_eq!(acting.asks.as_deref(), Some("ask-fixture-when"));
        assert_eq!(b.acting("fixture", "other").asks, None);
        assert_eq!(
            super::act_ask(&acting, Some("r-1".to_owned()), Some("tomorrow at nine")),
            ronoctua_brain::Ask::Act {
                module: "fixture".to_owned(),
                action: "when".to_owned(),
                row: Some("r-1".to_owned()),
                answer: Some("tomorrow at nine".to_owned()),
            }
        );
    }

    /// FAILS IF: the question's key under the pointer is not the one lit, a
    /// key is lit off the question's line, or a key lit is not the key its
    /// click presses.
    #[test]
    fn the_questions_key_under_the_pointer_is_lit_and_pressed() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let line = crate::line::Line::asking("fixture-when", "line-answer");
        let pieces = crate::door::list::question_pieces(&line, &texts, 80);
        let row = 9;
        let marks = crate::chrome::hint_marks(&pieces, tokens::space::CONTENT_INSET, row, 80);
        // THE TWO KEYS, each drawn as its notation and its word.
        let mut ids: Vec<&str> = marks.iter().map(|m| m.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), 2, "the question's two keys are not both marked");
        for m in &marks {
            let found = super::question_mark_at(&pieces, (m.at, row), row, 80)
                .flatten()
                .expect("nothing under the pointer on a key");
            assert_eq!((found.id, found.press), (m.id, m.press));
            let lit = crate::chrome::lit(&pieces, Some(found.id));
            let changed: Vec<_> = pieces
                .iter()
                .zip(&lit)
                .filter(|(was, now)| was.register != now.register)
                .map(|(_, now)| (now.id, now.register))
                .collect();
            assert!(
                changed.iter().all(|(id, _)| *id == Some(m.id)),
                "another key was lit: {changed:?}"
            );
            assert!(
                changed
                    .iter()
                    .any(|(_, r)| *r == crate::chrome::Register::KeyLit),
                "the key under the pointer was not lit"
            );
        }
        assert!(super::question_mark_at(&pieces, (marks[0].at, row + 1), row, 80).is_none());
        assert!(super::question_mark_at(&[], (0, row), row, 80).is_none());
    }

    /// A page of the module `fixture` at `generation`, with one row.
    fn a_page(generation: u64) -> Paged {
        use ronoctua_brain::module::{Cell, Row, Screen, Section, Tone};
        Paged::Screen {
            module: "fixture".to_owned(),
            generation,
            screen: Screen {
                sections: vec![Section {
                    heading: "fixture-heading".into(),
                    rows: vec![Row {
                        id: format!("row-{generation}"),
                        label: Cell::Key("fixture-row".into()),
                        value: Cell::Count(u64::from(generation > 0)),
                        note: Cell::Empty,
                        tone: Tone::Quiet,
                        act: Some("forget".into()),
                        also: None,
                    }],
                }],
            },
        }
    }

    /// FAILS IF: a module's page answered OUT OF ORDER is delivered — the
    /// host would hold one page's rows while the component draws another's,
    /// and `act-row` would name a row nobody sees — or a page that did not
    /// come is delivered as nothing instead of as a sentence, or an ask
    /// nobody answered is never asked again.
    #[test]
    fn a_module_page_is_delivered_in_order_and_a_refusal_is_a_sentence() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut b = Brain::fresh();
        b.watched = Some("fixture".to_owned());
        // THE NEWER ANSWERS FIRST; THE OLDER, LATE, IS DROPPED.
        let newer = b.page_arrived(&a_page(5), &texts).expect("the newer page");
        assert_eq!(newer.generation, 5);
        assert_eq!(
            b.screen_rows,
            vec![("row-5".to_owned(), "forget".to_owned(), String::new())]
        );
        assert!(b.page_arrived(&a_page(4), &texts).is_none());
        assert_eq!(
            b.screen_rows[0].0, "row-5",
            "the late page's rows were kept"
        );
        // ANOTHER MODULE'S PAGE is not the page followed.
        b.watched = Some("other".to_owned());
        assert!(b.page_arrived(&a_page(9), &texts).is_none());
        b.watched = Some("fixture".to_owned());

        // A REFUSAL IS THE BRAIN'S SENTENCE, in the rows' place — and it is
        // kept: the same page asked again is refused again.
        b.asked_at = Some(6);
        let refused = b
            .page_arrived(
                &Paged::Not {
                    module: "fixture".to_owned(),
                    asked: 6,
                    answer: crate::mind_wire::Answer::Refused("no page today".to_owned()),
                },
                &texts,
            )
            .expect("the refusal");
        assert!(refused.sections.is_empty() && refused.refused.contains("no page today"));
        assert!(
            b.screen_rows.is_empty(),
            "the refused page left rows to act on"
        );
        assert_eq!(b.asked_at, Some(6));
        // NOBODY ANSWERED: said, and forgotten, so the next mind asks again.
        b.asked_at = Some(7);
        let silent = b
            .page_arrived(
                &Paged::Not {
                    module: "fixture".to_owned(),
                    asked: 7,
                    answer: crate::mind_wire::Answer::Unreachable,
                },
                &texts,
            )
            .expect("the silence");
        assert_eq!(silent.refused, texts.t(crate::key::ECHO_BRAIN_UNREACHABLE));
        assert_eq!(b.asked_at, None, "an unanswered ask is never asked again");
    }

    /// THE RING SENT WHOLE AGAIN REPLACES WHAT WAS HELD — a reconnection, or a
    /// daemon that restarted and numbers from one — and a batch that follows
    /// is appended. Held twice, every line would be on the page twice.
    #[test]
    fn a_whole_ring_replaces_and_a_batch_appends() {
        let mut b = Brain::fresh();
        b.took_lines(lines(1, 10));
        b.took_lines(lines(11, 2));
        assert_eq!(b.log.len(), 12);
        b.took_lines(lines(1, 5));
        assert_eq!(b.log.len(), 5, "the ring was appended to what was held");
        let most = u64::try_from(ronoctua_brain::tail::KEPT).unwrap_or(u64::MAX);
        b.took_lines(lines(6, most));
        assert_eq!(
            b.log.len(),
            ronoctua_brain::tail::KEPT,
            "more than the ring was held"
        );
    }

    /// THE TIME IS THE MACHINE'S, in seconds, and the words cross as written.
    #[test]
    fn a_log_line_crosses_with_its_clock() {
        let one = lines(7, 1);
        let over = crate::mind_wire::log_of(&one, &ronoctua_files::when::Zone::utc());
        assert_eq!(over[0].seq, 7);
        // 1 700 000 007 s is 22:13:27 UTC.
        assert_eq!(over[0].at_text, "22:13:27");
        assert_eq!(over[0].text, "line 7");
    }

    /// A surface with a RESIDUE on both axes, which is every real window: a
    /// compositor hands out pixels and the grid is `floor` of them.
    fn a_surface() -> (f32, f32, u16) {
        let (cell_w, line_h) = cell();
        let width = cell_w.mul_add(100.0, cell_w * 0.4);
        let height = line_h.mul_add(24.0, line_h * 0.37);
        // The last column the COMPONENT was told about, derived the way
        // `context_of` derives it: the window's cells less the margin on both
        // sides. This is the column every list bar in a buffer sits in.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let across = (width / cell_w) as u16 - tokens::space::CONTENT_INSET * 2;
        (width, height, across - 1)
    }

    /// One bar over eight rows of a list of forty, and one over a list that
    /// fits — whose bar is never drawn, and may not be grabbed either.
    fn a_bar(column: u16) -> ronoctua_protocol::DisplayList {
        let mut d = ronoctua_protocol::DisplayList::fresh_one();
        d.push(ronoctua_protocol::Command::Bar {
            line: 16,
            lines: 8,
            column,
            total: 40,
            shown: 8,
            offset: 0,
            color: ronoctua_protocol::Color::name("rule-raised"),
        });
        d.push(ronoctua_protocol::Command::Bar {
            line: 2,
            lines: 13,
            column,
            total: 3,
            shown: 13,
            offset: 0,
            color: ronoctua_protocol::Color::name("rule-default"),
        });
        d
    }

    /// FAILS IF: a pixel of the thumb the host PAINTED is not a pixel the host
    /// answers a press on.
    ///
    /// # The law this is here to keep
    ///
    /// A control is grabbed where it is drawn. This one was not, and could not
    /// be: the thumb of a bar in the last column is painted past the last cell
    /// — deliberately, so it touches the border — and the press was resolved
    /// into a CELL, which in that strip names a column the component never
    /// drew in. `bar.column == pointer.column` could not match. Measured on the
    /// owner's screen: column 81 drawn, column 83 reported, and every drag in
    /// every buffer fell through to the row behind the bar.
    ///
    /// Nothing smaller than this catches it. A unit test of the placement
    /// passes, a unit test of the hit test passes, and the defect is the
    /// distance between them — so the assertion has to cross from one to the
    /// other, which is what reading the painted quad does.
    #[test]
    fn every_pixel_of_a_drawn_bar_can_be_grabbed() {
        let (width, height, last) = a_surface();
        // THE LAST COLUMN AND A MIDDLE ONE. The first is what every buffer
        // draws; the second is the manager's, down a section, and it must not
        // be pushed to the edge.
        for column in [last, last / 2] {
            let list = a_bar(column);
            let texts = ronoctua_i18n::Texts::fresh("pt-BR").unwrap();
            let quads = quads_of(&list, width, height, &texts, 100);
            assert_eq!(quads.len(), 1, "column {column}: one bar is drawn");
            let thumb = quads[0];
            // EVERY PIXEL OF IT, corners included.
            let mut x = thumb.x;
            while x < thumb.x + thumb.width {
                let mut y = thumb.y;
                while y < thumb.y + thumb.height {
                    let got = bar_grabbed(&list.commands, x, y, width, height, &texts, 100);
                    assert!(
                        got.is_some_and(|g| g.column == column),
                        "column {column}: the pointer at ({x}, {y}) is on the thumb and grabs nothing"
                    );
                    y += 1.0;
                }
                x += 1.0;
            }
            // AND THE TRACK AROUND IT, because a one-cell thumb is a target
            // nobody can hit: pressing the track jumps there, as every
            // scrollbar does.
            let grab = bar_grabbed(&list.commands, thumb.x, thumb.y, width, height, &texts, 100)
                .expect("the thumb is on the track");
            assert_eq!(
                grab.row_at(grab.y),
                0,
                "the top of the track is the first row"
            );
            assert_eq!(
                grab.row_at(grab.y + grab.height),
                40 - 8,
                "the bottom of the track is the last window"
            );
            // A CELL TO THE LEFT IS THE PAGE, not the bar: a window may have
            // two lists side by side, and a press on one may not move the
            // other.
            let (cell_w, line_h) = cell();
            assert!(
                bar_grabbed(
                    &list.commands,
                    grab.x - cell_w,
                    thumb.y,
                    width,
                    height,
                    &texts,
                    100
                )
                .is_none(),
                "column {column}: the cell beside the bar grabbed it"
            );
            // AND ABOVE AND BELOW THE BAND IS THE PAGE TOO.
            assert!(
                bar_grabbed(
                    &list.commands,
                    thumb.x,
                    grab.y - line_h,
                    width,
                    height,
                    &texts,
                    100
                )
                .is_none(),
                "column {column}: the row above the band grabbed it"
            );
            assert!(
                bar_grabbed(
                    &list.commands,
                    thumb.x,
                    grab.y + grab.height + 1.0,
                    width,
                    height,
                    &texts,
                    100
                )
                .is_none(),
                "column {column}: the row below the band grabbed it"
            );
            // AND A LIST THAT FITS HAS NO BAR AT ALL. Row 2 is inside the
            // second command's band; `Scroll::cut` is the one place that
            // decides a bar exists, and a control that does nothing may not be
            // grabbed either.
            assert!(
                bar_grabbed(
                    &list.commands,
                    thumb.x,
                    crate::paint::Rows::of(&list.commands, height).top(2),
                    width,
                    height,
                    &texts,
                    100
                )
                .is_none(),
                "column {column}: a list that fits was grabbed"
            );
        }
    }

    /// FAILS IF: the back button — a click, which the door now gives as a
    /// press AND a release — stops being a tap of `Esc` here, or a press alone
    /// is left holding `Esc` for the key after it (the defect: the button did
    /// nothing, and the next arrow rotated the buffer's state).
    #[test]
    fn a_click_on_escape_is_a_press_and_a_release() {
        let mut e = Escape::default();
        e.press(Keysym::Escape);
        assert_eq!(
            e.press(Keysym::Down),
            EscapeAct::Rotate(true),
            "a press alone holds"
        );
        let mut e = Escape::default();
        e.press(Keysym::Escape);
        assert_eq!(e.release(Keysym::Escape), EscapeAct::Tap);
        assert_eq!(e.press(Keysym::Down), EscapeAct::NotMine);
    }

    /// FAILS IF: a `launch-job` stops finding the entry the finished install
    /// made — the one thing about it the buffer is not told — or finds one for
    /// a row that made none.
    #[test]
    fn a_launch_opens_the_entry_the_install_made() {
        let mut installed = ronoctua_transfers::Job::fresh_one(
            7,
            ronoctua_transfers::Engine::Software,
            ronoctua_transfers::Work::Software {
                order: Box::new(ronoctua_software::Order::Install {
                    key: ronoctua_software::AppKey::Flatpak {
                        id: "org.gimp.GIMP".into(),
                    },
                    consent: ronoctua_software::order::Consent::default(),
                }),
                label: "GIMP".into(),
            },
        );
        installed.stage = ronoctua_transfers::Stage::Done;
        installed.outcome = Some(ronoctua_software::Outcome {
            key: None,
            desktop_ids: vec!["org.gimp.GIMP.desktop".into()],
            amount: None,
            rehearsed: false,
        });
        let copy = ronoctua_transfers::Job::fresh_one(
            1,
            ronoctua_transfers::Engine::Local,
            ronoctua_transfers::Work::Measure {
                path: std::path::PathBuf::from("/a"),
            },
        );
        let mut q = super::Queue::fresh();
        q.took(&[installed, copy]);
        assert_eq!(q.known, [7, 1]);
        assert_eq!(q.opening(7), Some("org.gimp.GIMP.desktop"));
        assert_eq!(q.opening(1), None);
    }

    /// FAILS IF: the door's message — a footer that is not there, a paste
    /// that failed, a program that could not start — is drawn nowhere in a
    /// buffer's window: it goes on the lines under the component's, one per
    /// wrapped line, in the register a window's line says a sentence in.
    #[test]
    fn the_doors_message_is_drawn_under_the_component() {
        let said = vec![
            "No footer is listening.".to_owned(),
            "Try again.".to_owned(),
        ];
        let spans = super::said_spans(&said, 20);
        let lines: Vec<u16> = spans.iter().map(|s| s.line).collect();
        assert_eq!(
            lines,
            [
                ronoctua_tokens::space::CONTENT_INSETY + 20,
                ronoctua_tokens::space::CONTENT_INSETY + 21
            ]
        );
        let text: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, "No footer is listening.Try again.");
        assert!(
            spans
                .iter()
                .all(|s| !s.foot && s.column == ronoctua_tokens::space::CONTENT_INSET)
        );
        assert!(super::said_spans(&[], 20).is_empty());
    }

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
    /// binding from the compositor.
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
    /// A CHORD IN THE BUFFER'S WINDOW IS ITS LOCAL KEY: `C-r` is `, r` — the
    /// group put in front by the host, and never typed.
    #[test]
    fn a_chord_is_the_local_key_with_its_group_in_front() {
        let keymap = vec![ronoctua_protocol::Binding {
            keys: ", r".into(),
            command: "run".into(),
        }];
        let mut pending: Vec<char> = ronoctua_protocol::LOCAL_PREFIX.chars().collect();
        assert_eq!(
            advance_sequence(&keymap, &mut pending, 'r').as_deref(),
            Some("run")
        );
        assert!(binds_the_chord(&keymap, 'r'));
        assert!(!binds_the_chord(&keymap, 'v'), "`C-v` still pastes");
    }

    fn buffer_keymap() -> Vec<ronoctua_protocol::Binding> {
        vec![ronoctua_protocol::Binding {
            keys: ", s".into(),
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
            advance_sequence(&km, &mut pending, ','),
            None,
            "`,` on its own does not fire"
        );
        assert_eq!(pending, vec![','], "the prefix has to stay pending");
        assert_eq!(
            advance_sequence(&km, &mut pending, 's').as_deref(),
            Some("memory-refresh")
        );
        assert!(pending.is_empty(), "firing has to clear the sequence");
    }

    #[test]
    fn a_dead_sequence_does_not_leak_into_the_text() {
        // `,` then `x`: no binding. Re-injecting the `,` would type it into the
        // file one keystroke after the person had moved on — the class of bug
        // that makes an editor untrustworthy.
        let km = buffer_keymap();
        let mut pending = Vec::new();
        advance_sequence(&km, &mut pending, ',');
        assert_eq!(advance_sequence(&km, &mut pending, 'x'), None);
        assert!(pending.is_empty(), "a dead sequence has to clear");
    }

    #[test]
    fn a_buffer_with_no_keymap_swallows_nothing() {
        // Every other buffer types text. If an empty keymap held a character
        // back, the search in the glyph browser would lose its first letter.
        let mut pending = Vec::new();
        assert_eq!(advance_sequence(&[], &mut pending, ','), None);
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
