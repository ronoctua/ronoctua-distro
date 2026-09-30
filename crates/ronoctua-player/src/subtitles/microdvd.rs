//! MicroDVD (`.sub`, `.txt`): frames, not times.
//!
//! ```text
//! {1}{1}23.976
//! {25}{50}{y:i}Hello|world
//! ```
//!
//! A cue is `{first frame}{last frame}text`, with `|` between lines. The
//! frame rate is the file's own when its first cue says it (`{1}{1}23.976`),
//! else the picture's, which the engine knows, else 23.976 — the rate most
//! MicroDVD files were made for. Control codes in braces set the look: `y:`
//! for one line, `Y:` for the whole cue (`i`, `b`, `u`), `c:`/`C:` a colour
//! written `$BBGGRR`.

use crate::subtitles::FrameRate;
use crate::subtitles::cue::{self, Cue, Rgb, Span};

/// Whether `line` is a MicroDVD cue (`{n}{m}` or `{n}{}`).
pub(crate) fn is_line(line: &str) -> bool {
    frames(line).is_some()
}

/// The two frames and the rest of a line; the end is `None` when empty.
fn frames(line: &str) -> Option<(u64, Option<u64>, &str)> {
    let rest = line.trim_start().strip_prefix('{')?;
    let (first, rest) = rest.split_once('}')?;
    let rest = rest.strip_prefix('{')?;
    let (last, text) = rest.split_once('}')?;
    let number = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    let first = number(first)?;
    let last = if last.is_empty() {
        None
    } else {
        Some(number(last)?)
    };
    Some((first, last, text))
}

/// Reads a MicroDVD's cues, at `rate` unless the file says its own.
#[must_use]
pub fn read(text: &str, rate: Option<FrameRate>) -> Vec<Cue> {
    let parsed: Vec<(u64, Option<u64>, &str)> = text.lines().filter_map(frames).collect();
    let mut rate = rate.unwrap_or(FrameRate::FILM);
    let mut body = parsed.as_slice();
    // `{1}{1}23.976`: the file's own rate, not a cue.
    if let Some(&(first, Some(last), words)) = parsed.first()
        && first <= 1
        && last <= 1
        && let Some(own) = FrameRate::decimal(words.trim())
    {
        rate = own;
        body = &parsed[1..];
    }
    let starts: Vec<u64> = body
        .iter()
        .map(|(f, _, _)| cue::frames_ms(*f, rate))
        .collect();
    body.iter()
        .enumerate()
        .map(|(n, &(_, last, words))| {
            let start_ms = starts[n];
            let end_ms = match last {
                Some(l) => cue::frames_ms(l, rate),
                None => cue::untimed_end(start_ms, starts.get(n + 1).copied()),
            };
            Cue {
                start_ms,
                end_ms,
                lines: lines_of(words),
                band: None,
            }
        })
        .collect()
}

#[derive(Clone, Copy, Default)]
struct Look {
    italic: bool,
    bold: bool,
    underline: bool,
    colour: Option<Rgb>,
}

fn lines_of(words: &str) -> Vec<Vec<Span>> {
    let mut whole = Look::default();
    let mut lines = Vec::new();
    for part in words.split('|') {
        let mut line_look = whole;
        let mut rest = part;
        // Control codes lead a line: `{y:i}{c:$0000FF}words`.
        while let Some(inner) = rest.strip_prefix('{') {
            let Some((code, after)) = inner.split_once('}') else {
                break;
            };
            let Some((key, value)) = code.split_once(':') else {
                break;
            };
            let all = key.starts_with(|c: char| c.is_ascii_uppercase());
            let apply = |look: &mut Look| match key.to_ascii_lowercase().as_str() {
                "y" => {
                    let value = value.to_ascii_lowercase();
                    look.italic |= value.contains('i');
                    look.bold |= value.contains('b');
                    look.underline |= value.contains('u');
                }
                "c" => {
                    look.colour = Rgb::ass(&format!("&H{}", value.trim().trim_start_matches('$')))
                }
                _ => {}
            };
            apply(&mut line_look);
            if all {
                apply(&mut whole);
            }
            rest = after;
        }
        // An MPL2-style leading `/` is read as italics by most players too.
        let (italic, rest) = match rest.strip_prefix('/') {
            Some(r) => (true, r),
            None => (false, rest),
        };
        lines.push(vec![Span {
            text: rest.to_owned(),
            italic: line_look.italic || italic,
            bold: line_look.bold,
            underline: line_look.underline,
            colour: line_look.colour,
        }]);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_become_times_at_the_files_own_rate() {
        let cues = read(
            "{1}{1}25\n{25}{50}{Y:i}Hello|world\n{75}{}{c:$0000FF}Red\n{300}{325}Last\n",
            None,
        );
        assert_eq!(cues.len(), 3);
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 2000));
        assert!(
            cues[0].lines.iter().all(|l| l[0].italic),
            "Y: covers every line"
        );
        assert_eq!(cues[1].start_ms, 3000);
        assert_eq!(
            cues[1].end_ms,
            3000 + cue::UNTIMED_MOST_MS,
            "an empty end: until the next, at most"
        );
        assert_eq!(cues[1].lines[0][0].colour, Some(Rgb::new(255, 0, 0)));
    }

    #[test]
    fn without_its_own_rate_the_pictures_is_used() {
        let cues = read(
            "{24}{48}One\n",
            Some(FrameRate {
                numerator: 24,
                denominator: 1,
            }),
        );
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1000, 2000));
        let film = read("{24}{48}One\n", None);
        assert_eq!(film[0].start_ms, 1001);
    }
}
