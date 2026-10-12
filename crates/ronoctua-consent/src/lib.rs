//! The person's consent, host by host — and the private files keys live in.
//!
//! # The one crate that answers "may this host be asked?"
//!
//! Nothing in this distro talks to the network before the person let the
//! host it would ask. That was a rule of the player alone (its `consented`
//! list in `player.json`); it is now the distro's, and this crate is where it
//! lives: `$XDG_CONFIG_HOME/ronoctua[-dev]/consent.json`, one entry per host.
//! A search the person configured explicitly is its own consent, and it is
//! RECORDED here like any other: adding a channel to video-metrics lets its
//! hosts (`ronoctua_video_metrics::HOSTS`) in the same file, so the scanner's
//! every request still takes a [`Pass`]. The repository of the belongings is
//! the same: typing its address lets its host here, and every fetch and push
//! of the belongings mints a [`Pass`] for it first.
//!
//! # A `Pass` is the proof
//!
//! [`pass`] answers a [`Pass`] for a host the person let, or [`NotLet`]. A
//! `Pass` can be minted only here (its field is private and it has no public
//! constructor), and every function that reaches a host on the person's
//! behalf takes one (`ronoctua_words::translate`, `ronoctua_websearch::search`,
//! `ronoctua_calc::rates::fetch`, and `ronoctua_http::web::request_let`). So
//! "asked before consent" is a program that does not compile, not a review
//! comment.
//!
//! # Only the person's file mints one
//!
//! [`pass`] reads THIS instance's `consent.json` and nothing else. The
//! helpers over another store (`store_at`, `pass_in`, `let_hosts_in`,
//! `allows_in`) exist for tests — a test lets a host in a folder of its own —
//! and only under the `test-support` feature, which a crate turns on in its
//! `[dev-dependencies]` alone: a product that pointed them at a scratch
//! folder would mint a pass the person never gave, so a release build does
//! not have them (`network-asks-with-a-pass` refuses the feature in a
//! `[dependencies]` table, and a call to them outside a test).
//!
//! # Read at the moment of asking
//!
//! The file is read by [`allows`] and [`pass`] every time, never cached:
//! consent given in one door counts in the daemon at once, and consent taken
//! back stops the next request. Writing ([`let_hosts`]) merges by host over
//! [`ronoctua_store::Store::parsed`], so two doors letting two hosts keep both,
//! and a file that will not parse is refused rather than overwritten.
//!
//! # And the private files
//!
//! [`keys`] holds what a key, a token or a password is kept in: a
//! [`keys::Secret`] that never prints and is wiped when dropped, a file
//! written `0600` from its first byte ([`keys::write_private_file`]), and a
//! `0700` folder of them ([`keys::Dir`]). `ronoctua-lookup` re-exports them.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
#[cfg(any(test, feature = "test-support"))]
use std::path::PathBuf;

use ronoctua_store::Store;
use serde::{Deserialize, Serialize};

pub mod keys;

/// The file's name, under the config root.
pub const FILE: &str = "consent.json";

/// What `consent.json` holds: `{ "hosts": { "<host>": true } }`.
///
/// `false` is kept rather than removed: a host the person refused stays
/// refused when a door that remembers an older answer writes its own.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consents {
    /// Every host the person answered for, by its [`normal`] name.
    #[serde(default)]
    pub hosts: BTreeMap<String, bool>,
}

impl Consents {
    /// Whether the person let this host. A host never answered for is not let.
    #[must_use]
    pub fn allows(&self, host: &str) -> bool {
        normal(host).is_some_and(|h| self.hosts.get(&h).copied().unwrap_or(false))
    }

    /// The same consents with these hosts set to `on`. Hosts that are not a
    /// host's name ([`normal`] refuses them) are left out.
    #[must_use]
    pub fn with(mut self, hosts: &[&str], on: bool) -> Self {
        for host in hosts.iter().filter_map(|h| normal(h)) {
            self.hosts.insert(host, on);
        }
        self
    }
}

/// Permission to ask one host, minted only by [`pass`].
///
/// It carries the host it was minted for; whoever sends a request with it
/// checks that the request's FIRST host is this one (redirects follow
/// `ronoctua_http::web`'s own rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pass {
    host: String,
}

impl Pass {
    /// The host this pass lets, as [`normal`] wrote it.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Whether a request to `host` is covered by this pass.
    #[must_use]
    pub fn covers(&self, host: &str) -> bool {
        normal(host).is_some_and(|h| h == self.host)
    }
}

/// The refusal [`pass`] gives: the person has not let this host (or the name
/// is not a host's).
///
/// Not an absence: a door that holds one draws the consent row ("Ask
/// {host}") in its place, and the words are the door's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotLet {
    /// The host that was asked for, as it was asked.
    pub host: String,
}

/// Why [`let_hosts`] kept nothing, as a closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unkept {
    /// Neither `XDG_CONFIG_HOME` nor `HOME` is set: there is nowhere to keep it.
    NoHome,
    /// The file is there and does not parse; writing would erase it.
    Unparseable,
    /// The file could not be written.
    Unwritable,
}

impl Unkept {
    /// Its kebab-case name, the suffix of the key a door says it with
    /// (`consent-unkept-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoHome => "no-home",
            Self::Unparseable => "unparseable",
            Self::Unwritable => "unwritable",
        }
    }
}

/// A host's name as it is kept: trimmed, ASCII-lowercased, without a trailing
/// dot. `None` for text that is not a host's name — empty, or carrying a
/// scheme, a path, a port, a user or whitespace.
#[must_use]
pub fn normal(host: &str) -> Option<String> {
    let host = host.trim().trim_end_matches('.');
    let fits = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '.';
    (!host.is_empty() && host.chars().all(fits)).then(|| host.to_ascii_lowercase())
}

/// The store of this instance: `consent.json` under the config root, or a
/// store that keeps nothing when there is no home.
#[must_use]
pub fn store() -> Store<Consents> {
    Store::at(ronoctua_store::config_dir(), FILE)
}

/// Whether the person let this host, read from the file now.
#[must_use]
pub fn allows(host: &str) -> bool {
    store().read().allows(host)
}

/// A [`Pass`] for this host, when the person let it — read from the file now.
///
/// # Errors
///
/// [`NotLet`] when the person has not let the host (or it is not a host's
/// name).
pub fn pass(host: &str) -> Result<Pass, NotLet> {
    mint(&store(), host)
}

/// The one place a [`Pass`] is built: `store` lets `host`.
fn mint(store: &Store<Consents>, host: &str) -> Result<Pass, NotLet> {
    let refused = || NotLet {
        host: host.to_owned(),
    };
    let name = normal(host).ok_or_else(refused)?;
    if store.read().allows(&name) {
        Ok(Pass { host: name })
    } else {
        Err(refused())
    }
}

/// Let (`on`) or refuse these hosts, merged into what the file already says.
///
/// # Errors
///
/// [`Unkept::NoHome`], [`Unkept::Unparseable`] (the file is kept as it is),
/// [`Unkept::Unwritable`].
pub fn let_hosts(hosts: &[&str], on: bool) -> Result<(), Unkept> {
    keep(&store(), hosts, on)
}

/// [`allows`], over the `consent.json` under `config` — the config root a
/// door already holds and reads its other files from (the belongings'
/// `Places::config`, which is this instance's own root, or the folder a
/// photograph is taken over). It mints nothing: a [`Pass`] is only
/// [`pass`]'s, over the person's own file.
#[must_use]
pub fn allows_under(config: &std::path::Path, host: &str) -> bool {
    Store::<Consents>::at(Some(config.to_path_buf()), FILE)
        .read()
        .allows(host)
}

/// [`let_hosts`], over the `consent.json` under `config` — see
/// [`allows_under`].
///
/// # Errors
///
/// As [`let_hosts`].
pub fn let_hosts_under(config: &std::path::Path, hosts: &[&str], on: bool) -> Result<(), Unkept> {
    keep(&Store::at(Some(config.to_path_buf()), FILE), hosts, on)
}

/// [`let_hosts`], over a given store.
fn keep(store: &Store<Consents>, hosts: &[&str], on: bool) -> Result<(), Unkept> {
    if store.path().is_none() {
        return Err(Unkept::NoHome);
    }
    // `parsed` and never `read`: this writes the whole value back, and a file
    // that does not parse would be replaced by the default plus this change.
    let now = store.parsed().ok_or(Unkept::Unparseable)?;
    store
        .write(&now.with(hosts, on))
        .map_err(|_| Unkept::Unwritable)
}

/// A store in `dir`: a test's own consent file.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
#[must_use]
pub fn store_at(dir: Option<PathBuf>) -> Store<Consents> {
    Store::at(dir, FILE)
}

/// [`allows`], over a test's store.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
#[must_use]
pub fn allows_in(store: &Store<Consents>, host: &str) -> bool {
    store.read().allows(host)
}

/// [`pass`], over a test's store.
///
/// # Errors
///
/// As [`pass`].
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub fn pass_in(store: &Store<Consents>, host: &str) -> Result<Pass, NotLet> {
    mint(store, host)
}

/// [`let_hosts`], over a test's store.
///
/// # Errors
///
/// As [`let_hosts`].
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub fn let_hosts_in(store: &Store<Consents>, hosts: &[&str], on: bool) -> Result<(), Unkept> {
    keep(store, hosts, on)
}

/// A door's watch on `consent.json`, so a consent given in another door (or
/// the daemon) is drawn at once. Dropping it ends the watch.
#[cfg(feature = "watch")]
#[derive(Debug)]
pub struct Watched {
    _notice: ronoctua_files::notice::Notice,
}

/// Why a [`Watched`] could not be raised.
#[cfg(feature = "watch")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unwatched {
    /// There is no config root to watch.
    NoHome,
    /// The inotify watch or its thread could not be raised.
    NoWatcher,
}

#[cfg(feature = "watch")]
impl Watched {
    /// Watches this instance's `consent.json`; `on_change` runs on the watch's
    /// thread, and returning `false` ends it.
    ///
    /// # Errors
    ///
    /// [`Unwatched::NoHome`], [`Unwatched::NoWatcher`].
    pub fn new<F>(on_change: F) -> Result<Self, Unwatched>
    where
        F: Fn() -> bool + Send + 'static,
    {
        let dir = ronoctua_store::config_dir().ok_or(Unwatched::NoHome)?;
        Self::at(&dir, on_change)
    }

    /// Watches `consent.json` in `dir`.
    ///
    /// # Errors
    ///
    /// [`Unwatched::NoWatcher`].
    pub fn at<F>(dir: &std::path::Path, on_change: F) -> Result<Self, Unwatched>
    where
        F: Fn() -> bool + Send + 'static,
    {
        use ronoctua_files::notice::{Notice, Pace};
        Notice::on_file(dir, FILE, Pace::AtOnce, on_change)
            .map(|notice| Self { _notice: notice })
            .map_err(|_| Unwatched::NoWatcher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-consent-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_pass_exists_only_after_the_person_lets_the_host() {
        let dir = scratch("pass");
        let store = store_at(Some(dir.clone()));
        assert_eq!(
            pass_in(&store, "api.mymemory.translated.net"),
            Err(NotLet {
                host: "api.mymemory.translated.net".to_owned()
            }),
            "nothing is let on a fresh machine"
        );
        let_hosts_in(&store, &["API.MyMemory.translated.net."], true).unwrap();
        let pass = pass_in(&store, "api.mymemory.translated.net").unwrap();
        assert_eq!(pass.host(), "api.mymemory.translated.net");
        assert!(pass.covers("API.mymemory.translated.net"));
        assert!(!pass.covers("en.wiktionary.org"));
        // Merged by host: letting a second keeps the first.
        let_hosts_in(&store, &["en.wiktionary.org"], true).unwrap();
        assert!(allows_in(&store, "api.mymemory.translated.net"));
        // And taken back stops the next request.
        let_hosts_in(&store, &["api.mymemory.translated.net"], false).unwrap();
        assert!(pass_in(&store, "api.mymemory.translated.net").is_err());
        assert!(
            pass_in(&store, "https://en.wiktionary.org/").is_err(),
            "not a host"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unparseable_file_is_never_overwritten() {
        let dir = scratch("broken");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE), b"{ not json").unwrap();
        let store = store_at(Some(dir.clone()));
        assert_eq!(
            let_hosts_in(&store, &["lrclib.net"], true),
            Err(Unkept::Unparseable)
        );
        assert_eq!(std::fs::read(dir.join(FILE)).unwrap(), b"{ not json");
        assert_eq!(
            let_hosts_in(&store_at(None), &["lrclib.net"], true),
            Err(Unkept::NoHome)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a consent given in one door (or the daemon) is not heard
    /// by a door watching the file — it would go on drawing "Ask" for a host
    /// already let.
    #[cfg(feature = "watch")]
    #[test]
    fn a_door_hears_a_consent_given_elsewhere() {
        let dir = scratch("watch");
        std::fs::create_dir_all(&dir).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let watched = Watched::at(&dir, move || tx.send(()).is_ok()).unwrap();
        let store = store_at(Some(dir.clone()));
        // THE WATCH IS ARMED ON ITS OWN THREAD, after `at` returns: a write
        // made at once can land before `add_watch`. So a scratch host is
        // toggled until the watch is heard (`notice::armed`'s way), and only
        // then is the real consent given and waited for.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut on = true;
        loop {
            let_hosts_in(&store, &["armed.example"], on).unwrap();
            if rx
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_ok()
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the watch was never armed"
            );
            on = !on;
        }
        while rx.try_recv().is_ok() {}
        let_hosts_in(&store, &["lrclib.net"], true).unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("the watch heard the write");
        assert!(allows_in(&store, "lrclib.net"));
        drop(watched);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: the config root a door holds is not the file it reads and
    /// writes — a door's consent would land in one file and be looked for
    /// in another — or a write there forgets what the file said.
    #[test]
    fn a_door_reads_and_writes_the_consent_under_its_own_root() {
        let dir = scratch("under");
        assert!(!allows_under(&dir, "github.com"));
        let_hosts_under(&dir, &["lrclib.net"], true).unwrap();
        let_hosts_under(&dir, &["GitHub.com."], true).unwrap();
        assert!(allows_under(&dir, "github.com"));
        assert!(
            allows_under(&dir, "lrclib.net"),
            "the earlier consent was lost"
        );
        let store = store_at(Some(dir.clone()));
        assert!(
            pass_in(&store, "github.com").is_ok(),
            "another file was written"
        );
        let_hosts_under(&dir, &["github.com"], false).unwrap();
        assert!(!allows_under(&dir, "github.com"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_file_is_the_schema_the_design_names() {
        let kept = Consents::default().with(&["lrclib.net"], true);
        assert_eq!(
            serde_json::to_string(&kept).unwrap(),
            r#"{"hosts":{"lrclib.net":true}}"#
        );
    }
}
