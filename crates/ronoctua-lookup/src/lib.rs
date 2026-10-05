//! The online lookups — the player's, and the security centre's checks — and
//! the person's keys for them.
//!
//! # The one crate that talks to these hosts
//!
//! lrclib.net (lyrics), sponsor.ajay.app (SponsorBlock), musicbrainz.org and
//! coverartarchive.org (a song's tags and cover), wikidata.org (a film,
//! keyless), api.themoviedb.org (a film's synopsis and poster, with the
//! person's token), api.opensubtitles.com and api.subdl.com (subtitles, with
//! the person's key), and www.podnapisi.net, whose reader is here and asleep
//! ([`podnapisi::AWAKE`]): the domain has not resolved since June 2026. And
//! the security centre's three checks: www.virustotal.com (a file's hash, and
//! the file itself only after a question that says what that shares —
//! [`virustotal`]), api.xposedornot.com (an e-mail address in known
//! breaches, [`breach`]) and api.pwnedpasswords.com (a password, by the first
//! five digits of its SHA-1 alone, [`pwned`]). Every first request goes to a
//! [`Host`]; nothing else in the workspace names one.
//!
//! # Whose each one is
//!
//! [`Service::scope`] and [`keys::Key::scope`] say it: the player's Internet
//! page lists the player's services and keys and never the security
//! centre's, and the other way round.
//!
//! # Typed answers and typed refusals
//!
//! Each service answers its own values ([`musicbrainz::Recording`],
//! [`wikidata::Film`], [`found::Found`]…) or a [`Refusal`]: which
//! [`Service`] and [`Why`], a closed set with a sentence each in the door.
//! Nothing here is a sentence — the words are the shell's, by key.
//!
//! # Pure, over a [`Net`]
//!
//! A service module builds a [`ask::Request`] (the address, the headers, a
//! key as a [`Secret`] that never prints) and reads a reply; the [`Net`] in
//! between is [`internet::Internet`] for the daemon (feature `net`) and a list
//! of answers for a test. So every parser here is tested against
//! hand-written JSON and every request against its address and headers,
//! without a socket.
//!
//! # Manners
//!
//! One static [`pace::Pace`] per host (MusicBrainz asks for one request a
//! second, and three doors would be three limiters: the daemon is the one
//! process that asks). A [`USER_AGENT`] naming the public repository.
//! Nothing asks on its own: the daemon asks when a person pressed a row that
//! names the host.
//!
//! # Every function that takes a `Net` blocks
//!
//! On the pace and on the network. Call it on a THREAD, never on a drawing
//! one.

#![deny(unsafe_code)]

use std::time::Duration;

use serde::{Deserialize, Serialize};

pub mod ask;
pub mod breach;
pub mod coverart;
pub mod found;
pub mod keys;
pub mod lrclib;
pub mod musicbrainz;
pub mod name;
pub mod opensubtitles;
pub mod pace;
pub mod picture;
pub mod podnapisi;
pub mod pwned;
pub mod query;
pub mod sponsorblock;
pub mod subdl;
pub mod suggestion;
pub mod tmdb;
pub mod virustotal;
pub mod wikidata;

#[cfg(feature = "net")]
pub mod internet;

#[cfg(test)]
pub(crate) mod canned;

pub use ask::{Net, Request};
pub use keys::Secret;

/// Who is asking, in every request.
///
/// MusicBrainz asks for "application name/version ( contact url )" and
/// answers a client that does not say with a 403; radio-browser and LRCLIB
/// ask the same. The address is the public repository (invariant 2 holds:
/// nothing personal).
pub const USER_AGENT: &str = concat!(
    "ronoctua-player/",
    env!("CARGO_PKG_VERSION"),
    " ( https://github.com/ronoctua/ronoctua-distro )"
);

/// A service a lookup asks — what a refusal names and a consent row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Service {
    /// Lyrics, synced or plain.
    #[serde(rename = "lrclib")]
    Lrclib,
    /// Sponsored stretches of a YouTube video.
    #[serde(rename = "sponsorblock")]
    SponsorBlock,
    /// A song's tags.
    #[serde(rename = "musicbrainz")]
    MusicBrainz,
    /// An album's cover.
    #[serde(rename = "coverartarchive")]
    CoverArtArchive,
    /// A film's facts, keyless.
    #[serde(rename = "wikidata")]
    Wikidata,
    /// A film's synopsis and poster, with the person's token.
    #[serde(rename = "tmdb")]
    Tmdb,
    /// Subtitles, with the person's key (and account).
    #[serde(rename = "opensubtitles")]
    OpenSubtitles,
    /// Subtitles, with the person's key.
    #[serde(rename = "subdl")]
    SubDl,
    /// Subtitles, keyless — asleep ([`podnapisi::AWAKE`]).
    #[serde(rename = "podnapisi")]
    Podnapisi,
    /// A file's verdict by many engines, with the person's key.
    #[serde(rename = "virustotal")]
    VirusTotal,
    /// An e-mail address in known breaches, keyless.
    #[serde(rename = "xposedornot")]
    XposedOrNot,
    /// A password in known breaches, by five digits of its hash, keyless.
    #[serde(rename = "pwnedpasswords")]
    PwnedPasswords,
}

/// Whose a service or a key is: which page lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    /// The player's Internet page.
    Player,
    /// The security centre's online checks.
    Security,
}

impl Service {
    /// Every service, in the order the Internet page lists them.
    pub const ALL: [Self; 12] = [
        Self::Lrclib,
        Self::SponsorBlock,
        Self::MusicBrainz,
        Self::CoverArtArchive,
        Self::Wikidata,
        Self::Tmdb,
        Self::OpenSubtitles,
        Self::SubDl,
        Self::Podnapisi,
        Self::VirusTotal,
        Self::XposedOrNot,
        Self::PwnedPasswords,
    ];

    /// Whose it is.
    #[must_use]
    pub const fn scope(self) -> Scope {
        match self {
            Self::VirusTotal | Self::XposedOrNot | Self::PwnedPasswords => Scope::Security,
            _ => Scope::Player,
        }
    }

    /// Its id: the serde name, and the tail of its text key
    /// (`player-service-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Lrclib => "lrclib",
            Self::SponsorBlock => "sponsorblock",
            Self::MusicBrainz => "musicbrainz",
            Self::CoverArtArchive => "coverartarchive",
            Self::Wikidata => "wikidata",
            Self::Tmdb => "tmdb",
            Self::OpenSubtitles => "opensubtitles",
            Self::SubDl => "subdl",
            Self::Podnapisi => "podnapisi",
            Self::VirusTotal => "virustotal",
            Self::XposedOrNot => "xposedornot",
            Self::PwnedPasswords => "pwnedpasswords",
        }
    }

    /// The hosts it reaches first — what a consent row names.
    #[must_use]
    pub const fn hosts(self) -> &'static [Host] {
        match self {
            Self::Lrclib => &[Host::Lrclib],
            Self::SponsorBlock => &[Host::SponsorBlock],
            Self::MusicBrainz => &[Host::MusicBrainz],
            Self::CoverArtArchive => &[Host::CoverArtArchive],
            Self::Wikidata => &[Host::Wikidata, Host::WikidataQuery, Host::Commons],
            Self::Tmdb => &[Host::Tmdb, Host::TmdbImages],
            Self::OpenSubtitles => &[Host::OpenSubtitles],
            Self::SubDl => &[Host::SubDl, Host::SubDlFiles],
            Self::Podnapisi => &[Host::Podnapisi],
            Self::VirusTotal => &[Host::VirusTotal],
            Self::XposedOrNot => &[Host::XposedOrNot],
            Self::PwnedPasswords => &[Host::PwnedPasswords],
        }
    }

    /// The domain it hands addresses out under, when it does: OpenSubtitles
    /// answers a login with a session host (`vip-api.opensubtitles.com`) and
    /// a download with a link on another (`www.opensubtitles.com`). A pass
    /// names one host and these are named at run time, so the person's
    /// consent to the service — its row's host let — lets the names under
    /// this domain too (`ronoctua_http::web::request_under`).
    #[must_use]
    pub const fn domain(self) -> Option<&'static str> {
        match self {
            Self::OpenSubtitles => Some("opensubtitles.com"),
            _ => None,
        }
    }

    /// The key it needs, when it needs one.
    #[must_use]
    pub const fn key(self) -> Option<keys::Key> {
        match self {
            Self::Tmdb => Some(keys::Key::Tmdb),
            Self::OpenSubtitles => Some(keys::Key::OpenSubtitles),
            Self::SubDl => Some(keys::Key::SubDl),
            Self::VirusTotal => Some(keys::Key::VirusTotal),
            _ => None,
        }
    }

    /// Whether it is asleep in this build: its reader is here and nothing
    /// asks it (see [`podnapisi::AWAKE`]).
    #[must_use]
    pub const fn is_dormant(self) -> bool {
        matches!(self, Self::Podnapisi) && !podnapisi::AWAKE
    }
}

/// A host a lookup's FIRST request goes to — one [`pace::Pace`] each.
///
/// Redirects go wherever the host sends them (archive.org's storage hosts, a
/// Commons thumbnail), by [`ronoctua_http::web`]'s rule; the pace is the
/// first host's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Host {
    /// `lrclib.net`.
    Lrclib,
    /// `sponsor.ajay.app`.
    SponsorBlock,
    /// `musicbrainz.org`.
    MusicBrainz,
    /// `coverartarchive.org`.
    CoverArtArchive,
    /// `www.wikidata.org` — the search.
    Wikidata,
    /// `query.wikidata.org` — SPARQL.
    WikidataQuery,
    /// `commons.wikimedia.org` — a film's poster (P3383).
    Commons,
    /// `api.themoviedb.org`.
    Tmdb,
    /// `image.tmdb.org` — posters.
    TmdbImages,
    /// `api.opensubtitles.com` (and the download link it hands out).
    OpenSubtitles,
    /// `api.subdl.com`.
    SubDl,
    /// `dl.subdl.com` — the zips.
    SubDlFiles,
    /// `www.podnapisi.net`.
    Podnapisi,
    /// `www.virustotal.com`.
    VirusTotal,
    /// `api.xposedornot.com`.
    XposedOrNot,
    /// `api.pwnedpasswords.com`.
    PwnedPasswords,
}

impl Host {
    /// Every host.
    pub const ALL: [Self; 16] = [
        Self::Lrclib,
        Self::SponsorBlock,
        Self::MusicBrainz,
        Self::CoverArtArchive,
        Self::Wikidata,
        Self::WikidataQuery,
        Self::Commons,
        Self::Tmdb,
        Self::TmdbImages,
        Self::OpenSubtitles,
        Self::SubDl,
        Self::SubDlFiles,
        Self::Podnapisi,
        Self::VirusTotal,
        Self::XposedOrNot,
        Self::PwnedPasswords,
    ];

    /// Its name, as the address carries it and a consent row shows it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Lrclib => "lrclib.net",
            Self::SponsorBlock => "sponsor.ajay.app",
            Self::MusicBrainz => "musicbrainz.org",
            Self::CoverArtArchive => "coverartarchive.org",
            Self::Wikidata => "www.wikidata.org",
            Self::WikidataQuery => "query.wikidata.org",
            Self::Commons => "commons.wikimedia.org",
            Self::Tmdb => "api.themoviedb.org",
            Self::TmdbImages => "image.tmdb.org",
            Self::OpenSubtitles => "api.opensubtitles.com",
            Self::SubDl => "api.subdl.com",
            Self::SubDlFiles => "dl.subdl.com",
            Self::Podnapisi => "www.podnapisi.net",
            Self::VirusTotal => "www.virustotal.com",
            Self::XposedOrNot => "api.xposedornot.com",
            Self::PwnedPasswords => "api.pwnedpasswords.com",
        }
    }

    /// The service it belongs to.
    #[must_use]
    pub const fn service(self) -> Service {
        match self {
            Self::Lrclib => Service::Lrclib,
            Self::SponsorBlock => Service::SponsorBlock,
            Self::MusicBrainz => Service::MusicBrainz,
            Self::CoverArtArchive => Service::CoverArtArchive,
            Self::Wikidata | Self::WikidataQuery | Self::Commons => Service::Wikidata,
            Self::Tmdb | Self::TmdbImages => Service::Tmdb,
            Self::OpenSubtitles => Service::OpenSubtitles,
            Self::SubDl | Self::SubDlFiles => Service::SubDl,
            Self::Podnapisi => Service::Podnapisi,
            Self::VirusTotal => Service::VirusTotal,
            Self::XposedOrNot => Service::XposedOrNot,
            Self::PwnedPasswords => Service::PwnedPasswords,
        }
    }

    /// The least time between two requests to it.
    ///
    /// MusicBrainz documents one a second per address and answers 503 above
    /// it; Podnapisi documented nothing and was known to block bursts;
    /// XposedOrNot asks for one a second. VirusTotal's free key is four
    /// requests a MINUTE, so fifteen seconds apart — which is also what an
    /// analysis is polled at. The rest are a quarter of a second: under every
    /// limit they publish (OpenSubtitles was measured at five a second) and
    /// far above anything a person pressing rows can ask.
    #[must_use]
    pub const fn every(self) -> Duration {
        match self {
            Self::MusicBrainz | Self::Podnapisi | Self::XposedOrNot => Duration::from_secs(1),
            Self::VirusTotal => Duration::from_secs(15),
            _ => Duration::from_millis(250),
        }
    }

    /// `https://<name>`.
    #[must_use]
    pub fn origin(self) -> String {
        format!("https://{}", self.name())
    }
}

/// Why a lookup gave nothing, as a closed set: one sentence each in the door
/// (`player-trouble-lookup-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// The service needs the person's key and there is none.
    NoKey,
    /// The service refused the key, the token or the account.
    KeyRefused,
    /// What was pasted is not the kind of key the service takes (TMDB's v3
    /// key where the read token goes).
    WrongKey,
    /// Today's allowance is spent (OpenSubtitles' downloads).
    Quota,
    /// Asked too fast: the service said to slow down.
    TooManyAsks,
    /// The service knows nothing about it.
    NothingFound,
    /// There was nothing to ask with: no title, no words.
    NothingToAsk,
    /// The service could not be reached, or is down.
    Unreachable,
    /// It answered something this does not read.
    Malformed,
    /// It answered more than any sane answer to that question.
    TooLarge,
    /// It said no, for a reason it did not name.
    Declined,
    /// The file to fingerprint could not be read.
    Unreadable,
    /// The file is too small to fingerprint (a moviehash reads 128 KiB).
    TooSmall,
    /// The service is asleep in this build.
    Dormant,
    /// The person has not let the host (`ronoctua-consent`): nothing was
    /// sent.
    NotLet,
}

impl Why {
    /// Every reason — one sentence each in the door.
    pub const ALL: [Self; 15] = [
        Self::NoKey,
        Self::KeyRefused,
        Self::WrongKey,
        Self::Quota,
        Self::TooManyAsks,
        Self::NothingFound,
        Self::NothingToAsk,
        Self::Unreachable,
        Self::Malformed,
        Self::TooLarge,
        Self::Declined,
        Self::Unreadable,
        Self::TooSmall,
        Self::Dormant,
        Self::NotLet,
    ];

    /// Its id: the serde name, and the tail of its text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoKey => "no-key",
            Self::KeyRefused => "key-refused",
            Self::WrongKey => "wrong-key",
            Self::Quota => "quota",
            Self::TooManyAsks => "too-many-asks",
            Self::NothingFound => "nothing-found",
            Self::NothingToAsk => "nothing-to-ask",
            Self::Unreachable => "unreachable",
            Self::Malformed => "malformed",
            Self::TooLarge => "too-large",
            Self::Declined => "declined",
            Self::Unreadable => "unreadable",
            Self::TooSmall => "too-small",
            Self::Dormant => "dormant",
            Self::NotLet => "not-let",
        }
    }
}

/// A lookup that gave nothing: which service, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// Who was asked.
    pub service: Service,
    /// Why nothing came.
    pub why: Why,
}

impl Refusal {
    /// `service` refused, for `why`.
    #[must_use]
    pub const fn new(service: Service, why: Why) -> Self {
        Self { service, why }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.service.id(), self.why.id())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_agent_names_the_player_and_the_public_repository() {
        assert!(USER_AGENT.starts_with("ronoctua-player/"), "{USER_AGENT}");
        assert!(
            USER_AGENT.ends_with(" ( https://github.com/ronoctua/ronoctua-distro )"),
            "{USER_AGENT}"
        );
        assert!(!USER_AGENT.chars().any(char::is_control));
    }

    #[test]
    fn every_host_belongs_to_a_service_that_names_it() {
        for host in Host::ALL {
            assert!(
                host.service().hosts().contains(&host),
                "{host:?} is not on {:?}'s consent row",
                host.service()
            );
            assert!(host.origin().starts_with("https://"));
        }
        for service in Service::ALL {
            assert!(!service.hosts().is_empty(), "{service:?}");
        }
    }

    #[test]
    fn ids_are_the_serde_names() {
        for s in Service::ALL {
            assert_eq!(
                serde_json::to_string(&s).unwrap(),
                format!("\"{}\"", s.id())
            );
        }
        let refusal = Refusal::new(Service::MusicBrainz, Why::TooManyAsks);
        assert_eq!(
            serde_json::to_string(&refusal).unwrap(),
            r#"{"service":"musicbrainz","why":"too-many-asks"}"#
        );
        for why in Why::ALL {
            assert_eq!(
                serde_json::to_string(&why).unwrap(),
                format!("\"{}\"", why.id())
            );
        }
    }

    /// FAILS IF: a security check lands on the player's Internet page, or a
    /// player's lookup in the security centre — or VirusTotal is asked faster
    /// than its free key allows.
    #[test]
    fn each_service_is_one_pages() {
        let security: Vec<Service> = Service::ALL
            .into_iter()
            .filter(|s| s.scope() == Scope::Security)
            .collect();
        assert_eq!(
            security,
            [
                Service::VirusTotal,
                Service::XposedOrNot,
                Service::PwnedPasswords
            ]
        );
        for s in Service::ALL {
            if let Some(key) = s.key() {
                assert_eq!(key.scope(), s.scope(), "{s:?}'s key is another page's");
            }
        }
        assert!(Host::VirusTotal.every() >= Duration::from_secs(15));
    }

    #[test]
    fn musicbrainz_is_asked_once_a_second_and_only_podnapisi_sleeps() {
        assert_eq!(Host::MusicBrainz.every(), Duration::from_secs(1));
        assert!(Host::Lrclib.every() < Duration::from_secs(1));
        let dormant: Vec<Service> = Service::ALL
            .into_iter()
            .filter(|s| s.is_dormant())
            .collect();
        assert_eq!(dormant, [Service::Podnapisi]);
    }
}
