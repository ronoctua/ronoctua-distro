//! video-metrics: a catalogue of the person's YouTube channels, scored.
//!
//! # The one boundary
//!
//! With YouTube over plain HTTP — the uploads feeds of a channel
//! (`/feeds/videos.xml?playlist_id=UULF…` for videos, `UUSH…` for Shorts) and
//! the `mqdefault` thumbnails of `i.ytimg.com` — and with the SQLite
//! catalogue at `$XDG_DATA_HOME/ronoctua[-dev]/video-metrics/catalog.sqlite`.
//! No SQL leaves this crate.
//!
//! # Who writes, who reads
//!
//! The scanner (the unit, feature `net`) and the video-metrics door
//! ([`person::Person`], the person's edits) write. The player's ranking and
//! fast-access read ([`read::Catalog`], `mode=ro`), with two exceptions that
//! go through the crate: [`read::Catalog::hide`] and
//! [`read::Catalog::delete`]. A door watches the `-wal` file and
//! [`read::Catalog::data_version`], never a clock.
//!
//! # The score
//!
//! `total = channel + static points + recency` ([`score`]): the channel's
//! score (0–10000), the static points of each video (duration bands, the
//! Short penalty, the preferred band, term rules global AND per channel, the
//! rating's weight), and a recency computed at read time. A rule or weight
//! that changes recomputes the static points in one transaction, off the
//! loop.
//!
//! # Tombstones
//!
//! What was skipped (before the reference, beyond the first scan, a blocked
//! Short, unavailable, deleted) stays in `skipped`: a deleted video never
//! comes back.
//!
//! # Layers
//!
//! Without features this is the SHAPE — ids, filters, rows, rules, weights,
//! refusals — and the pure [`score`]; every catalogue call refuses with
//! [`Why::NoDatabase`]. `db` is the catalogue: SQLite over the system's
//! `libsqlite3`, the schema and its migrations, the read and edit APIs, the
//! feed reader ([`feed`]) and the scanner's decisions ([`scan`], over any
//! [`scan::Reach`]). `net` is the one [`scan::Reach`] that reaches YouTube,
//! and the `ronoctua-video-metrics` binary.

#![forbid(unsafe_code)]

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod person;
pub mod read;
pub mod score;

#[cfg(feature = "db")]
mod db;
#[cfg(feature = "db")]
mod edit;
#[cfg(feature = "db")]
pub mod feed;
#[cfg(feature = "db")]
mod query;
#[cfg(feature = "net")]
mod reach;
#[cfg(feature = "net")]
pub use reach::fetch_thumb;
#[cfg(feature = "db")]
pub mod scan;
#[cfg(feature = "db")]
pub mod thumbs;

/// The catalogue's folder, under the data root.
pub const DIR: &str = "video-metrics";
/// The catalogue's file.
pub const FILE: &str = "catalog.sqlite";

/// `<data root>/video-metrics/catalog.sqlite`, or `None` with no home.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    ronoctua_xdg::data_dir().map(|d| d.join(DIR).join(FILE))
}

/// The hosts a scan asks: the uploads feeds and the thumbnails. Adding a
/// channel is the person's consent to both (`ronoctua_consent::let_hosts`),
/// and every request mints its `Pass` at the moment of asking — consent
/// taken back stops the next scan.
pub const HOSTS: [&str; 2] = [FEEDS_HOST, THUMBS_HOST];

/// Where the uploads feeds are asked.
pub const FEEDS_HOST: &str = "www.youtube.com";

/// Where the thumbnails are asked.
pub const THUMBS_HOST: &str = "i.ytimg.com";

/// A channel's score when it is added, and the middle of 0–10000.
pub const CHANNEL_SCORE: i32 = 5000;

/// The highest a channel's score goes.
pub const CHANNEL_SCORE_MOST: i32 = 10_000;

/// A video, by YouTube's id (`dQw4w9WgXcQ`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VideoId(pub String);

/// A channel, by YouTube's id (`UC…`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChannelId(pub String);

/// A rule, by the catalogue's row id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RuleId(pub i64);

/// The person's rating of a video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rating {
    /// Very negative.
    VeryNegative,
    /// Negative.
    Negative,
    /// Positive.
    Positive,
    /// Very positive.
    VeryPositive,
}

impl Rating {
    /// The four, in the ring's order.
    pub const ALL: [Self; 4] = [
        Self::VeryNegative,
        Self::Negative,
        Self::Positive,
        Self::VeryPositive,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::VeryNegative => "very-negative",
            Self::Negative => "negative",
            Self::Positive => "positive",
            Self::VeryPositive => "very-positive",
        }
    }
}

/// The points each rating adds (global, editable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Weights {
    /// For [`Rating::VeryNegative`].
    pub very_negative: i32,
    /// For [`Rating::Negative`].
    pub negative: i32,
    /// For [`Rating::Positive`].
    pub positive: i32,
    /// For [`Rating::VeryPositive`].
    pub very_positive: i32,
}

impl Weights {
    /// The weight of a rating.
    #[must_use]
    pub const fn of(&self, rating: Rating) -> i32 {
        match rating {
            Rating::VeryNegative => self.very_negative,
            Rating::Negative => self.negative,
            Rating::Positive => self.positive,
            Rating::VeryPositive => self.very_positive,
        }
    }
}

impl Default for Weights {
    /// −2000, −500, +500, +2000 (the design's, until the person edits them).
    fn default() -> Self {
        Self {
            very_negative: -2000,
            negative: -500,
            positive: 500,
            very_positive: 2000,
        }
    }
}

/// Which fields a term rule reads, as bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fields(pub u8);

impl Fields {
    /// The title.
    pub const TITLE: Self = Self(1);
    /// YouTube's description.
    pub const DESCRIPTION: Self = Self(1 << 1);
    /// YouTube's tags.
    pub const TAGS: Self = Self(1 << 2);
    /// The person's second description.
    pub const SECOND_DESCRIPTION: Self = Self(1 << 3);
    /// The person's second tags.
    pub const SECOND_TAGS: Self = Self(1 << 4);

    /// Whether every bit of `other` is set here.
    #[must_use]
    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Both sets of bits.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// How a term rule's pattern matches (accents and case folded, at word
/// boundaries).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TermMatch {
    /// A whole word.
    Word,
    /// The words in order.
    Phrase,
    /// A word that starts with it.
    Prefix,
}

/// What a rule measures.
///
/// The Short penalty, the preferred band, recency and being live are SINGLE:
/// one per scope (the global one, and each channel's version). Bands and
/// terms are lists.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuleKind {
    /// A term in the chosen fields; counts once per video.
    Term {
        /// The fields read.
        fields: Fields,
        /// The pattern, as the person typed it.
        pattern: String,
        /// How it matches.
        how: TermMatch,
    },
    /// A duration band, `[min_s, max_s]`.
    Band {
        /// The shortest duration in the band, seconds.
        min_s: u32,
        /// The longest, seconds.
        max_s: u32,
    },
    /// The preferred duration band, `[min_s, max_s]`: points ON TOP of the
    /// band a video falls in.
    Preferred {
        /// The shortest duration in the band, seconds.
        min_s: u32,
        /// The longest, seconds.
        max_s: u32,
    },
    /// Being a Short (the penalty).
    Short,
    /// Recency: the rule's points are R, fading to 0 over `days` (D).
    Recency {
        /// D, in days.
        days: u32,
    },
    /// Being live or a premiere.
    Live,
}

/// A rule of the score: global (`channel` is `None`) or a channel's own
/// version, which replaces the global one for that channel.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rule {
    /// Its id; `None` for a rule not yet kept.
    pub id: Option<RuleId>,
    /// The channel whose version it is; `None` for the global rule.
    pub channel: Option<ChannelId>,
    /// What it measures.
    pub kind: RuleKind,
    /// The points it adds (negative subtracts).
    pub points: i32,
    /// Whether it counts.
    pub enabled: bool,
}

/// The catalogue's switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Settings {
    /// Shorts are never catalogued (a channel may override); those already in
    /// are hidden with a reason.
    pub block_shorts: bool,
    /// The first scan of a channel takes only what was published in the last
    /// this many days…
    pub first_scan_days: u32,
    /// …and at most this many of the newest.
    pub first_scan_most: u32,
    /// Whether view counts are kept as snapshots.
    pub views_snapshots: bool,
}

impl RuleKind {
    /// Its kebab-case name, as the catalogue keeps it and the suffix of its
    /// key.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::Term { .. } => "term",
            Self::Band { .. } => "band",
            Self::Preferred { .. } => "preferred",
            Self::Short => "short",
            Self::Recency { .. } => "recency",
            Self::Live => "live",
        }
    }

    /// Whether there is one per scope (see [`RuleKind`]).
    #[must_use]
    pub const fn is_single(&self) -> bool {
        matches!(
            self,
            Self::Preferred { .. } | Self::Short | Self::Recency { .. } | Self::Live
        )
    }
}

impl TermMatch {
    /// Its kebab-case name, as the catalogue keeps it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Word => "word",
            Self::Phrase => "phrase",
            Self::Prefix => "prefix",
        }
    }

    /// The match a name keeps; `None` for a name that is none.
    #[must_use]
    pub fn of(id: &str) -> Option<Self> {
        [Self::Word, Self::Phrase, Self::Prefix]
            .into_iter()
            .find(|m| m.id() == id)
    }
}

impl Rating {
    /// Its number in the catalogue: −2, −1, 1, 2.
    #[must_use]
    pub const fn number(self) -> i64 {
        match self {
            Self::VeryNegative => -2,
            Self::Negative => -1,
            Self::Positive => 1,
            Self::VeryPositive => 2,
        }
    }

    /// The rating a number keeps; `None` for any other number.
    #[must_use]
    pub fn of_number(number: i64) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.number() == number)
    }
}

impl Default for Settings {
    /// The design's safeguard: 14 days, 10 videos; Shorts allowed; no views.
    fn default() -> Self {
        Self {
            block_shorts: false,
            first_scan_days: 14,
            first_scan_most: 10,
            views_snapshots: false,
        }
    }
}

/// Why a video id was never catalogued, or was taken out — the tombstones'
/// reasons, as a closed set (`video-metrics-skip-<id>`). A tombstone is
/// never lifted: what it names never comes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Skip {
    /// Published more than a day before the channel's reference.
    BeforeReference,
    /// Outside the first scan's window, or past its most.
    FirstScan,
    /// A Short while Shorts are blocked.
    BlockedShort,
    /// Private, removed, or not available (what yt-dlp said).
    Unavailable,
    /// Deleted by the person, or its channel removed.
    Deleted,
}

impl Skip {
    /// Every reason.
    pub const ALL: [Self; 5] = [
        Self::BeforeReference,
        Self::FirstScan,
        Self::BlockedShort,
        Self::Unavailable,
        Self::Deleted,
    ];

    /// Its kebab-case name, as the catalogue keeps it and the suffix of its
    /// key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::BeforeReference => "before-reference",
            Self::FirstScan => "first-scan",
            Self::BlockedShort => "blocked-short",
            Self::Unavailable => "unavailable",
            Self::Deleted => "deleted",
        }
    }
}

/// Why a scan stopped early, as a closed set (`video-metrics-stop-<id>`).
/// Recorded with the run; the brain reads the last one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stop {
    /// YouTube asked to prove this is not a bot.
    BotCheck,
    /// YouTube answered 429.
    TooManyRequests,
    /// The network went away.
    Offline,
    /// The person has not let (or took back) the hosts a scan asks
    /// ([`HOSTS`]): nothing was asked.
    NotLet,
}

impl Stop {
    /// Every stop.
    pub const ALL: [Self; 4] = [
        Self::BotCheck,
        Self::TooManyRequests,
        Self::Offline,
        Self::NotLet,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::BotCheck => "bot-check",
            Self::TooManyRequests => "too-many-requests",
            Self::Offline => "offline",
            Self::NotLet => "not-let",
        }
    }

    /// The stop a name keeps; `None` for a name that is none.
    #[must_use]
    pub fn of(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }
}

/// Why the catalogue answered nothing, as a closed set
/// (`video-metrics-why-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// Not answered any more: the skeleton's word, kept so a table of keys
    /// written against the frozen set stays total.
    NotYet,
    /// This build has no database (feature `db` off).
    NoDatabase,
    /// There is no catalogue yet: nothing was ever scanned.
    NotThere,
    /// The catalogue is there and cannot be read.
    Unreadable,
    /// The catalogue is of a newer schema than this build knows.
    OtherSchema,
    /// A writer held the catalogue past the busy timeout.
    Busy,
    /// No such video or channel.
    NotFound,
    /// A write failed.
    Failed,
    /// What was asked is not something the catalogue keeps: a rule with an
    /// empty term or a band that ends before it starts, a channel id that is
    /// not one.
    Invalid,
}

impl Why {
    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotYet => "not-yet",
            Self::NoDatabase => "no-database",
            Self::NotThere => "not-there",
            Self::Unreadable => "unreadable",
            Self::OtherSchema => "other-schema",
            Self::Busy => "busy",
            Self::NotFound => "not-found",
            Self::Failed => "failed",
            Self::Invalid => "invalid",
        }
    }
}

/// A refusal of the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
}

impl Why {
    /// Every answer, for a test that walks the words.
    pub const ALL: [Self; 9] = [
        Self::NotYet,
        Self::NoDatabase,
        Self::NotThere,
        Self::Unreadable,
        Self::OtherSchema,
        Self::Busy,
        Self::NotFound,
        Self::Failed,
        Self::Invalid,
    ];
}

impl Refusal {
    /// A refusal for this reason.
    #[must_use]
    pub const fn of(why: Why) -> Self {
        Self { why }
    }

    /// The refusal of a build without `db`.
    #[must_use]
    pub const fn no_database() -> Self {
        Self::of(Why::NoDatabase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_and_fields_are_what_the_design_says() {
        let weights = Weights::default();
        assert_eq!(weights.of(Rating::VeryNegative), -2000);
        assert_eq!(weights.of(Rating::Positive), 500);
        let both = Fields::TITLE.with(Fields::SECOND_TAGS);
        assert!(both.has(Fields::TITLE) && both.has(Fields::SECOND_TAGS));
        assert!(!both.has(Fields::DESCRIPTION));
        assert_eq!(Settings::default().first_scan_most, 10);
        let refused = read::Catalog::open_ro(std::path::Path::new("/nonexistent")).unwrap_err();
        assert_eq!(
            refused.why,
            if cfg!(feature = "db") {
                Why::NotThere
            } else {
                Why::NoDatabase
            }
        );
    }

    #[test]
    fn every_closed_set_has_its_names() {
        for why in Why::ALL {
            assert_eq!(
                serde_json::to_string(&why).unwrap(),
                format!("\"{}\"", why.id())
            );
        }
        for rating in Rating::ALL {
            assert_eq!(Rating::of_number(rating.number()), Some(rating));
        }
        assert_eq!(Rating::of_number(0), None);
        for how in [TermMatch::Word, TermMatch::Phrase, TermMatch::Prefix] {
            assert_eq!(TermMatch::of(how.id()), Some(how));
        }
        assert!(RuleKind::Short.is_single() && !RuleKind::Band { min_s: 0, max_s: 1 }.is_single());
        for skip in Skip::ALL {
            assert_eq!(
                serde_json::to_string(&skip).unwrap(),
                format!("\"{}\"", skip.id())
            );
        }
        for stop in Stop::ALL {
            assert_eq!(Stop::of(stop.id()), Some(stop));
            assert_eq!(
                serde_json::to_string(&stop).unwrap(),
                format!("\"{}\"", stop.id())
            );
        }
    }
}
