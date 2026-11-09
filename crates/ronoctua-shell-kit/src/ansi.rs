//! Machine text with colour in it, as pieces a grid can draw.
//!
//! # Why the colour is kept
//!
//! The journal keeps whatever a unit wrote, escapes and all: a program that
//! colours its `error:` red says so with `ESC [ 31 m`. Drawn as it came, that
//! is a row of tofu hiding the line it decorated; thrown away, it is a log
//! whose errors look like everything else. [`pieces`] keeps the text and the
//! colour a terminal would have shown — as one of the sixteen
//! `terminal.ansi.*` roles of the page it is drawn on, never a colour of its
//! own (invariant 9). A 256-colour or 24-bit request is FOLDED to one of the
//! sixteen by arithmetic on its channels ([`of_256`], [`of_rgb`]): the hex a
//! program asks for never reaches the screen.
//!
//! # Why bold is a brighter colour
//!
//! A span has a colour and no weight (`ronoctua_render::text::Span`), and
//! bold over one of the eight is what most terminals drew as its bright form
//! before they had a heavier face: `1;31` is the bright red.
//!
//! # What is dropped, and why with what it carries
//!
//! Everything a grid cannot show. A CSI that is not a colour (a cursor moved,
//! a line erased) goes whole. An OSC, DCS, APC, PM or SOS goes WITH ITS
//! PAYLOAD up to its terminator: the old reader swallowed the introducer and
//! printed the rest, so an OSC-8 link left its address on the line. A
//! carriage return keeps only what comes after the last one — what a
//! progress bar redrawn in place ends as; a tab is one space, because these
//! columns belong to the table above and eight cells read as an indent that
//! means something; every other control character goes.
//!
//! # Each line on its own
//!
//! The state starts over at every call: one journal entry, one line, and a
//! colour a program left on never leaks into the next entry, which may be
//! another program's.

use ronoctua_tokens as tokens;

/// The sixteen roles, in the order a program numbers them: black, red,
/// green, yellow, blue, magenta, cyan, white, then their bright forms.
pub const SIXTEEN: [tokens::Color; 16] = [
    tokens::color::TERMINAL_ANSI_BLACK,
    tokens::color::TERMINAL_ANSI_RED,
    tokens::color::TERMINAL_ANSI_GREEN,
    tokens::color::TERMINAL_ANSI_YELLOW,
    tokens::color::TERMINAL_ANSI_BLUE,
    tokens::color::TERMINAL_ANSI_MAGENTA,
    tokens::color::TERMINAL_ANSI_CYAN,
    tokens::color::TERMINAL_ANSI_WHITE,
    tokens::color::TERMINAL_ANSI_BRIGHT_BLACK,
    tokens::color::TERMINAL_ANSI_BRIGHT_RED,
    tokens::color::TERMINAL_ANSI_BRIGHT_GREEN,
    tokens::color::TERMINAL_ANSI_BRIGHT_YELLOW,
    tokens::color::TERMINAL_ANSI_BRIGHT_BLUE,
    tokens::color::TERMINAL_ANSI_BRIGHT_MAGENTA,
    tokens::color::TERMINAL_ANSI_BRIGHT_CYAN,
    tokens::color::TERMINAL_ANSI_BRIGHT_WHITE,
];

/// One run of text in one colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// What is drawn: no escape and no control character is left in it.
    pub text: String,
    /// The role a program asked for; `None`: none, and the line's own colour
    /// is the one.
    pub color: Option<tokens::Color>,
}

/// The pieces `raw` is, in order — never an empty one, and two neighbours
/// never in the same colour. Pure.
#[must_use]
pub fn pieces(raw: &str) -> Vec<Piece> {
    let mut read = Reading {
        out: Vec::new(),
        sgr: Sgr::default(),
        returned: false,
    };
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ESC => escape(&mut chars, &mut read.sgr),
            // THE SAME FOUR, in their eight-bit forms (C1): one character each
            // once the line is text.
            CSI_8 => csi(&mut chars, &mut read.sgr),
            OSC_8 => {
                if string(&mut chars, true) {
                    escape(&mut chars, &mut read.sgr);
                }
            }
            DCS_8 | SOS_8 | PM_8 | APC_8 => {
                if string(&mut chars, false) {
                    escape(&mut chars, &mut read.sgr);
                }
            }
            '\r' => read.returned = true,
            '\t' => read.put(' '),
            c if c.is_control() => {}
            c => read.put(c),
        }
    }
    read.out
}

/// The text of `raw`, with no escape and no colour — what [`pieces`] draws,
/// joined.
#[must_use]
pub fn plain(raw: &str) -> String {
    pieces(raw).into_iter().map(|p| p.text).collect()
}

/// The role of a 256-colour index: the sixteen as they are; the 6×6×6 cube
/// by the channels it stands for (0, 95, 135, 175, 215, 255), folded as
/// [`of_rgb`] folds them — one colour is one role however it is spelt —;
/// the grey ramp by quarters: black, bright black, white, bright white.
#[must_use]
pub const fn of_256(n: u8) -> u8 {
    /// The channel a level of the cube stands for.
    const fn level(l: u8) -> u8 {
        if l == 0 { 0 } else { 55 + 40 * l }
    }
    match n {
        0..=15 => n,
        16..=231 => {
            let i = n - 16;
            of_rgb(level(i / 36), level(i / 6 % 6), level(i % 6))
        }
        _ => match (n - 232) / 6 {
            0 => 0,
            1 => 8,
            2 => 7,
            _ => 15,
        },
    }
}

/// The role of a 24-bit colour: the cube's rule over 0–255 — a channel is lit
/// above half (128), and the colour is bright once the strongest is above
/// three quarters (192).
#[must_use]
pub const fn of_rgb(r: u8, g: u8, b: u8) -> u8 {
    folded([r, g, b], 128, 192)
}

/// The index of the eight whose bits are the channels lit from `lit` (red 1,
/// green 2, blue 4: the order a program numbers them), plus 8 when the
/// strongest reaches `bright`.
const fn folded([r, g, b]: [u8; 3], lit: u8, bright: u8) -> u8 {
    let mut i = 0;
    if r >= lit {
        i |= 1;
    }
    if g >= lit {
        i |= 2;
    }
    if b >= lit {
        i |= 4;
    }
    let strongest = if r > g { r } else { g };
    let strongest = if strongest > b { strongest } else { b };
    if strongest >= bright { i + 8 } else { i }
}

const ESC: char = '\u{1b}';
const BEL: char = '\u{7}';
/// The string terminator, `ESC \` in its eight-bit form.
const ST_8: char = '\u{9c}';
const CSI_8: char = '\u{9b}';
const OSC_8: char = '\u{9d}';
const DCS_8: char = '\u{90}';
const SOS_8: char = '\u{98}';
const PM_8: char = '\u{9e}';
const APC_8: char = '\u{9f}';

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// The pieces so far, the colour asked for now, and whether a carriage
/// return waits for what overwrites the line.
struct Reading {
    out: Vec<Piece>,
    sgr: Sgr,
    /// A `\r` came, and nothing has been drawn after it yet: the next
    /// character starts the line again. A `\r` that ends the line (a
    /// `\r\n` a program wrote) keeps what came before it, as a terminal
    /// would.
    returned: bool,
}

impl Reading {
    fn put(&mut self, c: char) {
        if self.returned {
            self.out.clear();
            self.returned = false;
        }
        let color = self.sgr.color();
        match self.out.last_mut() {
            Some(last) if last.color == color => last.text.push(c),
            _ => self.out.push(Piece {
                text: c.to_string(),
                color,
            }),
        }
    }
}

/// What a program asked of the foreground.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Ink {
    /// Nothing: the line's own colour.
    #[default]
    Default,
    /// `30`–`37`: one of the eight, which bold brightens.
    Basic(u8),
    /// One of the sixteen as it is — `90`–`97`, or an index or a channel
    /// triple folded to one — bold or not.
    Fixed(u8),
}

/// The colour a run of SGRs leaves, as far as a grid draws it.
#[derive(Debug, Clone, Copy, Default)]
struct Sgr {
    ink: Ink,
    bold: bool,
}

impl Sgr {
    fn color(self) -> Option<tokens::Color> {
        let n = match self.ink {
            Ink::Default => return None,
            Ink::Basic(n) if self.bold => n + 8,
            Ink::Basic(n) | Ink::Fixed(n) => n,
        };
        SIXTEEN.get(usize::from(n)).copied()
    }

    /// One `ESC [ … m`'s parameters. What is not a foreground (a background,
    /// an underline, an italic) is read past and changes nothing here.
    fn apply(&mut self, params: &str) {
        let mut fields = params.split(';');
        while let Some(field) = fields.next() {
            // THE COLON FORM (`38:5:196`, `38:2::255:0:0`): one field holds
            // the whole request.
            if field.contains(':') {
                let sub: Vec<&str> = field.split(':').collect();
                if sub.first() == Some(&"38")
                    && let Some(ink) = extended(&sub[1..], true)
                {
                    self.ink = ink;
                }
                continue;
            }
            match number(field) {
                Some(0) => *self = Self::default(),
                Some(1) => self.bold = true,
                Some(22) => self.bold = false,
                Some(n @ 30..=37) => self.ink = Ink::Basic(n - 30),
                Some(39) => self.ink = Ink::Default,
                Some(n @ 90..=97) => self.ink = Ink::Fixed(n - 90 + 8),
                // A FOREGROUND, a background or an underline's colour by
                // index or by channels: the arguments are read past either
                // way, so the next field is never taken for one of them.
                Some(which @ (38 | 48 | 58)) => {
                    let args: Vec<&str> = match fields.next() {
                        Some("5") => vec!["5", fields.next().unwrap_or_default()],
                        Some("2") => {
                            let mut a = vec!["2"];
                            a.extend(fields.by_ref().take(3));
                            a
                        }
                        _ => break,
                    };
                    if which == 38
                        && let Some(ink) = extended(&args, false)
                    {
                        self.ink = ink;
                    }
                }
                _ => {}
            }
        }
    }
}

/// A field's number; an empty field is 0, as a terminal reads `ESC [ m`.
fn number(field: &str) -> Option<u8> {
    if field.is_empty() {
        return Some(0);
    }
    field.parse().ok()
}

/// `5;n` or `2;r;g;b` after a `38` — the colon form may carry a colour
/// space's id before the channels (`2::r:g:b`).
fn extended(args: &[&str], colon: bool) -> Option<Ink> {
    match args {
        ["5", n, ..] => Some(Ink::Fixed(of_256(n.parse().ok()?))),
        ["2", rest @ ..] => {
            let channels = if colon && rest.len() >= 4 {
                &rest[1..4]
            } else {
                rest.get(..3)?
            };
            let [r, g, b] = [channels[0], channels[1], channels[2]].map(|c| c.parse::<u8>().ok());
            Some(Ink::Fixed(of_rgb(r?, g?, b?)))
        }
        _ => None,
    }
}

/// What follows an `ESC`: a CSI, a string, or an escape of its own (`ESC 7`,
/// `ESC ( B`) — read and dropped, a colour kept.
fn escape(chars: &mut Chars<'_>, sgr: &mut Sgr) {
    match chars.next() {
        Some('[') => csi(chars, sgr),
        Some(']') => {
            if string(chars, true) {
                escape(chars, sgr);
            }
        }
        Some('P' | 'X' | '^' | '_') => {
            if string(chars, false) {
                escape(chars, sgr);
            }
        }
        // INTERMEDIATES, then the one character that ends it.
        Some(' '..='/') => {
            let _ = chars.find(|c| !(' '..='/').contains(c));
        }
        // `ESC` and one character (`ESC 7`, `ESC =`, a stray `ESC \`), or
        // nothing at all at the line's end.
        _ => {}
    }
}

/// A control sequence after its introducer: the parameters, then the
/// intermediates, then the character that ends it. Only `m` with plain
/// parameters is a colour; a private one (`ESC [ > 4 ; 2 m`) is not. A
/// character no sequence holds ends it unread, and is read as itself.
fn csi(chars: &mut Chars<'_>, sgr: &mut Sgr) {
    let mut params = String::new();
    let mut plain = true;
    while let Some(&c) = chars.peek() {
        match c {
            '0'..='?' => {
                params.push(c);
                chars.next();
            }
            ' '..='/' => {
                plain = false;
                chars.next();
            }
            '@'..='~' => {
                chars.next();
                if c == 'm' && plain && !params.starts_with(['<', '=', '>', '?']) {
                    sgr.apply(&params);
                }
                return;
            }
            _ => return,
        }
    }
}

/// A string (OSC, DCS, SOS, PM, APC) read past with its payload, up to its
/// terminator: `ESC \` or its eight-bit form, and for an OSC a `BEL` too. A
/// line that ends first ends it. `true` when an `ESC` that does not
/// terminate it came: that one begins a sequence of its own, which the
/// caller reads.
fn string(chars: &mut Chars<'_>, osc: bool) -> bool {
    while let Some(c) = chars.next() {
        match c {
            ESC => {
                if chars.peek() == Some(&'\\') {
                    chars.next();
                    return false;
                }
                return true;
            }
            ST_8 => return false,
            BEL if osc => return false,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str, color: Option<tokens::Color>) -> Piece {
        Piece {
            text: text.to_owned(),
            color,
        }
    }

    const RED: Option<tokens::Color> = Some(tokens::color::TERMINAL_ANSI_RED);
    const BRIGHT_RED: Option<tokens::Color> = Some(tokens::color::TERMINAL_ANSI_BRIGHT_RED);

    /// FAILS IF: a basic foreground is not its role, the reset or `39` does
    /// not end it, or the text around it is lost.
    #[test]
    fn a_basic_colour_is_its_role_until_it_is_reset() {
        assert_eq!(
            pieces("a \u{1b}[31merror\u{1b}[0m: boom"),
            vec![one("a ", None), one("error", RED), one(": boom", None)]
        );
        assert_eq!(
            pieces("\u{1b}[32mok\u{1b}[39m."),
            vec![
                one("ok", Some(tokens::color::TERMINAL_ANSI_GREEN)),
                one(".", None)
            ]
        );
        assert_eq!(
            pieces("\u{1b}[33mwarn\u{1b}[mplain"),
            vec![
                one("warn", Some(tokens::color::TERMINAL_ANSI_YELLOW)),
                one("plain", None)
            ],
            "an empty SGR is a reset"
        );
    }

    /// FAILS IF: bold over one of the eight is not its bright form, in either
    /// order — or bold over a bright one, or over nothing, changes it.
    #[test]
    fn bold_brightens_the_eight_and_nothing_else() {
        assert_eq!(pieces("\u{1b}[1;31mx"), vec![one("x", BRIGHT_RED)]);
        assert_eq!(pieces("\u{1b}[31;1mx"), vec![one("x", BRIGHT_RED)]);
        assert_eq!(pieces("\u{1b}[1m\u{1b}[31mx"), vec![one("x", BRIGHT_RED)]);
        assert_eq!(
            pieces("\u{1b}[1;31mx\u{1b}[22my"),
            vec![one("x", BRIGHT_RED), one("y", RED)]
        );
        assert_eq!(pieces("\u{1b}[91mx"), vec![one("x", BRIGHT_RED)]);
        assert_eq!(pieces("\u{1b}[1;91mx"), vec![one("x", BRIGHT_RED)]);
        assert_eq!(pieces("\u{1b}[1mx"), vec![one("x", None)]);
        assert_eq!(
            pieces("\u{1b}[1;38;5;1mx"),
            vec![one("x", RED)],
            "an index is the role it names, bold or not"
        );
    }

    /// FAILS IF: a 256-colour or 24-bit request is not folded to the nearest
    /// of the sixteen — in either spelling — or its arguments are read as
    /// SGRs of their own.
    #[test]
    fn many_colours_fold_to_the_sixteen() {
        assert_eq!(pieces("\u{1b}[38;5;196mx"), vec![one("x", BRIGHT_RED)]);
        assert_eq!(pieces("\u{1b}[38;5;124mx"), vec![one("x", RED)]);
        assert_eq!(pieces("\u{1b}[38:5:196mx"), vec![one("x", BRIGHT_RED)]);
        assert_eq!(pieces("\u{1b}[38;5;9mx"), vec![one("x", BRIGHT_RED)]);
        assert_eq!(pieces("\u{1b}[38;2;255;0;0mx"), vec![one("x", BRIGHT_RED)]);
        assert_eq!(pieces("\u{1b}[38;2;160;0;0mx"), vec![one("x", RED)]);
        assert_eq!(pieces("\u{1b}[38:2::255:0:0mx"), vec![one("x", BRIGHT_RED)]);
        assert_eq!(
            pieces("\u{1b}[38;2;1;1;1mx"),
            vec![one("x", Some(tokens::color::TERMINAL_ANSI_BLACK))],
            "the `1`s are channels, never bold"
        );
        // THE CUBE: which channels are lit, and the strongest.
        assert_eq!(of_256(16), 0, "the cube's black");
        assert_eq!(of_256(231), 15, "the cube's white");
        assert_eq!(of_256(16 + 36 * 3 + 6 * 3), 3, "a yellow lit, not bright");
        assert_eq!(of_256(16 + 6 * 5 + 5), 14, "a bright cyan");
        // …WHICH IS THE 24-BIT RULE over the channels each level stands for:
        // a dark red (`#870000`) is a red, never the black.
        for (n, rgb) in [(88, (135, 0, 0)), (28, (0, 135, 0)), (102, (135, 135, 135))] {
            assert_eq!(of_256(n), of_rgb(rgb.0, rgb.1, rgb.2), "{n}");
        }
        assert_eq!(of_256(88), 1, "the cube's dark red");
        // THE GREYS, by quarters.
        assert_eq!(
            [232, 238, 244, 250, 255].map(of_256),
            [0, 8, 7, 15, 15],
            "the grey ramp"
        );
        assert_eq!(of_rgb(0, 200, 0), 10, "a bright green");
        assert_eq!(of_rgb(150, 150, 150), 7, "a light grey is white");
        // A BACKGROUND by channels is read past whole.
        assert_eq!(
            pieces("\u{1b}[48;2;1;31;0;31mx"),
            vec![one("x", RED)],
            "a background's channels were read as SGRs"
        );
    }

    /// FAILS IF: an OSC (a hyperlink, a title), a DCS or an APC leaves any of
    /// its payload on the line, by either terminator — or one cut off at the
    /// line's end leaks.
    #[test]
    fn strings_go_with_their_payload() {
        let link = "\u{1b}]8;;https://example.org/x\u{1b}\\here\u{1b}]8;;\u{1b}\\ and on";
        assert_eq!(plain(link), "here and on");
        assert_eq!(plain("\u{1b}]0;a title\u{7}text"), "text");
        assert_eq!(plain("\u{1b}Pq#0;2;0;0;0\u{1b}\\after"), "after");
        assert_eq!(plain("\u{1b}_payload\u{1b}\\after"), "after");
        assert_eq!(plain("\u{9d}8;;https://example.org\u{9c}text"), "text");
        assert_eq!(plain("before\u{1b}]8;;https://cut-off"), "before");
        assert_eq!(
            pieces("\u{1b}]0;title\u{1b}[31mx"),
            vec![one("x", RED)],
            "an ESC inside a string begins a sequence of its own"
        );
    }

    /// FAILS IF: a control sequence that is not a colour leaves a character
    /// behind, or a private `m` is read as a colour.
    #[test]
    fn other_sequences_are_dropped_whole() {
        assert_eq!(plain("\u{1b}[2Kline\u{1b}[?25l"), "line");
        assert_eq!(plain("\u{1b}(Bcharset"), "charset");
        assert_eq!(plain("\u{1b}7saved\u{1b}8"), "saved");
        assert_eq!(pieces("\u{1b}[>4;2mx"), vec![one("x", None)]);
        assert_eq!(pieces("\u{9b}31mx"), vec![one("x", RED)]);
    }

    /// FAILS IF: a carriage return does not leave only what was drawn after
    /// it (and the colour asked before it), a `\r` that ends the line loses
    /// the line, a tab is not one space, or another control is kept.
    #[test]
    fn a_return_a_tab_and_the_controls() {
        assert_eq!(plain("carriage\rreturn"), "return");
        assert_eq!(plain("10%\r50%\r100%"), "100%");
        assert_eq!(plain("a line\r"), "a line");
        assert_eq!(pieces("\u{1b}[31m10%\r\u{1b}[K50%"), vec![one("50%", RED)]);
        assert_eq!(plain("a\tb"), "a b");
        assert_eq!(plain("bell\u{7} and\u{8} back\u{7f}"), "bell and back");
        // AND NOTHING IS INVENTED: text with none of that comes back whole.
        assert_eq!(
            // pt-BR: the accent is the subject — no letter may be touched.
            plain("acentuação e ünicode 日本"),
            "acentuação e ünicode 日本"
        );
    }

    /// FAILS IF: a colour left on at the end of one call colours the next —
    /// each journal entry starts over.
    #[test]
    fn each_line_starts_over() {
        assert_eq!(pieces("\u{1b}[31mstill red"), vec![one("still red", RED)]);
        assert_eq!(pieces("next"), vec![one("next", None)]);
        assert!(pieces("").is_empty());
        assert!(pieces("\u{1b}[31m\u{1b}[0m").is_empty(), "an empty piece");
    }
}
