//! MPL2 (`.txt`, `.mpl`): deciseconds in brackets.
//!
//! ```text
//! [10][25]/Italic line|plain line
//! ```
//!
//! `|` separates lines, and a line that begins with `/` is in italics. An
//! empty end lasts until the next cue.

use crate::subtitles::cue::{self, Cue, Span};

/// Whether `line` is an MPL2 cue (`[n][m]` or `[n][]`).
pub(crate) fn is_line(line: &str) -> bool {
    times(line).is_some()
}

fn times(line: &str) -> Option<(u64, Option<u64>, &str)> {
    let rest = line.trim_start().strip_prefix('[')?;
    let (first, rest) = rest.split_once(']')?;
    let rest = rest.strip_prefix('[')?;
    let (last, text) = rest.split_once(']')?;
    let number = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse::<u64>().ok())
            .flatten()
    };
    let first = number(first)?;
    let last = if last.is_empty() {
        None
    } else {
        Some(number(last)?)
    };
    Some((first * 100, last.map(|l| l * 100), text))
}

/// Reads an MPL2's cues.
#[must_use]
pub fn read(text: &str) -> Vec<Cue> {
    let parsed: Vec<(u64, Option<u64>, &str)> = text.lines().filter_map(times).collect();
    parsed
        .iter()
        .enumerate()
        .map(|(n, &(start_ms, end_ms, words))| Cue {
            start_ms,
            end_ms: end_ms
                .unwrap_or_else(|| cue::untimed_end(start_ms, parsed.get(n + 1).map(|p| p.0))),
            lines: words
                .split('|')
                .map(|line| match line.strip_prefix('/') {
                    Some(slanted) => vec![Span {
                        italic: true,
                        ..Span::plain(slanted)
                    }],
                    None => vec![Span::plain(line)],
                })
                .collect(),
            band: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deciseconds_and_slanted_lines() {
        let cues = read("[10][25]/Hello|World\n[30][]Next\n[40][50]Last\n");
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 2500));
        assert!(cues[0].lines[0][0].italic);
        assert!(!cues[0].lines[1][0].italic);
        assert_eq!((cues[1].start_ms, cues[1].end_ms), (3000, 4000));
    }
}
