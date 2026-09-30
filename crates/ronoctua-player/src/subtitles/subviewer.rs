//! SubViewer (`.sub`), both versions.
//!
//! Version 2 has an `[INFORMATION]` header and blocks of a timing line and
//! text, lines broken by `[br]`:
//!
//! ```text
//! 00:00:01.00,00:00:03.00
//! Hello[br]world
//! ```
//!
//! Version 1 is `[SUBTITLE]` followed by a time in brackets, the text, and a
//! time that ends it:
//!
//! ```text
//! [00:00:01]
//! Hello|world
//! [00:00:03]
//! ```

use crate::subtitles::cue::{self, Cue, Span};

/// Whether `line` is a version 2 timing line (`hh:mm:ss.cc,hh:mm:ss.cc`,
/// hundredths or thousandths).
pub(crate) fn is_timing(line: &str) -> bool {
    timing(line).is_some()
}

fn timing(line: &str) -> Option<(u64, u64)> {
    let (start, end) = line.trim().split_once(',')?;
    let shaped = |s: &str| {
        let fraction = s.rsplit_once('.').map(|(_, f)| f.len());
        s.split(':').count() == 3 && matches!(fraction, Some(2 | 3))
    };
    if !shaped(start) || !shaped(end) {
        return None;
    }
    Some((cue::clock_ms(start)?, cue::clock_ms(end)?))
}

/// A version 1 time line: `[hh:mm:ss]`.
fn bracketed(line: &str) -> Option<u64> {
    let inner = line.trim().strip_prefix('[')?.strip_suffix(']')?;
    (inner.split(':').count() == 3)
        .then(|| cue::clock_ms(inner))
        .flatten()
}

/// Reads a SubViewer's cues.
#[must_use]
pub fn read(text: &str) -> Vec<Cue> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.iter().any(|l| is_timing(l)) {
        return version_two(&lines);
    }
    version_one(&lines)
}

fn version_two(lines: &[&str]) -> Vec<Cue> {
    let mut cues = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((start_ms, end_ms)) = timing(lines[i]) else {
            i += 1;
            continue;
        };
        let mut body = Vec::new();
        i += 1;
        while i < lines.len() && !lines[i].trim().is_empty() && !is_timing(lines[i]) {
            body.push(lines[i]);
            i += 1;
        }
        let joined = body.join("[br]");
        cues.push(Cue {
            start_ms,
            end_ms,
            lines: split_breaks(&joined),
            band: None,
        });
    }
    cues
}

fn version_one(lines: &[&str]) -> Vec<Cue> {
    let mut cues = Vec::new();
    let mut open: Option<(u64, Vec<&str>)> = None;
    for line in lines {
        if let Some(at) = bracketed(line) {
            if let Some((start_ms, body)) = open.take()
                && !body.is_empty()
            {
                cues.push(Cue {
                    start_ms,
                    end_ms: at,
                    lines: body
                        .iter()
                        .flat_map(|l| l.split('|'))
                        .map(|l| vec![Span::plain(l)])
                        .collect(),
                    band: None,
                });
            }
            open = Some((at, Vec::new()));
        } else if let Some((_, body)) = open.as_mut()
            && !line.trim().is_empty()
        {
            body.push(line);
        }
    }
    cues
}

/// `[br]` (any case) breaks the line.
fn split_breaks(text: &str) -> Vec<Vec<Span>> {
    let lower = text.to_ascii_lowercase();
    let mut lines = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find("[br]") {
        lines.push(vec![Span::plain(&text[from..from + at])]);
        from += at + 4;
    }
    lines.push(vec![Span::plain(&text[from..])]);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_two_breaks_on_br() {
        let cues = read(
            "[INFORMATION]\n[TITLE]A film\n[END INFORMATION]\n[SUBTITLE]\n[COLF]&HFFFFFF,[STYLE]no\n\
00:00:01.00,00:00:03.50\nHello[br]world\n\n00:00:04.00,00:00:05.00\nAgain\n",
        );
        assert_eq!(cues.len(), 2);
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 3500));
        assert_eq!(cues[0].text(), "Hello\nworld");
    }

    #[test]
    fn version_one_runs_from_one_time_to_the_next() {
        let cues = read(
            "[SUBTITLE]\n[00:00:01]\nHello|world\n[00:00:03]\n\n[00:00:04]\nAgain\n[00:00:06]\n",
        );
        assert_eq!(cues.len(), 2);
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 3000));
        assert_eq!(cues[0].text(), "Hello\nworld");
        assert_eq!((cues[1].start_ms, cues[1].end_ms), (4000, 6000));
    }
}
