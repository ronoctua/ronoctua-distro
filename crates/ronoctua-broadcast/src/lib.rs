//! World radio, world TV and the Internet Archive's films — the one crate that
//! talks to radio-browser, iptv-org and archive.org.
//!
//! # What it is
//!
//! Three public directories, read into typed values the player queues and a
//! door draws:
//!
//! - [`radio`]: radio-browser's stations, by country, genre (tag), language
//!   and name, and the click a station is owed once it plays;
//! - [`iptv`]: iptv-org's channels and streams, downloaded gzipped and kept as
//!   a compact [`iptv::Index`] every TV question is answered from, locally;
//! - [`archive`]: the Internet Archive's search and an item's files, reduced
//!   to what mpv plays.
//!
//! Beside them: [`country`] (one spelling of a country code — iptv-org writes
//! `UK`), [`cache`] (the player's cache directory, bounded and read-ordered),
//! [`picks`] (favourites and the recently played) and [`table`] (the
//! categories of "channels by theme", generated from `assets/channels.nix`).
//!
//! # How it talks
//!
//! Every fetch takes the network as a parameter ([`Net`]). The daemon hands
//! it [`internet::Internet`] (feature `net`); a test hands it a list of
//! answers that records every request, so an address is asserted without a
//! socket. Every parser is pure and takes bytes.
//!
//! NOTHING HERE SPAWNS A THREAD OR READS A CLOCK: a fetch blocks its caller,
//! which is a worker of the daemon, and "now" is an argument.
//!
//! # What it answers when it cannot
//!
//! A [`Refusal`] — which directory, and why, from a closed set ([`Why`]). No
//! sentence: the words are the door's, one per variant.

use serde::{Deserialize, Serialize};

mod ask;
mod json;
mod query;

pub mod archive;
pub mod cache;
pub mod country;
pub mod iptv;
pub mod picks;
pub mod radio;
pub mod table;

#[cfg(feature = "net")]
pub mod internet;

#[cfg(test)]
mod canned;

pub use ask::{MOST_BYTES, Net, Request, answered, why_of_status};
pub use country::Code;

/// Who is asking, on every request: radio-browser asks every client for a
/// descriptive `appname/version`, and a contact.
pub const USER_AGENT: &str = concat!(
    "ronoctua-player/",
    env!("CARGO_PKG_VERSION"),
    " ( https://github.com/ronoctua/ronoctua-distro )"
);

/// Which directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// radio-browser: the world's radio stations.
    Radio,
    /// iptv-org: the world's TV channels.
    Iptv,
    /// The Internet Archive: films and recordings.
    Archive,
}

impl Source {
    /// Every directory.
    pub const ALL: [Self; 3] = [Self::Radio, Self::Iptv, Self::Archive];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Radio => "radio",
            Self::Iptv => "iptv",
            Self::Archive => "archive",
        }
    }

    /// The party a consent row names: the domain every request of this
    /// directory goes to. radio-browser's servers are found at run time, and
    /// every one is under this domain ([`radio::is_server`]).
    #[must_use]
    pub const fn host(self) -> &'static str {
        match self {
            Self::Radio => "radio-browser.info",
            Self::Iptv => iptv::HOST,
            Self::Archive => archive::HOST,
        }
    }
}

/// Why a directory did not answer, as a closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// The host could not be reached, or said it is down (5xx).
    Unreachable,
    /// The host refused the question (a 4xx that is none of the below).
    Declined,
    /// The host asked to be asked less often (429).
    TooManyAsks,
    /// There is no such thing (404, 410, or an empty answer where one was
    /// named).
    NothingFound,
    /// The answer is not what this reads.
    Malformed,
    /// The answer is larger than its ceiling.
    TooLarge,
    /// The item exists and nothing in it is something mpv plays.
    NothingPlayable,
    /// There were no words to ask with.
    NothingToAsk,
    /// A name (a station's id, an item's identifier) is not shaped like one,
    /// and was not sent.
    NotAName,
}

impl Why {
    /// Every refusal, for a test that walks the words.
    pub const ALL: [Self; 9] = [
        Self::Unreachable,
        Self::Declined,
        Self::TooManyAsks,
        Self::NothingFound,
        Self::Malformed,
        Self::TooLarge,
        Self::NothingPlayable,
        Self::NothingToAsk,
        Self::NotAName,
    ];

    /// Its kebab-case name, the suffix of the key a door says it with.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unreachable => "unreachable",
            Self::Declined => "declined",
            Self::TooManyAsks => "too-many-asks",
            Self::NothingFound => "nothing-found",
            Self::Malformed => "malformed",
            Self::TooLarge => "too-large",
            Self::NothingPlayable => "nothing-playable",
            Self::NothingToAsk => "nothing-to-ask",
            Self::NotAName => "not-a-name",
        }
    }
}

/// A directory's refusal: which, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// Which directory refused.
    pub source: Source,
    /// Why.
    pub why: Why,
}

impl Refusal {
    /// `source` refused, for `why`.
    #[must_use]
    pub const fn new(source: Source, why: Why) -> Self {
        Self { source, why }
    }
}

/// A coordinate a directory gave, in signed decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Spot {
    /// North is positive.
    pub lat: f32,
    /// East is positive.
    pub lon: f32,
}

impl Spot {
    /// A coordinate, when it is one: on the planet, finite, and not exactly
    /// `0, 0` — the point in the Gulf of Guinea where an unset pair lands,
    /// and where no station stands.
    #[must_use]
    pub fn of(lat: f64, lon: f64) -> Option<Self> {
        let on_the_planet = lat.is_finite()
            && lon.is_finite()
            && (-90.0..=90.0).contains(&lat)
            && (-180.0..=180.0).contains(&lon);
        let unset = lat == 0.0 && lon == 0.0;
        (on_the_planet && !unset).then_some(Self {
            lat: lat as f32,
            lon: lon as f32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_is_its_serde_name() {
        for why in Why::ALL {
            assert_eq!(
                serde_json::to_string(&why).unwrap(),
                format!("\"{}\"", why.id())
            );
        }
        for source in Source::ALL {
            assert_eq!(
                serde_json::to_string(&source).unwrap(),
                format!("\"{}\"", source.id())
            );
        }
        assert_eq!(
            serde_json::to_string(&Refusal::new(Source::Radio, Why::NothingFound)).unwrap(),
            r#"{"source":"radio","why":"nothing-found"}"#
        );
    }

    #[test]
    fn an_unset_or_impossible_coordinate_is_none() {
        assert_eq!(Spot::of(0.0, 0.0), None);
        assert_eq!(Spot::of(91.0, 0.0), None);
        assert_eq!(Spot::of(0.0, f64::NAN), None);
        assert_eq!(
            Spot::of(-23.55, -46.63),
            Some(Spot {
                lat: -23.55,
                lon: -46.63
            })
        );
        assert!(Spot::of(0.0, 12.5).is_some(), "only BOTH zero is unset");
    }

    #[test]
    fn the_user_agent_names_the_program_and_a_contact() {
        assert!(USER_AGENT.starts_with("ronoctua-player/"));
        assert!(USER_AGENT.contains("https://github.com/ronoctua/ronoctua-distro"));
    }
}
