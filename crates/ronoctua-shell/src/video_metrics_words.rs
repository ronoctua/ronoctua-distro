//! The words the two video-metrics doors share: which key says each closed
//! set of the catalogue, and how a number of it is written — one place, so
//! the footer and the window cannot say it two ways.
//!
//! # Every closed set has its words, in every language
//!
//! The catalogue answers in closed sets — why it refused ([`Why`]), why a
//! scan stopped ([`Stop`]), why an id lies under a tombstone ([`Skip`]), the
//! four ratings, the kinds of rule — and each member has a key of its own,
//! named here by a `match` the compiler holds total. A member added to the
//! crate is a compile error in this file, never a bracketed key on screen.

use ronoctua_i18n::{Arg, key};
use ronoctua_video_metrics::read::{Hidden, Shorts};
use ronoctua_video_metrics::{Fields, Rating, RuleKind, Skip, Stop, TermMatch, Why};

/// The sentence for a refusal of the catalogue.
#[must_use]
pub const fn why_key(why: Why) -> &'static str {
    match why {
        Why::NotYet => key::VIDEO_METRICS_WHY_NOT_YET,
        Why::NoDatabase => key::VIDEO_METRICS_WHY_NO_DATABASE,
        Why::NotThere => key::VIDEO_METRICS_WHY_NOT_THERE,
        Why::Unreadable => key::VIDEO_METRICS_WHY_UNREADABLE,
        Why::OtherSchema => key::VIDEO_METRICS_WHY_OTHER_SCHEMA,
        Why::Busy => key::VIDEO_METRICS_WHY_BUSY,
        Why::NotFound => key::VIDEO_METRICS_WHY_NOT_FOUND,
        Why::Failed => key::VIDEO_METRICS_WHY_FAILED,
        Why::Invalid => key::VIDEO_METRICS_WHY_INVALID,
    }
}

/// The sentence for why a scan stopped early.
#[must_use]
pub const fn stop_key(stop: Stop) -> &'static str {
    match stop {
        Stop::BotCheck => key::VIDEO_METRICS_STOP_BOT_CHECK,
        Stop::TooManyRequests => key::VIDEO_METRICS_STOP_TOO_MANY_REQUESTS,
        Stop::Offline => key::VIDEO_METRICS_STOP_OFFLINE,
        Stop::NotLet => key::VIDEO_METRICS_STOP_NOT_LET,
    }
}

/// The words for why ids lie under a tombstone.
#[must_use]
pub const fn skip_key(skip: Skip) -> &'static str {
    match skip {
        Skip::BeforeReference => key::VIDEO_METRICS_SKIP_BEFORE_REFERENCE,
        Skip::FirstScan => key::VIDEO_METRICS_SKIP_FIRST_SCAN,
        Skip::BlockedShort => key::VIDEO_METRICS_SKIP_BLOCKED_SHORT,
        Skip::Unavailable => key::VIDEO_METRICS_SKIP_UNAVAILABLE,
        Skip::Deleted => key::VIDEO_METRICS_SKIP_DELETED,
    }
}

/// The words for a rating.
#[must_use]
pub const fn rating_key(rating: Rating) -> &'static str {
    match rating {
        Rating::VeryNegative => key::VIDEO_METRICS_RATING_VERY_NEGATIVE,
        Rating::Negative => key::VIDEO_METRICS_RATING_NEGATIVE,
        Rating::Positive => key::VIDEO_METRICS_RATING_POSITIVE,
        Rating::VeryPositive => key::VIDEO_METRICS_RATING_VERY_POSITIVE,
    }
}

/// The name of a kind of rule — the preferred band's included.
#[must_use]
pub const fn kind_key(kind: &RuleKind) -> &'static str {
    match kind {
        RuleKind::Term { .. } => key::VIDEO_METRICS_RULE_TERM,
        RuleKind::Band { .. } => key::VIDEO_METRICS_RULE_BAND,
        RuleKind::Preferred { .. } => key::VIDEO_METRICS_RULE_PREFERRED,
        RuleKind::Short => key::VIDEO_METRICS_RULE_SHORT,
        RuleKind::Recency { .. } => key::VIDEO_METRICS_RULE_RECENCY,
        RuleKind::Live => key::VIDEO_METRICS_RULE_LIVE,
    }
}

/// The words for how a term matches.
#[must_use]
pub const fn match_key(how: TermMatch) -> &'static str {
    match how {
        TermMatch::Word => key::VIDEO_METRICS_MATCH_WORD,
        TermMatch::Phrase => key::VIDEO_METRICS_MATCH_PHRASE,
        TermMatch::Prefix => key::VIDEO_METRICS_MATCH_PREFIX,
    }
}

/// The five fields a term may read, in the order a rule's page lists them.
pub const FIELDS: [Fields; 5] = [
    Fields::TITLE,
    Fields::DESCRIPTION,
    Fields::TAGS,
    Fields::SECOND_DESCRIPTION,
    Fields::SECOND_TAGS,
];

/// The words for one field a term reads — one bit of [`Fields`]; `None` for
/// anything but one of the five.
#[must_use]
pub const fn field_key(field: Fields) -> Option<&'static str> {
    Some(match field.0 {
        1 => key::VIDEO_METRICS_FIELD_TITLE,
        2 => key::VIDEO_METRICS_FIELD_DESCRIPTION,
        4 => key::VIDEO_METRICS_FIELD_TAGS,
        8 => key::VIDEO_METRICS_FIELD_SECOND_DESCRIPTION,
        16 => key::VIDEO_METRICS_FIELD_SECOND_TAGS,
        _ => return None,
    })
}

/// The words for which Shorts a list shows.
#[must_use]
pub const fn shorts_key(shorts: Shorts) -> &'static str {
    match shorts {
        Shorts::Any => key::VIDEO_METRICS_SHORTS_ANY,
        Shorts::Only => key::VIDEO_METRICS_SHORTS_ONLY,
        Shorts::Without => key::VIDEO_METRICS_SHORTS_WITHOUT,
    }
}

/// The words for which hidden videos a list shows.
#[must_use]
pub const fn hidden_key(hidden: Hidden) -> &'static str {
    match hidden {
        Hidden::Without => key::VIDEO_METRICS_HIDDEN_WITHOUT,
        Hidden::Only => key::VIDEO_METRICS_HIDDEN_ONLY,
        Hidden::Any => key::VIDEO_METRICS_HIDDEN_ANY,
    }
}

/// The words for why the consent to ask YouTube could not be kept.
#[must_use]
pub const fn unkept_key(unkept: ronoctua_consent::Unkept) -> &'static str {
    match unkept {
        ronoctua_consent::Unkept::NoHome => key::VIDEO_METRICS_NOT_LET_NO_HOME,
        ronoctua_consent::Unkept::Unparseable => key::VIDEO_METRICS_NOT_LET_UNPARSEABLE,
        ronoctua_consent::Unkept::Unwritable => key::VIDEO_METRICS_NOT_LET_UNWRITABLE,
    }
}

/// The sentence for why yt-dlp could not say which channel an address is.
///
/// A BOT CHECK OR A 429 IS SAID AS THE SCAN SAYS IT: they are YouTube's
/// answer to this machine, not to the address, and the same words tell the
/// person the same thing whichever door met it.
#[must_use]
pub const fn unresolved_key(
    trouble: ronoctua_ytdlp::Trouble,
    said: ronoctua_ytdlp::youtube::Said,
) -> &'static str {
    use ronoctua_ytdlp::Trouble;
    use ronoctua_ytdlp::youtube::Said;
    match said {
        Said::BotCheck => return key::VIDEO_METRICS_STOP_BOT_CHECK,
        Said::TooManyRequests => return key::VIDEO_METRICS_STOP_TOO_MANY_REQUESTS,
        Said::Gone => return key::VIDEO_METRICS_ADD_GONE,
        Said::Other => {}
    }
    match trouble {
        Trouble::Absent => key::VIDEO_METRICS_ADD_ABSENT,
        Trouble::Unstartable => key::VIDEO_METRICS_ADD_UNSTARTABLE,
        Trouble::Slow => key::VIDEO_METRICS_ADD_SLOW,
        Trouble::Failed => key::VIDEO_METRICS_ADD_FAILED,
        Trouble::Garbled | Trouble::TooLarge => key::VIDEO_METRICS_ADD_GARBLED,
        Trouble::NotAnAddress | Trouble::NotAList | Trouble::NothingToAsk => {
            key::VIDEO_METRICS_ADD_NOT_AN_ADDRESS
        }
    }
}

/// A number of points, in words — a NUMBER to Fluent, so "one point" and
/// "two points" are the language's.
#[must_use]
pub fn points(points: i64, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_args(
        key::VIDEO_METRICS_POINTS_VALUE,
        &[("points", Arg::Number(points))],
    )
}

/// A number of days, in words.
#[must_use]
pub fn days(days: u32, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_args(
        key::VIDEO_METRICS_DAYS_VALUE,
        &[("days", Arg::Number(i64::from(days)))],
    )
}

/// How long something runs, `m:ss` or `h:mm:ss` — the player's clock.
#[must_use]
pub fn length(seconds: u32) -> String {
    crate::player::clock(f64::from(seconds))
}

/// A length band, `from` to `to`, in words.
#[must_use]
pub fn band(min_s: u32, max_s: u32, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_with(
        key::VIDEO_METRICS_BAND_VALUE,
        &[("from", &length(min_s)), ("to", &length(max_s))],
    )
}

/// How long ago `published_s` was, at `now_s`, in whole days — "today",
/// "yesterday", "3 days ago". A moment in the future (a premiere) is today.
#[must_use]
pub fn age(published_s: i64, now_s: i64, texts: &ronoctua_i18n::Texts) -> String {
    let days = (now_s - published_s).max(0) / 86_400;
    texts.t_args(key::VIDEO_METRICS_AGE, &[("days", Arg::Number(days))])
}

/// A length band as a person types it: two durations, `m:ss` or plain
/// minutes, joined by `-`, `–` or a word of the language's (`to`, `a`) —
/// `4-20`, `4:00-20:00`, `0:30 to 3`. Seconds; `None` for anything else, and
/// for a band that ends before it starts.
#[must_use]
pub fn band_of(typed: &str, texts: &ronoctua_i18n::Texts) -> Option<(u32, u32)> {
    let joiner = texts.t(key::VIDEO_METRICS_BAND_JOINER);
    let typed = typed.trim().replace('–', "-");
    let (from, to) = typed
        .split_once('-')
        .or_else(|| typed.split_once(&format!(" {} ", joiner.trim())))?;
    let (from, to) = (duration_of(from)?, duration_of(to)?);
    (from <= to).then_some((from, to))
}

/// One duration as a person types it: `m:ss`, `h:mm:ss`, or whole minutes.
fn duration_of(typed: &str) -> Option<u32> {
    let parts: Vec<&str> = typed.trim().split(':').collect();
    let number = |s: &str| s.trim().parse::<u32>().ok();
    match parts.as_slice() {
        [minutes] => number(minutes)?.checked_mul(60),
        [minutes, seconds] => {
            let s = number(seconds).filter(|s| *s < 60)?;
            number(minutes)?.checked_mul(60)?.checked_add(s)
        }
        [hours, minutes, seconds] => {
            let m = number(minutes).filter(|m| *m < 60)?;
            let s = number(seconds).filter(|s| *s < 60)?;
            number(hours)?
                .checked_mul(3600)?
                .checked_add(m * 60)?
                .checked_add(s)
        }
        _ => None,
    }
}

/// A whole number as a person types it — a sign, a minus sign, spaces and
/// the language's thousands marks left out. `None` for anything else.
#[must_use]
pub fn whole_of(typed: &str) -> Option<i64> {
    let cleaned: String = typed
        .trim()
        .replace('−', "-")
        .chars()
        .filter(|c| !matches!(c, ' ' | '\u{a0}' | '\u{202f}' | '.' | ','))
        .collect();
    cleaned.strip_prefix('+').unwrap_or(&cleaned).parse().ok()
}

/// The tags a person types: separated by commas, trimmed, empty ones left
/// out (the catalogue drops repeats itself).
#[must_use]
pub fn tags_of(typed: &str) -> Vec<String> {
    typed
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    /// FAILS IF: a member of a closed set of the catalogue has no words in a
    /// declared language — the refusals, the stops, the tombstones, the
    /// ratings, the kinds of rule (the preferred band's included), the ways a
    /// term matches, the five fields, the two rings of a list, the consent's
    /// three refusals and every yt-dlp trouble.
    #[test]
    fn every_closed_set_has_words_in_every_language() {
        let mut wanted: Vec<&str> = Vec::new();
        wanted.extend(Why::ALL.map(why_key));
        wanted.extend(
            [
                Stop::BotCheck,
                Stop::TooManyRequests,
                Stop::Offline,
                Stop::NotLet,
            ]
            .map(stop_key),
        );
        wanted.extend(Skip::ALL.map(skip_key));
        wanted.extend(Rating::ALL.map(rating_key));
        for kind in [
            RuleKind::Term {
                fields: Fields::TITLE,
                pattern: String::new(),
                how: TermMatch::Word,
            },
            RuleKind::Band { min_s: 0, max_s: 1 },
            RuleKind::Preferred { min_s: 0, max_s: 1 },
            RuleKind::Short,
            RuleKind::Recency { days: 1 },
            RuleKind::Live,
        ] {
            wanted.push(kind_key(&kind));
        }
        wanted.extend([TermMatch::Word, TermMatch::Phrase, TermMatch::Prefix].map(match_key));
        for field in FIELDS {
            wanted.push(field_key(field).expect("one of the five"));
        }
        assert_eq!(field_key(Fields(3)), None, "two bits are no field");
        wanted.extend([Shorts::Any, Shorts::Only, Shorts::Without].map(shorts_key));
        wanted.extend([Hidden::Without, Hidden::Only, Hidden::Any].map(hidden_key));
        wanted.extend(
            [
                ronoctua_consent::Unkept::NoHome,
                ronoctua_consent::Unkept::Unparseable,
                ronoctua_consent::Unkept::Unwritable,
            ]
            .map(unkept_key),
        );
        for trouble in ronoctua_ytdlp::Trouble::ALL {
            wanted.push(unresolved_key(
                trouble,
                ronoctua_ytdlp::youtube::Said::Other,
            ));
        }
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for key in &wanted {
                assert!(t.has(key), "{language}: `{key}` has no words");
            }
        }
    }

    /// FAILS IF: a band typed the ways a person types one stops being read,
    /// or one that ends before it starts, or is no band, is read as one.
    #[test]
    fn a_band_is_read_the_ways_it_is_typed() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            assert_eq!(band_of("4-20", &t), Some((240, 1200)));
            assert_eq!(band_of("4:00 – 20:00", &t), Some((240, 1200)));
            assert_eq!(band_of("0:30-1:02:03", &t), Some((30, 3723)));
            let joiner = t.t(key::VIDEO_METRICS_BAND_JOINER);
            assert_eq!(
                band_of(&format!("3 {joiner} 5"), &t),
                Some((180, 300)),
                "{language}: the language's own joiner"
            );
            assert_eq!(band_of("20-4", &t), None, "ends before it starts");
            assert_eq!(band_of("4:75-5", &t), None, "no minute has 75 seconds");
            assert_eq!(band_of("long", &t), None);
            assert_eq!(band_of("4", &t), None, "one duration is no band");
        }
    }

    /// FAILS IF: a number typed with a sign, a minus sign or the marks that
    /// group thousands stops being read, or words are read as a number.
    #[test]
    fn a_whole_number_is_read_as_typed() {
        assert_eq!(whole_of("+500"), Some(500));
        assert_eq!(whole_of("−1 000"), Some(-1000));
        assert_eq!(whole_of("-2.000"), Some(-2000));
        assert_eq!(whole_of(" 10,000 "), Some(10_000));
        assert_eq!(whole_of("many"), None);
        assert_eq!(whole_of(""), None);
        assert_eq!(tags_of(" rust, ,  nix ,rust"), ["rust", "nix", "rust"]);
    }

    /// FAILS IF: a count reaches Fluent as text — a plural would then fall
    /// to its default branch without a word — or an age in the future is
    /// read as negative days.
    #[test]
    fn numbers_reach_the_words_as_numbers() {
        let t = texts("en-GB");
        assert_ne!(points(1, &t), points(2, &t).replace('2', "1"));
        assert_eq!(age(100, 100, &t), age(200, 100, &t), "a premiere is today");
        // ONE DAY IS THE LANGUAGE'S OWN WORDS, never the plural's with a 1 in.
        assert_ne!(days(1, &t), days(2, &t).replace('2', "1"));
        assert!(days(30, &t).contains("30"));
        assert_eq!(length(247), "4:07");
        assert_eq!(length(3723), "1:02:03");
    }
}
