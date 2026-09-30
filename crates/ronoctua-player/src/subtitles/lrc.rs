//! Timed lyrics (`.lrc`) as subtitles, read by the lyrics reader the player
//! already has ([`crate::lyrics::parse`]): stamps, several stamps on one line
//! and `[offset:]` mean the same here as there.
//!
//! A line lasts until the next one; an empty line is a pause, so it only
//! ends the one before. Lyrics with no stamps at all are not subtitles.

use crate::lyrics;
use crate::subtitles::cue::{self, Cue};

/// Reads timed lyrics as cues; `None` when no line has a time.
#[must_use]
pub fn read(text: &str) -> Option<Vec<Cue>> {
    let lines = lyrics::parse(text);
    let timed: Vec<(u64, &str)> = lines
        .iter()
        .filter_map(|l| Some(((l.at? * 1000.0).round().max(0.0) as u64, l.text.as_str())))
        .collect();
    if timed.is_empty() {
        return None;
    }
    let cues = timed
        .iter()
        .enumerate()
        .filter(|(_, (_, words))| !words.trim().is_empty())
        .map(|(n, &(start_ms, words))| {
            let next = timed[n + 1..].iter().map(|t| t.0).find(|&t| t > start_ms);
            Cue::plain(
                start_ms,
                next.unwrap_or(start_ms + cue::UNTIMED_MOST_MS),
                words,
            )
        })
        .collect();
    Some(cues)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_lasts_until_the_next_and_a_pause_only_ends_it() {
        let cues = read("[ar:Someone]\n[00:01.00]First\n[00:03.50]\n[00:05.00]Second\n")
            .unwrap_or_default();
        assert_eq!(cues.len(), 2);
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 3500));
        assert_eq!(cues[1].start_ms, 5000);
        assert_eq!(read("no stamps at all\n"), None);
    }
}
