//! What a copy puts on the clipboard, per collection and format.
//!
//! The formats are the three buffers' own (`glyphs-copy-*`, `unicode-copy-*`,
//! `lucide-copy-*`), now one list each here; a format's words are the door's
//! (`icons-copy-<collection>-<id>`).
//!
//! Two promises the tests hold: the copied SVG is byte for byte the one the
//! page previews (both are [`crate::lucide::svg`]), and a combining mark
//! copies ONE character — never the dotted circle it is drawn on, which is the
//! one defect that survives every look at the screen.

use crate::lucide::{self, Adjust};
use crate::{Char, Lucide, Nerd};

/// A Nerd Fonts glyph's copy formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NerdFormat {
    /// Its name (`md-home`).
    Name,
    /// The glyph itself.
    Glyph,
    /// The CSS class (`nf nf-md-home`).
    Css,
    /// `U+F02DC`.
    Hex,
    /// UTF-16 code units.
    Utf16,
    /// `\u{f02dc}`.
    Escape,
    /// `&#xF02DC;`.
    Html,
    /// A Rust `char` literal.
    Rust,
    /// The design-system token (`icon.md-home`).
    Token,
}

impl NerdFormat {
    /// Every format, in the sheet's order.
    pub const ALL: [Self; 9] = [
        Self::Name,
        Self::Glyph,
        Self::Css,
        Self::Hex,
        Self::Utf16,
        Self::Escape,
        Self::Html,
        Self::Rust,
        Self::Token,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Glyph => "glyph",
            Self::Css => "css",
            Self::Hex => "hex",
            Self::Utf16 => "utf16",
            Self::Escape => "escape",
            Self::Html => "html",
            Self::Rust => "rust",
            Self::Token => "token",
        }
    }
}

/// A Unicode character's copy formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnicodeFormat {
    /// The character (or sequence) itself.
    Character,
    /// Its official name.
    Name,
    /// `U+2605`, per codepoint.
    Hex,
    /// Decimal codepoints.
    Decimal,
    /// `\u{2605}`, per codepoint.
    Escape,
    /// `&#x2605;`, per codepoint.
    Html,
    /// A Rust literal.
    Rust,
    /// UTF-16 code units.
    Utf16,
    /// UTF-8 bytes.
    Utf8,
}

impl UnicodeFormat {
    /// Every format, in the sheet's order.
    pub const ALL: [Self; 9] = [
        Self::Character,
        Self::Name,
        Self::Hex,
        Self::Decimal,
        Self::Escape,
        Self::Html,
        Self::Rust,
        Self::Utf16,
        Self::Utf8,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::Name => "name",
            Self::Hex => "hex",
            Self::Decimal => "decimal",
            Self::Escape => "escape",
            Self::Html => "html",
            Self::Rust => "rust",
            Self::Utf16 => "utf16",
            Self::Utf8 => "utf8",
        }
    }
}

/// A Lucide figure's copy formats: lucide.dev's tabs first, then ours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LucideFormat {
    /// The SVG, with the adjustments.
    Svg,
    /// The SVG as JSX.
    Jsx,
    /// The vanilla JS import.
    Vanilla,
    /// The React component.
    React,
    /// The Vue component.
    Vue,
    /// The Svelte component.
    Svelte,
    /// The Preact component.
    Preact,
    /// The Solid component.
    Solid,
    /// The Angular component.
    Angular,
    /// The font's CSS class.
    Font,
    /// Its name.
    Name,
    /// Its component name (`House`).
    Component,
    /// Lucide's published character.
    Character,
    /// Lucide's published codepoint.
    Hex,
    /// The raw path data.
    Path,
    /// A data URI.
    Uri,
    /// The SVG in base64.
    Base64,
    /// A CSS rule with the SVG as a mask.
    Css,
    /// A `<symbol>` for a sprite.
    Sprite,
}

impl LucideFormat {
    /// Every format, in the sheet's order.
    pub const ALL: [Self; 19] = [
        Self::Svg,
        Self::Jsx,
        Self::Vanilla,
        Self::React,
        Self::Vue,
        Self::Svelte,
        Self::Preact,
        Self::Solid,
        Self::Angular,
        Self::Font,
        Self::Name,
        Self::Component,
        Self::Character,
        Self::Hex,
        Self::Path,
        Self::Uri,
        Self::Base64,
        Self::Css,
        Self::Sprite,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Jsx => "jsx",
            Self::Vanilla => "vanilla",
            Self::React => "react",
            Self::Vue => "vue",
            Self::Svelte => "svelte",
            Self::Preact => "preact",
            Self::Solid => "solid",
            Self::Angular => "angular",
            Self::Font => "font",
            Self::Name => "name",
            Self::Component => "component",
            Self::Character => "character",
            Self::Hex => "hex",
            Self::Path => "path",
            Self::Uri => "uri",
            Self::Base64 => "base64",
            Self::Css => "css",
            Self::Sprite => "sprite",
        }
    }
}

/// A glyph in a format.
#[must_use]
pub fn nerd(glyph: &Nerd, format: NerdFormat) -> String {
    let cp = glyph.cp;
    let name = glyph.name.as_deref().unwrap_or_default();
    let Some(c) = char::from_u32(cp) else {
        return String::new();
    };
    match format {
        NerdFormat::Name => name.to_owned(),
        NerdFormat::Glyph => c.to_string(),
        // The CSS class carries `nf-`, which the NAME does not (AGENTS.md).
        NerdFormat::Css => format!("nf nf-{name}"),
        NerdFormat::Hex => format!("U+{cp:04X}"),
        NerdFormat::Utf16 => utf16(c),
        NerdFormat::Escape => format!("\\u{{{cp:x}}}"),
        NerdFormat::Html => format!("&#x{cp:X};"),
        NerdFormat::Rust => format!("'\\u{{{cp:x}}}'"),
        // The design system's token when one uses the glyph, and the name
        // under `icon.` otherwise — what a token for it WOULD be called.
        NerdFormat::Token => glyph
            .token
            .as_ref()
            .map_or_else(|| format!("icon.{name}"), |t| format!("icon.{t}")),
    }
}

/// `\uXXXX` per UTF-16 code unit — a surrogate pair for a supplementary
/// codepoint.
fn utf16(c: char) -> String {
    let mut units = [0u16; 2];
    c.encode_utf16(&mut units)
        .iter()
        .map(|u| format!("\\u{u:04X}"))
        .collect()
}

/// A character in a format. EVERY format walks the whole sequence: `U+1F469`
/// alone would be the hexadecimal of a woman, not of 👩‍💻.
#[must_use]
pub fn unicode(character: &Char, format: UnicodeFormat) -> String {
    let each = |f: &dyn Fn(u32) -> String, joiner: &str| {
        character
            .seq
            .iter()
            .map(|c| f(*c))
            .collect::<Vec<_>>()
            .join(joiner)
    };
    match format {
        // THE TEXT, never the drawing: see `Char::shows`.
        UnicodeFormat::Character => character.text(),
        UnicodeFormat::Name => character.name.clone(),
        UnicodeFormat::Hex => each(&|c| format!("U+{c:04X}"), " "),
        UnicodeFormat::Decimal => each(&|c| c.to_string(), " "),
        UnicodeFormat::Escape => each(&|c| format!("\\u{{{c:x}}}"), " "),
        UnicodeFormat::Html => each(&|c| format!("&#x{c:X};"), ""),
        UnicodeFormat::Rust if character.seq.len() == 1 => {
            format!("'\\u{{{:x}}}'", character.cp)
        }
        // A sequence does not fit in a `char`: it is a `&str`.
        UnicodeFormat::Rust => format!("\"{}\"", each(&|c| format!("\\u{{{c:x}}}"), "")),
        UnicodeFormat::Utf16 => character.text().chars().map(utf16).collect(),
        // THE BYTES, as `xxd` shows them: what answers "why did my file grow
        // two bytes".
        UnicodeFormat::Utf8 => character
            .text()
            .bytes()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// A figure, adjusted, in a format.
#[must_use]
pub fn lucide(figure: &Lucide, adjust: &Adjust, format: LucideFormat) -> String {
    let component = lucide::component(figure);
    let name = &figure.name;
    let import = |package: &str| {
        format!(
            "import {{ {component} }} from '{package}';\n\n<{component} {} />",
            properties(adjust)
        )
    };
    let size = adjust.size();
    match format {
        LucideFormat::Svg => lucide::svg(figure, adjust, false),
        LucideFormat::Jsx => lucide::svg(figure, adjust, true),
        LucideFormat::Vanilla => format!(r#"<i data-lucide="{name}"></i>"#),
        LucideFormat::React => import("lucide-react"),
        LucideFormat::Vue => import("lucide-vue-next"),
        LucideFormat::Svelte => import("@lucide/svelte"),
        LucideFormat::Preact => import("lucide-preact"),
        LucideFormat::Solid => import("lucide-solid"),
        LucideFormat::Angular => format!(
            r#"<lucide-angular name="{name}" [size]="{size}" [strokeWidth]="{}" color="{}"></lucide-angular>"#,
            adjust.stroke(),
            adjust.colour(),
        ),
        LucideFormat::Font => format!(r#"<i class="lucide lucide-{name}"></i>"#),
        LucideFormat::Name => name.clone(),
        LucideFormat::Component => component,
        // LUCIDE'S character, not the one on the screen: the face here was
        // moved to plane 15 because Lucide's own range collides with 634 Nerd
        // Fonts glyphs, and whoever pastes this pastes into a page with
        // Lucide's font.
        LucideFormat::Character => char::from_u32(figure.lcp)
            .map(String::from)
            .unwrap_or_default(),
        LucideFormat::Hex => format!("U+{:04X}", figure.lcp),
        LucideFormat::Path => lucide::nodes(figure)
            .into_iter()
            .filter(|(element, _)| *element == "path")
            .filter_map(|(_, attributes)| {
                attributes
                    .into_iter()
                    .find(|(k, _)| *k == "d")
                    .map(|(_, v)| v.to_owned())
            })
            .collect::<Vec<_>>()
            .join(" "),
        LucideFormat::Uri => data_uri(&lucide::svg_one_line(figure, adjust)),
        LucideFormat::Base64 => format!(
            "data:image/svg+xml;base64,{}",
            base64(lucide::svg_one_line(figure, adjust).as_bytes())
        ),
        LucideFormat::Css => format!(
            ".icon-{name} {{\n  background-image: url(\"{}\");\n  background-size: {size}px {size}px;\n}}",
            data_uri(&lucide::svg_one_line(figure, adjust)),
        ),
        LucideFormat::Sprite => format!(
            "<svg class=\"lucide\" width=\"{size}\" height=\"{size}\">\n  <use href=\"lucide.svg#{name}\" />\n</svg>"
        ),
    }
}

/// The properties the framework components take.
fn properties(adjust: &Adjust) -> String {
    format!(
        r#"color="{}" size={{{}}} strokeWidth={{{}}} absoluteStrokeWidth={{{}}}"#,
        adjust.colour(),
        adjust.size(),
        adjust.stroke(),
        adjust.absolute,
    )
}

/// A `data:` URI with only what would break it inside `url(…)` escaped — a
/// readable URI is more useful than a fully escaped one, and browsers take the
/// rest raw. The double quotes become single ones FIRST, so the `url("…")`
/// around it still closes where it should.
fn data_uri(svg: &str) -> String {
    let mut out = String::from("data:image/svg+xml,");
    for c in svg.replace('"', "'").chars() {
        match c {
            '<' | '>' | '#' | '%' | '{' | '}' | '|' | '\\' | '^' | '~' | '[' | ']' | '`' => {
                out.push_str(&format!("%{:02X}", u32::from(c)));
            }
            other => out.push(other),
        }
    }
    out
}

/// Standard base64 (RFC 4648, padded).
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 0x3F) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_is_written_in_every_format() {
        let home = Nerd {
            cp: 0xF02DC,
            name: Some("md-home".to_owned()),
            ..Nerd::default()
        };
        let all: Vec<String> = NerdFormat::ALL.iter().map(|f| nerd(&home, *f)).collect();
        assert_eq!(
            all,
            [
                "md-home",
                "\u{F02DC}",
                "nf nf-md-home",
                "U+F02DC",
                "\\uDB80\\uDEDC",
                "\\u{f02dc}",
                "&#xF02DC;",
                "'\\u{f02dc}'",
                "icon.md-home",
            ]
        );
    }

    #[test]
    fn a_sequence_is_copied_whole_and_a_mark_alone() {
        let coder = Char {
            cp: 0x1F469,
            seq: vec![0x1F469, 0x200D, 0x1F4BB],
            shows: vec![0x1F469, 0x200D, 0x1F4BB],
            name: "woman technologist".to_owned(),
            ..Char::default()
        };
        assert_eq!(unicode(&coder, UnicodeFormat::Character).chars().count(), 3);
        assert_eq!(
            unicode(&coder, UnicodeFormat::Hex),
            "U+1F469 U+200D U+1F4BB"
        );
        assert_eq!(
            unicode(&coder, UnicodeFormat::Rust),
            "\"\\u{1f469}\\u{200d}\\u{1f4bb}\""
        );
        assert_eq!(
            unicode(&coder, UnicodeFormat::Utf8),
            "F0 9F 91 A9 E2 80 8D F0 9F 92 BB"
        );
        let acute = Char {
            cp: 0x301,
            seq: vec![0x301],
            shows: vec![0x25CC, 0x301],
            ..Char::default()
        };
        assert_eq!(unicode(&acute, UnicodeFormat::Character), "\u{301}");
        assert_eq!(acute.drawn(), "\u{25CC}\u{301}");
        assert_eq!(unicode(&acute, UnicodeFormat::Rust), "'\\u{301}'");
    }

    #[test]
    fn base64_is_the_standard_one() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"<svg />"), "PHN2ZyAvPg==");
        assert_eq!(
            data_uri(r##"<a b="#1">"##),
            "data:image/svg+xml,%3Ca b='%231'%3E"
        );
    }
}
