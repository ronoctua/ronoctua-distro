//! Incus — the one crate that talks to it.
//!
//! # REST over the unix socket; the CLI only where it must be
//!
//! Reads and actions go to the daemon's REST API over its unix socket (the
//! framer is `ronoctua-http`'s), and `/1.0/events` is a WebSocket over the
//! same socket: typed JSON, operations with progress, events instead of
//! polling, no process per call. The CLI (`RONOCTUA_INCUS`, read at run time
//! from the environment, never baked: its closure is QEMU's and LXC's) is for
//! what needs a terminal or the client's own configuration: `console`,
//! `exec`/shell, `console --type=vga` (remote-viewer), the client's remotes
//! and a remote's image catalogue.
//!
//! # Nothing reaches the network by itself
//!
//! The preseed sets `images.auto_update_interval = 0`, and every call that
//! makes the daemon contact a remote takes a consent [`Pass`] for that
//! remote's host ([`Incus::create`], [`Incus::catalogue`], and the queue's
//! [`Job::Launch`] and [`Job::ImageCopy`], whose child mints its own from
//! `consent.json`). A remote's hosts are ALL the hosts its client reaches by
//! its own constants ([`hosts_of`]: Docker Hub's address is served from two
//! more), and each must be let. A pass missing for one of them is refused
//! BEFORE the socket is opened ([`Why::NotLet`]).
//!
//! # The shapes are the pinned daemon's
//!
//! Every parser reads what the pinned Incus prints (`shared/api` of the
//! package `nix/modules/incus.nix` pins): an instance's `type` is `container`
//! or `virtual-machine`, a status is told by its `status_code`, a snapshot's
//! name may come with its instance's in front. The fixtures in
//! `fixtures.rs` are those shapes; `tests/incus-vm.nix` asks the same
//! questions of a booted daemon and holds it to every key read here.
//!
//! # Blocks
//!
//! Every call waits on the daemon: a thread, never the event loop. The
//! actions that are operations ([`Incus::act`], the snapshots) wait for theirs
//! to end; what lasts minutes (a launch from a remote image, an export) goes
//! on the queue as a [`Job`].
//!
//! # Two layers
//!
//! Without features, the wire: the queue's [`Job`], the [`Line`]s
//! `ronoctua-incus-job` prints, [`Why`] and the shapes. `client` adds the
//! daemon ([`Incus`], [`Events`]); `cli` the job's binary.

#![forbid(unsafe_code)]

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[cfg(feature = "client")]
mod cli;
#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
mod events;
#[cfg(all(test, feature = "client"))]
mod fixtures;
#[cfg(feature = "client")]
pub mod job;
mod line;
#[cfg(feature = "client")]
mod parse;
#[cfg(feature = "client")]
mod rest;

#[cfg(feature = "client")]
pub use client::Incus;
#[cfg(feature = "client")]
pub use events::{Event, Events, Stopper, Topic};
pub use line::{Line, step_of};
#[cfg(feature = "client")]
pub use rest::{OpStatus, Operation};

/// The variable naming the CLI's store path (`nix/modules/incus.nix` sets
/// it where Incus is on, and nowhere else).
pub const VARIABLE: &str = "RONOCTUA_INCUS";

/// The variable naming `ronoctua-incus-job`'s store path, for the queue.
pub const JOB_VARIABLE: &str = "RONOCTUA_INCUS_JOB";

/// The daemon's own variable for its socket, which its CLI honours too: when
/// the session sets it, both talk to the same daemon.
pub const SOCKET_VARIABLE: &str = "INCUS_SOCKET";

/// Where the daemon listens, as the NixOS module declares it (`incus.socket`,
/// group `incus-admin`, mode 0660).
pub const SOCKET: &str = "/var/lib/incus/unix.socket";

/// Why a call did not happen, as a closed set (`incus-refused-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// Incus is not on this machine (the minimal profile), or not running.
    NoIncus,
    /// The person is not in `incus-admin` — or is, since a login that has not
    /// happened yet.
    NotInGroup,
    /// The instance, image or snapshot is gone.
    Gone,
    /// Another operation holds it.
    Busy,
    /// A host the remote's client asks ([`hosts_of`]) was not let in
    /// `consent.json`.
    NotLet,
    /// A name, a path or a limit Incus does not take, refused before asking.
    Invalid,
    /// The daemon said no, or a program failed.
    Failed,
}

impl Why {
    /// Every one.
    pub const ALL: [Self; 7] = [
        Self::NoIncus,
        Self::NotInGroup,
        Self::Gone,
        Self::Busy,
        Self::NotLet,
        Self::Invalid,
        Self::Failed,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoIncus => "no-incus",
            Self::NotInGroup => "not-in-group",
            Self::Gone => "gone",
            Self::Busy => "busy",
            Self::NotLet => "not-let",
            Self::Invalid => "invalid",
            Self::Failed => "failed",
        }
    }

    /// The one with this name.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.id() == id)
    }
}

/// A refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
}

impl Refusal {
    /// A refusal for this reason.
    #[must_use]
    pub const fn of(why: Why) -> Self {
        Self { why }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.why.id())
    }
}

impl std::error::Error for Refusal {}

/// A container or a virtual machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A system container.
    Container,
    /// A virtual machine.
    Vm,
}

impl Kind {
    /// The daemon's word for it (`type`).
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Container => "container",
            Self::Vm => "virtual-machine",
        }
    }

    /// The kind the daemon's word says; `None` for any other.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        match word {
            "container" => Some(Self::Container),
            "virtual-machine" => Some(Self::Vm),
            _ => None,
        }
    }
}

/// An instance's state, as the daemon says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Running.
    Running,
    /// Stopped.
    Stopped,
    /// Frozen.
    Frozen,
    /// In an error state.
    Error,
    /// Anything else the daemon said (starting, stopping…).
    Other,
}

impl Status {
    /// The status a daemon's `status_code` says (`shared/api/status_code.go`).
    #[must_use]
    pub const fn of_code(code: i64) -> Self {
        match code {
            103 => Self::Running,
            102 => Self::Stopped,
            110 => Self::Frozen,
            112 => Self::Error,
            _ => Self::Other,
        }
    }
}

/// One instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instance {
    /// Its name.
    pub name: String,
    /// Container or VM.
    pub kind: Kind,
    /// Its state.
    pub status: Status,
    /// The project it is in.
    pub project: String,
    /// Its description, data.
    pub description: String,
    /// Its profiles.
    pub profiles: Vec<String>,
    /// When it was made (RFC 3339, the daemon's).
    pub created_at: String,
    /// What its image said it was (`image.description`), data.
    pub image: String,
    /// Its limits, as its EXPANDED configuration says them (profiles
    /// included).
    pub limits: Limits,
    /// The limits its OWN configuration sets — the ones [`Incus::set_limit`]
    /// changes. A limit in [`Instance::limits`] that is not here is a
    /// profile's.
    #[serde(default)]
    pub own_limits: Limits,
    /// Its live state, when it was asked with it (`recursion=2`).
    pub state: Option<State>,
    /// Its snapshots, when they were asked with it.
    pub snapshots: Vec<Snapshot>,
}

/// An instance's live state.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct State {
    /// IPv4 addresses (global scope, the loopback left out).
    pub ipv4: Vec<String>,
    /// IPv6 addresses (global scope).
    pub ipv6: Vec<String>,
    /// CPU time used, in nanoseconds.
    pub cpu_ns: u64,
    /// Memory in use, in bytes.
    pub memory_bytes: u64,
    /// Processes.
    pub processes: u64,
    /// The root disk's use, in bytes, when the driver measures it.
    pub disk_bytes: u64,
}

/// A local image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    /// Its fingerprint.
    pub fingerprint: String,
    /// Its aliases.
    pub aliases: Vec<String>,
    /// What it says it is.
    pub description: String,
    /// Container or VM.
    pub kind: Kind,
    /// Its size, in bytes.
    pub size_bytes: u64,
}

/// A profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Its name.
    pub name: String,
    /// Its description.
    pub description: String,
}

/// A network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Network {
    /// Its name (`incusbr0`).
    pub name: String,
    /// Whether Incus manages it.
    pub managed: bool,
    /// Its type (`bridge`, `physical`…), as the daemon says it.
    pub kind: String,
}

/// A storage pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pool {
    /// Its name.
    pub name: String,
    /// Its driver (`btrfs`).
    pub driver: String,
}

/// A storage volume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Volume {
    /// Its name.
    pub name: String,
    /// Its type (`custom`, `container`…), as the daemon says it.
    pub kind: String,
}

/// A project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Its name.
    pub name: String,
    /// Its description.
    pub description: String,
}

/// A snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Its name.
    pub name: String,
    /// When it was taken (RFC 3339, the daemon's).
    pub created_at: String,
}

/// What an instance can be told to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// Start.
    Start,
    /// Stop.
    Stop,
    /// Restart.
    Restart,
    /// Freeze.
    Freeze,
    /// Unfreeze.
    Unfreeze,
    /// Delete (after a question whose first answer is "no", the door's) — a
    /// running instance is stopped first.
    Delete,
}

impl Action {
    /// Every one.
    pub const ALL: [Self; 6] = [
        Self::Start,
        Self::Stop,
        Self::Restart,
        Self::Freeze,
        Self::Unfreeze,
        Self::Delete,
    ];

    /// The daemon's `action` for a state change; `None` for [`Action::Delete`],
    /// which is a `DELETE` and not a state.
    #[must_use]
    pub const fn state_word(self) -> Option<&'static str> {
        match self {
            Self::Start => Some("start"),
            Self::Stop => Some("stop"),
            Self::Restart => Some("restart"),
            Self::Freeze => Some("freeze"),
            Self::Unfreeze => Some("unfreeze"),
            Self::Delete => None,
        }
    }
}

/// An instance's limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Limits {
    /// CPUs, when limited.
    pub cpu: Option<u32>,
    /// Memory, when limited, in bytes.
    pub memory_bytes: Option<u64>,
    /// Processes, when limited (containers only: a VM's kernel counts its
    /// own).
    pub processes: Option<u64>,
}

/// ONE of an instance's limits, as it is to be — the others are left as
/// they are ([`Incus::set_limit`]). `None` takes the instance's own away
/// (a profile's, if there is one, counts again).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LimitChange {
    /// Its processors.
    Cpu(Option<u32>),
    /// Its memory, in bytes.
    MemoryBytes(Option<u64>),
    /// Its processes (a container's).
    Processes(Option<u64>),
}

/// Which protocol a remote speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Protocol {
    /// Simplestreams (`images.linuxcontainers.org`).
    SimpleStreams,
    /// An OCI registry.
    Oci,
}

impl Protocol {
    /// The daemon's word for it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::SimpleStreams => "simplestreams",
            Self::Oci => "oci",
        }
    }

    /// The protocol the daemon's word says; `None` for any other (`incus`,
    /// the protocol of a daemon — this one is read through its socket).
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        match word {
            "simplestreams" => Some(Self::SimpleStreams),
            "oci" => Some(Self::Oci),
            _ => None,
        }
    }
}

/// Where a new instance's image comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "from")]
pub enum Source {
    /// A remote's image, by alias.
    Remote {
        /// The server's address (`https://images.linuxcontainers.org`).
        server: String,
        /// Its protocol.
        protocol: Protocol,
        /// The alias (`debian/13`).
        alias: String,
    },
    /// A local image, by fingerprint.
    Local {
        /// The fingerprint.
        fingerprint: String,
    },
}

impl Source {
    /// Every host whose consent this source needs ([`hosts_of`]); none for a
    /// local image. A door's consent row lets ALL of them: a launch is
    /// refused ([`Why::NotLet`]) while one is not let.
    #[must_use]
    pub fn needs_hosts(&self) -> Vec<String> {
        match self {
            Self::Remote {
                server, protocol, ..
            } => hosts_of(server, *protocol),
            Self::Local { .. } => Vec::new(),
        }
    }
}

/// A new instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Create {
    /// Its name.
    pub name: String,
    /// Container or VM.
    pub kind: Kind,
    /// Its image.
    pub source: Source,
    /// Its profiles; EMPTY means the daemon's `default` (an empty list sent
    /// as such would mean no profile, and no root disk).
    pub profiles: Vec<String>,
    /// Its limits.
    pub limits: Limits,
}

/// A client-side remote (`incus remote list`), one that serves images.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remote {
    /// Its name.
    pub name: String,
    /// Its address.
    pub address: String,
    /// Its protocol.
    pub protocol: Protocol,
    /// Whether it is public.
    pub public: bool,
}

/// An image of a remote's catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offered {
    /// Its alias — the shortest of its aliases (`debian/13`).
    pub alias: String,
    /// What it says it is.
    pub description: String,
    /// Container or VM.
    pub kind: Kind,
    /// Its fingerprint.
    pub fingerprint: String,
    /// Its size, in bytes, as the remote says it.
    pub size_bytes: u64,
}

/// A console's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Console {
    /// Text, in our terminal.
    Text,
    /// The VGA screen, in remote-viewer.
    Vga,
}

/// Long work, on the transfer queue (`Work::Incus`), run by
/// `ronoctua-incus-job`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "job")]
pub enum Job {
    /// Create and start an instance.
    Launch {
        /// What.
        create: Create,
    },
    /// Copy a remote's image here.
    ImageCopy {
        /// The server.
        server: String,
        /// Its protocol.
        protocol: Protocol,
        /// The alias.
        alias: String,
        /// Which of the alias's images: a container's (the daemon's default
        /// when absent) or a VM's.
        #[serde(default)]
        kind: Option<Kind>,
    },
    /// Export an instance to a file (never over one: a file that is there is
    /// refused).
    Export {
        /// The instance.
        name: String,
        /// The file.
        to: PathBuf,
    },
    /// Import an instance from a file.
    Import {
        /// The file.
        from: PathBuf,
    },
    /// Publish an instance as an image.
    Publish {
        /// The instance.
        name: String,
        /// The image's alias.
        alias: String,
    },
    /// Push a file into an instance.
    Push {
        /// The instance.
        name: String,
        /// The file here.
        from: PathBuf,
        /// The path there.
        to: String,
    },
    /// Pull a file out of an instance (never over one here).
    Pull {
        /// The instance.
        name: String,
        /// The path there.
        from: String,
        /// The file here.
        to: PathBuf,
    },
}

impl Job {
    /// The stem of its text key (`transfers-work-<word>`).
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Launch { .. } => "incus-launch",
            Self::ImageCopy { .. } => "incus-image-copy",
            Self::Export { .. } => "incus-export",
            Self::Import { .. } => "incus-import",
            Self::Publish { .. } => "incus-publish",
            Self::Push { .. } => "incus-push",
            Self::Pull { .. } => "incus-pull",
        }
    }

    /// Every stem.
    pub const WORDS: [&'static str; 7] = [
        "incus-launch",
        "incus-image-copy",
        "incus-export",
        "incus-import",
        "incus-publish",
        "incus-push",
        "incus-pull",
    ];

    /// Every host whose consent the job needs before the daemon is asked
    /// anything: the remote's ([`hosts_of`]) of a launch from a remote image
    /// and of an image copy. None for everything that stays on this machine.
    #[must_use]
    pub fn needs_hosts(&self) -> Vec<String> {
        match self {
            Self::Launch { create } => create.source.needs_hosts(),
            Self::ImageCopy {
                server, protocol, ..
            } => hosts_of(server, *protocol),
            Self::Export { .. }
            | Self::Import { .. }
            | Self::Publish { .. }
            | Self::Push { .. }
            | Self::Pull { .. } => Vec::new(),
        }
    }
}

/// Where a job is, read from one line `ronoctua-incus-job` printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    /// A percentage the daemon reported.
    Percent(u8),
    /// Bytes so far, of a total when known.
    Bytes {
        /// So far.
        done: u64,
        /// Of.
        total: Option<u64>,
    },
}

/// The host a remote server's address names, lower-case:
/// `https://images.linuxcontainers.org` → `images.linuxcontainers.org`.
/// `None` for an address that is not `http(s)://` with a host — `unix://`
/// is this machine, and nothing to consent to.
#[must_use]
pub fn host_of(server: &str) -> Option<String> {
    let rest = server
        .strip_prefix("https://")
        .or_else(|| server.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    // A user before the host is never ours to send, and an IPv6 literal is
    // not a name a person lets.
    if authority.contains('@') || authority.starts_with('[') {
        return None;
    }
    let host = authority.split(':').next().unwrap_or("");
    let fits = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '.';
    (!host.is_empty() && host.chars().all(fits)).then(|| host.to_ascii_lowercase())
}

/// Docker Hub's hosts, which its address does not name.
const DOCKER_HUB: [&str; 2] = ["registry-1.docker.io", "auth.docker.io"];

/// The hosts an OCI registry's client reaches BY ITS OWN CONSTANTS, besides
/// the one the registry's address names. The daemon pulls an OCI image
/// through `skopeo` (`containers/image`), which serves Docker Hub's address
/// (`docker.io`) from `registry-1.docker.io`, whose tokens come from
/// `auth.docker.io`. Every other registry serves its manifests and tokens
/// from its own host. Where a registry then SENDS the client for a layer (a
/// CDN) is a redirect, the host's own answer, which is how
/// `ronoctua_http::web` follows one under the first host's pass.
const OCI_HOSTS: [(&str, [&str; 2]); 3] = [
    ("docker.io", DOCKER_HUB),
    ("index.docker.io", DOCKER_HUB),
    ("registry-1.docker.io", DOCKER_HUB),
];

/// Every host the daemon (or its CLI) reaches for a remote's images: the
/// address's ([`host_of`]), and, for an OCI registry, the hosts its client
/// reaches by its own constants (Docker Hub's three). Without duplicates,
/// the address's first. An address that names no host needs a consent no
/// pass gives: `[""]`. Pure.
#[must_use]
pub fn hosts_of(server: &str, protocol: Protocol) -> Vec<String> {
    let Some(host) = host_of(server) else {
        return vec![String::new()];
    };
    let mut hosts = vec![host.clone()];
    if protocol == Protocol::Oci
        && let Some((_, others)) = OCI_HOSTS.iter().find(|(named, _)| *named == host)
    {
        for other in others {
            if !hosts.iter().any(|h| h == other) {
                hosts.push((*other).to_owned());
            }
        }
    }
    hosts
}

/// Whether Incus takes this as an instance's name: 1 to 63 letters, digits
/// and hyphens, starting with a letter and not ending with a hyphen. Pure — a
/// door asks it before queueing, and every call here refuses a name that is
/// not one ([`Why::Invalid`]) before it reaches a path.
#[must_use]
pub fn is_instance_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=63).contains(&bytes.len())
        && bytes[0].is_ascii_alphabetic()
        && bytes[bytes.len() - 1] != b'-'
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
}

/// Whether Incus takes this as a snapshot's name, which is freer than an
/// instance's: the daemon refuses only a space and a `/` in it, so
/// `pre_upgrade`, `v1.0` and a dated `snapshots.pattern` are names it makes.
/// Refused here besides those: an empty name, `.` and `..`, a control
/// character, and more than a file name's 255 bytes. Pure — and a name that
/// passes still goes into a path percent-encoded.
#[must_use]
pub fn is_snapshot_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && !name.chars().any(|c| c == ' ' || c == '/' || c.is_control())
}

/// The CLI's arguments for a console (after the CLI's own path). Pure.
#[must_use]
pub fn console_arguments(name: &str, console: Console) -> Vec<String> {
    let mut args = vec!["console".to_owned(), name.to_owned()];
    if console == Console::Vga {
        args.push("--type=vga".to_owned());
    }
    args
}

/// The CLI's arguments for a login shell in an instance — what the CLI's own
/// `shell` alias expands to. Pure.
#[must_use]
pub fn shell_arguments(name: &str) -> Vec<String> {
    ["exec", name, "--", "su", "-l"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_job_has_its_word_and_crosses_the_wire_whole() {
        let jobs = [
            Job::Launch {
                create: Create {
                    name: "web".to_owned(),
                    kind: Kind::Container,
                    source: Source::Remote {
                        server: "https://images.linuxcontainers.org".to_owned(),
                        protocol: Protocol::SimpleStreams,
                        alias: "debian/13".to_owned(),
                    },
                    profiles: Vec::new(),
                    limits: Limits::default(),
                },
            },
            Job::ImageCopy {
                server: "https://images.linuxcontainers.org".to_owned(),
                protocol: Protocol::SimpleStreams,
                alias: "alpine/3.22".to_owned(),
                kind: Some(Kind::Vm),
            },
            Job::Export {
                name: "web".to_owned(),
                to: "/home/p/web.tar.gz".into(),
            },
            Job::Import {
                from: "/x.tar".into(),
            },
            Job::Publish {
                name: "a".to_owned(),
                alias: "b".to_owned(),
            },
            Job::Push {
                name: "a".to_owned(),
                from: "/tmp/x".into(),
                to: "/root/x".to_owned(),
            },
            Job::Pull {
                name: "a".to_owned(),
                from: "/etc/hosts".to_owned(),
                to: "/home/p/hosts".into(),
            },
        ];
        let mut words: Vec<&str> = jobs.iter().map(Job::word).collect();
        words.sort_unstable();
        let mut all = Job::WORDS.to_vec();
        all.sort_unstable();
        assert_eq!(words, all, "one job per word, and a word per job");
        for job in &jobs {
            let wire = serde_json::to_string(job).unwrap();
            assert_eq!(&serde_json::from_str::<Job>(&wire).unwrap(), job, "{wire}");
        }
        // AN OLDER LINE WITHOUT THE IMAGE'S KIND STILL READS: the container's.
        let older: Job = serde_json::from_str(
            r#"{"job":"image-copy","server":"https://x.example","protocol":"oci","alias":"nginx"}"#,
        )
        .unwrap();
        assert!(matches!(older, Job::ImageCopy { kind: None, .. }));
    }

    #[test]
    fn only_a_remote_needs_consent_and_its_host_is_the_address_s() {
        assert_eq!(
            host_of("https://Images.LinuxContainers.org/"),
            Some("images.linuxcontainers.org".to_owned())
        );
        assert_eq!(
            host_of("https://docker.io:443/v2"),
            Some("docker.io".to_owned())
        );
        assert_eq!(host_of("unix://"), None);
        assert_eq!(host_of("https://user@evil.example"), None);
        assert_eq!(host_of("https://[::1]:8443"), None);
        assert_eq!(host_of("images.linuxcontainers.org"), None, "no scheme");
        let launch = Job::Launch {
            create: Create {
                name: "a".to_owned(),
                kind: Kind::Vm,
                source: Source::Local {
                    fingerprint: "abc".to_owned(),
                },
                profiles: Vec::new(),
                limits: Limits::default(),
            },
        };
        assert!(launch.needs_hosts().is_empty(), "a local image asks nobody");
        let copy = Job::ImageCopy {
            server: "ftp://x".to_owned(),
            protocol: Protocol::Oci,
            alias: "a".to_owned(),
            kind: None,
        };
        assert_eq!(
            copy.needs_hosts(),
            [""],
            "an address with no host needs a consent no pass gives"
        );
    }

    /// FAILS IF: an OCI pull from Docker Hub needs the consent of `docker.io`
    /// alone, when its client asks `registry-1.docker.io` and
    /// `auth.docker.io` by its own constants — or a simplestreams remote, or
    /// another registry, is said to need a host it does not ask.
    #[test]
    fn a_remote_needs_every_host_its_client_asks() {
        assert_eq!(
            hosts_of("https://docker.io", Protocol::Oci),
            ["docker.io", "registry-1.docker.io", "auth.docker.io"]
        );
        assert_eq!(
            hosts_of("https://registry-1.docker.io", Protocol::Oci),
            ["registry-1.docker.io", "auth.docker.io"],
            "no host twice"
        );
        assert_eq!(hosts_of("https://ghcr.io", Protocol::Oci), ["ghcr.io"]);
        assert_eq!(
            hosts_of("https://docker.io", Protocol::SimpleStreams),
            ["docker.io"],
            "the constants are the OCI client's"
        );
        assert_eq!(
            hosts_of(
                "https://images.linuxcontainers.org",
                Protocol::SimpleStreams
            ),
            ["images.linuxcontainers.org"]
        );
        let hub = Job::Launch {
            create: Create {
                name: "web".to_owned(),
                kind: Kind::Container,
                source: Source::Remote {
                    server: "https://docker.io".to_owned(),
                    protocol: Protocol::Oci,
                    alias: "nginx".to_owned(),
                },
                profiles: Vec::new(),
                limits: Limits::default(),
            },
        };
        assert_eq!(hub.needs_hosts().len(), 3);
    }

    #[test]
    fn a_name_incus_refuses_is_refused_here_first() {
        for good in ["web", "a", "debian-13", "A1b2"] {
            assert!(is_instance_name(good), "{good}");
        }
        for bad in [
            "",
            "1web",
            "-web",
            "web-",
            "../x",
            "web/snap",
            "we b",
            "wéb",
            &"a".repeat(64),
        ] {
            assert!(!is_instance_name(bad), "{bad}");
        }
    }

    /// FAILS IF: a snapshot the daemon itself made (`incus snapshot create
    /// web pre_upgrade`, a dated `snapshots.pattern`) is listed and then
    /// refused when the person restores or deletes it.
    #[test]
    fn a_snapshot_name_is_as_free_as_the_daemon_s() {
        for good in [
            "before-upgrade",
            "pre_upgrade",
            "v1.0",
            "snap0",
            "2026-10-05_12:30",
            "1st",
            "x%y",
            "wéb",
        ] {
            assert!(is_snapshot_name(good), "{good}");
        }
        for bad in ["", ".", "..", "a/b", "a b", "a\nb", &"a".repeat(256)] {
            assert!(!is_snapshot_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn the_words_are_the_daemon_s() {
        for kind in [Kind::Container, Kind::Vm] {
            assert_eq!(Kind::of_word(kind.word()), Some(kind));
        }
        for p in [Protocol::SimpleStreams, Protocol::Oci] {
            assert_eq!(Protocol::of_word(p.word()), Some(p));
        }
        assert_eq!(Protocol::of_word("incus"), None);
        for why in Why::ALL {
            assert_eq!(Why::of_id(why.id()), Some(why));
        }
        assert_eq!(Status::of_code(103), Status::Running);
        assert_eq!(Status::of_code(106), Status::Other, "starting is neither");
        assert_eq!(Action::Delete.state_word(), None);
        assert_eq!(
            console_arguments("vm1", Console::Vga),
            ["console", "vm1", "--type=vga"]
        );
        assert_eq!(shell_arguments("c1"), ["exec", "c1", "--", "su", "-l"]);
    }
}
