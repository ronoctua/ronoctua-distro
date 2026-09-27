//! The screen resizer — `LEADER d r`: two curtains that cut the screen with
//! the focus from the right and from below, so the whole system can be seen
//! living on a smaller screen without the monitor changing its mode.
//!
//! # Why curtains, and not a video mode
//!
//! Because what is being tested is the SHELL on a smaller screen, and a mode
//! change tests the monitor: it goes black for a second, it only offers the
//! sizes the EDID lists, and it moves every other output. A curtain is a
//! layer surface with an exclusive zone, and the compositor already lays out
//! everything else — the footer, the column, the windows — in what the zones
//! leave. The room that is left is anchored at the top-left corner, and the
//! arithmetic of every door is the one it runs on a real screen that size.
//!
//! # Why the OVERLAY layer
//!
//! The pinned compositor applies exclusive zones layer by layer, overlay
//! first, then top (`arrange_layers`). The footer and the column live on
//! `top`, so a zone taken on `overlay` narrows them too; on `top` it would
//! depend on which surface happened to be made first. It is the third
//! exception to invariant 14 the owner approved: two surfaces that exist only
//! while the resizer is on and cover only what it cut.
//!
//! # Why two surfaces, and who owns the corner
//!
//! A layer surface has ONE exclusive edge, and the cut has two. Whichever
//! curtain the compositor arranges first spans the corner square, and nobody
//! decides which: each curtain reads it off its own configure (the right one
//! is the screen's full height, or the bottom one its full width). The
//! drawing does not depend on the answer — the two edges run across the whole
//! screen either way, and the corner square is outlined as the grip that
//! moves both.
//!
//! # Three processes
//!
//! - The KEEPER (`ronoctua-shell resizer`) has no surface. It measures the
//!   screen, writes the state, raises both curtains and waits: when either
//!   ends it ends the other and deletes the files — both curtains or none. It
//!   survives the nested loop's hot reload of the footer, and dies with the
//!   footer's cgroup in production.
//! - ONE CURTAIN PER PROCESS (`ronoctua-shell resizer-curtain right|bottom`),
//!   because `theme::reload` and `toolkits::reload` answer "did it change?"
//!   once per process: a second door in the same process would never repaint
//!   on a new theme or a new size of text.
//!
//! # The file is the broadcast
//!
//! `resizer.json`, per instance (`ronoctua_xdg::runtime_file`), exists exactly
//! while the resizer is on. Every writer merges BY FIELD: a curtain writes the
//! axes it moved and nothing else, and while it drags an axis it ignores that
//! axis in what it reads back — which is how it recognises its own writes.
//!
//! # Units
//!
//! Every size here is in LOGICAL pixels, the compositor's unit for a layer's
//! size and zone; the scale never enters the arithmetic (the door handles
//! it). A key moves by whole cells and lines, so one arrow changes the
//! footer's column count by exactly one; a drag follows the pointer pixel by
//! pixel.

use crate::chrome::{Piece, Register};
use crate::door::{self as base, Canvas, Content, Hand, LEFT_BUTTON, Shell, Ui, Wiring};
use anyhow::{Context as _, Result};
use ronoctua_keymap::{Key, Sym};
use ronoctua_render::quad::Quad;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::FrameCallbackData,
    reexports::calloop::channel,
    seat::{
        keyboard::{KeyEvent, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
    shell::{
        WaylandSurface as _,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerSurface},
    },
};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use wayland_client::{QueueHandle, protocol::wl_surface};

/// The runtime file that holds the cut, and exists exactly while it is on.
pub const FILE: &str = "resizer.json";

/// The runtime file that holds the keeper's pid.
const KEEPER_FILE: &str = "resizer.pid";

/// The word after the program's name that starts the keeper.
pub const KEEPER_WORD: &str = "resizer";

/// The word after the program's name that starts one curtain.
pub const CURTAIN_WORD: &str = "resizer-curtain";

/// The layer-shell namespace of both curtains.
const NAMESPACE: &str = "ronoctua-resizer";

/// The curtains' `app_id`.
const APP_ID: &str = "br.ronoctua.resizer";

/// The keys the bottom curtain's line teaches, and what their words are.
///
/// `ESC` FIRST: the line is cut at its far end on a narrow room, and the way
/// out is the one key that must never be the part cut off.
pub const OF_THE_RESIZER: &[(&str, &str)] = &[
    ("ESC", "resizer-off"),
    ("<left>", "resizer-narrower"),
    ("<right>", "resizer-wider"),
    ("<up>", "resizer-shorter"),
    ("<down>", "resizer-taller"),
    ("f", "resizer-largest"),
];

/// Which side of the screen a curtain cuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// The right one: the room's width.
    Right,
    /// The bottom one: the room's height, and the line that says the size.
    Bottom,
}

impl Edge {
    /// Both, in the order the keeper raises them.
    pub const ALL: [Self; 2] = [Self::Right, Self::Bottom];

    /// Its word on the command line.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Right => "right",
            Self::Bottom => "bottom",
        }
    }

    /// The edge a word names.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.word() == word)
    }
}

/// The cut, as the file holds it — every size in logical pixels.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct State {
    /// The connector of the screen that is cut.
    pub output: String,
    /// That screen's whole width.
    pub screen_width_px: u32,
    /// That screen's whole height.
    pub screen_height_px: u32,
    /// The width of the room the curtains leave.
    pub width_px: u32,
    /// The height of the room the curtains leave.
    pub height_px: u32,
}

/// What a cell and a line measure now, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Units {
    /// One cell's width.
    pub cell: f32,
    /// One line's height.
    pub line: f32,
}

impl Units {
    /// At the size of the text the person chose.
    #[must_use]
    pub fn now() -> Self {
        let size = tokens::text::size();
        Self {
            cell: tokens::metrics::cell_width(size),
            line: tokens::metrics::line_height(size),
        }
    }
}

/// How far the cut may go, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The narrowest room: `space.resizer.minColumns` cells.
    pub min_width_px: u32,
    /// The shortest room: `space.resizer.minLines` lines.
    pub min_height_px: u32,
    /// The thinnest right curtain: `space.resizer.edgeCells` cells, so it
    /// can still be grabbed.
    pub right_min_px: u32,
    /// The thinnest bottom curtain: one line, the one that says the size.
    pub bottom_min_px: u32,
}

/// Which way a grip moves the cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grip {
    /// The right edge: the room's width.
    Width,
    /// The bottom edge: the room's height.
    Height,
    /// The corner square: both.
    Both,
}

/// How the keeper ended, as the footer reads it off its output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    /// It could not cut the screen, and why.
    Failed(String),
    /// It ended — a curtain was closed, or the footer ended it.
    Ended,
}

/// What a key does to the cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// The screen whole again.
    Off,
    /// So many columns more (or fewer).
    Width(i64),
    /// So many lines more (or fewer).
    Height(i64),
    /// As large as the curtains allow.
    Largest,
}

/// A rectangle in a curtain's own logical pixels: `(x, y, width, height)`.
pub type Box4 = (f32, f32, f32, f32);

/// How many whole `unit`s fit in `px`.
#[allow(clippy::cast_possible_truncation)]
fn count(px: u32, unit: f32) -> i64 {
    if unit <= 0.0 {
        return 0;
    }
    (f64::from(px) / f64::from(unit)).floor() as i64
}

/// `px` moved by `by` whole units: the SMALLEST size that holds exactly the
/// units it had plus `by` — so one step changes what fits by exactly one, at
/// any size of cell, and a size that was not on the grid lands on it.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn stepped(px: u32, unit: f32, by: i64) -> u32 {
    if unit <= 0.0 {
        return px;
    }
    let wanted = count(px, unit).saturating_add(by).max(0);
    let mut out = (wanted as f64 * f64::from(unit)).ceil() as u32;
    // THE FLOATING POINT IS ASKED, not trusted: the product can land a hair
    // either side of the boundary, and the answer is whatever the same
    // division the grid uses says.
    while count(out, unit) < wanted && out < u32::MAX {
        out += 1;
    }
    while out > 0 && count(out - 1, unit) >= wanted {
        out -= 1;
    }
    out
}

/// The limits at these units.
#[must_use]
pub fn limits(u: Units) -> Limits {
    Limits {
        min_width_px: stepped(0, u.cell, i64::from(tokens::space::RESIZER_MINCOLUMNS)),
        min_height_px: stepped(0, u.line, i64::from(tokens::space::RESIZER_MINLINES)),
        right_min_px: stepped(0, u.cell, i64::from(tokens::space::RESIZER_EDGECELLS)),
        bottom_min_px: stepped(0, u.line, 1),
    }
}

/// The cut kept between its floor and the largest room the curtains allow.
///
/// A SCREEN TOO SMALL FOR THE FLOOR keeps the largest room instead: the
/// curtains have to stay thick enough to be grabbed, or there would be no way
/// to take them back.
#[must_use]
pub fn fitted(mut s: State, l: Limits) -> State {
    let most_wide = s.screen_width_px.saturating_sub(l.right_min_px);
    let most_tall = s.screen_height_px.saturating_sub(l.bottom_min_px);
    s.width_px = s.width_px.max(l.min_width_px).min(most_wide);
    s.height_px = s.height_px.max(l.min_height_px).min(most_tall);
    s
}

/// The largest room a screen of `width` × `height` leaves.
#[must_use]
pub fn largest(output: String, width: u32, height: u32, l: Limits) -> State {
    fitted(
        State {
            output,
            screen_width_px: width,
            screen_height_px: height,
            width_px: u32::MAX,
            height_px: u32::MAX,
        },
        l,
    )
}

/// The same room on a screen that is now `size`: kept where it still fits.
#[must_use]
pub fn remeasured(mut s: State, size: (u32, u32), l: Limits) -> State {
    (s.screen_width_px, s.screen_height_px) = size;
    fitted(s, l)
}

/// How many columns and lines the room holds.
#[must_use]
pub fn grid_of(s: &State, u: Units) -> (i64, i64) {
    (count(s.width_px, u.cell), count(s.height_px, u.line))
}

/// How thick a curtain is: what the room leaves of its axis.
#[must_use]
pub const fn curtain_px(edge: Edge, s: &State) -> u32 {
    match edge {
        Edge::Right => s.screen_width_px.saturating_sub(s.width_px),
        Edge::Bottom => s.screen_height_px.saturating_sub(s.height_px),
    }
}

/// Whether a curtain spans the corner square — read off its own configure:
/// the right one is the whole height, the bottom one the whole width.
#[must_use]
pub const fn owns_corner(edge: Edge, s: &State, configured: (u32, u32)) -> bool {
    match edge {
        Edge::Right => configured.1.saturating_add(1) >= s.screen_height_px,
        Edge::Bottom => configured.0.saturating_add(1) >= s.screen_width_px,
    }
}

/// Whether a configure says something the cut does not: the curtain's long
/// axis is neither the whole screen nor the room, which a screen that changed
/// its size does (a pixel either way is the door's own rounding).
#[must_use]
pub const fn needs_measuring(edge: Edge, s: &State, configured: (u32, u32)) -> bool {
    let (long, whole, room) = match edge {
        Edge::Right => (configured.1, s.screen_height_px, s.height_px),
        Edge::Bottom => (configured.0, s.screen_width_px, s.width_px),
    };
    long != 0 && long.abs_diff(whole) > 1 && long.abs_diff(room) > 1
}

/// Every grip of a curtain, and the rectangle it is taken on — the whole
/// curtain, split at the corner when it spans the corner square.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn grips(edge: Edge, s: &State, configured: (u32, u32)) -> Vec<(Grip, Box4)> {
    let (w, h) = (configured.0 as f32, configured.1 as f32);
    let owns = owns_corner(edge, s, configured);
    match edge {
        Edge::Right if owns => {
            let room = (s.height_px as f32).min(h);
            vec![
                (Grip::Width, (0.0, 0.0, w, room)),
                (Grip::Both, (0.0, room, w, h - room)),
            ]
        }
        Edge::Right => vec![(Grip::Width, (0.0, 0.0, w, h))],
        Edge::Bottom if owns => {
            let room = (s.width_px as f32).min(w);
            vec![
                (Grip::Height, (0.0, 0.0, room, h)),
                (Grip::Both, (room, 0.0, w - room, h)),
            ]
        }
        Edge::Bottom => vec![(Grip::Height, (0.0, 0.0, w, h))],
    }
}

/// How wide the bottom curtain's line is, in logical pixels: the whole
/// curtain, or — when it spans the corner — only the part under the room, so
/// the corner square stays the grip of both axes (`Curtain::paint_the_line`).
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn line_width(s: &State, surface: (u32, u32), configured: (u32, u32)) -> f32 {
    let whole = surface.0 as f32;
    if owns_corner(Edge::Bottom, s, configured) {
        whole.min(s.width_px as f32)
    } else {
        whole
    }
}

/// How many cells of text fit in `width` logical pixels inside the door's
/// margin — the arithmetic `Ui::recount` uses for the whole surface.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn columns_in(width: f32, margin: u16) -> u16 {
    let cell = tokens::metrics::cell_width(tokens::text::size());
    let usable = width - 2.0 * cell * f32::from(margin);
    (usable / cell).floor().max(0.0) as u16
}

/// The grip at a point of the curtain.
#[must_use]
pub fn grip_at(grips: &[(Grip, Box4)], (x, y): (f32, f32)) -> Option<Grip> {
    grips
        .iter()
        .find(|(_, b)| x >= b.0 && y >= b.1 && x < b.0 + b.2 && y < b.1 + b.3)
        .map(|(g, _)| *g)
}

/// A rectangle of one colour.
const fn quad(x: f32, y: f32, width: f32, height: f32, color: tokens::Color) -> Quad {
    Quad {
        x,
        y,
        width,
        height,
        color,
    }
}

/// The edges, drawn from the SAME rectangles [`grips`] answers — a control
/// is grabbed where it is drawn.
///
/// Each grip draws the sides of its rectangle that face the room: the right
/// one its left side, the bottom one its top, and the corner square both, so
/// the two edges run across the whole screen whichever curtain spans the
/// corner. `space.resizer.edgePx` thick, in `rule.strong` at rest and in the
/// capture's guide colour while the pointer is on it or drags it.
#[must_use]
pub fn edges(grips: &[(Grip, Box4)], lit: Option<Grip>) -> Vec<Quad> {
    let thick = f32::from(tokens::space::RESIZER_EDGEPX);
    let mut out = Vec::with_capacity(grips.len() * 2);
    for &(grip, (x, y, w, h)) in grips {
        let color = if lit == Some(grip) {
            tokens::color::CAPTURE_GUIDE
        } else {
            tokens::color::RULE_STRONG
        };
        if matches!(grip, Grip::Height | Grip::Both) {
            out.push(quad(x, y, w, thick.min(h), color));
        }
        if matches!(grip, Grip::Width | Grip::Both) {
            out.push(quad(x, y, thick.min(w), h, color));
        }
    }
    out
}

/// The cut after a drag of `delta` logical pixels — how far the hand went
/// since the button went down (see `Drag::delta`).
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn dragged(from: &State, grip: Grip, delta: (f64, f64), l: Limits) -> State {
    let moved = |px: u32, by: f64| (f64::from(px) + by).round().max(0.0) as u32;
    let mut s = from.clone();
    if matches!(grip, Grip::Width | Grip::Both) {
        s.width_px = moved(from.width_px, delta.0);
    }
    if matches!(grip, Grip::Height | Grip::Both) {
        s.height_px = moved(from.height_px, delta.1);
    }
    fitted(s, l)
}

/// What a key does — `None` for a key the curtains do not answer.
///
/// `Shift` with an arrow is `space.resizer.bigStep` of them.
#[must_use]
pub fn press_of(key: Key) -> Option<Press> {
    let m = key.mods;
    if m.ctrl || m.alt || m.logo {
        return None;
    }
    let stride = if m.shift {
        i64::from(tokens::space::RESIZER_BIGSTEP)
    } else {
        1
    };
    Some(match key.sym {
        Sym::Esc => Press::Off,
        Sym::Left => Press::Width(-stride),
        Sym::Right => Press::Width(stride),
        Sym::Up => Press::Height(-stride),
        Sym::Down => Press::Height(stride),
        Sym::Char('f') => Press::Largest,
        _ => return None,
    })
}

/// How the keeper ended, from what it printed: `failed <why>` or nothing.
#[must_use]
pub fn last_word(said: &str) -> Heard {
    said.lines()
        .rev()
        .find_map(|l| l.strip_prefix("failed "))
        .map_or(Heard::Ended, |why| Heard::Failed(why.to_owned()))
}

/// The height the footer has on a screen: the room, where the resizer cuts
/// that screen, and the screen's own height anywhere else.
#[must_use]
pub fn room_height(output_height: u32, output: Option<&str>, cut: Option<&State>) -> u32 {
    match cut {
        Some(s) if output == Some(s.output.as_str()) => output_height.min(s.height_px),
        _ => output_height,
    }
}

/// The height of the SHORTEST room among `screens` — each screen's height and
/// its connector, when it has one — under `cut`: what the footer's ceiling is
/// measured on, since its height is the same on every screen. A screen of no
/// height yet is not counted, and `None` is no screen at all.
#[must_use]
pub fn smallest_room(
    screens: impl IntoIterator<Item = (u32, Option<String>)>,
    cut: Option<&State>,
) -> Option<u32> {
    screens
        .into_iter()
        .map(|(height, name)| room_height(height, name.as_deref(), cut))
        .filter(|h| *h > 0)
        .min()
}

/// The width a column has on a screen: the room, where the resizer cuts that
/// screen, and the screen's own width anywhere else — the twin of
/// [`room_height`]. A column that took its share of the MONITOR under a cut
/// took most of a narrow room, and past the room's own width it hung off
/// its left edge.
#[must_use]
pub fn room_width(output_width: u32, output: Option<&str>, cut: Option<&State>) -> u32 {
    match cut {
        Some(s) if output == Some(s.output.as_str()) => output_width.min(s.width_px),
        _ => output_width,
    }
}

/// Which axes a writer owns in what it writes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Axes {
    /// The room's width.
    width: bool,
    /// The room's height.
    height: bool,
}

impl Axes {
    /// The axes a grip moves.
    const fn of(grip: Grip) -> Self {
        Self {
            width: matches!(grip, Grip::Width | Grip::Both),
            height: matches!(grip, Grip::Height | Grip::Both),
        }
    }

    /// Both sets together.
    const fn and(self, other: Self) -> Self {
        Self {
            width: self.width || other.width,
            height: self.height || other.height,
        }
    }

    /// Whether any axis is set.
    const fn any(self) -> bool {
        self.width || self.height
    }
}

/// What the file says, with `mine`'s value for the axes this writer owns —
/// the merge BY FIELD both directions use: a write puts only its own axes
/// over the file, and a read keeps the axes being dragged.
fn merged(mine: &State, mut file: State, keep: Axes) -> State {
    if keep.width {
        file.width_px = mine.width_px;
    }
    if keep.height {
        file.height_px = mine.height_px;
    }
    file
}

/// The file's path, when there is a runtime directory.
fn path() -> Option<PathBuf> {
    ronoctua_xdg::runtime_file(FILE)
}

/// The cut, when the file is there and reads; `None` for no file and for one
/// that does not parse.
#[must_use]
pub fn read() -> Option<State> {
    read_at(&path()?)
}

/// See [`read`].
fn read_at(path: &Path) -> Option<State> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Writes the cut, atomically.
fn write_at(path: &Path, s: &State) -> std::io::Result<()> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return Err(std::io::Error::other("the resizer's file has no directory"));
    };
    let bytes = serde_json::to_vec(s).map_err(std::io::Error::other)?;
    ronoctua_xdg::write_atomically(dir, name, &bytes)
}

/// Changes the file, and answers what it holds now — `None` when there is no
/// file or it does not parse: A WRITER NEVER WRITES OVER WHAT IT COULD NOT
/// READ. A missing file is the resizer being off, and writing it would turn
/// it back on behind the person's back.
fn update(f: impl FnOnce(&mut State)) -> std::io::Result<Option<State>> {
    match path() {
        Some(p) => update_at(&p, f),
        None => Ok(None),
    }
}

/// See [`update`].
fn update_at(path: &Path, f: impl FnOnce(&mut State)) -> std::io::Result<Option<State>> {
    let Some(mut s) = read_at(path) else {
        return Ok(None);
    };
    f(&mut s);
    write_at(path, &s)?;
    Ok(Some(s))
}

/// Whether a `/proc/<pid>/cmdline` is this program started as the keeper.
fn is_the_keeper(cmdline: &[u8]) -> bool {
    crate::right_column::started_as(cmdline, &[KEEPER_WORD])
}

/// The keeper's pid — only while that process is alive, is not this one,
/// and is this program started as the keeper. A pid is recycled, and a
/// signal sent to a number is sent to whoever holds it now.
#[must_use]
pub fn keeper() -> Option<i32> {
    let text = std::fs::read_to_string(ronoctua_xdg::runtime_file(KEEPER_FILE)?).ok()?;
    let pid: i32 = text.trim().parse().ok()?;
    if u32::try_from(pid).ok() == Some(std::process::id()) {
        return None;
    }
    let line = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    is_the_keeper(&line).then_some(pid)
}

/// The cut, while the resizer is on: the file reads AND its keeper lives.
#[must_use]
pub fn is_on() -> Option<State> {
    keeper()?;
    read()
}

/// Starts the keeper, with its output piped: the footer reads how it ended.
///
/// # Errors
///
/// When this program cannot find itself or the process cannot be started.
pub fn start() -> std::io::Result<std::process::Child> {
    std::process::Command::new(std::env::current_exe()?)
        .arg(KEEPER_WORD)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
}

/// Ends the resizer: the file first — both curtains watch it and leave — and
/// then the keeper, so nothing is left even if a curtain is stuck.
pub fn stop() {
    if let Some(p) = path() {
        let _ = std::fs::remove_file(p);
    }
    if let Some(pid) = keeper().and_then(rustix::process::Pid::from_raw) {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
    }
}

/// Deletes the files when no keeper answers for them — a keeper killed by a
/// signal (or with a session) leaves both behind.
pub fn sweep_stale() {
    if keeper().is_some() {
        return;
    }
    for leaf in [FILE, KEEPER_FILE] {
        if let Some(p) = ronoctua_xdg::runtime_file(leaf) {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// The directory and the name to watch the file by — out of the same
/// `runtime_file` every reader and writer uses, so the per-instance name is
/// written once.
#[must_use]
pub fn watch_target() -> Option<(PathBuf, String)> {
    let p = path()?;
    Some((
        p.parent()?.to_path_buf(),
        p.file_name()?.to_str()?.to_owned(),
    ))
}

/// One line for the footer, which reads the keeper's output. A footer that
/// went away (the nested loop reloads it) closed the pipe, and a line nobody
/// reads is not a reason to fail.
fn say(line: &str) {
    use std::io::Write as _;
    let _ = writeln!(std::io::stdout(), "{line}");
}

/// THE KEEPER: measures the screen with the focus, writes the cut, raises the
/// two curtains and ends both when either ends.
///
/// Prints `failed <why>` for any failure — a curtain that could not start
/// included — and nothing when it ended because somebody ended it.
///
/// # Errors
///
/// No compositor, no screen with the focus, no runtime directory, a curtain
/// that could not be raised or that failed.
pub fn keep_it() -> Result<()> {
    keep().inspect_err(|e| say(&format!("failed {e:#}")))
}

/// The files, deleted when the keeper is done with them — on every way out
/// of [`keep`], and only while the pid file still names this process: a
/// keeper that was replaced deletes nothing of the one that replaced it.
struct Kept {
    /// `resizer.json`.
    state: PathBuf,
    /// `resizer.pid`.
    pid: PathBuf,
}

impl Drop for Kept {
    fn drop(&mut self) {
        let mine = std::fs::read_to_string(&self.pid)
            .ok()
            .and_then(|t| t.trim().parse::<u32>().ok())
            == Some(std::process::id());
        if mine {
            let _ = std::fs::remove_file(&self.state);
            let _ = std::fs::remove_file(&self.pid);
        }
    }
}

/// The whole of the keeper.
fn keep() -> Result<()> {
    // ONE KEEPER: a second start while one lives is the same resizer.
    if keeper().is_some() {
        return Ok(());
    }
    let state_file = path().context("there is no runtime directory")?;
    let pid_file =
        ronoctua_xdg::runtime_file(KEEPER_FILE).context("there is no runtime directory")?;
    let mut wm = crate::daemon::open_the_compositor().context("no compositor to ask")?;
    let out = wm
        .focused_output()
        .context("the compositor did not say which screen has the focus")?
        .context("no screen has the focus")?;
    let at = out.logical.context("the screen with the focus is off")?;
    let cut = largest(out.name, at.width, at.height, limits(Units::now()));

    let kept = Kept {
        state: state_file.clone(),
        pid: pid_file.clone(),
    };
    // THE PID BEFORE THE CUT: a reader that sees the cut asks for its keeper.
    let dir = pid_file
        .parent()
        .context("the runtime directory has no path")?;
    ronoctua_xdg::write_atomically(
        dir,
        &name_of(&pid_file)?,
        std::process::id().to_string().as_bytes(),
    )
    .context("could not write the keeper's pid")?;
    write_at(&state_file, &cut).context("could not write the cut")?;

    let me = std::env::current_exe().context("this program cannot find itself")?;
    let raise = |edge: Edge| {
        std::process::Command::new(&me)
            .args([CURTAIN_WORD, edge.word()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .with_context(|| format!("could not raise the {} curtain", edge.word()))
    };
    let right = raise(Edge::Right)?;
    let bottom = match raise(Edge::Bottom) {
        Ok(b) => b,
        Err(e) => {
            end(rustix::process::Pid::from_child(&right));
            return Err(e);
        }
    };
    let curtains = [
        (Edge::Right, rustix::process::Pid::from_child(&right)),
        (Edge::Bottom, rustix::process::Pid::from_child(&bottom)),
    ];
    let (first, status) = first_to_end(&curtains)?;
    // BOTH OR NONE: the other one goes with it.
    for (edge, pid) in curtains {
        if edge != first {
            end(pid);
        }
    }
    drop(kept);
    verdict(first, status.exit_status(), status.terminating_signal())
}

/// A path's last part, as the name `write_atomically` takes.
fn name_of(path: &Path) -> Result<String> {
    Ok(path
        .file_name()
        .and_then(|n| n.to_str())
        .context("a runtime file with no name")?
        .to_owned())
}

/// Waits for the first curtain to end.
fn first_to_end(
    curtains: &[(Edge, rustix::process::Pid)],
) -> Result<(Edge, rustix::process::WaitStatus)> {
    loop {
        match rustix::process::wait(rustix::process::WaitOptions::empty()) {
            Ok(Some((pid, status))) => {
                if let Some((edge, _)) = curtains.iter().find(|(_, p)| *p == pid) {
                    return Ok((*edge, status));
                }
            }
            Ok(None) | Err(rustix::io::Errno::INTR) => {}
            Err(e) => return Err(anyhow::Error::from(e).context("waiting for the curtains")),
        }
    }
}

/// Ends a curtain and waits for it.
fn end(pid: rustix::process::Pid) {
    let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
    loop {
        match rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::empty()) {
            Err(rustix::io::Errno::INTR) => {}
            _ => return,
        }
    }
}

/// What the first curtain's end means: nothing to say when it was closed
/// (its own `Esc`) or ended (`TERM`), and a failure otherwise.
fn verdict(edge: Edge, exit: Option<i32>, signal: Option<i32>) -> Result<()> {
    if exit == Some(0) || signal == Some(rustix::process::Signal::TERM.as_raw()) {
        return Ok(());
    }
    match (exit, signal) {
        (Some(code), _) => anyhow::bail!("the {} curtain exited with {code}", edge.word()),
        (_, Some(sig)) => anyhow::bail!("the {} curtain was killed by signal {sig}", edge.word()),
        _ => anyhow::bail!("the {} curtain ended", edge.word()),
    }
}

/// ONE CURTAIN: raised by the keeper and by nothing else, and gone with it.
///
/// # Errors
///
/// When the parent is not the keeper, the resizer is off, or there is no
/// compositor to draw on.
pub fn curtain_it(edge: Edge) -> Result<()> {
    // GONE WITH THE KEEPER, whatever ends it: a curtain with no keeper is a
    // cut nobody will ever take back.
    rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::TERM))
        .context("could not tie the curtain to its keeper")?;
    let parent = rustix::process::getppid().context("the curtain has no parent")?;
    let line = std::fs::read(format!("/proc/{}/cmdline", parent.as_raw_nonzero()))
        .context("could not read the parent's command line")?;
    anyhow::ensure!(
        is_the_keeper(&line),
        "a curtain is raised by the resizer's keeper, and this one's parent is not it"
    );
    let state = read().context("the resizer is off: there is no cut to draw")?;
    base::run(move |wiring| {
        let (told, heard) = channel::channel::<()>();
        let watch = watch_target().and_then(|(dir, name)| {
            ronoctua_files::notice::Notice::on_file(
                &dir,
                &name,
                ronoctua_files::notice::Pace::AtOnce,
                move || told.send(()).is_ok(),
            )
            .ok()
        });
        wiring
            .handle
            .insert_source(heard, |event, (), door: &mut base::Door<Curtain>| {
                if let channel::Event::Msg(()) = event {
                    door.content.file_changed(&mut door.ui);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the resizer's watch: {e}"))?;
        Ok(Curtain {
            edge,
            state,
            configured: (0, 0),
            told_px: Cell::new(0),
            layer: RefCell::new(None),
            qh: wiring.qh.clone(),
            settle_first: true,
            settle_asked: false,
            _watch: watch,
            marks: Vec::new(),
            hint_lit: None,
            lit: None,
            dragging: None,
            unsaved: Axes::default(),
            wm: None,
        })
    })
}

/// What one pointer keeps of its own: each hand drags on its own.
#[derive(Debug, Default)]
struct PerPointer {
    /// The drag this pointer is making, while its button is down.
    drag: Option<Drag>,
}

/// A drag in progress.
#[derive(Debug, Clone)]
struct Drag {
    /// What it moves.
    grip: Grip,
    /// The cut when the button went down.
    from: State,
    /// Where the button went down, in the curtain's own pixels AT THE PRESS.
    pressed_local: (f64, f64),
}

impl Drag {
    /// How far the hand went since the press, with the pointer reported at
    /// `local`.
    ///
    /// # Why no origin
    ///
    /// Because the compositor already took it out. While a button is held
    /// on a layer surface, the pinned compositor reports the pointer in the
    /// GRAB's frame: the local point of the press plus how far the cursor
    /// moved on the screen (`seatop_down`'s `handle_pointer_motion`, and its
    /// touch twin), and it never re-bases that when the surface moves. The
    /// curtain moves under the hand at every configure, and adding where it
    /// is now on top counted every step again: the room ran to its floor
    /// under a hand that had stopped.
    fn delta(&self, local: (f64, f64)) -> (f64, f64) {
        (
            local.0 - self.pressed_local.0,
            local.1 - self.pressed_local.1,
        )
    }

    /// The cut with the pointer at `local`, over the cut as it is `now`:
    /// ONLY THE AXES DRAGGED — the other one is whatever the other curtain
    /// made it since the press.
    fn to(&self, now: &State, local: (f64, f64), l: Limits) -> State {
        let moved = dragged(&self.from, self.grip, self.delta(local), l);
        merged(&moved, now.clone(), Axes::of(self.grip))
    }
}

/// One curtain, as a door's content.
struct Curtain {
    /// Which side it cuts.
    edge: Edge,
    /// The cut, as this curtain last read or made it.
    state: State,
    /// The size the compositor last configured, in logical pixels.
    configured: (u32, u32),
    /// The thickness this curtain last told the compositor.
    told_px: Cell<u32>,
    /// The surface's role, kept to tell it a new size (a handle onto the
    /// same object the door holds).
    layer: RefCell<Option<LayerSurface>>,
    /// The queue, for the first frame's callback.
    qh: QueueHandle<base::Door<Self>>,
    /// Whether the frame that MAPS the curtain is still to be drawn.
    settle_first: bool,
    /// Whether that frame's callback was asked for.
    settle_asked: bool,
    /// The watch on the file.
    _watch: Option<ronoctua_files::notice::Notice>,
    /// Where each shortcut of the line landed, from what was drawn.
    marks: Vec<crate::chrome::Mark>,
    /// The shortcut under the pointer.
    hint_lit: Option<&'static str>,
    /// The grip under the pointer.
    lit: Option<Grip>,
    /// The grip a hand is dragging.
    dragging: Option<Grip>,
    /// The axes moved and not yet written — written once per turn.
    unsaved: Axes,
    /// The compositor, opened the first time it is needed.
    wm: Option<Box<dyn ronoctua_compositor::Compositor>>,
}

/// An exclusive zone of `px`.
fn zone_of(px: u32) -> i32 {
    i32::try_from(px).unwrap_or(i32::MAX)
}

/// The bottom curtain's line: the way out, the size in pixels and in the
/// grid, and the keys — and, while one of them is under the pointer, what it
/// does, RIGHT AFTER IT, in the place of everything that followed it.
///
/// Right after it, because the line is cut at the curtain's end, and a room
/// cut narrow is what this tool is for: after the whole line, the sentence
/// was the part cut off, and pointing at a key explained nothing. In the
/// place of what FOLLOWED, because nothing up to the key under the pointer
/// may move: a key that moved would leave the pointer, its sentence would go
/// and the key come back, in a loop.
fn line_of(s: &State, lit: Option<&str>, texts: &ronoctua_i18n::Texts) -> Vec<Piece> {
    use ronoctua_i18n::{Arg, key};
    let (way_out, rest) = OF_THE_RESIZER.split_at(1);
    let (columns, lines) = grid_of(s, Units::now());
    let mut pieces = crate::chrome::hint_of(way_out, texts);
    pieces.push(Piece::separator());
    pieces.push(Piece::said(texts.t_args(
        key::RESIZER_SIZE,
        &[
            ("width", Arg::Number(i64::from(s.width_px))),
            ("height", Arg::Number(i64::from(s.height_px))),
        ],
    )));
    pieces.push(Piece::separator());
    pieces.push(Piece::said(texts.t_args(
        key::RESIZER_GRID,
        &[
            ("columns", Arg::Number(columns)),
            ("lines", Arg::Number(lines)),
        ],
    )));
    pieces.push(Piece::separator());
    pieces.extend(crate::chrome::hint_of(rest, texts));
    let mut pieces = crate::chrome::lit(&pieces, lit);
    let explained = lit.and_then(|id| {
        let last = pieces.iter().rposition(|p| p.id == Some(id))?;
        Some((last, crate::chrome::hint_sentence(id, texts)?))
    });
    if let Some((last, said)) = explained {
        pieces.truncate(last + 1);
        pieces.push(Piece::separator());
        pieces.push(Piece {
            text: said,
            register: Register::Explain,
            id: None,
            press: None,
        });
    }
    pieces
}

impl Curtain {
    /// The shortcut drawn under `(x, y)`.
    fn mark_at(&self, (x, y): (f64, f64)) -> Option<crate::chrome::Mark> {
        let (line, cell) = crate::text_select::Selecting::cell_of(x, y)?;
        self.marks.iter().find(|m| m.covers(line, cell)).cloned()
    }

    /// This curtain's grips, at the size it was configured.
    fn grips_now(&self) -> Vec<(Grip, Box4)> {
        grips(self.edge, &self.state, self.configured)
    }

    /// Tells the compositor this curtain's thickness, when the cut moved it.
    fn apply(&self) {
        let px = curtain_px(self.edge, &self.state);
        if px == self.told_px.get() {
            return;
        }
        if let Some(l) = &*self.layer.borrow() {
            match self.edge {
                Edge::Right => l.set_size(px, 0),
                Edge::Bottom => l.set_size(0, px),
            }
            l.set_exclusive_zone(zone_of(px));
            l.commit();
        }
        self.told_px.set(px);
    }

    /// A new cut made HERE, owed to the file and to the compositor: both
    /// are told in [`Self::write`], once however many motions a turn had.
    fn moved(&mut self, ui: &mut Ui, next: State, axes: Axes) {
        if next == self.state {
            return;
        }
        self.state = next;
        self.unsaved = self.unsaved.and(axes);
        ui.ask_paint();
    }

    /// Writes the axes moved here over the file — the file's own values for
    /// the rest, which is the other curtain's — and THEN tells the
    /// compositor. In that order: the other curtain is configured by this
    /// commit, and what the configure says has to be in the file already
    /// when it reads it (see `resized`).
    fn write(&mut self, ui: &mut Ui, axes: Axes) {
        let mine = self.state.clone();
        match update(|s| *s = merged(&mine, s.clone(), axes)) {
            Ok(Some(s)) => self.state = s,
            // NO FILE: the resizer was ended while this hand moved it.
            Ok(None) => {
                ui.quit = true;
                return;
            }
            Err(e) => tracing::warn!(error = %e, "the resizer's cut could not be written"),
        }
        self.apply();
    }

    /// The file changed: gone is the resizer off; otherwise the cut, with
    /// the axes this curtain is moving left as they are.
    fn file_changed(&mut self, ui: &mut Ui) {
        let Some(p) = path() else {
            return;
        };
        let Some(file) = read_at(&p) else {
            if !p.exists() {
                ui.quit = true;
            }
            return;
        };
        let keep = self
            .unsaved
            .and(self.dragging.map(Axes::of).unwrap_or_default());
        let next = merged(&self.state, file, keep);
        if next != self.state {
            self.state = next;
            self.apply();
            ui.ask_paint();
        }
    }

    /// The configure said the screen is not the size the cut was made on:
    /// asks the compositor, and keeps the room it was given where it still
    /// fits. A screen that is gone takes the curtain with it.
    fn measure_again(&mut self, ui: &mut Ui) {
        if self.wm.is_none() {
            self.wm = crate::daemon::open_the_compositor().ok();
        }
        let Some(wm) = self.wm.as_mut() else {
            return;
        };
        let size = match wm.outputs() {
            Ok(all) => all
                .get(&self.state.output)
                .and_then(|o| o.logical)
                .map(|l| (l.width, l.height)),
            Err(e) => {
                tracing::warn!(error = %e, "the screens could not be read again");
                return;
            }
        };
        let Some(size) = size else {
            tracing::info!(output = %self.state.output, "the screen that was cut is gone");
            ui.quit = true;
            return;
        };
        if size == (self.state.screen_width_px, self.state.screen_height_px) {
            return;
        }
        let l = limits(Units::now());
        match update(|s| *s = remeasured(s.clone(), size, l)) {
            Ok(Some(s)) => {
                self.state = s;
                self.apply();
                ui.ask_paint();
            }
            Ok(None) => ui.quit = true,
            Err(e) => tracing::warn!(error = %e, "the resizer's cut could not be written"),
        }
    }

    /// Tells the compositor to send the windows the layout the curtain's
    /// zone made — asked once the frame that MAPPED it has been shown. A
    /// map carries no layer state, so the compositor sends nothing on it
    /// (the player's column found this first: `settle_the_windows`).
    fn settle_the_windows(&mut self) {
        if self.wm.is_none() {
            self.wm = crate::daemon::open_the_compositor().ok();
        }
        match self.wm.as_mut().map(|w| w.settle()) {
            Some(Ok(())) => {
                tracing::info!(edge = self.edge.word(), "the windows were told the room")
            }
            Some(Err(e)) => tracing::warn!(error = %e, "the windows were not told the room"),
            None => tracing::warn!("no compositor to tell the room to"),
        }
    }

    /// The bottom curtain's line, on the header's ground — the one ground of
    /// the curtain text is measured against.
    ///
    /// THE AIR, on both sides. Above: the curtain's edge is its top
    /// `edgePx`, laid over this line, and the line's own leading (1.45 of the
    /// size, against a glyph box of 1.25 that still has its ascent room)
    /// keeps the letters clear of it. On the left the door's margin, and on
    /// the right the line is cut a margin short of the curtain's end.
    ///
    /// AND IT STOPS WHERE THE ROOM STOPS. When this curtain spans the corner,
    /// the square past the room's width is the grip of BOTH axes — the place
    /// a hand goes to resize the whole screen at once — and a shortcut drawn
    /// there took the press instead (measured: a press two pixels into the
    /// corner narrowed the room by one column). The line is laid out on the
    /// room's width ([`line_width`]), and the corner square keeps only its
    /// edges.
    #[allow(clippy::cast_precision_loss)]
    fn paint_the_line(&mut self, ui: &Ui, canvas: &mut Canvas<'_>) {
        let f = ui.frame(0);
        let width = line_width(&self.state, (ui.width, ui.height), self.configured);
        let columns = columns_in(width, ui.margin);
        canvas.under.push(quad(
            0.0,
            f.y_of(0),
            width,
            f.line(),
            tokens::color::SURFACE_HEADER,
        ));
        let pieces = line_of(&self.state, self.hint_lit, &ui.texts);
        canvas.spans.extend(crate::chrome::hint_spans(
            &pieces,
            ronoctua_render::Size::Grid,
            ui.margin,
            0,
            columns,
        ));
        self.marks = crate::chrome::hint_marks(&pieces, ui.margin, 0, columns);
    }

    /// A press: a step, the largest room, or the way out.
    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        let u = Units::now();
        let mut next = self.state.clone();
        let axes = match press {
            Press::Off => {
                ui.quit = true;
                return;
            }
            Press::Width(by) => {
                next.width_px = stepped(next.width_px, u.cell, by);
                Axes::of(Grip::Width)
            }
            Press::Height(by) => {
                next.height_px = stepped(next.height_px, u.line, by);
                Axes::of(Grip::Height)
            }
            Press::Largest => {
                (next.width_px, next.height_px) = (u32::MAX, u32::MAX);
                Axes::of(Grip::Both)
            }
        };
        self.moved(ui, fitted(next, limits(u)), axes);
        // A KEY IS WRITTEN AT ONCE: the next key reads what this one did.
        let moved = std::mem::take(&mut self.unsaved);
        if moved.any() {
            self.write(ui, moved);
        }
    }
}

impl Content for Curtain {
    type Hand = PerPointer;

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t(ronoctua_i18n::key::CMD_DEV_RESIZER)
    }

    /// OVER EVERYTHING, on the screen with the focus, and a zone that is its
    /// thickness — the room is what the zones leave. ON DEMAND: a click on it
    /// takes the keys, and being raised takes them from nobody who asked.
    fn open(&self, wiring: &Wiring<'_, Self>, surface: wl_surface::WlSurface) -> Result<Shell> {
        let layer_shell = base::layer_shell(wiring)?;
        let layer = layer_shell.create_layer_surface(
            wiring.qh,
            surface,
            Layer::Overlay,
            Some(NAMESPACE),
            None,
        );
        let px = curtain_px(self.edge, &self.state);
        match self.edge {
            Edge::Right => {
                layer.set_anchor(Anchor::TOP | Anchor::RIGHT | Anchor::BOTTOM);
                layer.set_size(px, 0);
            }
            Edge::Bottom => {
                layer.set_anchor(Anchor::LEFT | Anchor::RIGHT | Anchor::BOTTOM);
                layer.set_size(0, px);
            }
        }
        layer.set_exclusive_zone(zone_of(px));
        layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        layer.commit();
        self.told_px.set(px);
        self.layer.replace(Some(layer.clone()));
        Ok(Shell::Layer(layer, layer_shell))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) = crate::door::keys::key_of(event, mods).and_then(press_of) {
            self.pressed(ui, press);
        }
    }

    /// NO HEADER, so no drawer for the right `Ctrl` to open and no drawer
    /// for `Esc` to shut before the way out.
    fn header_keys(&self) -> bool {
        false
    }

    /// NO SWEEP OF TEXT: a held button here is the drag.
    fn text_area(&self, _: &Ui, _: u16, _: u16) -> Option<crate::text_select::Area> {
        None
    }

    /// Every configure: the curtain's size, which curtain spans the corner
    /// (read off `configured` wherever it is needed), and whether the screen
    /// under the cut changed its size. Where the curtain now sits on the
    /// screen is not kept: a drag does not need it (`Drag::delta`).
    ///
    /// A LONG AXIS THE CUT DOES NOT EXPLAIN is asked about in two steps. The
    /// file first: the other curtain moved and this configure is its commit,
    /// which it wrote before it made. Only what the file does not explain
    /// either is a screen that changed, and costs a question to the
    /// compositor.
    fn resized(&mut self, ui: &mut Ui) {
        self.configured = (ui.width, ui.height);
        if needs_measuring(self.edge, &self.state, self.configured) {
            self.file_changed(ui);
            if !ui.quit && needs_measuring(self.edge, &self.state, self.configured) {
                self.measure_again(ui);
            }
        }
        tracing::debug!(
            edge = self.edge.word(),
            width = ui.width,
            height = ui.height,
            corner = owns_corner(self.edge, &self.state, self.configured),
            "configured"
        );
    }

    /// What a drag moved, written once per turn however many motions it had.
    fn each_turn(&mut self, ui: &mut Ui) {
        let moved = std::mem::take(&mut self.unsaved);
        if moved.any() {
            self.write(ui, moved);
        }
    }

    fn frame_done(&mut self, _: &mut Ui) {
        if std::mem::take(&mut self.settle_asked) {
            self.settle_the_windows();
        }
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<PerPointer>, events: &[PointerEvent]) {
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let at = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if let Some(d) = hand.extra.drag.clone() {
                        let to = d.to(&self.state, e.position, limits(Units::now()));
                        self.moved(ui, to, Axes::of(d.grip));
                        continue;
                    }
                    let mark = self.mark_at(e.position).map(|m| m.id);
                    let grip = if mark.is_some() {
                        None
                    } else {
                        grip_at(&self.grips_now(), at)
                    };
                    if (mark, grip) != (self.hint_lit, self.lit) {
                        (self.hint_lit, self.lit) = (mark, grip);
                        ui.ask_paint();
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.hint_lit = None;
                    self.lit = None;
                    ui.ask_paint();
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    // A SHORTCUT OF THE LINE IS A CLICK: pressed, and no drag
                    // starts under it.
                    if let Some(act) = self.mark_at(e.position).and_then(|m| m.press) {
                        if let crate::chrome::Act::Key(key) = act {
                            let (event, mods) = crate::door::keys::event_of(key);
                            self.typed(ui, &event, mods);
                        }
                        continue;
                    }
                    if let Some(grip) = grip_at(&self.grips_now(), at) {
                        hand.extra.drag = Some(Drag {
                            grip,
                            from: self.state.clone(),
                            pressed_local: e.position,
                        });
                        self.lit = Some(grip);
                        ui.ask_paint();
                    }
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    if hand.extra.drag.take().is_some() {
                        ui.ask_paint();
                    }
                }
                // THE WHEEL STEPS the axis under it, a unit per line of wheel:
                // down and right grow the room.
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::surface::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
                    let by = crate::surface::wheel_lines(&mut hand.lines_owed);
                    let Some(grip) = grip_at(&self.grips_now(), at).filter(|_| by != 0) else {
                        continue;
                    };
                    let u = Units::now();
                    let mut next = self.state.clone();
                    let axes = Axes::of(grip);
                    if axes.width {
                        next.width_px = stepped(next.width_px, u.cell, by);
                    }
                    if axes.height {
                        next.height_px = stepped(next.height_px, u.line, by);
                    }
                    self.moved(ui, fitted(next, limits(u)), axes);
                }
                _ => {}
            }
        }
        self.dragging = hand.extra.drag.as_ref().map(|d| d.grip);
    }

    #[allow(clippy::cast_precision_loss)]
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        canvas.under.push(quad(
            0.0,
            0.0,
            ui.width as f32,
            ui.height as f32,
            tokens::color::SURFACE_BASE,
        ));
        self.marks.clear();
        if self.edge == Edge::Bottom {
            self.paint_the_line(ui, canvas);
        }
        let lit = self.dragging.or(self.lit);
        canvas.over.extend(edges(&self.grips_now(), lit));

        // THE FRAME THAT MAPS THE CURTAIN asks to be told when it is SHOWN:
        // only then has the compositor counted its zone, and only then is
        // there a layout to send the windows (`frame_done`).
        if std::mem::take(&mut self.settle_first) {
            let _ = ui
                .surface
                .frame(&self.qh, FrameCallbackData(ui.surface.clone()));
            self.settle_asked = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cut(screen: (u32, u32), room: (u32, u32)) -> State {
        State {
            output: "DP-1".to_owned(),
            screen_width_px: screen.0,
            screen_height_px: screen.1,
            width_px: room.0,
            height_px: room.1,
        }
    }

    fn units() -> Units {
        Units {
            cell: 10.8,
            line: 26.1,
        }
    }

    /// FAILS IF: an arrow moves the room by anything but one whole column or
    /// line — a step that lands a hair short of the grid moves the footer's
    /// count by nothing, and one a hair past it by two.
    #[test]
    fn a_step_moves_the_grid_by_exactly_one_unit() {
        for unit in [7.2_f32, 9.6, 10.8, 23.2, 26.1] {
            for px in (0..2400_u32).step_by(37).chain([144, 1896, 1080, 2160]) {
                for by in [-10_i64, -1, 1, 10] {
                    let out = stepped(px, unit, by);
                    let wanted = (count(px, unit) + by).max(0);
                    assert_eq!(
                        count(out, unit),
                        wanted,
                        "{px} px by {by} of {unit} gave {out} px"
                    );
                    // AND THE SMALLEST SUCH SIZE: the room stays on the grid.
                    if out > 0 {
                        assert!(count(out - 1, unit) < wanted, "{out} px is not the least");
                    }
                }
            }
        }
    }

    /// FAILS IF: the room goes under its floor or past what the curtains
    /// need to stay grabbable — and a screen smaller than the floor keeps
    /// the curtains rather than the floor.
    #[test]
    fn the_screen_stays_between_its_floor_and_the_curtains() {
        let u = units();
        let l = limits(u);
        assert_eq!(
            count(l.min_width_px, u.cell),
            i64::from(tokens::space::RESIZER_MINCOLUMNS)
        );
        assert_eq!(
            count(l.min_height_px, u.line),
            i64::from(tokens::space::RESIZER_MINLINES)
        );
        assert_eq!(l.bottom_min_px, 27, "one line, rounded up");
        let small = fitted(cut((1920, 1080), (1, 1)), l);
        assert_eq!(
            (small.width_px, small.height_px),
            (l.min_width_px, l.min_height_px)
        );
        let big = fitted(cut((1920, 1080), (5000, 5000)), l);
        assert_eq!(
            (big.width_px, big.height_px),
            (1920 - l.right_min_px, 1080 - l.bottom_min_px)
        );
        assert_eq!(largest("DP-1".into(), 1920, 1080, l), big);
        // A SCREEN TOO SMALL FOR THE FLOOR: the curtains stay.
        let tiny = fitted(cut((100, 100), (50, 50)), l);
        assert_eq!(
            (tiny.width_px, tiny.height_px),
            (100 - l.right_min_px, 100 - l.bottom_min_px)
        );
    }

    /// FAILS IF: a curtain is not exactly what the room leaves of its axis.
    #[test]
    fn each_curtain_takes_what_the_screen_does_not() {
        let s = cut((1920, 1080), (1500, 900));
        assert_eq!(curtain_px(Edge::Right, &s), 420);
        assert_eq!(curtain_px(Edge::Bottom, &s), 180);
        assert_eq!(zone_of(curtain_px(Edge::Right, &s)), 420);
    }

    /// A hand held on a curtain, as the pinned compositor reports it:
    /// pressed at `press` in the curtain's pixels, and then the cursor moved
    /// by each of `cursor` in turn. After every motion the curtain writes,
    /// commits and is configured at its new thickness — and the next motion
    /// is STILL `press + cursor movement` (`seatop_down` keeps the grab's
    /// frame and never re-bases it). Answers the cut after each motion.
    fn held(from: &State, grip: Grip, press: (f64, f64), cursor: &[(f64, f64)]) -> Vec<State> {
        let l = limits(units());
        let d = Drag {
            grip,
            from: from.clone(),
            pressed_local: press,
        };
        let mut now = from.clone();
        cursor
            .iter()
            .map(|c| {
                now = d.to(&now, (press.0 + c.0, press.1 + c.1), l);
                now.clone()
            })
            .collect()
    }

    /// FAILS IF: the drag counts the curtain's own movement. The compositor
    /// reports a held pointer in the grab's frame, so a curtain that adds
    /// where it now sits on top counts every step again — the right one
    /// pressed at its local 1 and dragged 10 px left went 1790, 1779, 1767…
    /// under a hand that had stopped, and the room slammed into its floor.
    #[test]
    fn a_drag_follows_the_hand_and_not_the_curtain_that_moves() {
        let l = limits(units());
        let from = cut((1920, 1080), (1800, 1000));
        assert_eq!(
            held(&from, Grip::Width, (1.0, 300.0), &[(0.0, 0.0)]),
            vec![from.clone()],
            "a press alone moves nothing"
        );
        // THE RIGHT ONE: 10 px left, and then a hand that stays there over
        // five configures — one width, every time.
        let still = held(&from, Grip::Width, (1.0, 300.0), &[(-10.0, 0.0); 6]);
        assert!(
            still
                .iter()
                .all(|s| s.width_px == 1790 && s.height_px == 1000),
            "a steady hand ran away: {:?}",
            still.iter().map(|s| s.width_px).collect::<Vec<_>>()
        );
        // AND A HAND THAT GOES ON is where the hand is, not the sum of the
        // steps: 10, 20, 35 px left and back to 5.
        let going = held(
            &from,
            Grip::Width,
            (1.0, 300.0),
            &[(-10.0, 0.0), (-20.0, 0.0), (-35.0, 0.0), (-5.0, 0.0)],
        );
        assert_eq!(
            going.iter().map(|s| s.width_px).collect::<Vec<_>>(),
            vec![1790, 1780, 1765, 1795]
        );
        // THE BOTTOM ONE, by its top edge: 40 px up, held.
        let up = held(&from, Grip::Height, (300.0, 1.0), &[(0.0, -40.0); 4]);
        assert!(
            up.iter().all(|s| s.height_px == 960 && s.width_px == 1800),
            "{:?}",
            up.iter()
                .map(|s| (s.width_px, s.height_px))
                .collect::<Vec<_>>()
        );
        // THE CORNER MOVES BOTH, and holds both.
        let corner = held(&from, Grip::Both, (5.0, 5.0), &[(-100.0, -50.0); 4]);
        assert!(
            corner
                .iter()
                .all(|s| (s.width_px, s.height_px) == (1700, 950)),
            "{:?}",
            corner
                .iter()
                .map(|s| (s.width_px, s.height_px))
                .collect::<Vec<_>>()
        );
        // AND THE AXIS NOT DRAGGED is whatever the other curtain made it
        // since the press.
        let d = Drag {
            grip: Grip::Width,
            from,
            pressed_local: (1.0, 300.0),
        };
        let other = cut((1920, 1080), (1790, 700));
        let to = d.to(&other, (-19.0, 300.0), l);
        assert_eq!((to.width_px, to.height_px), (1780, 700));
    }

    /// Where a curtain of `configured` size sits on the screen.
    fn origin(edge: Edge, s: &State, configured: (u32, u32)) -> (f64, f64) {
        match edge {
            Edge::Right => (f64::from(s.screen_width_px) - f64::from(configured.0), 0.0),
            Edge::Bottom => (0.0, f64::from(s.screen_height_px) - f64::from(configured.1)),
        }
    }

    /// Every grip's edges, on the screen, as whole pixels.
    #[allow(clippy::cast_possible_truncation)]
    fn drawn_on_screen(edge: Edge, s: &State, configured: (u32, u32)) -> Vec<(i64, i64, i64, i64)> {
        let o = origin(edge, s, configured);
        edges(&grips(edge, s, configured), None)
            .iter()
            .map(|q| {
                (
                    (f64::from(q.x) + o.0).round() as i64,
                    (f64::from(q.y) + o.1).round() as i64,
                    f64::from(q.width).round() as i64,
                    f64::from(q.height).round() as i64,
                )
            })
            .collect()
    }

    /// FAILS IF: both curtains or neither claim the corner square, or the
    /// picture depends on which one the compositor arranged first.
    #[test]
    fn the_corner_belongs_to_the_curtain_that_spans_it() {
        let s = cut((1920, 1080), (1800, 1000));
        // THE RIGHT ONE ARRANGED FIRST: it is the whole height.
        let right_first = [(Edge::Right, (120, 1080)), (Edge::Bottom, (1800, 80))];
        // THE BOTTOM ONE FIRST: it is the whole width.
        let bottom_first = [(Edge::Right, (120, 1000)), (Edge::Bottom, (1920, 80))];
        let mut pictures = Vec::new();
        for order in [right_first, bottom_first] {
            let owners = order
                .iter()
                .filter(|(e, c)| owns_corner(*e, &s, *c))
                .count();
            assert_eq!(owners, 1, "{order:?}");
            let mut drawn: Vec<_> = order
                .iter()
                .flat_map(|(e, c)| drawn_on_screen(*e, &s, *c))
                .collect();
            drawn.sort_unstable();
            pictures.push(drawn);
            // AND EVERY PIXEL OF EACH CURTAIN IS SOME GRIP'S.
            for (e, c) in order {
                let area: f32 = grips(e, &s, c).iter().map(|(_, b)| b.2 * b.3).sum();
                #[allow(clippy::cast_precision_loss)]
                let whole = (c.0 * c.1) as f32;
                assert!((area - whole).abs() < 0.5, "{e:?} {c:?}");
            }
        }
        assert_eq!(pictures[0], pictures[1], "the corner changed the drawing");
        assert!(owns_corner(Edge::Right, &s, (120, 1080)));
        assert!(!owns_corner(Edge::Right, &s, (120, 1000)));
    }

    /// FAILS IF: an edge is drawn where its grip does not take the pointer,
    /// or the lit one is not the one under the pointer.
    #[test]
    fn a_grip_is_taken_where_it_is_drawn() {
        let s = cut((1920, 1080), (1800, 1000));
        for (edge, configured) in [
            (Edge::Right, (120, 1080)),
            (Edge::Right, (120, 1000)),
            (Edge::Bottom, (1920, 80)),
            (Edge::Bottom, (1800, 80)),
        ] {
            let all = grips(edge, &s, configured);
            for (grip, b) in &all {
                let lines = edges(&[(*grip, *b)], Some(*grip));
                assert!(!lines.is_empty(), "{grip:?} draws nothing");
                for q in lines {
                    assert!(
                        q.x >= b.0
                            && q.y >= b.1
                            && q.x + q.width <= b.0 + b.2
                            && q.y + q.height <= b.1 + b.3,
                        "{grip:?}'s edge {q:?} lies outside {b:?}"
                    );
                    let middle = (q.x + q.width / 2.0, q.y + q.height / 2.0);
                    assert_eq!(grip_at(&all, middle), Some(*grip), "{edge:?} {q:?}");
                    assert_eq!(q.color, tokens::color::CAPTURE_GUIDE);
                }
            }
            assert!(
                edges(&all, None)
                    .iter()
                    .all(|q| q.color == tokens::color::RULE_STRONG),
                "an edge lit with nothing under the pointer"
            );
        }
    }

    /// FAILS IF: a screen that changed its size loses the room it was given
    /// where the room still fits — or keeps one that no longer does.
    #[test]
    fn a_screen_that_changed_size_keeps_the_room_it_was_given() {
        let l = limits(units());
        let s = cut((1920, 1080), (1500, 900));
        let bigger = remeasured(s.clone(), (2560, 1440), l);
        assert_eq!((bigger.width_px, bigger.height_px), (1500, 900));
        assert_eq!(
            (bigger.screen_width_px, bigger.screen_height_px),
            (2560, 1440)
        );
        let smaller = remeasured(s.clone(), (1280, 720), l);
        assert_eq!(
            (smaller.width_px, smaller.height_px),
            (1280 - l.right_min_px, 720 - l.bottom_min_px)
        );
        // AND ONLY A CONFIGURE THAT IS NEITHER THE SCREEN NOR THE ROOM asks.
        assert!(!needs_measuring(Edge::Right, &s, (420, 1080)));
        assert!(!needs_measuring(Edge::Right, &s, (420, 900)));
        assert!(
            !needs_measuring(Edge::Right, &s, (420, 899)),
            "the door's rounding"
        );
        assert!(needs_measuring(Edge::Right, &s, (420, 1440)));
        assert!(!needs_measuring(Edge::Bottom, &s, (1920, 180)));
        assert!(needs_measuring(Edge::Bottom, &s, (1280, 180)));
    }

    /// FAILS IF: a key the line teaches does something else than what its
    /// words say, or `Shift` stops being the big stride.
    #[test]
    fn every_key_the_line_teaches_is_the_key_that_acts() {
        let big = i64::from(tokens::space::RESIZER_BIGSTEP);
        for (notation, id) in OF_THE_RESIZER {
            let key = Key::parse(notation).expect(notation);
            let wanted = match *id {
                "resizer-off" => Press::Off,
                "resizer-narrower" => Press::Width(-1),
                "resizer-wider" => Press::Width(1),
                "resizer-shorter" => Press::Height(-1),
                "resizer-taller" => Press::Height(1),
                "resizer-largest" => Press::Largest,
                other => panic!("`{other}` is taught and means nothing here"),
            };
            assert_eq!(press_of(key), Some(wanted), "{notation}");
        }
        for (notation, wanted) in [
            ("S-<left>", Press::Width(-big)),
            ("S-<right>", Press::Width(big)),
            ("S-<up>", Press::Height(-big)),
            ("S-<down>", Press::Height(big)),
        ] {
            assert_eq!(press_of(Key::parse(notation).unwrap()), Some(wanted));
        }
        assert_eq!(press_of(Key::parse("C-<left>").unwrap()), None);
        // AND THE LINE'S CLICKS ARE THOSE KEYS, `ESC` first.
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let line = line_of(&cut((1920, 1080), (1500, 900)), None, &texts);
        let pressed: Vec<&str> = line
            .iter()
            .filter(|p| p.is_a_key())
            .filter_map(|p| p.id)
            .collect();
        assert_eq!(
            pressed,
            OF_THE_RESIZER.iter().map(|(_, id)| *id).collect::<Vec<_>>()
        );
        for p in &line {
            if let (Some(id), Some(crate::chrome::Act::Key(k))) = (p.id, p.press) {
                let row = OF_THE_RESIZER.iter().find(|(_, i)| *i == id).unwrap();
                assert_eq!(Some(k), Key::parse(row.0).ok(), "{id}");
            }
        }
        let said: String = line.iter().map(|p| p.text.as_str()).collect();
        assert!(said.contains("1500") && said.contains("900"), "{said}");
    }

    /// FAILS IF: a shortcut of the bottom curtain's line is laid out over the
    /// corner square when that curtain spans it — a press there, where a hand
    /// goes to resize both axes at once, stepped the room one column instead
    /// (seen in the VM: a press two pixels into the corner). The line is laid
    /// out on the room's width, so every clickable cell sits under the room.
    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn the_corner_is_a_grip_and_not_a_shortcut() {
        let s = cut((1024, 768), (686, 653));
        let configured = (1024, 115);
        assert!(owns_corner(Edge::Bottom, &s, configured));
        let cell = tokens::metrics::cell_width(tokens::text::size());
        let margin = 1;
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let pieces = line_of(&s, None, &texts);
        let reach = |columns: u16| -> f32 {
            crate::chrome::hint_marks(&pieces, margin, 0, columns)
                .iter()
                .map(|m| f32::from(m.at + m.cells) * cell)
                .fold(0.0, f32::max)
        };
        // THE PRECONDITION: laid out on the whole curtain, the line does
        // reach into the corner — otherwise this test proves nothing.
        assert!(reach(columns_in(1024.0, margin)) > 686.0);
        let width = line_width(&s, configured, configured);
        assert!((width - 686.0).abs() < f32::EPSILON, "{width}");
        let edge = reach(columns_in(width, margin));
        assert!(edge <= 686.0, "a shortcut reaches {edge} px, past the room");
        // AND WHEN THE OTHER CURTAIN SPANS THE CORNER, the line is the whole
        // curtain, which is only as wide as the room anyway.
        assert!((line_width(&s, (686, 115), (686, 115)) - 686.0).abs() < f32::EPSILON);
    }

    /// FAILS IF: the sentence of the key under the pointer is put where the
    /// line is cut — after the whole line it started past the 110th cell,
    /// and in the narrow rooms this tool exists to make, pointing at a key
    /// explained nothing — or anything up to that key moves when it lights.
    #[test]
    fn the_key_under_the_pointer_is_explained_where_the_line_is_seen() {
        let width = |pieces: &[Piece]| -> u16 {
            pieces
                .iter()
                .map(|p| ronoctua_render::layout::width(&p.text))
                .sum()
        };
        let s = cut((1920, 1080), (400, 900));
        for language in ["en-GB", "pt-BR"] {
            let texts = ronoctua_i18n::Texts::fresh(language).expect(language);
            let rest = line_of(&s, None, &texts);
            for (_, id) in OF_THE_RESIZER {
                let line = line_of(&s, Some(id), &texts);
                let sentence = crate::chrome::hint_sentence(id, &texts).expect(id);
                let at = line
                    .iter()
                    .position(|p| p.register == Register::Explain)
                    .unwrap_or_else(|| panic!("{language} {id}: no sentence"));
                assert_eq!(line[at].text, sentence, "{language} {id}");
                // RIGHT AFTER THE KEY: its last run, then the dot.
                let key_ends = rest.iter().rposition(|p| p.id == Some(*id)).expect(id);
                assert_eq!(at, key_ends + 2, "{language} {id}");
                assert_eq!(line[at - 1].register, Register::Separator);
                // NOTHING UP TO IT MOVED: the same runs, the same cells.
                let before: Vec<&str> = line[..=key_ends].iter().map(|p| p.text.as_str()).collect();
                let unlit: Vec<&str> = rest[..=key_ends].iter().map(|p| p.text.as_str()).collect();
                assert_eq!(before, unlit, "{language} {id}");
            }
            // THE WAY OUT, in a room of 40 columns: its sentence is drawn
            // inside them, where the old line started it past the 110th.
            let line = line_of(&s, Some("resizer-off"), &texts);
            let at = line
                .iter()
                .position(|p| p.register == Register::Explain)
                .unwrap();
            let starts = width(&line[..at]);
            assert!(
                starts < 40,
                "{language}: the sentence starts at cell {starts}"
            );
            let drawn = crate::chrome::hint_spans(&line, ronoctua_render::Size::Grid, 0, 0, 40);
            assert!(
                drawn
                    .iter()
                    .any(|sp| sp.column == starts && !sp.text.is_empty()),
                "{language}: nothing of the sentence is drawn in 40 columns"
            );
        }
    }

    /// FAILS IF: a recycled pid is taken for the keeper — `stop` would send
    /// somebody else's process `TERM`.
    #[test]
    fn only_the_keeper_is_the_keeper() {
        assert!(is_the_keeper(b"/nix/store/x/bin/ronoctua-shell\0resizer\0"));
        assert!(is_the_keeper(
            b"/nix/store/x/bin/.ronoctua-shell-wrapped\0resizer\0"
        ));
        assert!(!is_the_keeper(
            b"/nix/store/x/bin/ronoctua-shell\0resizer-curtain\0right\0"
        ));
        assert!(!is_the_keeper(b"/nix/store/x/bin/ronoctua-shell\0column\0"));
        assert!(!is_the_keeper(b"/usr/bin/zsh\0resizer\0"));
        assert!(!is_the_keeper(b""));
        assert_eq!(Edge::of_word("right"), Some(Edge::Right));
        assert_eq!(Edge::of_word("bottom"), Some(Edge::Bottom));
        assert_eq!(Edge::of_word("left"), None);
    }

    /// A file of this test's own, in the temporary directory.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-resizer-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(FILE)
    }

    /// FAILS IF: a writer writes over a file it could not read — which would
    /// turn a resizer that was ended back on, or put a made-up cut over one
    /// somebody else wrote.
    #[test]
    fn a_file_that_does_not_parse_is_never_written_over() {
        let p = scratch("parse");
        assert_eq!(update_at(&p, |s| s.width_px = 1).unwrap(), None);
        assert!(!p.exists(), "a missing file was written");
        std::fs::write(&p, b"{ not json").unwrap();
        assert_eq!(update_at(&p, |s| s.width_px = 1).unwrap(), None);
        assert_eq!(std::fs::read(&p).unwrap(), b"{ not json");
        // AND ONE THAT READS IS CHANGED BY FIELD: the other curtain's axis
        // stays what the file said.
        let file = cut((1920, 1080), (1500, 900));
        write_at(&p, &file).unwrap();
        let mine = cut((1920, 1080), (1400, 1));
        let written = update_at(&p, |s| {
            *s = merged(&mine, s.clone(), Axes::of(Grip::Width));
        })
        .unwrap()
        .unwrap();
        assert_eq!((written.width_px, written.height_px), (1400, 900));
        assert_eq!(read_at(&p), Some(written));
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    /// FAILS IF: the footer misses a keeper's failure, or takes an ordinary
    /// end for one.
    #[test]
    fn the_keepers_last_word_is_read() {
        assert_eq!(last_word(""), Heard::Ended);
        assert_eq!(
            last_word("failed no screen has the focus\n"),
            Heard::Failed("no screen has the focus".into())
        );
        assert!(verdict(Edge::Right, Some(0), None).is_ok());
        assert!(
            verdict(
                Edge::Right,
                None,
                Some(rustix::process::Signal::TERM.as_raw())
            )
            .is_ok(),
            "a curtain ended by the keeper is no failure"
        );
        let e = verdict(Edge::Bottom, Some(1), None).unwrap_err();
        assert!(e.to_string().contains("bottom"), "{e}");
    }

    /// FAILS IF: the footer's ceiling is measured on a screen the resizer
    /// cut, or the cut of one screen reaches another.
    #[test]
    fn the_room_is_the_height_of_the_screen_that_was_cut() {
        let s = cut((1920, 1080), (1500, 600));
        assert_eq!(room_height(1080, Some("DP-1"), Some(&s)), 600);
        assert_eq!(room_height(1080, Some("HDMI-1"), Some(&s)), 1080);
        assert_eq!(room_height(1080, None, Some(&s)), 1080);
        assert_eq!(room_height(1080, Some("DP-1"), None), 1080);
    }

    /// FAILS IF: the footer's ceiling is measured on the monitor of a screen
    /// the resizer cut, on a screen it did not cut, or on a screen of no
    /// height — the fold `Footer::expansion_ceiling` runs over its screens.
    #[test]
    fn the_shortest_room_is_measured_on_every_screen() {
        let s = cut((3840, 2160), (1000, 600));
        let named = |h: u32, n: &str| (h, Some(n.to_owned()));
        // THE CUT SCREEN IS THE SHORTEST, though its monitor is the tallest.
        assert_eq!(
            smallest_room([named(2160, "DP-1"), named(1080, "HDMI-1")], Some(&s)),
            Some(600)
        );
        // AN UNCUT SCREEN SHORTER THAN THE ROOM is still the one in charge.
        assert_eq!(
            smallest_room([named(2160, "DP-1"), named(500, "HDMI-1")], Some(&s)),
            Some(500)
        );
        // A CUT NAMING A SCREEN THAT IS NOT THERE reaches none of them.
        assert_eq!(
            smallest_room([named(2160, "HDMI-1"), named(1080, "HDMI-2")], Some(&s)),
            Some(1080)
        );
        // A SCREEN NOT YET NAMED keeps its monitor, and none of no height
        // counts.
        assert_eq!(
            smallest_room([(2160, None), named(0, "DP-1")], Some(&s)),
            Some(2160)
        );
        assert_eq!(smallest_room([named(2160, "DP-1")], None), Some(2160));
        assert_eq!(smallest_room(Vec::new(), Some(&s)), None);
    }

    /// FAILS IF: a column's width is capped by the monitor of a screen the
    /// resizer cut — or the cut of one screen narrows a column on another.
    #[test]
    fn the_room_is_the_width_of_the_screen_that_was_cut() {
        let s = cut((1920, 1080), (700, 900));
        assert_eq!(room_width(1920, Some("DP-1"), Some(&s)), 700);
        assert_eq!(room_width(1920, Some("HDMI-1"), Some(&s)), 1920);
        assert_eq!(room_width(1920, None, Some(&s)), 1920);
        assert_eq!(room_width(1920, Some("DP-1"), None), 1920);
    }
}
