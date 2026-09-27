//! A page's head, read for its title, its manifest and its icons. Pure: it
//! takes the bytes and the page's final URL and returns what it found.
//!
//! # A tag scanner, not an HTML parser
//!
//! What is wanted is five kinds of tag in one well-known place: `<base>`,
//! `<title>`, `<meta name="application-name">`, `<link rel=manifest>` and the
//! icon links. A conforming parser is a large dependency to answer that, and
//! the pages it would be pointed at are exactly the ones that break parsers —
//! unquoted attributes, a `<link>` inside a `<script>` string, a head that
//! never closes. So this is FORGIVING by construction: it walks `<…>` spans,
//! skips comments and the raw text of `<script>`, `<style>` and `<template>`
//! (a `"<link rel=icon"` inside a script is not a link), stops at `</head>`,
//! at `<body>` or at [`HEAD_MOST`] bytes, and anything it cannot read it
//! passes over instead of failing on.
//!
//! # Every URL is resolved here
//!
//! Against `<base href>` when the page has one (itself resolved against the
//! page's URL), else against the page's final URL — the one AFTER redirects,
//! which is why the caller passes it. A manifest's icons resolve against the
//! MANIFEST's URL, not the page's (the Web App Manifest spec says so, and a
//! manifest served from a CDN is the case where the two differ). Only `http`
//! and `https` survive: a `data:` icon is not fetched, and a `javascript:`
//! one is not an icon.

use url::Url;

/// How much of a page is read looking for its head.
///
/// Half a mebibyte: a head with inlined styles and scripts can pass a hundred
/// kilobytes (measured on a large mail client), and past this the scanner is
/// reading a body that did not say `<body>`.
pub const HEAD_MOST: usize = 512 << 10;

/// One icon a page offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    /// The icon's absolute URL.
    pub url: String,
    /// Where it was declared.
    pub from: Declared,
    /// The largest side it claims (0 = unknown; `any` for SVG is `u32::MAX`).
    pub side: u32,
    /// Its declared type, when given.
    pub mime: Option<String>,
}

impl Offered {
    /// Whether it is a vector drawing, by its declared type or its name.
    #[must_use]
    pub fn is_svg(&self) -> bool {
        self.mime.as_deref().is_some_and(|m| m.contains("svg"))
            || Url::parse(&self.url).is_ok_and(|u| u.path().to_ascii_lowercase().ends_with(".svg"))
    }
}

/// Where a page declared an icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Declared {
    /// The web app manifest, purpose `any`.
    Manifest,
    /// `<link rel="apple-touch-icon">`, or the well-known path.
    AppleTouch,
    /// `<link rel="icon">`.
    Icon,
    /// `/favicon.ico`.
    Favicon,
    /// The manifest, purpose `maskable` only (padded; the last resort).
    Maskable,
}

impl Declared {
    /// The name the web gives the place, for a candidate's line of data
    /// (`manifest · 512 px · SVG`). Technical words, like a MIME type — not
    /// text a person reads as a sentence.
    #[must_use]
    pub const fn term(self) -> &'static str {
        match self {
            Self::Manifest => "manifest",
            Self::AppleTouch => "apple-touch-icon",
            Self::Icon => "icon",
            Self::Favicon => "favicon.ico",
            Self::Maskable => "maskable",
        }
    }
}

/// What a page's head says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Head {
    /// `<title>` or `application-name`.
    pub title: Option<String>,
    /// The manifest's absolute URL.
    pub manifest: Option<String>,
    /// The icons declared in the head (the manifest's come separately).
    pub icons: Vec<Offered>,
}

/// Read a page's head (up to `</head>` or the first 512 KiB).
///
/// `title` is the page's `application-name` when it declares one — the name
/// the site gives ITSELF, `WhatsApp` rather than the `(3) WhatsApp` a tab
/// shows — and its `<title>` otherwise, with whitespace collapsed and
/// entities decoded.
#[must_use]
pub fn head_of(page: &[u8], url: &str) -> Head {
    let Ok(page_url) = Url::parse(url) else {
        return Head::default();
    };
    let bytes = &page[..page.len().min(HEAD_MOST)];
    let text = String::from_utf8_lossy(bytes);
    let mut scan = Scan { text: &text, at: 0 };

    let mut base: Option<String> = None;
    let mut title: Option<String> = None;
    let mut app_name: Option<String> = None;
    let mut manifest: Option<String> = None;
    // In document order, resolved at the end: the document's base is its
    // FIRST `<base href>`, wherever it stands.
    let mut links: Vec<Link> = Vec::new();

    while let Some(tag) = scan.next_tag() {
        match tag {
            Tag::Close(name) if name == "head" => break,
            Tag::Open(name, _) if name == "body" => break,
            Tag::Open(name, attrs) => match name.as_str() {
                "script" | "style" | "template" | "textarea" => scan.skip_raw(&name),
                "title" => {
                    let raw = scan.raw_text("title");
                    if title.is_none() {
                        title = Some(collapsed(&decoded(raw))).filter(|t| !t.is_empty());
                    }
                }
                "base" => {
                    if base.is_none() {
                        base = attr(&attrs, "href").map(str::to_owned);
                    }
                }
                "meta" => {
                    if app_name.is_none()
                        && attr(&attrs, "name")
                            .is_some_and(|n| n.trim().eq_ignore_ascii_case("application-name"))
                    {
                        app_name = attr(&attrs, "content")
                            .map(collapsed)
                            .filter(|t| !t.is_empty());
                    }
                }
                "link" => {
                    let (Some(rel), Some(href)) = (attr(&attrs, "rel"), attr(&attrs, "href"))
                    else {
                        continue;
                    };
                    let rel: Vec<String> = rel
                        .split_ascii_whitespace()
                        .map(str::to_ascii_lowercase)
                        .collect();
                    if rel.iter().any(|r| r == "manifest") && manifest.is_none() {
                        manifest = Some(href.to_owned());
                    }
                    links.push(Link {
                        rel,
                        href: href.to_owned(),
                        sizes: attr(&attrs, "sizes").map(str::to_owned),
                        mime: attr(&attrs, "type").map(|t| t.trim().to_ascii_lowercase()),
                    });
                }
                _ => {}
            },
            Tag::Close(_) => {}
        }
    }

    let base_url = base
        .and_then(|b| page_url.join(b.trim()).ok())
        .filter(is_web)
        .unwrap_or(page_url);
    let resolve = |href: &str| {
        base_url
            .join(href.trim())
            .ok()
            .filter(is_web)
            .map(|u| u.to_string())
    };
    let icons = links
        .iter()
        .filter_map(
            |Link {
                 rel,
                 href,
                 sizes,
                 mime,
             }| {
                let from = if rel
                    .iter()
                    .any(|r| r == "apple-touch-icon" || r == "apple-touch-icon-precomposed")
                {
                    Declared::AppleTouch
                } else if rel.iter().any(|r| r == "icon") {
                    Declared::Icon
                } else {
                    // `mask-icon` is Safari's one-colour pinned-tab drawing: a
                    // silhouette, never a launcher's icon. Everything else is not
                    // an icon at all.
                    return None;
                };
                let url = resolve(href)?;
                let mut offered = Offered {
                    url,
                    from,
                    side: sizes.as_deref().map_or(0, side_of),
                    mime: mime.clone(),
                };
                if offered.side == 0 && offered.is_svg() {
                    offered.side = u32::MAX;
                }
                Some(offered)
            },
        )
        .collect();
    Head {
        title: app_name.or(title),
        manifest: manifest.as_deref().and_then(resolve),
        icons,
    }
}

/// The icons a manifest declares, resolved against the manifest's URL, and its
/// `name`.
///
/// The name is `short_name` when there is one, else `name`: the manifest
/// spec's `short_name` is the one meant for "where there is not enough space
/// for the full name", which is a launcher's row — `Spotify` against
/// `Spotify Web Player`.
///
/// `purpose` is a set of words: with `any` in it (or no `purpose` at all,
/// whose default is `any`) the icon is [`Declared::Manifest`]; `maskable`
/// without `any` is [`Declared::Maskable`] — drawn to be cropped into a
/// circle, so it looks padded uncropped and comes last; `monochrome` alone
/// is a silhouette and is left out.
#[must_use]
pub fn manifest_icons(manifest: &[u8], url: &str) -> (Option<String>, Vec<Offered>) {
    let Ok(base) = Url::parse(url) else {
        return (None, Vec::new());
    };
    let bytes = manifest.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(manifest);
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return (None, Vec::new());
    };
    let text = |key: &str| {
        json.get(key)
            .and_then(serde_json::Value::as_str)
            .map(collapsed)
            .filter(|s| !s.is_empty())
    };
    let name = text("short_name").or_else(|| text("name"));
    let icons = json
        .get("icons")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|icon| {
            let field = |k: &str| icon.get(k).and_then(serde_json::Value::as_str);
            let src = field("src")?;
            let url = base.join(src.trim()).ok().filter(is_web)?.to_string();
            let purposes: Vec<String> = field("purpose")
                .unwrap_or("any")
                .split_ascii_whitespace()
                .map(str::to_ascii_lowercase)
                .collect();
            let from = if purposes.is_empty() || purposes.iter().any(|p| p == "any") {
                Declared::Manifest
            } else if purposes.iter().any(|p| p == "maskable") {
                Declared::Maskable
            } else {
                return None;
            };
            let mime = field("type").map(|t| t.trim().to_ascii_lowercase());
            let mut offered = Offered {
                url,
                from,
                side: field("sizes").map_or(0, side_of),
                mime,
            };
            if offered.side == 0 && offered.is_svg() {
                offered.side = u32::MAX;
            }
            Some(offered)
        })
        .collect();
    (name, icons)
}

/// One `<link>` as the head wrote it, before it is resolved.
struct Link {
    /// Its `rel` words, lower case.
    rel: Vec<String>,
    /// Its `href`, as written.
    href: String,
    /// Its `sizes`, as written.
    sizes: Option<String>,
    /// Its `type`, lower case.
    mime: Option<String>,
}

/// The largest side a `sizes` value claims: `"192x192 512x512"` is 512,
/// `"any"` is `u32::MAX`, anything unreadable is 0. Pure.
#[must_use]
pub fn side_of(sizes: &str) -> u32 {
    sizes
        .split_ascii_whitespace()
        .filter_map(|s| {
            if s.eq_ignore_ascii_case("any") {
                return Some(u32::MAX);
            }
            let (w, h) = s.split_once(['x', 'X'])?;
            Some(w.parse::<u32>().ok()?.max(h.parse::<u32>().ok()?))
        })
        .max()
        .unwrap_or(0)
}

fn is_web(u: &Url) -> bool {
    matches!(u.scheme(), "http" | "https") && u.host().is_some()
}

/// One attribute's value, by name (already lower case).
fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

/// Whitespace runs as one space, trimmed.
fn collapsed(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The character references a head actually carries — the five named ones,
/// `&nbsp;`, and the numeric forms. An `&` that starts none of them stays.
fn decoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        // A reference is short; a `;` further on belongs to the text. Counted
        // in characters, so a multi-byte one is never cut in half.
        let Some(end) = rest
            .char_indices()
            .take(12)
            .find(|&(_, c)| c == ';')
            .map(|(i, _)| i)
        else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let named = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => name.strip_prefix('#').and_then(|n| {
                let code = match n.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => n.parse().ok(),
                };
                code.and_then(char::from_u32)
            }),
        };
        if let Some(c) = named {
            out.push(c);
            rest = &rest[end + 1..];
        } else {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

/// A tag the scanner met.
enum Tag {
    /// `<name attr=…>`, attribute names lower case, values decoded.
    Open(String, Vec<(String, String)>),
    /// `</name>`.
    Close(String),
}

/// The scanner's position in the text.
struct Scan<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Scan<'a> {
    /// The next tag, skipping comments, doctypes and processing instructions;
    /// `None` at the end.
    fn next_tag(&mut self) -> Option<Tag> {
        loop {
            let open = self.text[self.at..].find('<')? + self.at;
            let rest = &self.text[open..];
            if rest.starts_with("<!--") {
                self.at = rest.find("-->").map_or(self.text.len(), |e| open + e + 3);
                continue;
            }
            if rest.starts_with("<!") || rest.starts_with("<?") {
                self.at = rest.find('>').map_or(self.text.len(), |e| open + e + 1);
                continue;
            }
            let (closing, after) = match rest[1..].strip_prefix('/') {
                Some(r) => (true, r),
                None => (false, &rest[1..]),
            };
            let name_len = after
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == ':'))
                .unwrap_or(after.len());
            if name_len == 0 {
                // `a < b` in text: not a tag.
                self.at = open + 1;
                continue;
            }
            let name = after[..name_len].to_ascii_lowercase();
            let start = open + 1 + usize::from(closing) + name_len;
            let (attrs, end) = attributes(self.text, start);
            self.at = end;
            return Some(if closing {
                Tag::Close(name)
            } else {
                Tag::Open(name, attrs)
            });
        }
    }

    /// Past the raw text of `<name>` and its closing tag.
    fn skip_raw(&mut self, name: &str) {
        let _ = self.raw_text(name);
    }

    /// The raw text up to `</name`, and past its closing tag.
    fn raw_text(&mut self, name: &str) -> &'a str {
        let text: &'a str = self.text;
        let rest = &text[self.at..];
        let Some(end) = closing_at(rest, name) else {
            self.at = text.len();
            return rest;
        };
        let raw = &rest[..end];
        let close = rest[end..].find('>').map_or(rest.len(), |e| end + e + 1);
        self.at += close;
        raw
    }
}

/// Where `</name` starts in `text`, the name compared without case and
/// without copying the text (a head with fifty inline scripts would otherwise
/// be lowered fifty times).
fn closing_at(text: &str, name: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(at) = text[from..].find("</").map(|i| from + i) {
        let after = &bytes[at + 2..];
        if after.len() >= name.len() && after[..name.len()].eq_ignore_ascii_case(name.as_bytes()) {
            return Some(at);
        }
        from = at + 2;
    }
    None
}

/// The attributes from `from` to the tag's `>`, and the position after it.
/// Quoted values may hold a `>`; unquoted ones end at whitespace or `>`.
fn attributes(text: &str, from: usize) -> (Vec<(String, String)>, usize) {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = from;
    loop {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        if i >= bytes.len() {
            return (out, bytes.len());
        }
        if bytes[i] == b'>' {
            return (out, i + 1);
        }
        let name_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && !matches!(bytes[i], b'=' | b'>' | b'/')
        {
            i += 1;
        }
        let name = text[name_start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                let start = i + 1;
                let end = bytes[start..]
                    .iter()
                    .position(|&b| b == quote)
                    .map_or(bytes.len(), |p| start + p);
                value = decoded(&text[start..end]);
                i = (end + 1).min(bytes.len());
            } else {
                let start = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                    i += 1;
                }
                value = decoded(&text[start..i]);
            }
        }
        if !name.is_empty() {
            out.push((name, value));
        } else if i < bytes.len() {
            // A stray `=` with no name: step over it, never stall.
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/web")
            .join(name);
        std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    #[test]
    fn a_plain_head_gives_its_title_manifest_and_icons() {
        let head = head_of(
            &fixture("plain-head.html"),
            "https://mail.example.org/inbox/",
        );
        assert_eq!(head.title.as_deref(), Some("Example Mail"));
        assert_eq!(
            head.manifest.as_deref(),
            Some("https://mail.example.org/manifest.webmanifest")
        );
        let urls: Vec<(&str, Declared, u32)> = head
            .icons
            .iter()
            .map(|o| (o.url.as_str(), o.from, o.side))
            .collect();
        assert_eq!(
            urls,
            [
                (
                    "https://mail.example.org/favicon-32.png",
                    Declared::Icon,
                    32
                ),
                (
                    "https://mail.example.org/favicon.svg",
                    Declared::Icon,
                    u32::MAX
                ),
                (
                    "https://cdn.example.net/touch-180.png",
                    Declared::AppleTouch,
                    180
                ),
                (
                    "https://mail.example.org/icons/precomposed.png",
                    Declared::AppleTouch,
                    0
                ),
                (
                    "https://mail.example.org/inbox/legacy.ico",
                    Declared::Icon,
                    48
                ),
            ]
        );
    }

    #[test]
    fn a_messy_head_is_read_all_the_same() {
        // Unquoted and single-quoted values, upper-case tags, entities in an
        // href, a `<link>` inside a script and inside a comment, a mask-icon,
        // a data: icon, a <base>, and a head that never closes.
        let head = head_of(
            &fixture("messy-head.html"),
            "http://app.example.org/deep/page?x=1",
        );
        assert_eq!(
            head.title.as_deref(),
            Some("Tom & Jerry's app"),
            "application-name wins, entities decoded"
        );
        assert_eq!(
            head.manifest.as_deref(),
            Some("https://static.example.org/assets/app.json")
        );
        let urls: Vec<&str> = head.icons.iter().map(|o| o.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://static.example.org/assets/i.png?v=2&size=big",
                "https://static.example.org/assets/apple.png",
                "https://static.example.org/assets/shortcut.ico",
            ]
        );
        assert_eq!(head.icons[0].side, 512);
        assert_eq!(head.icons[0].mime.as_deref(), Some("image/png"));
    }

    #[test]
    fn the_body_ends_the_head() {
        let page = b"<html><title>A</title><body><link rel=icon href=/late.png></body>";
        assert!(head_of(page, "https://a.example/").icons.is_empty());
        let page = b"<head></head><link rel=icon href=/late.png>";
        assert!(head_of(page, "https://a.example/").icons.is_empty());
    }

    #[test]
    fn only_so_much_is_read() {
        let mut page = b"<head><title>A</title>".to_vec();
        page.extend(std::iter::repeat_n(b' ', HEAD_MOST));
        page.extend_from_slice(b"<link rel=icon href=/far.png>");
        let head = head_of(&page, "https://a.example/");
        assert_eq!(head.title.as_deref(), Some("A"));
        assert!(head.icons.is_empty(), "a link past the ceiling is not read");
    }

    #[test]
    fn nothing_that_is_not_a_page_breaks_the_scanner() {
        for page in [
            &b""[..],
            b"<",
            b"<<<>>>",
            b"<link",
            b"<link rel='icon",
            b"<title>never closed",
            b"\xff\xfe<",
        ] {
            let _ = head_of(page, "https://a.example/");
        }
        assert_eq!(head_of(b"<title>x</title>", "not a url"), Head::default());
    }

    #[test]
    fn a_real_head_is_read() {
        // The public head of https://mail.proton.me/ as served on 2026-09-26,
        // cut at `</head>`.
        let head = head_of(
            &fixture("proton-mail-head.html"),
            "https://account.proton.me/mail",
        );
        assert!(head.title.is_some());
        assert!(!head.icons.is_empty(), "{head:?}");
        assert!(
            head.icons.iter().all(|o| o.url.starts_with("https://")),
            "{head:?}"
        );
    }

    #[test]
    fn sizes_say_the_largest_side() {
        assert_eq!(side_of("192x192 512x512"), 512);
        assert_eq!(side_of("16X16"), 16);
        assert_eq!(side_of("120x60"), 120);
        assert_eq!(side_of("any"), u32::MAX);
        assert_eq!(side_of("48x48 any"), u32::MAX);
        assert_eq!(side_of("big"), 0);
        assert_eq!(side_of(""), 0);
    }

    #[test]
    fn a_manifest_gives_its_short_name_and_sorts_its_icons_by_purpose() {
        let (name, icons) = manifest_icons(
            &fixture("example-manifest.json"),
            "https://cdn.example.net/app/manifest.json",
        );
        assert_eq!(name.as_deref(), Some("Example"));
        let got: Vec<(&str, Declared, u32)> = icons
            .iter()
            .map(|o| (o.url.as_str(), o.from, o.side))
            .collect();
        assert_eq!(
            got,
            [
                // Relative to the MANIFEST, not the page.
                (
                    "https://cdn.example.net/app/icons/192.png",
                    Declared::Manifest,
                    192
                ),
                (
                    "https://cdn.example.net/app/icons/512.png",
                    Declared::Manifest,
                    512
                ),
                (
                    "https://cdn.example.net/logo.svg",
                    Declared::Manifest,
                    u32::MAX
                ),
                (
                    "https://cdn.example.net/app/icons/maskable.png",
                    Declared::Maskable,
                    512
                ),
                (
                    "https://cdn.example.net/app/icons/both.png",
                    Declared::Manifest,
                    256
                ),
            ]
        );
    }

    #[test]
    fn a_manifest_without_a_short_name_gives_its_name_and_a_broken_one_nothing() {
        let (name, icons) =
            manifest_icons(br#"{"name":"  Long   Name "}"#, "https://a.example/m.json");
        assert_eq!(name.as_deref(), Some("Long Name"));
        assert!(icons.is_empty());
        assert_eq!(
            manifest_icons(b"{not json", "https://a.example/m.json"),
            (None, Vec::new())
        );
        // A BOM is not a syntax error.
        let (name, _) = manifest_icons(
            b"\xEF\xBB\xBF{\"short_name\":\"B\"}",
            "https://a.example/m.json",
        );
        assert_eq!(name.as_deref(), Some("B"));
    }

    #[test]
    fn entities_are_decoded_and_a_lone_ampersand_stays() {
        assert_eq!(
            decoded("a &amp; b &lt;c&gt; &#39;d&#x27; &nbsp;e"),
            "a & b <c> 'd'  e"
        );
        // A multi-byte character right after an `&` is never cut in half.
        assert_eq!(decoded("&çãõ€€€€€€€€€€;"), "&çãõ€€€€€€€€€€;");
        assert_eq!(decoded("R&D &unknown; &"), "R&D &unknown; &");
    }
}
