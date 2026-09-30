//! TTML (`.ttml`, `.dfxp`, `.xml`): the W3C's timed text, in its three
//! dialects — TTML 1 and 2, the older DFXP namespace, and the IMSC text
//! profile broadcasters and streaming services deliver.
//!
//! ffmpeg 8.1.2 only WRITES it, so this is ours. What is kept:
//!
//! - every time expression: clock times (`00:00:01.500`, and with frames
//!   `00:00:01:12` or sub-frames `00:00:01:12.1`) and offsets in hours,
//!   minutes, seconds, milliseconds, frames or ticks (`1.5s`, `90f`,
//!   `10000000t`), at the document's `ttp:frameRate`,
//!   `ttp:frameRateMultiplier`, `ttp:subFrameRate` and `ttp:tickRate`;
//! - nested timing: a `div` or `p` begins relative to its parent, and a
//!   `span` with its own time becomes a cue of its own (roll-up captions);
//! - styles by reference and inline: italics, bold, underline, colour;
//! - `<br/>` and the collapsing of whitespace;
//! - the band: a region whose text sits in the upper half is the top.

use std::collections::HashMap;

use ronoctua_xml::roxmltree::{self, Node, ParsingOptions};

use crate::subtitles::Damaged;
use crate::subtitles::cue::{self, Band, Cue, Rgb, Span};

/// How many nodes a document may have: far past any film's subtitles, far
/// below what would exhaust the daemon.
const NODES_MOST: u32 = 2_000_000;
/// How deep elements are followed: past any real document, short of what
/// would run the reading thread out of stack.
const DEPTH_MOST: u32 = 64;
/// How deep a chain of style references is followed.
const STYLE_DEPTH_MOST: u32 = 8;

/// The clock of a document: frames, sub-frames and ticks per second.
#[derive(Debug, Clone, Copy)]
struct Clock {
    frames_per_second: f64,
    sub_frames: f64,
    ticks_per_second: f64,
}

/// Reads a TTML document's cues.
///
/// # Errors
///
/// [`Damaged`] when the text is not well-formed XML.
pub fn read(text: &str) -> Result<Vec<Cue>, Damaged> {
    let options = ParsingOptions {
        allow_dtd: true,
        nodes_limit: NODES_MOST,
        ..ParsingOptions::default()
    };
    // How deep a document may nest at all is measured BEFORE it is parsed
    // (`ronoctua_xml::DEPTH_MOST`): the reader recurses once per level, and a
    // crafted file would take the daemon down with it.
    let document = ronoctua_xml::parse(text, options).map_err(|_| Damaged)?;
    let root = document.root_element();
    let clock = clock_of(root);
    let styles = styles_of(root);
    let geometry = Geometry::of(root);
    let regions = regions_of(root, &styles, &geometry);
    let reader = Reader {
        clock,
        styles,
        regions,
    };
    let mut cues = Vec::new();
    if let Some(body) = root.children().find(|n| n.has_tag_name_local("body")) {
        let top = Scope {
            begin: 0.0,
            end: None,
            look: Look::default(),
            band: None,
            depth: 0,
        };
        reader.walk(body, reader.inside(body, top), &mut cues);
    }
    // A cue with no end anywhere lasts until the next one.
    cues.sort_by_key(|c| c.0.start_ms);
    let starts: Vec<u64> = cues.iter().map(|c| c.0.start_ms).collect();
    Ok(cues
        .into_iter()
        .enumerate()
        .map(|(n, (mut c, open))| {
            if open {
                c.end_ms = cue::untimed_end(
                    c.start_ms,
                    starts[n + 1..].iter().copied().find(|&s| s > c.start_ms),
                );
            }
            c
        })
        .collect())
}

trait LocalName {
    fn has_tag_name_local(&self, name: &str) -> bool;
}

impl LocalName for Node<'_, '_> {
    fn has_tag_name_local(&self, name: &str) -> bool {
        self.is_element() && self.tag_name().name() == name
    }
}

/// A `ttp:` parameter of the root.
fn parameter<'a>(root: Node<'a, '_>, name: &str) -> Option<&'a str> {
    root.attributes()
        .find(|a| a.name() == name && a.namespace().is_some_and(|n| n.contains("parameter")))
        .map(|a| a.value())
}

fn clock_of(root: Node) -> Clock {
    let frame_rate = parameter(root, "frameRate").and_then(|v| v.trim().parse::<f64>().ok());
    let multiplier = parameter(root, "frameRateMultiplier")
        .and_then(|v| {
            let mut parts = v.split_whitespace().map(|p| p.parse::<f64>().ok());
            Some(parts.next()?? / parts.next()??)
        })
        .filter(|m| *m > 0.0)
        .unwrap_or(1.0);
    let frames_per_second = frame_rate.unwrap_or(30.0).max(1.0) * multiplier;
    let sub_frames = parameter(root, "subFrameRate")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(1.0);
    let ticks_per_second = parameter(root, "tickRate")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(if frame_rate.is_some() {
            frames_per_second * sub_frames
        } else {
            1.0
        });
    Clock {
        frames_per_second,
        sub_frames,
        ticks_per_second,
    }
}

/// A time expression, in milliseconds.
fn time_ms(expression: &str, clock: Clock) -> Option<f64> {
    let expression = expression.trim();
    if expression.contains(':') {
        let fields: Vec<&str> = expression.split(':').collect();
        let whole = |s: &str| s.trim().parse::<u64>().ok().map(|v| v as f64);
        let (hours, minutes) = (whole(fields.first()?)?, whole(fields.get(1)?)?);
        let seconds: f64 = fields.get(2)?.trim().parse().ok()?;
        let mut ms = (hours * 3600.0 + minutes * 60.0 + seconds) * 1000.0;
        if let Some(frames) = fields.get(3) {
            let (frames, sub) = frames.split_once('.').unwrap_or((frames, "0"));
            ms += whole(frames)? * 1000.0 / clock.frames_per_second;
            ms += whole(sub)? * 1000.0 / (clock.frames_per_second * clock.sub_frames);
        }
        return (fields.len() <= 4).then_some(ms);
    }
    let metric_at = expression.find(|c: char| c.is_ascii_alphabetic())?;
    let value: f64 = expression[..metric_at].parse().ok()?;
    let per_ms = match &expression[metric_at..] {
        "h" => 3_600_000.0,
        "m" => 60_000.0,
        "s" => 1000.0,
        "ms" => 1.0,
        "f" => 1000.0 / clock.frames_per_second,
        "t" => 1000.0 / clock.ticks_per_second,
        _ => return None,
    };
    Some(value * per_ms)
}

/// The look a style gives, each part only when it says it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Look {
    italic: bool,
    bold: bool,
    underline: bool,
    colour: Option<Rgb>,
}

/// A style's own attributes by local name, and the styles it refers to.
#[derive(Debug, Clone, Default)]
struct StyleSheet {
    attributes: Vec<(String, String)>,
    refers: Vec<String>,
}

fn id_of<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    node.attribute((roxmltree::NS_XML_URI, "id"))
        .or_else(|| node.attribute("id"))
}

/// The styling attributes written on an element (any namespace but none:
/// `tts:` has had three).
fn styling_of(node: Node) -> Vec<(String, String)> {
    node.attributes()
        .filter(|a| {
            a.namespace()
                .is_some_and(|n| n != roxmltree::NS_XML_URI && !n.contains("parameter"))
        })
        .map(|a| (a.name().to_owned(), a.value().to_owned()))
        .collect()
}

fn references(node: Node) -> Vec<String> {
    node.attribute("style")
        .map(|s| s.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

fn styles_of(root: Node) -> HashMap<String, StyleSheet> {
    root.descendants()
        .filter(|n| n.has_tag_name_local("style"))
        .filter_map(|n| {
            Some((
                id_of(n)?.to_owned(),
                StyleSheet {
                    attributes: styling_of(n),
                    refers: references(n),
                },
            ))
        })
        .collect()
}

/// Every styling attribute a style gives, following its references first.
fn flattened(
    id: &str,
    styles: &HashMap<String, StyleSheet>,
    depth: u32,
    into: &mut Vec<(String, String)>,
) {
    if depth > STYLE_DEPTH_MOST {
        return;
    }
    let Some(sheet) = styles.get(id) else {
        return;
    };
    for inner in &sheet.refers {
        flattened(inner, styles, depth + 1, into);
    }
    into.extend(sheet.attributes.iter().cloned());
}

fn apply(look: &mut Look, attributes: &[(String, String)]) {
    for (name, value) in attributes {
        let value = value.trim();
        match name.as_str() {
            "fontStyle" => look.italic = value == "italic" || value == "oblique",
            "fontWeight" => look.bold = value == "bold",
            "textDecoration" => {
                if value.contains("noUnderline") || value == "none" {
                    look.underline = false;
                } else if value.contains("underline") {
                    look.underline = true;
                }
            }
            "color" => look.colour = Rgb::css(value).or(look.colour),
            _ => {}
        }
    }
}

/// The picture's size in pixels and in cells, for regions placed in either.
#[derive(Debug, Clone, Copy)]
struct Geometry {
    height_px: Option<f64>,
    rows: f64,
}

impl Geometry {
    fn of(root: Node) -> Self {
        let height_px = root
            .attributes()
            .find(|a| a.name() == "extent" && a.namespace().is_some())
            .and_then(|a| a.value().split_whitespace().nth(1).map(str::to_owned))
            .and_then(|h| h.strip_suffix("px").and_then(|v| v.parse::<f64>().ok()));
        let rows = parameter(root, "cellResolution")
            .and_then(|v| {
                v.split_whitespace()
                    .nth(1)
                    .and_then(|r| r.parse::<f64>().ok())
            })
            .filter(|r| *r > 0.0)
            .unwrap_or(15.0);
        Self { height_px, rows }
    }

    /// A length down the picture, in percent of its height.
    fn percent(self, length: &str) -> Option<f64> {
        let length = length.trim();
        if let Some(v) = length.strip_suffix('%') {
            return v.parse().ok();
        }
        if let Some(v) = length.strip_suffix("px") {
            return Some(v.parse::<f64>().ok()? * 100.0 / self.height_px?);
        }
        if let Some(v) = length.strip_suffix('c') {
            return Some(v.parse::<f64>().ok()? * 100.0 / self.rows);
        }
        None
    }
}

/// The band of every region, from where its text sits.
fn regions_of(
    root: Node,
    styles: &HashMap<String, StyleSheet>,
    geometry: &Geometry,
) -> HashMap<String, Option<Band>> {
    let mut regions = HashMap::new();
    for region in root
        .descendants()
        .filter(|n| n.has_tag_name_local("region"))
    {
        let Some(id) = id_of(region) else {
            continue;
        };
        let mut attributes = Vec::new();
        for reference in references(region) {
            flattened(&reference, styles, 0, &mut attributes);
        }
        for inner in region.children().filter(|n| n.has_tag_name_local("style")) {
            attributes.extend(styling_of(inner));
        }
        attributes.extend(styling_of(region));
        let get = |name: &str| {
            attributes
                .iter()
                .rev()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        };
        let origin_y = get("origin")
            .and_then(|o| o.split_whitespace().nth(1))
            .and_then(|y| geometry.percent(y));
        let extent_y = get("extent")
            .and_then(|e| e.split_whitespace().nth(1))
            .and_then(|y| geometry.percent(y));
        let align = get("displayAlign");
        if get("origin").is_none() && align.is_none() {
            continue;
        }
        let (top, height) = (
            origin_y.unwrap_or(0.0),
            extent_y.unwrap_or(100.0 - origin_y.unwrap_or(0.0)),
        );
        let anchor = match align.map(str::trim) {
            Some("after") => top + height,
            Some("center") => top + height / 2.0,
            _ => top,
        };
        regions.insert(id.to_owned(), (anchor < 50.0).then_some(Band::Top));
    }
    regions
}

struct Reader {
    clock: Clock,
    styles: HashMap<String, StyleSheet>,
    regions: HashMap<String, Option<Band>>,
}

/// What an element passes to its children: its time, look and band, and how
/// deep it is.
#[derive(Debug, Clone, Copy)]
struct Scope {
    begin: f64,
    end: Option<f64>,
    look: Look,
    band: Option<Band>,
    depth: u32,
}

/// A piece of a paragraph: words with their look and time, or a break.
enum Piece {
    Words {
        text: String,
        look: Look,
        begin: f64,
        end: Option<f64>,
    },
    Break,
}

impl Reader {
    fn look_of(&self, node: Node, inherited: Look) -> Look {
        let mut look = inherited;
        let mut attributes = Vec::new();
        for reference in references(node) {
            flattened(&reference, &self.styles, 0, &mut attributes);
        }
        attributes.extend(styling_of(node));
        apply(&mut look, &attributes);
        look
    }

    fn band_of(&self, node: Node, inherited: Option<Band>) -> Option<Band> {
        // A region named here decides, even to put the cue back at the foot.
        match node.attribute("region") {
            Some(region) => self.regions.get(region).copied().flatten(),
            None => inherited,
        }
    }

    /// An element's begin and end, on the document's clock.
    fn times(&self, node: Node, parent_begin: f64, parent_end: Option<f64>) -> (f64, Option<f64>) {
        let get = |name: &str| node.attribute(name).and_then(|v| time_ms(v, self.clock));
        let begin = parent_begin + get("begin").unwrap_or(0.0);
        let end = get("end")
            .map(|e| parent_begin + e)
            .or_else(|| get("dur").map(|d| begin + d))
            .or(parent_end);
        (begin, end)
    }

    /// What an element's children inherit from it.
    fn inside(&self, child: Node, scope: Scope) -> Scope {
        let (begin, end) = self.times(child, scope.begin, scope.end);
        Scope {
            begin,
            end,
            look: self.look_of(child, scope.look),
            band: self.band_of(child, scope.band),
            depth: scope.depth + 1,
        }
    }

    fn walk(&self, node: Node, scope: Scope, cues: &mut Vec<(Cue, bool)>) {
        if scope.depth >= DEPTH_MOST {
            return;
        }
        for child in node.children().filter(Node::is_element) {
            let inner = self.inside(child, scope);
            match child.tag_name().name() {
                "div" => self.walk(child, inner, cues),
                "p" => {
                    let mut pieces = Vec::new();
                    let preserve =
                        child.attribute((roxmltree::NS_XML_URI, "space")) == Some("preserve");
                    self.pieces(child, inner, preserve, &mut pieces);
                    cues.extend(group(&pieces, inner.band));
                }
                _ => {}
            }
        }
    }

    fn pieces(&self, node: Node, scope: Scope, preserve: bool, out: &mut Vec<Piece>) {
        if scope.depth >= DEPTH_MOST {
            return;
        }
        let Scope {
            begin, end, look, ..
        } = scope;
        for child in node.children() {
            if child.is_text() {
                let text = child.text().unwrap_or("");
                if preserve {
                    for (n, line) in text.split('\n').enumerate() {
                        if n > 0 {
                            out.push(Piece::Break);
                        }
                        out.push(Piece::Words {
                            text: line.to_owned(),
                            look,
                            begin,
                            end,
                        });
                    }
                } else {
                    out.push(Piece::Words {
                        text: collapse(text),
                        look,
                        begin,
                        end,
                    });
                }
                continue;
            }
            if !child.is_element() {
                continue;
            }
            match child.tag_name().name() {
                "br" => out.push(Piece::Break),
                "span" => self.pieces(child, self.inside(child, scope), preserve, out),
                _ => {}
            }
        }
    }
}

/// Whitespace collapsed to single spaces, as `xml:space="default"` asks.
fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        if c.is_whitespace() && c != '\u{a0}' {
            space = true;
            continue;
        }
        if space {
            out.push(' ');
        }
        space = false;
        out.push(c);
    }
    if space {
        out.push(' ');
    }
    out
}

/// A paragraph's pieces as cues: one per distinct time, each keeping the
/// paragraph's breaks. The flag is "no end was given".
fn group(pieces: &[Piece], band: Option<Band>) -> Vec<(Cue, bool)> {
    let mut times: Vec<(f64, Option<f64>)> = Vec::new();
    for piece in pieces {
        if let Piece::Words {
            begin, end, text, ..
        } = piece
            && !text.trim().is_empty()
            && !times.iter().any(|t| t.0 == *begin && t.1 == *end)
        {
            times.push((*begin, *end));
        }
    }
    times
        .into_iter()
        .map(|(begin, end)| {
            let mut lines: Vec<Vec<Span>> = vec![Vec::new()];
            for piece in pieces {
                match piece {
                    Piece::Break => lines.push(Vec::new()),
                    Piece::Words {
                        text,
                        look,
                        begin: b,
                        end: e,
                    } if *b == begin && *e == end => {
                        if let Some(line) = lines.last_mut() {
                            line.push(Span {
                                text: text.clone(),
                                italic: look.italic,
                                bold: look.bold,
                                underline: look.underline,
                                colour: look.colour,
                            });
                        }
                    }
                    Piece::Words { .. } => {}
                }
            }
            let start_ms = begin.max(0.0).round() as u64;
            let cue = Cue {
                start_ms,
                end_ms: end.map_or(start_ms, |e| e.max(0.0).round() as u64),
                lines,
                band,
            };
            (cue, end.is_none())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOCUMENT: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttp="http://www.w3.org/ns/ttml#parameter"
    xmlns:tts="http://www.w3.org/ns/ttml#styling" xml:lang="en"
    ttp:tickRate="10000000" ttp:frameRate="25" ttp:frameRateMultiplier="1000 1000"
    tts:extent="1920px 1080px">
  <head>
    <styling>
      <style xml:id="base" tts:color="#ffff00"/>
      <style xml:id="slanted" style="base" tts:fontStyle="italic"/>
    </styling>
    <layout>
      <region xml:id="top" tts:origin="10% 5%" tts:extent="80% 20%" tts:displayAlign="before"/>
      <region xml:id="bottom" tts:origin="10% 80%" tts:extent="80% 15%" tts:displayAlign="after"/>
    </layout>
  </head>
  <body region="bottom">
    <div>
      <p begin="10000000t" end="25000000t">Ticks: <span style="slanted">one</span><br/>and   a
         half</p>
      <p begin="00:00:03:00" end="00:00:04:12" region="top">Frames</p>
      <p begin="00:00:05.250" dur="1.5s" tts:fontWeight="bold">Clock</p>
    </div>
    <div begin="10s">
      <p begin="1s" end="2s">Nested</p>
      <p begin="3s"><span begin="0s" end="1s">roll</span> <span begin="1s" end="2s">up</span></p>
    </div>
  </body>
</tt>"##;

    #[test]
    fn every_time_expression_and_look_is_read() {
        let cues = read(DOCUMENT).unwrap_or_default();
        let times: Vec<(u64, u64)> = cues.iter().map(|c| (c.start_ms, c.end_ms)).collect();
        assert_eq!(
            times,
            vec![
                (1000, 2500),
                (3000, 4480),
                (5250, 6750),
                (11000, 12000),
                (13000, 14000),
                (14000, 15000)
            ]
        );
        assert_eq!(cues[0].text(), "Ticks: one\nand a half");
        let one = &cues[0].lines[0][1];
        assert!(one.italic);
        assert_eq!(
            one.colour,
            Some(Rgb::new(255, 255, 0)),
            "the style it refers to"
        );
        assert_eq!(cues[0].band, None, "a region at the foot follows the lane");
        assert_eq!(cues[1].band, Some(Band::Top));
        assert!(cues[2].lines[0][0].bold);
        assert_eq!(cues[4].text(), "roll");
        assert_eq!(cues[5].text(), "up");
    }

    #[test]
    fn a_dfxp_with_the_old_namespace_is_read_too() {
        let dfxp = r#"<tt xmlns="http://www.w3.org/2006/10/ttaf1" xmlns:tts="http://www.w3.org/2006/10/ttaf1#styling">
<body><div><p begin="0:00:01.0" end="0:00:02.0" tts:textDecoration="underline">Old</p></div></body></tt>"#;
        let cues = read(dfxp).unwrap_or_default();
        assert_eq!(cues.len(), 1);
        assert!(cues[0].lines[0][0].underline);
        assert_eq!(read("<tt><p>").map(|c| c.len()), Err(Damaged));
    }
}
