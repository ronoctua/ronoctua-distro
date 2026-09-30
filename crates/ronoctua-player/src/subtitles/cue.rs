//! The one shape every reader gives: timed cues of styled lines.
//!
//! A reader keeps what a subtitle SAYS — the words, where a line breaks,
//! italics, bold, underline, a colour, and whether the cue asked for the top
//! of the picture — and drops how one format happens to spell it. The merge
//! (`merge.rs`) then writes every lane in one language, ASS, which is the one
//! mpv's libass draws.
//!
//! The helpers under the types are shared by the readers: clock times, the
//! HTML-like markup of SRT, WebVTT and SAMI, and the tidying every reader's
//! output goes through.

use crate::subtitles::FrameRate;

/// Where a cue sits: the lane's own [`Band`], unless the file put the cue at
/// the top (`{\an8}` in an SRT, `line:0` in a WebVTT, a TTML region in the
/// upper half) — to clear text burnt into the picture, or to tell a speaker
/// apart. The same two places a lane can be stacked in.
///
/// Only the TOP is a request: the foot is where every format puts a cue that
/// says nothing, so a cue at the foot follows its lane, and a lane the person
/// stacked at the top takes all of it there.
pub use ronoctua_playlist::own::Stack as Band;

/// A colour as a subtitle file names it, eight bits a channel.
///
/// Never a colour of the interface: it is what the FILE said (`<font
/// color="yellow">`, a teletext colour code) or what the caller resolved from
/// its tokens and hands to the merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red, 0–255.
    pub red: u8,
    /// Green, 0–255.
    pub green: u8,
    /// Blue, 0–255.
    pub blue: u8,
}

impl Rgb {
    /// A colour from its three channels.
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    /// `#rgb`, `#rrggbb` or `#rrggbbaa` (the alpha is dropped: a subtitle is
    /// drawn opaque). The `#` may be missing.
    #[must_use]
    pub fn hex(text: &str) -> Option<Self> {
        let digits = text.trim().trim_start_matches('#');
        if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(digits.get(i..i + 2)?, 16).ok();
        match digits.len() {
            3 => {
                let nibble = |i: usize| {
                    let v = u8::from_str_radix(digits.get(i..=i)?, 16).ok()?;
                    Some(v * 17)
                };
                Some(Self::new(nibble(0)?, nibble(1)?, nibble(2)?))
            }
            6 | 8 => Some(Self::new(byte(0)?, byte(2)?, byte(4)?)),
            _ => None,
        }
    }

    /// A colour as CSS and TTML write it: hex, `rgb(r, g, b)`, `rgba(r, g,
    /// b, a)` or one of the names HTML 4 defined (which is what subtitle
    /// files use; WebVTT's colour classes are among them).
    #[must_use]
    pub fn css(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.starts_with('#') {
            return Self::hex(text);
        }
        let lower = text.to_ascii_lowercase();
        if let Some(inner) = lower
            .strip_prefix("rgba(")
            .or_else(|| lower.strip_prefix("rgb("))
            .and_then(|rest| rest.strip_suffix(')'))
        {
            let mut parts = inner.split(',').map(|p| p.trim().parse::<u8>().ok());
            return Some(Self::new(parts.next()??, parts.next()??, parts.next()??));
        }
        Self::named(&lower)
    }

    /// One of the sixteen colours HTML 4 names, and `orange`.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        let (red, green, blue) = match name.trim().to_ascii_lowercase().as_str() {
            "black" => (0, 0, 0),
            "silver" => (192, 192, 192),
            "gray" | "grey" => (128, 128, 128),
            "white" => (255, 255, 255),
            "maroon" => (128, 0, 0),
            "red" => (255, 0, 0),
            "purple" => (128, 0, 128),
            "fuchsia" | "magenta" => (255, 0, 255),
            "green" => (0, 128, 0),
            "lime" => (0, 255, 0),
            "olive" => (128, 128, 0),
            "yellow" => (255, 255, 0),
            "navy" => (0, 0, 128),
            "blue" => (0, 0, 255),
            "teal" => (0, 128, 128),
            "aqua" | "cyan" => (0, 255, 255),
            "orange" => (255, 165, 0),
            _ => return None,
        };
        Some(Self::new(red, green, blue))
    }

    /// An ASS colour: `&HAABBGGRR`, `&HBBGGRR`, with or without the closing
    /// `&`, or the decimal integer SSA files often carry. Blue comes FIRST.
    #[must_use]
    pub fn ass(text: &str) -> Option<Self> {
        let text = text.trim().trim_end_matches('&');
        let value = if let Some(hex) = text.strip_prefix("&H").or_else(|| text.strip_prefix("&h")) {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            let decimal = text.parse::<i64>().ok()?;
            u32::try_from(decimal & 0xFFFF_FFFF).ok()?
        };
        let [red, green, blue, _] = value.to_le_bytes();
        Some(Self::new(red, green, blue))
    }

    /// The colour as an ASS style writes it: `&H00BBGGRR`, opaque.
    #[must_use]
    pub fn ass_style(self) -> String {
        format!("&H00{:02X}{:02X}{:02X}", self.blue, self.green, self.red)
    }

    /// The colour as an ASS override writes it: `&HBBGGRR&`.
    #[must_use]
    pub fn ass_override(self) -> String {
        format!("&H{:02X}{:02X}{:02X}&", self.blue, self.green, self.red)
    }
}

/// A run of text drawn one way.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Span {
    /// The words, with no line break in them: breaks are [`Cue::lines`].
    pub text: String,
    /// Slanted.
    pub italic: bool,
    /// Heavy.
    pub bold: bool,
    /// Underlined.
    pub underline: bool,
    /// The colour the FILE asked for; `None` is the lane's own.
    pub colour: Option<Rgb>,
}

impl Span {
    /// Text drawn the lane's way.
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    /// Whether the two are drawn the same way (so they can be one span).
    #[must_use]
    pub fn looks_like(&self, other: &Self) -> bool {
        self.italic == other.italic
            && self.bold == other.bold
            && self.underline == other.underline
            && self.colour == other.colour
    }
}

/// One subtitle: lines shown from `start_ms` until `end_ms`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cue {
    /// When it appears, in milliseconds on the FILE's clock.
    pub start_ms: u64,
    /// When it goes, in milliseconds on the file's clock; after `start_ms`.
    pub end_ms: u64,
    /// The lines, top first, each a run of spans.
    pub lines: Vec<Vec<Span>>,
    /// `Some(Band::Top)` when the file put the cue at the top; `None`
    /// follows the lane. Readers never ask for the foot.
    pub band: Option<Band>,
}

impl Cue {
    /// A cue of plain lines, one per `\n` of `text`.
    #[must_use]
    pub fn plain(start_ms: u64, end_ms: u64, text: &str) -> Self {
        Self {
            start_ms,
            end_ms,
            lines: text.split('\n').map(|l| vec![Span::plain(l)]).collect(),
            band: None,
        }
    }

    /// The words, one line per `\n`, without their looks.
    #[must_use]
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.iter().map(|s| s.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Puts a reader's cues in the one form the rest expects.
///
/// Spans that look alike are joined, empty ones dropped, each line trimmed,
/// empty lines at the top and bottom dropped; a cue left with no words, or
/// that ends before it starts, goes; and the cues are sorted by start, keeping
/// the file's order among cues that start together (it is their stacking
/// order).
pub(crate) fn tidy(cues: &mut Vec<Cue>) {
    for cue in cues.iter_mut() {
        for line in &mut cue.lines {
            join_alike(line);
            trim_line(line);
        }
        while cue.lines.first().is_some_and(|l| l.is_empty()) {
            cue.lines.remove(0);
        }
        while cue.lines.last().is_some_and(|l| l.is_empty()) {
            cue.lines.pop();
        }
    }
    cues.retain(|c| c.end_ms > c.start_ms && !c.lines.is_empty());
    cues.sort_by_key(|c| c.start_ms);
}

fn join_alike(line: &mut Vec<Span>) {
    let mut joined: Vec<Span> = Vec::with_capacity(line.len());
    for span in line.drain(..) {
        if span.text.is_empty() {
            continue;
        }
        match joined.last_mut() {
            Some(last) if last.looks_like(&span) => last.text.push_str(&span.text),
            _ => joined.push(span),
        }
    }
    *line = joined;
}

fn trim_line(line: &mut Vec<Span>) {
    while let Some(first) = line.first_mut() {
        let kept = first.text.trim_start().len();
        first.text.drain(..first.text.len() - kept);
        if first.text.is_empty() {
            line.remove(0);
        } else {
            break;
        }
    }
    while let Some(last) = line.last_mut() {
        let kept = last.text.trim_end().len();
        last.text.truncate(kept);
        if last.text.is_empty() {
            line.pop();
        } else {
            break;
        }
    }
}

/// How long a cue with no end of its own stays: until the next one, and at
/// most this long.
pub(crate) const UNTIMED_MOST_MS: u64 = 8_000;

/// The end of a cue whose format gives only starts (TMPlayer, a MicroDVD or
/// MPL2 cue with an empty end): the next cue's start, at most
/// [`UNTIMED_MOST_MS`] later.
pub(crate) fn untimed_end(start_ms: u64, next_ms: Option<u64>) -> u64 {
    let most = start_ms + UNTIMED_MOST_MS;
    next_ms
        .filter(|&n| n > start_ms)
        .map_or(most, |n| n.min(most))
}

/// The milliseconds of a decimal fraction's digits: `5` → 500, `05` → 50,
/// `500` → 500, `5004` → 500. Digits past the third are below a millisecond.
pub(crate) fn fraction_ms(digits: &str) -> Option<u64> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut ms = 0u64;
    for (i, b) in digits.bytes().take(3).enumerate() {
        ms += u64::from(b - b'0') * [100, 10, 1][i];
    }
    Some(ms)
}

/// A clock time in milliseconds: `h:mm:ss`, `h:mm:ss.fff`, `h:mm:ss,fff` or
/// `mm:ss.fff`. Every field is digits; the fraction may be any length.
pub(crate) fn clock_ms(text: &str) -> Option<u64> {
    let text = text.trim();
    let (whole, fraction) = match text.rfind(['.', ',']) {
        Some(at) if text[..at].contains(':') => (&text[..at], Some(&text[at + 1..])),
        _ => (text, None),
    };
    let fields: Vec<&str> = whole.split(':').collect();
    if !(2..=3).contains(&fields.len()) {
        return None;
    }
    let mut total = 0u64;
    for field in &fields {
        if field.is_empty() || field.len() > 3 || !field.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        total = total * 60 + field.parse::<u64>().ok()?;
    }
    let ms = match fraction {
        Some(f) => fraction_ms(f)?,
        None => 0,
    };
    Some(total * 1000 + ms)
}

/// `frames` at `rate`, in milliseconds.
pub(crate) fn frames_ms(frames: u64, rate: FrameRate) -> u64 {
    let numerator = u64::from(rate.numerator.max(1));
    frames.saturating_mul(1000 * u64::from(rate.denominator)) / numerator
}

/// Which markup a line of text is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    /// SubRip: `<i> <b> <u> <font color>` and ASS override blocks; a newline
    /// breaks the line; an unknown `<tag>` is text.
    SubRip,
    /// WebVTT: `<i> <b> <u> <c.class> <v> <lang> <ruby>`, timestamps and
    /// entities; a newline breaks the line; an unknown tag is dropped.
    WebVtt,
    /// HTML, as SAMI and YouTube's first format write it: whitespace
    /// collapses, `<br>` breaks the line, entities are decoded.
    Html,
}

/// What a markup walk is drawing with, and how deep each look goes.
#[derive(Debug, Clone, Default)]
struct Pen {
    italic: u32,
    bold: u32,
    underline: u32,
    colours: Vec<Option<Rgb>>,
    /// Inside `<rt>`: ruby text, which is not drawn.
    ruby_text: u32,
}

impl Pen {
    fn span(&self, text: String) -> Span {
        Span {
            text,
            italic: self.italic > 0,
            bold: self.bold > 0,
            underline: self.underline > 0,
            colour: self.colours.iter().rev().find_map(|c| *c),
        }
    }
}

/// Reads one cue's markup into lines of spans, and the band an ASS
/// `{\anN}` inside it asked for.
pub(crate) fn markup(text: &str, dialect: Dialect) -> (Vec<Vec<Span>>, Option<Band>) {
    let mut walk = Walk {
        lines: vec![Vec::new()],
        pen: Pen::default(),
        word: String::new(),
        band: None,
        dialect,
    };
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c == '<'
            && let Some(taken) = walk.tag(rest)
        {
            rest = &rest[taken..];
            continue;
        }
        if c == '{'
            && dialect == Dialect::SubRip
            && let Some(taken) = walk.override_block(rest)
        {
            rest = &rest[taken..];
            continue;
        }
        if c == '&'
            && dialect != Dialect::SubRip
            && let Some((decoded, taken)) = entity(rest)
        {
            walk.push_char(decoded);
            rest = &rest[taken..];
            continue;
        }
        walk.push_char(c);
        rest = &rest[c.len_utf8()..];
    }
    walk.flush();
    (walk.lines, walk.band)
}

struct Walk {
    lines: Vec<Vec<Span>>,
    pen: Pen,
    word: String,
    band: Option<Band>,
    dialect: Dialect,
}

impl Walk {
    fn flush(&mut self) {
        if self.word.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.word);
        if self.pen.ruby_text > 0 {
            return;
        }
        let span = self.pen.span(text);
        if let Some(line) = self.lines.last_mut() {
            line.push(span);
        }
    }

    fn push_char(&mut self, c: char) {
        match (c, self.dialect) {
            ('\n', Dialect::SubRip | Dialect::WebVtt) => self.break_line(),
            (c, Dialect::Html) if c.is_whitespace() && c != '\u{a0}' => {
                let last = self.word.chars().next_back().or_else(|| {
                    self.lines
                        .last()
                        .and_then(|l| l.last())
                        .and_then(|s| s.text.chars().next_back())
                });
                if last.is_some_and(|l| l != ' ') {
                    self.word.push(' ');
                }
            }
            ('\t', _) => self.word.push(' '),
            (c, _) => self.word.push(c),
        }
    }

    fn break_line(&mut self) {
        self.flush();
        self.lines.push(Vec::new());
    }

    /// Reads the tag at the start of `rest`; the bytes it took, or `None`
    /// when `<` is just text there.
    fn tag(&mut self, rest: &str) -> Option<usize> {
        let end = rest.find('>')?;
        let inner = &rest[1..end];
        if inner.contains('<') || inner.contains('\n') {
            return None;
        }
        let (closing, body) = match inner.strip_prefix('/') {
            Some(b) => (true, b),
            None => (false, inner),
        };
        let body = body.trim_end_matches('/').trim();
        // A WebVTT timestamp inside a cue (`<00:00:01.000>`): karaoke timing,
        // not drawn.
        if self.dialect == Dialect::WebVtt && body.starts_with(|c: char| c.is_ascii_digit()) {
            return clock_ms(body).map(|_| end + 1);
        }
        let name_end = body
            .find(|c: char| c.is_whitespace() || c == '.')
            .unwrap_or(body.len());
        let name = body[..name_end].to_ascii_lowercase();
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return None;
        }
        let attributes = &body[name_end..];
        let known = matches!(
            name.as_str(),
            "i" | "em"
                | "b"
                | "strong"
                | "u"
                | "font"
                | "br"
                | "c"
                | "v"
                | "lang"
                | "ruby"
                | "rt"
                | "s"
                | "span"
                | "p"
        );
        if !known && self.dialect == Dialect::SubRip {
            return None;
        }
        self.flush();
        let bump = |depth: &mut u32| {
            if closing {
                *depth = depth.saturating_sub(1);
            } else {
                *depth += 1;
            }
        };
        match name.as_str() {
            "i" | "em" => bump(&mut self.pen.italic),
            "b" | "strong" => bump(&mut self.pen.bold),
            "u" => bump(&mut self.pen.underline),
            "rt" => bump(&mut self.pen.ruby_text),
            "br" => self.break_line(),
            "font" => {
                if closing {
                    self.pen.colours.pop();
                } else {
                    self.pen
                        .colours
                        .push(attribute(attributes, "color").and_then(|c| Rgb::css(&c)));
                }
            }
            "c" => {
                if closing {
                    self.pen.colours.pop();
                } else {
                    // `<c.yellow.bg_black>`: the first class that names a
                    // colour is the text's.
                    let colour = attributes
                        .split(|c: char| c == '.' || c.is_whitespace())
                        .find_map(Rgb::named);
                    self.pen.colours.push(colour);
                }
            }
            _ => {}
        }
        Some(end + 1)
    }

    /// Reads an ASS override block inside an SRT (`{\i1}`, `{\an8}`); the
    /// bytes it took, or `None` when the braces are just text.
    fn override_block(&mut self, rest: &str) -> Option<usize> {
        let end = rest.find('}')?;
        let inner = &rest[1..end];
        if !inner.starts_with('\\') {
            return None;
        }
        self.flush();
        for tag in inner.split('\\').filter(|t| !t.is_empty()) {
            let flag = |t: &str| t.trim() != "0";
            if let Some(v) = tag.strip_prefix("an") {
                self.band = band_of_numpad(v.trim().parse().unwrap_or(0)).or(self.band);
            } else if let Some(v) = tag
                .strip_prefix('a')
                .filter(|v| v.bytes().all(|b| b.is_ascii_digit()) && !v.is_empty())
            {
                self.band = band_of_legacy(v.parse().unwrap_or(0)).or(self.band);
            } else if let Some(v) = tag.strip_prefix('i').filter(|v| v.len() == 1) {
                self.pen.italic = u32::from(flag(v));
            } else if let Some(v) = tag.strip_prefix('b').filter(|v| v.len() == 1) {
                self.pen.bold = u32::from(flag(v));
            } else if let Some(v) = tag.strip_prefix('u').filter(|v| v.len() == 1) {
                self.pen.underline = u32::from(flag(v));
            } else if let Some(v) = tag
                .strip_prefix("1c")
                .or_else(|| tag.strip_prefix('c'))
                .filter(|v| v.is_empty() || v.starts_with('&'))
            {
                let colour = Rgb::ass(v);
                self.pen.colours.clear();
                self.pen.colours.push(colour);
            }
        }
        Some(end + 1)
    }
}

/// The band of an ASS numpad alignment (`\an`): 7–9 are the top; the rest
/// follows the lane.
pub(crate) fn band_of_numpad(value: u8) -> Option<Band> {
    (7..=9).contains(&value).then_some(Band::Top)
}

/// The band of an SSA legacy alignment (`\a`, and SSA styles): 5–7 are the
/// top; the rest follows the lane.
pub(crate) fn band_of_legacy(value: u8) -> Option<Band> {
    (5..=7).contains(&value).then_some(Band::Top)
}

/// An attribute's value in a tag's attribute text (`color="#ff0"`,
/// `color=red`), matched without regard to case.
pub(crate) fn attribute(attributes: &str, name: &str) -> Option<String> {
    let lower = attributes.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(name) {
        let at = from + found;
        from = at + name.len();
        let before_ok = at == 0 || !lower.as_bytes()[at - 1].is_ascii_alphanumeric();
        let after = lower[from..].trim_start();
        if !before_ok || !after.starts_with('=') {
            continue;
        }
        let value_at = attributes.len() - after.len() + 1;
        let value = attributes[value_at..].trim_start();
        return Some(match value.chars().next() {
            Some(q @ ('"' | '\'')) => value[1..].split(q).next().unwrap_or("").to_owned(),
            _ => value
                .split(|c: char| c.is_whitespace() || c == '>')
                .next()
                .unwrap_or("")
                .to_owned(),
        });
    }
    None
}

/// Decodes the entity at the start of `rest` (`&amp;`, `&#233;`, `&#xE9;`);
/// the character and the bytes it took.
pub(crate) fn entity(rest: &str) -> Option<(char, usize)> {
    // An entity's name is short: `;` within the first dozen bytes.
    let end = rest
        .char_indices()
        .take_while(|(at, _)| *at < 12)
        .find(|(_, c)| *c == ';')?
        .0;
    let name = &rest[1..end];
    let c = if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse().ok()?,
        };
        char::from_u32(code)?
    } else {
        match name {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            "nbsp" => '\u{a0}',
            "lrm" => '\u{200e}',
            "rlm" => '\u{200f}',
            _ => return None,
        }
    };
    Some((c, end + 1))
}

/// Every entity of `text` decoded; what is not one stays as it was.
pub(crate) fn entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        match entity(rest) {
            Some((c, taken)) => {
                out.push(c);
                rest = &rest[taken..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clock_reads_every_spelling() {
        assert_eq!(clock_ms("01:02:03,456"), Some(3_723_456));
        assert_eq!(clock_ms("1:02:03.4"), Some(3_723_400));
        assert_eq!(clock_ms("02:03.450"), Some(123_450));
        assert_eq!(clock_ms("00:00:05"), Some(5_000));
        assert_eq!(clock_ms("5.5"), None);
        assert_eq!(clock_ms("aa:00:01"), None);
    }

    #[test]
    fn subrip_markup_keeps_looks_and_the_top_band() {
        let (lines, band) = markup(
            "{\\an8}<i>Olá</i>, <font color=\"#ff0000\">mundo</font>\n<b>dois</b> <3",
            Dialect::SubRip,
        );
        assert_eq!(band, Some(Band::Top));
        assert_eq!(lines.len(), 2);
        assert!(lines[0][0].italic);
        assert_eq!(lines[0][0].text, "Olá");
        assert_eq!(lines[0][2].colour, Some(Rgb::new(255, 0, 0)));
        assert!(lines[1][0].bold);
        assert_eq!(lines[1][1].text, " <3");
    }

    #[test]
    fn html_markup_collapses_whitespace_and_breaks_on_br() {
        let (lines, _) = markup("one\n   two<br>three&nbsp;&amp;", Dialect::Html);
        let text: Vec<String> = lines
            .iter()
            .map(|l| l.iter().map(|s| s.text.clone()).collect())
            .collect();
        assert_eq!(text, vec!["one two".to_owned(), "three\u{a0}&".to_owned()]);
    }

    #[test]
    fn ass_colours_are_blue_first() {
        assert_eq!(Rgb::ass("&H00563412"), Some(Rgb::new(0x12, 0x34, 0x56)));
        assert_eq!(Rgb::ass("&H563412&"), Some(Rgb::new(0x12, 0x34, 0x56)));
        assert_eq!(Rgb::new(0x12, 0x34, 0x56).ass_style(), "&H00563412");
        assert_eq!(Rgb::new(0x12, 0x34, 0x56).ass_override(), "&H563412&");
        assert_eq!(Rgb::ass("16777215"), Some(Rgb::new(255, 255, 255)));
    }

    #[test]
    fn tidy_joins_trims_and_drops_what_says_nothing() {
        let mut cues = vec![
            Cue {
                start_ms: 5,
                end_ms: 9,
                lines: vec![vec![], vec![Span::plain("  a"), Span::plain("b  ")], vec![]],
                band: None,
            },
            Cue::plain(1, 1, "zero length"),
            Cue::plain(0, 4, "  "),
        ];
        tidy(&mut cues);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].lines, vec![vec![Span::plain("ab")]]);
    }
}
