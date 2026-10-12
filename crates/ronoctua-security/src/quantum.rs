//! What each channel of this machine does against a quantum computer — the
//! table the security centre's post-quantum section draws (`LEADER s e q`).
//!
//! # The question, and the three answers that are not "yes"
//!
//! A conversation or a file recorded today can be read the day a quantum
//! computer breaks the key it was agreed or sealed with ("harvest now,
//! decrypt later"). So every channel says one [`Status`]:
//!
//! - [`Status::PostQuantum`] — the key is agreed or sealed with ML-KEM (or
//!   Streamlined NTRU Prime) beside a classical curve: both must fall.
//! - [`Status::Symmetric`] — a symmetric cipher with a key from a passphrase
//!   (AES-XTS-512, ChaCha20-Poly1305, AES-256-GCM): Grover's algorithm halves
//!   its strength, and half of 256 bits still holds. A weak passphrase is the
//!   weak part, quantum or not.
//! - [`Status::Classical`] — X25519, RSA, ed25519: what a quantum computer
//!   breaks. For a SIGNATURE (an extension, a cache, F-Droid, Secure Boot)
//!   the danger is a forgery made at the moment of an attack, never a
//!   recording read later — the sentence says which.
//! - [`Status::Unknown`] — nothing to read it from yet.
//!
//! # Where the answers come from
//!
//! [`Facts`]: what the machine says now — the belongings' recipient, the
//! groups our HTTPS hosts agreed (`ronoctua-http`'s record), the key
//! exchanges ssh would offer, in its order, once its configuration is read
//! (`ssh -G`, [`ssh_kex`], by store path), whether sops-nix decrypted
//! anything ([`sops_in_use`]) — and [`Built`], what this generation ships,
//! written at build (`RONOCTUA_PQ_FACTS`, `nix/pq-facts.nix`): the versions
//! of Chrome, OpenSSL, the Go a Syncthing was built with and whether its
//! source narrows TLS's curves, and which of the optional channels this
//! configuration turned on ([`Ships`]). [`channels`] is pure: the same facts,
//! the same table.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// What a channel does against a quantum computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// A post-quantum hybrid agrees or seals the key.
    PostQuantum,
    /// A symmetric cipher with a key from a passphrase.
    Symmetric,
    /// Broken by a quantum computer.
    Classical,
    /// Not known yet.
    Unknown,
}

impl Status {
    /// Every one.
    pub const ALL: [Self; 4] = [
        Self::PostQuantum,
        Self::Symmetric,
        Self::Classical,
        Self::Unknown,
    ];

    /// Its kebab-case name (`security-quantum-status-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::PostQuantum => "post-quantum",
            Self::Symmetric => "symmetric",
            Self::Classical => "classical",
            Self::Unknown => "unknown",
        }
    }
}

/// A channel: a way something of the person's leaves the machine or rests
/// on it, sealed or signed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Channel {
    /// The belongings, sealed in their repository.
    Belongings,
    /// The belongings' way to their repository (git over SSH or HTTPS).
    BelongingsTransport,
    /// Encrypt anything, to a key (the person's or someone's).
    SealToKey,
    /// Encrypt anything, with a passphrase.
    SealPassphrase,
    /// The `/private` vault.
    Private,
    /// The system's disk.
    RootDisk,
    /// The HTTPS this system speaks itself (rustls).
    Https,
    /// OpenSSL's TLS: git over HTTPS, curl, what links it.
    OpenSsl,
    /// The system's ssh.
    Ssh,
    /// Google Chrome.
    Chrome,
    /// Syncthing between devices.
    Syncthing,
    /// The tailnet (WireGuard).
    Tailscale,
    /// Tor.
    Tor,
    /// The clipboard shared with ClipCascade.
    ClipCascade,
    /// The machine's secrets (sops-nix) — only when there are some.
    Sops,
    /// The browser extension's signature (CRX3).
    Extension,
    /// F-Droid's signed index.
    FDroid,
    /// The Nix binary cache's signatures.
    NixCache,
    /// Secure Boot's keys.
    SecureBoot,
}

impl Channel {
    /// Every one, in the section's order: what is the person's own first,
    /// then the transports, then the signatures.
    pub const ALL: [Self; 19] = [
        Self::Belongings,
        Self::BelongingsTransport,
        Self::SealToKey,
        Self::SealPassphrase,
        Self::Private,
        Self::RootDisk,
        Self::Https,
        Self::OpenSsl,
        Self::Ssh,
        Self::Chrome,
        Self::Syncthing,
        Self::Tailscale,
        Self::Tor,
        Self::ClipCascade,
        Self::Sops,
        Self::Extension,
        Self::FDroid,
        Self::NixCache,
        Self::SecureBoot,
    ];

    /// Its kebab-case name (`security-quantum-channel-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Belongings => "belongings",
            Self::BelongingsTransport => "belongings-transport",
            Self::SealToKey => "seal-to-key",
            Self::SealPassphrase => "seal-passphrase",
            Self::Private => "private",
            Self::RootDisk => "root-disk",
            Self::Https => "https",
            Self::OpenSsl => "openssl",
            Self::Ssh => "ssh",
            Self::Chrome => "chrome",
            Self::Syncthing => "syncthing",
            Self::Tailscale => "tailscale",
            Self::Tor => "tor",
            Self::ClipCascade => "clipcascade",
            Self::Sops => "sops",
            Self::Extension => "extension",
            Self::FDroid => "f-droid",
            Self::NixCache => "nix-cache",
            Self::SecureBoot => "secure-boot",
        }
    }
}

/// Why a channel has its status — one sentence each
/// (`security-quantum-why-<id>`), with the numbers and names of a
/// [`Reading`] as its arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// Format 2: sealed to the person's ML-KEM-768 + X25519 key.
    BelongingsPq,
    /// A repository this machine has not synced yet: sealed post-quantum,
    /// as the first sync will be.
    BelongingsUnsynced,
    /// No repository yet: one set up now is sealed post-quantum.
    BelongingsNew,
    /// Post-quantum key exchange only, SSH or HTTPS.
    BelongingsTransport,
    /// mlkem768x25519, to the person's key or someone's.
    SealToKey,
    /// scrypt and ChaCha20-Poly1305.
    SealPassphrase,
    /// LUKS2, AES-XTS-512, argon2id.
    Private,
    /// The same, for the system's disk.
    RootDisk,
    /// The system's disk could not be read.
    RootDiskUnread,
    /// Every host heard agreed a post-quantum group (`$hosts`).
    HttpsAll,
    /// Some still agree a classical one (`$classical` of `$hosts`: `$names`).
    HttpsSome,
    /// No host heard yet.
    HttpsNoneHeard,
    /// OpenSSL ≥ 3.5 offers X25519MLKEM768 first (`$version`).
    OpenSsl,
    /// An older OpenSSL (`$version`).
    OpenSslOld,
    /// The ssh offers a post-quantum exchange first (`$version`, `$kex`).
    Ssh,
    /// The ssh offers no post-quantum exchange.
    SshClassical,
    /// A post-quantum exchange is in the ssh's list, after a classical one
    /// (`$kex`): a server that knows both agrees the classical one.
    SshNotFirst,
    /// `ssh -G` did not answer.
    SshUnread,
    /// Chrome ≥ 131 agrees X25519MLKEM768 by default (`$version`).
    Chrome,
    /// An older Chrome (`$version`).
    ChromeOld,
    /// Built with a Go that offers X25519MLKEM768, curves not narrowed
    /// (`$go`).
    Syncthing,
    /// Its source narrows TLS to classical curves.
    SyncthingPinned,
    /// Built with an older Go, or for an older one (`$go`).
    SyncthingOldGo,
    /// WireGuard's handshake: X25519.
    Tailscale,
    /// Tor's circuits: X25519 (ntor).
    Tor,
    /// AES-256-GCM over our TLS.
    ClipCascade,
    /// ssh-to-age: X25519.
    Sops,
    /// CRX3: RSA-2048, a signature.
    Extension,
    /// F-Droid: RSA, a signature, upstream's.
    FDroid,
    /// ed25519 signatures.
    NixCache,
    /// UEFI: RSA-2048.
    SecureBoot,
    /// What was built did not say (the facts are missing).
    NotBuilt,
}

impl Why {
    /// Every one.
    pub const ALL: [Self; 32] = [
        Self::BelongingsPq,
        Self::BelongingsUnsynced,
        Self::BelongingsNew,
        Self::BelongingsTransport,
        Self::SealToKey,
        Self::SealPassphrase,
        Self::Private,
        Self::RootDisk,
        Self::RootDiskUnread,
        Self::HttpsAll,
        Self::HttpsSome,
        Self::HttpsNoneHeard,
        Self::OpenSsl,
        Self::OpenSslOld,
        Self::Ssh,
        Self::SshClassical,
        Self::SshNotFirst,
        Self::SshUnread,
        Self::Chrome,
        Self::ChromeOld,
        Self::Syncthing,
        Self::SyncthingPinned,
        Self::SyncthingOldGo,
        Self::Tailscale,
        Self::Tor,
        Self::ClipCascade,
        Self::Sops,
        Self::Extension,
        Self::FDroid,
        Self::NixCache,
        Self::SecureBoot,
        Self::NotBuilt,
    ];

    /// Its kebab-case name (`security-quantum-why-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::BelongingsPq => "belongings-pq",
            Self::BelongingsUnsynced => "belongings-unsynced",
            Self::BelongingsNew => "belongings-new",
            Self::BelongingsTransport => "belongings-transport",
            Self::SealToKey => "seal-to-key",
            Self::SealPassphrase => "seal-passphrase",
            Self::Private => "private",
            Self::RootDisk => "root-disk",
            Self::RootDiskUnread => "root-disk-unread",
            Self::HttpsAll => "https-all",
            Self::HttpsSome => "https-some",
            Self::HttpsNoneHeard => "https-none-heard",
            Self::OpenSsl => "openssl",
            Self::OpenSslOld => "openssl-old",
            Self::Ssh => "ssh",
            Self::SshClassical => "ssh-classical",
            Self::SshNotFirst => "ssh-not-first",
            Self::SshUnread => "ssh-unread",
            Self::Chrome => "chrome",
            Self::ChromeOld => "chrome-old",
            Self::Syncthing => "syncthing",
            Self::SyncthingPinned => "syncthing-pinned",
            Self::SyncthingOldGo => "syncthing-old-go",
            Self::Tailscale => "tailscale",
            Self::Tor => "tor",
            Self::ClipCascade => "clipcascade",
            Self::Sops => "sops",
            Self::Extension => "extension",
            Self::FDroid => "f-droid",
            Self::NixCache => "nix-cache",
            Self::SecureBoot => "secure-boot",
            Self::NotBuilt => "not-built",
        }
    }
}

/// What this generation ships, written at build (`nix/pq-facts.nix`): the
/// versions — the same strings nixpkgs names them by — and which optional
/// channels it turned on. Every field may be missing: a version a build did
/// not write is [`Why::NotBuilt`], never a guess.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Built {
    /// OpenSSH.
    pub openssh: Option<String>,
    /// OpenSSL.
    pub openssl: Option<String>,
    /// Google Chrome.
    pub chrome: Option<String>,
    /// Syncthing, and how it was built.
    pub syncthing: Option<Syncthing>,
    /// Tor.
    pub tor: Option<String>,
    /// Tailscale.
    pub tailscale: Option<String>,
    /// Which optional channels this configuration turned on — written by the
    /// system's own copy of the facts, which the footer's unit names
    /// (`nix/modules/security.nix`). The workspace's copy, baked into the
    /// binary that every configuration shares, cannot know and has none:
    /// there (the development loop, a photograph) every channel is drawn.
    pub ships: Option<Ships>,
}

/// Which optional channels a configuration turned on. A channel that is off
/// is no row: a table of what this machine does would otherwise speak for a
/// browser that is not installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Ships {
    /// Google Chrome, and with it the browser extension (`ronoctua.browser`).
    pub chrome: bool,
    /// Syncthing (`services.syncthing`).
    pub syncthing: bool,
    /// The tailnet (`services.tailscale`).
    pub tailscale: bool,
    /// Tor (`services.tor`).
    pub tor: bool,
    /// Android apps, and with them F-Droid's index (`virtualisation.waydroid`).
    pub android: bool,
}

/// How Syncthing was built — what decides its TLS's key exchange.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Syncthing {
    /// Its version.
    pub version: String,
    /// The Go it was compiled with: X25519MLKEM768 is on by default from 1.24.
    pub go: String,
    /// The `go` line of its `go.mod`: Go's defaults follow the version a
    /// module says it is written for, so one older than 1.24 is built
    /// without the hybrid even by a newer compiler.
    pub go_mod: String,
    /// Its source sets `CurvePreferences` itself — narrowing the exchange to
    /// the curves it names.
    pub pins_curves: bool,
}

/// What the HTTPS this system speaks agreed, host by host
/// (`ronoctua_http::groups`, counted by whoever reads it).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Tls {
    /// Hosts that agreed a post-quantum group.
    pub quantum: usize,
    /// Hosts that agreed a classical one.
    pub classical: usize,
    /// Some of those, by name — data, as the consent keeps them.
    pub classical_hosts: Vec<String>,
}

/// What the belongings are sealed with now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Belongings {
    /// No repository set.
    #[default]
    None,
    /// A repository, and the post-quantum recipient its last sync left.
    PostQuantum,
    /// A repository, and no recipient left by a sync here yet.
    Unsynced,
}

/// Everything the table is read from — gathered on a door's thread, or
/// handed in whole by a photograph.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Facts {
    /// The belongings.
    pub belongings: Belongings,
    /// The system's disk is encrypted; `None` when it could not be read.
    pub root_disk: Option<bool>,
    /// What our HTTPS hosts agreed.
    pub tls: Tls,
    /// The key exchanges ssh would offer a host, in its order, after its
    /// configuration ([`ssh_kex`]); empty when it did not answer.
    pub ssh_kex: Vec<String>,
    /// sops-nix decrypted the machine's secrets.
    pub sops: bool,
    /// The versions this generation ships.
    pub built: Built,
}

/// One line of the table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Reading {
    /// Which channel.
    pub channel: Channel,
    /// What it does.
    pub status: Status,
    /// Why.
    pub why: Why,
    /// The sentence's words, by name — versions, algorithms and host names
    /// (data).
    pub args: Vec<(String, String)>,
    /// The sentence's counts, by name: NUMBERS, which a plural picks its
    /// branch by — as text, `1` would never be "one"
    /// (`numeric-selectors-get-numbers`).
    pub counts: Vec<(String, u64)>,
}

impl Reading {
    fn of(channel: Channel, status: Status, why: Why) -> Self {
        Self {
            channel,
            status,
            why,
            args: Vec::new(),
            counts: Vec::new(),
        }
    }

    fn with(mut self, name: &str, value: impl Into<String>) -> Self {
        self.args.push((name.to_owned(), value.into()));
        self
    }

    fn counting(mut self, name: &str, n: usize) -> Self {
        self.counts
            .push((name.to_owned(), u64::try_from(n).unwrap_or(u64::MAX)));
        self
    }
}

/// The key exchanges of OpenSSH that hold against a quantum computer.
pub const SSH_PQ_KEX: [&str; 3] = [
    "mlkem768x25519-sha256",
    "sntrup761x25519-sha512",
    "sntrup761x25519-sha512@openssh.com",
];

/// The first OpenSSL whose default groups put X25519MLKEM768 first.
const OPENSSL_PQ: [u32; 2] = [3, 5];

/// The first Chrome that agrees X25519MLKEM768 by default.
const CHROME_PQ: [u32; 1] = [131];

/// The first Go whose `crypto/tls` offers X25519MLKEM768 by default.
const GO_PQ: [u32; 2] = [1, 24];

/// How many classical hosts a reading names.
pub const NAMED_HOSTS: usize = 5;

/// A version's leading numbers: `10.4p1` is `[10, 4]`, `3.6.3` is
/// `[3, 6, 3]`. Pure.
#[must_use]
pub fn numbers_of(version: &str) -> Vec<u32> {
    version
        .split('.')
        .map_while(|part| {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            let n = digits.parse().ok()?;
            // A part with more after its digits (`4p1`) ends the version.
            Some((n, digits.len() == part.len()))
        })
        .scan(true, |going, (n, whole)| {
            if !*going {
                return None;
            }
            *going = whole;
            Some(n)
        })
        .collect()
}

/// Whether `version` is at least `floor`, compared number by number (a
/// missing number is zero). `None` for a version with no number at all.
fn at_least(version: &str, floor: &[u32]) -> Option<bool> {
    let have = numbers_of(version);
    if have.is_empty() {
        return None;
    }
    for (i, want) in floor.iter().enumerate() {
        let got = have.get(i).copied().unwrap_or(0);
        if got != *want {
            return Some(got > *want);
        }
    }
    Some(true)
}

/// A channel whose answer is one fact of the build: post-quantum from
/// `floor` on, classical before it, and not known without the fact.
fn by_version(
    channel: Channel,
    version: Option<&String>,
    floor: &[u32],
    (good, old): (Why, Why),
) -> Reading {
    match version.map(|v| (v, at_least(v, floor))) {
        Some((v, Some(true))) => Reading::of(channel, Status::PostQuantum, good).with("version", v),
        Some((v, Some(false))) => Reading::of(channel, Status::Classical, old).with("version", v),
        _ => Reading::of(channel, Status::Unknown, Why::NotBuilt),
    }
}

/// The table: every channel this machine has, with its status and why.
/// Pure. sops-nix is a row only when it decrypted something, the system's
/// disk only when it is encrypted, and an optional channel only when this
/// generation turned it on ([`Ships`]); every other channel always is — what
/// a channel does is worth knowing before one uses it.
#[must_use]
pub fn channels(facts: &Facts) -> Vec<Reading> {
    Channel::ALL
        .into_iter()
        .filter_map(|channel| reading(channel, facts))
        .collect()
}

/// One channel's line, or `None` for a channel this machine does not have.
fn reading(channel: Channel, facts: &Facts) -> Option<Reading> {
    use Status::{Classical, PostQuantum, Symmetric, Unknown};
    let built = &facts.built;
    // OFF IN THIS GENERATION, no row; not known (the workspace's copy), drawn.
    let ships = |on: fn(&Ships) -> bool| built.ships.as_ref().is_none_or(on);
    Some(match channel {
        Channel::Chrome | Channel::Extension if !ships(|s| s.chrome) => return None,
        Channel::Syncthing if !ships(|s| s.syncthing) => return None,
        Channel::Tailscale if !ships(|s| s.tailscale) => return None,
        Channel::Tor if !ships(|s| s.tor) => return None,
        Channel::FDroid if !ships(|s| s.android) => return None,
        Channel::Belongings => match facts.belongings {
            Belongings::None => Reading::of(channel, PostQuantum, Why::BelongingsNew),
            Belongings::PostQuantum => Reading::of(channel, PostQuantum, Why::BelongingsPq),
            // EVERY REPOSITORY IS SEALED POST-QUANTUM (format 2 is the only
            // one): one this machine has not synced is no weaker.
            Belongings::Unsynced => Reading::of(channel, PostQuantum, Why::BelongingsUnsynced),
        },
        Channel::BelongingsTransport => Reading::of(channel, PostQuantum, Why::BelongingsTransport),
        Channel::SealToKey => Reading::of(channel, PostQuantum, Why::SealToKey),
        Channel::SealPassphrase => Reading::of(channel, Symmetric, Why::SealPassphrase),
        Channel::Private => Reading::of(channel, Symmetric, Why::Private),
        // NOT ENCRYPTED IS NOT A QUANTUM QUESTION: the device's section says
        // it, in the danger colour; here there is nothing to break.
        Channel::RootDisk => match facts.root_disk {
            Some(true) => Reading::of(channel, Symmetric, Why::RootDisk),
            Some(false) => return None,
            None => Reading::of(channel, Unknown, Why::RootDiskUnread),
        },
        Channel::Https => {
            let tls = &facts.tls;
            let hosts = tls.quantum + tls.classical;
            if hosts == 0 {
                Reading::of(channel, Unknown, Why::HttpsNoneHeard)
            } else if tls.classical == 0 {
                Reading::of(channel, PostQuantum, Why::HttpsAll).counting("hosts", hosts)
            } else {
                let names: Vec<&str> = tls
                    .classical_hosts
                    .iter()
                    .take(NAMED_HOSTS)
                    .map(String::as_str)
                    .collect();
                Reading::of(channel, Classical, Why::HttpsSome)
                    .counting("hosts", hosts)
                    .counting("classical", tls.classical)
                    .with("names", names.join(", "))
            }
        }
        Channel::OpenSsl => by_version(
            channel,
            built.openssl.as_ref(),
            &OPENSSL_PQ,
            (Why::OpenSsl, Why::OpenSslOld),
        ),
        // WHAT IT OFFERS, IN ITS ORDER: a client offers its list and the
        // server takes the first one it also knows, so a post-quantum
        // exchange after a classical one is agreed only with a server that
        // knows nothing of the classical one.
        Channel::Ssh => {
            let pq = |k: &String| SSH_PQ_KEX.contains(&k.as_str());
            let version = built.openssh.clone().unwrap_or_default();
            match facts.ssh_kex.first() {
                None => Reading::of(channel, Unknown, Why::SshUnread),
                Some(first) if pq(first) => Reading::of(channel, PostQuantum, Why::Ssh)
                    .with("version", version)
                    .with("kex", first.clone()),
                Some(first) if facts.ssh_kex.iter().any(pq) => {
                    Reading::of(channel, Classical, Why::SshNotFirst)
                        .with("version", version)
                        .with("kex", first.clone())
                }
                Some(_) => {
                    Reading::of(channel, Classical, Why::SshClassical).with("version", version)
                }
            }
        }
        Channel::Chrome => by_version(
            channel,
            built.chrome.as_ref(),
            &CHROME_PQ,
            (Why::Chrome, Why::ChromeOld),
        ),
        Channel::Syncthing => match &built.syncthing {
            None => Reading::of(channel, Unknown, Why::NotBuilt),
            Some(s) if s.pins_curves => Reading::of(channel, Classical, Why::SyncthingPinned)
                .with("version", s.version.clone()),
            Some(s) => {
                let go = at_least(&s.go, &GO_PQ).unwrap_or(false);
                let module = at_least(&s.go_mod, &GO_PQ).unwrap_or(false);
                let why = if go && module {
                    Why::Syncthing
                } else {
                    Why::SyncthingOldGo
                };
                let status = if why == Why::Syncthing {
                    PostQuantum
                } else {
                    Classical
                };
                Reading::of(channel, status, why)
                    .with("version", s.version.clone())
                    .with("go", s.go.clone())
            }
        },
        Channel::Tailscale => Reading::of(channel, Classical, Why::Tailscale)
            .with("version", built.tailscale.clone().unwrap_or_default()),
        Channel::Tor => Reading::of(channel, Classical, Why::Tor)
            .with("version", built.tor.clone().unwrap_or_default()),
        Channel::ClipCascade => Reading::of(channel, Symmetric, Why::ClipCascade),
        Channel::Sops if !facts.sops => return None,
        Channel::Sops => Reading::of(channel, Classical, Why::Sops),
        Channel::Extension => Reading::of(channel, Classical, Why::Extension),
        Channel::FDroid => Reading::of(channel, Classical, Why::FDroid),
        Channel::NixCache => Reading::of(channel, Classical, Why::NixCache),
        Channel::SecureBoot => Reading::of(channel, Classical, Why::SecureBoot),
    })
}

/// The variable naming the build's facts file (a test, a unit may point it
/// elsewhere).
pub const FACTS_VARIABLE: &str = "RONOCTUA_PQ_FACTS";

/// Where the build's facts are: the environment, else what `nix/rust.nix`
/// compiled in.
fn facts_file() -> Option<PathBuf> {
    std::env::var_os(FACTS_VARIABLE)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            option_env!("RONOCTUA_PQ_FACTS")
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
        })
}

/// The versions this generation ships; every field empty when the build
/// wrote none (a `cargo` build outside Nix) or the file does not parse.
#[must_use]
pub fn built() -> Built {
    facts_file()
        .and_then(|f| std::fs::read(f).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// The host `ssh -G` is asked about: a name no `Host` block names (`.invalid`
/// is reserved, RFC 2606), so what comes back is what EVERY host gets — the
/// system's file and the person's `Host *`.
pub const SSH_PROBE_HOST: &str = "post-quantum.invalid";

/// The key exchanges in what `ssh -G` printed, in its order: its
/// `kexalgorithms` line, comma-separated. Empty when there is none. Pure.
#[must_use]
pub fn kex_of(printed: &str) -> Vec<String> {
    printed
        .lines()
        .find_map(|line| {
            let (name, value) = line.trim().split_once(char::is_whitespace)?;
            name.eq_ignore_ascii_case("kexalgorithms").then_some(value)
        })
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|k| !k.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The key exchanges this system's ssh would offer a host, in its order,
/// once its configuration is applied — `ssh -G`, by store path, which reads
/// `/etc/ssh/ssh_config` and `~/.ssh/config` and connects nowhere. Not `ssh
/// -Q kex`: that lists what the program was BUILT with, and a configuration
/// that names only `curve25519-sha256` would still read as post-quantum.
/// Empty when there is no ssh or it did not answer. BLOCKS for as long as ssh
/// takes to read its files: a door's thread.
#[must_use]
pub fn ssh_kex() -> Vec<String> {
    let program = std::env::var_os("RONOCTUA_SSH")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            option_env!("RONOCTUA_SSH")
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
        });
    let Some(program) = program else {
        return Vec::new();
    };
    std::process::Command::new(program)
        .args(["-G", SSH_PROBE_HOST])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| kex_of(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// Where sops-nix puts what it decrypted.
pub const SOPS_SECRETS: &str = "/run/secrets";

/// Whether sops-nix decrypted the machine's secrets this boot.
#[must_use]
pub fn sops_in_use() -> bool {
    std::path::Path::new(SOPS_SECRETS).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Built {
        Built {
            openssh: Some("10.4p1".into()),
            openssl: Some("3.6.3".into()),
            chrome: Some("151.0.7922.71".into()),
            syncthing: Some(Syncthing {
                version: "2.1.1".into(),
                go: "1.26.5".into(),
                go_mod: "1.25.0".into(),
                pins_curves: false,
            }),
            tor: Some("0.4.9.11".into()),
            tailscale: Some("1.98.10".into()),
            ships: None,
        }
    }

    /// Every optional channel on, or every one off.
    fn ships(on: bool) -> Ships {
        Ships {
            chrome: on,
            syncthing: on,
            tailscale: on,
            tor: on,
            android: on,
        }
    }

    #[test]
    fn a_version_is_read_by_its_leading_numbers() {
        assert_eq!(numbers_of("10.4p1"), [10, 4]);
        assert_eq!(numbers_of("3.6.3"), [3, 6, 3]);
        assert_eq!(numbers_of("151.0.7922.71"), [151, 0, 7922, 71]);
        assert_eq!(numbers_of("1.25.0"), [1, 25, 0]);
        assert!(numbers_of("unstable").is_empty());
        assert_eq!(at_least("3.5", &OPENSSL_PQ), Some(true));
        assert_eq!(at_least("3.4.9", &OPENSSL_PQ), Some(false));
        assert_eq!(at_least("4", &OPENSSL_PQ), Some(true));
        assert_eq!(at_least("1.23.12", &GO_PQ), Some(false));
        assert_eq!(at_least("x", &GO_PQ), None);
    }

    /// FAILS IF: a channel is missing from the table, comes twice, or has
    /// no status — under any facts — or sops-nix shows without secrets.
    #[test]
    fn every_channel_says_its_status_under_every_fact() {
        let mut matrix = Vec::new();
        for belongings in [
            Belongings::None,
            Belongings::PostQuantum,
            Belongings::Unsynced,
        ] {
            for root_disk in [None, Some(true), Some(false)] {
                for (quantum, classical) in [(0, 0), (3, 0), (2, 5)] {
                    for kex in [
                        vec![],
                        vec!["curve25519-sha256".to_owned()],
                        vec![SSH_PQ_KEX[0].to_owned()],
                        vec!["curve25519-sha256".to_owned(), SSH_PQ_KEX[0].to_owned()],
                    ] {
                        for sops in [false, true] {
                            for built in [Built::default(), shipped()] {
                                for on in [None, Some(ships(true)), Some(ships(false))] {
                                    matrix.push(Facts {
                                        belongings,
                                        root_disk,
                                        tls: Tls {
                                            quantum,
                                            classical,
                                            classical_hosts: vec!["a.example".into(); classical],
                                        },
                                        ssh_kex: kex.clone(),
                                        sops,
                                        built: Built {
                                            ships: on,
                                            ..built.clone()
                                        },
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        for facts in &matrix {
            let table = channels(facts);
            // OFF IN THE GENERATION is no row; unknown (no `ships`) is drawn.
            let on =
                |is: fn(&Ships) -> bool| usize::from(facts.built.ships.as_ref().is_none_or(is));
            for channel in Channel::ALL {
                let n = table.iter().filter(|r| r.channel == channel).count();
                let expected = match channel {
                    Channel::Sops => usize::from(facts.sops),
                    Channel::RootDisk => usize::from(facts.root_disk != Some(false)),
                    Channel::Chrome | Channel::Extension => on(|s| s.chrome),
                    Channel::Syncthing => on(|s| s.syncthing),
                    Channel::Tailscale => on(|s| s.tailscale),
                    Channel::Tor => on(|s| s.tor),
                    Channel::FDroid => on(|s| s.android),
                    _ => 1,
                };
                assert_eq!(n, expected, "{channel:?} under {facts:?}");
            }
        }
    }

    /// FAILS IF: a channel this generation did not turn on is drawn — the
    /// minimal profile has no Chrome — or one it did is left out.
    #[test]
    fn a_channel_the_generation_did_not_turn_on_is_no_row() {
        let minimal = Facts {
            built: Built {
                ships: Some(Ships {
                    chrome: false,
                    ..ships(true)
                }),
                ..shipped()
            },
            ..Facts::default()
        };
        let drawn: Vec<Channel> = channels(&minimal).iter().map(|r| r.channel).collect();
        for gone in [Channel::Chrome, Channel::Extension] {
            assert!(!drawn.contains(&gone), "{gone:?} drawn without a browser");
        }
        for kept in [
            Channel::Syncthing,
            Channel::Tailscale,
            Channel::Tor,
            Channel::FDroid,
        ] {
            assert!(drawn.contains(&kept), "{kept:?} left out");
        }
    }

    /// FAILS IF: ssh is called post-quantum for what it was BUILT with and not
    /// for what its configuration offers, or "first" is said of an exchange
    /// that comes after a classical one — or `ssh -G`'s line is misread.
    #[test]
    fn ssh_is_read_in_the_order_its_configuration_offers() {
        let printed = "user p\nhostname post-quantum.invalid\n\
            kexalgorithms curve25519-sha256,mlkem768x25519-sha256\nport 22\n";
        assert_eq!(
            kex_of(printed),
            ["curve25519-sha256", "mlkem768x25519-sha256"]
        );
        assert!(kex_of("port 22\n").is_empty());
        let ssh = |kex: &[&str]| {
            let facts = Facts {
                ssh_kex: kex.iter().map(|k| (*k).to_owned()).collect(),
                built: shipped(),
                ..Facts::default()
            };
            channels(&facts)
                .into_iter()
                .find(|r| r.channel == Channel::Ssh)
                .map(|r| (r.status, r.why, r.args))
                .unwrap()
        };
        let (status, why, args) = ssh(&["sntrup761x25519-sha512", "curve25519-sha256"]);
        assert_eq!((status, why), (Status::PostQuantum, Why::Ssh));
        assert!(args.contains(&("kex".to_owned(), "sntrup761x25519-sha512".to_owned())));
        let (status, why, _) = ssh(&kex_of(printed)
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>());
        assert_eq!((status, why), (Status::Classical, Why::SshNotFirst));
        let (status, why, _) = ssh(&["curve25519-sha256"]);
        assert_eq!((status, why), (Status::Classical, Why::SshClassical));
        assert_eq!(ssh(&[]).1, Why::SshUnread);
    }

    #[test]
    fn what_this_generation_ships_is_post_quantum_where_it_can_be() {
        let facts = Facts {
            belongings: Belongings::PostQuantum,
            root_disk: Some(true),
            tls: Tls {
                quantum: 4,
                classical: 0,
                classical_hosts: Vec::new(),
            },
            ssh_kex: SSH_PQ_KEX.map(ToOwned::to_owned).to_vec(),
            sops: false,
            built: shipped(),
        };
        let status = |c: Channel| {
            channels(&facts)
                .into_iter()
                .find(|r| r.channel == c)
                .map(|r| (r.status, r.why))
        };
        assert_eq!(
            status(Channel::OpenSsl),
            Some((Status::PostQuantum, Why::OpenSsl))
        );
        assert_eq!(
            status(Channel::Chrome),
            Some((Status::PostQuantum, Why::Chrome))
        );
        assert_eq!(
            status(Channel::Syncthing),
            Some((Status::PostQuantum, Why::Syncthing))
        );
        assert_eq!(status(Channel::Ssh), Some((Status::PostQuantum, Why::Ssh)));
        assert_eq!(
            status(Channel::Https),
            Some((Status::PostQuantum, Why::HttpsAll))
        );
        assert_eq!(status(Channel::Tor), Some((Status::Classical, Why::Tor)));
        assert_eq!(
            status(Channel::RootDisk),
            Some((Status::Symmetric, Why::RootDisk))
        );
        // A SYNCTHING THAT NARROWS ITS CURVES, or written for an older Go, is
        // classical whatever compiled it.
        let mut older = facts.clone();
        if let Some(s) = older.built.syncthing.as_mut() {
            s.go_mod = "1.22".into();
        }
        assert_eq!(
            channels(&older)
                .iter()
                .find(|r| r.channel == Channel::Syncthing)
                .map(|r| r.why),
            Some(Why::SyncthingOldGo)
        );
        let mut pinned = facts;
        if let Some(s) = pinned.built.syncthing.as_mut() {
            s.pins_curves = true;
        }
        assert_eq!(
            channels(&pinned)
                .iter()
                .find(|r| r.channel == Channel::Syncthing)
                .map(|r| r.status),
            Some(Status::Classical)
        );
    }

    #[test]
    fn hosts_that_agree_a_classical_key_are_named_and_counted() {
        let facts = Facts {
            tls: Tls {
                quantum: 1,
                classical: 7,
                classical_hosts: (0..7).map(|n| format!("h{n}.example")).collect(),
            },
            ..Facts::default()
        };
        let https = channels(&facts)
            .into_iter()
            .find(|r| r.channel == Channel::Https)
            .unwrap();
        assert_eq!(
            (https.status, https.why),
            (Status::Classical, Why::HttpsSome)
        );
        let arg = |n: &str| {
            https
                .args
                .iter()
                .find(|(k, _)| k == n)
                .map(|(_, v)| v.clone())
        };
        // COUNTS ARE NUMBERS, for the plural's sake — never text.
        let count = |n: &str| https.counts.iter().find(|(k, _)| k == n).map(|(_, v)| *v);
        assert_eq!(count("hosts"), Some(8));
        assert_eq!(count("classical"), Some(7));
        assert_eq!(arg("hosts"), None);
        assert_eq!(
            arg("names").map(|n| n.split(", ").count()),
            Some(NAMED_HOSTS)
        );
    }

    #[test]
    fn the_build_s_facts_are_read_and_a_missing_one_is_not_built() {
        let text = r#"{"openssh":"10.4p1","openssl":"3.6.3","chrome":"151.0.7922.71",
            "syncthing":{"version":"2.1.1","go":"1.26.5","go-mod":"1.25.0","pins-curves":false},
            "tor":"0.4.9.11","tailscale":"1.98.10"}"#;
        let built: Built = serde_json::from_str(text).unwrap();
        assert_eq!(built, shipped());
        // THE SYSTEM'S COPY says what it turned on — and a word it does not
        // know is no copy at all, never a guess.
        let system = text.replace(
            r#""tailscale":"1.98.10"}"#,
            r#""tailscale":"1.98.10","ships":{"android":false,"chrome":false,"syncthing":true,"tailscale":true,"tor":true}}"#,
        );
        let built: Built = serde_json::from_str(&system).unwrap();
        assert_eq!(
            built.ships,
            Some(Ships {
                chrome: false,
                android: false,
                ..ships(true)
            })
        );
        let unknown = system.replace(r#""tor":true"#, r#""tor":true,"vpn":true"#);
        assert!(serde_json::from_str::<Built>(&unknown).is_err());
        let none = Facts::default();
        assert_eq!(
            channels(&none)
                .iter()
                .find(|r| r.channel == Channel::Chrome)
                .map(|r| r.why),
            Some(Why::NotBuilt)
        );
        for why in Why::ALL {
            assert!(Why::ALL.iter().filter(|w| w.id() == why.id()).count() == 1);
        }
    }
}
