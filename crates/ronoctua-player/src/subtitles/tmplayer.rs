//! TMPlayer (`.txt`): a start time and the text, nothing else.
//!
//! ```text
//! 00:00:01:Hello|world
//! 00:00:04:
//! 0:00:05=Equals works too
//! 00:00:07,1=TMPlayer+ numbers
//! 00:00:07,2=its lines
//! ```
//!
//! ffmpeg 8.1.2 has no reader for it. A line that says nothing clears the
//! screen; otherwise a cue lasts until the next one, at most
//! [`UNTIMED_MOST_MS`](crate::subtitles::cue) — some files never clear.

use crate::subtitles::cue::{self, Cue, Span};

/// Whether `line` is a TMPlayer line.
pub(crate) fn is_line(line: &str) -> bool {
    parse(line).is_some()
}

/// The start, the line number of a TMPlayer+ line (`,1=`), and the text.
fn parse(line: &str) -> Option<(u64, Option<u32>, &str)> {
    let line = line.trim_start();
    let mut fields = line.splitn(3, ':');
    let hours = fields.next()?;
    let minutes = fields.next()?;
    let rest = fields.next()?;
    let digits = |s: &str, most: usize| {
        !s.is_empty() && s.len() <= most && s.bytes().all(|b| b.is_ascii_digit())
    };
    if !digits(hours, 2) || !digits(minutes, 2) {
        return None;
    }
    let seconds_end = rest.find(|c: char| !c.is_ascii_digit())?;
    let seconds = &rest[..seconds_end];
    if seconds.len() != 2 {
        return None;
    }
    let after = &rest[seconds_end..];
    let (number, text) = if let Some(numbered) = after.strip_prefix(',') {
        let (n, text) = numbered.split_once('=')?;
        (Some(n.trim().parse::<u32>().ok()?), text)
    } else {
        (None, after.strip_prefix([':', '='])?)
    };
    let start = (hours.parse::<u64>().ok()? * 3600
        + minutes.parse::<u64>().ok()? * 60
        + seconds.parse::<u64>().ok()?)
        * 1000;
    Some((start, number, text))
}

/// Reads a TMPlayer's cues.
#[must_use]
pub fn read(text: &str) -> Vec<Cue> {
    // One entry per start: TMPlayer+ gives each line of a cue its own row.
    let mut starts: Vec<(u64, Vec<&str>)> = Vec::new();
    for (start, number, words) in text.lines().filter_map(parse) {
        match starts.last_mut() {
            Some((at, lines)) if *at == start && number.is_some_and(|n| n > 1) => lines.push(words),
            _ => starts.push((start, vec![words])),
        }
    }
    let mut cues = Vec::new();
    for (n, (start_ms, rows)) in starts.iter().enumerate() {
        let lines: Vec<Vec<Span>> = rows
            .iter()
            .flat_map(|r| r.split('|'))
            .map(|l| vec![Span::plain(l)])
            .collect();
        cues.push(Cue {
            start_ms: *start_ms,
            end_ms: cue::untimed_end(*start_ms, starts.get(n + 1).map(|s| s.0)),
            lines,
            band: None,
        });
    }
    cues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tmplayer_lasts_until_the_next_line_and_joins_numbered_lines() {
        let mut cues = read(
            "00:00:01:Hello|world\n00:00:04:\n0:00:05=Equals\n00:00:07,1=First\n00:00:07,2=Second\n00:01:00:Late\n",
        );
        crate::subtitles::cue::tidy(&mut cues);
        assert_eq!(cues.len(), 4, "the empty line only clears");
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 4000));
        assert_eq!(cues[0].text(), "Hello\nworld");
        assert_eq!(cues[1].text(), "Equals");
        assert_eq!(cues[2].text(), "First\nSecond");
        assert_eq!(cues[2].end_ms, 7000 + cue::UNTIMED_MOST_MS);
    }
}
