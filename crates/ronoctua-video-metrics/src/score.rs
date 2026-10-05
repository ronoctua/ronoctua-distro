//! The score's arithmetic — pure, so every rule is a test.
//!
//! `total = channel + static points + recency`. The static points are summed
//! once per change ([`static_points`]): the duration band the video falls in,
//! the Short penalty, the preferred band, the term rules — global AND the
//! channel's, each once per video — and the rating's weight. Recency is the
//! one term that moves with the clock, and SQL computes it at read time with
//! the same formula as [`recency`], from the R and D [`in_force`] resolved for
//! the channel (and the recompute stored on its row).
//!
//! # A channel's version replaces the global one
//!
//! Every rule has a channel version. For the bands, the channel's list
//! replaces the global list when it has at least one enabled band (a channel
//! whose bands are all switched off has the global list again); the TERMS add
//! up — a global term and a channel's term both count, and one switched off
//! counts nowhere. For the Short penalty, the preferred band, being live and
//! recency, the channel's rule replaces the global one WHETHER IT IS ON OR
//! OFF: switched off on a channel's page, the kind is off for that channel —
//! which is what the person switched — and the global one comes back only
//! when the channel's version is removed.
//!
//! # Terms
//!
//! Accents and case are folded away on both sides ([`fold`]), and matching is
//! by WORD ([`words_of`]): `cat` does not match `category`, unless the rule
//! says prefix. A phrase is its words in order inside ONE field — the end of
//! the title and the start of the description are not a phrase. A rule counts
//! once per video, however many fields or times it matches.

use crate::{ChannelId, Fields, Rating, Rule, RuleKind, TermMatch, Weights};

/// Recency: `floor(R · max(0, D − age) / D)` — `most_points` (R) for a video
/// published today, fading to 0 at `days` (D). A `days` of 0 gives 0.
#[must_use]
pub fn recency(most_points: i32, days: u32, age_days: u32) -> i32 {
    if days == 0 {
        return 0;
    }
    let left = i64::from(days.saturating_sub(age_days));
    let points = (i64::from(most_points) * left).div_euclid(i64::from(days));
    i32::try_from(points).unwrap_or(most_points)
}

/// Text with its accents and case folded away: `Configurações` and
/// `CONFIGURACOES` read the same. The tree's one accent folding
/// (`ronoctua_apps::fold::fold`), then Unicode lowercase for what it leaves (a
/// Cyrillic or Greek capital).
#[must_use]
pub fn fold(text: &str) -> String {
    ronoctua_apps::fold::fold(text).to_lowercase()
}

/// The folded words of `text`: runs of letters and digits, everything else a
/// boundary.
#[must_use]
pub fn words_of(text: &str) -> Vec<String> {
    fold(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Whether the pattern's folded `words` are found in a field's folded
/// `text_words`, the way `how` says. Empty words never match.
#[must_use]
pub fn term_matches(words: &[String], how: TermMatch, text_words: &[String]) -> bool {
    let Some((last, before)) = words.split_last() else {
        return false;
    };
    if text_words.len() < words.len() {
        return false;
    }
    text_words.windows(words.len()).any(|window| {
        let (seen_last, seen_before) = window.split_last().unwrap_or((&window[0], &[]));
        let head = seen_before == before;
        head && match how {
            TermMatch::Word | TermMatch::Phrase => seen_last == last,
            TermMatch::Prefix => seen_last.starts_with(last.as_str()),
        }
    })
}

/// What the score reads of one video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts<'a> {
    /// Its duration, when known.
    pub duration_s: Option<u32>,
    /// Whether it is a Short.
    pub is_short: bool,
    /// Whether it is (or was) live, or a premiere.
    pub live: bool,
    /// Its title.
    pub title: &'a str,
    /// YouTube's description.
    pub description: &'a str,
    /// YouTube's tags.
    pub tags: &'a [String],
    /// The person's second description.
    pub second_description: &'a str,
    /// The person's second tags.
    pub second_tags: &'a [String],
    /// The person's rating.
    pub rating: Option<Rating>,
}

/// Whether yt-dlp's `live_status` counts as live for the Live rule: live
/// now, a recording of a live, a premiere to come or just ended.
#[must_use]
pub fn is_live(live_status: Option<&str>) -> bool {
    matches!(
        live_status,
        Some("is_live" | "was_live" | "is_upcoming" | "post_live")
    )
}

/// A duration band and its points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Band {
    /// The shortest duration in the band, seconds.
    pub min_s: u32,
    /// The longest, seconds.
    pub max_s: u32,
    /// The points a video in it gets.
    pub points: i32,
}

impl Band {
    /// Whether `duration_s` is in the band (both ends included).
    #[must_use]
    pub const fn holds(&self, duration_s: u32) -> bool {
        self.min_s <= duration_s && duration_s <= self.max_s
    }
}

/// A term rule, its pattern already in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    /// The fields it reads.
    pub fields: Fields,
    /// The pattern's folded words.
    pub words: Vec<String>,
    /// How it matches.
    pub how: TermMatch,
    /// Its points.
    pub points: i32,
}

/// The rules in force for one channel (or the global ones), resolved.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InForce {
    /// The duration bands, by their start: a video gets the FIRST that holds
    /// its duration.
    pub bands: Vec<Band>,
    /// The preferred band, on top of the bands.
    pub preferred: Option<Band>,
    /// The points a Short gets (a penalty is negative).
    pub short: i32,
    /// The points a live or a premiere gets.
    pub live: i32,
    /// Every term rule that counts: the global ones and the channel's.
    pub terms: Vec<Term>,
    /// R: recency's points for a video published today; never negative.
    pub recency_points: i32,
    /// D: the days over which it fades.
    pub recency_days: u32,
}

/// The rules in force for `channel` (`None`: the global ones) out of every
/// rule of the catalogue. Pure.
#[must_use]
pub fn in_force(rules: &[Rule], channel: Option<&ChannelId>) -> InForce {
    // BY ID: `Person::put_rule` keeps one single rule per scope and kind, and
    // should two ever be there, the first kept is the one in force.
    let mut every: Vec<&Rule> = rules.iter().collect();
    every.sort_by_key(|r| r.id.map_or(i64::MAX, |id| id.0));
    let sorted: Vec<&Rule> = every.iter().copied().filter(|r| r.enabled).collect();

    let bands_of = |mine: &dyn Fn(&Rule) -> bool| -> Vec<Band> {
        sorted
            .iter()
            .filter(|r| matches!(r.kind, RuleKind::Band { .. }) && mine(r))
            .filter_map(|r| band_of(r))
            .collect()
    };
    let mut bands = bands_of(&|r| is_own(r, channel));
    if bands.is_empty() {
        bands = bands_of(&|r| r.channel.is_none());
    }
    bands.sort_by_key(|b| (b.min_s, b.max_s));

    let terms = sorted
        .iter()
        .filter(|r| is_own(r, channel) || r.channel.is_none())
        .filter_map(|r| match &r.kind {
            RuleKind::Term {
                fields,
                pattern,
                how,
            } => Some(Term {
                fields: *fields,
                words: words_of(pattern),
                how: *how,
                points: r.points,
            }),
            _ => None,
        })
        .filter(|t| !t.words.is_empty())
        .collect();

    let (recency_points, recency_days) =
        match single(&every, channel, |k| matches!(k, RuleKind::Recency { .. })) {
            Some(Rule {
                kind: RuleKind::Recency { days },
                points,
                ..
            }) => ((*points).max(0), *days),
            _ => (0, 0),
        };
    InForce {
        bands,
        preferred: single(&every, channel, |k| matches!(k, RuleKind::Preferred { .. }))
            .and_then(band_of),
        short: single(&every, channel, |k| matches!(k, RuleKind::Short)).map_or(0, |r| r.points),
        live: single(&every, channel, |k| matches!(k, RuleKind::Live)).map_or(0, |r| r.points),
        terms,
        recency_points,
        recency_days,
    }
}

/// Whether `rule` is `channel`'s own version.
fn is_own(rule: &Rule, channel: Option<&ChannelId>) -> bool {
    channel.is_some() && rule.channel.as_ref() == channel
}

/// The rule of a kind in force for `channel`, out of `every` rule (on or
/// off): the channel's own version when it has one — in force only when it
/// is on, for a version switched off is the kind switched off for that
/// channel — else the global one, when that is on.
fn single<'a>(
    every: &[&'a Rule],
    channel: Option<&ChannelId>,
    kind: fn(&RuleKind) -> bool,
) -> Option<&'a Rule> {
    let of_kind = || every.iter().copied().filter(move |r| kind(&r.kind));
    match of_kind().find(|r| is_own(r, channel)) {
        Some(own) => own.enabled.then_some(own),
        None => of_kind().find(|r| r.channel.is_none() && r.enabled),
    }
}

/// A band rule (or the preferred one) as a band, its ends in order.
fn band_of(rule: &Rule) -> Option<Band> {
    match rule.kind {
        RuleKind::Band { min_s, max_s } | RuleKind::Preferred { min_s, max_s } => Some(Band {
            min_s: min_s.min(max_s),
            max_s: max_s.max(min_s),
            points: rule.points,
        }),
        _ => None,
    }
}

/// The folded words of every field a video has, each field on its own.
struct Read {
    title: Vec<String>,
    description: Vec<String>,
    tags: Vec<Vec<String>>,
    second_description: Vec<String>,
    second_tags: Vec<Vec<String>>,
}

impl Read {
    fn of(facts: &Facts<'_>) -> Self {
        Self {
            title: words_of(facts.title),
            description: words_of(facts.description),
            tags: facts.tags.iter().map(|t| words_of(t)).collect(),
            second_description: words_of(facts.second_description),
            second_tags: facts.second_tags.iter().map(|t| words_of(t)).collect(),
        }
    }

    /// Whether `term` matches any field it reads.
    fn matches(&self, term: &Term) -> bool {
        let one = |words: &[String]| term_matches(&term.words, term.how, words);
        (term.fields.has(Fields::TITLE) && one(&self.title))
            || (term.fields.has(Fields::DESCRIPTION) && one(&self.description))
            || (term.fields.has(Fields::TAGS) && self.tags.iter().any(|t| one(t)))
            || (term.fields.has(Fields::SECOND_DESCRIPTION) && one(&self.second_description))
            || (term.fields.has(Fields::SECOND_TAGS) && self.second_tags.iter().any(|t| one(t)))
    }
}

/// A video's static points under the rules in force for its channel. Pure.
#[must_use]
pub fn static_points(rules: &InForce, weights: &Weights, facts: &Facts<'_>) -> i64 {
    let mut points = 0_i64;
    if let Some(duration) = facts.duration_s {
        if let Some(band) = rules.bands.iter().find(|b| b.holds(duration)) {
            points += i64::from(band.points);
        }
        if let Some(preferred) = rules.preferred.filter(|b| b.holds(duration)) {
            points += i64::from(preferred.points);
        }
    }
    if facts.is_short {
        points += i64::from(rules.short);
    }
    if facts.live {
        points += i64::from(rules.live);
    }
    if !rules.terms.is_empty() {
        let read = Read::of(facts);
        for term in rules.terms.iter().filter(|t| read.matches(t)) {
            points += i64::from(term.points);
        }
    }
    if let Some(rating) = facts.rating {
        points += i64::from(weights.of(rating));
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuleId;

    fn rule(id: i64, channel: Option<&str>, kind: RuleKind, points: i32) -> Rule {
        Rule {
            id: Some(RuleId(id)),
            channel: channel.map(|c| ChannelId(c.to_owned())),
            kind,
            points,
            enabled: true,
        }
    }

    fn term(pattern: &str, how: TermMatch, fields: Fields) -> RuleKind {
        RuleKind::Term {
            fields,
            pattern: pattern.to_owned(),
            how,
        }
    }

    const NOTHING: [String; 0] = [];

    fn facts<'a>(title: &'a str, duration_s: Option<u32>) -> Facts<'a> {
        Facts {
            duration_s,
            is_short: false,
            live: false,
            title,
            description: "",
            tags: &NOTHING,
            second_description: "",
            second_tags: &NOTHING,
            rating: None,
        }
    }

    #[test]
    fn recency_fades_by_the_day_and_floors() {
        assert_eq!(recency(1000, 10, 0), 1000, "published today");
        assert_eq!(recency(1000, 10, 3), 700);
        assert_eq!(recency(1000, 3, 1), 666, "floored");
        assert_eq!(recency(1000, 10, 10), 0);
        assert_eq!(recency(1000, 10, 400), 0, "never negative");
        assert_eq!(recency(1000, 0, 0), 0);
    }

    #[test]
    fn a_video_gets_the_first_band_that_holds_it_and_the_channel_list_replaces_the_global() {
        let rules = [
            rule(
                1,
                None,
                RuleKind::Band {
                    min_s: 0,
                    max_s: 59,
                },
                -300,
            ),
            rule(
                2,
                None,
                RuleKind::Band {
                    min_s: 60,
                    max_s: 1199,
                },
                100,
            ),
            rule(
                3,
                None,
                RuleKind::Band {
                    min_s: 600,
                    max_s: 3600,
                },
                400,
            ),
            rule(
                4,
                Some("UCa"),
                RuleKind::Band {
                    min_s: 1200,
                    max_s: 99_999,
                },
                900,
            ),
        ];
        let weights = Weights::default();
        let global = in_force(&rules, None);
        assert_eq!(static_points(&global, &weights, &facts("", Some(30))), -300);
        assert_eq!(
            static_points(&global, &weights, &facts("", Some(700))),
            100,
            "overlapping bands: the first by its start, once"
        );
        assert_eq!(
            static_points(&global, &weights, &facts("", Some(2000))),
            400
        );
        assert_eq!(
            static_points(&global, &weights, &facts("", None)),
            0,
            "no duration, no band"
        );
        let a = in_force(&rules, Some(&ChannelId("UCa".to_owned())));
        assert_eq!(
            a.bands.len(),
            1,
            "the channel's list replaces the global one"
        );
        assert_eq!(static_points(&a, &weights, &facts("", Some(30))), 0);
        assert_eq!(static_points(&a, &weights, &facts("", Some(2000))), 900);
        // A DISABLED channel list is no list: the global one is back.
        let mut off = rules;
        off[3].enabled = false;
        let a = in_force(&off, Some(&ChannelId("UCa".to_owned())));
        assert_eq!(static_points(&a, &weights, &facts("", Some(30))), -300);
    }

    #[test]
    fn a_channel_version_replaces_each_single_rule() {
        let rules = [
            rule(1, None, RuleKind::Short, -1000),
            rule(2, None, RuleKind::Recency { days: 30 }, 2000),
            rule(
                3,
                None,
                RuleKind::Preferred {
                    min_s: 600,
                    max_s: 1200,
                },
                250,
            ),
            rule(4, None, RuleKind::Live, -50),
            rule(5, Some("UCa"), RuleKind::Short, 0),
            rule(6, Some("UCa"), RuleKind::Recency { days: 7 }, 40),
            rule(7, Some("UCb"), RuleKind::Live, 70),
        ];
        let global = in_force(&rules, None);
        assert_eq!((global.short, global.live), (-1000, -50));
        assert_eq!((global.recency_points, global.recency_days), (2000, 30));
        assert_eq!(
            global.preferred,
            Some(Band {
                min_s: 600,
                max_s: 1200,
                points: 250
            })
        );
        let a = in_force(&rules, Some(&ChannelId("UCa".to_owned())));
        assert_eq!(a.short, 0, "no penalty for this channel's Shorts");
        assert_eq!(
            (a.recency_points, a.recency_days),
            (40, 7),
            "the channel's recency"
        );
        // A NEGATIVE R is refused where it is kept (`edit::valid`); one that
        // reached the table all the same scores no recency rather than less.
        let mut negative = rules.clone();
        negative[5].points = -40;
        let a = in_force(&negative, Some(&ChannelId("UCa".to_owned())));
        assert_eq!((a.recency_points, a.recency_days), (0, 7));
        assert_eq!(
            a.live, -50,
            "what the channel does not override stays global"
        );
        let b = in_force(&rules, Some(&ChannelId("UCb".to_owned())));
        assert_eq!((b.short, b.live), (-1000, 70));
        // A CHANNEL'S VERSION SWITCHED OFF switches the kind off THERE — the
        // global one does not come back in its place — and a global one
        // switched off counts nowhere.
        let mut off = rules;
        off[6].enabled = false;
        off[4].enabled = false;
        let b = in_force(&off, Some(&ChannelId("UCb".to_owned())));
        assert_eq!(
            b.live, 0,
            "the channel's live rule is off, not the global one on"
        );
        assert_eq!(b.short, -1000, "what it has no version of stays global");
        let a = in_force(&off, Some(&ChannelId("UCa".to_owned())));
        assert_eq!(a.short, 0);
        off[0].enabled = false;
        let c = in_force(&off, Some(&ChannelId("UCc".to_owned())));
        assert_eq!(
            (c.short, c.live),
            (0, -50),
            "a global one off counts nowhere"
        );

        let weights = Weights::default();
        let mut short = facts("", Some(900));
        short.is_short = true;
        short.live = true;
        assert_eq!(static_points(&global, &weights, &short), -1000 + 250 - 50);
        assert_eq!(static_points(&a, &weights, &short), 250 - 50);
    }

    #[test]
    fn terms_count_global_and_channel_once_each_folded_and_by_word() {
        let all = Fields::TITLE
            .with(Fields::DESCRIPTION)
            .with(Fields::TAGS)
            .with(Fields::SECOND_DESCRIPTION)
            .with(Fields::SECOND_TAGS);
        let rules = [
            rule(
                1,
                None,
                // pt-BR: the accents are the subject, folded on both sides.
                term("Configuração", TermMatch::Word, Fields::TITLE),
                100,
            ),
            rule(2, None, term("rust lang", TermMatch::Phrase, all), 30),
            rule(3, Some("UCa"), term("tutor", TermMatch::Prefix, all), 7),
            rule(4, Some("UCa"), term("cat", TermMatch::Word, all), 1000),
            rule(5, None, term("   ", TermMatch::Word, all), 5000),
        ];
        let weights = Weights::default();
        let a = in_force(&rules, Some(&ChannelId("UCa".to_owned())));
        assert_eq!(
            a.terms.len(),
            4,
            "global AND channel; an empty pattern is none"
        );

        // Folded: accents and case on both sides; counted once though it
        // matches the title twice.
        // pt-BR: the same word, without and with its accents.
        let one = facts("CONFIGURACAO — configuração!", None);
        assert_eq!(static_points(&a, &weights, &one), 100);
        // Word boundaries: `category` is not `cat`, `tutorials` starts with
        // `tutor`.
        let two = facts("Category: tutorials", None);
        assert_eq!(static_points(&a, &weights, &two), 7);
        // A phrase inside one field, never across two.
        let mut three = facts("Learning rust", None);
        three.description = "lang servers";
        assert_eq!(static_points(&a, &weights, &three), 0);
        three.description = "about RUST-lang, again rust lang";
        assert_eq!(static_points(&a, &weights, &three), 30);
        // The title rule reads the title only.
        let mut four = facts("", None);
        // pt-BR: the word again, in a field the rule does not read.
        four.description = "configuração";
        assert_eq!(static_points(&a, &weights, &four), 0);
        // The second fields are read too, tag by tag.
        let second = ["my cat".to_owned(), "misc".to_owned()];
        let mut five = facts("", None);
        five.second_tags = &second;
        assert_eq!(static_points(&a, &weights, &five), 1000);
        let mut six = facts("", None);
        six.second_description = "a Tutorial";
        assert_eq!(static_points(&a, &weights, &six), 7);
        // The global rules do not see a channel's terms.
        let global = in_force(&rules, None);
        assert_eq!(static_points(&global, &weights, &five), 0);
    }

    #[test]
    fn the_rating_adds_its_weight() {
        let weights = Weights {
            very_negative: -2000,
            negative: -500,
            positive: 500,
            very_positive: 2000,
        };
        let rules = InForce::default();
        for (rating, points) in [
            (Rating::VeryNegative, -2000),
            (Rating::Negative, -500),
            (Rating::Positive, 500),
            (Rating::VeryPositive, 2000),
        ] {
            let mut rated = facts("", None);
            rated.rating = Some(rating);
            assert_eq!(static_points(&rules, &weights, &rated), points);
        }
        let edited = Weights {
            positive: 1,
            ..weights
        };
        let mut rated = facts("", None);
        rated.rating = Some(Rating::Positive);
        assert_eq!(
            static_points(&rules, &edited, &rated),
            1,
            "the weights are editable"
        );
    }

    #[test]
    fn words_and_matching_at_their_edges() {
        assert_eq!(
            words_of("Ärger über C++ & 日本語"),
            ["arger", "uber", "c", "日本語"]
        );
        let w = |s: &str| words_of(s);
        assert!(term_matches(
            &w("rust"),
            TermMatch::Word,
            &w("I like Rust.")
        ));
        assert!(!term_matches(&w("rust"), TermMatch::Word, &w("rusty")));
        assert!(term_matches(&w("rust"), TermMatch::Prefix, &w("rusty")));
        assert!(term_matches(
            &w("new rus"),
            TermMatch::Prefix,
            &w("the new rustc")
        ));
        assert!(!term_matches(
            &w("new rus"),
            TermMatch::Prefix,
            &w("new, old rustc")
        ));
        assert!(!term_matches(&[], TermMatch::Word, &w("anything")));
        assert!(!term_matches(&w("a b c"), TermMatch::Phrase, &w("a b")));
        assert!(is_live(Some("was_live")) && !is_live(Some("not_live")) && !is_live(None));
    }
}
