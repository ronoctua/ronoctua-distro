//! What a file's name says it is: a title, a year, a season and an episode.
//!
//! Release names are a convention, not a grammar — `The.Matrix.1999.1080p.
//! BluRay.x264-GROUP`, `Show.S01E02.720p.HDTV`, `show_2x05`, `[Group] Title -
//! 12 (1080p) [ABCD1234]`, `Blade Runner 2049 (2017)` — so this reads them
//! the way a person does: the title is what comes before the first thing
//! that is not a title (an episode mark, a resolution, a codec, a group in
//! brackets), and the year is the LAST year-like number before it, so a
//! title that is a number (`1917`, `2001 A Space Odyssey`, `Blade Runner
//! 2049`) keeps it.
//!
//! It is a GUESS, said as one: the suggestion and the subtitle search both
//! let the person search with other words.

use serde::{Deserialize, Serialize};

/// What a file's name seems to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A film: a year or release marks, and no episode.
    Film,
    /// An episode: a season or an episode mark.
    Episode,
    /// Nothing says which.
    Unknown,
}

/// What [`guess`] read out of a name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guess {
    /// The title, as the name spells it (separators made spaces).
    pub title: String,
    /// The year, when one is there.
    pub year: Option<u16>,
    /// The season, when one is there.
    pub season: Option<u16>,
    /// The episode, when one is there.
    pub episode: Option<u16>,
    /// Which kind of thing it seems to be.
    pub kind: Kind,
}

/// Words that are release marks and never a title's.
///
/// Only the unambiguous ones: `Cam`, `Web`, `Limited` and `Complete` are
/// titles too, and a word AFTER the year is dropped anyway (the title ends at
/// the year), so an ambiguous mark would only ever cut a title short.
const MARKS: &[&str] = &[
    "480p", "576p", "720p", "1080p", "1080i", "2160p", "4320p", "4k", "uhd", "bluray", "blu-ray",
    "bdrip", "brrip", "bdremux", "remux", "web-dl", "webdl", "webrip", "web-rip", "hdtv", "pdtv",
    "dvdrip", "dvdscr", "hdrip", "hdcam", "x264", "x265", "h264", "h265", "h.264", "h.265", "hevc",
    "avc", "xvid", "divx", "av1", "vp9", "aac", "aac2.0", "ac3", "eac3", "dts", "dts-hd", "truehd",
    "atmos", "ddp", "ddp5.1", "dd5.1", "flac", "hdr", "hdr10", "10bit", "8bit", "repack",
];

/// One word of a name, read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Word {
    /// A title's word.
    Plain,
    /// A year-like number (1900–2099), bare or in parentheses.
    Year(u16),
    /// `S01E02`, `1x02`, `S01`, `E05` — where the title ends.
    Mark(Option<u16>, Option<u16>),
    /// `Season` or `Episode`, whose number is the next word.
    Named(bool),
    /// A lone `-` — an episode's number may follow (`Title - 12`).
    Dash,
    /// A release mark, or a group in parentheses: where the title ends.
    Stop,
}

/// What a name says it is. Pure.
#[must_use]
pub fn guess(stem: &str) -> Guess {
    let words = words_of(stem);
    let read: Vec<Word> = words.iter().map(String::as_str).map(word).collect();

    let mut season = None;
    let mut episode = None;
    // WHERE THE TITLE ENDS: the first word that is not a title's.
    let mut end = words.len();
    for (i, w) in read.iter().enumerate() {
        let number_after = words.get(i + 1).map(String::as_str).and_then(number);
        let found = match *w {
            Word::Mark(s, e) => {
                season = season.or(s);
                episode = episode.or(e);
                true
            }
            Word::Named(true) if number_after.is_some() => {
                season = season.or(number_after);
                true
            }
            Word::Named(false) if number_after.is_some() => {
                episode = episode.or(number_after);
                true
            }
            // `Title - 12`: a dash and then a number that is not a year.
            Word::Dash
                if number_after.is_some() && !matches!(read.get(i + 1), Some(Word::Year(_))) =>
            {
                episode = episode.or(number_after);
                true
            }
            // A mark cannot be the whole title: a name that starts with one
            // is read from there.
            Word::Stop => i > 0,
            _ => false,
        };
        if found && end == words.len() {
            end = i;
        }
    }
    // The year is the last year-like word of the title's part — unless it is
    // the FIRST word, which makes it the title (`1917.1080p`).
    let year_at = read[..end]
        .iter()
        .rposition(|w| matches!(w, Word::Year(_)))
        .filter(|&i| i > 0);
    let year = year_at.and_then(|i| match read[i] {
        Word::Year(y) => Some(y),
        _ => None,
    });
    let title_end = year_at.unwrap_or(end);
    let title = tidy(&words[..title_end]);

    let marked = end < words.len() || year.is_some();
    let kind = if season.is_some() || episode.is_some() {
        Kind::Episode
    } else if marked {
        Kind::Film
    } else {
        Kind::Unknown
    };
    Guess {
        title,
        year,
        season,
        episode,
        kind,
    }
}

/// The words of a stem: groups in square brackets dropped (a release group,
/// a checksum, a resolution), `_` always a space, and `.` a space too when the
/// name has no space of its own (`The.Matrix.1999` — but `Mr. Robot` keeps
/// its dot).
fn words_of(stem: &str) -> Vec<String> {
    let mut clean = String::with_capacity(stem.len());
    let mut depth = 0usize;
    for c in stem.chars() {
        match c {
            '[' | '{' => {
                depth += 1;
                clean.push(' ');
            }
            ']' | '}' => {
                depth = depth.saturating_sub(1);
                clean.push(' ');
            }
            _ if depth > 0 => {}
            '_' => clean.push(' '),
            _ => clean.push(c),
        }
    }
    let dotted = !clean.trim().contains(' ');
    let mut words: Vec<String> = Vec::new();
    for piece in clean.split(' ').filter(|p| !p.is_empty()) {
        if dotted {
            words.extend(
                piece
                    .split('.')
                    .filter(|p| !p.is_empty())
                    .map(str::to_owned),
            );
        } else {
            words.push(piece.to_owned());
        }
    }
    words
}

/// What one word is.
fn word(w: &str) -> Word {
    if w == "-" || w == "–" {
        return Word::Dash;
    }
    let lower = w.to_lowercase();
    // PARENTHESES: a year, or the end of the title (`(1080p)`, `(Director's
    // Cut)`).
    if let Some(inner) = lower.strip_prefix('(') {
        let inner = inner.trim_end_matches(')');
        return year(inner).map_or(Word::Stop, Word::Year);
    }
    let bare = lower.trim_matches(|c: char| matches!(c, ',' | ';' | ':' | '!' | ')'));
    if let Some(y) = year(bare) {
        return Word::Year(y);
    }
    if let Some((s, e)) = episode_mark(bare) {
        return Word::Mark(s, e);
    }
    match bare {
        "season" | "temporada" | "saison" | "staffel" => return Word::Named(true),
        "episode" | "ep" | "episodio" | "episódio" | "épisode" | "folge" => {
            return Word::Named(false);
        }
        _ => {}
    }
    // `x264-GROUP` is a mark with the group glued on.
    let head = bare.split('-').next().unwrap_or(bare);
    if MARKS.contains(&bare) || MARKS.contains(&head) {
        return Word::Stop;
    }
    Word::Plain
}

/// A year-like number: four digits, 1900–2099.
fn year(w: &str) -> Option<u16> {
    (w.len() == 4 && w.bytes().all(|b| b.is_ascii_digit()))
        .then(|| w.parse::<u16>().ok())
        .flatten()
        .filter(|y| (1900..=2099).contains(y))
}

/// A number of one to four digits, maybe with a version (`12v2`) — an
/// episode's or a season's.
fn number(w: &str) -> Option<u16> {
    let digits: &str = w
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap_or_default();
    let rest = &w[digits.len()..];
    let versioned = rest.is_empty()
        || (rest.starts_with(['v', 'V']) && rest[1..].bytes().all(|b| b.is_ascii_digit()));
    ((1..=4).contains(&digits.len()) && versioned)
        .then(|| digits.parse().ok())
        .flatten()
}

/// `s01e02` (and `s01e02e03`, `s01e02-e03`), `1x02`, `s01`, `e05`, `ep05`:
/// the season and the episode. `1920x1080` is not one: a season has at most
/// four digits and an episode at most four.
fn episode_mark(w: &str) -> Option<(Option<u16>, Option<u16>)> {
    let digits = |s: &str| -> (usize, Option<u16>) {
        let n = s.bytes().take_while(u8::is_ascii_digit).count();
        let value = (1..=4).contains(&n).then(|| s[..n].parse().ok()).flatten();
        (n, value)
    };
    if let Some(rest) = w.strip_prefix('s') {
        let (n, season) = digits(rest);
        season?;
        let after = &rest[n..];
        if after.is_empty() {
            return Some((season, None));
        }
        let after = after.strip_prefix('e')?;
        let (_, episode) = digits(after);
        return Some((season, Some(episode?)));
    }
    if let Some(rest) = w.strip_prefix("ep").or_else(|| w.strip_prefix('e')) {
        let (n, episode) = digits(rest);
        return (n == rest.len()).then_some((None, Some(episode?)));
    }
    let (n, season) = digits(w);
    let rest = w[n..].strip_prefix('x')?;
    let (m, episode) = digits(rest);
    (m == rest.len() && n <= 2).then_some((Some(season?), Some(episode?)))
}

/// Words joined with spaces, with a trailing dash or bracket left by a cut
/// taken off.
fn tidy(words: &[String]) -> String {
    let joined = words.join(" ");
    joined
        .trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '–' | '(' | ','))
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(stem: &str) -> (String, Option<u16>, Option<u16>, Option<u16>, Kind) {
        let x = guess(stem);
        (x.title, x.year, x.season, x.episode, x.kind)
    }

    #[test]
    fn a_film_release_name_gives_its_title_and_year() {
        assert_eq!(
            g("The.Matrix.1999.1080p.BluRay.x264-GROUP"),
            ("The Matrix".into(), Some(1999), None, None, Kind::Film)
        );
        assert_eq!(
            g("Movie.Name.2010.720p.WEB-DL.DD5.1.H264-FGT"),
            ("Movie Name".into(), Some(2010), None, None, Kind::Film)
        );
        assert_eq!(
            g("Movie Name (2010) [1080p]"),
            ("Movie Name".into(), Some(2010), None, None, Kind::Film)
        );
        assert_eq!(
            g("Amélie_2001_DVDRip"),
            ("Amélie".into(), Some(2001), None, None, Kind::Film)
        );
    }

    #[test]
    fn a_title_that_is_a_number_keeps_it() {
        assert_eq!(
            g("Blade Runner 2049 (2017)"),
            (
                "Blade Runner 2049".into(),
                Some(2017),
                None,
                None,
                Kind::Film
            )
        );
        assert_eq!(
            g("2001.A.Space.Odyssey.1968.1080p"),
            (
                "2001 A Space Odyssey".into(),
                Some(1968),
                None,
                None,
                Kind::Film
            )
        );
        assert_eq!(
            g("1917.1080p.WEBRip"),
            ("1917".into(), None, None, None, Kind::Film)
        );
        assert_eq!(
            g("1917.2019.1080p"),
            ("1917".into(), Some(2019), None, None, Kind::Film)
        );
    }

    #[test]
    fn an_episode_gives_its_show_season_and_number() {
        assert_eq!(
            g("Breaking.Bad.S01E02.720p.HDTV.x264"),
            ("Breaking Bad".into(), None, Some(1), Some(2), Kind::Episode)
        );
        assert_eq!(
            g("breaking.bad.s05e14.multi"),
            (
                "breaking bad".into(),
                None,
                Some(5),
                Some(14),
                Kind::Episode
            )
        );
        assert_eq!(
            g("the_office_2x05"),
            ("the office".into(), None, Some(2), Some(5), Kind::Episode)
        );
        assert_eq!(
            g("Mr. Robot S02E01 eps2.0"),
            ("Mr. Robot".into(), None, Some(2), Some(1), Kind::Episode)
        );
        assert_eq!(
            g("Game of Thrones - Season 3 Episode 9"),
            (
                "Game of Thrones".into(),
                None,
                Some(3),
                Some(9),
                Kind::Episode
            )
        );
        assert_eq!(
            g("Show.Name.2019.S01E01.1080p"),
            (
                "Show Name".into(),
                Some(2019),
                Some(1),
                Some(1),
                Kind::Episode
            )
        );
        assert_eq!(
            g("Show.S02.Complete.1080p"),
            ("Show".into(), None, Some(2), None, Kind::Episode)
        );
        assert_eq!(
            g("Show S01E01E02"),
            ("Show".into(), None, Some(1), Some(1), Kind::Episode)
        );
    }

    #[test]
    fn an_anime_release_gives_its_episode_and_drops_the_brackets() {
        assert_eq!(
            g("[SubsPlease] Frieren Beyond Journeys End - 12 (1080p) [ABCD1234]"),
            (
                "Frieren Beyond Journeys End".into(),
                None,
                None,
                Some(12),
                Kind::Episode
            )
        );
        assert_eq!(
            g("[Group] Title - 03v2 [720p]"),
            ("Title".into(), None, None, Some(3), Kind::Episode)
        );
    }

    #[test]
    fn a_title_word_that_looks_like_a_mark_is_kept() {
        assert_eq!(
            g("Cam.2018.1080p.WEB-DL"),
            ("Cam".into(), Some(2018), None, None, Kind::Film)
        );
        assert_eq!(
            g("The.Complete.Works.2010.EXTENDED.1080p"),
            (
                "The Complete Works".into(),
                Some(2010),
                None,
                None,
                Kind::Film
            )
        );
        assert_eq!(
            g("Épisode 4"),
            (String::new(), None, None, Some(4), Kind::Episode),
            "a word read in any case"
        );
    }

    #[test]
    fn a_name_with_nothing_to_read_is_its_own_title() {
        assert_eq!(
            g("Artist - Song Title"),
            (
                "Artist - Song Title".into(),
                None,
                None,
                None,
                Kind::Unknown
            )
        );
        assert_eq!(
            g("holiday video"),
            ("holiday video".into(), None, None, None, Kind::Unknown)
        );
        assert_eq!(g(""), (String::new(), None, None, None, Kind::Unknown));
        // A screen size is not an episode.
        assert_eq!(
            g("clip 1920x1080"),
            ("clip 1920x1080".into(), None, None, None, Kind::Unknown)
        );
    }
}
