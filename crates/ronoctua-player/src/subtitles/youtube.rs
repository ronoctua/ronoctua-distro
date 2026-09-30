//! YouTube's timed text: the three XML formats (`srv1`, `srv2`, `srv3`) and
//! the JSON one (`json3`), which is what `yt-dlp --sub-format` saves.
//!
//! - **srv1**: `<transcript><text start="1.5" dur="2">…</text>`, seconds, the
//!   text HTML-escaped once more inside the XML.
//! - **srv2**: `<timedtext><text t="1500" d="2000">…</text>`, milliseconds.
//! - **srv3**: `<timedtext format="3">` with pens (`b`, `i`, `u`, `fc`) and
//!   window positions (`av`, the height in percent) in its head, and
//!   `<p t d p wp>` paragraphs of `<s>` segments.
//! - **json3**: the same as srv3, in JSON: `pens`, `wpWinPositions` and
//!   `events` of `segs`.
//!
//! A newline inside the text breaks the line. Automatic captions write each
//! word as it is spoken; they are shown a paragraph at a time.

use std::collections::HashMap;

use ronoctua_xml::roxmltree::{Node, ParsingOptions};
use serde::Deserialize;

use crate::subtitles::Damaged;
use crate::subtitles::cue::{self, Band, Cue, Dialect, Rgb, Span};

/// How many nodes a document may have.
const NODES_MOST: u32 = 2_000_000;

/// A pen: how a segment is drawn.
#[derive(Debug, Clone, Copy, Default)]
struct Pen {
    italic: bool,
    bold: bool,
    underline: bool,
    colour: Option<Rgb>,
}

impl Pen {
    fn span(self, text: &str) -> Span {
        Span {
            text: text.to_owned(),
            italic: self.italic,
            bold: self.bold,
            underline: self.underline,
            colour: self.colour,
        }
    }
}

/// A window position in the upper half of the picture is the top band; the
/// rest follows the lane.
fn band_of_height(percent: f64) -> Option<Band> {
    (percent < 50.0).then_some(Band::Top)
}

/// Reads YouTube's XML formats.
///
/// # Errors
///
/// [`Damaged`] when the text is not well-formed XML.
pub fn read_xml(text: &str) -> Result<Vec<Cue>, Damaged> {
    let options = ParsingOptions {
        allow_dtd: true,
        nodes_limit: NODES_MOST,
        ..ParsingOptions::default()
    };
    let document = ronoctua_xml::parse(text, options).map_err(|_| Damaged)?;
    let root = document.root_element();
    let flag = |n: Node, name: &str| n.attribute(name) == Some("1");
    let pens: HashMap<&str, Pen> = root
        .descendants()
        .filter(|n| n.has_tag_name("pen"))
        .filter_map(|n| {
            Some((
                n.attribute("id")?,
                Pen {
                    italic: flag(n, "i"),
                    bold: flag(n, "b"),
                    underline: flag(n, "u"),
                    colour: n
                        .attribute("fc")
                        .and_then(Rgb::hex)
                        .filter(|c| *c != Rgb::new(255, 255, 255)),
                },
            ))
        })
        .collect();
    let bands: HashMap<&str, Option<Band>> = root
        .descendants()
        .filter(|n| n.has_tag_name("wp"))
        .filter_map(|n| {
            Some((
                n.attribute("id")?,
                band_of_height(n.attribute("av")?.parse().ok()?),
            ))
        })
        .collect();
    let number = |n: Node, name: &str| n.attribute(name).and_then(|v| v.trim().parse::<f64>().ok());
    let mut cues = Vec::new();
    for node in root
        .descendants()
        .filter(|n| n.has_tag_name("p") || n.has_tag_name("text"))
    {
        // srv2 and srv3 count milliseconds (`t`, `d`); srv1 seconds.
        let (start, duration) = match (number(node, "t"), number(node, "start")) {
            (Some(t), _) => (t, number(node, "d").unwrap_or(0.0)),
            (None, Some(s)) => (s * 1000.0, number(node, "dur").unwrap_or(0.0) * 1000.0),
            (None, None) => continue,
        };
        let pen = node
            .attribute("p")
            .and_then(|p| pens.get(p).copied())
            .unwrap_or_default();
        let band = node
            .attribute("wp")
            .and_then(|w| bands.get(w).copied().flatten());
        let start_ms = start.max(0.0).round() as u64;
        let end_ms = (start + duration).max(0.0).round() as u64;
        let lines = if node.has_tag_name("text") {
            // srv1 and srv2: HTML inside, escaped once more.
            let inner = cue::entities(node.text().unwrap_or(""));
            cue::markup(&inner, Dialect::SubRip).0
        } else {
            segments(node, pen, &pens)
        };
        cues.push(Cue {
            start_ms,
            end_ms,
            lines,
            band,
        });
    }
    Ok(cues)
}

/// An srv3 paragraph's segments as lines.
fn segments(paragraph: Node, pen: Pen, pens: &HashMap<&str, Pen>) -> Vec<Vec<Span>> {
    let mut lines: Vec<Vec<Span>> = vec![Vec::new()];
    let mut add = |text: &str, pen: Pen| {
        for (n, part) in text.split('\n').enumerate() {
            if n > 0 {
                lines.push(Vec::new());
            }
            if let Some(line) = lines.last_mut() {
                line.push(pen.span(part));
            }
        }
    };
    for child in paragraph.children() {
        if child.is_text() {
            add(child.text().unwrap_or(""), pen);
        } else if child.has_tag_name("s") {
            let own = child
                .attribute("p")
                .and_then(|p| pens.get(p).copied())
                .unwrap_or(pen);
            add(child.text().unwrap_or(""), own);
        } else if child.has_tag_name("br") {
            add("\n", pen);
        }
    }
    lines
}

#[derive(Debug, Deserialize)]
struct Json3 {
    #[serde(default)]
    pens: Vec<JsonPen>,
    #[serde(default, rename = "wpWinPositions")]
    positions: Vec<JsonPosition>,
    #[serde(default)]
    events: Vec<JsonEvent>,
}

#[derive(Debug, Default, Deserialize)]
struct JsonPen {
    #[serde(rename = "bAttr")]
    bold: Option<u8>,
    #[serde(rename = "iAttr")]
    italic: Option<u8>,
    #[serde(rename = "uAttr")]
    underline: Option<u8>,
    #[serde(rename = "fcForeColor")]
    colour: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct JsonPosition {
    #[serde(rename = "avVerPos")]
    height_percent: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct JsonEvent {
    #[serde(rename = "tStartMs", default)]
    start_ms: u64,
    #[serde(rename = "dDurationMs", default)]
    duration_ms: u64,
    #[serde(rename = "pPenId")]
    pen: Option<usize>,
    #[serde(rename = "wpWinPosId")]
    position: Option<usize>,
    #[serde(default)]
    segs: Vec<JsonSegment>,
}

#[derive(Debug, Deserialize)]
struct JsonSegment {
    #[serde(default)]
    utf8: String,
    #[serde(rename = "pPenId")]
    pen: Option<usize>,
}

/// Reads YouTube's JSON format.
///
/// # Errors
///
/// [`Damaged`] when the text is not the JSON it should be.
pub fn read_json(text: &str) -> Result<Vec<Cue>, Damaged> {
    let file: Json3 = serde_json::from_str(text).map_err(|_| Damaged)?;
    let pens: Vec<Pen> = file
        .pens
        .iter()
        .map(|p| Pen {
            italic: p.italic == Some(1),
            bold: p.bold == Some(1),
            underline: p.underline == Some(1),
            colour: p.colour.filter(|&c| c != 0xFF_FFFF).map(|c| {
                let [_, red, green, blue] = c.to_be_bytes();
                Rgb::new(red, green, blue)
            }),
        })
        .collect();
    let pen_of = |id: Option<usize>| id.and_then(|i| pens.get(i).copied());
    let mut cues = Vec::new();
    for event in &file.events {
        // A window's definition, or an automatic caption's lone newline.
        if event.segs.iter().all(|s| s.utf8.trim().is_empty()) {
            continue;
        }
        let pen = pen_of(event.pen).unwrap_or_default();
        let mut lines: Vec<Vec<Span>> = vec![Vec::new()];
        for segment in &event.segs {
            let own = pen_of(segment.pen).unwrap_or(pen);
            for (n, part) in segment.utf8.split('\n').enumerate() {
                if n > 0 {
                    lines.push(Vec::new());
                }
                if let Some(line) = lines.last_mut() {
                    line.push(own.span(part));
                }
            }
        }
        let band = event
            .position
            .and_then(|p| file.positions.get(p))
            .and_then(|p| p.height_percent)
            .and_then(band_of_height);
        cues.push(Cue {
            start_ms: event.start_ms,
            end_ms: event.start_ms + event.duration_ms,
            lines,
            band,
        });
    }
    Ok(cues)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json3_keeps_pens_positions_and_breaks() {
        let cues = read_json(
            r#"{"wireMagic":"pb3",
 "pens":[{},{"iAttr":1,"fcForeColor":16776960}],
 "wpWinPositions":[{},{"apPoint":1,"avVerPos":0},{"apPoint":7,"avVerPos":100}],
 "events":[
  {"tStartMs":0,"dDurationMs":900000,"id":1,"wpWinPosId":2},
  {"tStartMs":1000,"dDurationMs":1500,"wWinId":1,"wpWinPosId":1,
   "segs":[{"utf8":"Hello "},{"utf8":"world\nagain","pPenId":1}]},
  {"tStartMs":2500,"dDurationMs":10,"aAppend":1,"segs":[{"utf8":"\n"}]},
  {"tStartMs":3000,"dDurationMs":1000,"wpWinPosId":2,"segs":[{"utf8":"Bottom"}]}
 ]}"#,
        )
        .unwrap_or_default();
        assert_eq!(cues.len(), 2);
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 2500));
        assert_eq!(cues[0].text(), "Hello world\nagain");
        let world = &cues[0].lines[0][1];
        assert!(world.italic);
        assert_eq!(world.colour, Some(Rgb::new(255, 255, 0)));
        assert_eq!(cues[0].band, Some(Band::Top));
        assert_eq!(cues[1].band, None, "the foot follows the lane");
        assert_eq!(read_json("{\"events\": 3}").map(|c| c.len()), Err(Damaged));
    }

    #[test]
    fn the_three_xml_formats_are_read() {
        let srv1 = read_xml(
            r#"<?xml version="1.0" encoding="utf-8" ?><transcript><text start="1.5" dur="2">It&amp;#39;s &amp;lt;i&amp;gt;fine&amp;lt;/i&amp;gt;</text></transcript>"#,
        )
        .unwrap_or_default();
        assert_eq!((srv1[0].start_ms, srv1[0].end_ms), (1500, 3500));
        assert_eq!(srv1[0].text(), "It's fine");
        assert!(srv1[0].lines[0][1].italic);

        let srv2 = read_xml(
            r#"<timedtext><window id="1"/><text t="500" d="1000" w="1">Two</text></timedtext>"#,
        )
        .unwrap_or_default();
        assert_eq!((srv2[0].start_ms, srv2[0].end_ms), (500, 1500));

        let srv3 = read_xml(
            r##"<timedtext format="3"><head><pen id="1" b="1" fc="#00FF00"/><wp id="0" ap="1" av="5"/></head>
<body><p t="1000" d="2000" wp="0">Top <s p="1">green</s>
next</p></body></timedtext>"##,
        )
        .unwrap_or_default();
        assert_eq!(srv3[0].band, Some(Band::Top));
        assert_eq!(srv3[0].text(), "Top green\nnext");
        assert!(srv3[0].lines[0][1].bold);
        assert_eq!(srv3[0].lines[0][1].colour, Some(Rgb::new(0, 255, 0)));
    }
}
