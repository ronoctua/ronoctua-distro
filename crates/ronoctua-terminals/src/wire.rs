//! The holder's socket — one table for both directions.
//!
//! # The first line, then frames
//!
//! Every connection opens with ONE line of JSON, an [`Ask`], written with one
//! `write_all`. Most asks are answered with one line and the connection
//! ends ([`Ask::State`] answers the [`crate::state::TerminalState`], the
//! orders answer [`DONE_LINE`]); [`Ask::Watch`] answers a state line on every
//! change, as long as the connection lasts. [`Ask::Attach`] turns the
//! connection into frames: [`ToViewer`] one way, [`ToHolder`] the other.
//!
//! # A frame
//!
//! `u32 length, little-endian | u8 kind | payload` — the length counts the
//! payload only. Built whole and written with ONE `write_all`
//! ([`ToViewer::encode`], [`ToHolder::encode`]): serialised straight into a
//! socket, every token would be a write, and a reader on the other side could
//! receive half a frame and a scheduling later the rest. Control payloads are
//! JSON; [`ToViewer::Snapshot`] and [`ToViewer::Bytes`] are raw, and
//! [`ToHolder::Text`] is UTF-8.
//!
//! # Not always the whole message
//!
//! The daemons of this system send whole messages, never deltas. The frames
//! after an attach are the one stream that is not: [`ToViewer::Bytes`] is
//! what the holder's engine was fed, in order, and a viewer that misses one
//! has a wrong screen — which is why a viewer that falls behind is told
//! [`ToViewer::Lagging`] and reattaches with a fresh snapshot instead of
//! being caught up. Everything ELSE a viewer learns is whole: the snapshot,
//! each [`ToViewer::State`], each [`ToViewer::Colors`].
//!
//! # What the viewer may say
//!
//! Semantic input only — a key, a mouse event, a paste, the text an input
//! method committed — and the holder encodes it against the modes the
//! program actually set. There is no frame of raw bytes for the PTY: "a
//! window never writes to the program" holds by construction, and a viewer
//! whose mirror is a frame behind the holder's modes cannot encode a key the
//! wrong way.

use std::io::{self, BufRead, Read};

use serde::{Deserialize, Serialize};

use crate::state::TerminalState;
use crate::{GroupId, TerminalId};

/// The largest payload a frame may carry. Larger is a broken peer, refused
/// before it is read — the holder sends history in pieces far below this.
pub const FRAME_MAX_BYTES: usize = 64 << 20;

/// The longest first line (an attach carries the 256-colour palette, a few
/// kilobytes).
pub const ASK_MAX_BYTES: usize = 1 << 20;

/// The answer to an order ([`Ask::Front`], [`Ask::End`], [`Ask::Place`],
/// [`Ask::Detach`]): it was done.
pub const DONE_LINE: &[u8] = b"{\"done\":true}\n";

/// The first line of a connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "ask", rename_all = "kebab-case")]
pub enum Ask {
    /// The whole state now, and again on every change.
    Watch,
    /// The whole state, once.
    State,
    /// Become this terminal's window: frames from here on.
    Attach(Attach),
    /// Where the attached window shows it.
    Place(Place),
    /// Ask its window to put it in front ([`TerminalState::front_asked`]).
    Front,
    /// End it: the program is hung up, as when a terminal is closed.
    End,
    /// Send its window away; it goes to the background.
    Detach,
}

impl Ask {
    /// The line, newline included, built whole.
    ///
    /// # Errors
    ///
    /// Serialisation failed, which these types do not.
    pub fn line(&self) -> serde_json::Result<Vec<u8>> {
        let mut out = serde_json::to_vec(self)?;
        out.push(b'\n');
        Ok(out)
    }

    /// An ask, out of its line.
    ///
    /// # Errors
    ///
    /// The line is not an ask.
    pub fn parse(line: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(line)
    }
}

/// A terminal's size: cells, and the pixels of one cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    /// Columns.
    pub columns: u16,
    /// Rows.
    pub rows: u16,
    /// One cell's width, in physical pixels; 0 when not known.
    pub cell_width_px: u16,
    /// One cell's height, in physical pixels; 0 when not known.
    pub cell_height_px: u16,
}

impl Size {
    /// What a terminal is born with when nobody says: the size every
    /// terminal has meant since the VT100, pixels unknown.
    pub const BORN: Self = Self {
        columns: 80,
        rows: 24,
        cell_width_px: 0,
        cell_height_px: 0,
    };
}

/// A window attaching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attach {
    /// The window's [`crate::PROTOCOL`].
    pub protocol: u32,
    /// The engine build the window decodes with (`ronoctua_term::ghostty::
    /// ENGINE`): a holder of another build answers in the portable form.
    pub engine: String,
    /// The window's process.
    pub pid: u32,
    /// The size the window has for it; the holder resizes to it.
    pub size: Size,
    /// The colours of the window's page.
    pub colors: Colors,
    /// The group the window shows it in.
    pub group: GroupId,
    /// Its place in that group.
    pub order: u32,
}

/// Where a window shows its terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    /// It is the one in front.
    pub front: bool,
    /// It is in front and the window holds the keyboard.
    pub seen: bool,
    /// Its place in the group, when the window moved it: a window that ran
    /// out of room between two places numbers its group afresh, and each
    /// holder hears its new number here — not only at its next attach, which
    /// may come after the window is gone and the old numbers are all a
    /// window opened on the group would have. `None` (a window of the first
    /// protocol says none) keeps the place the holder has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<u32>,
}

/// An 8-bit colour, `#rrggbb` on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Rgb(pub [u8; 3]);

impl TryFrom<String> for Rgb {
    type Error = String;
    fn try_from(text: String) -> Result<Self, String> {
        let hex = text.strip_prefix('#').filter(|h| h.len() == 6);
        let channel = |i: usize| hex.and_then(|h| u8::from_str_radix(h.get(i..i + 2)?, 16).ok());
        match (channel(0), channel(2), channel(4)) {
            (Some(r), Some(g), Some(b)) => Ok(Self([r, g, b])),
            _ => Err(format!("{text:?} is not #rrggbb")),
        }
    }
}

impl From<Rgb> for String {
    fn from(c: Rgb) -> Self {
        let [r, g, b] = c.0;
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

/// Light or dark, for the programs that ask (mode 2031).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scheme {
    /// Dark text on a light ground.
    Light,
    /// Light text on a dark ground.
    Dark,
}

/// The colours a terminal starts from, before a program changes any: the
/// `terminal.*` roles of the window's page, resolved — data, not tokens,
/// by the time they cross.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Colors {
    /// The default foreground.
    pub foreground: Rgb,
    /// The default background.
    pub background: Rgb,
    /// The cursor, or `None` for the foreground.
    pub cursor: Option<Rgb>,
    /// The 256-colour palette — exactly 256 ([`Colors::palette_of_256`]).
    pub palette: Vec<Rgb>,
    /// Light or dark.
    pub scheme: Scheme,
}

impl Colors {
    /// The palette, when it has the 256 entries it must; `None` from a peer
    /// that sent another number, which is refused rather than padded.
    #[must_use]
    pub fn palette_of_256(&self) -> Option<&[Rgb; 256]> {
        self.palette.as_slice().try_into().ok()
    }
}

/// Modifier keys.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Mods {
    /// Shift.
    pub shift: bool,
    /// Control.
    pub ctrl: bool,
    /// Alt.
    pub alt: bool,
    /// Super.
    #[serde(rename = "super")]
    pub super_key: bool,
    /// Caps Lock is on.
    pub caps_lock: bool,
    /// Num Lock is on.
    pub num_lock: bool,
}

/// What happened to a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyAction {
    /// It went down.
    Press,
    /// It is held and the keyboard repeats it.
    Repeat,
    /// It came up.
    Release,
}

/// A key, as the window saw it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvent {
    /// The physical key's code (evdev's), as the compositor sent it.
    pub code: u32,
    /// The physical key by its W3C UI Events `code` (`KeyA`, `Enter`,
    /// `ArrowUp`) — what the holder encodes by.
    pub key: String,
    /// Press, repeat or release.
    pub action: KeyAction,
    /// The modifiers held.
    #[serde(default)]
    pub mods: Mods,
    /// The modifiers the layout spent producing `utf8` (Shift for `A`).
    #[serde(default)]
    pub consumed: Mods,
    /// The character the key gives with no modifier.
    #[serde(default)]
    pub unshifted: Option<char>,
    /// What the layout produced, before Ctrl or Alt changed it.
    #[serde(default)]
    pub utf8: String,
    /// It is part of a composition (a dead key, an input method).
    #[serde(default)]
    pub composing: bool,
}

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Button {
    /// The primary button.
    Left,
    /// The middle button.
    Middle,
    /// The secondary button.
    Right,
    /// The wheel up (button 4).
    WheelUp,
    /// The wheel down (button 5).
    WheelDown,
    /// The wheel left (button 6).
    WheelLeft,
    /// The wheel right (button 7).
    WheelRight,
    /// Back (button 8).
    Back,
    /// Forward (button 9).
    Forward,
}

/// What the mouse did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MouseAction {
    /// A button went down.
    Press,
    /// A button came up.
    Release,
    /// The pointer moved.
    Motion,
}

/// A mouse event for a program that tracks the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MouseEvent {
    /// Press, release or motion.
    pub action: MouseAction,
    /// The button, or `None` for motion with none.
    pub button: Option<Button>,
    /// The modifiers held.
    #[serde(default)]
    pub mods: Mods,
    /// Where, in physical pixels from the grid's top-left corner.
    pub x_px: f32,
    /// Where, in physical pixels from the grid's top-left corner.
    pub y_px: f32,
    /// A button is held, which decides what motion reports.
    #[serde(default)]
    pub any_button_held: bool,
}

/// Where a paste came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PasteSource {
    /// A clipboard: a key, a middle click.
    Clipboard,
    /// Text inserted another way: a dropped file's path.
    Text,
}

/// A paste.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paste {
    /// The text.
    pub text: String,
    /// Where it came from.
    pub source: PasteSource,
    /// The person was asked about a paste that could run a command, and said
    /// yes; without it such a paste is not written.
    #[serde(default)]
    pub allow_unsafe: bool,
}

/// A focus change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Focus {
    /// The window gained the keyboard (`false`: lost it).
    pub gained: bool,
}

/// Which clipboard a program means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClipboardKind {
    /// The ordinary clipboard.
    Standard,
    /// X11's selection (OSC 52 `s`).
    Selection,
    /// The primary selection (OSC 52 `p`).
    Primary,
}

/// One representation of a clipboard's value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardContent {
    /// Its MIME type.
    pub mime: String,
    /// Its bytes, base64 on the wire (an image is not text).
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
}

/// A program writing a clipboard (OSC 52) — sent only to an attached window,
/// which puts it there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardWrite {
    /// Which clipboard.
    pub clipboard: ClipboardKind,
    /// The representations, all of one value; empty clears it.
    pub contents: Vec<ClipboardContent>,
}

/// A program asking to READ a clipboard: the window asks the person, "no"
/// first, and answers [`ToHolder::Answer`] with the same id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consent {
    /// Which question — the answer names it.
    pub id: u64,
    /// Which clipboard.
    pub clipboard: ClipboardKind,
    /// The types it wants, preferred first.
    pub mimes: Vec<String>,
    /// The name the program gave, when the protocol carries one.
    #[serde(default)]
    pub program: String,
}

/// The window's answer to a [`Consent`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    /// The question's id.
    pub id: u64,
    /// The clipboard's text if the person allowed it; `None` refuses.
    pub text: Option<String>,
}

/// A mode the holder changed on its own (the end of a synchronized-output
/// hold it timed out), at its point in the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeChange {
    /// The mode's number.
    pub number: u16,
    /// An ANSI mode (`CSI n h`); `false` for a DEC private one (`CSI ? n h`).
    pub ansi: bool,
    /// Set or reset.
    pub on: bool,
}

/// How a terminal's program ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exit {
    /// Its exit code, a shell's way (128 plus the signal that killed it).
    pub code: i32,
}

/// What the holder sends after the snapshot it starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttachMode {
    /// A snapshot of the engine, which a window of the same build decodes.
    Exact,
    /// The terminal written out as VT sequences, for a window of another
    /// build or protocol: less exact, never unreadable.
    Portable,
}

/// The first frame after an attach. Its kind and shape never change, so a
/// window of any protocol can read it and tell whether to stay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attached {
    /// The holder's [`crate::PROTOCOL`].
    pub protocol: u32,
    /// What the snapshot frames hold.
    pub mode: AttachMode,
    /// Which terminal.
    pub id: TerminalId,
    /// Its size, which the snapshot is already at.
    pub size: Size,
}

/// The one table of frame kinds, both directions: holder to viewer below 64,
/// viewer to holder from 64, so a frame sent the wrong way is refused by its
/// number before its payload is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kind {
    /// [`ToViewer::Attached`].
    Attached = 1,
    /// [`ToViewer::Snapshot`].
    Snapshot = 2,
    /// [`ToViewer::Ready`].
    Ready = 3,
    /// [`ToViewer::Bytes`].
    Bytes = 4,
    /// [`ToViewer::Resize`].
    Resize = 5,
    /// [`ToViewer::Mode`].
    Mode = 6,
    /// [`ToViewer::Colors`].
    Colors = 7,
    /// [`ToViewer::Clipboard`].
    Clipboard = 8,
    /// [`ToViewer::Consent`].
    Consent = 9,
    /// [`ToViewer::State`].
    State = 10,
    /// [`ToViewer::Taken`].
    Taken = 11,
    /// [`ToViewer::Lagging`].
    Lagging = 12,
    /// [`ToViewer::Exited`].
    Exited = 13,
    /// [`ToViewer::PasteRefused`].
    PasteRefused = 14,
    /// [`ToHolder::Key`].
    Key = 64,
    /// [`ToHolder::Text`].
    Text = 65,
    /// [`ToHolder::Mouse`].
    Mouse = 66,
    /// [`ToHolder::Focus`].
    Focus = 67,
    /// [`ToHolder::Paste`].
    Paste = 68,
    /// [`ToHolder::Resize`].
    AskResize = 69,
    /// [`ToHolder::Colors`].
    AskColors = 70,
    /// [`ToHolder::Answer`].
    Answer = 71,
    /// [`ToHolder::Place`].
    Place = 72,
    /// [`ToHolder::Detach`].
    Detach = 73,
}

impl Kind {
    /// Every kind, for the table's own test.
    pub const ALL: [Self; 24] = [
        Self::Attached,
        Self::Snapshot,
        Self::Ready,
        Self::Bytes,
        Self::Resize,
        Self::Mode,
        Self::Colors,
        Self::Clipboard,
        Self::Consent,
        Self::State,
        Self::Taken,
        Self::Lagging,
        Self::Exited,
        Self::PasteRefused,
        Self::Key,
        Self::Text,
        Self::Mouse,
        Self::Focus,
        Self::Paste,
        Self::AskResize,
        Self::AskColors,
        Self::Answer,
        Self::Place,
        Self::Detach,
    ];

    /// The kind a byte names, if any.
    #[must_use]
    pub fn of(byte: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|k| *k as u8 == byte)
    }

    /// It goes from the holder to a viewer.
    #[must_use]
    pub const fn to_viewer(self) -> bool {
        (self as u8) < 64
    }
}

/// Why a frame could not be read.
#[derive(Debug)]
pub enum WireError {
    /// The byte names no kind.
    UnknownKind(u8),
    /// A kind of the other direction.
    WrongWay(Kind),
    /// The payload is not what the kind says.
    Payload(Kind, String),
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownKind(b) => write!(f, "frame kind {b} is no kind of this protocol"),
            Self::WrongWay(k) => write!(f, "a {k:?} frame came the wrong way"),
            Self::Payload(k, why) => write!(f, "a {k:?} frame's payload is not one: {why}"),
        }
    }
}

impl std::error::Error for WireError {}

/// A frame: header and payload, in one buffer, for one `write_all`.
fn frame(kind: Kind, payload: &[u8]) -> Vec<u8> {
    let len = u32::try_from(payload.len()).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(5 + payload.len());
    out.extend_from_slice(&len.to_le_bytes());
    out.push(kind as u8);
    out.extend_from_slice(payload);
    out
}

/// A frame whose payload is JSON.
fn json_frame<T: Serialize>(kind: Kind, value: &T) -> Vec<u8> {
    // These types are strings, numbers and lists: serialising them does not
    // fail, and an empty payload would be refused by the reader as it should.
    frame(kind, &serde_json::to_vec(value).unwrap_or_default())
}

/// A JSON payload, read as the kind says.
fn json<T: serde::de::DeserializeOwned>(kind: Kind, payload: &[u8]) -> Result<T, WireError> {
    serde_json::from_slice(payload).map_err(|e| WireError::Payload(kind, e.to_string()))
}

/// The kind of a frame that came this way.
fn kind_of(byte: u8, to_viewer: bool) -> Result<Kind, WireError> {
    let kind = Kind::of(byte).ok_or(WireError::UnknownKind(byte))?;
    if kind.to_viewer() == to_viewer {
        Ok(kind)
    } else {
        Err(WireError::WrongWay(kind))
    }
}

/// The [`ToViewer::Bytes`] frame of `bytes`, from a slice: the holder feeds
/// its engine the same bytes it frames, and owning them twice would be a copy
/// of every byte a program writes.
#[must_use]
pub fn bytes_frame(bytes: &[u8]) -> Vec<u8> {
    frame(Kind::Bytes, bytes)
}

/// What the holder sends an attached window.
#[derive(Debug, Clone, PartialEq)]
pub enum ToViewer {
    /// The first frame: what follows, at what size.
    Attached(Attached),
    /// A piece of the snapshot (or of the portable form).
    Snapshot(Vec<u8>),
    /// The snapshot is whole: decode it, and everything after is live.
    Ready,
    /// What the engine was fed, in order — after the holder's inbound filter,
    /// so a mirror sees exactly the bytes the engine did.
    Bytes(Vec<u8>),
    /// The holder resized its engine and the PTY HERE: resize the mirror at
    /// this point of the stream, never before.
    Resize(Size),
    /// The holder changed a mode on its own, here.
    Mode(ModeChange),
    /// The default colours changed, here.
    Colors(Colors),
    /// The program wrote a clipboard.
    Clipboard(ClipboardWrite),
    /// The program asks to read a clipboard: ask the person.
    Consent(Consent),
    /// The terminal's whole state, on every change.
    State(Box<TerminalState>),
    /// A newer window attached; this one no longer shows the terminal.
    Taken,
    /// This window fell too far behind and was let go: attach again.
    Lagging,
    /// The program ended.
    Exited(Exit),
    /// A paste the window sent was not written — and why: the window asks the
    /// person before a paste that could run a command, by the modes its
    /// MIRROR has, and the holder judges again by its own, which a program
    /// may have changed a frame before. A paste that vanished with only a line
    /// in the holder's log is one the person waits for.
    PasteRefused(PasteRefusal),
}

/// Why a paste was not written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PasteRefusal {
    /// It could run a command as it lands, and came without the person's yes.
    Unsafe,
    /// The engine refused it.
    Failed,
}

impl ToViewer {
    /// The frame's kind.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Attached(_) => Kind::Attached,
            Self::Snapshot(_) => Kind::Snapshot,
            Self::Ready => Kind::Ready,
            Self::Bytes(_) => Kind::Bytes,
            Self::Resize(_) => Kind::Resize,
            Self::Mode(_) => Kind::Mode,
            Self::Colors(_) => Kind::Colors,
            Self::Clipboard(_) => Kind::Clipboard,
            Self::Consent(_) => Kind::Consent,
            Self::State(_) => Kind::State,
            Self::Taken => Kind::Taken,
            Self::Lagging => Kind::Lagging,
            Self::Exited(_) => Kind::Exited,
            Self::PasteRefused(_) => Kind::PasteRefused,
        }
    }

    /// The whole frame, for one `write_all`.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let kind = self.kind();
        match self {
            Self::Attached(a) => json_frame(kind, a),
            Self::Snapshot(b) | Self::Bytes(b) => frame(kind, b),
            Self::Ready | Self::Taken | Self::Lagging => frame(kind, &[]),
            Self::Resize(s) => json_frame(kind, s),
            Self::Mode(m) => json_frame(kind, m),
            Self::Colors(c) => json_frame(kind, c),
            Self::Clipboard(c) => json_frame(kind, c),
            Self::Consent(c) => json_frame(kind, c),
            Self::State(s) => json_frame(kind, s),
            Self::Exited(e) => json_frame(kind, e),
            Self::PasteRefused(why) => json_frame(kind, why),
        }
    }

    /// A frame read off the socket.
    ///
    /// # Errors
    ///
    /// The kind is unknown or the viewer's own, or the payload is not what
    /// the kind says.
    pub fn decode(byte: u8, payload: Vec<u8>) -> Result<Self, WireError> {
        let kind = kind_of(byte, true)?;
        Ok(match kind {
            Kind::Attached => Self::Attached(json(kind, &payload)?),
            Kind::Snapshot => Self::Snapshot(payload),
            Kind::Ready => Self::Ready,
            Kind::Bytes => Self::Bytes(payload),
            Kind::Resize => Self::Resize(json(kind, &payload)?),
            Kind::Mode => Self::Mode(json(kind, &payload)?),
            Kind::Colors => Self::Colors(json(kind, &payload)?),
            Kind::Clipboard => Self::Clipboard(json(kind, &payload)?),
            Kind::Consent => Self::Consent(json(kind, &payload)?),
            Kind::State => Self::State(Box::new(json(kind, &payload)?)),
            Kind::Taken => Self::Taken,
            Kind::Lagging => Self::Lagging,
            Kind::Exited => Self::Exited(json(kind, &payload)?),
            Kind::PasteRefused => Self::PasteRefused(json(kind, &payload)?),
            _ => return Err(WireError::WrongWay(kind)),
        })
    }
}

/// What an attached window sends its holder.
#[derive(Debug, Clone, PartialEq)]
pub enum ToHolder {
    /// A key.
    Key(KeyEvent),
    /// Text an input method committed (or a composition's result).
    Text(String),
    /// A mouse event, for a program that tracks the mouse.
    Mouse(MouseEvent),
    /// The window gained or lost the keyboard.
    Focus(Focus),
    /// A paste.
    Paste(Paste),
    /// The window's size for the terminal changed: the holder resizes and
    /// answers [`ToViewer::Resize`] at the byte it did.
    Resize(Size),
    /// The page's colours changed (a theme): answered [`ToViewer::Colors`].
    Colors(Colors),
    /// The person's answer to a [`Consent`].
    Answer(Answer),
    /// Where the window shows it.
    Place(Place),
    /// The window lets it go to the background.
    Detach,
}

impl ToHolder {
    /// The frame's kind.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Key(_) => Kind::Key,
            Self::Text(_) => Kind::Text,
            Self::Mouse(_) => Kind::Mouse,
            Self::Focus(_) => Kind::Focus,
            Self::Paste(_) => Kind::Paste,
            Self::Resize(_) => Kind::AskResize,
            Self::Colors(_) => Kind::AskColors,
            Self::Answer(_) => Kind::Answer,
            Self::Place(_) => Kind::Place,
            Self::Detach => Kind::Detach,
        }
    }

    /// The whole frame, for one `write_all`.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let kind = self.kind();
        match self {
            Self::Key(k) => json_frame(kind, k),
            Self::Text(t) => frame(kind, t.as_bytes()),
            Self::Mouse(m) => json_frame(kind, m),
            Self::Focus(f) => json_frame(kind, f),
            Self::Paste(p) => json_frame(kind, p),
            Self::Resize(s) => json_frame(kind, s),
            Self::Colors(c) => json_frame(kind, c),
            Self::Answer(a) => json_frame(kind, a),
            Self::Place(p) => json_frame(kind, p),
            Self::Detach => frame(kind, &[]),
        }
    }

    /// A frame read off the socket.
    ///
    /// # Errors
    ///
    /// The kind is unknown or the holder's own, or the payload is not what
    /// the kind says.
    pub fn decode(byte: u8, payload: Vec<u8>) -> Result<Self, WireError> {
        let kind = kind_of(byte, false)?;
        Ok(match kind {
            Kind::Key => Self::Key(json(kind, &payload)?),
            Kind::Text => Self::Text(
                String::from_utf8(payload).map_err(|e| WireError::Payload(kind, e.to_string()))?,
            ),
            Kind::Mouse => Self::Mouse(json(kind, &payload)?),
            Kind::Focus => Self::Focus(json(kind, &payload)?),
            Kind::Paste => Self::Paste(json(kind, &payload)?),
            Kind::AskResize => Self::Resize(json(kind, &payload)?),
            Kind::AskColors => Self::Colors(json(kind, &payload)?),
            Kind::Answer => Self::Answer(json(kind, &payload)?),
            Kind::Place => Self::Place(json(kind, &payload)?),
            Kind::Detach => Self::Detach,
            _ => return Err(WireError::WrongWay(kind)),
        })
    }
}

/// Reads one frame: its kind byte and payload, or `None` at a clean end
/// (the peer closed between two frames).
///
/// # Errors
///
/// The stream ended inside a frame, a read failed, or the length is past
/// [`FRAME_MAX_BYTES`] — read no further: what follows is not a frame.
pub fn read_frame(r: &mut impl Read) -> io::Result<Option<(u8, Vec<u8>)>> {
    let mut header = [0u8; 5];
    let mut got = 0;
    while got < header.len() {
        match r.read(&mut header[got..]) {
            Ok(0) if got == 0 => return Ok(None),
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    let len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
    if len > FRAME_MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a frame of {len} bytes is past the {FRAME_MAX_BYTES}-byte limit"),
        ));
    }
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)?;
    Ok(Some((header[4], payload)))
}

/// Reads the first line of a connection, without its newline, or `None` if
/// the peer closed before sending one.
///
/// # Errors
///
/// A read failed, or the line is past [`ASK_MAX_BYTES`].
pub fn read_line(r: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    let read = r
        .take(ASK_MAX_BYTES as u64 + 1)
        .read_until(b'\n', &mut line)?;
    if read == 0 {
        return Ok(None);
    }
    if line.last() != Some(&b'\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "a first line with no end, or past its limit",
        ));
    }
    line.pop();
    Ok(Some(line))
}

/// Bytes as base64, in a JSON field.
mod base64_bytes {
    use base64::Engine as _;
    use serde::{Deserialize as _, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD
            .decode(text)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors() -> Colors {
        Colors {
            foreground: Rgb([0xdd, 0xdd, 0xdd]),
            background: Rgb([0x11, 0x11, 0x11]),
            cursor: None,
            palette: (0..=255u8).map(|i| Rgb([i, i, i])).collect(),
            scheme: Scheme::Dark,
        }
    }

    fn every_viewer_frame() -> Vec<ToViewer> {
        vec![
            ToViewer::Attached(Attached {
                protocol: crate::PROTOCOL,
                mode: AttachMode::Exact,
                id: "0123456789abcdef".parse().unwrap(),
                size: Size::BORN,
            }),
            ToViewer::Snapshot(vec![0, 1, 2, 255]),
            ToViewer::Ready,
            ToViewer::Bytes(b"\x1b[31mred\x1b[m\r\n".to_vec()),
            ToViewer::Resize(Size {
                columns: 100,
                rows: 30,
                cell_width_px: 9,
                cell_height_px: 18,
            }),
            ToViewer::Mode(ModeChange {
                number: 2026,
                ansi: false,
                on: false,
            }),
            ToViewer::Colors(colors()),
            ToViewer::Clipboard(ClipboardWrite {
                clipboard: ClipboardKind::Primary,
                contents: vec![ClipboardContent {
                    mime: "image/png".to_owned(),
                    data: vec![0x89, b'P', b'N', b'G', 0, 0xff],
                }],
            }),
            ToViewer::Consent(Consent {
                id: 9,
                clipboard: ClipboardKind::Standard,
                mimes: vec!["text/plain".to_owned()],
                program: String::new(),
            }),
            ToViewer::State(Box::new(crate::state::tests::full())),
            ToViewer::Taken,
            ToViewer::Lagging,
            ToViewer::Exited(Exit { code: 130 }),
            ToViewer::PasteRefused(PasteRefusal::Unsafe),
            ToViewer::PasteRefused(PasteRefusal::Failed),
        ]
    }

    fn every_holder_frame() -> Vec<ToHolder> {
        vec![
            ToHolder::Key(KeyEvent {
                code: 30,
                key: "KeyA".to_owned(),
                action: KeyAction::Repeat,
                mods: Mods {
                    shift: true,
                    ..Mods::default()
                },
                consumed: Mods {
                    shift: true,
                    ..Mods::default()
                },
                unshifted: Some('a'),
                utf8: "A".to_owned(),
                composing: false,
            }),
            ToHolder::Text("日本語".to_owned()),
            ToHolder::Mouse(MouseEvent {
                action: MouseAction::Press,
                button: Some(Button::WheelUp),
                mods: Mods::default(),
                x_px: 12.5,
                y_px: 3.0,
                any_button_held: false,
            }),
            ToHolder::Focus(Focus { gained: true }),
            ToHolder::Paste(Paste {
                text: "ls\n".to_owned(),
                source: PasteSource::Clipboard,
                allow_unsafe: true,
            }),
            ToHolder::Resize(Size::BORN),
            ToHolder::Colors(colors()),
            ToHolder::Answer(Answer { id: 9, text: None }),
            ToHolder::Place(Place {
                front: true,
                seen: true,
                order: Some(3 << 16),
            }),
            ToHolder::Place(Place {
                front: false,
                seen: false,
                order: None,
            }),
            ToHolder::Detach,
        ]
    }

    /// Reads back one frame out of exactly the bytes `encode` produced.
    fn one(bytes: &[u8]) -> (u8, Vec<u8>) {
        let mut r = bytes;
        let frame = read_frame(&mut r).unwrap().unwrap();
        assert!(r.is_empty(), "the frame is all of its bytes");
        frame
    }

    #[test]
    fn every_frame_survives_the_wire_both_ways() {
        // FAILS IF a frame is written in a form its reader does not read
        // back, or read as another kind.
        for f in every_viewer_frame() {
            let (kind, payload) = one(&f.encode());
            assert_eq!(ToViewer::decode(kind, payload).unwrap(), f);
        }
        for f in every_holder_frame() {
            let (kind, payload) = one(&f.encode());
            assert_eq!(ToHolder::decode(kind, payload).unwrap(), f);
        }
    }

    #[test]
    fn a_frame_is_one_buffer_with_its_length_in_front() {
        // FAILS IF a frame stops being one buffer — each one is written with
        // one `write_all`, and a frame built in pieces would be written in
        // pieces — or the length counts something else than the payload.
        let bytes = ToViewer::Bytes(b"abc".to_vec()).encode();
        assert_eq!(bytes, [3, 0, 0, 0, Kind::Bytes as u8, b'a', b'b', b'c']);
        assert_eq!(bytes_frame(b"abc"), bytes);
        let mut stream = Vec::new();
        for f in every_viewer_frame() {
            stream.extend(f.encode());
        }
        let mut r = stream.as_slice();
        let mut n = 0;
        while let Some((kind, payload)) = read_frame(&mut r).unwrap() {
            ToViewer::decode(kind, payload).unwrap();
            n += 1;
        }
        assert_eq!(n, every_viewer_frame().len(), "frames back to back");
    }

    #[test]
    fn a_frame_the_wrong_way_or_of_no_kind_is_refused() {
        // FAILS IF a viewer can send a holder's frame (a forged `Bytes` would
        // be a way to write to the program), or an unknown kind is guessed at.
        let (kind, payload) = one(&ToViewer::Bytes(b"rm -rf ~\n".to_vec()).encode());
        assert!(matches!(
            ToHolder::decode(kind, payload),
            Err(WireError::WrongWay(Kind::Bytes))
        ));
        assert!(matches!(
            ToViewer::decode(200, Vec::new()),
            Err(WireError::UnknownKind(200))
        ));
        assert!(matches!(
            ToHolder::decode(Kind::Key as u8, b"{}".to_vec()),
            Err(WireError::Payload(Kind::Key, _))
        ));
    }

    #[test]
    fn the_wire_has_no_raw_input() {
        // FAILS IF a viewer→holder frame carries raw bytes for the PTY: only
        // `Text` is not JSON, and it must be text — what the holder writes is
        // always something it encoded.
        for k in Kind::ALL.into_iter().filter(|k| !k.to_viewer()) {
            assert_ne!(k, Kind::Bytes);
        }
        assert!(ToHolder::decode(Kind::Text as u8, vec![0xff, 0xfe]).is_err());
    }

    #[test]
    fn the_kinds_are_one_table_with_no_number_twice() {
        // FAILS IF two kinds share a number, or a direction is mislabelled.
        let mut numbers: Vec<u8> = Kind::ALL.iter().map(|k| *k as u8).collect();
        numbers.sort_unstable();
        numbers.dedup();
        assert_eq!(numbers.len(), Kind::ALL.len());
        assert!(Kind::Exited.to_viewer() && !Kind::Detach.to_viewer());
        for k in Kind::ALL {
            assert_eq!(Kind::of(k as u8), Some(k));
        }
    }

    #[test]
    fn a_cut_or_oversized_frame_is_an_error_and_a_clean_end_is_none() {
        // FAILS IF a half frame is taken for a whole one, or a length a
        // broken peer sent is allocated before anything is checked.
        assert!(read_frame(&mut &b""[..]).unwrap().is_none());
        assert!(read_frame(&mut &[3, 0, 0, 0, 4, b'a'][..]).is_err());
        assert!(read_frame(&mut &[3, 0][..]).is_err());
        assert!(read_frame(&mut &[0xff, 0xff, 0xff, 0x7f, 4][..]).is_err());
    }

    #[test]
    fn an_ask_is_one_line_and_reads_back() {
        // FAILS IF an ask spans lines (the holder reads one) or does not
        // read back as itself.
        let asks = [
            Ask::Watch,
            Ask::State,
            Ask::Attach(Attach {
                protocol: crate::PROTOCOL,
                engine: "x-libghostty-vt-1.4.0".to_owned(),
                pid: 77,
                size: Size::BORN,
                colors: colors(),
                group: "fedcba9876543210".parse().unwrap(),
                order: 1,
            }),
            Ask::Place(Place {
                front: true,
                seen: false,
                order: None,
            }),
            Ask::Front,
            Ask::End,
            Ask::Detach,
        ];
        for ask in asks {
            let line = ask.line().unwrap();
            assert_eq!(line.iter().filter(|b| **b == b'\n').count(), 1);
            let mut r = line.as_slice();
            let read = read_line(&mut r).unwrap().unwrap();
            assert_eq!(Ask::parse(&read).unwrap(), ask);
        }
        assert_eq!(Ask::Front.line().unwrap(), b"{\"ask\":\"front\"}\n");
    }

    #[test]
    fn a_place_says_its_order_only_when_it_has_one() {
        // FAILS IF a place with no order is written with one a holder would
        // take for a renumbering (`0`, a default), or a place of a window of
        // the first protocol — which says no order — is refused instead of
        // read as one that keeps the holder's.
        let kept = Place {
            front: true,
            seen: false,
            order: None,
        };
        assert_eq!(
            serde_json::to_string(&kept).unwrap(),
            "{\"front\":true,\"seen\":false}"
        );
        let first: Place = serde_json::from_str("{\"front\":false,\"seen\":false}").unwrap();
        assert_eq!(first.order, None);
        let moved: Place =
            serde_json::from_str("{\"front\":false,\"seen\":false,\"order\":7}").unwrap();
        assert_eq!(moved.order, Some(7));
    }

    #[test]
    fn a_palette_of_another_size_is_refused() {
        // FAILS IF a palette of 255 is padded into a plausible 256: a
        // fallback with no word.
        let mut c = colors();
        assert!(c.palette_of_256().is_some());
        c.palette.pop();
        assert!(c.palette_of_256().is_none());
        assert!(serde_json::from_str::<Rgb>("\"#12345\"").is_err());
        assert_eq!(
            serde_json::to_string(&Rgb([1, 2, 255])).unwrap(),
            "\"#0102ff\""
        );
    }
}
