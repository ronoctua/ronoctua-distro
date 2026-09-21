//! What `LEADER d` glances at: what time it is somewhere, and what the weather
//! and the money are doing there.
//!
//! # Two halves, and only one of them needs the network
//!
//! The CLOCK and the DATE cost nothing: `ronoctua_files::when` already reads
//! TZif by hand, so any zone in the tzdata is a file away. That half works on
//! a machine that has never been online, and it is the half the owner asked to
//! be the default for a complementary place.
//!
//! The WEATHER and the RATE need somebody else to know. That is a thread, a
//! cache and two hosts — never the drawing loop, which must not wait on a
//! socket for any reason at all.
//!
//! # The file is the cache, and the cache is the broadcast
//!
//! `$XDG_STATE_HOME/ronoctua/world.json`. STATE and not configuration: it is
//! what happened, not what anybody chose. The places themselves are
//! configuration and live elsewhere.
//!
//! It is the same arrangement as the theme and the notifications: whoever
//! draws reads the file, whoever fetches writes it, and nobody has to be told.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod fetch;
pub mod http;
pub mod prefs;
pub mod store;

include!(concat!(env!("OUT_DIR"), "/world.rs"));

/// What is known about a place right now.
///
/// Every field is an `Option`, and that is the design: a glance drawn before
/// the first answer has arrived shows the clock and simply has no weather in
/// it. An absent temperature is not an error state to render, it is a piece
/// the line does not have — see `shell::world` for how the line closes over
/// the gap rather than reserving space for it.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Known {
    /// Degrees Celsius. Metric whatever the language — the same decision
    /// `LC_MEASUREMENT` records, and for the same reason.
    pub temperature: Option<f64>,
    /// What one unit of the place's currency is worth in its `rate_in`.
    pub rate: Option<f64>,
}

/// How the last pass at ONE SOURCE went.
///
/// # Why a failure is written down at all
///
/// Because it was not, and the whole of it was one `debug!` line: a host that
/// was down cost the temperature, and the bar simply did not have it. From the
/// outside that is indistinguishable from a feature that does not work, which
/// is what the owner read. `AGENTS.md` already names the rule — a refusal is a
/// VALUE and not an absence, because an empty panel and a refused one look the
/// same on screen and only one of them has a sentence. This is the sentence.
///
/// # And why it is per SOURCE and not per place
///
/// Because who is asked and whether they answered is about the HOST. Five
/// places watched with one weather host that is down is one thing that
/// happened, not five.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Attempt {
    /// Which host answered, or was tried last. Empty before the first pass.
    pub host: String,
    /// When something last ARRIVED, in epoch seconds. `None` while nothing
    /// ever has — which is not the same as a fetch that has just failed.
    pub at: Option<i64>,
    /// What went wrong on the last pass, in the fetcher's own words. Empty
    /// when the last pass went right.
    ///
    /// NOT a text key: it carries a host's own words and a status number, and
    /// nothing here can translate those. The screen draws it as the machine's
    /// answer, which is the same thing `Job::trouble` does.
    pub trouble: String,
}

/// Everything the fetcher has found, by zone.
///
/// Keyed by ZONE and not by index, because the places are a build-time table
/// and the cache outlives a rebuild: an index would hand Brasília's weather to
/// whoever took its place in the list.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Cache {
    /// zone -> what is known about it.
    pub places: std::collections::BTreeMap<String, Known>,
    /// source id -> how the last pass at it went.
    pub sources: std::collections::BTreeMap<String, Attempt>,
}

impl Cache {
    /// What is known about a zone. Never fails: an unknown zone is a place
    /// nothing has arrived for yet.
    #[must_use]
    pub fn of(&self, zone: &str) -> Known {
        self.places.get(zone).copied().unwrap_or_default()
    }

    /// How the last pass at a source went. Never fails: a source nobody has
    /// asked yet is an empty record, not a missing key.
    #[must_use]
    pub fn about(&self, source: Source) -> Attempt {
        self.sources.get(source.id()).cloned().unwrap_or_default()
    }
}

/// One of the two things the glance asks somebody else for.
///
/// # Why this lives here and not in the screen that draws it
///
/// Because it was in the screen, as a second enum with the same two arms and a
/// `host()` that read the generated table — and the day a source grew a list
/// of hosts, the copy would have had to grow one too. What a source IS belongs
/// beside the table that declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The temperature.
    Weather,
    /// One unit of a currency in another.
    Rate,
}

impl Source {
    /// Both, in the order they are declared.
    pub const ALL: [Self; 2] = [Self::Weather, Self::Rate];

    /// The id, which is the key it is written down under and the tail of its
    /// text keys.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Weather => "weather",
            Self::Rate => "rate",
        }
    }

    /// Who is asked, in order. The first that answers wins.
    #[must_use]
    pub const fn from(self) -> &'static [Provider] {
        match self {
            Self::Weather => WEATHER_FROM,
            Self::Rate => RATE_FROM,
        }
    }

    /// How often, in minutes — the declared cadence of THIS source.
    ///
    /// It used to be the smaller of the two for both, which asked the exchange
    /// host twelve times as often as `assets/world.nix` says to.
    #[must_use]
    pub const fn every_minutes(self) -> u64 {
        match self {
            Self::Weather => WEATHER_EVERY_MINUTES,
            Self::Rate => RATE_EVERY_MINUTES,
        }
    }

    /// Is this machine asking it at all?
    #[must_use]
    pub const fn asked(self, prefs: &prefs::Preferences) -> bool {
        match self {
            Self::Weather => prefs.ask_weather,
            Self::Rate => prefs.ask_rate,
        }
    }

    /// Turns it off, or on again.
    pub const fn flip(self, prefs: &mut prefs::Preferences) {
        match self {
            Self::Weather => prefs.ask_weather = !prefs.ask_weather,
            Self::Rate => prefs.ask_rate = !prefs.ask_rate,
        }
    }

    /// Does this place want it?
    #[must_use]
    pub const fn wanted_by(self, place: &prefs::Wanted) -> bool {
        match self {
            Self::Weather => place.weather,
            Self::Rate => place.rate,
        }
    }
}

/// The place the glance is about.
///
/// # Why this cannot be empty
///
/// `PLACES` is generated from `assets/world.nix`, whose `primary` is a record
/// and not a list — so the table always has at least one. Saying so here keeps
/// every caller from carrying an `Option` that cannot be `None`.
#[must_use]
pub fn primary() -> &'static Place {
    PLACES
        .first()
        .expect("assets/world.nix always declares a primary place")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is generated, so what is worth asserting is the SHAPE the
    /// rest of the system relies on rather than its contents.
    #[test]
    fn there_is_always_a_primary_and_it_is_first() {
        assert!(!PLACES.is_empty());
        assert_eq!(primary().zone, PLACES[0].zone);
    }

    /// A zone appears once. The generator refuses a repeat, and this is what
    /// notices if that refusal is ever softened — the cache is keyed by zone,
    /// so two places sharing one would share a temperature.
    #[test]
    fn no_zone_is_named_twice() {
        let mut seen = std::collections::BTreeSet::new();
        for place in PLACES {
            assert!(seen.insert(place.zone), "{} is named twice", place.zone);
        }
    }

    /// Every place shows at least one thing, and the generator is what makes
    /// that true. A place showing nothing would draw its name and a separator.
    #[test]
    fn every_place_shows_something() {
        for place in PLACES {
            assert!(
                place.clock || place.date || place.weather || place.rate,
                "{} shows nothing",
                place.zone
            );
        }
    }

    /// An unknown zone is an empty answer, not a panic: the cache outlives the
    /// table, so a place that was removed is a key nothing asks about and a
    /// place that was added is a key nothing has written yet.
    #[test]
    fn the_cache_answers_for_a_zone_it_has_never_seen() {
        let cache = Cache::default();
        assert_eq!(cache.of("Antarctica/Troll"), Known::default());
        assert_eq!(cache.of(primary().zone).temperature, None);
    }
}
