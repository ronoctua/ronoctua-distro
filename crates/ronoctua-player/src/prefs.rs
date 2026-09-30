//! What a person chose about the player: `$XDG_CONFIG_HOME/ronoctua/player.json`.
//!
//! Every door writes it and the daemon rereads it on `Ask::Reload` — the file
//! is the broadcast, the rule every preference in this system follows. The
//! consents below the daemon reads at the moment it would ask, never from a
//! copy it read at start.
//!
//! # Nothing reaches the network until somebody says so
//!
//! Two switches ask on their own, and both are born OFF: lyrics from the
//! internet send what is playing to `lrclib.net` whenever a song starts, and
//! skipping sponsored segments sends which video is playing to
//! `sponsor.ajay.app`. Neither is a thing a machine should start doing by
//! itself; the view names the host beside each switch.
//!
//! Every other lookup asks only when a person presses a row, and the row
//! names the hosts it asks: the PRESS is the consent. The door records it
//! ([`Preferences::consent`]) and then asks; the daemon asks a service only
//! while it is recorded ([`Preferences::allows`]), and otherwise refuses,
//! naming the host. The list is born empty, and the Internet page takes a
//! service off it.
//!
//! The directories — the world's radio and TV, the Internet Archive's films,
//! YouTube's lives — are let the same way and apart
//! ([`Preferences::directories`]): the first press on "Ask {host}" lets that
//! one, and none is let on a fresh machine.

use ronoctua_lookup::{Host, Service};
use serde::{Deserialize, Serialize};

use crate::find::Directory;

/// The file's name.
pub const FILE: &str = "player.json";

/// The host lyrics are asked of.
pub const LRCLIB_HOST: &str = Host::Lrclib.name();

/// The host sponsored segments are asked of.
pub const SPONSORBLOCK_HOST: &str = Host::SponsorBlock.name();

/// Every category of SponsorBlock a player can SKIP — the ones that are a
/// stretch of time. `poi_highlight` is a point and `chapter` a label; neither
/// is something to jump over, and the service's own player leaves them out.
pub const CATEGORIES: [&str; 9] = [
    "sponsor",
    "selfpromo",
    "interaction",
    "intro",
    "outro",
    "preview",
    "hook",
    "music_offtopic",
    "filler",
];

/// The segments skipped when nobody chose: the ones that are not the video.
pub const SKIPPED_BY_DEFAULT: &[&str] = &["sponsor", "selfpromo", "interaction"];

/// The preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Ask `lrclib.net` for lyrics when a song has none of its own.
    pub lyrics_online: bool,
    /// Skip what SponsorBlock marks inside a YouTube video.
    pub sponsorblock: bool,
    /// Which of its categories are skipped.
    pub skipped: Vec<String>,
    /// The services the person let the player ask when they press a row
    /// that names them, in the order they were let.
    pub consented: Vec<Service>,
    /// The directories the person let the player list from, in the order
    /// they were let.
    pub directories: Vec<Directory>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            lyrics_online: false,
            sponsorblock: false,
            skipped: SKIPPED_BY_DEFAULT.iter().map(|s| (*s).to_owned()).collect(),
            consented: Vec::new(),
            directories: Vec::new(),
        }
    }
}

impl Preferences {
    /// Whether the player may ask `service` now: the person let it — by a
    /// press on a row naming it, or by switching on the lookup that asks it
    /// on its own. A service asleep in this build is never asked.
    #[must_use]
    pub fn allows(&self, service: Service) -> bool {
        if service.is_dormant() {
            return false;
        }
        self.consented.contains(&service)
            || match service {
                Service::Lrclib => self.lyrics_online,
                Service::SponsorBlock => self.sponsorblock,
                _ => false,
            }
    }

    /// Let the player ask `service` — a press on a row that names it.
    pub fn consent(&mut self, service: Service) {
        if !self.consented.contains(&service) {
            self.consented.push(service);
        }
    }

    /// Stop letting the player ask `service` when a row is pressed.
    pub fn withdraw(&mut self, service: Service) {
        self.consented.retain(|s| *s != service);
    }

    /// The first of `services` the player may not ask now, when there is
    /// one: the one a refusal names.
    #[must_use]
    pub fn first_off(&self, services: &[Service]) -> Option<Service> {
        services.iter().copied().find(|s| !self.allows(*s))
    }

    /// Whether the player may list from `directory` now.
    #[must_use]
    pub fn allows_directory(&self, directory: Directory) -> bool {
        self.directories.contains(&directory)
    }

    /// Let the player list from `directory` — a press on a row naming its
    /// host.
    pub fn let_directory(&mut self, directory: Directory) {
        if !self.directories.contains(&directory) {
            self.directories.push(directory);
        }
    }

    /// Stop letting the player list from `directory`.
    pub fn withdraw_directory(&mut self, directory: Directory) {
        self.directories.retain(|d| *d != directory);
    }

    /// Let — or stop letting — these services and directories at once: what
    /// a switch of the Internet page and a row that asks write, merged into
    /// the file as it is on disk.
    pub fn let_all(&mut self, services: &[Service], directories: &[Directory], on: bool) {
        for s in services {
            if on {
                self.consent(*s);
            } else {
                self.withdraw(*s);
            }
        }
        for d in directories {
            if on {
                self.let_directory(*d);
            } else {
                self.withdraw_directory(*d);
            }
        }
    }

    /// Let whatever a refusal's host names — a lookup's service
    /// ([`service_of_host`]) or a directory ([`Directory::of_host`]): the
    /// press on the row a [`crate::wire::Trouble::Off`] is said on. `false`
    /// when the host is none of them, and nothing was let.
    pub fn let_host(&mut self, host: &str) -> bool {
        if let Some(service) = service_of_host(host) {
            self.consent(service);
            return true;
        }
        if let Some(directory) = Directory::of_host(host) {
            self.let_directory(directory);
            return true;
        }
        false
    }
}

/// The host a refusal names for a service not let: its first — the one its
/// row names first.
#[must_use]
pub fn host_of(service: Service) -> &'static str {
    service.hosts().first().map_or("", |h| h.name())
}

/// The service a refusal's host belongs to — what a door lets when the
/// person presses the row the refusal is said on.
#[must_use]
pub fn service_of_host(host: &str) -> Option<Service> {
    Host::ALL
        .iter()
        .find(|h| h.name().eq_ignore_ascii_case(host))
        .map(|h| h.service())
}

/// The store.
#[must_use]
pub fn store() -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a switch of the Internet page stops letting — or taking back
    /// — exactly what it names, services and directories alike, or lets one
    /// twice.
    #[test]
    fn let_all_lets_and_takes_back_what_it_names() {
        let mut p = Preferences::default();
        p.let_all(
            &[Service::MusicBrainz, Service::CoverArtArchive],
            &[Directory::Radio],
            true,
        );
        p.let_all(&[Service::MusicBrainz], &[], true);
        assert_eq!(
            p.consented,
            [Service::MusicBrainz, Service::CoverArtArchive]
        );
        assert!(p.allows_directory(Directory::Radio));
        p.let_all(&[Service::CoverArtArchive], &[Directory::Radio], false);
        assert_eq!(p.consented, [Service::MusicBrainz]);
        assert!(!p.allows_directory(Directory::Radio));
    }

    /// FAILS IF: a fresh machine — or a file from before the consents — may
    /// ask ANY service: every lookup, the ones that ask on their own and the
    /// ones a row asks, is off until a person says so.
    #[test]
    fn nothing_reaches_the_network_until_somebody_says_so() {
        let p = Preferences::default();
        assert!(!p.lyrics_online && !p.sponsorblock);
        for service in Service::ALL {
            assert!(!p.allows(service), "{service:?} is let on a fresh machine");
        }
        let parsed: Preferences = serde_json::from_str("{}").expect("an empty file parses");
        assert_eq!(parsed, p);
        let older: Preferences =
            serde_json::from_str(r#"{"lyrics_online":false,"sponsorblock":false,"skipped":[]}"#)
                .expect("a file from before the consents");
        for service in Service::ALL {
            assert!(!older.allows(service), "{service:?}");
        }
        // AND NO DIRECTORY: the world's radio and TV, the archive and the
        // lives are each let by a press, never born let.
        for directory in Directory::ALL {
            assert!(
                !p.allows_directory(directory) && !parsed.allows_directory(directory),
                "{directory:?} is let on a fresh machine"
            );
            assert!(!older.allows_directory(directory), "{directory:?}");
        }
    }

    /// FAILS IF: the press on a refusal's row stops letting exactly what the
    /// host names — a directory's or a lookup's — or lets something for a
    /// host that is neither.
    #[test]
    fn a_press_on_a_refusal_lets_what_its_host_names() {
        let mut p = Preferences::default();
        assert!(p.let_host("radio-browser.info"));
        assert_eq!(p.directories, [Directory::Radio]);
        assert!(p.allows_directory(Directory::Radio));
        assert!(!p.allows_directory(Directory::Iptv));
        assert!(p.let_host("lrclib.net"));
        assert!(p.allows(Service::Lrclib));
        assert!(!p.let_host("example.org"));
        for directory in Directory::ALL {
            assert_eq!(Directory::of_host(directory.host()), Some(directory));
        }
        p.withdraw_directory(Directory::Radio);
        assert!(!p.allows_directory(Directory::Radio));
        let s = serde_json::to_string(&p).expect("serialises");
        assert!(s.contains(r#""directories":[]"#), "{s}");
        let read: Preferences =
            serde_json::from_str(r#"{"directories":["iptv","lives"]}"#).expect("parses");
        assert!(read.allows_directory(Directory::Lives));
    }

    /// FAILS IF: a press stops letting exactly the service it names — or a
    /// switch that asks on its own stops counting for its host, or a service
    /// asleep in this build can be let.
    #[test]
    fn a_press_lets_the_service_it_names_and_no_other() {
        let mut p = Preferences::default();
        p.consent(Service::MusicBrainz);
        p.consent(Service::MusicBrainz);
        assert_eq!(p.consented, [Service::MusicBrainz]);
        assert!(p.allows(Service::MusicBrainz));
        assert!(!p.allows(Service::CoverArtArchive));
        assert_eq!(
            p.first_off(&[Service::MusicBrainz, Service::CoverArtArchive]),
            Some(Service::CoverArtArchive)
        );
        p.lyrics_online = true;
        assert!(
            p.allows(Service::Lrclib),
            "the automatic lyrics let the host"
        );
        p.consent(Service::Podnapisi);
        assert!(!p.allows(Service::Podnapisi), "asleep is never asked");
        p.withdraw(Service::MusicBrainz);
        assert!(!p.allows(Service::MusicBrainz));
        let s = serde_json::to_string(&p).expect("serialises");
        assert!(s.contains(r#""consented":["podnapisi"]"#), "{s}");
    }

    /// FAILS IF: the host a refusal names stops leading back to the service
    /// a press on it lets.
    #[test]
    fn a_refusals_host_leads_back_to_its_service() {
        for service in Service::ALL {
            assert_eq!(service_of_host(host_of(service)), Some(service));
        }
        assert_eq!(service_of_host("example.org"), None);
        assert_eq!(LRCLIB_HOST, "lrclib.net");
        assert_eq!(SPONSORBLOCK_HOST, "sponsor.ajay.app");
    }
}
