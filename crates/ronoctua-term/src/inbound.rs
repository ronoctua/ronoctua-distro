//! What the program writes, before the engine reads it — the holder's filter.
//!
//! # Why a filter, and why before the engine
//!
//! Two things a program may write cannot reach the engine as they are:
//!
//! - **Sixel** (`DCS P1;P2;P3 q … ST`), which `libghostty-vt` does not draw.
//!   The filter decodes it ([`crate::sixel`]) and writes a kitty image in its
//!   place, with the cursor where xterm would leave it.
//! - **Kitty images by file, temporary file or shared memory** (`t=f`, `t=t`,
//!   `t=s`). The engine reads them only if told to, and a temporary file is
//!   DELETED by whoever reads it: a holder and a window's mirror reading one
//!   would each find what the other left — and the second, nothing. The
//!   filter reads it ONCE, here, and writes the same command with the bytes
//!   inline (`t=d`); the engines are never given a file.
//!
//! The holder feeds the engine the FILTER'S output, and sends a window the
//! same bytes: holder and mirror read one stream, so they cannot disagree
//! about a picture.
//!
//! # A state machine of its own, the engine's
//!
//! What is a DCS and what is text inside an OSC is decided the way the
//! engine decides it — the DEC parser of vt100.net, as Ghostty's
//! `parse_table.zig` writes it, transition for transition, its overrides
//! included. So a `0x90` (an 8-bit DCS) inside a title, or a UTF-8 text whose
//! bytes include one, is no sixel, exactly as the engine does not take it for
//! a DCS; and an `ESC` ends a string, whatever is inside it, exactly as the
//! engine ends it there. A filter that parsed "close enough" would turn text
//! into pictures, or pictures into text the engine prints.
//!
//! # Held, then released
//!
//! A sequence the filter might rewrite is HELD until it knows: a CSI until
//! its final byte (XTSMGRAPHICS is answered here, the engine has no
//! sixel), a DCS until its introducer is complete, an APC until its kitty
//! keys are read. Anything that turns out to be nothing of ours is released
//! as it came, in order — so the output, cut wherever the program's writes
//! cut it, is the same bytes.

use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::os::fd::AsRawFd as _;
use std::path::{Path, PathBuf};

use base64::Engine as _;

use crate::sixel;

/// The most bytes a file, temporary file or shared memory medium is read
/// for — the engine's own limit for an image's data (`graphics_image.zig`).
pub const KITTY_MEDIUM_MAX_BYTES: u64 = 400 * 1024 * 1024;

/// The longest kitty key list read before the payload. Real ones are a few
/// dozen bytes; past this it is passed on as it came.
const KITTY_KEYS_MAX: usize = 1024;

/// The longest path (base64) a medium is read from.
const KITTY_PATH_MAX: usize = 8192;

/// The longest CSI or DCS introducer held. Past it, it is no sequence of
/// ours and is released.
const HELD_MAX: usize = 512;

/// The longest string (an APC whose kitty keys or medium path are being read)
/// held: room for both bounds above, and the bytes the engine skips in
/// between.
const STRING_HELD_MAX: usize = 2 * (KITTY_KEYS_MAX + KITTY_PATH_MAX);

/// The engine's limit on parameters: a DCS with more is not hooked.
const PARAMS_MAX: usize = 24;

/// The engine's limit on intermediates; more are not collected.
const INTERMEDIATES_MAX: usize = 4;

/// How much base64 one kitty command carries — the protocol's own chunk.
const KITTY_CHUNK: usize = 4096;

/// The image ids the filter gives the pictures it makes: the top of the
/// range, far from the small numbers programs choose, and the same in the
/// holder and in every mirror because the stream carries them.
const SIXEL_IDS: std::ops::RangeInclusive<u32> = 0xF000_0000..=0xFFFF_FFFE;

/// What the filter hands on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Bytes for the engine — and for the windows, the same bytes.
    Feed(Vec<u8>),
    /// Bytes for the program: an answer the filter gave itself.
    Reply(Vec<u8>),
}

/// The parser's states — the engine's, plus the two this filter keeps for
/// itself: a sixel being decoded, and a kitty medium's path being read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    Escape,
    EscapeIntermediate,
    CsiEntry,
    CsiParam,
    CsiIntermediate,
    CsiIgnore,
    DcsEntry,
    DcsParam,
    DcsIntermediate,
    DcsPassthrough,
    DcsIgnore,
    Osc,
    /// SOS, PM or APC — one state, as the engine has it.
    Str,
    /// A DCS that was a sixel, being decoded instead of passed on.
    Sixel,
}

/// Where an APC is, for kitty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kitty {
    /// Its first byte is not read yet.
    Start,
    /// Reading the keys (`G` seen).
    Keys,
    /// A medium to read: reading its path.
    Path,
    /// Nothing of ours: passed on.
    Off,
}

/// The parameters and intermediates of a CSI or DCS, as the engine collects
/// them.
#[derive(Debug, Clone, Default)]
struct Params {
    numbers: Vec<u16>,
    digits: Option<u16>,
    intermediates: Vec<u8>,
    /// Too long to hold: released already, so nothing of it is ours to
    /// answer or take.
    spilled: bool,
}

impl Params {
    fn digit(&mut self, b: u8) {
        let d = u16::from(b - b'0');
        self.digits = Some(
            self.digits
                .unwrap_or(0)
                .saturating_mul(10)
                .saturating_add(d),
        );
    }

    /// The next parameter. Past one more than the engine's limit nothing is
    /// kept — the count already says "too many", and a CSI of endless `;`
    /// would otherwise grow the list for as long as it is written.
    fn separator(&mut self) {
        let d = self.digits.take().unwrap_or(0);
        if self.numbers.len() <= PARAMS_MAX {
            self.numbers.push(d);
        }
    }

    /// An intermediate byte, up to the engine's limit.
    fn intermediate(&mut self, b: u8) {
        if self.intermediates.len() < INTERMEDIATES_MAX {
            self.intermediates.push(b);
        }
    }

    fn done(&mut self) -> &[u16] {
        if let Some(d) = self.digits.take()
            && self.numbers.len() <= PARAMS_MAX
        {
            self.numbers.push(d);
        }
        &self.numbers
    }
}

/// The holder's inbound filter. One per terminal, fed in order.
pub struct Inbound {
    state: State,
    /// Bytes of the sequence being read, not handed on yet.
    held: Vec<u8>,
    /// The bytes of this call that go on.
    feed: Vec<u8>,
    out: Vec<Piece>,
    params: Params,
    kitty: Kitty,
    /// The APC's content, while its keys or its path are read.
    apc: Vec<u8>,
    sixel: Option<sixel::Decoder>,
    /// An `ESC` that ended a sequence the filter rewrote — whose own ST the
    /// rewrite already carries — is swallowed with the `\` after it.
    swallow_st: bool,
    /// The escape being read went past what is held, and was released: what
    /// it turns into is not the filter's to answer or take.
    escape_spilled: bool,
    /// DECSDM (`CSI ? 80 h`): sixels drawn at the top left, the cursor still.
    display_mode: bool,
    next_image: u32,
    palette: Option<Box<[[u8; 3]; 256]>>,
    background: Option<[u8; 3]>,
    /// Columns and rows, and a cell's pixels.
    size: (u16, u16, u16, u16),
}

impl Default for Inbound {
    fn default() -> Self {
        Self::new()
    }
}

impl Inbound {
    /// A filter in the ground state, as a terminal starts.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: State::Ground,
            held: Vec::new(),
            feed: Vec::new(),
            out: Vec::new(),
            params: Params::default(),
            kitty: Kitty::Off,
            apc: Vec::new(),
            sixel: None,
            swallow_st: false,
            escape_spilled: false,
            display_mode: false,
            next_image: *SIXEL_IDS.start(),
            palette: None,
            background: None,
            size: (0, 0, 0, 0),
        }
    }

    /// The terminal's colours, for a sixel's undefined registers and its
    /// background.
    pub fn set_colors(&mut self, palette: &[[u8; 3]; 256], background: [u8; 3]) {
        self.palette = Some(Box::new(*palette));
        self.background = Some(background);
    }

    /// The terminal's size, for where a sixel leaves the cursor and what
    /// XTSMGRAPHICS answers.
    pub const fn set_size(
        &mut self,
        columns: u16,
        rows: u16,
        cell_width_px: u16,
        cell_height_px: u16,
    ) {
        self.size = (columns, rows, cell_width_px, cell_height_px);
    }

    /// Filters one read of the program's output.
    pub fn filter(&mut self, input: &[u8]) -> Vec<Piece> {
        let mut i = 0;
        while i < input.len() {
            // The runs that need no decision, at once.
            let run = match self.state {
                State::Ground => until(&input[i..], |b| b == 0x1B),
                State::Osc => until(&input[i..], |b| matches!(b, 0x07 | 0x18 | 0x1A | 0x1B)),
                State::DcsPassthrough | State::DcsIgnore => {
                    until(&input[i..], |b| matches!(b, 0x18 | 0x1A | 0x1B))
                }
                State::Str if self.kitty == Kitty::Off => until(&input[i..], |b| {
                    matches!(b, 0x18 | 0x1A | 0x1B | 0x80..=0x9F)
                }),
                State::Sixel => {
                    let n = until(&input[i..], |b| matches!(b, 0x18 | 0x1A | 0x1B));
                    if let Some(d) = &mut self.sixel {
                        d.feed(&input[i..i + n]);
                    }
                    i += n;
                    0
                }
                _ => 0,
            };
            if run > 0 {
                self.feed.extend_from_slice(&input[i..i + run]);
                i += run;
                continue;
            }
            if i < input.len() {
                self.step(input[i]);
                i += 1;
            }
        }
        self.flush();
        std::mem::take(&mut self.out)
    }

    /// Hands on what this call let through so far, as one piece.
    fn flush(&mut self) {
        if !self.feed.is_empty() {
            self.out.push(Piece::Feed(std::mem::take(&mut self.feed)));
        }
    }

    /// A reply, in order with what went on before it.
    fn reply(&mut self, bytes: Vec<u8>) {
        self.flush();
        self.out.push(Piece::Reply(bytes));
    }

    /// What was held goes on as it came.
    fn release(&mut self) {
        self.feed.append(&mut self.held);
    }

    fn step(&mut self, b: u8) {
        if self.swallow_st {
            self.swallow_st = false;
            if self.state == State::Escape && b == b'\\' {
                // The rewrite ended with its own ST: this one is its twin.
                self.held.clear();
                self.state = State::Ground;
                return;
            }
        }
        match self.state {
            State::Ground => {
                if b == 0x1B {
                    self.enter_escape(b);
                } else {
                    self.feed.push(b);
                }
            }
            State::Osc => match b {
                0x07 | 0x18 | 0x1A => {
                    self.feed.push(b);
                    self.state = State::Ground;
                }
                0x1B => self.enter_escape(b),
                // 0x20–0xFF are the string's, C1 included: the engine is
                // UTF-8, and a title's `ϐ` has a 0x90 in it.
                _ => self.feed.push(b),
            },
            State::DcsPassthrough | State::DcsIgnore => match b {
                0x18 | 0x1A => {
                    self.feed.push(b);
                    self.state = State::Ground;
                }
                0x1B => self.enter_escape(b),
                _ => self.feed.push(b),
            },
            State::Sixel => match b {
                0x18 | 0x1A => {
                    // Cancelled: nothing is drawn, and the engine sees the
                    // cancel as it would have.
                    self.sixel = None;
                    self.feed.push(b);
                    self.state = State::Ground;
                }
                0x1B => {
                    self.finish_sixel();
                    self.enter_escape(b);
                    self.swallow_st = true;
                }
                _ => {
                    if let Some(d) = &mut self.sixel {
                        d.feed(&[b]);
                    }
                }
            },
            _ => self.in_sequence(b),
        }
        self.bound_the_held();
    }

    /// What is held is bounded, whatever the program writes: an `ESC` and the
    /// bytes the engine skips after it, a kitty command padded with bytes it
    /// ignores, an introducer that re-enters itself. Past the bound the
    /// sequence is no sequence of ours, and what was held goes on as it came
    /// — the engine's reading of it is untouched; only the filter's claim on
    /// it is given up.
    fn bound_the_held(&mut self) {
        let limit = if self.state == State::Str {
            STRING_HELD_MAX
        } else {
            HELD_MAX
        };
        if self.held.len() <= limit {
            return;
        }
        self.release();
        match self.state {
            State::Escape => self.escape_spilled = true,
            State::Str => {
                self.kitty = Kitty::Off;
                self.apc.clear();
            }
            _ => self.params.spilled = true,
        }
    }

    /// Enters the escape state on `b` (an `ESC`), holding it: what follows
    /// decides whether it is ours.
    fn enter_escape(&mut self, b: u8) {
        if self.state == State::Escape {
            // `ESC ESC` stays in the escape state (the engine does not clear).
            self.held.push(b);
            return;
        }
        self.release();
        self.held.push(b);
        self.state = State::Escape;
        self.escape_spilled = false;
    }

    /// The transitions the engine takes from ANY state but the ones handled
    /// above, or `None` for a byte the state decides itself.
    fn anywhere(b: u8) -> Option<State> {
        match b {
            0x18 | 0x1A | 0x80..=0x8F | 0x91..=0x97 | 0x99 | 0x9A | 0x9C => Some(State::Ground),
            0x1B => Some(State::Escape),
            0x98 | 0x9E | 0x9F => Some(State::Str),
            0x9B => Some(State::CsiEntry),
            0x90 => Some(State::DcsEntry),
            0x9D => Some(State::Osc),
            _ => None,
        }
    }

    /// The escape, CSI, DCS-introducer and string states, byte by byte.
    fn in_sequence(&mut self, b: u8) {
        if let Some(next) = Self::anywhere(b) {
            if next == self.state {
                // The engine changes nothing for a state it is already in.
                self.held_or_fed(b);
                return;
            }
            self.leave(b);
            self.enter(next, b);
            return;
        }
        match self.state {
            State::Escape => match b {
                0x5B => self.enter(State::CsiEntry, b),
                0x50 => self.enter(State::DcsEntry, b),
                0x5D => {
                    self.held.push(b);
                    self.release();
                    self.escape_spilled = false;
                    self.state = State::Osc;
                }
                0x58 | 0x5E | 0x5F => self.enter(State::Str, b),
                0x20..=0x2F => {
                    self.held.push(b);
                    self.release();
                    self.escape_spilled = false;
                    self.state = State::EscapeIntermediate;
                }
                0x30..=0x7E => {
                    if b == b'c' {
                        // RIS: sixels scroll again.
                        self.display_mode = false;
                    }
                    self.held.push(b);
                    self.release();
                    self.escape_spilled = false;
                    self.state = State::Ground;
                }
                // C0 executes inside the escape; DEL and 0xA0–0xFF are
                // ignored there — all held with it.
                _ => self.held.push(b),
            },
            State::EscapeIntermediate => {
                self.feed.push(b);
                if (0x30..=0x7E).contains(&b) {
                    self.state = State::Ground;
                }
            }
            State::CsiEntry | State::CsiParam | State::CsiIntermediate | State::CsiIgnore => {
                self.in_csi(b);
            }
            State::DcsEntry | State::DcsParam | State::DcsIntermediate => self.in_dcs_introducer(b),
            State::Str => self.in_string(b),
            _ => self.feed.push(b),
        }
    }

    /// `b` goes where the current state's bytes go: held while a decision
    /// is pending, on otherwise.
    fn held_or_fed(&mut self, b: u8) {
        if self.holding() {
            self.held.push(b);
        } else {
            self.feed.push(b);
        }
    }

    /// Whether this state's bytes are held.
    fn holding(&self) -> bool {
        match self.state {
            State::Escape
            | State::CsiEntry
            | State::CsiParam
            | State::CsiIntermediate
            | State::CsiIgnore
            | State::DcsEntry
            | State::DcsParam
            | State::DcsIntermediate => true,
            State::Str => self.kitty != Kitty::Off,
            _ => false,
        }
    }

    /// The current sequence ends on `b`, which starts another (or none).
    fn leave(&mut self, b: u8) {
        match self.state {
            State::Str if self.kitty == Kitty::Path && b == 0x1B => {
                if let Some(rewritten) = self.rewrite_medium() {
                    self.held.clear();
                    self.feed.extend_from_slice(&rewritten);
                    self.swallow_st = true;
                } else {
                    self.release();
                }
            }
            _ => self.release(),
        }
        self.kitty = Kitty::Off;
        self.apc.clear();
    }

    /// Enters `next` on the byte `b` that leads into it.
    fn enter(&mut self, next: State, b: u8) {
        // An escape that spilled already went on: what it introduces cannot
        // be taken back out of the stream. Whatever comes next, the escape is
        // over, and so is its spill.
        let spilled = std::mem::take(&mut self.escape_spilled);
        match next {
            State::Ground => {
                self.feed.push(b);
            }
            State::Escape => {
                self.held.push(b);
            }
            State::CsiEntry | State::DcsEntry => {
                self.params = Params {
                    spilled,
                    ..Params::default()
                };
                self.held.push(b);
            }
            State::Str => {
                self.held.push(b);
                self.apc.clear();
                if spilled {
                    self.release();
                    self.kitty = Kitty::Off;
                } else {
                    self.kitty = Kitty::Start;
                }
            }
            _ => {
                self.held.push(b);
                self.release();
            }
        }
        self.state = next;
    }

    fn in_csi(&mut self, b: u8) {
        self.held.push(b);
        let state = self.state;
        match b {
            // C0 executes inside a CSI and DEL is ignored: held with it.
            0x00..=0x1F | 0x7F => {}
            0x40..=0x7E => {
                if state != State::CsiIgnore {
                    self.dispatch_csi(b);
                } else {
                    self.release();
                }
                self.state = State::Ground;
            }
            0x30..=0x39 if state != State::CsiIntermediate && state != State::CsiIgnore => {
                self.params.digit(b);
                self.state = State::CsiParam;
            }
            0x3B if state != State::CsiIntermediate && state != State::CsiIgnore => {
                self.params.separator();
                self.state = State::CsiParam;
            }
            0x3A if state == State::CsiParam => {
                // A sub-parameter: nothing of ours reads one.
                self.params.separator();
            }
            0x3A if state == State::CsiEntry => self.state = State::CsiIgnore,
            0x3C..=0x3F if state == State::CsiEntry => {
                self.params.intermediate(b);
                self.state = State::CsiParam;
            }
            0x20..=0x2F if state != State::CsiIgnore => {
                self.params.intermediate(b);
                self.state = State::CsiIntermediate;
            }
            0x30..=0x3F => self.state = State::CsiIgnore,
            _ => {}
        }
        if self.held.len() > HELD_MAX && self.state != State::Ground {
            self.params.spilled = true;
            self.release();
        }
    }

    /// A CSI is complete: answer XTSMGRAPHICS, follow DECSDM, release the
    /// rest.
    fn dispatch_csi(&mut self, last: u8) {
        if self.params.spilled {
            self.release();
            return;
        }
        let private = self.params.intermediates == *b"?";
        let numbers = self.params.done().to_vec();
        match last {
            b'S' if private => {
                self.held.clear();
                let answer = self.graphics_attributes(&numbers);
                self.reply(answer);
            }
            b'h' | b'l' if private && numbers.contains(&80) => {
                self.display_mode = last == b'h';
                self.release();
            }
            _ => self.release(),
        }
    }

    /// XTSMGRAPHICS (`CSI ? Pi ; Pa ; Pv S`): the colour registers and the
    /// sixel geometry, from this filter's own limits; ReGIS is not spoken.
    fn graphics_attributes(&self, numbers: &[u16]) -> Vec<u8> {
        let item = numbers.first().copied().unwrap_or(0);
        let action = numbers.get(1).copied().unwrap_or(0);
        let max = sixel::MAX_SIDE_PX;
        let (columns, rows, w, h) = self.size;
        let area = (
            (usize::from(columns) * usize::from(w)).min(max),
            (usize::from(rows) * usize::from(h)).min(max),
        );
        let text = match (item, action) {
            // Registers: read, reset, set, read the most — always the 1024
            // there are.
            (1, 1..=4) => format!("\x1b[?1;0;{}S", sixel::REGISTERS),
            // The geometry now: the text area, when a window said its cells.
            (2, 1) if area.0 > 0 && area.1 > 0 => format!("\x1b[?2;0;{};{}S", area.0, area.1),
            (2, 1 | 4) => format!("\x1b[?2;0;{max};{max}S"),
            // It cannot be set: it is the window's.
            (2, 2 | 3) => "\x1b[?2;3;0S".to_owned(),
            (1 | 2, _) => format!("\x1b[?{item};2;0S"),
            _ => format!("\x1b[?{item};1;0S"),
        };
        text.into_bytes()
    }

    fn in_dcs_introducer(&mut self, b: u8) {
        self.held.push(b);
        let state = self.state;
        match b {
            0x00..=0x1F | 0x7F => {}
            0x40..=0x7E => self.hook(b),
            0x30..=0x39 if state != State::DcsIntermediate => {
                self.params.digit(b);
                self.state = State::DcsParam;
            }
            0x3B if state != State::DcsIntermediate => {
                self.params.separator();
                self.state = State::DcsParam;
            }
            0x3C..=0x3F if state == State::DcsEntry => {
                self.params.intermediate(b);
                self.state = State::DcsParam;
            }
            0x20..=0x2F => {
                self.params.intermediate(b);
                self.state = State::DcsIntermediate;
            }
            // 0xA0–0xFF are ignored inside an introducer (0x80–0x9F were
            // taken above, as anywhere).
            0x80..=0xFF => {}
            // `:`, a private marker after a parameter, or a digit after an
            // intermediate: the engine ignores the whole string.
            _ => {
                self.release();
                self.state = State::DcsIgnore;
            }
        }
        if self.held.len() > HELD_MAX && self.state != State::DcsPassthrough {
            self.release();
            self.state = State::DcsIgnore;
        }
    }

    /// A DCS's introducer is complete. A sixel (`q`, no intermediates) is
    /// taken; anything else goes on to the engine.
    fn hook(&mut self, last: u8) {
        let numbers = self.params.done().to_vec();
        if last == b'q'
            && !self.params.spilled
            && self.params.intermediates.is_empty()
            && numbers.len() <= PARAMS_MAX
        {
            self.held.clear();
            self.sixel = Some(sixel::Decoder::new(&numbers, self.palette.as_deref()));
            self.state = State::Sixel;
        } else {
            self.release();
            self.state = State::DcsPassthrough;
        }
    }

    /// A sixel is complete: its picture, as kitty commands, and the cursor
    /// where xterm leaves it.
    fn finish_sixel(&mut self) {
        let image = self.sixel.take().and_then(|d| d.finish(self.background));
        let cell_height = usize::from(self.size.3);
        if image.is_some() && cell_height == 0 {
            // No window ever said how tall a cell is: there is no row to
            // leave the cursor on, and an image with no place is dropped
            // rather than placed wrongly.
            tracing::debug!("a sixel arrived before any window gave the cell's size; dropped");
        }
        let Some(image) = image.filter(|_| cell_height > 0) else {
            // The `ESC` that began it was the engine's to see — a string the
            // engine was in ended there — and this ST ends it the same way.
            self.feed.extend_from_slice(b"\x1b\\");
            return;
        };
        let id = self.next_image;
        self.next_image = if id >= *SIXEL_IDS.end() {
            *SIXEL_IDS.start()
        } else {
            id + 1
        };
        let mut out = Vec::new();
        if self.display_mode {
            // DECSDM: the image at the top left, the cursor left there — the
            // one difference from xterm, which leaves it where it was: that
            // would need the saved cursor, which is the program's.
            out.extend_from_slice(b"\x1b[1;1H");
        }
        let keys = format!(
            "a=T,f=32,s={},v={},i={id},q=2,C=1",
            image.width, image.height
        );
        kitty_commands(&keys, &image.rgba, &mut out);
        if !self.display_mode {
            // xterm's place: the line after the image, in the column it
            // began. `IND` and not `LF`: it scrolls at the margin and keeps
            // the column whatever LNM says.
            let rows = image.height.div_ceil(cell_height);
            for _ in 0..rows {
                out.extend_from_slice(b"\x1bD");
            }
        }
        self.feed.extend_from_slice(&out);
    }

    fn in_string(&mut self, b: u8) {
        match self.kitty {
            Kitty::Off => self.feed.push(b),
            Kitty::Start => {
                self.held.push(b);
                if b == b'G' {
                    self.kitty = Kitty::Keys;
                } else if b < 0xA0 {
                    self.release();
                    self.kitty = Kitty::Off;
                }
            }
            Kitty::Keys => {
                self.held.push(b);
                if b == b';' {
                    let keys = Keys::parse(&self.apc);
                    if keys.wants_a_medium() {
                        self.kitty = Kitty::Path;
                        self.apc.push(b);
                    } else {
                        self.release();
                        self.kitty = Kitty::Off;
                    }
                } else if b < 0xA0 {
                    // 0xA0–0xFF are ignored by the engine inside a string:
                    // not content, so not part of the keys either.
                    self.apc.push(b);
                }
                if self.apc.len() > KITTY_KEYS_MAX {
                    self.release();
                    self.kitty = Kitty::Off;
                }
            }
            Kitty::Path => {
                self.held.push(b);
                if b < 0xA0 {
                    self.apc.push(b);
                }
                if self.apc.len() > KITTY_KEYS_MAX + KITTY_PATH_MAX {
                    self.release();
                    self.kitty = Kitty::Off;
                }
            }
        }
    }

    /// The kitty command whose medium is a file, read now, as the same
    /// command with its bytes inline — or `None`, and it goes on as it came
    /// (the engine, which reads no files, refuses it with its own answer).
    fn rewrite_medium(&self) -> Option<Vec<u8>> {
        let (keys, payload) = self.apc.split_at(self.apc.iter().position(|b| *b == b';')?);
        let keys = Keys::parse(keys);
        let path = base64_engine().decode(&payload[1..]).ok()?;
        let size = keys.number(b'S').unwrap_or(0);
        let offset = keys.number(b'O').unwrap_or(0);
        let data = match keys.get(b't')? {
            "f" => read_file(&path, size, offset, false),
            "t" => read_file(&path, size, offset, true),
            "s" => read_shared_memory(&path, size, offset),
            _ => None,
        }?;
        let kept = keys.without(b"tSOm");
        let mut out = Vec::new();
        kitty_commands(&kept, &data, &mut out);
        Some(out)
    }
}

/// How many bytes from the start of `bytes` are not a `stop` byte.
fn until(bytes: &[u8], stop: impl Fn(u8) -> bool) -> usize {
    bytes.iter().position(|b| stop(*b)).unwrap_or(bytes.len())
}

/// Base64 as kitty writes it: the standard alphabet, padding or not.
fn base64_engine() -> base64::engine::GeneralPurpose {
    base64::engine::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::GeneralPurposeConfig::new()
            .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
    )
}

/// `data` as kitty graphics commands of `keys`, in the protocol's chunks:
/// the keys on the first, `m=1` on every one but the last.
fn kitty_commands(keys: &str, data: &[u8], out: &mut Vec<u8>) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    let chunks: Vec<&[u8]> = if encoded.is_empty() {
        vec![&[]]
    } else {
        encoded.as_bytes().chunks(KITTY_CHUNK).collect()
    };
    let last = chunks.len() - 1;
    for (i, chunk) in chunks.into_iter().enumerate() {
        out.extend_from_slice(b"\x1b_G");
        match (i, last) {
            (0, 0) => out.extend_from_slice(keys.as_bytes()),
            (0, _) => out.extend_from_slice(format!("{keys},m=1").as_bytes()),
            (i, last) if i == last => out.extend_from_slice(b"m=0"),
            _ => out.extend_from_slice(b"m=1"),
        }
        out.push(b';');
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\x1b\\");
    }
}

/// A kitty command's keys (`a=T,f=100,t=f`), in order.
struct Keys(Vec<(u8, String)>);

impl Keys {
    fn parse(text: &[u8]) -> Self {
        Self(
            text.split(|b| *b == b',')
                .filter_map(|pair| match pair {
                    [k, b'=', v @ ..] => Some((*k, String::from_utf8_lossy(v).into_owned())),
                    _ => None,
                })
                .collect(),
        )
    }

    fn get(&self, key: u8) -> Option<&str> {
        self.0
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
    }

    fn number(&self, key: u8) -> Option<u64> {
        self.get(key)?.parse().ok()
    }

    /// A transmission (`a=t`, the default, `a=T` or an animation frame) from
    /// a file, a temporary file or shared memory, in one piece.
    fn wants_a_medium(&self) -> bool {
        let transmits = matches!(self.get(b'a'), None | Some("t" | "T" | "f"));
        let medium = matches!(self.get(b't'), Some("f" | "t" | "s"));
        let whole = matches!(self.get(b'm'), None | Some("0"));
        transmits && medium && whole
    }

    fn without(&self, dropped: &[u8]) -> String {
        self.0
            .iter()
            .filter(|(k, _)| !dropped.contains(k))
            .map(|(k, v)| format!("{}={v}", char::from(*k)))
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Whether `path` may be read as an image: not a kernel file, not a device
/// (shared memory aside) — kitty's own blocklist, which the engine keeps too.
fn readable_place(path: &Path) -> bool {
    let shm = Path::new("/dev/shm");
    !(path.starts_with("/proc") || path.starts_with("/sys"))
        && (!path.starts_with("/dev") || path.starts_with(shm) && path != shm)
}

/// Whether `path` is in a temporary folder: `/tmp`, `/dev/shm` or the
/// session's (`$TMPDIR`), canonical or not.
fn in_a_temporary_folder(path: &Path) -> bool {
    let session = std::env::temp_dir();
    let canonical = session.canonicalize().unwrap_or_else(|_| session.clone());
    [
        PathBuf::from("/tmp"),
        PathBuf::from("/dev/shm"),
        session,
        canonical,
    ]
    .iter()
    .any(|dir| path.starts_with(dir) && path != dir)
}

/// Opens what a program named as a medium, without waiting on it and
/// without it becoming anything of this process's — what it IS is judged
/// after, on the descriptor.
///
/// - `NONBLOCK`: a FIFO's open waits for a writer, and this runs on the
///   engine's thread — the terminal would stop with it, its input, its
///   windows and its hang-up, for as long as nobody wrote.
/// - `NOCTTY`: the holder leads a session with no controlling terminal, and
///   the first terminal such a process opens becomes its own.
/// - `NOFOLLOW` where the protocol names an object and not a path (shared
///   memory), as `shm_open` itself does.
fn open_medium(path: &Path, follow: bool) -> Option<File> {
    use rustix::fs::{Mode, OFlags};
    let mut flags = OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOCTTY | OFlags::CLOEXEC;
    if !follow {
        flags |= OFlags::NOFOLLOW;
    }
    rustix::fs::open(path, flags, Mode::empty())
        .ok()
        .map(File::from)
}

/// The path a descriptor is really open on — asked of the descriptor, so
/// the file judged is the file read (a name resolved first could be swapped
/// in between).
fn real_path(file: &File) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())).ok()
}

/// Reads `size` bytes at `offset` (or all of it, for a size of 0), within
/// the medium limit.
fn read_from(file: &mut File, size: u64, offset: u64) -> Option<Vec<u8>> {
    if size > KITTY_MEDIUM_MAX_BYTES {
        return None;
    }
    if offset > 0 {
        file.seek(SeekFrom::Start(offset)).ok()?;
    }
    let mut data = Vec::new();
    if size > 0 {
        // `S` is the exact number of bytes, not a ceiling.
        file.take(size).read_to_end(&mut data).ok()?;
        (data.len() as u64 == size).then_some(data)
    } else {
        file.take(KITTY_MEDIUM_MAX_BYTES + 1)
            .read_to_end(&mut data)
            .ok()?;
        (data.len() as u64 <= KITTY_MEDIUM_MAX_BYTES).then_some(data)
    }
}

/// A file (`t=f`), or a temporary file (`t=t`) — deleted once read, and
/// only one in a temporary folder whose name says it is one.
fn read_file(raw: &[u8], size: u64, offset: u64, temporary: bool) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt as _;
    let path = Path::new(std::ffi::OsStr::from_bytes(raw));
    let mut file = open_medium(path, true)?;
    let real = real_path(&file)?;
    if !readable_place(&real) || !file.metadata().ok()?.is_file() {
        return None;
    }
    if temporary {
        let named = real
            .as_os_str()
            .as_bytes()
            .windows(21)
            .any(|w| w == b"tty-graphics-protocol");
        if !in_a_temporary_folder(&real) || !named {
            return None;
        }
    }
    let data = read_from(&mut file, size, offset);
    if temporary {
        // Deleted whether or not it read: the protocol hands the file over.
        let _ = std::fs::remove_file(&real);
    }
    data
}

/// A shared memory object (`t=s`), by its POSIX name — unlinked once opened,
/// as the protocol says.
fn read_shared_memory(raw: &[u8], size: u64, offset: u64) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt as _;
    let name = raw.strip_prefix(b"/").unwrap_or(raw);
    if name.is_empty() || name.len() > 255 || name.contains(&b'/') || name.contains(&0) {
        return None;
    }
    if name == b"." || name == b".." {
        return None;
    }
    let path = Path::new("/dev/shm").join(std::ffi::OsStr::from_bytes(name));
    let mut file = open_medium(&path, false)?;
    let _ = std::fs::remove_file(&path);
    let meta = file.metadata().ok()?;
    let length = meta.len();
    if !meta.is_file() || length == 0 {
        return None;
    }
    // Shared memory is a whole number of pages: without `S`, the object's
    // own length, from the offset.
    let size = if size > 0 {
        size
    } else {
        length.saturating_sub(offset)
    };
    read_from(&mut file, size, offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The output of `input` fed in one piece, as the engine gets it, and the
    /// replies apart.
    fn run(filter: &mut Inbound, input: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let (mut fed, mut replies) = (Vec::new(), Vec::new());
        for piece in filter.filter(input) {
            match piece {
                Piece::Feed(b) => fed.extend(b),
                Piece::Reply(r) => replies.extend(r),
            }
        }
        (fed, replies)
    }

    fn sized() -> Inbound {
        let mut f = Inbound::new();
        f.set_size(80, 24, 10, 20);
        f
    }

    const SIXEL: &[u8] = b"\x1bP0;1q\"1;1;3;12#1;2;100;0;0#1~~~-#2;2;0;0;100#2@@@\x1b\\";

    #[test]
    fn text_and_ordinary_sequences_go_on_as_they_came() {
        // FAILS IF the filter changes what it has no business with — every
        // byte of a session goes through it.
        let input = b"hello \x1b[31mred\x1b[m \xcf\x90 \x1b]0;t\x07\x1bP+q544e\x1b\\\x1b[?25l\x1b7";
        let (fed, replies) = run(&mut sized(), input);
        assert_eq!(fed, input);
        assert!(replies.is_empty());
    }

    #[test]
    fn sixel_becomes_kitty_deterministically() {
        // FAILS IF a sixel reaches the engine (which would print nothing and
        // lose the picture), its pixels are not the ones drawn, the cursor is
        // not left on the line after it, or two runs of the same bytes write
        // different ones.
        let (fed, _) = run(&mut sized(), SIXEL);
        let (again, _) = run(&mut sized(), SIXEL);
        assert_eq!(fed, again);
        let text = String::from_utf8(fed).unwrap();
        assert!(!text.contains("\x1bP"), "no DCS left: {text:?}");
        assert!(
            text.starts_with("\x1b_Ga=T,f=32,s=3,v=12,i=4026531840,q=2,C=1;"),
            "{text:?}"
        );
        // 12 pixels tall in cells of 20: one row, so one index after it.
        assert!(text.ends_with("\x1b\\\x1bD"), "{text:?}");
        assert_eq!(text.matches("\x1bD").count(), 1);
        let b64 = &text[text.find(';').unwrap() + 1..text.find("\x1b\\").unwrap()];
        let rgba = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        assert_eq!(rgba.len(), 3 * 12 * 4);
        assert_eq!(&rgba[..4], &[255, 0, 0, 255], "the first band is red");
        assert_eq!(
            &rgba[6 * 3 * 4..6 * 3 * 4 + 4],
            &[0, 0, 255, 255],
            "the second's top is blue"
        );
        assert_eq!(
            &rgba[7 * 3 * 4..7 * 3 * 4 + 4],
            &[0, 0, 0, 0],
            "and under it, nothing"
        );
        // A second image gets the next id.
        let mut f = sized();
        let _ = run(&mut f, SIXEL);
        let (second, _) = run(&mut f, SIXEL);
        assert!(
            String::from_utf8(second)
                .unwrap()
                .contains(",i=4026531841,")
        );
    }

    #[test]
    fn a_sixel_split_across_reads_is_one_image() {
        // FAILS IF where the program's writes cut a sixel changes what comes
        // out: the PTY cuts wherever it likes.
        let whole = run(&mut sized(), SIXEL).0;
        for cut in 1..SIXEL.len() {
            let mut f = sized();
            let mut fed = run(&mut f, &SIXEL[..cut]).0;
            fed.extend(run(&mut f, &SIXEL[cut..]).0);
            assert_eq!(fed, whole, "cut at {cut}");
        }
        // And one byte at a time.
        let mut f = sized();
        let mut fed = Vec::new();
        for b in SIXEL {
            fed.extend(run(&mut f, std::slice::from_ref(b)).0);
        }
        assert_eq!(fed, whole);
    }

    #[test]
    fn any_cut_of_a_mixed_stream_gives_the_same_bytes_and_answers() {
        // FAILS IF a byte held across two reads is lost, doubled or moved —
        // text, a CSI, an OSC, a DCS that is not a sixel, a kitty command, a
        // question the filter answers, all cut wherever the PTY likes.
        let stream: Vec<u8> = [
            &b"a\x1b[31mb\x1b]2;t\x07"[..],
            SIXEL,
            b"\x1b[?1;1;0S\x1bP+q544e\x1b\\\x1b_Ga=T,f=100;AAAA\x1b\\end\x1b[?2;1;0Sz",
        ]
        .concat();
        let whole = run(&mut sized(), &stream);
        assert!(!whole.1.is_empty(), "the stream asks questions");
        for cut in 1..stream.len() {
            let mut f = sized();
            let (mut fed, mut replies) = run(&mut f, &stream[..cut]);
            let (rest, more) = run(&mut f, &stream[cut..]);
            fed.extend(rest);
            replies.extend(more);
            assert_eq!((fed, replies), whole, "cut at {cut}");
        }
    }

    #[test]
    fn a_dcs_inside_an_osc_is_no_sixel() {
        // FAILS IF a DCS introducer that is a string's TEXT is taken for a
        // sixel: an 8-bit DCS (0x90) inside a title — the second byte of `ϐ`
        // in UTF-8 — or `P` and `q` in an OSC 52's base64. The engine does
        // not take them for a DCS, and neither may the filter.
        for input in [
            &b"\x1b]2;\xcf\x90q#1;2;100;0;0#1~~~\x07after"[..],
            &b"\x1b]2;\x90q#1;2;100;0;0#1~~~\x1b\\after"[..],
            &b"\x1b]52;c;UHEjMTsyOzEwMDswOzAjMX5+fg==\x07"[..],
            &b"\x1b_Ga=t;UHEjMQ==\x1b\\"[..],
            &b"text \xcf\x90q#1~~~"[..],
        ] {
            let (fed, replies) = run(&mut sized(), input);
            assert_eq!(fed, input, "{input:?}");
            assert!(replies.is_empty());
        }
        // While an `ESC` ends a string whatever is inside it — as the
        // engine ends it — so what follows one IS a sequence.
        let (fed, _) = run(&mut sized(), b"\x1b]2;title\x1bP0;1q#1;2;100;0;0#1~\x1b\\");
        assert!(
            fed.starts_with(b"\x1b]2;title\x1b_G"),
            "{:?}",
            String::from_utf8_lossy(&fed)
        );
    }

    #[test]
    fn xtsmgraphics_is_answered_here_and_not_passed_on() {
        // FAILS IF a program asking how large a sixel may be gets no answer
        // (it would draw nothing), or the question reaches the engine.
        let (fed, replies) = run(&mut sized(), b"a\x1b[?1;1;0Sb\x1b[?2;1;0Sc");
        assert_eq!(fed, b"abc");
        assert_eq!(replies, b"\x1b[?1;0;1024S\x1b[?2;0;800;480S");
        let (_, replies) = run(&mut sized(), b"\x1b[?3;1;0S\x1b[?2;3;9;9S");
        assert_eq!(replies, b"\x1b[?3;1;0S\x1b[?2;3;0S");
    }

    #[test]
    fn decsdm_puts_the_picture_at_the_top_left() {
        // FAILS IF sixel display mode is not followed: the image would be
        // drawn at the cursor and push the text down.
        let mut f = sized();
        let mut input = b"\x1b[?80h".to_vec();
        input.extend_from_slice(SIXEL);
        let (fed, _) = run(&mut f, &input);
        let text = String::from_utf8(fed).unwrap();
        assert!(text.starts_with("\x1b[?80h\x1b[1;1H\x1b_G"), "{text:?}");
        assert!(!text.contains("\x1bD"));
        // RIS ends it.
        let (fed, _) = run(&mut f, &[b"\x1bc", SIXEL].concat());
        assert!(String::from_utf8(fed).unwrap().contains("\x1bD"));
    }

    #[test]
    fn a_sixel_with_no_picture_still_ends_what_its_esc_ended() {
        // FAILS IF a swallowed sixel that draws nothing leaves the engine in
        // the string its `ESC` would have ended: the text after it would be
        // eaten as a title.
        let (fed, _) = run(&mut sized(), b"\x1b]2;t\x1bPq\x1b\\next");
        assert_eq!(fed, b"\x1b]2;t\x1b\\next");
        // Before any window said the cell's size, the same.
        let (fed, _) = run(&mut Inbound::new(), SIXEL);
        assert_eq!(fed, b"\x1b\\");
    }

    #[test]
    fn a_cancelled_sixel_draws_nothing() {
        // FAILS IF a sixel the program cancelled (CAN) is drawn anyway.
        let (fed, _) = run(&mut sized(), b"\x1bPq#1;2;100;0;0#1~~\x18x");
        assert_eq!(fed, b"\x18x");
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-inbound-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn a_kitty_file_medium_becomes_direct() {
        // FAILS IF an image sent by file reaches the engines as a file (a
        // mirror would read it again, or not at all), its bytes are not the
        // file's, or a kernel file can be read through it.
        use std::os::unix::ffi::OsStrExt as _;
        let dir = scratch("file");
        let image = dir.join("picture.png");
        std::fs::write(&image, b"\x89PNG-bytes-of-a-picture").unwrap();
        let path = b64(image.as_os_str().as_bytes());
        let input = format!("\x1b_Ga=T,f=100,t=f,i=7;{path}\x1b\\after");
        let (fed, _) = run(&mut sized(), input.as_bytes());
        let want = format!(
            "\x1b_Ga=T,f=100,i=7;{}\x1b\\after",
            b64(b"\x89PNG-bytes-of-a-picture")
        );
        assert_eq!(String::from_utf8(fed).unwrap(), want);
        assert!(
            image.exists(),
            "a file medium is not the protocol's to delete"
        );

        // With a size and an offset: exactly those bytes.
        let input = format!("\x1b_Gf=100,t=f,S=5,O=4;{path}\x1b\\");
        let (fed, _) = run(&mut sized(), input.as_bytes());
        assert_eq!(
            String::from_utf8(fed).unwrap(),
            format!("\x1b_Gf=100;{}\x1b\\", b64(b"-byte"))
        );

        // A kernel file is refused, and goes on as it came for the engine to
        // refuse in its own words.
        let input = format!("\x1b_Gt=f;{}\x1b\\", b64(b"/proc/self/environ"));
        let (fed, _) = run(&mut sized(), input.as_bytes());
        assert_eq!(fed, input.as_bytes());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_temporary_file_is_read_and_deleted_once() {
        // FAILS IF a temporary file survives being read (it piles up in
        // /tmp), is read where it may not be, or is read twice.
        use std::os::unix::ffi::OsStrExt as _;
        let dir = scratch("temporary");
        let file = dir.join("tty-graphics-protocol-1.rgba");
        std::fs::write(&file, [1u8, 2, 3, 4]).unwrap();
        let input = format!(
            "\x1b_Ga=t,f=32,s=1,v=1,t=t;{}\x1b\\",
            b64(file.as_os_str().as_bytes())
        );
        let (fed, _) = run(&mut sized(), input.as_bytes());
        assert_eq!(
            String::from_utf8(fed).unwrap(),
            format!("\x1b_Ga=t,f=32,s=1,v=1;{}\x1b\\", b64(&[1, 2, 3, 4]))
        );
        assert!(!file.exists(), "read once, and gone");
        // A file whose name does not say it is one is not taken.
        let plain = dir.join("notes.txt");
        std::fs::write(&plain, b"keep").unwrap();
        let input = format!("\x1b_Gt=t;{}\x1b\\", b64(plain.as_os_str().as_bytes()));
        let (fed, _) = run(&mut sized(), input.as_bytes());
        assert_eq!(fed, input.as_bytes());
        assert!(plain.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Runs `f` on a thread, and its answer — or `None` if it did not come in
    /// 20 s (it waits on something it should not).
    fn in_time<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
        let (said, heard) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = said.send(f());
        });
        heard.recv_timeout(std::time::Duration::from_secs(20)).ok()
    }

    #[test]
    fn a_fifo_named_as_a_medium_does_not_stop_the_terminal() {
        // FAILS IF the filter opens a medium a program named the blocking way:
        // a FIFO's open waits for a writer, on the engine's thread, and the
        // terminal — its input, its windows, its hang-up — waits with it.
        use rustix::fs::{CWD, FileType, Mode, mknodat};
        use std::os::unix::ffi::OsStrExt as _;
        let dir = scratch("fifo");
        let fifo = dir.join("tty-graphics-protocol-fifo");
        mknodat(CWD, &fifo, FileType::Fifo, Mode::from_raw_mode(0o600), 0).unwrap();
        let shm_name = format!("ronoctua-inbound-{}-fifo", std::process::id());
        let shm = Path::new("/dev/shm").join(&shm_name);
        let with_shm = mknodat(CWD, &shm, FileType::Fifo, Mode::from_raw_mode(0o600), 0).is_ok();
        let mut inputs = vec![
            format!("\x1b_Gt=f;{}\x1b\\", b64(fifo.as_os_str().as_bytes())),
            format!("\x1b_Gt=t;{}\x1b\\", b64(fifo.as_os_str().as_bytes())),
        ];
        if with_shm {
            inputs.push(format!("\x1b_Gt=s;{}\x1b\\", b64(shm_name.as_bytes())));
        }
        for input in inputs {
            let fed = in_time({
                let input = input.clone();
                move || run(&mut sized(), input.as_bytes()).0
            })
            .unwrap_or_else(|| panic!("the filter waited on a FIFO: {input:?}"));
            assert_eq!(fed, input.as_bytes(), "not a medium: it goes on as it came");
        }
        let _ = std::fs::remove_file(&shm);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shared_memory_is_an_object_and_not_a_link() {
        // FAILS IF a shared memory name that is a link is followed — the
        // protocol names an object in `/dev/shm`, and `shm_open` itself does
        // not follow one.
        let dir = scratch("shm-link");
        let target = dir.join("picture");
        std::fs::write(&target, [9u8; 16]).unwrap();
        let name = format!("ronoctua-inbound-{}-link", std::process::id());
        let link = Path::new("/dev/shm").join(&name);
        if std::os::unix::fs::symlink(&target, &link).is_err() {
            // No shared memory folder here to plant a link in.
            return;
        }
        let input = format!("\x1b_Gf=24,s=2,v=2,t=s;{}\x1b\\", b64(name.as_bytes()));
        let (fed, _) = run(&mut sized(), input.as_bytes());
        assert_eq!(fed, input.as_bytes());
        let _ = std::fs::remove_file(&link);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_is_held_is_bounded_and_goes_on_as_it_came() {
        // FAILS IF a sequence that never ends makes the filter hold it
        // without end — an `ESC` and the bytes the engine skips after it, an
        // introducer that re-enters itself, a kitty command padded with bytes
        // it ignores, a CSI of endless parameters — or what it held is not,
        // byte for byte, what came.
        for (start, filler) in [
            (&b"\x1b"[..], 0xA0u8),
            (b"\x1b", 0x1B),
            (b"\x1b", 0x07),
            (b"\x1b[", 0x9B),
            (b"\x1bP", 0x90),
            (b"\x1b_", 0xA0),
            (b"\x1b_G", 0xA0),
            (b"\x1b_Ga=T,t=f;", 0xA0),
            (b"\x1b[?", b';'),
            (b"\x1b[", b' '),
            (b"\x1bP", b';'),
        ] {
            let mut f = sized();
            let mut input = start.to_vec();
            let mut fed = run(&mut f, start).0;
            let chunk = vec![filler; 64 << 10];
            for _ in 0..16 {
                fed.extend(run(&mut f, &chunk).0);
                input.extend_from_slice(&chunk);
                assert!(
                    f.held.len() <= STRING_HELD_MAX
                        && f.apc.len() <= STRING_HELD_MAX
                        && f.params.numbers.len() <= PARAMS_MAX + 1
                        && f.params.intermediates.len() <= INTERMEDIATES_MAX,
                    "{start:?} then {filler:#x}: {} held",
                    f.held.len()
                );
            }
            fed.extend_from_slice(&f.held);
            assert!(
                fed == input,
                "{start:?} then {filler:#x}: the bytes changed"
            );
        }
        // And a sequence an escape spilled into is not taken out of the
        // stream afterwards: the `ESC` it began with has already gone on.
        let mut f = sized();
        let input = [&b"\x1b"[..], &[0xA0; HELD_MAX + 8], b"[?1;1;0S", SIXEL].concat();
        let (fed, replies) = run(&mut f, &input);
        assert!(replies.is_empty(), "no answer to a question that went on");
        assert!(fed.starts_with(&input[..input.len() - SIXEL.len()]));
    }

    #[test]
    fn a_chunked_or_direct_kitty_image_is_not_touched() {
        // FAILS IF the filter holds or rewrites what it has no reason to:
        // inline images are most of the traffic, and big.
        let big = "A".repeat(10_000);
        for input in [
            format!("\x1b_Ga=T,f=100,m=1;{big}\x1b\\\x1b_Gm=0;{big}\x1b\\"),
            "\x1b_Ga=d\x1b\\".to_owned(),
            format!("\x1b_Gt=f,m=1;{}\x1b\\", b64(b"/tmp/x")),
            "\x1bXsos\x1b\\\x1b^pm\x1b\\".to_owned(),
        ] {
            let (fed, _) = run(&mut sized(), input.as_bytes());
            assert_eq!(fed, input.as_bytes());
        }
    }

    #[test]
    fn many_chunks_carry_the_keys_once_and_end_with_m_0() {
        // FAILS IF a large image's commands are not the protocol's chunks:
        // the engine would drop the image or read keys as data.
        let mut out = Vec::new();
        kitty_commands("a=T,f=32", &[7u8; 6000], &mut out);
        let text = String::from_utf8(out).unwrap();
        let commands: Vec<&str> = text.split("\x1b\\").filter(|c| !c.is_empty()).collect();
        assert_eq!(commands.len(), 2);
        assert!(commands[0].starts_with("\x1b_Ga=T,f=32,m=1;"));
        assert!(commands[1].starts_with("\x1b_Gm=0;"));
    }
}
