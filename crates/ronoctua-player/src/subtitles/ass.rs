//! Advanced SubStation Alpha (`.ass`) and SubStation Alpha (`.ssa`).
//!
//! # Dialogue and signs
//!
//! An ASS file holds two kinds of event. DIALOGUE is what someone says, placed
//! by its style at the foot or the top; it becomes [`Cue`]s like any other
//! format's, keeping italics, bold, underline and colour, and the merge stacks
//! it with the other lanes in the lane's colour. A SIGN is typesetting: a
//! translation laid over a shop front with `\pos`, a title that moves with
//! `\move`, a drawing (`\p1`), a banner, anything in the middle row. libass
//! never moves those to avoid another line, and neither does the merge: a
//! [`Sign`] keeps its own style, its override tags and its place, scaled from
//! its script's resolution to the merged one.
//!
//! A lane that is ASS and alone never goes through here at all: the engine
//! loads the file itself, with every tag intact.

use crate::subtitles::cue::{self, Band, Cue, Rgb, Span};

/// A style as a script declares it, in that script's own units.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// Its name, which events refer to.
    pub name: String,
    /// The font family.
    pub font: String,
    /// The font size, in script pixels.
    pub size: f64,
    /// The fill colour, `AABBGGRR`.
    pub primary: u32,
    /// The karaoke colour, `AABBGGRR`.
    pub secondary: u32,
    /// The outline colour, `AABBGGRR`.
    pub outline_colour: u32,
    /// The shadow (or box) colour, `AABBGGRR`.
    pub back: u32,
    /// Heavy.
    pub bold: bool,
    /// Slanted.
    pub italic: bool,
    /// Underlined.
    pub underline: bool,
    /// Struck through.
    pub strike_out: bool,
    /// Horizontal stretch, percent.
    pub scale_x: f64,
    /// Vertical stretch, percent.
    pub scale_y: f64,
    /// Extra space between letters, script pixels.
    pub spacing: f64,
    /// Rotation, degrees.
    pub angle: f64,
    /// 1: outline and shadow; 3: an opaque box.
    pub border_style: i32,
    /// Outline width, script pixels.
    pub outline: f64,
    /// Shadow depth, script pixels.
    pub shadow: f64,
    /// Where on the picture, as the numeric keypad lays it out (1–9).
    pub alignment: u8,
    /// Left margin, script pixels.
    pub margin_l: f64,
    /// Right margin, script pixels.
    pub margin_r: f64,
    /// Vertical margin, script pixels.
    pub margin_v: f64,
    /// The font's charset number (1 is the default).
    pub encoding: i32,
}

impl Default for Style {
    /// What libass draws an event with whose style does not exist.
    fn default() -> Self {
        Self {
            name: "Default".to_owned(),
            font: "Arial".to_owned(),
            size: 18.0,
            primary: 0x00FF_FFFF,
            secondary: 0x00FF_0000,
            outline_colour: 0,
            back: 0,
            bold: false,
            italic: false,
            underline: false,
            strike_out: false,
            scale_x: 100.0,
            scale_y: 100.0,
            spacing: 0.0,
            angle: 0.0,
            border_style: 1,
            outline: 2.0,
            shadow: 2.0,
            alignment: 2,
            margin_l: 20.0,
            margin_r: 20.0,
            margin_v: 20.0,
            encoding: 1,
        }
    }
}

impl Style {
    /// The `Style:` line of this style under `name`, scaled by `sx` across
    /// and `sy` down.
    pub(crate) fn line(&self, name: &str, sx: f64, sy: f64) -> String {
        let flag = |b: bool| if b { "-1" } else { "0" };
        format!(
            "Style: {name},{font},{size},&H{p:08X},&H{s:08X},&H{o:08X},&H{b:08X},{bold},{italic},{underline},{strike},{scx},{scy},{spacing},{angle},{border},{outline},{shadow},{align},{ml},{mr},{mv},{enc}",
            font = self.font.replace(',', " "),
            size = number(self.size * sy),
            p = self.primary,
            s = self.secondary,
            o = self.outline_colour,
            b = self.back,
            bold = flag(self.bold),
            italic = flag(self.italic),
            underline = flag(self.underline),
            strike = flag(self.strike_out),
            scx = number(self.scale_x),
            scy = number(self.scale_y),
            spacing = number(self.spacing * sx),
            angle = number(self.angle),
            border = self.border_style,
            outline = number(self.outline * sy),
            shadow = number(self.shadow * sy),
            align = self.alignment,
            ml = number(self.margin_l * sx),
            mr = number(self.margin_r * sx),
            mv = number(self.margin_v * sy),
            enc = self.encoding,
        )
    }
}

/// An event that is typesetting, kept where its script put it.
#[derive(Debug, Clone, PartialEq)]
pub struct Sign {
    /// When it appears, in milliseconds on the file's clock.
    pub start_ms: u64,
    /// When it goes, in milliseconds on the file's clock.
    pub end_ms: u64,
    /// Its layer in its script.
    pub layer: i32,
    /// Its style, in its script's units.
    pub style: Style,
    /// The event's own margins (left, right, vertical); `0` is the style's.
    pub margins: [f64; 3],
    /// The event's effect field (`Banner;…`, `Scroll up;…`), or empty.
    pub effect: String,
    /// The text as the script wrote it, override tags and all.
    pub text: String,
    /// The script's resolution (`PlayResX`, `PlayResY`).
    pub play_res: (u32, u32),
    /// The script's wrapping (`WrapStyle`, 0–3).
    pub wrap_style: u8,
}

impl Sign {
    fn factors(&self, to: (u32, u32)) -> (f64, f64) {
        let (x, y) = self.play_res;
        (
            f64::from(to.0) / f64::from(x.max(1)),
            f64::from(to.1) / f64::from(y.max(1)),
        )
    }

    /// Its style's line, under `name`, in a script of resolution `to`.
    pub(crate) fn style_line(&self, name: &str, to: (u32, u32)) -> String {
        let (sx, sy) = self.factors(to);
        self.style.line(name, sx, sy)
    }

    /// Its margins, left, right and vertical, in a script of resolution `to`.
    pub(crate) fn margins_in(&self, to: (u32, u32)) -> [String; 3] {
        let (sx, sy) = self.factors(to);
        [
            number(self.margins[0] * sx),
            number(self.margins[1] * sx),
            number(self.margins[2] * sy),
        ]
    }

    /// Its effect field in a script of resolution `to`: a scroll's two edges
    /// are rows of the picture.
    pub(crate) fn effect_in(&self, to: (u32, u32)) -> String {
        let (_, sy) = self.factors(to);
        let mut fields: Vec<String> = self.effect.split(';').map(str::to_owned).collect();
        if fields
            .first()
            .is_some_and(|f| f.trim().to_ascii_lowercase().starts_with("scroll"))
        {
            for field in fields.iter_mut().skip(1).take(2) {
                if let Ok(v) = field.trim().parse::<f64>() {
                    *field = number(v * sy);
                }
            }
        }
        fields.join(";").replace(',', " ")
    }

    /// Its text in a script of resolution `to`, and wrapping its own way
    /// whatever the merged script's is (`\q`).
    pub(crate) fn text_in(&self, to: (u32, u32)) -> String {
        let (sx, sy) = self.factors(to);
        format!(
            "{{\\q{}}}{}",
            self.wrap_style,
            scale_text(&self.text, sx, sy)
        )
    }
}

/// A script, as read.
#[derive(Debug, Clone, Default)]
struct Script {
    play_res: (u32, u32),
    wrap_style: u8,
    styles: Vec<Style>,
    events: Vec<Event>,
}

#[derive(Debug, Clone)]
struct Event {
    layer: i32,
    start_ms: u64,
    end_ms: u64,
    style: String,
    margins: [f64; 3],
    effect: String,
    text: String,
}

/// The fields of a `[V4+ Styles]` style when the script has no `Format:`.
const STYLE_FIELDS_ASS: &str = "name,fontname,fontsize,primarycolour,secondarycolour,outlinecolour,backcolour,bold,italic,underline,strikeout,scalex,scaley,spacing,angle,borderstyle,outline,shadow,alignment,marginl,marginr,marginv,encoding";
/// The fields of a `[V4 Styles]` style (SSA) when the script has no
/// `Format:`.
const STYLE_FIELDS_SSA: &str = "name,fontname,fontsize,primarycolour,secondarycolour,tertiarycolour,backcolour,bold,italic,borderstyle,outline,shadow,alignment,marginl,marginr,marginv,alphalevel,encoding";
/// The fields of an event when the script has no `Format:`.
const EVENT_FIELDS: &str = "layer,start,end,style,name,marginl,marginr,marginv,effect,text";

fn parse(text: &str) -> Script {
    let mut script = Script::default();
    let (mut res_x, mut res_y) = (None::<u32>, None::<u32>);
    let mut section = String::new();
    let mut ssa = false;
    let mut style_fields: Vec<String> = Vec::new();
    let mut event_fields: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line.to_ascii_lowercase();
            if section == "[v4 styles]" {
                ssa = true;
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim_start();
        match section.as_str() {
            "[script info]" => match key.as_str() {
                "playresx" => res_x = value.trim().parse().ok().filter(|&v| v > 0),
                "playresy" => res_y = value.trim().parse().ok().filter(|&v| v > 0),
                "wrapstyle" => script.wrap_style = value.trim().parse().unwrap_or(0),
                "scripttype" => ssa = ssa || value.trim().eq_ignore_ascii_case("v4.00"),
                _ => {}
            },
            s if s.starts_with("[v4") && s.contains("styles") => match key.as_str() {
                "format" => style_fields = fields_of(value),
                "style" => {
                    if style_fields.is_empty() {
                        style_fields = fields_of(if ssa {
                            STYLE_FIELDS_SSA
                        } else {
                            STYLE_FIELDS_ASS
                        });
                    }
                    script.styles.push(style_of(&style_fields, value, ssa));
                }
                _ => {}
            },
            "[events]" => match key.as_str() {
                "format" => event_fields = fields_of(value),
                "dialogue" => {
                    if event_fields.is_empty() {
                        event_fields = fields_of(EVENT_FIELDS);
                    }
                    if let Some(event) = event_of(&event_fields, value) {
                        script.events.push(event);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    // libass's rules for a missing resolution.
    script.play_res = match (res_x, res_y) {
        (Some(x), Some(y)) => (x, y),
        (Some(1280), None) => (1280, 1024),
        (Some(x), None) => (x, x * 3 / 4),
        (None, Some(1024)) => (1280, 1024),
        (None, Some(y)) => (y * 4 / 3, y),
        (None, None) => (384, 288),
    };
    script
}

fn fields_of(format: &str) -> Vec<String> {
    format
        .split(',')
        .map(|f| f.trim().to_ascii_lowercase())
        .collect()
}

fn style_of(fields: &[String], value: &str, ssa: bool) -> Style {
    let values: Vec<&str> = value
        .splitn(fields.len().max(1), ',')
        .map(str::trim)
        .collect();
    let get = |name: &str| {
        fields
            .iter()
            .position(|f| f == name)
            .and_then(|i| values.get(i).copied())
    };
    let real = |name: &str, or: f64| get(name).and_then(|v| v.parse::<f64>().ok()).unwrap_or(or);
    let colour = |name: &str, or: u32| get(name).and_then(ass_colour).unwrap_or(or);
    let flag = |name: &str| {
        get(name)
            .and_then(|v| v.parse::<i64>().ok())
            .is_some_and(|v| v == -1 || v == 1 || v >= 600)
    };
    let default = Style::default();
    let alignment = get("alignment")
        .and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(2);
    Style {
        name: get("name")
            .unwrap_or("Default")
            .trim_start_matches('*')
            .to_owned(),
        font: get("fontname").unwrap_or("Arial").to_owned(),
        size: real("fontsize", default.size),
        primary: colour("primarycolour", default.primary),
        secondary: colour("secondarycolour", default.secondary),
        outline_colour: get("outlinecolour")
            .or_else(|| get("tertiarycolour"))
            .and_then(ass_colour)
            .unwrap_or(default.outline_colour),
        back: colour("backcolour", default.back),
        bold: flag("bold"),
        italic: flag("italic"),
        underline: flag("underline"),
        strike_out: flag("strikeout"),
        scale_x: real("scalex", 100.0),
        scale_y: real("scaley", 100.0),
        spacing: real("spacing", 0.0),
        angle: real("angle", 0.0),
        border_style: get("borderstyle").and_then(|v| v.parse().ok()).unwrap_or(1),
        outline: real("outline", default.outline),
        shadow: real("shadow", default.shadow),
        alignment: if ssa {
            numpad_of_legacy(alignment)
        } else {
            alignment.clamp(1, 9)
        },
        margin_l: real("marginl", default.margin_l),
        margin_r: real("marginr", default.margin_r),
        margin_v: real("marginv", default.margin_v),
        encoding: get("encoding").and_then(|v| v.parse().ok()).unwrap_or(1),
    }
}

/// An SSA alignment (1–3 bottom, 5–7 top, 9–11 middle) on the keypad.
fn numpad_of_legacy(legacy: u8) -> u8 {
    let column = legacy & 3;
    let column = if column == 0 { 2 } else { column };
    if legacy & 4 != 0 {
        column + 6
    } else if legacy & 8 != 0 {
        column + 3
    } else {
        column
    }
}

/// An ASS colour's full `AABBGGRR`.
fn ass_colour(text: &str) -> Option<u32> {
    let text = text.trim().trim_end_matches('&');
    if let Some(hex) = text.strip_prefix("&H").or_else(|| text.strip_prefix("&h")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        text.parse::<i64>()
            .ok()
            .and_then(|v| u32::try_from(v & 0xFFFF_FFFF).ok())
    }
}

fn event_of(fields: &[String], value: &str) -> Option<Event> {
    let values: Vec<&str> = value.splitn(fields.len().max(1), ',').collect();
    let get = |name: &str| {
        fields
            .iter()
            .position(|f| f == name)
            .and_then(|i| values.get(i).copied())
    };
    let margin = |name: &str| {
        get(name)
            .and_then(|v| v.trim().parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    Some(Event {
        layer: get("layer")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0),
        start_ms: cue::clock_ms(get("start")?)?,
        end_ms: cue::clock_ms(get("end")?)?,
        style: get("style")
            .unwrap_or("Default")
            .trim()
            .trim_start_matches('*')
            .to_owned(),
        margins: [margin("marginl"), margin("marginr"), margin("marginv")],
        effect: get("effect").unwrap_or("").trim().to_owned(),
        text: get("text").unwrap_or("").to_owned(),
    })
}

/// Reads an ASS or SSA script: its dialogue as cues, its typesetting as
/// signs.
#[must_use]
pub fn read(text: &str) -> (Vec<Cue>, Vec<Sign>) {
    let script = parse(text);
    let fallback = Style::default();
    let style_named = |name: &str| {
        script
            .styles
            .iter()
            .rev()
            .find(|s| s.name.eq_ignore_ascii_case(name))
            .or_else(|| {
                script
                    .styles
                    .iter()
                    .rev()
                    .find(|s| s.name.eq_ignore_ascii_case("Default"))
            })
            .unwrap_or(&fallback)
    };
    let mut cues = Vec::new();
    let mut signs = Vec::new();
    for event in &script.events {
        let style = style_named(&event.style);
        if is_sign(event, style) {
            signs.push(Sign {
                start_ms: event.start_ms,
                end_ms: event.end_ms,
                layer: event.layer,
                style: style.clone(),
                margins: event.margins,
                effect: event.effect.clone(),
                text: event.text.clone(),
                play_res: script.play_res,
                wrap_style: script.wrap_style,
            });
            continue;
        }
        let (lines, band) = dialogue(&event.text, style, script.wrap_style, &style_named);
        cues.push(Cue {
            start_ms: event.start_ms,
            end_ms: event.end_ms,
            lines,
            band,
        });
    }
    (cues, signs)
}

/// The override tags of every `{…}` block in `text`, each without its `\`.
fn tags_of(text: &str) -> Vec<&str> {
    let mut tags = Vec::new();
    let mut rest = text;
    while let Some((open, close)) = next_block(rest) {
        tags.extend(
            split_tags(&rest[open + 1..close])
                .into_iter()
                .filter_map(|t| t.strip_prefix('\\')),
        );
        rest = &rest[close + 1..];
    }
    tags
}

/// The next override block of `text`: the byte of its `{` and of its `}`.
/// A `\{` is a brace to draw, not a block, and a `{` never closed is text.
fn next_block(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let open = (0..bytes.len()).find(|&i| bytes[i] == b'{' && (i == 0 || bytes[i - 1] != b'\\'))?;
    let close = open + text[open..].find('}')?;
    Some((open, close))
}

/// A block's content cut into its tags, each starting with its `\`; a
/// parenthesised argument keeps its own `\` (`\t(\fs20)`). What comes before
/// the first `\` (a comment) is a piece of its own.
fn split_tags(block: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut depth = 0u32;
    let mut from = 0;
    for (at, c) in block.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '\\' if depth == 0 && at > from => {
                pieces.push(&block[from..at]);
                from = at;
            }
            _ => {}
        }
    }
    if from < block.len() {
        pieces.push(&block[from..]);
    }
    pieces
}

fn is_sign(event: &Event, style: &Style) -> bool {
    let effect = event.effect.to_ascii_lowercase();
    if effect.starts_with("banner") || effect.starts_with("scroll") {
        return true;
    }
    let mut alignment = style.alignment;
    let mut aligned = false;
    for tag in tags_of(&event.text) {
        if ["pos(", "move(", "org("].iter().any(|p| tag.starts_with(p)) {
            return true;
        }
        if let Some(n) = tag
            .strip_prefix('p')
            .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            && n.parse::<u32>().is_ok_and(|n| n > 0)
        {
            return true;
        }
        if !aligned {
            if let Some(n) = tag
                .strip_prefix("an")
                .and_then(|n| n.trim().parse::<u8>().ok())
            {
                alignment = n;
                aligned = true;
            } else if let Some(n) = tag
                .strip_prefix('a')
                .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|n| n.parse::<u8>().ok())
            {
                alignment = numpad_of_legacy(n);
                aligned = true;
            }
        }
    }
    (4..=6).contains(&alignment)
}

struct Look {
    italic: bool,
    bold: bool,
    underline: bool,
    colour: Option<Rgb>,
}

impl Look {
    fn of(style: &Style) -> Self {
        Self {
            italic: style.italic,
            bold: style.bold,
            underline: style.underline,
            colour: None,
        }
    }
}

fn dialogue<'s>(
    text: &str,
    style: &Style,
    wrap_style: u8,
    style_named: &dyn Fn(&str) -> &'s Style,
) -> (Vec<Vec<Span>>, Option<Band>) {
    let mut lines: Vec<Vec<Span>> = vec![Vec::new()];
    let mut look = Look::of(style);
    let mut band = cue::band_of_numpad(style.alignment);
    let mut aligned = false;
    let mut word = String::new();
    let flush = |word: &mut String, look: &Look, lines: &mut Vec<Vec<Span>>| {
        if word.is_empty() {
            return;
        }
        let span = Span {
            text: std::mem::take(word),
            italic: look.italic,
            bold: look.bold,
            underline: look.underline,
            colour: look.colour,
        };
        if let Some(line) = lines.last_mut() {
            line.push(span);
        }
    };
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c == '{'
            && let Some(close) = rest.find('}')
        {
            flush(&mut word, &look, &mut lines);
            for tag in split_tags(&rest[1..close])
                .into_iter()
                .filter_map(|t| t.strip_prefix('\\'))
            {
                apply_tag(tag, style, style_named, &mut look, &mut band, &mut aligned);
            }
            rest = &rest[close + 1..];
            continue;
        }
        if c == '\\' {
            let next = rest[1..].chars().next();
            let taken = match next {
                Some('N') => {
                    flush(&mut word, &look, &mut lines);
                    lines.push(Vec::new());
                    true
                }
                Some('n') => {
                    if wrap_style == 2 {
                        flush(&mut word, &look, &mut lines);
                        lines.push(Vec::new());
                    } else {
                        word.push(' ');
                    }
                    true
                }
                Some('h') => {
                    word.push('\u{a0}');
                    true
                }
                Some(b @ ('{' | '}')) => {
                    word.push(b);
                    true
                }
                _ => false,
            };
            if taken {
                rest = &rest[2..];
                continue;
            }
        }
        word.push(if c == '\t' { ' ' } else { c });
        rest = &rest[c.len_utf8()..];
    }
    flush(&mut word, &look, &mut lines);
    (lines, band)
}

fn apply_tag<'s>(
    tag: &str,
    style: &Style,
    style_named: &dyn Fn(&str) -> &'s Style,
    look: &mut Look,
    band: &mut Option<Band>,
    aligned: &mut bool,
) {
    let digits = |v: &str| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit() || b == b'-');
    let on = |v: &str, default: bool| {
        if v.is_empty() {
            default
        } else {
            v.parse::<i64>()
                .is_ok_and(|n| n == 1 || n == -1 || n >= 600)
        }
    };
    if let Some(v) = tag.strip_prefix("an") {
        if !*aligned && let Ok(n) = v.trim().parse::<u8>() {
            *band = cue::band_of_numpad(n);
            *aligned = true;
        }
    } else if let Some(v) = tag.strip_prefix('i').filter(|v| v.is_empty() || digits(v)) {
        look.italic = on(v, style.italic);
    } else if let Some(v) = tag.strip_prefix('b').filter(|v| v.is_empty() || digits(v)) {
        look.bold = on(v, style.bold);
    } else if let Some(v) = tag.strip_prefix('u').filter(|v| v.is_empty() || digits(v)) {
        look.underline = on(v, style.underline);
    } else if let Some(v) = tag
        .strip_prefix("1c")
        .or_else(|| tag.strip_prefix('c'))
        .filter(|v| v.is_empty() || v.starts_with('&'))
    {
        look.colour = Rgb::ass(v);
    } else if let Some(v) = tag.strip_prefix('a').filter(|v| digits(v)) {
        if !*aligned && let Ok(n) = v.parse::<u8>() {
            *band = cue::band_of_legacy(n);
            *aligned = true;
        }
    } else if let Some(v) = tag.strip_prefix('r') {
        let to = if v.trim().is_empty() {
            style
        } else {
            style_named(v.trim())
        };
        *look = Look::of(to);
    }
}

/// A number the way a script writes it: no decimals when whole, at most
/// three otherwise.
pub(crate) fn number(value: f64) -> String {
    if (value - value.round()).abs() < 1e-6 {
        return format!("{}", value.round() as i64);
    }
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A sign's text scaled by `sx` across and `sy` down: every position, clip,
/// size, border, shadow and drawing in it.
fn scale_text(text: &str, sx: f64, sy: f64) -> String {
    if (sx - 1.0).abs() < 1e-9 && (sy - 1.0).abs() < 1e-9 {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len() + 16);
    let mut drawing = false;
    let mut rest = text;
    while !rest.is_empty() {
        let block = next_block(rest);
        let plain = block.map_or(rest, |(o, _)| &rest[..o]);
        if drawing {
            out.push_str(&scale_drawing(plain, sx, sy));
        } else {
            out.push_str(plain);
        }
        let Some((o, c)) = block else {
            break;
        };
        out.push('{');
        out.push_str(&scale_block(&rest[o + 1..c], sx, sy, &mut drawing));
        out.push('}');
        rest = &rest[c + 1..];
    }
    out
}

fn scale_block(block: &str, sx: f64, sy: f64, drawing: &mut bool) -> String {
    split_tags(block)
        .into_iter()
        .map(|piece| match piece.strip_prefix('\\') {
            Some(tag) => format!("\\{}", scale_tag(tag, sx, sy, drawing)),
            None => piece.to_owned(),
        })
        .collect()
}

fn scale_tag(tag: &str, sx: f64, sy: f64, drawing: &mut bool) -> String {
    let arguments = |name: &str| -> Option<Vec<f64>> {
        let inner = tag
            .strip_prefix(name)?
            .strip_prefix('(')?
            .trim_end()
            .strip_suffix(')')?;
        inner
            .split(',')
            .map(|v| v.trim().parse::<f64>().ok())
            .collect()
    };
    let with = |name: &str, values: &[f64]| {
        let list: Vec<String> = values.iter().map(|v| number(*v)).collect();
        format!("{name}({})", list.join(","))
    };
    let single = |name: &str, factor: f64| -> Option<String> {
        let v = tag.strip_prefix(name)?;
        if v.starts_with(['+', '-']) {
            return None;
        }
        let n = v.trim().parse::<f64>().ok()?;
        Some(format!("{name}{}", number(n * factor)))
    };
    for name in ["pos", "org"] {
        if let Some(v) = arguments(name).filter(|v| v.len() == 2) {
            return with(name, &[v[0] * sx, v[1] * sy]);
        }
    }
    if let Some(v) = arguments("move").filter(|v| v.len() == 4 || v.len() == 6) {
        let mut scaled = vec![v[0] * sx, v[1] * sy, v[2] * sx, v[3] * sy];
        scaled.extend_from_slice(&v[4..]);
        return with("move", &scaled);
    }
    for name in ["iclip", "clip"] {
        if !tag.starts_with(name) {
            continue;
        }
        if let Some(v) = arguments(name).filter(|v| v.len() == 4) {
            return with(name, &[v[0] * sx, v[1] * sy, v[2] * sx, v[3] * sy]);
        }
        // A vector clip: an optional scale, then a drawing.
        if let Some(inner) = tag
            .strip_prefix(name)
            .and_then(|t| t.strip_prefix('('))
            .and_then(|t| t.trim_end().strip_suffix(')'))
        {
            let (scale, shape) = match inner.split_once(',') {
                Some((s, d)) if s.trim().parse::<f64>().is_ok() => (Some(s.trim()), d),
                _ => (None, inner),
            };
            let shape = scale_drawing(shape, sx, sy);
            return match scale {
                Some(s) => format!("{name}({s},{shape})"),
                None => format!("{name}({shape})"),
            };
        }
        return tag.to_owned();
    }
    if let Some(inner) = tag
        .strip_prefix("t(")
        .and_then(|t| t.trim_end().strip_suffix(')'))
    {
        let at = inner.find('\\').unwrap_or(inner.len());
        let mut nested = false;
        return format!(
            "t({}{})",
            &inner[..at],
            scale_block(&inner[at..], sx, sy, &mut nested)
        );
    }
    if tag.starts_with("fscx") || tag.starts_with("fscy") {
        return tag.to_owned();
    }
    let rules: [(&str, f64); 10] = [
        ("fsp", sx),
        ("fs", sy),
        ("xbord", sx),
        ("ybord", sy),
        ("bord", sy),
        ("xshad", sx),
        ("yshad", sy),
        ("shad", sy),
        ("blur", sy),
        ("pbo", sy),
    ];
    for (name, factor) in rules {
        if tag.starts_with(name) {
            return single(name, factor).unwrap_or_else(|| tag.to_owned());
        }
    }
    if let Some(n) = tag
        .strip_prefix('p')
        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    {
        *drawing = n.parse::<u32>().is_ok_and(|n| n > 0);
    }
    tag.to_owned()
}

/// A drawing's coordinates scaled: numbers alternate across and down; a
/// command letter starts a new pair.
fn scale_drawing(drawing: &str, sx: f64, sy: f64) -> String {
    let mut across = true;
    drawing
        .split_whitespace()
        .map(|word| match word.parse::<f64>() {
            Ok(v) => {
                let factor = if across { sx } else { sy };
                across = !across;
                number(v * factor)
            }
            Err(_) => {
                across = true;
                word.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = "[Script Info]\nScriptType: v4.00+\nPlayResX: 1280\nPlayResY: 720\n\n\
[V4+ Styles]\n\
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,20,20,30,1\n\
Style: Top,Arial,40,&H0000FFFF,&H000000FF,&H00000000,&H00000000,0,-1,0,0,100,100,0,0,1,2,0,8,20,20,30,1\n\
Style: Sign,Georgia,60,&H00336699,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,3,0,5,10,10,10,1\n\n\
[Events]\n\
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
Dialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello, {\\i1}world{\\i0}\\Nsecond\n\
Dialogue: 0,0:00:02.00,0:00:04.00,Top,,0,0,0,,Up here\n\
Dialogue: 1,0:00:02.00,0:00:04.00,Default,,0,0,0,,{\\pos(640,360)\\fs30}SHOP\n\
Dialogue: 0,0:00:05.00,0:00:06.00,Sign,,0,0,0,,Centred\n\
Comment: 0,0:00:05.00,0:00:06.00,Default,,0,0,0,,not shown\n\
Dialogue: 0,0:00:07.00,0:00:08.00,Default,,0,0,0,,{\\an8\\c&H0000FF&}red top \\{literal\\}\n";

    #[test]
    fn dialogue_becomes_cues_and_typesetting_signs() {
        let (cues, signs) = read(SCRIPT);
        assert_eq!(cues.len(), 3);
        assert_eq!(signs.len(), 2);
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 3500));
        assert_eq!(cues[0].text(), "Hello, world\nsecond");
        assert!(
            cues[0].lines[0]
                .iter()
                .any(|s| s.italic && s.text == "world")
        );
        assert_eq!(cues[0].band, None, "the foot follows the lane");
        assert_eq!(cues[1].band, Some(Band::Top));
        assert!(cues[1].lines[0][0].italic, "the style's italics");
        assert_eq!(cues[2].band, Some(Band::Top));
        assert_eq!(cues[2].lines[0][0].colour, Some(Rgb::new(255, 0, 0)));
        assert_eq!(cues[2].text(), "red top {literal}");
        assert_eq!(signs[0].play_res, (1280, 720));
        assert_eq!(signs[0].layer, 1);
        assert_eq!(signs[1].style.name, "Sign", "the middle row is a sign");
    }

    #[test]
    fn a_sign_scales_to_the_merged_resolution() {
        let (_, signs) = read(SCRIPT);
        let to = (1920, 1080);
        assert_eq!(signs[0].text_in(to), "{\\q0}{\\pos(960,540)\\fs45}SHOP");
        let text = "{\\move(0,0,100,50,0,500)\\clip(0,0,640,360)\\t(0,500,\\fs20\\bord2)\\p1}m 0 0 l 100 0 100 50{\\p0}";
        let sign = Sign {
            text: text.to_owned(),
            ..signs[0].clone()
        };
        assert_eq!(
            sign.text_in(to),
            "{\\q0}{\\move(0,0,150,75,0,500)\\clip(0,0,960,540)\\t(0,500,\\fs30\\bord3)\\p1}m 0 0 l 150 0 150 75{\\p0}"
        );
        assert!(
            signs[1]
                .style_line("L1S1", to)
                .starts_with("Style: L1S1,Georgia,90,&H00336699,")
        );
    }

    #[test]
    fn ssa_styles_use_the_old_alignment() {
        let ssa = "[Script Info]\nScriptType: v4.00\n\n[V4 Styles]\n\
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, TertiaryColour, BackColour, Bold, Italic, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, AlphaLevel, Encoding\n\
Style: Default,Arial,20,16777215,65535,65535,0,0,0,1,2,0,6,10,10,10,0,1\n\n\
[Events]\nFormat: Marked, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
Dialogue: Marked=0,0:00:01.00,0:00:02.00,Default,,0000,0000,0000,,Top of the screen\n";
        let (cues, signs) = read(ssa);
        assert!(signs.is_empty());
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].band, Some(Band::Top));
    }
}
