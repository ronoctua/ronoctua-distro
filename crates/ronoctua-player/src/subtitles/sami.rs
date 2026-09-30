//! Microsoft SAMI (`.smi`): HTML with a `<SYNC Start=ms>` before every
//! change of text.
//!
//! ```text
//! <SAMI><BODY>
//! <SYNC Start=1000><P Class=ENCC>Hello<br>world
//! <SYNC Start=3000><P Class=ENCC>&nbsp;
//! </BODY></SAMI>
//! ```
//!
//! A cue lasts until the next `SYNC`; one that holds only a space clears the
//! screen. A file can carry several languages, one `Class` each; the first
//! class that says something is the lane's, and the others are left out —
//! adding the same file again is not a second language, so a door that wants
//! one says which (not yet: see the integration notes).

use crate::subtitles::cue::{self, Cue, Dialect};

/// Reads a SAMI's cues.
#[must_use]
pub fn read(text: &str) -> Vec<Cue> {
    let lower = text.to_ascii_lowercase();
    // Every `<sync`, where its tag ends, and its start.
    let mut syncs: Vec<(usize, usize, u64)> = Vec::new();
    let mut from = 0;
    while let Some(found) = lower[from..].find("<sync") {
        let at = from + found;
        let Some(close) = lower[at..].find('>') else {
            break;
        };
        let tag_end = at + close + 1;
        from = tag_end;
        let start = cue::attribute(&text[at + 5..tag_end - 1], "start")
            .and_then(|v| v.trim().parse::<u64>().ok());
        if let Some(start) = start {
            syncs.push((at, tag_end, start));
        }
    }
    let body_end = lower.find("</body>").unwrap_or(text.len());
    let mut class: Option<String> = None;
    let mut cues = Vec::new();
    for (n, &(_, tag_end, start_ms)) in syncs.iter().enumerate() {
        let next = syncs.get(n + 1);
        let content_end = next.map_or(body_end, |s| s.0).max(tag_end);
        let content = &text[tag_end..content_end];
        let Some(body) = paragraph(content, &mut class) else {
            continue;
        };
        let (lines, band) = cue::markup(&body, Dialect::Html);
        // Until the next SYNC, however far: SAMI clears the screen itself.
        let end_ms = next.map_or(start_ms + cue::UNTIMED_MOST_MS, |s| s.2);
        cues.push(Cue {
            start_ms,
            end_ms,
            lines,
            band,
        });
    }
    cues
}

/// The text of the paragraph of `class` in one SYNC's content — the first
/// class with words decides it — or the whole content when it has no `<P>`.
fn paragraph(content: &str, class: &mut Option<String>) -> Option<String> {
    let lower = content.to_ascii_lowercase();
    let mut paragraphs: Vec<(Option<String>, &str)> = Vec::new();
    let mut from = 0;
    while let Some(found) = find_p(&lower[from..]) {
        let at = from + found;
        let close = at + lower[at..].find('>')?;
        let next = find_p(&lower[close..]).map_or(content.len(), |n| close + n);
        let end = lower[close..next].find("</p").map_or(next, |e| close + e);
        let this_class =
            cue::attribute(&content[at + 2..close], "class").map(|c| c.to_ascii_lowercase());
        paragraphs.push((this_class, &content[close + 1..end]));
        from = next;
    }
    if paragraphs.is_empty() {
        return Some(content.to_owned());
    }
    if class.is_none() {
        *class = paragraphs
            .iter()
            .find(|(_, text)| !is_blank(text))
            .map(|(c, _)| c.clone().unwrap_or_default());
    }
    let wanted = class.clone().unwrap_or_default();
    paragraphs
        .into_iter()
        .find(|(c, _)| c.clone().unwrap_or_default() == wanted)
        .map(|(_, text)| text.to_owned())
}

/// `<p` followed by a space or `>`: a paragraph, not `<param`.
fn find_p(lower: &str) -> Option<usize> {
    lower.match_indices("<p").map(|(at, _)| at).find(|&at| {
        lower[at + 2..]
            .chars()
            .next()
            .is_some_and(|c| c.is_whitespace() || c == '>')
    })
}

fn is_blank(html: &str) -> bool {
    cue::entities(html)
        .replace("<br>", "")
        .chars()
        .all(|c| c.is_whitespace() || c == '\u{a0}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sami_lasts_until_the_next_sync_and_keeps_one_language() {
        let mut cues = read(
            "<SAMI><HEAD><STYLE><!-- .ENCC {Name: English; lang: en-US;} --></STYLE></HEAD><BODY>\n\
<SYNC Start=1000><P Class=ENCC>Hello<br>\n  <i>world</i> &amp; you\n<P Class=FRCC>Bonjour\n\
<SYNC Start=3000><P Class=ENCC>&nbsp;\n\
<SYNC Start=4000><P Class=ENCC><font color=\"#00ff00\">Green</font>\n\
</BODY></SAMI>",
        );
        cue::tidy(&mut cues);
        assert_eq!(cues.len(), 2, "the blank one only clears");
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 3000));
        assert_eq!(cues[0].text(), "Hello\nworld & you", "English only");
        assert!(cues[0].lines[1][0].italic);
        // The last one has no next SYNC: it stays at most the ceiling.
        assert_eq!(cues[1].start_ms, 4000);
        assert_eq!(cues[1].end_ms, 4000 + cue::UNTIMED_MOST_MS);
        assert_eq!(
            cues[1].lines[0][0].colour,
            Some(crate::subtitles::Rgb::new(0, 255, 0))
        );
    }
}
