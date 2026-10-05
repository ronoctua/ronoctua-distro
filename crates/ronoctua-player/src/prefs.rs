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
//! names the hosts it asks: the PRESS is the consent. The door records it —
//! host by host, in the distro's one consent file (`ronoctua-consent`,
//! [`let_merged`]) — and then asks; the daemon asks a service only while
//! every host of it is let there ([`Preferences::allows`], over
//! [`Service::hosts`]), and otherwise refuses, naming the host. Nothing is
//! let on a fresh machine, and the Internet page takes a service off.
//!
//! # The player's own list, merged once
//!
//! Before the rule was the distro's, the player kept its own list of
//! services let (`consented`, in this file). A file that still has one is
//! merged into the consent file ONCE ([`merge_consent`], by the daemon at its
//! start and on every reload) and the list leaves this file; until then it
//! still counts, so nothing the person let stops working in between.
//!
//! The directories — the world's radio and TV, the Internet Archive's films,
//! YouTube's lives — are let the same way and apart
//! ([`Preferences::directories`]): the first press on "Ask {host}" lets that
//! one, and none is let on a fresh machine.

use ronoctua_consent::Consents;
use ronoctua_lookup::{Host, Service};
use serde::{Deserialize, Serialize};

use crate::find::Directory;

/// The file's name.
pub const FILE: &str = "player.json";

/// The consent file's name, beside it — what a door watches as well, since
/// a press keeps the hosts it lets there ([`let_merged`]).
pub const CONSENT_FILE: &str = ronoctua_consent::FILE;

/// Why the consent file would not keep a press — named here, so a door
/// that says it needs no second path to the consent crate.
pub use ronoctua_consent::Unkept as ConsentUnkept;

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
    /// THE PLAYER'S OWN LIST, from before the consent file: the services a
    /// press let, as an older file keeps them. Merged into the consent file
    /// once ([`merge_consent`]) and never written again; it counts until
    /// then.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub consented: Vec<Service>,
    /// The directories the person let the player list from, in the order
    /// they were let.
    pub directories: Vec<Directory>,
    /// Whether the lyrics shown are translated, and into what.
    pub lyrics_translation: LyricsTranslation,
    /// THE HOSTS THE PERSON LET, as the consent file said when these
    /// preferences were read ([`read`]) — kept there and never here.
    #[serde(skip)]
    pub hosts: Consents,
}

/// The lyrics' translation, as the person left it: on or off, into which
/// language — `None` is the language of the person's doors — and WHO
/// translates: MyMemory (keyless, the default), DeepL (the person's key) or a
/// LibreTranslate server the person names (`ronoctua_words::Provider`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LyricsTranslation {
    /// Translate.
    pub on: bool,
    /// Into which language, by its code.
    pub to: Option<String>,
    /// Who translates.
    pub by: ronoctua_words::Provider,
    /// THE LIBRETRANSLATE SERVER last named, kept while another translates:
    /// turning the ring back to LibreTranslate asks it again only when there
    /// is none.
    pub address: Option<String>,
}

impl Default for LyricsTranslation {
    fn default() -> Self {
        Self {
            on: false,
            to: None,
            by: ronoctua_words::Provider::MyMemory,
            address: None,
        }
    }
}

impl LyricsTranslation {
    /// The translators, in the order a door's ring turns them: the
    /// LibreTranslate server is the one last named (`address`), or `None`
    /// when none was — a door asks for it then.
    #[must_use]
    pub fn ring(&self) -> [Option<ronoctua_words::Provider>; 3] {
        use ronoctua_words::Provider;
        [
            Some(Provider::MyMemory),
            Some(Provider::DeepL),
            self.address
                .clone()
                .map(|url| Provider::LibreTranslate { url }),
        ]
    }
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            lyrics_online: false,
            sponsorblock: false,
            skipped: SKIPPED_BY_DEFAULT.iter().map(|s| (*s).to_owned()).collect(),
            consented: Vec::new(),
            directories: Vec::new(),
            lyrics_translation: LyricsTranslation::default(),
            hosts: Consents::default(),
        }
    }
}

/// Every host `services` ask, by name, each once — what a press lets in the
/// consent file.
#[must_use]
pub fn hosts_of(services: &[Service]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for host in services.iter().flat_map(|s| s.hosts()) {
        if !out.contains(&host.name()) {
            out.push(host.name());
        }
    }
    out
}

impl Preferences {
    /// Whether the player may ask `service` now: the person let EVERY host
    /// it asks — by a press on a row naming it — or switched on the lookup
    /// that asks it on its own. A service asleep in this build is never
    /// asked.
    #[must_use]
    pub fn allows(&self, service: Service) -> bool {
        if service.is_dormant() {
            return false;
        }
        self.pressed(service)
            || match service {
                Service::Lrclib => self.lyrics_online,
                Service::SponsorBlock => self.sponsorblock,
                _ => false,
            }
    }

    /// Whether the person let `host` — one no lookup's service names, as the
    /// lyrics' translator's (`Trouble::Off` names it).
    #[must_use]
    pub fn allows_host(&self, host: &str) -> bool {
        self.hosts.allows(host)
    }

    /// Whether a PRESS let `service` — every one of its hosts let in the
    /// consent file, or the player's own older list names it — as apart from
    /// a switch that asks it on its own.
    #[must_use]
    pub fn pressed(&self, service: Service) -> bool {
        let hosts = service.hosts();
        (!hosts.is_empty() && hosts.iter().all(|h| self.hosts.allows(h.name())))
            || self.consented.contains(&service)
    }

    /// Let the player ask `service` — a press on a row that names it: its
    /// hosts, in this copy. The door keeps it ([`let_merged`]).
    pub fn consent(&mut self, service: Service) {
        self.hosts = std::mem::take(&mut self.hosts).with(&hosts_of(&[service]), true);
    }

    /// Stop letting the player ask `service` when a row is pressed: its
    /// hosts, and the older list's word for it.
    pub fn withdraw(&mut self, service: Service) {
        self.hosts = std::mem::take(&mut self.hosts).with(&hosts_of(&[service]), false);
        self.consented.retain(|s| *s != service);
    }

    /// The first of `services` the player may not ask now, when there is
    /// one: the one a refusal names.
    #[must_use]
    pub fn first_off(&self, services: &[Service]) -> Option<Service> {
        services.iter().copied().find(|s| !self.allows(*s))
    }

    /// Whether the player may list from `directory` now: its host let in
    /// the consent file — which is what its requests are sent on — or the
    /// player's own older list naming it, until that list is merged
    /// ([`merge_consent`]).
    #[must_use]
    pub fn allows_directory(&self, directory: Directory) -> bool {
        self.hosts.allows(directory.host()) || self.directories.contains(&directory)
    }

    /// Let the player list from `directory` — a press on a row naming its
    /// host: its host, in this copy. The door keeps it ([`let_merged`]).
    pub fn let_directory(&mut self, directory: Directory) {
        self.hosts = std::mem::take(&mut self.hosts).with(&[directory.host()], true);
    }

    /// Stop letting the player list from `directory`: its host, and the
    /// older list's word for it.
    pub fn withdraw_directory(&mut self, directory: Directory) {
        self.hosts = std::mem::take(&mut self.hosts).with(&[directory.host()], false);
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

/// The preferences as they are now, with the hosts the consent file lets —
/// what the daemon asks by, at the moment it asks, and what a door shows.
#[must_use]
pub fn read() -> Preferences {
    let mut p = store().read();
    p.hosts = ronoctua_consent::store().read();
    p
}

/// Why the consents were not kept.
#[derive(Debug)]
pub enum Unlet {
    /// `player.json` is there and will not parse: writing the whole of it
    /// back would replace what is in it.
    WillNotParse,
    /// `player.json` could not be written.
    Unwritable(std::io::Error),
    /// The consent file would not take it (`ronoctua_consent::Unkept`).
    Consent(ronoctua_consent::Unkept),
}

/// The services and directories let — or no longer — KEPT: their hosts in
/// the consent file, merged by host — the file a request is let by
/// (`ronoctua_consent::pass`), so a directory let is a directory asked. Taken
/// off, the player's own older lists in `player.json` lose their word for it
/// too, merged with what is on disk (read as it is there, changed only in
/// what was pressed, and written back whole). A file that will not parse is
/// refused, never written over.
///
/// # Errors
///
/// [`Unlet`], saying which file.
pub fn let_merged(services: &[Service], directories: &[Directory], on: bool) -> Result<(), Unlet> {
    let_merged_with(&store(), services, directories, on, |hosts, on| {
        ronoctua_consent::let_hosts(hosts, on)
    })
}

/// Hosts let — or no longer — by name, in the consent file: the press on a
/// row a [`crate::wire::Trouble::Off`] is said on when its host is no
/// lookup's service nor a directory (the lyrics' translator's).
///
/// # Errors
///
/// [`Unlet::Consent`].
pub fn let_hosts(hosts: &[&str], on: bool) -> Result<(), Unlet> {
    ronoctua_consent::let_hosts(hosts, on).map_err(Unlet::Consent)
}

/// [`let_merged`], over a store and a way to let hosts a test hands it.
fn let_merged_with(
    store: &ronoctua_store::Store<Preferences>,
    services: &[Service],
    directories: &[Directory],
    on: bool,
    let_hosts: impl FnOnce(&[&str], bool) -> Result<(), ronoctua_consent::Unkept>,
) -> Result<(), Unlet> {
    let mut hosts = hosts_of(services);
    for d in directories {
        if !hosts.contains(&d.host()) {
            hosts.push(d.host());
        }
    }
    if !hosts.is_empty() {
        let_hosts(&hosts, on).map_err(Unlet::Consent)?;
    }
    // THE OLDER LISTS' WORD goes too, when one is taken off: it would
    // otherwise still count. Let, there is nothing of `player.json` to touch.
    if on {
        return Ok(());
    }
    let mut p = store.parsed().ok_or(Unlet::WillNotParse)?;
    let before = p.clone();
    p.consented.retain(|s| !services.contains(s));
    p.directories.retain(|d| !directories.contains(d));
    if p == before {
        return Ok(());
    }
    store.write(&p).map_err(Unlet::Unwritable)
}

/// The player's own older lists of services and directories let, MERGED
/// ONCE into the consent file — every host of every one of them let there —
/// and taken out of `player.json`. `Ok(false)` when there was nothing to
/// merge.
///
/// # Errors
///
/// [`Unlet`]: the list stays where it is, and still counts.
pub fn merge_consent() -> Result<bool, Unlet> {
    merge_consent_with(&store(), |hosts| ronoctua_consent::let_hosts(hosts, true))
}

/// [`merge_consent`], over a store and a way to let hosts a test hands it.
fn merge_consent_with(
    store: &ronoctua_store::Store<Preferences>,
    let_hosts: impl FnOnce(&[&str]) -> Result<(), ronoctua_consent::Unkept>,
) -> Result<bool, Unlet> {
    // `parsed` and never `read`: this writes the whole file back.
    let mut p = store.parsed().ok_or(Unlet::WillNotParse)?;
    if p.consented.is_empty() && p.directories.is_empty() {
        return Ok(false);
    }
    let mut hosts = hosts_of(&p.consented);
    for d in &p.directories {
        if !hosts.contains(&d.host()) {
            hosts.push(d.host());
        }
    }
    let_hosts(&hosts).map_err(Unlet::Consent)?;
    p.consented.clear();
    p.directories.clear();
    store.write(&p).map_err(Unlet::Unwritable)?;
    Ok(true)
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
        assert!(p.pressed(Service::MusicBrainz) && p.pressed(Service::CoverArtArchive));
        assert!(p.allows_directory(Directory::Radio));
        p.let_all(&[Service::CoverArtArchive], &[Directory::Radio], false);
        assert!(p.pressed(Service::MusicBrainz));
        assert!(!p.pressed(Service::CoverArtArchive));
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
        // LET BY ITS HOST, where its requests are let — the older list
        // untouched.
        assert!(p.hosts.allows(Directory::Radio.host()));
        assert!(p.directories.is_empty());
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
        assert_eq!(
            p.hosts,
            Consents::default().with(&["musicbrainz.org"], true),
            "its host, once"
        );
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
        // THE HOSTS ARE THE CONSENT FILE'S, never written into this one.
        let s = serde_json::to_string(&p).expect("serialises");
        assert!(
            !s.contains("musicbrainz") && !s.contains("consented"),
            "{s}"
        );
    }

    /// FAILS IF: a service is let while one of its hosts is not — Wikidata
    /// asks three — or the older list stops counting before it is merged.
    #[test]
    fn a_service_is_let_when_every_host_it_asks_is() {
        let mut p = Preferences {
            hosts: Consents::default().with(&["www.wikidata.org", "query.wikidata.org"], true),
            ..Preferences::default()
        };
        assert_eq!(Service::Wikidata.hosts().len(), 3);
        assert!(!p.allows(Service::Wikidata), "commons is not let");
        assert!(p.allows_host("www.wikidata.org") && !p.allows_host("api.mymemory.translated.net"));
        p.hosts = std::mem::take(&mut p.hosts).with(&hosts_of(&[Service::Wikidata]), true);
        assert!(p.allows(Service::Wikidata));
        let older = Preferences {
            consented: vec![Service::Tmdb],
            ..Preferences::default()
        };
        assert!(older.allows(Service::Tmdb), "an older file still counts");
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-player-prefs-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        dir
    }

    /// FAILS IF: the player's older list is not merged into the consent
    /// file host by host, stays in `player.json` after it was merged, is
    /// merged into nothing, or the rest of the file is lost on the way.
    #[test]
    fn the_older_list_is_merged_once_and_leaves_the_file() {
        let dir = scratch("merge");
        std::fs::write(
            dir.join(FILE),
            r#"{"consented":["lrclib","tmdb"],"directories":["radio"],"sponsorblock":true}"#,
        )
        .expect("an older file");
        let prefs = ronoctua_store::Store::<Preferences>::at(Some(dir.clone()), FILE);
        let consent = ronoctua_consent::store_at(Some(dir.clone()));
        let merged = merge_consent_with(&prefs, |hosts| {
            ronoctua_consent::let_hosts_in(&consent, hosts, true)
        })
        .expect("merged");
        assert!(merged);
        for host in [
            "lrclib.net",
            "api.themoviedb.org",
            "image.tmdb.org",
            Directory::Radio.host(),
        ] {
            assert!(ronoctua_consent::allows_in(&consent, host), "{host}");
        }
        let mut after = prefs.read();
        assert!(after.consented.is_empty() && after.directories.is_empty());
        assert!(after.sponsorblock);
        // THE DIRECTORY STILL LET, now by its host in the consent file.
        after.hosts = consent.read();
        assert!(after.allows_directory(Directory::Radio));
        let text = std::fs::read_to_string(dir.join(FILE)).expect("written");
        assert!(!text.contains("consented"), "{text}");
        // ONCE: nothing left to merge.
        assert!(!merge_consent_with(&prefs, |_| panic!("merged twice")).expect("read"));
        // A FILE THAT WILL NOT PARSE is refused, and kept.
        std::fs::write(dir.join(FILE), b"{ not json").expect("broken");
        assert!(matches!(
            merge_consent_with(&prefs, |_| Ok(())),
            Err(Unlet::WillNotParse)
        ));
        assert_eq!(std::fs::read(dir.join(FILE)).expect("kept"), b"{ not json");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a press keeps a service or a directory anywhere but the
    /// consent file — where its requests are let — one taken off still
    /// counts by an older list, or a press that changed nothing rewrites
    /// `player.json`.
    #[test]
    fn a_press_is_kept_where_its_consent_lives() {
        let dir = scratch("let");
        let prefs = ronoctua_store::Store::<Preferences>::at(Some(dir.clone()), FILE);
        let consent = ronoctua_consent::store_at(Some(dir.clone()));
        let keep = |services: &[Service], directories: &[Directory], on: bool| {
            let_merged_with(&prefs, services, directories, on, |hosts, on| {
                ronoctua_consent::let_hosts_in(&consent, hosts, on)
            })
            .expect("kept");
        };
        keep(&[Service::MusicBrainz], &[], true);
        assert!(ronoctua_consent::allows_in(&consent, "musicbrainz.org"));
        assert!(!dir.join(FILE).exists(), "nothing of player.json changed");
        keep(&[], &[Directory::Iptv], true);
        assert!(ronoctua_consent::allows_in(
            &consent,
            Directory::Iptv.host()
        ));
        assert!(!dir.join(FILE).exists(), "nothing of player.json changed");
        std::fs::write(
            dir.join(FILE),
            r#"{"consented":["musicbrainz"],"directories":["iptv"]}"#,
        )
        .expect("older");
        keep(&[Service::MusicBrainz], &[Directory::Iptv], false);
        assert!(!ronoctua_consent::allows_in(&consent, "musicbrainz.org"));
        assert!(!ronoctua_consent::allows_in(
            &consent,
            Directory::Iptv.host()
        ));
        let after = prefs.read();
        assert!(
            after.consented.is_empty() && after.directories.is_empty(),
            "the older words went too"
        );
        let _ = std::fs::remove_dir_all(dir);
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
