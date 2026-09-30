//! WebVTT (`.vtt`): SRT's successor, with a header, blocks that are not cues
//! (`NOTE`, `STYLE`, `REGION`) and settings after the timing.
//!
//! Of the settings only `line` says something the merge keeps: a cue put on
//! a line in the upper half of the picture asked for the top band.

use crate::subtitles::cue::{self, Band, Cue, Dialect};

/// Reads a WebVTT's cues.
#[must_use]
pub fn read(text: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    // Blocks are separated by blank lines; the first is the header.
    let mut block: Vec<&str> = Vec::new();
    let mut header = true;
    for line in text.lines().chain(std::iter::once("")) {
        if !line.trim().is_empty() {
            block.push(line);
            continue;
        }
        if header {
            header = block.is_empty();
            block.clear();
            continue;
        }
        if let Some(c) = cue_of(&block) {
            cues.push(c);
        }
        block.clear();
    }
    cues
}

fn cue_of(block: &[&str]) -> Option<Cue> {
    let first = block.first()?.trim_start();
    if ["NOTE", "STYLE", "REGION"]
        .iter()
        .any(|w| first.starts_with(w) && !first.contains("-->"))
    {
        return None;
    }
    // An identifier may come before the timing line.
    let at = block.iter().position(|l| l.contains("-->"))?;
    let (start, rest) = block[at].split_once("-->")?;
    let mut words = rest.split_whitespace();
    let end = words.next()?;
    let start_ms = cue::clock_ms(start)?;
    let end_ms = cue::clock_ms(end)?;
    let band = words
        .find_map(|setting| setting.strip_prefix("line:"))
        .and_then(band_of_line);
    let (lines, markup_band) = cue::markup(&block[at + 1..].join("\n"), Dialect::WebVtt);
    Some(Cue {
        start_ms,
        end_ms,
        lines,
        band: band.or(markup_band),
    })
}

/// `line:0` (a line number from the top) or `line:10%` is the top;
/// `line:-1` (from the bottom) and `line:90%` follow the lane. An alignment
/// may follow a comma.
fn band_of_line(value: &str) -> Option<Band> {
    let value = value.split(',').next()?.trim();
    let top = match value.strip_suffix('%') {
        Some(percent) => percent.parse::<f64>().ok()? < 50.0,
        None => value.parse::<i64>().ok()? >= 0,
    };
    top.then_some(Band::Top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitles::Rgb;

    #[test]
    fn a_vtt_skips_what_is_not_a_cue_and_keeps_settings_and_classes() {
        let cues = read(
            "WEBVTT - a title\nKind: captions\n\nNOTE a comment\n--> not a cue\n\nSTYLE\n::cue { color: red }\n\n\
intro\n00:01.000 --> 00:02.500 line:0 align:center\n<c.yellow>Top</c> <v Ana>speaks</v>\n\n\
00:00:03.000 --> 00:00:04.000 line:90%\n<i>Bottom</i> &amp; <00:00:03.500>more\n",
        );
        assert_eq!(cues.len(), 2);
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 2500));
        assert_eq!(cues[0].band, Some(Band::Top));
        assert_eq!(cues[0].lines[0][0].colour, Some(Rgb::new(255, 255, 0)));
        assert_eq!(cues[0].text(), "Top speaks");
        assert_eq!(cues[1].band, None, "the foot follows the lane");
        assert_eq!(cues[1].text(), "Bottom & more");
        assert!(cues[1].lines[0][0].italic);
    }
}
