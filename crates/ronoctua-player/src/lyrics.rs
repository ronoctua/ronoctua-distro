//! Lyrics: LRC read, and the line sung at a moment.
//!
//! Pure — the daemon finds the text (a `.lrc` beside the file, the file's own
//! tag, or LRCLIB when the person turned it on) and this makes lines of it.

use crate::wire::LyricLine;

/// Parses LRC text. Lines with several stamps (`[00:12.00][01:30.50]words`)
/// are one line per stamp; `[offset:+250]` moves them all; tags that are not
/// times (`[ar:…]`, `[ti:…]`) are dropped. With no stamp at all the text is
/// UNSYNCHRONISED lyrics, and each line comes back with `at: None`.
#[must_use]
pub fn parse(text: &str) -> Vec<LyricLine> {
    let mut offset = 0.0f64;
    let mut timed: Vec<LyricLine> = Vec::new();
    let mut plain: Vec<LyricLine> = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut stamps = Vec::new();
        while let Some(inner) = rest.strip_prefix('[') {
            let Some(end) = inner.find(']') else { break };
            let tag = &inner[..end];
            rest = &inner[end + 1..];
            if let Some(ms) = tag.strip_prefix("offset:") {
                // The LRC convention: a POSITIVE offset makes lyrics come SOONER.
                offset = ms.trim().parse::<f64>().unwrap_or(0.0) / 1000.0;
            } else if let Some(at) = stamp(tag) {
                stamps.push(at);
            }
        }
        let words = rest.trim().to_owned();
        if stamps.is_empty() {
            if !words.is_empty() && !raw.trim_start().starts_with('[') {
                plain.push(LyricLine {
                    at: None,
                    text: words,
                });
            }
            continue;
        }
        for at in stamps {
            timed.push(LyricLine {
                at: Some(at),
                text: words.clone(),
            });
        }
    }
    if timed.is_empty() {
        return plain;
    }
    for l in &mut timed {
        l.at = l.at.map(|t| (t - offset).max(0.0));
    }
    timed.sort_by(|a, b| a.at.partial_cmp(&b.at).unwrap_or(std::cmp::Ordering::Equal));
    timed
}

/// `mm:ss.xx` (or `mm:ss`, or `mm:ss:xx`) in seconds.
fn stamp(tag: &str) -> Option<f64> {
    let (m, s) = tag.split_once(':')?;
    let minutes: f64 = m.trim().parse().ok()?;
    let s = s.replacen(':', ".", 1);
    let seconds: f64 = s.trim().parse().ok()?;
    (minutes >= 0.0 && seconds >= 0.0).then_some(minutes * 60.0 + seconds)
}

/// Which line is being sung at `position` — the last one whose time has come.
#[must_use]
pub fn current(lines: &[LyricLine], position: f64) -> Option<usize> {
    lines
        .iter()
        .rposition(|l| l.at.is_some_and(|t| t <= position))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_sort_and_a_line_with_two_stamps_is_sung_twice() {
        let l = parse("[ti:Song]\n[00:05.00]one\n[00:01.50][00:10.00]refrain\n");
        let at: Vec<f64> = l.iter().filter_map(|x| x.at).collect();
        assert_eq!(at, vec![1.5, 5.0, 10.0]);
        assert_eq!(l[0].text, "refrain");
        assert_eq!(l[2].text, "refrain");
    }

    #[test]
    fn a_positive_offset_brings_every_line_sooner() {
        let l = parse("[offset:+500]\n[00:02.00]a\n");
        assert_eq!(l[0].at, Some(1.5));
    }

    #[test]
    fn the_current_line_is_the_last_one_reached() {
        let l = parse("[00:01.00]a\n[00:03.00]b\n");
        assert_eq!(current(&l, 0.5), None);
        assert_eq!(current(&l, 1.0), Some(0));
        assert_eq!(current(&l, 9.0), Some(1));
    }

    #[test]
    fn lyrics_without_stamps_are_kept_unsynchronised() {
        let l = parse("first line\nsecond line\n");
        assert_eq!(l.len(), 2);
        assert!(l.iter().all(|x| x.at.is_none()));
    }
}
