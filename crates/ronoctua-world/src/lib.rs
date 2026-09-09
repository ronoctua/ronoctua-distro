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
}

impl Cache {
    /// What is known about a zone. Never fails: an unknown zone is a place
    /// nothing has arrived for yet.
    #[must_use]
    pub fn of(&self, zone: &str) -> Known {
        self.places.get(zone).copied().unwrap_or_default()
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
