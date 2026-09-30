//! SBV (`.sbv`): YouTube's own export, SubViewer's timing with thousandths
//! and no header.
//!
//! ```text
//! 0:00:01.000,0:00:03.500
//! Hello
//! world
//! ```
//!
//! ffmpeg 8.1.2 has no reader for it.

use crate::subtitles::cue::{self, Cue, Span};

/// Whether `line` is an SBV timing line: two clock times with thousandths,
/// a comma between.
pub(crate) fn is_timing(line: &str) -> bool {
    timing(line).is_some()
}

fn timing(line: &str) -> Option<(u64, u64)> {
    let (start, end) = line.trim().split_once(',')?;
    let shaped = |s: &str| {
        s.split(':').count() == 3 && s.rsplit_once('.').is_some_and(|(_, f)| f.len() == 3)
    };
    if !shaped(start) || !shaped(end) {
        return None;
    }
    Some((cue::clock_ms(start)?, cue::clock_ms(end)?))
}

/// Reads an SBV's cues.
#[must_use]
pub fn read(text: &str) -> Vec<Cue> {
    let lines: Vec<&str> = text.lines().collect();
    let mut cues = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((start_ms, end_ms)) = timing(lines[i]) else {
            i += 1;
            continue;
        };
        i += 1;
        let mut body = Vec::new();
        while i < lines.len() && !lines[i].trim().is_empty() && !is_timing(lines[i]) {
            body.push(vec![Span::plain(lines[i])]);
            i += 1;
        }
        cues.push(Cue {
            start_ms,
            end_ms,
            lines: body,
            band: None,
        });
    }
    cues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_sbv_is_blocks_of_timing_and_text() {
        let cues = read(
            "0:00:01.000,0:00:03.500\nHello\nworld\n\n0:00:04.250,0:00:05.000\n>> Next speaker\n",
        );
        assert_eq!(cues.len(), 2);
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 3500));
        assert_eq!(cues[0].text(), "Hello\nworld");
        assert_eq!((cues[1].start_ms, cues[1].end_ms), (4250, 5000));
        assert_eq!(cues[1].text(), ">> Next speaker");
    }
}
