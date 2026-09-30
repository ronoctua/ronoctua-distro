//! SubRip (`.srt`): the common case, and the one written most carelessly.
//!
//! ```text
//! 1
//! 00:00:01,000 --> 00:00:02,500
//! <i>First</i> line
//! second line
//! ```
//!
//! Read forgivingly: the index may be missing or wrong, the fraction may be
//! written with a dot, the hours may be missing, and a file that forgot the
//! blank line between two cues still has two cues — the next cue's number,
//! just above its timing line, ends the text of the one before.

use crate::subtitles::cue::{self, Cue, Dialect};

/// Whether `line` is an SRT timing line (`start --> end`, anything after).
pub(crate) fn is_timing(line: &str) -> bool {
    timing(line).is_some()
}

/// The start and end of a timing line, in milliseconds.
pub(crate) fn timing(line: &str) -> Option<(u64, u64)> {
    let (start, rest) = line.split_once("-->")?;
    let end = rest.split_whitespace().next()?;
    let start = cue::clock_ms(start)?;
    let end = cue::clock_ms(end)?;
    Some((start, end))
}

/// Reads an SRT's cues.
#[must_use]
pub fn read(text: &str) -> Vec<Cue> {
    let lines: Vec<&str> = text.lines().collect();
    let timings: Vec<usize> = (0..lines.len()).filter(|&i| is_timing(lines[i])).collect();
    let mut cues = Vec::with_capacity(timings.len());
    for (n, &at) in timings.iter().enumerate() {
        let Some((start_ms, end_ms)) = timing(lines[at]) else {
            continue;
        };
        let next = timings.get(n + 1).copied().unwrap_or(lines.len());
        let mut last = next;
        // The next cue's number, right above its timing line, is not text.
        if next < lines.len() && next > at + 1 && is_index(lines[next - 1]) {
            last = next - 1;
        }
        // A blank line ends the text.
        let body_end = (at + 1..last)
            .find(|&i| lines[i].trim().is_empty())
            .unwrap_or(last);
        let body = lines[at + 1..body_end].join("\n");
        let (text_lines, band) = cue::markup(&body, Dialect::SubRip);
        cues.push(Cue {
            start_ms,
            end_ms,
            lines: text_lines,
            band,
        });
    }
    cues
}

fn is_index(line: &str) -> bool {
    let line = line.trim();
    !line.is_empty() && line.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitles::{self, Band, Format, Opened};

    #[test]
    fn a_windows_1252_srt_is_read_with_its_looks() {
        let bytes = b"1\r\n00:00:01,000 --> 00:00:02,500\r\nOl\xE1, <i>voc\xEA</i>!\r\nN\xE3o \xE9 f\xE1cil.\r\n\r\n\
2\r\n00:00:03,000 --> 00:00:04,000\r\n{\\an8}Cora\xE7\xE3o\r\n\r\n";
        let Ok(Opened::Ours(track)) = subtitles::open(bytes, Some("srt"), None) else {
            panic!("not read");
        };
        assert_eq!(track.format, Format::SubRip);
        assert_eq!(track.charset.map(|c| c.name), Some("windows-1252"));
        assert_eq!(track.cues.len(), 2);
        let first = &track.cues[0];
        assert_eq!((first.start_ms, first.end_ms), (1000, 2500));
        assert_eq!(first.text(), "Olá, você!\nNão é fácil.");
        assert!(first.lines[0].iter().any(|s| s.italic && s.text == "você"));
        assert_eq!(track.cues[1].band, Some(Band::Top));
        // pt-BR: the file's own words, under test.
        assert_eq!(track.cues[1].text(), "Coração");
    }

    #[test]
    fn a_missing_blank_line_and_dotted_times_still_make_two_cues() {
        let cues = read("1\n00:00:01.000 --> 00:00:02.000\nOne\n2\n00:02.000 --> 00:03.000\nTwo\n");
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text(), "One");
        assert_eq!((cues[1].start_ms, cues[1].end_ms), (2000, 3000));
    }
}
