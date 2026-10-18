//! Incus (`LEADER s x`) — the person's system containers and virtual
//! machines: instances, their snapshots and limits, the images here and in
//! the remotes' catalogues, profiles, networks, storage and projects. One
//! view, two doors, in the containers' shape.
//!
//! # What is here
//!
//! Everything the screen DECIDES: which lines each layout draws, what a key
//! or a click does to them, what has to be asked of the daemon as a result,
//! and what has to be read while something is shown ([`View::wants`]).
//! Nothing here talks to Incus: `ronoctua-incus` is the one crate that does,
//! `incus_feed.rs` asks it on threads (and follows its events while the view
//! is shown), the queue (`Work::Incus`) runs what lasts minutes, and the
//! footer and `incus_window.rs` are the two doors.
//!
//! # The containers' layout
//!
//! The same two layouts and the same arithmetic as the containers' view
//! (`crate::places::places`): side by side from a window's
//! `space.containers.panesFrom` cells — the kinds with their counts, the list
//! of the kind, and the detail of the row chosen — and stacked in the
//! footer's region and a narrow window, the kinds on a strip and the detail
//! over the list. Both are [`Line`]s whose ZONES the drawing and the pointer
//! read alike, so a control is taken where it is drawn.
//!
//! # A new instance is a draft, never a question
//!
//! `C-l` (and `LEADER s x n`) opens the draft in the detail's place: the kind
//! (container or VM), the name, the image, the three limits, and the row that
//! launches it on the queue. Each value is typed on the door's own line
//! (`Errand::Ask`), and the draft is KEPT when it is put away — what a person
//! typed is not lost to a stray click — until it is launched.
//!
//! # A question opens on "no"
//!
//! Deleting an instance, an image or a snapshot, and restoring a snapshot
//! (which loses what changed since), ask first, in the detail's place, and
//! the cursor is born on "no".
//!
//! # Nothing reaches the network before the person lets it
//!
//! A remote's catalogue, and an instance or an image copied from it, need
//! every host the remote's client asks (`ronoctua_incus::hosts_of`). While
//! one is not let, the detail draws the consent row (`crate::consent_rows`)
//! in place of what would be fetched, and nothing is asked.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ronoctua_incus::{
    Action, Create, Image, Instance, Job, Kind as Machine, LimitChange, Limits, Network, Offered,
    Pool, Profile, Project, Refusal, Remote, Snapshot, Source, Status, Volume, Why,
};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::fields::{Field as Cell, Hit as FilesHit, Ink};
use crate::incus_words as words;
pub use crate::places::{BAR_CELLS, Layout, Pane, PaneScroll, extent, places};

/// The keys this view teaches, and where each one applies.
///
/// EVERY COMMAND IS `C-`, the rule of every view with a detail to act on,
/// and `M-` never: `M-` is the compositor's family. `C-n` and `C-p` walk,
/// as in every list, so a new instance is `C-l` — Incus's own word for
/// "make and start", `launch`.
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "incus-down", Where::Anywhere),
    ("<up>", "incus-up", Where::Anywhere),
    ("RET", "incus-enter", Where::Anywhere),
    ("TAB", "incus-next-kind", Where::Anywhere),
    ("S-TAB", "incus-previous-kind", Where::Anywhere),
    ("<left>", "incus-left", Where::Leftward),
    ("<right>", "incus-right", Where::Rightward),
    ("S-<left>", "incus-previous-tab", Where::OnADetail),
    ("S-<right>", "incus-next-tab", Where::OnADetail),
    ("C-l", "incus-launch", Where::OnAnswering),
    ("C-s", "incus-start-stop", Where::OnAnInstance),
    ("C-r", "incus-restart", Where::OnARunning),
    ("C-f", "incus-freeze", Where::OnFreezable),
    ("C-d", "incus-delete", Where::OnDeletable),
    ("C-o", "incus-console", Where::OnARunning),
    ("C-v", "incus-screen", Where::OnARunningVm),
    ("C-t", "incus-shell", Where::OnARunning),
    ("C-u", "incus-push", Where::OnAnInstance),
    ("C-w", "incus-pull", Where::OnAnInstance),
    ("C-e", "incus-export", Where::OnAnInstance),
    ("C-i", "incus-import", Where::OnInstances),
    ("C-k", "incus-copy-image", Where::OnAnOffered),
];

/// The consent row's key, every app's (`consent_rows`): taught while the row
/// is drawn, and only then — a key that does nothing is not taught.
pub const OF_THE_CONSENT: &[(&str, &str)] = crate::consent_rows::OF_THE_CONSENT;

/// THE TYPED QUESTIONS, by the command that asks and answers. The draft's
/// name and image…
pub const NAME: &str = "incus-name";
/// See [`NAME`].
pub const IMAGE: &str = "incus-image";
/// …the three limits — the draft's, or an instance's on its Limits tab…
pub const CPU: &str = "incus-cpu";
/// See [`CPU`].
pub const MEMORY: &str = "incus-memory";
/// See [`CPU`].
pub const PROCESSES: &str = "incus-processes";
/// …a snapshot's name…
pub const SNAPSHOT: &str = "incus-snapshot";
/// …a file sent in (then where inside, [`PUSH_TO`])…
pub const PUSH: &str = "incus-push";
/// See [`PUSH`].
pub const PUSH_TO: &str = "incus-push-to";
/// …a file taken out (then where here, [`PULL_TO`])…
pub const PULL: &str = "incus-pull";
/// See [`PULL`].
pub const PULL_TO: &str = "incus-pull-to";
/// …the file an export becomes, the file an import reads, and the alias an
/// instance is published under.
pub const EXPORT: &str = "incus-export";
/// See [`EXPORT`].
pub const IMPORT: &str = "incus-import";
/// See [`EXPORT`].
pub const PUBLISH: &str = "incus-publish";

/// Every typed question, by its id.
pub const QUESTIONS: [&str; 13] = [
    NAME, IMAGE, CPU, MEMORY, PROCESSES, SNAPSHOT, PUSH, PUSH_TO, PULL, PULL_TO, EXPORT, IMPORT,
    PUBLISH,
];

/// Where a key is worth teaching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// When there is a column to the left to go to.
    Leftward,
    /// When there is a column to the right to go to.
    Rightward,
    /// While an instance's detail, with its tabs, is shown.
    OnADetail,
    /// On an instance.
    OnAnInstance,
    /// On a running instance.
    OnARunning,
    /// On a running instance, or a frozen one.
    OnFreezable,
    /// On what can be deleted: an instance, an image, a snapshot.
    OnDeletable,
    /// On a running virtual machine.
    OnARunningVm,
    /// On the instances, while the daemon answers.
    OnInstances,
    /// While the daemon answers: a key that would ask it nothing it could
    /// do is not taught.
    OnAnswering,
    /// On an image of a remote's catalogue.
    OnAnOffered,
}

/// A key, once the door has read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row.
    Up,
    /// Down one row.
    Down,
    /// Do what the row does.
    Enter,
    /// One level back, out of a question or the draft, and out.
    Escape,
    /// The column to the left.
    Left,
    /// The column to the right.
    Right,
    /// The next kind.
    NextKind,
    /// The previous kind.
    PreviousKind,
    /// The next tab of an instance's detail.
    NextTab,
    /// The previous tab.
    PreviousTab,
    /// A new instance: the draft.
    Launch,
    /// Start it, or stop it.
    StartStop,
    /// Restart it.
    Restart,
    /// Freeze it, or thaw it.
    Freeze,
    /// Delete it, after a question.
    Delete,
    /// Its text console, in the terminal.
    Console,
    /// Its screen, in remote-viewer.
    Screen,
    /// A shell inside it, in the terminal.
    Shell,
    /// Send a file into it.
    Push,
    /// Take a file out of it.
    Pull,
    /// Export it to a file.
    Export,
    /// Import an instance from a file.
    Import,
    /// Keep a copy of a catalogue's image here.
    CopyImage,
    /// Let the hosts the consent row names (`C-a`, every app's).
    Allow,
}

/// Which key a command id is — the ONE map both doors reach.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "incus-up" => Press::Up,
        "incus-down" => Press::Down,
        "incus-enter" => Press::Enter,
        "incus-left" => Press::Left,
        "incus-right" => Press::Right,
        "incus-next-kind" => Press::NextKind,
        "incus-previous-kind" => Press::PreviousKind,
        "incus-next-tab" => Press::NextTab,
        "incus-previous-tab" => Press::PreviousTab,
        "incus-launch" => Press::Launch,
        "incus-start-stop" => Press::StartStop,
        "incus-restart" => Press::Restart,
        "incus-freeze" => Press::Freeze,
        "incus-delete" => Press::Delete,
        "incus-console" => Press::Console,
        "incus-screen" => Press::Screen,
        "incus-shell" => Press::Shell,
        "incus-push" => Press::Push,
        "incus-pull" => Press::Pull,
        "incus-export" => Press::Export,
        "incus-import" => Press::Import,
        "incus-copy-image" => Press::CopyImage,
        _ => return None,
    })
}

/// Which key a notation is, through [`OF_THE_VIEW`] and the consent row's.
/// `ESC` is the way back, which every view has and no table teaches
/// (`chrome::with_the_way_back`).
#[must_use]
pub fn pressed(notation: &str) -> Option<Press> {
    if notation == "ESC" {
        return Some(Press::Escape);
    }
    if OF_THE_CONSENT.iter().any(|(k, _)| *k == notation) {
        return Some(Press::Allow);
    }
    OF_THE_VIEW
        .iter()
        .find(|(k, _, _)| *k == notation)
        .and_then(|(_, id, _)| press_of(id))
}

/// What a key means in the view — THE ONE READER both doors call, by the
/// key's whole notation: `C-g` is `Esc`, as in every list.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    if let Some(p) = pressed(&key.notation()) {
        return Some(p);
    }
    match crate::minibuffer::gesture_of(key) {
        crate::minibuffer::Gesture::Cancel => Some(Press::Escape),
        // `C-n` AND `C-p` walk, as in every list here.
        crate::minibuffer::Gesture::Down => Some(Press::Down),
        crate::minibuffer::Gesture::Up => Some(Press::Up),
        _ => None,
    }
}

/// Every command id this view reaches — its keys, and the typed questions
/// its rows and keys ask — for the registry's census.
pub fn reached() -> impl Iterator<Item = &'static str> {
    OF_THE_VIEW
        .iter()
        .map(|(_, id, _)| *id)
        .chain(QUESTIONS)
        .chain(["incus-open", "incus-window", "incus-new", "incus-images"])
}

/// A kind of the daemon's things.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Kind {
    /// Containers and virtual machines.
    #[default]
    Instances,
    /// The images here.
    Images,
    /// The client's remotes, and each one's catalogue.
    Remotes,
    /// Profiles.
    Profiles,
    /// Networks.
    Networks,
    /// Storage pools, and each one's volumes.
    Storage,
    /// Projects.
    Projects,
}

impl Kind {
    /// The seven, in the order drawn.
    pub const ALL: [Self; 7] = [
        Self::Instances,
        Self::Images,
        Self::Remotes,
        Self::Profiles,
        Self::Networks,
        Self::Storage,
        Self::Projects,
    ];

    /// Where it is in [`Kind::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }

    /// The tabs its detail has: an instance's four, one page for the rest.
    #[must_use]
    pub const fn tabs(self) -> &'static [Tab] {
        match self {
            Self::Instances => &Tab::ALL,
            _ => &[Tab::Overview],
        }
    }

    /// The next kind, round.
    #[must_use]
    pub fn turned(self, forward: bool) -> Self {
        let n = Self::ALL.len();
        let i = self.index();
        Self::ALL[if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        }]
    }
}

/// A tab of an instance's detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tab {
    /// What it is, and its state: addresses, processor, memory.
    #[default]
    Overview,
    /// Its snapshots: taken, restored, deleted.
    Snapshots,
    /// Its limits, each one changed by typing it.
    Limits,
    /// What one does with it: the consoles, a shell, files, export, publish.
    Actions,
}

impl Tab {
    /// The four, in the order drawn.
    pub const ALL: [Self; 4] = [Self::Overview, Self::Snapshots, Self::Limits, Self::Actions];
}

/// One of an instance's limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Limit {
    /// Processors.
    Cpu,
    /// Memory.
    Memory,
    /// Processes (a container's; a VM's kernel counts its own).
    Processes,
}

impl Limit {
    /// The three.
    pub const ALL: [Self; 3] = [Self::Cpu, Self::Memory, Self::Processes];

    /// The typed question that changes it.
    #[must_use]
    pub const fn question(self) -> &'static str {
        match self {
            Self::Cpu => CPU,
            Self::Memory => MEMORY,
            Self::Processes => PROCESSES,
        }
    }

    /// What it is set to, as the line holds it to begin with.
    #[must_use]
    pub fn typed(self, limits: Limits) -> Option<String> {
        match self {
            Self::Cpu => limits.cpu.map(|n| n.to_string()),
            Self::Memory => limits.memory_bytes.map(words::bytes_typed),
            Self::Processes => limits.processes.map(|n| n.to_string()),
        }
    }

    /// Whether `limits` set it.
    #[must_use]
    pub const fn is_set(self, limits: Limits) -> bool {
        match self {
            Self::Cpu => limits.cpu.is_some(),
            Self::Memory => limits.memory_bytes.is_some(),
            Self::Processes => limits.processes.is_some(),
        }
    }

    /// This limit, and nothing else, as it is to be: `None` takes the
    /// instance's own away. `None` for a number the daemon's word cannot
    /// hold (a count of processors past `u32`).
    #[must_use]
    pub fn change(self, value: Option<u64>) -> Option<LimitChange> {
        Some(match self {
            Self::Cpu => LimitChange::Cpu(match value {
                Some(n) => Some(u32::try_from(n).ok()?),
                None => None,
            }),
            Self::Memory => LimitChange::MemoryBytes(value),
            Self::Processes => LimitChange::Processes(value),
        })
    }
}

/// What one does with an instance — a row of its Actions tab, and a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Use {
    /// Its text console, in our terminal.
    Console,
    /// Its screen, in remote-viewer — a VM's.
    Screen,
    /// A login shell inside it, in our terminal.
    Shell,
    /// A file sent in.
    Push,
    /// A file taken out.
    Pull,
    /// Exported to a file.
    Export,
    /// Published as an image.
    Publish,
}

impl Use {
    /// The seven, in the order drawn.
    pub const ALL: [Self; 7] = [
        Self::Console,
        Self::Screen,
        Self::Shell,
        Self::Push,
        Self::Pull,
        Self::Export,
        Self::Publish,
    ];

    /// Whether it needs the instance running.
    #[must_use]
    pub const fn needs_running(self) -> bool {
        matches!(self, Self::Console | Self::Screen | Self::Shell)
    }
}

/// A row of the new instance's draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// Container or VM.
    Machine,
    /// Its name.
    Name,
    /// Its image.
    Image,
    /// A limit.
    Limit(Limit),
}

impl Field {
    /// Every one, in the order drawn.
    pub const ALL: [Self; 6] = [
        Self::Machine,
        Self::Name,
        Self::Image,
        Self::Limit(Limit::Cpu),
        Self::Limit(Limit::Memory),
        Self::Limit(Limit::Processes),
    ];
}

/// How a launched program is started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launch {
    /// In OUR terminal (`ronoctua-shell terminal <cli> …`): a console and a
    /// shell are a terminal's, and the terminal is this system's own.
    InTheTerminal,
    /// By itself: `incus console --type=vga` opens remote-viewer and waits.
    ByItself,
}

impl Use {
    /// How what it opens is started — `None` for what is not opened.
    #[must_use]
    pub const fn launch(self) -> Option<Launch> {
        match self {
            Self::Console | Self::Shell => Some(Launch::InTheTerminal),
            Self::Screen => Some(Launch::ByItself),
            Self::Push | Self::Pull | Self::Export | Self::Publish => None,
        }
    }
}

/// THE PROGRAM A DOOR STARTS for a console, the screen or a shell, and its
/// words — the one answer both doors launch. `me` is this binary (the
/// terminal is its subcommand), `line` the CLI's whole command line
/// (`ronoctua_incus::Incus::console_command`). A text console and a shell
/// open in our terminal; the screen is remote-viewer's, started by the CLI
/// itself. `None` for an empty line. Pure.
#[must_use]
pub fn launched(how: Launch, me: &str, line: &[String]) -> Option<(String, Vec<String>)> {
    let (cli, args) = line.split_first()?;
    Some(match how {
        Launch::InTheTerminal => {
            let mut words = vec!["terminal".to_owned(), cli.clone()];
            words.extend(args.iter().cloned());
            (me.to_owned(), words)
        }
        Launch::ByItself => (cli.clone(), args.to_vec()),
    })
}

// ---------------------------------------------------------------------------
// Readings: what the door holds and hands in.
// ---------------------------------------------------------------------------

/// What the daemon listed, at once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    /// The instances of the default project, with their state and snapshots.
    pub instances: Vec<Instance>,
    /// The images here.
    pub images: Vec<Image>,
    /// The profiles.
    pub profiles: Vec<Profile>,
    /// The networks.
    pub networks: Vec<Network>,
    /// The storage pools.
    pub pools: Vec<Pool>,
    /// The projects.
    pub projects: Vec<Project>,
}

/// A remote's catalogue, answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalogue {
    /// Whose.
    pub remote: String,
    /// What it offers this machine, or why not.
    pub answer: Result<Vec<Offered>, Refusal>,
}

/// A pool's volumes, answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volumes {
    /// Whose.
    pub pool: String,
    /// Its volumes, or why not.
    pub answer: Result<Vec<Volume>, Refusal>,
}

/// Whether the daemon's events are followed — and so whether what is shown
/// follows a change made elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Following {
    /// Not known yet, or no live daemon's to follow (a photograph's scene,
    /// a daemon that answers nothing — its refusal says enough).
    #[default]
    Unknown,
    /// Followed.
    Yes,
    /// The stream ended (the daemon restarted) or would not open: what is
    /// shown may be old, and the feed opens it again on its own.
    Lost,
}

/// Everything the view draws from, handed in by the door.
#[derive(Debug, Clone, Default)]
pub struct Readings {
    /// The listing; `None` while the first one is on its way.
    pub listing: Option<Result<Listing, Refusal>>,
    /// The client's remotes; `None` while they are read.
    pub remotes: Option<Result<Vec<Remote>, Refusal>>,
    /// The last catalogue asked for, and whose.
    pub catalogue: Option<Catalogue>,
    /// The last volumes asked for, and whose.
    pub volumes: Option<Volumes>,
    /// The remotes' hosts the person has let (`consent.json`), as read by
    /// the feed — never by the view, which draws.
    pub consented: BTreeSet<String>,
    /// The queue's jobs that are Incus's.
    pub jobs: Vec<ronoctua_transfers::Job>,
    /// Whether the daemon's events are followed.
    pub events: Following,
    /// The machine's zone, for the dates.
    pub zone: ronoctua_files::when::Zone,
    /// The keys that open the queue, as this door's leader teaches them.
    pub queue_keys: String,
}

impl Readings {
    /// The listing, when there is one.
    #[must_use]
    pub fn listed(&self) -> Option<&Listing> {
        self.listing.as_ref().and_then(|l| l.as_ref().ok())
    }

    /// The remotes, when they were read.
    #[must_use]
    pub fn remotes_read(&self) -> &[Remote] {
        match &self.remotes {
            Some(Ok(r)) => r,
            _ => &[],
        }
    }

    /// How many things of a kind there are — the kinds column's counts;
    /// `None` before the kind was read.
    #[must_use]
    pub fn count(&self, kind: Kind) -> Option<usize> {
        if kind == Kind::Remotes {
            return match &self.remotes {
                Some(Ok(r)) => Some(r.len()),
                _ => None,
            };
        }
        self.listed().map(|l| match kind {
            Kind::Instances => l.instances.len(),
            Kind::Images => l.images.len(),
            Kind::Profiles => l.profiles.len(),
            Kind::Networks => l.networks.len(),
            Kind::Storage => l.pools.len(),
            Kind::Projects => l.projects.len(),
            Kind::Remotes => 0,
        })
    }

    /// The identities of a kind's things, in the order drawn.
    #[must_use]
    pub fn keys(&self, kind: Kind) -> Vec<String> {
        if kind == Kind::Remotes {
            return self.remotes_read().iter().map(|r| r.name.clone()).collect();
        }
        self.listed().map_or_else(Vec::new, |l| match kind {
            Kind::Instances => l.instances.iter().map(|i| i.name.clone()).collect(),
            Kind::Images => l.images.iter().map(|i| i.fingerprint.clone()).collect(),
            Kind::Profiles => l.profiles.iter().map(|p| p.name.clone()).collect(),
            Kind::Networks => l.networks.iter().map(|n| n.name.clone()).collect(),
            Kind::Storage => l.pools.iter().map(|p| p.name.clone()).collect(),
            Kind::Projects => l.projects.iter().map(|p| p.name.clone()).collect(),
            Kind::Remotes => Vec::new(),
        })
    }

    /// An instance, by its name.
    #[must_use]
    pub fn instance(&self, name: &str) -> Option<&Instance> {
        self.listed()?.instances.iter().find(|i| i.name == name)
    }

    fn image(&self, fingerprint: &str) -> Option<&Image> {
        self.listed()?
            .images
            .iter()
            .find(|i| i.fingerprint == fingerprint)
    }

    fn remote(&self, name: &str) -> Option<&Remote> {
        self.remotes_read().iter().find(|r| r.name == name)
    }

    fn profile(&self, name: &str) -> Option<&Profile> {
        self.listed()?.profiles.iter().find(|p| p.name == name)
    }

    fn network(&self, name: &str) -> Option<&Network> {
        self.listed()?.networks.iter().find(|n| n.name == name)
    }

    fn pool(&self, name: &str) -> Option<&Pool> {
        self.listed()?.pools.iter().find(|p| p.name == name)
    }

    fn project(&self, name: &str) -> Option<&Project> {
        self.listed()?.projects.iter().find(|p| p.name == name)
    }

    /// Whether every one of `hosts` is let.
    #[must_use]
    pub fn lets(&self, hosts: &[String]) -> bool {
        hosts.iter().all(|h| self.consented.contains(h))
    }

    /// The hosts of `hosts` that are not let yet.
    #[must_use]
    pub fn unlet(&self, hosts: &[String]) -> Vec<String> {
        hosts
            .iter()
            .filter(|h| !self.consented.contains(*h))
            .cloned()
            .collect()
    }

    /// The catalogue of `remote`, when the last one asked was its.
    fn catalogue_of(&self, remote: &str) -> Option<&Result<Vec<Offered>, Refusal>> {
        self.catalogue
            .as_ref()
            .filter(|c| c.remote == remote)
            .map(|c| &c.answer)
    }

    /// The volumes of `pool`, when the last ones asked were its.
    fn volumes_of(&self, pool: &str) -> Option<&Result<Vec<Volume>, Refusal>> {
        self.volumes
            .as_ref()
            .filter(|v| v.pool == pool)
            .map(|v| &v.answer)
    }
}

/// An image's name: its first alias, or its short fingerprint.
#[must_use]
pub fn image_name(i: &Image) -> String {
    i.aliases
        .first()
        .cloned()
        .unwrap_or_else(|| short(&i.fingerprint))
}

/// The daemon's short form of a fingerprint.
#[must_use]
pub fn short(fingerprint: &str) -> String {
    fingerprint.chars().take(12).collect()
}

/// Whether an instance is up — running, or frozen inside a run.
const fn is_up(status: Status) -> bool {
    matches!(status, Status::Running | Status::Frozen)
}

// ---------------------------------------------------------------------------
// Lines and zones.
// ---------------------------------------------------------------------------

/// What a stretch of a line is, for a key and for the pointer alike.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Nothing: a heading, a note.
    Nothing,
    /// A kind, in the kinds column or on the strip.
    Kind(Kind),
    /// A row of the list — the thing's identity.
    Item(String),
    /// A tab of the detail.
    Tab(Tab),
    /// A fact of the detail: `RET` copies it.
    Copy(String),
    /// A snapshot: `RET` restores it, after a question.
    Snapshot(String),
    /// The row that takes a snapshot.
    TakeSnapshot,
    /// A limit of the instance's: `RET` asks it.
    Limit(Limit),
    /// An action row.
    Use(Use),
    /// An image's row that opens the draft over it.
    LaunchFrom,
    /// An image of a remote's catalogue, by its alias and kind: `RET` opens
    /// the draft over it, `C-k` keeps a copy here.
    Offered(String, Machine),
    /// The consent row: `RET` lets the hosts it names.
    Consent(Vec<String>),
    /// A row of the draft.
    Field(Field),
    /// The draft's last row: it is launched.
    Create,
    /// An answer to the question.
    Answer(bool),
}

impl Target {
    /// Whether a click or `RET` on it does anything.
    #[must_use]
    pub const fn acts(&self) -> bool {
        !matches!(self, Self::Nothing)
    }
}

/// One row of one column, its fields counted from the column's first cell.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// What is drawn.
    pub fields: Vec<Cell>,
    /// What it is.
    pub target: Target,
    /// The cursor skips it.
    pub heading: bool,
}

/// A stretch of a drawn line and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    /// Its first cell, counted as the fields are.
    pub from: u16,
    /// The cell after its last.
    pub to: u16,
    /// Which column it belongs to.
    pub pane: Pane,
    /// Which row of that column it is.
    pub row: usize,
    /// What it is.
    pub target: Target,
}

/// One drawn line.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Line {
    /// Its fields, placed.
    pub fields: Vec<Cell>,
    /// Its stretches, left to right.
    pub zones: Vec<Zone>,
}

// ---------------------------------------------------------------------------
// Questions, the draft and errands.
// ---------------------------------------------------------------------------

/// A question, drawn in the detail's place, "no" first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Question {
    /// Delete this instance.
    Delete {
        /// Its name.
        name: String,
        /// It is up: it is stopped first.
        running: bool,
        /// It has snapshots, which go with it.
        snapshots: bool,
    },
    /// Delete this image.
    DeleteImage {
        /// Its fingerprint.
        fingerprint: String,
        /// Its name, for the sentence.
        label: String,
    },
    /// Delete this snapshot.
    DeleteSnapshot {
        /// The instance.
        name: String,
        /// The snapshot.
        snapshot: String,
    },
    /// Restore this snapshot — what changed since is lost.
    Restore {
        /// The instance.
        name: String,
        /// The snapshot.
        snapshot: String,
    },
}

/// Something the daemon is told, on the feed's thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deed {
    /// Start, stop, restart, freeze, thaw or delete an instance.
    Act {
        /// The instance.
        name: String,
        /// What.
        action: Action,
    },
    /// Set ONE of an instance's limits in its own configuration — the
    /// others, and a profile's, are left as they are.
    Limit {
        /// The instance.
        name: String,
        /// Which, and as it is to be.
        change: LimitChange,
    },
    /// Take a snapshot.
    Snapshot {
        /// The instance.
        name: String,
        /// The snapshot's name.
        snapshot: String,
    },
    /// Restore a snapshot.
    Restore {
        /// The instance.
        name: String,
        /// The snapshot.
        snapshot: String,
    },
    /// Delete a snapshot.
    DeleteSnapshot {
        /// The instance.
        name: String,
        /// The snapshot.
        snapshot: String,
    },
    /// Delete an image.
    DeleteImage {
        /// Its fingerprint.
        fingerprint: String,
    },
}

impl Deed {
    /// Its word, for the sentences before and after it.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        use crate::key;
        match self {
            Self::Act { action, .. } => match action {
                Action::Start => key::INCUS_DID_START,
                Action::Stop => key::INCUS_DID_STOP,
                Action::Restart => key::INCUS_DID_RESTART,
                Action::Freeze => key::INCUS_DID_FREEZE,
                Action::Unfreeze => key::INCUS_DID_UNFREEZE,
                Action::Delete => key::INCUS_DID_DELETE,
            },
            Self::Limit { .. } => key::INCUS_DID_LIMITS,
            Self::Snapshot { .. } => key::INCUS_DID_SNAPSHOT,
            Self::Restore { .. } => key::INCUS_DID_RESTORE,
            Self::DeleteSnapshot { .. } => key::INCUS_DID_DELETE_SNAPSHOT,
            Self::DeleteImage { .. } => key::INCUS_DID_DELETE_IMAGE,
        }
    }
}

/// A new instance, being drafted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// Container or VM.
    pub machine: Machine,
    /// Its name; empty until typed.
    pub name: String,
    /// Its image, once chosen.
    pub source: Option<Source>,
    /// The image as the person reads it: `images:debian/13`, a local alias.
    pub image: String,
    /// Its limits.
    pub limits: Limits,
}

impl Draft {
    /// A fresh draft: a container, nothing chosen.
    #[must_use]
    pub fn fresh() -> Self {
        Self {
            machine: Machine::Container,
            name: String::new(),
            source: None,
            image: String::new(),
            limits: Limits::default(),
        }
    }

    /// The hosts its image needs let: a remote's, none for a local image.
    #[must_use]
    pub fn hosts(&self) -> Vec<String> {
        self.source
            .as_ref()
            .map(Source::needs_hosts)
            .unwrap_or_default()
    }

    /// The fields it draws: a VM has no limit of processes.
    #[must_use]
    pub fn fields(&self) -> Vec<Field> {
        Field::ALL
            .into_iter()
            .filter(|f| !(self.machine == Machine::Vm && *f == Field::Limit(Limit::Processes)))
            .collect()
    }
}

/// What the typed question on the door's line is for — kept by the view
/// between the asking and the answer, and let go with the question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asking {
    /// A field of the draft.
    Draft(Field),
    /// One of an instance's limits.
    Limit {
        /// The instance.
        name: String,
        /// Which.
        limit: Limit,
        /// All three, as they are now, its profiles' included.
        limits: Limits,
        /// The ones its own configuration sets.
        own: Limits,
    },
    /// A snapshot's name.
    Snapshot {
        /// The instance.
        name: String,
    },
    /// The file sent in.
    PushFrom {
        /// The instance.
        name: String,
    },
    /// Where it goes inside.
    PushTo {
        /// The instance.
        name: String,
        /// The file here.
        from: PathBuf,
    },
    /// The path taken out.
    PullFrom {
        /// The instance.
        name: String,
    },
    /// Where it goes here.
    PullTo {
        /// The instance.
        name: String,
        /// The path there.
        from: String,
    },
    /// The file an export becomes.
    Export {
        /// The instance.
        name: String,
    },
    /// The alias an instance is published under.
    Publish {
        /// The instance.
        name: String,
    },
    /// The file an import reads.
    Import,
}

impl Asking {
    /// The typed question it is the answer to.
    #[must_use]
    pub const fn question(&self) -> &'static str {
        match self {
            Self::Draft(Field::Name) => NAME,
            Self::Draft(Field::Image) => IMAGE,
            Self::Draft(Field::Limit(l)) | Self::Limit { limit: l, .. } => l.question(),
            // THE KIND IS TURNED, never typed; nothing asks it.
            Self::Draft(Field::Machine) => NAME,
            Self::Snapshot { .. } => SNAPSHOT,
            Self::PushFrom { .. } => PUSH,
            Self::PushTo { .. } => PUSH_TO,
            Self::PullFrom { .. } => PULL,
            Self::PullTo { .. } => PULL_TO,
            Self::Export { .. } => EXPORT,
            Self::Publish { .. } => PUBLISH,
            Self::Import => IMPORT,
        }
    }
}

/// What the door has to do about a key, a click or an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Draw again, and nothing else.
    Nothing,
    /// Shut the view.
    Close,
    /// Put this on the clipboard.
    Copy(String),
    /// Tell the daemon this, on the feed's thread; the name is for the
    /// sentence.
    Do(Deed, String),
    /// Put this on the queue, under this label.
    Queue(Job, String),
    /// Ask this typed question on the door's line, holding this to begin
    /// with.
    Ask(&'static str, Option<String>),
    /// Open a console, the screen or a shell of this instance
    /// ([`launched`]).
    Open(Use, String),
    /// Let these hosts in `consent.json`.
    Allow(Vec<String>),
    /// Say this; it is already a sentence.
    Say(String),
}

/// What has to be read for what is shown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wants {
    /// This remote's catalogue (only once its hosts are let).
    pub catalogue: Option<String>,
    /// This pool's volumes.
    pub volumes: Option<String>,
    /// The instances whose state is READ AGAIN on a short clock, because no
    /// event of the daemon's says it changed: one that runs with no address
    /// yet (its lease comes later), and the one whose Overview is on screen
    /// (its processor, memory and processes move). Empty, the clock is not
    /// armed ([`STATE_SECONDS`]).
    pub states: Vec<String>,
}

/// How often the state [`Wants::states`] names is read again — and the
/// daemon's events opened again once they were lost — while the view shows
/// it. A clock armed only while something on screen needs it.
pub const STATE_SECONDS: u64 = 2;

// ---------------------------------------------------------------------------
// The view.
// ---------------------------------------------------------------------------

/// The view's state.
#[derive(Debug, Clone, Default)]
pub struct View {
    kind: Kind,
    focus: Pane,
    /// The row chosen in each kind, by identity.
    picked: [Option<String>; 7],
    /// Stacked: the detail is open over the list.
    open: bool,
    tab: Tab,
    /// The detail's cursor, a row of the detail — drawn by its index, and
    /// KEPT by what it is on ([`Held`]).
    at: usize,
    /// What the detail's cursor is on, so new readings find its row again.
    held: Option<Held>,
    /// Each column's first row on screen, in the panes.
    offsets: [usize; 3],
    question: Option<Question>,
    /// Where the keyboard was, and whether the detail was open, when the
    /// question or the draft came: going back puts both back.
    before: (Pane, bool),
    /// The question's cursor: `true` on "yes".
    yes: bool,
    /// The new instance, kept from its first key to its launch.
    draft: Option<Draft>,
    /// The draft is drawn in the detail's place.
    drafting: bool,
    /// What the typed question on the door's line is for.
    asking: Option<Asking>,
}

/// What the detail's cursor is on, as the daemon's data names it: a list the
/// daemon reorders (a snapshot expired, one taken in the other door, a
/// volume made) keeps the cursor on the THING, never on the row's index.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Held {
    /// Whose detail, which tab, and whether the draft is drawn in its place:
    /// the cursor is looked for only in the same one.
    about: (Kind, Option<String>, Tab, bool),
    /// The row's target — a snapshot's name, a volume's, a fact's value.
    target: Target,
    /// How many rows above it have the same target: two facts can copy the
    /// same value (a profile and a project both called `default`).
    nth: usize,
}

/// The row of `rows` that is the `nth` with `target`.
fn nth_row(rows: &[Row], target: &Target, nth: usize) -> Option<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| row.target == *target)
        .nth(nth)
        .map(|(n, _)| n)
}

/// A lean row builder for one column.
struct Col<'a> {
    rows: Vec<Row>,
    room: u16,
    texts: &'a ronoctua_i18n::Texts,
}

fn piece(at: u16, text: String, color: tokens::Color) -> Cell {
    Cell {
        at,
        text,
        color,
        hit: FilesHit::Nothing,
        explain: None,
        ink: Ink::Grid,
    }
}

impl<'a> Col<'a> {
    const fn new(room: u16, texts: &'a ronoctua_i18n::Texts) -> Self {
        Self {
            rows: Vec::new(),
            room,
            texts,
        }
    }

    fn push(&mut self, fields: Vec<Cell>, target: Target, heading: bool) {
        self.rows.push(Row {
            fields,
            target,
            heading,
        });
    }

    /// A section title — air before it, never before the first.
    fn heading(&mut self, key: &str) {
        if !self.rows.is_empty() {
            self.push(crate::chrome::AIR, Target::Nothing, true);
        }
        let text = layout::elide_end(&self.texts.t(key), self.room);
        self.push(
            vec![piece(0, text, tokens::color::TEXT_HEADING)],
            Target::Nothing,
            true,
        );
    }

    /// A sentence, wrapped, never cut.
    fn note(&mut self, text: &str, color: tokens::Color) {
        for line in crate::chrome::note_lines(text, self.room) {
            self.push(vec![piece(0, line, color)], Target::Nothing, true);
        }
    }

    /// A fact: `RET` copies its value.
    fn fact(&mut self, label: &str, value: &str, focused: bool, tint: Option<tokens::Color>) {
        let fields = crate::chrome::field_pair(
            0,
            self.room,
            label,
            value,
            crate::chrome::Kind::Fact,
            focused,
        );
        self.push(
            crate::chrome::tinted(fields, tint, focused),
            Target::Copy(value.to_owned()),
            false,
        );
    }

    /// A fact whose `RET` copies something else than its value — a volume's
    /// name, beside its type.
    fn fact_copying(&mut self, label: &str, value: &str, copy: &str, focused: bool) {
        let fields = crate::chrome::field_pair(
            0,
            self.room,
            label,
            value,
            crate::chrome::Kind::Fact,
            focused,
        );
        self.push(
            crate::chrome::tinted(fields, None, focused),
            Target::Copy(copy.to_owned()),
            false,
        );
    }

    /// A row that acts: its label, and its value in the tones of a setting.
    fn acting(&mut self, label: &str, value: &str, focused: bool, target: Target) {
        let fields = crate::chrome::field_pair(
            0,
            self.room,
            label,
            value,
            crate::chrome::Kind::Setting,
            focused,
        );
        self.push(fields, target, false);
    }

    /// A row drawn refusing: reachable, so it can say why, and inert.
    fn refusing(&mut self, label: &str, value: &str, focused: bool, target: Target) {
        let fields = crate::chrome::field_pair(
            0,
            self.room,
            label,
            value,
            crate::chrome::Kind::Disabled,
            focused,
        );
        self.push(fields, target, false);
    }

    /// The consent row (`consent_rows`): the hosts to let, then what for —
    /// every line of it the one press.
    fn consent(&mut self, hosts: &[String], what: &'static str, focused: bool) {
        let named: Vec<&str> = hosts.iter().map(String::as_str).collect();
        let lines = crate::consent_rows::lines(
            crate::consent_rows::Ask {
                hosts: &named,
                what,
            },
            self.room,
            focused,
            self.texts,
        );
        for (n, (text, color)) in lines.into_iter().enumerate() {
            self.push(
                vec![piece(0, text, color)],
                Target::Consent(hosts.to_vec()),
                n > 0,
            );
        }
    }
}

/// Numbered when it is not the first: two rows of one detail never say the
/// same thing (`Address`, `Address 2`), in the words of the person's
/// language.
fn nth_label(texts: &ronoctua_i18n::Texts, key: &str, n: usize) -> String {
    if n == 0 {
        texts.t(key)
    } else {
        texts.t_args(
            crate::key::INCUS_ROW_NTH,
            &[
                ("label", ronoctua_i18n::Arg::Text(&texts.t(key))),
                ("n", ronoctua_i18n::Arg::Number(words::count(n + 1))),
            ],
        )
    }
}

/// A moment of the daemon's, as the person's dates read — or the daemon's
/// own words when they are not one.
fn when_of(raw: &str, r: &Readings, texts: &ronoctua_i18n::Texts) -> String {
    words::moment(raw).map_or_else(
        || raw.to_owned(),
        |s| crate::chrome::when_text(s, &r.zone, false, texts),
    )
}

/// A size, in the person's decimal mark.
fn size_of(bytes: u64, texts: &ronoctua_i18n::Texts) -> String {
    crate::sizes::readable_bytes(bytes, texts.decimal_mark())
}

/// A limit's value as drawn: the number, or "no limit".
fn limit_text(limit: Limit, limits: Limits, texts: &ronoctua_i18n::Texts) -> String {
    let none = || texts.t(crate::key::INCUS_LIMIT_NONE);
    match limit {
        Limit::Cpu => limits.cpu.map_or_else(none, |n| {
            texts.t_args(
                crate::key::INCUS_LIMIT_CPUS,
                &[("count", ronoctua_i18n::Arg::Number(i64::from(n)))],
            )
        }),
        Limit::Memory => limits.memory_bytes.map_or_else(none, |b| size_of(b, texts)),
        Limit::Processes => limits.processes.map_or_else(none, |n| {
            texts.t_args(
                crate::key::INCUS_LIMIT_COUNT,
                &[(
                    "count",
                    ronoctua_i18n::Arg::Number(i64::try_from(n).unwrap_or(i64::MAX)),
                )],
            )
        }),
    }
}

/// The first snapshot name `snapN` the instance does not have yet — the
/// daemon's own pattern, so a name typed over it reads like one it made.
#[must_use]
pub fn next_snapshot(snapshots: &[Snapshot]) -> String {
    (0..)
        .map(|n| format!("snap{n}"))
        .find(|name| !snapshots.iter().any(|s| s.name == *name))
        .unwrap_or_default()
}

impl View {
    /// A fresh view, on the instances.
    #[must_use]
    pub fn fresh() -> Self {
        Self::default()
    }

    /// A fresh view on a kind — `incus-images` opens on the images.
    #[must_use]
    pub fn on(kind: Kind) -> Self {
        Self {
            kind,
            ..Self::default()
        }
    }

    /// The kind shown.
    #[must_use]
    #[cfg(test)]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// The live view turned to `kind` — `LEADER s x i` or `LEADER s x x`
    /// while it is open on another kind. NOTHING IS LOST: the draft is put
    /// away (kept, as `Esc` keeps it), each kind's chosen row and the tab
    /// stay; a question up is taken down, as any other key takes it.
    pub fn show(&mut self, kind: Kind) {
        if self.kind == kind {
            return;
        }
        if self.question.is_some() {
            self.unask();
        }
        self.undraft();
        self.choose_kind(kind);
    }

    /// The column the keyboard is in.
    #[must_use]
    pub const fn focus(&self) -> Pane {
        self.focus
    }

    /// The question up, if any.
    #[must_use]
    #[cfg(test)]
    pub const fn question(&self) -> Option<&Question> {
        self.question.as_ref()
    }

    /// The draft, when there is one (shown or put away).
    #[must_use]
    #[cfg(test)]
    pub const fn draft(&self) -> Option<&Draft> {
        self.draft.as_ref()
    }

    /// Whether the draft is drawn.
    #[must_use]
    #[cfg(test)]
    pub const fn drafting(&self) -> bool {
        self.drafting
    }

    /// The question on the door's line was let go — `Esc`, `C-g`: what it was
    /// for goes with it, so an answer typed later under another command is
    /// never taken for it.
    pub fn let_go(&mut self) {
        self.asking = None;
    }

    /// The identity of the row chosen in the kind shown, once it is a row
    /// that exists — the first row when none was chosen yet.
    #[must_use]
    pub fn picked(&self, r: &Readings) -> Option<String> {
        let keys = r.keys(self.kind);
        match &self.picked[self.kind.index()] {
            Some(k) if keys.contains(k) => Some(k.clone()),
            _ => keys.into_iter().next(),
        }
    }

    /// Whether the detail (the row's, or the draft or a question in its
    /// place) is on screen: always beside the list in the panes once there is
    /// a row, and over the list when opened when stacked.
    #[must_use]
    pub fn detail_shown(&self, r: &Readings, layout: Layout) -> bool {
        (self.picked(r).is_some() || self.covered())
            && match layout {
                Layout::Panes => true,
                Layout::Stacked => self.open,
            }
    }

    /// Whether the detail's place holds the draft or a question.
    const fn covered(&self) -> bool {
        self.question.is_some() || self.drafting
    }

    /// The tab actually drawn: the one chosen, when the kind has it.
    #[must_use]
    pub fn tab_shown(&self) -> Tab {
        if self.kind.tabs().contains(&self.tab) {
            self.tab
        } else {
            Tab::Overview
        }
    }

    /// What has to be read for what is on screen. The door hands it to the
    /// feed after every change: a catalogue is asked only while its remote's
    /// detail is shown and its hosts are let, a pool's volumes only while
    /// its detail is shown.
    #[must_use]
    pub fn wants(&self, r: &Readings, layout: Layout) -> Wants {
        let states = self.states_wanted(r, layout);
        if !self.detail_shown(r, layout) || self.covered() {
            return Wants {
                states,
                ..Wants::default()
            };
        }
        let Some(key) = self.picked(r) else {
            return Wants {
                states,
                ..Wants::default()
            };
        };
        match self.kind {
            Kind::Remotes => {
                let lets = r.remote(&key).is_some_and(|remote| {
                    r.lets(&ronoctua_incus::hosts_of(&remote.address, remote.protocol))
                });
                Wants {
                    catalogue: lets.then_some(key),
                    volumes: None,
                    states,
                }
            }
            Kind::Storage => Wants {
                catalogue: None,
                volumes: Some(key),
                states,
            },
            _ => Wants {
                states,
                ..Wants::default()
            },
        }
    }

    /// The instances whose state no event reports and the screen shows
    /// ([`Wants::states`]): while the list is on screen, each one that runs
    /// with no address yet; while an Overview is, its instance.
    fn states_wanted(&self, r: &Readings, layout: Layout) -> Vec<String> {
        let Some(l) = r.listed().filter(|_| self.kind == Kind::Instances) else {
            return Vec::new();
        };
        let detail = self.detail_shown(r, layout);
        let list_shown = match layout {
            Layout::Panes => true,
            Layout::Stacked => !detail,
        };
        let mut names: Vec<String> = Vec::new();
        if list_shown {
            names.extend(
                l.instances
                    .iter()
                    .filter(|i| {
                        i.status == Status::Running
                            && i.state
                                .as_ref()
                                .is_none_or(|s| s.ipv4.is_empty() && s.ipv6.is_empty())
                    })
                    .map(|i| i.name.clone()),
            );
        }
        if detail
            && !self.covered()
            && self.tab_shown() == Tab::Overview
            && let Some(i) = self.picked_instance(r)
            && i.status == Status::Running
            && !names.contains(&i.name)
        {
            names.push(i.name.clone());
        }
        names
    }

    /// The instance chosen, when the instances are shown.
    fn picked_instance<'r>(&self, r: &'r Readings) -> Option<&'r Instance> {
        (self.kind == Kind::Instances)
            .then(|| self.picked(r))
            .flatten()
            .and_then(|k| r.instance(&k))
    }

    /// The row the detail's cursor is on.
    fn detail_target(
        &self,
        r: &Readings,
        columns: u16,
        layout: Layout,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Target> {
        if self.focus != Pane::Detail {
            return None;
        }
        self.detail_rows(r, detail_width(columns, layout), texts)
            .get(self.at)
            .map(|row| row.target.clone())
    }

    /// Whether the consent row is drawn now — and its key, `C-a`, taught.
    #[must_use]
    pub fn asks_consent(&self, r: &Readings, layout: Layout) -> bool {
        if !self.detail_shown(r, layout) || self.question.is_some() {
            return false;
        }
        if self.drafting {
            return self.draft.as_ref().is_some_and(|d| !r.lets(&d.hosts()));
        }
        self.kind == Kind::Remotes
            && self
                .picked(r)
                .and_then(|k| r.remote(&k).cloned())
                .is_some_and(|remote| {
                    !r.lets(&ronoctua_incus::hosts_of(&remote.address, remote.protocol))
                })
    }

    /// The hint line: the keys that work where the cursor is.
    #[must_use]
    pub fn hint(
        &self,
        r: &Readings,
        columns: u16,
        layout: Layout,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        if self.question.is_some() {
            // A QUESTION: the arrows between the two answers, `RET` to give
            // one, `Esc` for no.
            let table: Vec<(&str, &str, bool)> = OF_THE_VIEW
                .iter()
                .filter(|(_, id, _)| matches!(*id, "incus-down" | "incus-up" | "incus-enter"))
                .map(|(k, id, _)| (*k, *id, false))
                .collect();
            return crate::chrome::with_the_way_back(
                crate::chrome::hint_of_marked(&table, |_| None, texts),
                texts,
            );
        }
        let mut here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| self.shown(*w, r, columns, layout, texts))
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        if self.asks_consent(r, layout) {
            here.extend(OF_THE_CONSENT.iter().map(|(k, id)| (*k, *id, true)));
        }
        crate::chrome::with_the_way_back(
            crate::chrome::hint_of_marked(&here, |_| None, texts),
            texts,
        )
    }

    /// Whether a key of the table works now.
    fn shown(
        &self,
        w: Where,
        r: &Readings,
        columns: u16,
        layout: Layout,
        texts: &ronoctua_i18n::Texts,
    ) -> bool {
        // THE DRAFT HAS ITS OWN ROWS: what acts on an instance waits.
        let instance = self.picked_instance(r).filter(|_| !self.drafting);
        let up = instance.is_some_and(|i| i.status == Status::Running);
        match w {
            Where::Anywhere => true,
            Where::Leftward => match layout {
                Layout::Panes => self.focus != Pane::Kinds,
                Layout::Stacked => self.open,
            },
            Where::Rightward => {
                self.picked(r).is_some()
                    && !self.covered()
                    && match layout {
                        Layout::Panes => self.focus != Pane::Detail,
                        Layout::Stacked => !self.open,
                    }
            }
            Where::OnADetail => {
                self.detail_shown(r, layout) && !self.covered() && self.kind.tabs().len() > 1
            }
            Where::OnAnInstance => instance.is_some(),
            Where::OnARunning => up,
            Where::OnFreezable => instance.is_some_and(|i| is_up(i.status)),
            Where::OnARunningVm => up && instance.is_some_and(|i| i.kind == Machine::Vm),
            Where::OnDeletable => {
                !self.drafting
                    && match self.kind {
                        Kind::Instances => instance.is_some(),
                        Kind::Images => self.picked(r).is_some(),
                        _ => false,
                    }
            }
            Where::OnInstances => {
                self.kind == Kind::Instances && !self.drafting && r.listed().is_some()
            }
            Where::OnAnswering => r.listed().is_some(),
            Where::OnAnOffered => matches!(
                self.detail_target(r, columns, layout, texts),
                Some(Target::Offered(..))
            ),
        }
    }

    // -- the columns ------------------------------------------------------

    /// The kinds column: each kind, its glyph and its count.
    fn kinds_rows(&self, r: &Readings, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        Kind::ALL
            .iter()
            .map(|&kind| {
                let chosen = kind == self.kind;
                let (label, value) = crate::chrome::Kind::Setting.tones(chosen);
                let count = r.count(kind).map(|n| n.to_string()).unwrap_or_default();
                let count_w = layout::width(&count);
                let name_room = room.saturating_sub(2 + count_w + 1);
                let mut fields = vec![
                    piece(0, words::kind_glyph(kind).to_string(), label),
                    piece(
                        2,
                        layout::elide_end(&texts.t(words::kind_key(kind)), name_room),
                        label,
                    ),
                ];
                if !count.is_empty() {
                    fields.push(piece(room.saturating_sub(count_w), count, value));
                }
                Row {
                    fields,
                    target: Target::Kind(kind),
                    heading: false,
                }
            })
            .collect()
    }

    /// What the queue is doing for this kind, above its list: a launch, an
    /// export, a file sent or taken under the instances; a copy or a
    /// publication under the images — the queue's stage, in its words.
    fn jobs_rows(&self, col: &mut Col<'_>, r: &Readings) {
        let texts = col.texts;
        for job in &r.jobs {
            let ronoctua_transfers::Work::Incus { job: which, label } = &job.work else {
                continue;
            };
            let mine = match which.as_ref() {
                Job::ImageCopy { .. } => self.kind == Kind::Images,
                Job::Publish { .. } => matches!(self.kind, Kind::Images | Kind::Instances),
                Job::Launch { .. }
                | Job::Export { .. }
                | Job::Import { .. }
                | Job::Push { .. }
                | Job::Pull { .. } => self.kind == Kind::Instances,
            };
            if !mine || matches!(job.stage, ronoctua_transfers::Stage::Done) {
                continue;
            }
            let what = texts.t_with(
                crate::key::INCUS_WORKING,
                &[
                    ("what", &words::job_said(which, texts)),
                    ("name", label),
                    (
                        "stage",
                        &texts.t(&format!("transfers-stage-{}", job.stage.word())),
                    ),
                ],
            );
            col.note(&what, tokens::color::TEXT_DIM);
        }
    }

    /// The list of the kind shown.
    fn list_rows(&self, r: &Readings, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let mut col = Col::new(room, texts);
        self.jobs_rows(&mut col, r);
        // THE REMOTES ARE THE CLIENT'S, read apart from the daemon's listing.
        let read = if self.kind == Kind::Remotes {
            r.remotes
                .as_ref()
                .map(|a| a.as_ref().map(|_| ()).map_err(|e| *e))
        } else {
            r.listing
                .as_ref()
                .map(|a| a.as_ref().map(|_| ()).map_err(|e| *e))
        };
        match read {
            None => {
                col.note(&texts.t(crate::key::INCUS_READING), tokens::color::TEXT_DIM);
                return col.rows;
            }
            Some(Err(refusal)) => {
                col.note(&words::refused(refusal, texts), tokens::color::STATE_ERROR);
                return col.rows;
            }
            Some(Ok(())) => {}
        }
        // HONEST WHEN IT MAY BE OLD: the daemon's events stopped (it
        // restarted), so a change made elsewhere is not shown until the feed
        // has them again — which it tries on its own.
        if r.events == Following::Lost {
            col.note(
                &texts.t(crate::key::INCUS_EVENTS_LOST),
                tokens::color::STATE_WARNING,
            );
            col.push(crate::chrome::AIR, Target::Nothing, true);
        }
        let picked = self.picked(r);
        let focused_list = self.focus == Pane::List;
        // EVERY ROW FIRST, then where the values go: one column for the
        // whole list, after its longest name (`chrome::section_column`).
        let mut items: Vec<(String, String, String, Option<tokens::Color>)> = Vec::new();
        match (self.kind, r.listed()) {
            (Kind::Remotes, _) => {
                for remote in r.remotes_read() {
                    let value = ronoctua_incus::host_of(&remote.address)
                        .unwrap_or_else(|| remote.address.clone());
                    items.push((remote.name.clone(), remote.name.clone(), value, None));
                }
            }
            (Kind::Instances, Some(l)) => {
                for i in &l.instances {
                    let mut value = texts.t(words::status_key(i.status));
                    // THE FIRST ADDRESS of one that runs, beside its word.
                    if let Some(ip) = i.state.as_ref().and_then(|s| s.ipv4.first()) {
                        value = texts.t_with(
                            crate::key::INCUS_LIST_ADDRESS,
                            &[("status", &value), ("address", ip)],
                        );
                    }
                    let name = format!("{} {}", words::machine_glyph(i.kind), i.name);
                    items.push((i.name.clone(), name, value, words::status_tint(i.status)));
                }
            }
            (Kind::Images, Some(l)) => {
                for i in &l.images {
                    items.push((
                        i.fingerprint.clone(),
                        format!("{} {}", words::machine_glyph(i.kind), image_name(i)),
                        size_of(i.size_bytes, texts),
                        None,
                    ));
                }
            }
            (Kind::Profiles, Some(l)) => {
                for p in &l.profiles {
                    items.push((p.name.clone(), p.name.clone(), p.description.clone(), None));
                }
            }
            (Kind::Networks, Some(l)) => {
                for n in &l.networks {
                    items.push((n.name.clone(), n.name.clone(), n.kind.clone(), None));
                }
            }
            (Kind::Storage, Some(l)) => {
                for p in &l.pools {
                    items.push((p.name.clone(), p.name.clone(), p.driver.clone(), None));
                }
            }
            (Kind::Projects, Some(l)) => {
                for p in &l.projects {
                    items.push((p.name.clone(), p.name.clone(), p.description.clone(), None));
                }
            }
            (_, None) => {}
        }
        let pairs: Vec<(u16, u16, u16)> = items
            .iter()
            .map(|(_, name, value, _)| (0, layout::width(name), layout::width(value)))
            .collect();
        let column = crate::chrome::section_column(0, room, &pairs);
        let empty = items.is_empty();
        for (key, name, value, tint) in items {
            let on = picked.as_deref() == Some(key.as_str());
            let (label_tone, value_tone) = crate::chrome::Kind::ListedSetting.tones(on);
            let mut fields = vec![piece(
                0,
                layout::elide_end(&name, column.saturating_sub(2).max(1)),
                label_tone,
            )];
            if !value.is_empty() {
                fields.push(piece(
                    column,
                    layout::elide_end(&value, room.saturating_sub(column)),
                    value_tone,
                ));
            }
            // THE ROW CHOSEN keeps its accent while the keyboard is in
            // another column: each column keeps its own selection.
            let fields = crate::chrome::tinted(fields, tint, on && focused_list);
            col.push(fields, Target::Item(key), false);
        }
        if empty {
            col.note(
                &texts.t(words::empty_key(self.kind)),
                tokens::color::TEXT_DIM,
            );
        }
        if self.kind == Kind::Projects {
            // HONEST: the view acts in the default project only.
            col.push(crate::chrome::AIR, Target::Nothing, true);
            col.note(
                &texts.t(crate::key::INCUS_PROJECTS_NOTE),
                tokens::color::TEXT_DIM,
            );
        }
        col.rows
    }

    /// The detail's tabs laid out in `room` cells: each line's tabs as
    /// `(first cell, cells, tab)`. A tab that does not fit goes to the next
    /// line, never away (the containers' rule): `S-→` reaches every tab, so
    /// every tab is drawn and clicked.
    fn tab_lines(&self, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<Vec<(u16, u16, Tab)>> {
        let mut lines = vec![Vec::new()];
        let mut at = 0u16;
        for &tab in self.kind.tabs() {
            let w = layout::width(&texts.t(words::tab_key(tab))).min(room.max(1));
            if at > 0 && at + w > room {
                lines.push(Vec::new());
                at = 0;
            }
            if let Some(line) = lines.last_mut() {
                line.push((at, w, tab));
            }
            at += w + 2;
        }
        lines
    }

    /// Whether the detail draws its strip of tabs now: an instance's, with
    /// neither the draft nor a question in its place.
    fn strip_shown(&self) -> bool {
        self.kind.tabs().len() > 1 && !self.covered()
    }

    /// One line of the strip, its first cell at `from`: the tabs' words and
    /// one zone per tab, so the pointer takes the tab it is on.
    fn tab_line(&self, from: u16, line: &[(u16, u16, Tab)], texts: &ronoctua_i18n::Texts) -> Line {
        let shown = self.tab_shown();
        let mut out = Line::default();
        for &(at, w, tab) in line {
            let color = if tab == shown {
                tokens::color::STATE_ACCENT
            } else {
                crate::chrome::Kind::Setting.tones(false).0
            };
            out.fields.push(piece(
                from + at,
                layout::elide_end(&texts.t(words::tab_key(tab)), w),
                color,
            ));
            out.zones.push(Zone {
                from: from + at,
                to: from + at + w,
                pane: Pane::Detail,
                row: usize::MAX,
                target: Target::Tab(tab),
            });
        }
        out
    }

    /// How many lines the panes' detail pins above its rows: its strip.
    fn pinned_in_panes(&self, r: &Readings, columns: u16, texts: &ronoctua_i18n::Texts) -> usize {
        if self.detail_shown(r, Layout::Panes) && self.strip_shown() {
            self.tab_lines(detail_width(columns, Layout::Panes), texts)
                .len()
        } else {
            0
        }
    }

    /// The detail's rows under its strip: the tab shown, the draft or the
    /// question.
    fn detail_rows(&self, r: &Readings, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let mut col = Col::new(room, texts);
        if let Some(q) = &self.question {
            self.question_rows(&mut col, q);
            return col.rows;
        }
        if self.drafting {
            if let Some(d) = &self.draft {
                self.draft_rows(&mut col, r, d);
            }
            return col.rows;
        }
        let Some(key) = self.picked(r) else {
            return col.rows;
        };
        match self.kind {
            Kind::Instances => {
                let Some(i) = r.instance(&key) else {
                    return col.rows;
                };
                match self.tab_shown() {
                    Tab::Overview => self.instance_overview(&mut col, r, i),
                    Tab::Snapshots => self.snapshot_rows(&mut col, r, i),
                    Tab::Limits => self.limit_rows(&mut col, i),
                    Tab::Actions => self.action_rows(&mut col, i),
                }
            }
            Kind::Images => {
                if let Some(i) = r.image(&key) {
                    self.image_rows(&mut col, i);
                }
            }
            Kind::Remotes => {
                if let Some(remote) = r.remote(&key) {
                    self.remote_rows(&mut col, r, remote);
                }
            }
            Kind::Profiles => {
                if let Some(p) = r.profile(&key) {
                    self.facts(
                        &mut col,
                        vec![
                            (crate::key::INCUS_ROW_NAME, p.name.clone()),
                            (crate::key::INCUS_ROW_DESCRIPTION, p.description.clone()),
                        ],
                    );
                }
            }
            Kind::Networks => {
                if let Some(n) = r.network(&key) {
                    let managed = texts.t(if n.managed {
                        crate::key::INCUS_YES
                    } else {
                        crate::key::INCUS_NO
                    });
                    self.facts(
                        &mut col,
                        vec![
                            (crate::key::INCUS_ROW_NAME, n.name.clone()),
                            (crate::key::INCUS_ROW_TYPE, n.kind.clone()),
                            (crate::key::INCUS_ROW_MANAGED, managed),
                        ],
                    );
                }
            }
            Kind::Storage => {
                if let Some(p) = r.pool(&key) {
                    self.pool_rows(&mut col, r, p);
                }
            }
            Kind::Projects => {
                if let Some(p) = r.project(&key) {
                    self.facts(
                        &mut col,
                        vec![
                            (crate::key::INCUS_ROW_NAME, p.name.clone()),
                            (crate::key::INCUS_ROW_DESCRIPTION, p.description.clone()),
                        ],
                    );
                }
            }
        }
        col.rows
    }

    /// Facts by their label's key; an empty value is not drawn.
    fn facts(&self, col: &mut Col<'_>, rows: Vec<(&str, String)>) {
        for (label, value) in rows {
            if value.is_empty() {
                continue;
            }
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            let label = col.texts.t(label);
            col.fact(&label, &value, on, None);
        }
    }

    fn instance_overview(&self, col: &mut Col<'_>, r: &Readings, i: &Instance) {
        let texts = col.texts;
        let mut rows: Vec<(String, String, Option<tokens::Color>)> = vec![
            (texts.t(crate::key::INCUS_ROW_NAME), i.name.clone(), None),
            (
                texts.t(crate::key::INCUS_ROW_KIND),
                texts.t(words::machine_key(i.kind)),
                None,
            ),
            (
                texts.t(crate::key::INCUS_ROW_STATUS),
                texts.t(words::status_key(i.status)),
                words::status_tint(i.status),
            ),
        ];
        if let Some(s) = &i.state {
            // EACH ADDRESS a row of its own, numbered after the first.
            for (n, ip) in s.ipv4.iter().enumerate() {
                rows.push((
                    nth_label(texts, crate::key::INCUS_ROW_IPV4, n),
                    ip.clone(),
                    None,
                ));
            }
            for (n, ip) in s.ipv6.iter().enumerate() {
                rows.push((
                    nth_label(texts, crate::key::INCUS_ROW_IPV6, n),
                    ip.clone(),
                    None,
                ));
            }
            if is_up(i.status) {
                rows.push((
                    texts.t(crate::key::INCUS_ROW_CPU),
                    words::cpu_time(s.cpu_ns, texts),
                    None,
                ));
                rows.push((
                    texts.t(crate::key::INCUS_ROW_MEMORY),
                    size_of(s.memory_bytes, texts),
                    None,
                ));
                rows.push((
                    texts.t(crate::key::INCUS_ROW_PROCESSES),
                    s.processes.to_string(),
                    None,
                ));
            }
            if s.disk_bytes > 0 {
                rows.push((
                    texts.t(crate::key::INCUS_ROW_DISK),
                    size_of(s.disk_bytes, texts),
                    None,
                ));
            }
        }
        for (key, value) in [
            (crate::key::INCUS_ROW_IMAGE, i.image.clone()),
            (crate::key::INCUS_ROW_PROFILES, i.profiles.join(", ")),
            (crate::key::INCUS_ROW_MADE, when_of(&i.created_at, r, texts)),
            (crate::key::INCUS_ROW_PROJECT, i.project.clone()),
            (crate::key::INCUS_ROW_DESCRIPTION, i.description.clone()),
        ] {
            if !value.is_empty() {
                rows.push((texts.t(key), value, None));
            }
        }
        for (label, value, tint) in rows {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            col.fact(&label, &value, on, tint);
        }
    }

    fn snapshot_rows(&self, col: &mut Col<'_>, r: &Readings, i: &Instance) {
        let texts = col.texts;
        let on = self.focus == Pane::Detail && col.rows.len() == self.at;
        col.acting(
            &texts.t(crate::key::INCUS_SNAPSHOT_TAKE),
            &texts.t(crate::key::INCUS_SNAPSHOT_TAKE_VALUE),
            on,
            Target::TakeSnapshot,
        );
        col.push(crate::chrome::AIR, Target::Nothing, true);
        if i.snapshots.is_empty() {
            col.note(
                &texts.t(crate::key::INCUS_SNAPSHOTS_NONE),
                tokens::color::TEXT_DIM,
            );
            return;
        }
        // EACH SNAPSHOT a row that acts: `RET` restores it after a question,
        // `C-d` deletes it after one. Its name is the daemon's — data.
        let pairs: Vec<(u16, u16, u16)> = i
            .snapshots
            .iter()
            .map(|s| {
                (
                    0,
                    layout::width(&s.name),
                    layout::width(&when_of(&s.created_at, r, texts)),
                )
            })
            .collect();
        let column = crate::chrome::section_column(0, col.room, &pairs);
        for s in &i.snapshots {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            let (label, value) = crate::chrome::Kind::ListedSetting.tones(on);
            let fields = vec![
                piece(
                    0,
                    layout::elide_end(&s.name, column.saturating_sub(2).max(1)),
                    label,
                ),
                piece(
                    column,
                    layout::elide_end(
                        &when_of(&s.created_at, r, texts),
                        col.room.saturating_sub(column),
                    ),
                    value,
                ),
            ];
            col.push(fields, Target::Snapshot(s.name.clone()), false);
        }
    }

    fn limit_rows(&self, col: &mut Col<'_>, i: &Instance) {
        let texts = col.texts;
        for limit in Limit::ALL {
            if limit == Limit::Processes && i.kind == Machine::Vm {
                continue;
            }
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            let mut value = limit_text(limit, i.limits, texts);
            // A PROFILE'S LIMIT says so: it is not the instance's own, and
            // nothing typed here takes it away.
            if limit.is_set(i.limits) && !limit.is_set(i.own_limits) {
                value = texts.t_with(crate::key::INCUS_LIMIT_FROM_PROFILE, &[("value", &value)]);
            }
            col.acting(
                &texts.t(words::limit_key(limit)),
                &value,
                on,
                Target::Limit(limit),
            );
        }
        col.push(crate::chrome::AIR, Target::Nothing, true);
        col.note(
            &texts.t(crate::key::INCUS_LIMITS_NOTE),
            tokens::color::TEXT_DIM,
        );
        if i.kind == Machine::Vm {
            col.note(
                &texts.t(crate::key::INCUS_LIMITS_VM_PROCESSES),
                tokens::color::TEXT_DIM,
            );
        }
    }

    fn action_rows(&self, col: &mut Col<'_>, i: &Instance) {
        let texts = col.texts;
        for what in Use::ALL {
            // THE SCREEN IS A VIRTUAL MACHINE'S: a container has none.
            if what == Use::Screen && i.kind != Machine::Vm {
                continue;
            }
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            let label = texts.t(words::use_key(what));
            // WHAT NEEDS IT RUNNING IS DRAWN REFUSING while it is not —
            // reachable, so its `RET` can say why.
            if what.needs_running() && i.status != Status::Running {
                col.refusing(
                    &label,
                    &texts.t(crate::key::INCUS_USE_NOT_RUNNING),
                    on,
                    Target::Use(what),
                );
            } else {
                col.acting(
                    &label,
                    &texts.t(words::use_where_key(what)),
                    on,
                    Target::Use(what),
                );
            }
        }
    }

    fn image_rows(&self, col: &mut Col<'_>, i: &Image) {
        let texts = col.texts;
        let mut rows: Vec<(String, String)> = Vec::new();
        // EACH ALIAS a row of its own, numbered.
        for (n, alias) in i.aliases.iter().enumerate() {
            rows.push((
                nth_label(texts, crate::key::INCUS_ROW_ALIAS, n),
                alias.clone(),
            ));
        }
        for (key, value) in [
            (crate::key::INCUS_ROW_DESCRIPTION, i.description.clone()),
            (
                crate::key::INCUS_ROW_KIND,
                texts.t(words::machine_key(i.kind)),
            ),
            (crate::key::INCUS_ROW_SIZE, size_of(i.size_bytes, texts)),
            (crate::key::INCUS_ROW_FINGERPRINT, i.fingerprint.clone()),
        ] {
            if !value.is_empty() {
                rows.push((texts.t(key), value));
            }
        }
        for (label, value) in rows {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            col.fact(&label, &value, on, None);
        }
        col.push(crate::chrome::AIR, Target::Nothing, true);
        let on = self.focus == Pane::Detail && col.rows.len() == self.at;
        col.acting(
            &texts.t(crate::key::INCUS_IMAGE_LAUNCH),
            &texts.t(crate::key::INCUS_IMAGE_LAUNCH_VALUE),
            on,
            Target::LaunchFrom,
        );
    }

    fn remote_rows(&self, col: &mut Col<'_>, r: &Readings, remote: &Remote) {
        let texts = col.texts;
        self.facts(
            col,
            vec![
                (crate::key::INCUS_ROW_NAME, remote.name.clone()),
                (crate::key::INCUS_ROW_ADDRESS, remote.address.clone()),
                (
                    crate::key::INCUS_ROW_PROTOCOL,
                    texts.t(words::protocol_key(remote.protocol)),
                ),
            ],
        );
        col.heading(crate::key::INCUS_HEADING_CATALOGUE);
        let hosts = ronoctua_incus::hosts_of(&remote.address, remote.protocol);
        // NOTHING IS ASKED BEFORE EVERY HOST IS LET: the row that lets them,
        // in place of the catalogue.
        if !r.lets(&hosts) {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            col.consent(&r.unlet(&hosts), crate::key::INCUS_CONSENT_CATALOGUE, on);
            return;
        }
        let offered = match r.catalogue_of(&remote.name) {
            None => {
                col.note(
                    &texts.t_with(
                        crate::key::INCUS_CATALOGUE_READING,
                        &[("remote", &remote.name)],
                    ),
                    tokens::color::TEXT_DIM,
                );
                return;
            }
            Some(Err(refusal)) => {
                col.note(&words::refused(*refusal, texts), tokens::color::STATE_ERROR);
                return;
            }
            Some(Ok(offered)) => offered,
        };
        if offered.is_empty() {
            col.note(
                &texts.t(crate::key::INCUS_CATALOGUE_EMPTY),
                tokens::color::TEXT_DIM,
            );
            return;
        }
        let value_of = |o: &Offered| {
            texts.t_with(
                crate::key::INCUS_OFFERED_VALUE,
                &[
                    ("kind", &texts.t(words::machine_key(o.kind))),
                    ("size", &size_of(o.size_bytes, texts)),
                ],
            )
        };
        let pairs: Vec<(u16, u16, u16)> = offered
            .iter()
            .map(|o| (0, layout::width(&o.alias), layout::width(&value_of(o))))
            .collect();
        let column = crate::chrome::section_column(0, col.room, &pairs);
        for o in offered {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            let (label, value) = crate::chrome::Kind::ListedSetting.tones(on);
            let fields = vec![
                piece(
                    0,
                    layout::elide_end(&o.alias, column.saturating_sub(2).max(1)),
                    label,
                ),
                piece(
                    column,
                    layout::elide_end(&value_of(o), col.room.saturating_sub(column)),
                    value,
                ),
            ];
            col.push(fields, Target::Offered(o.alias.clone(), o.kind), false);
        }
    }

    fn pool_rows(&self, col: &mut Col<'_>, r: &Readings, p: &Pool) {
        let texts = col.texts;
        self.facts(
            col,
            vec![
                (crate::key::INCUS_ROW_NAME, p.name.clone()),
                (crate::key::INCUS_ROW_DRIVER, p.driver.clone()),
            ],
        );
        col.heading(crate::key::INCUS_HEADING_VOLUMES);
        match r.volumes_of(&p.name) {
            None => col.note(&texts.t(crate::key::INCUS_READING), tokens::color::TEXT_DIM),
            Some(Err(refusal)) => {
                col.note(&words::refused(*refusal, texts), tokens::color::STATE_ERROR);
            }
            Some(Ok(volumes)) if volumes.is_empty() => col.note(
                &texts.t(crate::key::INCUS_VOLUMES_NONE),
                tokens::color::TEXT_DIM,
            ),
            Some(Ok(volumes)) => {
                // EACH VOLUME: its name, and its type as the daemon says it;
                // `RET` copies its name, which is also what the cursor keeps.
                for v in volumes {
                    let on = self.focus == Pane::Detail && col.rows.len() == self.at;
                    col.fact_copying(&v.name, &v.kind, &v.name, on);
                }
            }
        }
    }

    fn question_rows(&self, col: &mut Col<'_>, q: &Question) {
        let texts = col.texts;
        let sentence = match q {
            Question::Delete {
                name,
                running,
                snapshots,
            } => {
                let mut s = texts.t_with(crate::key::INCUS_ASK_DELETE, &[("name", name)]);
                if *running {
                    s.push(' ');
                    s.push_str(&texts.t(crate::key::INCUS_ASK_DELETE_RUNNING));
                }
                if *snapshots {
                    s.push(' ');
                    s.push_str(&texts.t(crate::key::INCUS_ASK_DELETE_SNAPSHOTS));
                }
                s
            }
            Question::DeleteImage { label, .. } => {
                texts.t_with(crate::key::INCUS_ASK_DELETE_IMAGE, &[("name", label)])
            }
            Question::DeleteSnapshot { name, snapshot } => texts.t_with(
                crate::key::INCUS_ASK_DELETE_SNAPSHOT,
                &[("name", name), ("snapshot", snapshot)],
            ),
            Question::Restore { name, snapshot } => texts.t_with(
                crate::key::INCUS_ASK_RESTORE,
                &[("name", name), ("snapshot", snapshot)],
            ),
        };
        col.note(&sentence, tokens::color::TEXT_PRIMARY);
        col.push(crate::chrome::AIR, Target::Nothing, true);
        // "NO" FIRST, and the cursor is born on it. "Yes" is in the colour
        // of danger: what it does cannot be undone.
        for (yes, key) in [
            (false, crate::key::INCUS_ANSWER_NO),
            (true, crate::key::INCUS_ANSWER_YES),
        ] {
            let on = self.yes == yes;
            let color = if on {
                tokens::color::TEXT_HERE_ACTS
            } else if yes {
                tokens::color::STATE_ERROR
            } else {
                crate::chrome::Kind::Setting.tones(false).0
            };
            col.push(
                vec![piece(0, layout::elide_end(&texts.t(key), col.room), color)],
                Target::Answer(yes),
                false,
            );
        }
    }

    fn draft_rows(&self, col: &mut Col<'_>, r: &Readings, d: &Draft) {
        let texts = col.texts;
        col.heading(crate::key::INCUS_NEW_TITLE);
        let unchosen = || texts.t(crate::key::INCUS_UNCHOSEN);
        for field in d.fields() {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            let value = match field {
                Field::Machine => texts.t(words::machine_key(d.machine)),
                Field::Name if d.name.is_empty() => unchosen(),
                Field::Name => d.name.clone(),
                Field::Image if d.image.is_empty() => unchosen(),
                Field::Image => d.image.clone(),
                Field::Limit(limit) => limit_text(limit, d.limits, texts),
            };
            col.acting(
                &texts.t(words::field_key(field)),
                &value,
                on,
                Target::Field(field),
            );
        }
        // WHAT THE IMAGE'S LINE TAKES, with the remotes there are.
        col.push(crate::chrome::AIR, Target::Nothing, true);
        let remotes: Vec<&str> = r.remotes_read().iter().map(|x| x.name.as_str()).collect();
        col.note(
            &texts.t_with(
                crate::key::INCUS_IMAGE_HINT,
                &[("remotes", &remotes.join(", "))],
            ),
            tokens::color::TEXT_DIM,
        );
        col.push(crate::chrome::AIR, Target::Nothing, true);
        // A REMOTE'S IMAGE: its hosts let first, on the row that lets them.
        let hosts = d.hosts();
        if !r.lets(&hosts) {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            col.consent(&r.unlet(&hosts), crate::key::INCUS_CONSENT_LAUNCH, on);
            col.push(crate::chrome::AIR, Target::Nothing, true);
        }
        let on = self.focus == Pane::Detail && col.rows.len() == self.at;
        col.acting(
            &texts.t(crate::key::INCUS_CREATE),
            &texts.t(crate::key::INCUS_CREATE_VALUE),
            on,
            Target::Create,
        );
    }

    // -- composing the lines ---------------------------------------------

    /// Every line the door draws, at its width, in its layout, `room` lines
    /// tall in the panes (each column scrolls on its own inside it).
    #[must_use]
    pub fn lines(
        &self,
        r: &Readings,
        columns: u16,
        layout: Layout,
        room: usize,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Line> {
        match layout {
            Layout::Panes => self.panes(r, columns, room, texts),
            Layout::Stacked => self.stacked(r, columns, texts),
        }
    }

    fn panes(
        &self,
        r: &Readings,
        columns: u16,
        room: usize,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Line> {
        let at = places(columns);
        let kinds = self.kinds_rows(r, at.kinds.1, texts);
        let list = self.list_rows(r, at.list.1.saturating_sub(BAR_CELLS), texts);
        let detail_room = at.detail.1.saturating_sub(BAR_CELLS);
        let shown = self.detail_shown(r, Layout::Panes);
        let strip = if shown && self.strip_shown() {
            self.tab_lines(detail_room, texts)
        } else {
            Vec::new()
        };
        let detail = if shown {
            self.detail_rows(r, detail_room, texts)
        } else {
            Vec::new()
        };
        // THE STRIP IS PINNED to the detail's first lines; its rows scroll
        // under it.
        let pinned = strip.len();
        let room = room.max(1);
        let tall = kinds
            .len()
            .max(list.len().min(room))
            .max((detail.len() + pinned).min(room))
            .min(room);
        let [_, list_off, detail_off] = self.offsets;
        let list_off = list_off.min(list.len().saturating_sub(room));
        let detail_room = room.saturating_sub(pinned);
        let detail_off = detail_off.min(detail.len().saturating_sub(detail_room));
        let mut out = Vec::with_capacity(tall);
        for i in 0..tall {
            let mut line = Line::default();
            let put = |line: &mut Line, pane: Pane, row: usize, r: &Row| {
                let (from, width) = at.of(pane);
                // A SCROLLING COLUMN'S LAST CELL IS ITS BAR'S, and a band or
                // a click stops before it (`crate::bars`).
                let bar = if pane == Pane::Kinds { 0 } else { BAR_CELLS };
                line.fields.extend(r.fields.iter().map(|f| Cell {
                    at: f.at + from,
                    ..f.clone()
                }));
                line.zones.push(Zone {
                    from,
                    to: from + width.saturating_sub(bar),
                    pane,
                    row,
                    target: r.target.clone(),
                });
            };
            if let Some(k) = kinds.get(i) {
                put(&mut line, Pane::Kinds, i, k);
            }
            if let Some(l) = list.get(list_off + i) {
                put(&mut line, Pane::List, list_off + i, l);
            }
            if let Some(tabs) = strip.get(i) {
                let pinned_line = self.tab_line(at.detail.0, tabs, texts);
                line.fields.extend(pinned_line.fields);
                line.zones.extend(pinned_line.zones);
            } else if let Some(d) = detail.get(detail_off + i - pinned) {
                put(&mut line, Pane::Detail, detail_off + i - pinned, d);
            }
            out.push(line);
        }
        out
    }

    fn stacked(&self, r: &Readings, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        let (left, room) = extent(columns);
        let mut out = Vec::new();
        let whole = |row: usize, pane: Pane, r: &Row| Line {
            fields: r
                .fields
                .iter()
                .map(|f| Cell {
                    at: f.at + left,
                    ..f.clone()
                })
                .collect(),
            zones: vec![Zone {
                from: left,
                to: left + room,
                pane,
                row,
                target: r.target.clone(),
            }],
        };
        if self.open && self.detail_shown(r, Layout::Stacked) {
            // THE DETAIL OVER THE LIST: whose it is, its tabs, its rows. The
            // draft's first row is its own title ("A new instance", which the
            // panes' detail draws too), so it gets no second one here.
            if !(self.drafting && self.question.is_none()) {
                let title = self
                    .picked(r)
                    .map(|k| self.title_of(r, &k))
                    .unwrap_or_default();
                let (label, _) = crate::chrome::Kind::Setting.tones(false);
                out.push(Line {
                    fields: vec![
                        piece(left, words::kind_glyph(self.kind).to_string(), label),
                        piece(
                            left + 2,
                            layout::elide_end(&title, room.saturating_sub(2)),
                            tokens::color::TEXT_HEADING,
                        ),
                    ],
                    zones: Vec::new(),
                });
            }
            if self.strip_shown() {
                for tabs in self.tab_lines(room, texts) {
                    out.push(self.tab_line(left, &tabs, texts));
                }
            }
            for (n, row) in self.detail_rows(r, room, texts).iter().enumerate() {
                out.push(whole(n, Pane::Detail, row));
            }
            return out;
        }
        // THE KINDS ON A STRIP, each a zone, the chosen one in the accent.
        let mut strip = Line::default();
        let mut cell = 0u16;
        for kind in Kind::ALL {
            let count = r.count(kind).map(|n| n.to_string()).unwrap_or_default();
            let name = texts.t(words::kind_key(kind));
            let text = if count.is_empty() {
                format!("{} {name}", words::kind_glyph(kind))
            } else {
                format!("{} {name} {count}", words::kind_glyph(kind))
            };
            // A KIND THAT DOES NOT FIT goes to the next line: TAB reaches
            // every kind, so every kind is drawn.
            let text = layout::elide_end(&text, room);
            let w = layout::width(&text);
            if cell > 0 && cell + w > room {
                out.push(std::mem::take(&mut strip));
                cell = 0;
            }
            let color = if kind == self.kind {
                tokens::color::STATE_ACCENT
            } else {
                crate::chrome::Kind::Setting.tones(false).0
            };
            strip.fields.push(piece(left + cell, text, color));
            strip.zones.push(Zone {
                from: left + cell,
                to: left + cell + w,
                pane: Pane::Kinds,
                row: kind.index(),
                target: Target::Kind(kind),
            });
            // THREE CELLS between two kinds: each is a glyph, a word and a
            // number, and two would read as one run.
            cell += w + 3;
        }
        out.push(strip);
        for (n, row) in self.list_rows(r, room, texts).iter().enumerate() {
            out.push(whole(n, Pane::List, row));
        }
        out
    }

    /// What the detail is about, as its title.
    fn title_of(&self, r: &Readings, key: &str) -> String {
        match self.kind {
            Kind::Images => r.image(key).map_or_else(|| short(key), image_name),
            _ => key.to_owned(),
        }
    }

    /// The line the keyboard's cursor is on, for the door to keep in sight
    /// — stacked only: the panes scroll inside themselves.
    #[must_use]
    pub fn cursor_line(&self, lines: &[Line], r: &Readings) -> Option<usize> {
        let picked = self.picked(r);
        lines.iter().position(|l| {
            l.zones.iter().any(|z| match (&z.target, z.pane) {
                (Target::Answer(yes), _) => *yes == self.yes,
                (Target::Item(k), Pane::List) => {
                    self.focus == Pane::List && Some(k) == picked.as_ref()
                }
                (Target::Tab(_) | Target::Kind(_), _) => false,
                (_, Pane::Detail) => self.focus == Pane::Detail && z.row == self.at,
                _ => false,
            })
        })
    }

    // -- keeping the cursors ----------------------------------------------

    /// The detail's cursor put on row `n` — by a key, a click, or the view
    /// itself (a new tab, a new row chosen): what it is on is read again at
    /// the next [`View::settle`], never looked for.
    fn put_at(&mut self, n: usize) {
        self.at = n;
        self.held = None;
    }

    /// Keeps every cursor on a row that exists, after new readings: the row
    /// chosen by identity, the detail's by WHAT IT IS ON ([`Held`]) — a
    /// snapshot that expired above it does not move it to the next one.
    ///
    /// NEVER THE PAGE: a column the person scrolled stays where it is when
    /// data arrives. [`View::keep_in_sight`] is the door's, after a key or a
    /// click of the person's.
    pub fn settle(
        &mut self,
        r: &Readings,
        columns: u16,
        layout: Layout,
        texts: &ronoctua_i18n::Texts,
    ) {
        let keys = r.keys(self.kind);
        let read = if self.kind == Kind::Remotes {
            r.remotes.as_ref().is_some_and(Result::is_ok)
        } else {
            r.listed().is_some()
        };
        if let Some(p) = &self.picked[self.kind.index()]
            && !keys.contains(p)
            && read
        {
            self.picked[self.kind.index()] = None;
        }
        if !self.detail_shown(r, layout) && self.focus == Pane::Detail {
            self.focus = Pane::List;
            self.open = false;
        }
        // A QUESTION'S ROWS are not the detail's: its cursor is its own
        // (`yes`), and the detail's is left as it was, for when it goes.
        if self.question.is_some() {
            return;
        }
        let rows = self.detail_rows(r, detail_width(columns, layout), texts);
        let about = (self.kind, self.picked(r), self.tab_shown(), self.drafting);
        // THE CURSOR FOLLOWS ITS THING to wherever its row is now.
        if let Some(n) = self
            .held
            .as_ref()
            .filter(|h| h.about == about)
            .and_then(|h| nth_row(&rows, &h.target, h.nth))
        {
            self.at = n;
        }
        self.at = self.at.min(rows.len().saturating_sub(1));
        // NEVER ON A HEADING: the first row that acts at or after it, or the
        // last before it.
        if rows.get(self.at).is_some_and(|row| row.heading) {
            self.at = rows
                .iter()
                .enumerate()
                .skip(self.at)
                .find(|(_, row)| !row.heading)
                .or_else(|| {
                    rows.iter()
                        .enumerate()
                        .take(self.at)
                        .rfind(|(_, row)| !row.heading)
                })
                .map_or(self.at, |(n, _)| n);
        }
        self.held = rows.get(self.at).map(|row| Held {
            about,
            target: row.target.clone(),
            nth: rows[..self.at]
                .iter()
                .filter(|x| x.target == row.target)
                .count(),
        });
    }

    /// The offsets of the panes follow their cursors — the door calls it
    /// after a KEY or a CLICK of the person's, never for data that arrived
    /// (AGENTS: data from a daemon never moves the page). Only in the panes:
    /// a stacked door scrolls its whole region to the cursor line.
    pub fn keep_in_sight(
        &mut self,
        r: &Readings,
        columns: u16,
        layout: Layout,
        room: usize,
        texts: &ronoctua_i18n::Texts,
    ) {
        if layout != Layout::Panes || room == 0 {
            return;
        }
        let at = places(columns);
        let list = self.list_rows(r, at.list.1.saturating_sub(BAR_CELLS), texts);
        let picked = self.picked(r);
        if let Some(i) = list
            .iter()
            .position(|row| matches!(&row.target, Target::Item(k) if Some(k) == picked.as_ref()))
        {
            self.offsets[1] = follow(self.offsets[1], i, room, list.len());
        }
        let pinned = self.pinned_in_panes(r, columns, texts);
        let detail_room = room.saturating_sub(pinned).max(1);
        let detail = self.detail_rows(r, at.detail.1.saturating_sub(BAR_CELLS), texts);
        self.offsets[2] = follow(self.offsets[2], self.at, detail_room, detail.len());
    }

    /// The panes' scroll, as the door's bars and wheel read it.
    #[must_use]
    pub fn scrolls(
        &self,
        r: &Readings,
        columns: u16,
        room: usize,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<PaneScroll> {
        let at = places(columns);
        let list = self
            .list_rows(r, at.list.1.saturating_sub(BAR_CELLS), texts)
            .len();
        let shown = self.detail_shown(r, Layout::Panes);
        let pinned = self.pinned_in_panes(r, columns, texts);
        let detail = if shown {
            self.detail_rows(r, at.detail.1.saturating_sub(BAR_CELLS), texts)
                .len()
        } else {
            0
        };
        let detail_room = room.saturating_sub(pinned);
        vec![
            PaneScroll {
                pane: Pane::List,
                first: 0,
                lines: room,
                total: list,
                offset: self.offsets[1].min(list.saturating_sub(room)),
                column: at.list.0 + at.list.1.saturating_sub(BAR_CELLS),
            },
            PaneScroll {
                pane: Pane::Detail,
                first: pinned,
                lines: detail_room,
                total: detail,
                offset: self.offsets[2].min(detail.saturating_sub(detail_room)),
                column: at.detail.0 + at.detail.1.saturating_sub(BAR_CELLS),
            },
        ]
    }

    /// A pane put at `offset` — the wheel over it, or its bar dragged. The
    /// cursor stays where it is: a scroll by the pointer is not a key.
    pub fn scroll_to(&mut self, pane: Pane, offset: usize) {
        match pane {
            Pane::Kinds => {}
            Pane::List => self.offsets[1] = offset,
            Pane::Detail => self.offsets[2] = offset,
        }
    }

    // -- the pointer --------------------------------------------------------

    /// A click on drawn line `line`, cell `cell` — or, with no cell, the
    /// line's first zone. Answers whether the thing under it was ALREADY
    /// chosen: the second click, which the door turns into `RET`
    /// (invariant 19).
    pub fn clicked(
        &mut self,
        lines: &[Line],
        line: usize,
        cell: Option<u16>,
        layout: Layout,
        r: &Readings,
    ) -> bool {
        let Some(l) = lines.get(line) else {
            return false;
        };
        let zone = match cell {
            Some(c) => l
                .zones
                .iter()
                .find(|z| crate::chrome::in_cell(c, z.from, z.to.saturating_sub(z.from))),
            None => l.zones.first(),
        };
        let Some(zone) = zone.cloned() else {
            return false;
        };
        // A CLICK THAT TAKES ANYTHING BUT AN ANSWER is "no", as any other key
        // is: the question is taken down before what was clicked is taken.
        if self.question.is_some() && !matches!(zone.target, Target::Answer(_) | Target::Nothing) {
            self.unask();
        }
        // AND ONE OUTSIDE THE DRAFT puts it away — kept, as `Esc` keeps it.
        if self.drafting && zone.pane != Pane::Detail && zone.target.acts() {
            self.undraft();
        }
        match &zone.target {
            Target::Nothing => false,
            Target::Kind(kind) => {
                self.choose_kind(*kind);
                match layout {
                    // THE COLUMN: the first click chooses, the second takes
                    // the keyboard to the list — `RET` on a kind.
                    Layout::Panes => {
                        let again = self.focus == Pane::Kinds;
                        self.focus = Pane::Kinds;
                        again
                    }
                    // THE STRIP: a kind clicked there is the kind shown.
                    Layout::Stacked => {
                        self.focus = Pane::List;
                        false
                    }
                }
            }
            Target::Tab(tab) => {
                self.tab = *tab;
                self.put_at(0);
                self.offsets[2] = 0;
                false
            }
            Target::Item(key) => {
                let again = self.focus == Pane::List && self.picked(r).as_deref() == Some(key);
                self.pick(key.clone());
                self.focus = Pane::List;
                again
            }
            Target::Answer(yes) => {
                let again = self.yes == *yes;
                self.yes = *yes;
                again
            }
            _ => {
                // A ROW OF THE DETAIL: a row drawn over several lines (the
                // consent's) is one row, its first line.
                let mut row = zone.row;
                while row > 0
                    && lines.iter().flat_map(|l| l.zones.iter()).any(|z| {
                        z.pane == Pane::Detail && z.row == row - 1 && z.target == zone.target
                    })
                {
                    row -= 1;
                }
                let again = self.focus == Pane::Detail && self.at == row;
                self.focus = Pane::Detail;
                self.put_at(row);
                again
            }
        }
    }

    /// The cells a hover band may cover on drawn line `line` under cell
    /// `cell`: the zone the pointer is in, so a band never says two columns
    /// are one row. `None` over nothing that acts.
    #[must_use]
    pub fn hover_span(lines: &[Line], line: usize, cell: u16) -> Option<(u16, u16)> {
        let z = lines
            .get(line)?
            .zones
            .iter()
            .find(|z| crate::chrome::in_cell(cell, z.from, z.to.saturating_sub(z.from)))?;
        z.target.acts().then_some((z.from, z.to))
    }

    fn pick(&mut self, key: String) {
        let i = self.kind.index();
        if self.picked[i].as_ref() != Some(&key) {
            self.picked[i] = Some(key);
            self.put_at(0);
            self.offsets[2] = 0;
        }
    }

    fn choose_kind(&mut self, kind: Kind) {
        if self.kind != kind {
            self.kind = kind;
            self.put_at(0);
            self.offsets[1] = 0;
            self.offsets[2] = 0;
            self.open = false;
            if self.focus == Pane::Detail {
                self.focus = Pane::List;
            }
        }
    }

    fn move_in_list(&mut self, r: &Readings, forward: bool) {
        let keys = r.keys(self.kind);
        if keys.is_empty() {
            return;
        }
        let now = self
            .picked(r)
            .and_then(|k| keys.iter().position(|x| *x == k))
            .unwrap_or(0);
        let next = if forward {
            (now + 1).min(keys.len() - 1)
        } else {
            now.saturating_sub(1)
        };
        self.pick(keys[next].clone());
    }

    fn move_in_detail(&mut self, rows: &[Row], forward: bool) {
        let mut i = self.at;
        loop {
            let next = if forward {
                i.checked_add(1)
            } else {
                i.checked_sub(1)
            };
            match next {
                Some(n) if n < rows.len() => i = n,
                _ => break,
            }
            if !rows[i].heading {
                self.put_at(i);
                break;
            }
        }
    }

    // -- keys ---------------------------------------------------------------

    /// Acts on a key. The WIDTH and the LAYOUT are the door's: the rows a
    /// key acts on are the rows it drew.
    pub fn key(
        &mut self,
        press: Press,
        r: &Readings,
        columns: u16,
        layout: Layout,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        if self.question.is_some() {
            return self.answer_key(press);
        }
        let detail = self.detail_rows(r, detail_width(columns, layout), texts);
        match press {
            Press::Up | Press::Down => {
                let forward = press == Press::Down;
                match self.focus {
                    Pane::Kinds => self.choose_kind(self.kind.turned(forward)),
                    Pane::List => self.move_in_list(r, forward),
                    Pane::Detail => self.move_in_detail(&detail, forward),
                }
                Errand::Nothing
            }
            Press::NextKind | Press::PreviousKind => {
                self.undraft();
                self.choose_kind(self.kind.turned(press == Press::NextKind));
                Errand::Nothing
            }
            Press::NextTab | Press::PreviousTab => {
                let tabs = self.kind.tabs();
                if tabs.len() > 1 && self.detail_shown(r, layout) && !self.covered() {
                    let i = tabs
                        .iter()
                        .position(|t| *t == self.tab_shown())
                        .unwrap_or(0);
                    let n = tabs.len();
                    self.tab = tabs[if press == Press::NextTab {
                        (i + 1) % n
                    } else {
                        (i + n - 1) % n
                    }];
                    self.put_at(0);
                    self.offsets[2] = 0;
                }
                Errand::Nothing
            }
            Press::Escape => {
                if self.drafting {
                    self.undraft();
                    return Errand::Nothing;
                }
                match self.focus {
                    Pane::Detail => {
                        self.focus = Pane::List;
                        self.open = false;
                        Errand::Nothing
                    }
                    _ => Errand::Close,
                }
            }
            Press::Left => {
                match (self.focus, layout) {
                    (Pane::Detail, _) if self.drafting => self.undraft(),
                    (Pane::Detail, _) => {
                        self.focus = Pane::List;
                        self.open = false;
                    }
                    (Pane::List, Layout::Panes) => self.focus = Pane::Kinds,
                    _ => {}
                }
                Errand::Nothing
            }
            Press::Right => {
                match self.focus {
                    Pane::Kinds => self.focus = Pane::List,
                    Pane::List => self.enter_detail(r, layout),
                    Pane::Detail => {}
                }
                Errand::Nothing
            }
            Press::Enter => match self.focus {
                Pane::Kinds => {
                    self.focus = Pane::List;
                    Errand::Nothing
                }
                Pane::List => {
                    self.enter_detail(r, layout);
                    Errand::Nothing
                }
                Pane::Detail => match detail.get(self.at).map(|row| row.target.clone()) {
                    Some(target) => self.enter_on(target, r, texts),
                    None => Errand::Nothing,
                },
            },
            Press::Launch => {
                let over = self.launch_over(r, &detail);
                self.open_draft(over);
                Errand::Nothing
            }
            Press::Allow => {
                let hosts = if self.drafting {
                    self.draft.as_ref().map(|d| r.unlet(&d.hosts()))
                } else if self.kind == Kind::Remotes && self.detail_shown(r, layout) {
                    self.picked(r)
                        .and_then(|k| r.remote(&k).cloned())
                        .map(|remote| {
                            r.unlet(&ronoctua_incus::hosts_of(&remote.address, remote.protocol))
                        })
                } else {
                    None
                };
                match hosts {
                    Some(h) if !h.is_empty() => Errand::Allow(h),
                    _ => Errand::Nothing,
                }
            }
            Press::CopyImage => match detail.get(self.at).map(|row| row.target.clone()) {
                Some(Target::Offered(alias, machine)) if self.focus == Pane::Detail => {
                    self.copy_offered(r, &alias, machine, texts)
                }
                _ => self.cannot(texts),
            },
            Press::Import => {
                self.asking = Some(Asking::Import);
                Errand::Ask(IMPORT, None)
            }
            Press::Delete => self.delete(r, &detail, texts),
            Press::StartStop
            | Press::Restart
            | Press::Freeze
            | Press::Console
            | Press::Screen
            | Press::Shell
            | Press::Push
            | Press::Pull
            | Press::Export => self.on_instance(press, r, texts),
        }
    }

    /// `RET` on a row of the detail.
    fn enter_on(&mut self, target: Target, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        match target {
            Target::Copy(text) if !text.is_empty() => Errand::Copy(text),
            Target::Snapshot(snapshot) => match self.picked_instance(r) {
                Some(i) => {
                    let name = i.name.clone();
                    self.ask(Question::Restore { name, snapshot });
                    Errand::Nothing
                }
                None => Errand::Nothing,
            },
            Target::TakeSnapshot => match self.picked_instance(r) {
                Some(i) => {
                    let initial = next_snapshot(&i.snapshots);
                    self.asking = Some(Asking::Snapshot {
                        name: i.name.clone(),
                    });
                    Errand::Ask(SNAPSHOT, Some(initial))
                }
                None => Errand::Nothing,
            },
            Target::Limit(limit) => match self.picked_instance(r) {
                Some(i) => {
                    self.asking = Some(Asking::Limit {
                        name: i.name.clone(),
                        limit,
                        limits: i.limits,
                        own: i.own_limits,
                    });
                    Errand::Ask(limit.question(), limit.typed(i.limits))
                }
                None => Errand::Nothing,
            },
            Target::Use(what) => self.use_row(what, r, texts),
            Target::LaunchFrom => {
                let over = self.launch_over(r, &[]);
                self.open_draft(over);
                Errand::Nothing
            }
            Target::Offered(alias, machine) => {
                let over = self.offered_source(r, &alias, machine);
                self.open_draft(over);
                Errand::Nothing
            }
            Target::Consent(hosts) => Errand::Allow(hosts),
            Target::Field(field) => self.ask_field(field),
            Target::Create => self.create(r, texts),
            Target::Copy(_)
            | Target::Nothing
            | Target::Kind(_)
            | Target::Item(_)
            | Target::Tab(_)
            | Target::Answer(_) => Errand::Nothing,
        }
    }

    /// A key while a question is up.
    fn answer_key(&mut self, press: Press) -> Errand {
        match press {
            Press::Up | Press::Down | Press::Left | Press::Right => {
                self.yes = matches!(press, Press::Down | Press::Right);
                Errand::Nothing
            }
            Press::Enter => {
                let yes = self.yes;
                match (self.unask(), yes) {
                    (Some(Question::Delete { name, .. }), true) => Errand::Do(
                        Deed::Act {
                            name: name.clone(),
                            action: Action::Delete,
                        },
                        name,
                    ),
                    (Some(Question::DeleteImage { fingerprint, label }), true) => {
                        Errand::Do(Deed::DeleteImage { fingerprint }, label)
                    }
                    (Some(Question::DeleteSnapshot { name, snapshot }), true) => Errand::Do(
                        Deed::DeleteSnapshot {
                            name: name.clone(),
                            snapshot: snapshot.clone(),
                        },
                        format!("{name}/{snapshot}"),
                    ),
                    (Some(Question::Restore { name, snapshot }), true) => Errand::Do(
                        Deed::Restore {
                            name: name.clone(),
                            snapshot: snapshot.clone(),
                        },
                        format!("{name}/{snapshot}"),
                    ),
                    _ => Errand::Nothing,
                }
            }
            // EVERY OTHER KEY IS "NO": a question somebody walked away from
            // is not a yes.
            _ => {
                self.unask();
                Errand::Nothing
            }
        }
    }

    fn ask(&mut self, q: Question) {
        if !self.covered() {
            self.before = (self.focus, self.open);
        }
        self.question = Some(q);
        self.yes = false;
        self.open = true;
        self.focus = Pane::Detail;
    }

    /// The question taken down, whatever the answer: the keyboard goes back
    /// where it was (the draft's rows included, when it was asked over them).
    fn unask(&mut self) -> Option<Question> {
        let q = self.question.take();
        self.yes = false;
        if q.is_some() && !self.drafting {
            (self.focus, self.open) = self.before;
        }
        q
    }

    /// The draft drawn, over `over` when a row or a catalogue's image said
    /// which: a new one when there was none, the kept one otherwise — its
    /// image replaced by the one it was opened over.
    fn open_draft(&mut self, over: Option<(Source, String, Machine)>) {
        let mut d = self.draft.take().unwrap_or_else(Draft::fresh);
        if let Some((source, image, machine)) = over {
            d.source = Some(source);
            d.image = image;
            d.machine = machine;
            if machine == Machine::Vm {
                d.limits.processes = None;
            }
        }
        self.draft = Some(d);
        if !self.covered() {
            self.before = (self.focus, self.open);
        }
        self.drafting = true;
        self.open = true;
        self.focus = Pane::Detail;
        // THE CURSOR ON THE FIRST ROW THAT IS STILL TO BE CHOSEN — under
        // the draft's title, its first row (`draft_rows`).
        if let Some(d) = &self.draft {
            let first = if d.name.is_empty() {
                Field::Name
            } else if d.source.is_none() {
                Field::Image
            } else {
                Field::Machine
            };
            self.put_at(1 + d.fields().iter().position(|f| *f == first).unwrap_or(0));
        }
        self.offsets[2] = 0;
    }

    /// The draft put away — kept, so nothing typed is lost.
    fn undraft(&mut self) {
        if self.drafting {
            self.drafting = false;
            self.question = None;
            (self.focus, self.open) = self.before;
            self.put_at(0);
        }
    }

    /// The image the draft opens over: the image chosen in the images, or
    /// the catalogue's image under the cursor.
    fn launch_over(&self, r: &Readings, detail: &[Row]) -> Option<(Source, String, Machine)> {
        match self.kind {
            Kind::Images if !self.drafting => {
                let i = self.picked(r).and_then(|k| r.image(&k).cloned())?;
                Some((
                    Source::Local {
                        fingerprint: i.fingerprint.clone(),
                    },
                    image_name(&i),
                    i.kind,
                ))
            }
            Kind::Remotes if self.focus == Pane::Detail && !self.drafting => {
                match detail.get(self.at).map(|row| &row.target) {
                    Some(Target::Offered(alias, machine)) => {
                        self.offered_source(r, alias, *machine)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// A catalogue's image as the draft's source: the remote's address and
    /// protocol, the alias, and the kind it is.
    fn offered_source(
        &self,
        r: &Readings,
        alias: &str,
        machine: Machine,
    ) -> Option<(Source, String, Machine)> {
        let remote = self.picked(r).and_then(|k| r.remote(&k).cloned())?;
        Some((
            Source::Remote {
                server: remote.address.clone(),
                protocol: remote.protocol,
                alias: alias.to_owned(),
            },
            format!("{}:{alias}", remote.name),
            machine,
        ))
    }

    /// `C-k` on a catalogue's image: a copy kept here, on the queue.
    fn copy_offered(
        &self,
        r: &Readings,
        alias: &str,
        machine: Machine,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(remote) = self.picked(r).and_then(|k| r.remote(&k).cloned()) else {
            return self.cannot(texts);
        };
        let job = Job::ImageCopy {
            server: remote.address.clone(),
            protocol: remote.protocol,
            alias: alias.to_owned(),
            kind: Some(machine),
        };
        if !r.lets(&job.needs_hosts()) {
            return Errand::Say(words::refused(Refusal::of(Why::NotLet), texts));
        }
        Errand::Queue(job, format!("{}:{alias}", remote.name))
    }

    /// `RET` on a row of the draft: the kind turns, the rest is typed.
    fn ask_field(&mut self, field: Field) -> Errand {
        let Some(d) = self.draft.as_mut() else {
            return Errand::Nothing;
        };
        let (id, initial) = match field {
            Field::Machine => {
                d.machine = match d.machine {
                    Machine::Container => Machine::Vm,
                    Machine::Vm => Machine::Container,
                };
                // A VM'S KERNEL COUNTS ITS OWN PROCESSES: the limit goes.
                if d.machine == Machine::Vm {
                    d.limits.processes = None;
                }
                return Errand::Nothing;
            }
            Field::Name => (NAME, Some(d.name.clone()).filter(|n| !n.is_empty())),
            Field::Image => (IMAGE, Some(d.image.clone()).filter(|n| !n.is_empty())),
            Field::Limit(limit) => (limit.question(), limit.typed(d.limits)),
        };
        self.asking = Some(Asking::Draft(field));
        Errand::Ask(id, initial)
    }

    /// The draft's last row: checked, then launched on the queue.
    fn create(&mut self, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(d) = self.draft.clone() else {
            return Errand::Nothing;
        };
        let (Some(source), false) = (d.source.clone(), d.name.is_empty()) else {
            return Errand::Say(texts.t(crate::key::INCUS_REFUSED_UNCHOSEN));
        };
        if let Some(refused) = name_refused(&d.name, r, texts) {
            return Errand::Say(refused);
        }
        if !r.lets(&d.hosts()) {
            return Errand::Say(words::refused(Refusal::of(Why::NotLet), texts));
        }
        let mut limits = d.limits;
        if d.machine == Machine::Vm {
            limits.processes = None;
        }
        let job = Job::Launch {
            create: Create {
                name: d.name.clone(),
                kind: d.machine,
                source,
                profiles: Vec::new(),
                limits,
            },
        };
        // LAUNCHED: the draft is done with, and the view goes back.
        self.undraft();
        self.draft = None;
        Errand::Queue(job, d.name)
    }

    fn enter_detail(&mut self, r: &Readings, layout: Layout) {
        if self.picked(r).is_some() {
            self.open = true;
            self.focus = Pane::Detail;
            if !self.detail_shown(r, layout) {
                self.focus = Pane::List;
            }
        }
    }

    fn cannot(&self, texts: &ronoctua_i18n::Texts) -> Errand {
        Errand::Say(texts.t_with(
            crate::key::INCUS_REFUSED_NOT_HERE,
            &[("kind", &texts.t(words::kind_key(self.kind)))],
        ))
    }

    /// `C-d`: an instance, an image, or the snapshot under the cursor — each
    /// after a question.
    fn delete(&mut self, r: &Readings, detail: &[Row], texts: &ronoctua_i18n::Texts) -> Errand {
        if self.drafting {
            return Errand::Nothing;
        }
        if let (Pane::Detail, Some(Target::Snapshot(snapshot))) =
            (self.focus, detail.get(self.at).map(|row| &row.target))
            && let Some(i) = self.picked_instance(r)
        {
            let q = Question::DeleteSnapshot {
                name: i.name.clone(),
                snapshot: snapshot.clone(),
            };
            self.ask(q);
            return Errand::Nothing;
        }
        let q =
            match self.kind {
                Kind::Instances => self.picked_instance(r).map(|i| Question::Delete {
                    name: i.name.clone(),
                    running: is_up(i.status),
                    snapshots: !i.snapshots.is_empty(),
                }),
                Kind::Images => self.picked(r).and_then(|k| r.image(&k).cloned()).map(|i| {
                    Question::DeleteImage {
                        fingerprint: i.fingerprint.clone(),
                        label: image_name(&i),
                    }
                }),
                _ => return self.cannot(texts),
            };
        if let Some(q) = q {
            self.ask(q);
        }
        Errand::Nothing
    }

    /// A key that acts on the instance chosen.
    fn on_instance(&mut self, press: Press, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        if self.drafting {
            return Errand::Nothing;
        }
        let Some(i) = self.picked_instance(r) else {
            return self.cannot(texts);
        };
        let name = i.name.clone();
        let running = i.status == Status::Running;
        let not_running = || Errand::Say(texts.t(crate::key::INCUS_REFUSED_NOT_RUNNING));
        let act = |action| {
            Errand::Do(
                Deed::Act {
                    name: name.clone(),
                    action,
                },
                name.clone(),
            )
        };
        match press {
            Press::StartStop => act(if is_up(i.status) {
                Action::Stop
            } else {
                Action::Start
            }),
            Press::Restart if running => act(Action::Restart),
            Press::Freeze if running => act(Action::Freeze),
            Press::Freeze if i.status == Status::Frozen => act(Action::Unfreeze),
            Press::Restart | Press::Freeze => not_running(),
            Press::Console | Press::Shell if running => Errand::Open(
                if press == Press::Console {
                    Use::Console
                } else {
                    Use::Shell
                },
                name,
            ),
            Press::Screen if i.kind != Machine::Vm => {
                Errand::Say(texts.t(crate::key::INCUS_REFUSED_NOT_VM))
            }
            Press::Screen if running => Errand::Open(Use::Screen, name),
            Press::Console | Press::Shell | Press::Screen => not_running(),
            Press::Push => {
                self.asking = Some(Asking::PushFrom { name });
                Errand::Ask(PUSH, None)
            }
            Press::Pull => {
                self.asking = Some(Asking::PullFrom { name });
                Errand::Ask(PULL, None)
            }
            Press::Export => {
                let initial = format!("~/{name}.tar.gz");
                self.asking = Some(Asking::Export { name });
                Errand::Ask(EXPORT, Some(initial))
            }
            _ => Errand::Nothing,
        }
    }

    /// An action row of the Actions tab, as its key — the publication has
    /// none, and asks here.
    fn use_row(&mut self, what: Use, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        if what == Use::Publish {
            return match self.picked_instance(r) {
                Some(i) => {
                    self.asking = Some(Asking::Publish {
                        name: i.name.clone(),
                    });
                    Errand::Ask(PUBLISH, Some(i.name.clone()))
                }
                None => self.cannot(texts),
            };
        }
        self.on_instance(press_of_use(what), r, texts)
    }

    // -- answers --------------------------------------------------------------

    /// What was typed on a typed question, as an errand — one call for both
    /// doors: the footer's region through `Region::answered`, the window
    /// through `Content::answered`. `home` expands a leading `~`.
    /// An answer to a question this view did not ask (or let go) is nothing.
    pub fn answered(
        &mut self,
        id: &str,
        said: &str,
        r: &Readings,
        home: Option<&Path>,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(asking) = self.asking.take().filter(|a| a.question() == id) else {
            return Errand::Nothing;
        };
        let said = said.trim();
        let path = |s: &str| expanded(s, home);
        match asking {
            Asking::Draft(field) => self.answer_field(field, said, r, texts),
            Asking::Limit {
                name,
                limit,
                limits,
                own,
            } => match limit_of(limit, said, texts) {
                // NOTHING TYPED takes the instance's OWN limit away — and
                // only it: a profile's is changed in the profile, and saying
                // "done" over a row that still reads the same would lie.
                Ok(None) if !limit.is_set(own) => {
                    if limit.is_set(limits) {
                        Errand::Say(texts.t_with(
                            crate::key::INCUS_REFUSED_LIMIT_PROFILE,
                            &[
                                ("limit", &texts.t(words::limit_key(limit))),
                                ("name", &name),
                            ],
                        ))
                    } else {
                        Errand::Nothing
                    }
                }
                // ONE LIMIT, and nothing else, goes to the daemon: the
                // others stay as they are, a profile's never copied in.
                Ok(value) => match limit.change(value) {
                    Some(change) => Errand::Do(
                        Deed::Limit {
                            name: name.clone(),
                            change,
                        },
                        name,
                    ),
                    None => Errand::Say(
                        texts.t_with(crate::key::INCUS_REFUSED_NUMBER, &[("said", said)]),
                    ),
                },
                Err(refused) => Errand::Say(refused),
            },
            Asking::Snapshot { name } => {
                if said.is_empty() {
                    Errand::Nothing
                } else if ronoctua_incus::is_snapshot_name(said) {
                    Errand::Do(
                        Deed::Snapshot {
                            name: name.clone(),
                            snapshot: said.to_owned(),
                        },
                        format!("{name}/{said}"),
                    )
                } else {
                    Errand::Say(
                        texts.t_with(crate::key::INCUS_REFUSED_SNAPSHOT_NAME, &[("name", said)]),
                    )
                }
            }
            Asking::PushFrom { name } => {
                if said.is_empty() {
                    return Errand::Nothing;
                }
                let from = path(said);
                if !from.is_file() {
                    return Errand::Say(
                        texts.t_with(crate::key::INCUS_REFUSED_NO_FILE, &[("path", said)]),
                    );
                }
                let base = from
                    .file_name()
                    .map(|b| b.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.asking = Some(Asking::PushTo { name, from });
                Errand::Ask(PUSH_TO, Some(format!("/root/{base}")))
            }
            Asking::PushTo { name, from } => match inside(said) {
                Some(to) => Errand::Queue(
                    Job::Push {
                        name: name.clone(),
                        from,
                        to,
                    },
                    name,
                ),
                None if said.is_empty() => Errand::Nothing,
                None => {
                    Errand::Say(texts.t_with(crate::key::INCUS_REFUSED_INSIDE, &[("path", said)]))
                }
            },
            Asking::PullFrom { name } => match inside(said) {
                Some(from) => {
                    let base = from.rsplit('/').next().unwrap_or_default().to_owned();
                    self.asking = Some(Asking::PullTo { name, from });
                    Errand::Ask(PULL_TO, Some(format!("~/{base}")))
                }
                None if said.is_empty() => Errand::Nothing,
                None => {
                    Errand::Say(texts.t_with(crate::key::INCUS_REFUSED_INSIDE, &[("path", said)]))
                }
            },
            Asking::PullTo { name, from } => match new_file(said, home, texts) {
                Ok(to) => Errand::Queue(
                    Job::Pull {
                        name: name.clone(),
                        from,
                        to,
                    },
                    name,
                ),
                Err(said) => refusing(said),
            },
            Asking::Export { name } => match new_file(said, home, texts) {
                Ok(to) => Errand::Queue(
                    Job::Export {
                        name: name.clone(),
                        to,
                    },
                    name,
                ),
                Err(said) => refusing(said),
            },
            Asking::Import => {
                if said.is_empty() {
                    return Errand::Nothing;
                }
                let from = path(said);
                if !from.is_file() {
                    return Errand::Say(
                        texts.t_with(crate::key::INCUS_REFUSED_NO_FILE, &[("path", said)]),
                    );
                }
                let label = from
                    .file_name()
                    .map(|b| b.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Errand::Queue(Job::Import { from }, label)
            }
            Asking::Publish { name } => {
                if said.is_empty() {
                    Errand::Nothing
                } else if said.chars().any(|c| c.is_whitespace() || c.is_control())
                    || said.len() > 255
                {
                    Errand::Say(texts.t_with(crate::key::INCUS_REFUSED_ALIAS, &[("alias", said)]))
                } else {
                    Errand::Queue(
                        Job::Publish {
                            name: name.clone(),
                            alias: said.to_owned(),
                        },
                        name,
                    )
                }
            }
        }
    }

    /// A field of the draft, typed.
    fn answer_field(
        &mut self,
        field: Field,
        said: &str,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(d) = self.draft.as_mut() else {
            return Errand::Nothing;
        };
        match field {
            Field::Machine => {}
            Field::Name => {
                if said.is_empty() {
                    return Errand::Nothing;
                }
                if let Some(refused) = name_refused(said, r, texts) {
                    return Errand::Say(refused);
                }
                d.name = said.to_owned();
            }
            Field::Image => {
                if said.is_empty() {
                    return Errand::Nothing;
                }
                match image_of(said, r) {
                    Some((source, machine)) => {
                        d.source = Some(source);
                        d.image = said.to_owned();
                        if let Some(m) = machine {
                            d.machine = m;
                        }
                        if d.machine == Machine::Vm {
                            d.limits.processes = None;
                        }
                    }
                    None => {
                        return Errand::Say(
                            texts.t_with(crate::key::INCUS_REFUSED_NO_IMAGE, &[("image", said)]),
                        );
                    }
                }
            }
            Field::Limit(limit) => match limit_of(limit, said, texts) {
                Ok(value) => set_limit(&mut d.limits, limit, value),
                Err(refused) => return Errand::Say(refused),
            },
        }
        Errand::Nothing
    }
}

/// The press an action row is — the same path its key takes.
const fn press_of_use(what: Use) -> Press {
    match what {
        Use::Console => Press::Console,
        Use::Screen => Press::Screen,
        Use::Shell => Press::Shell,
        Use::Push => Press::Push,
        Use::Pull => Press::Pull,
        Use::Export | Use::Publish => Press::Export,
    }
}

/// Why a new instance's name is refused, as a sentence: a name Incus does
/// not take, or one an instance already has.
fn name_refused(name: &str, r: &Readings, texts: &ronoctua_i18n::Texts) -> Option<String> {
    if !ronoctua_incus::is_instance_name(name) {
        return Some(texts.t_with(crate::key::INCUS_REFUSED_NAME, &[("name", name)]));
    }
    r.instance(name)
        .map(|_| texts.t_with(crate::key::INCUS_REFUSED_NAME_TAKEN, &[("name", name)]))
}

/// A limit typed: `None` (no limit) for an empty answer, the value, or the
/// sentence that refuses it.
fn limit_of(limit: Limit, said: &str, texts: &ronoctua_i18n::Texts) -> Result<Option<u64>, String> {
    if said.is_empty() {
        return Ok(None);
    }
    match limit {
        Limit::Memory => words::bytes_of(said)
            .map(Some)
            .map_err(|_| texts.t_with(crate::key::INCUS_REFUSED_SIZE, &[("said", said)])),
        Limit::Cpu | Limit::Processes => words::whole_of(said)
            .filter(|n| limit != Limit::Cpu || *n <= u64::from(u32::MAX))
            .map(Some)
            .ok_or_else(|| texts.t_with(crate::key::INCUS_REFUSED_NUMBER, &[("said", said)])),
    }
}

/// One limit set in all three.
fn set_limit(limits: &mut Limits, limit: Limit, value: Option<u64>) {
    match limit {
        Limit::Cpu => limits.cpu = value.and_then(|n| u32::try_from(n).ok()),
        Limit::Memory => limits.memory_bytes = value,
        Limit::Processes => limits.processes = value,
    }
}

/// The image a person typed: `<remote>:<alias>` for a remote's (its address
/// and protocol from the client's remotes), or a local image by an alias or
/// the start of its fingerprint (and then the kind it is). Pure.
#[must_use]
pub fn image_of(said: &str, r: &Readings) -> Option<(Source, Option<Machine>)> {
    let said = said.trim();
    if let Some((remote, alias)) = said.split_once(':')
        && let Some(found) = r.remotes_read().iter().find(|x| x.name == remote)
        && !alias.is_empty()
    {
        return Some((
            Source::Remote {
                server: found.address.clone(),
                protocol: found.protocol,
                alias: alias.to_owned(),
            },
            None,
        ));
    }
    let images = r.listed().map(|l| l.images.as_slice()).unwrap_or_default();
    let by_alias = images.iter().find(|i| i.aliases.iter().any(|a| a == said));
    // A FINGERPRINT'S START, at least the daemon's short form's length, and
    // only when it names one image.
    let by_fingerprint = || {
        let lower = said.to_ascii_lowercase();
        let mut hits = images
            .iter()
            .filter(|i| lower.len() >= 12 && i.fingerprint.starts_with(&lower));
        match (hits.next(), hits.next()) {
            (Some(one), None) => Some(one),
            _ => None,
        }
    };
    by_alias.or_else(by_fingerprint).map(|i| {
        (
            Source::Local {
                fingerprint: i.fingerprint.clone(),
            },
            Some(i.kind),
        )
    })
}

/// A path typed here: a leading `~` is the home.
fn expanded(said: &str, home: Option<&Path>) -> PathBuf {
    match (said.strip_prefix('~'), home) {
        (Some(rest), Some(home)) => home.join(rest.trim_start_matches('/')),
        _ => PathBuf::from(said),
    }
}

/// A path inside an instance, as the daemon takes it: absolute, one line.
fn inside(said: &str) -> Option<String> {
    (said.starts_with('/') && !said.chars().any(char::is_control)).then(|| said.to_owned())
}

/// A file to be made here — never over one, in a folder that is there. The
/// refusal is what the line says instead: nothing for nothing typed, or the
/// sentence that says why not.
fn new_file(
    said: &str,
    home: Option<&Path>,
    texts: &ronoctua_i18n::Texts,
) -> Result<PathBuf, Option<String>> {
    if said.is_empty() {
        return Err(None);
    }
    let to = expanded(said, home);
    if to.exists() || to.is_symlink() {
        return Err(Some(
            texts.t_with(crate::key::INCUS_REFUSED_EXISTS, &[("path", said)]),
        ));
    }
    if !to.parent().is_some_and(Path::is_dir) {
        return Err(Some(
            texts.t_with(crate::key::INCUS_REFUSED_NO_FOLDER, &[("path", said)]),
        ));
    }
    Ok(to)
}

/// What the line says for a refusal of [`new_file`].
fn refusing(said: Option<String>) -> Errand {
    said.map_or(Errand::Nothing, Errand::Say)
}

/// An offset that keeps `cursor` inside `room` lines of `total`.
const fn follow(offset: usize, cursor: usize, room: usize, total: usize) -> usize {
    let most = total.saturating_sub(room);
    let offset = if cursor < offset {
        cursor
    } else if room > 0 && cursor >= offset + room {
        cursor + 1 - room
    } else {
        offset
    };
    if offset > most { most } else { offset }
}

/// The detail's width at this door: its column in the panes, the whole row
/// stacked.
fn detail_width(columns: u16, layout: Layout) -> u16 {
    match layout {
        Layout::Panes => places(columns).detail.1.saturating_sub(BAR_CELLS),
        Layout::Stacked => extent(columns).1,
    }
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "incus",
    commands: &[
        ronoctua_cmd::Command {
            id: "incus-open",
            label: crate::key::CMD_INCUS_OPEN,
        },
        ronoctua_cmd::Command {
            id: "incus-window",
            label: crate::key::CMD_INCUS_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "incus-new",
            label: crate::key::CMD_INCUS_NEW,
        },
        ronoctua_cmd::Command {
            id: "incus-images",
            label: crate::key::CMD_INCUS_IMAGES,
        },
        // THE VIEW'S OWN KEYS (`incus::OF_THE_VIEW`) AND THE QUESTIONS ITS
        // ROWS AND KEYS ASK (`incus::QUESTIONS`).
        ronoctua_cmd::Command {
            id: "incus-down",
            label: crate::key::CMD_INCUS_DOWN,
        },
        ronoctua_cmd::Command {
            id: "incus-up",
            label: crate::key::CMD_INCUS_UP,
        },
        ronoctua_cmd::Command {
            id: "incus-enter",
            label: crate::key::CMD_INCUS_ENTER,
        },
        ronoctua_cmd::Command {
            id: "incus-left",
            label: crate::key::CMD_INCUS_LEFT,
        },
        ronoctua_cmd::Command {
            id: "incus-right",
            label: crate::key::CMD_INCUS_RIGHT,
        },
        ronoctua_cmd::Command {
            id: "incus-next-kind",
            label: crate::key::CMD_INCUS_NEXT_KIND,
        },
        ronoctua_cmd::Command {
            id: "incus-previous-kind",
            label: crate::key::CMD_INCUS_PREVIOUS_KIND,
        },
        ronoctua_cmd::Command {
            id: "incus-next-tab",
            label: crate::key::CMD_INCUS_NEXT_TAB,
        },
        ronoctua_cmd::Command {
            id: "incus-previous-tab",
            label: crate::key::CMD_INCUS_PREVIOUS_TAB,
        },
        ronoctua_cmd::Command {
            id: "incus-launch",
            label: crate::key::CMD_INCUS_LAUNCH,
        },
        ronoctua_cmd::Command {
            id: "incus-start-stop",
            label: crate::key::CMD_INCUS_START_STOP,
        },
        ronoctua_cmd::Command {
            id: "incus-restart",
            label: crate::key::CMD_INCUS_RESTART,
        },
        ronoctua_cmd::Command {
            id: "incus-freeze",
            label: crate::key::CMD_INCUS_FREEZE,
        },
        ronoctua_cmd::Command {
            id: "incus-delete",
            label: crate::key::CMD_INCUS_DELETE,
        },
        ronoctua_cmd::Command {
            id: "incus-console",
            label: crate::key::CMD_INCUS_CONSOLE,
        },
        ronoctua_cmd::Command {
            id: "incus-screen",
            label: crate::key::CMD_INCUS_SCREEN,
        },
        ronoctua_cmd::Command {
            id: "incus-shell",
            label: crate::key::CMD_INCUS_SHELL,
        },
        ronoctua_cmd::Command {
            id: "incus-push",
            label: crate::key::CMD_INCUS_PUSH,
        },
        ronoctua_cmd::Command {
            id: "incus-pull",
            label: crate::key::CMD_INCUS_PULL,
        },
        ronoctua_cmd::Command {
            id: "incus-export",
            label: crate::key::CMD_INCUS_EXPORT,
        },
        ronoctua_cmd::Command {
            id: "incus-import",
            label: crate::key::CMD_INCUS_IMPORT,
        },
        ronoctua_cmd::Command {
            id: "incus-copy-image",
            label: crate::key::CMD_INCUS_COPY_IMAGE,
        },
        ronoctua_cmd::Command {
            id: "incus-name",
            label: crate::key::CMD_INCUS_NAME,
        },
        ronoctua_cmd::Command {
            id: "incus-image",
            label: crate::key::CMD_INCUS_IMAGE,
        },
        ronoctua_cmd::Command {
            id: "incus-cpu",
            label: crate::key::CMD_INCUS_CPU,
        },
        ronoctua_cmd::Command {
            id: "incus-memory",
            label: crate::key::CMD_INCUS_MEMORY,
        },
        ronoctua_cmd::Command {
            id: "incus-processes",
            label: crate::key::CMD_INCUS_PROCESSES,
        },
        ronoctua_cmd::Command {
            id: "incus-snapshot",
            label: crate::key::CMD_INCUS_SNAPSHOT,
        },
        ronoctua_cmd::Command {
            id: "incus-push-to",
            label: crate::key::CMD_INCUS_PUSH_TO,
        },
        ronoctua_cmd::Command {
            id: "incus-pull-to",
            label: crate::key::CMD_INCUS_PULL_TO,
        },
        ronoctua_cmd::Command {
            id: "incus-publish",
            label: crate::key::CMD_INCUS_PUBLISH,
        },
    ],
    bindings: &[
        ("s x x", "incus-open", None),
        ("s x w", "incus-window", None),
        ("s x n", "incus-new", None),
        ("s x i", "incus-images", None),
    ],
    groups: &[("s x", crate::key::GROUP_INCUS)],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::incus::reached().collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ronoctua_incus::{Protocol, State};

    const WIDE: u16 = 140;
    const NARROW: u16 = 80;
    const ROOM: usize = 24;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn texts_in(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).unwrap()
    }

    pub(crate) fn instance(name: &str, kind: Machine, status: Status) -> Instance {
        Instance {
            name: name.to_owned(),
            kind,
            status,
            project: "default".to_owned(),
            description: String::new(),
            profiles: vec!["default".to_owned()],
            created_at: "2026-10-04T05:24:11Z".to_owned(),
            image: "Debian trixie amd64 (20261004_05:24)".to_owned(),
            limits: Limits {
                cpu: Some(2),
                memory_bytes: Some(2 << 30),
                processes: None,
            },
            // THE MEMORY IS A PROFILE'S: the instance sets only its
            // processors.
            own_limits: Limits {
                cpu: Some(2),
                memory_bytes: None,
                processes: None,
            },
            state: (status == Status::Running).then(|| State {
                ipv4: vec!["10.218.42.5".to_owned(), "10.218.42.6".to_owned()],
                ipv6: Vec::new(),
                cpu_ns: 12_400_000_000,
                memory_bytes: 300 << 20,
                processes: 12,
                disk_bytes: 0,
            }),
            snapshots: vec![Snapshot {
                name: "snap0".to_owned(),
                created_at: "2026-10-04T06:00:00Z".to_owned(),
            }],
        }
    }

    /// The readings a photograph's scene gives, as the view's tests read them.
    pub(crate) fn readings() -> Readings {
        let listing = Listing {
            instances: vec![
                instance("web", Machine::Container, Status::Running),
                instance("desk", Machine::Vm, Status::Stopped),
            ],
            images: vec![Image {
                fingerprint: "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90"
                    .to_owned(),
                aliases: vec!["debian/13".to_owned()],
                description: "Debian trixie amd64".to_owned(),
                kind: Machine::Container,
                size_bytes: 120 << 20,
            }],
            profiles: vec![Profile {
                name: "default".to_owned(),
                description: "Default Incus profile".to_owned(),
            }],
            networks: vec![Network {
                name: "incusbr0".to_owned(),
                managed: true,
                kind: "bridge".to_owned(),
            }],
            pools: vec![Pool {
                name: "default".to_owned(),
                driver: "btrfs".to_owned(),
            }],
            projects: vec![Project {
                name: "default".to_owned(),
                description: "Default Incus project".to_owned(),
            }],
        };
        Readings {
            listing: Some(Ok(listing)),
            remotes: Some(Ok(vec![
                Remote {
                    name: "images".to_owned(),
                    address: "https://images.linuxcontainers.org".to_owned(),
                    protocol: Protocol::SimpleStreams,
                    public: true,
                },
                Remote {
                    name: "docker".to_owned(),
                    address: "https://docker.io".to_owned(),
                    protocol: Protocol::Oci,
                    public: true,
                },
            ])),
            ..Readings::default()
        }
    }

    /// A text with its wrapping undone, as a sentence reads.
    fn flat(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn texts_of(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|l| {
                l.fields
                    .iter()
                    .map(|f| f.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_first_row_is_chosen_and_the_arrows_walk_by_identity() {
        let (t, mut r) = (texts(), readings());
        let mut v = View::fresh();
        assert_eq!(v.picked(&r).as_deref(), Some("web"));
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.picked(&r).as_deref(), Some("desk"));
        // A LISTING THAT REORDERS keeps the row, not the index.
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.instances.reverse();
        }
        assert_eq!(v.picked(&r).as_deref(), Some("desk"));
        // AND ONE WHERE IT IS GONE falls back to the first.
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.instances.retain(|i| i.name != "desk");
        }
        v.settle(&r, WIDE, Layout::Panes, &t);
        assert_eq!(v.picked(&r).as_deref(), Some("web"));
    }

    #[test]
    fn the_kinds_turn_with_tab_and_count_what_there_is() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        v.key(Press::PreviousKind, &r, WIDE, Layout::Stacked, &t);
        assert_eq!(v.kind(), Kind::Projects, "round");
        let lines = v.lines(&r, WIDE, Layout::Stacked, ROOM, &t);
        let strip = texts_of(&lines[..1]);
        assert!(strip.contains("Instances 2"), "{strip}");
        assert!(strip.contains("Remotes 2"), "{strip}");
        let said = texts_of(&lines);
        assert!(
            said.contains(&t.t(crate::key::INCUS_PROJECTS_NOTE)),
            "{said}"
        );
    }

    /// FAILS IF: a catalogue is asked for while its remote's hosts are not
    /// all let — Docker Hub's client asks two more than its address — or the
    /// consent row is not drawn (and taught) in its place.
    #[test]
    fn a_catalogue_is_wanted_only_once_every_host_is_let() {
        let (t, mut r) = (texts(), readings());
        let mut v = View::on(Kind::Remotes);
        v.key(Press::Right, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.wants(&r, Layout::Panes), Wants::default());
        assert!(v.asks_consent(&r, Layout::Panes));
        let said = texts_of(&v.lines(&r, WIDE, Layout::Panes, ROOM, &t));
        assert!(said.contains("images.linuxcontainers.org"), "{said}");
        let hint: String = v
            .hint(&r, WIDE, Layout::Panes, &t)
            .iter()
            .map(|p| p.text.clone())
            .collect();
        assert!(hint.contains("C-a"), "{hint}");
        // `C-a` LETS THE HOSTS THE ROW NAMES.
        assert_eq!(
            v.key(Press::Allow, &r, WIDE, Layout::Panes, &t),
            Errand::Allow(vec!["images.linuxcontainers.org".to_owned()])
        );
        r.consented.insert("images.linuxcontainers.org".to_owned());
        assert_eq!(
            v.wants(&r, Layout::Panes).catalogue.as_deref(),
            Some("images")
        );
        // DOCKER HUB: its address alone is not enough.
        v.key(Press::Left, &r, WIDE, Layout::Panes, &t);
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        v.key(Press::Right, &r, WIDE, Layout::Panes, &t);
        r.consented.insert("docker.io".to_owned());
        assert_eq!(v.wants(&r, Layout::Panes), Wants::default());
        assert_eq!(
            v.key(Press::Allow, &r, WIDE, Layout::Panes, &t),
            Errand::Allow(vec![
                "registry-1.docker.io".to_owned(),
                "auth.docker.io".to_owned()
            ])
        );
    }

    #[test]
    fn a_catalogue_s_image_opens_the_draft_or_is_copied_here() {
        let (t, mut r) = (texts(), readings());
        r.consented.insert("images.linuxcontainers.org".to_owned());
        r.catalogue = Some(Catalogue {
            remote: "images".to_owned(),
            answer: Ok(vec![
                Offered {
                    alias: "alpine/3.22".to_owned(),
                    description: "Alpine".to_owned(),
                    kind: Machine::Container,
                    fingerprint: "f00d".to_owned(),
                    size_bytes: 3 << 20,
                },
                Offered {
                    alias: "debian/13".to_owned(),
                    description: "Debian".to_owned(),
                    kind: Machine::Vm,
                    fingerprint: "beef".to_owned(),
                    size_bytes: 400 << 20,
                },
            ]),
        });
        let mut v = View::on(Kind::Remotes);
        v.key(Press::Right, &r, WIDE, Layout::Panes, &t);
        v.settle(&r, WIDE, Layout::Panes, &t);
        // DOWN TO THE SECOND IMAGE.
        while !matches!(
            v.detail_target(&r, WIDE, Layout::Panes, &t),
            Some(Target::Offered(ref a, _)) if a == "debian/13"
        ) {
            v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        }
        assert_eq!(
            v.key(Press::CopyImage, &r, WIDE, Layout::Panes, &t),
            Errand::Queue(
                Job::ImageCopy {
                    server: "https://images.linuxcontainers.org".to_owned(),
                    protocol: Protocol::SimpleStreams,
                    alias: "debian/13".to_owned(),
                    kind: Some(Machine::Vm),
                },
                "images:debian/13".to_owned()
            )
        );
        v.key(Press::Enter, &r, WIDE, Layout::Panes, &t);
        let d = v.draft().unwrap();
        assert!(v.drafting());
        assert_eq!(d.machine, Machine::Vm, "the kind the catalogue said");
        assert_eq!(d.image, "images:debian/13");
    }

    /// FAILS IF: a delete is not asked first, the cursor is not born on "no",
    /// or another key is taken for a yes.
    #[test]
    fn deleting_asks_first_and_the_cursor_is_born_on_no() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        assert_eq!(
            v.key(Press::Delete, &r, WIDE, Layout::Panes, &t),
            Errand::Nothing
        );
        let said = texts_of(&v.lines(&r, WIDE, Layout::Panes, ROOM, &t));
        assert!(said.contains("web"), "{said}");
        assert!(
            said.contains(&t.t(crate::key::INCUS_ASK_DELETE_RUNNING)),
            "{said}"
        );
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Nothing
        );
        assert!(v.question().is_none());
        v.key(Press::Delete, &r, WIDE, Layout::Panes, &t);
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Do(
                Deed::Act {
                    name: "web".to_owned(),
                    action: Action::Delete
                },
                "web".to_owned()
            )
        );
        v.key(Press::Delete, &r, WIDE, Layout::Panes, &t);
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.key(Press::NextKind, &r, WIDE, Layout::Panes, &t),
            Errand::Nothing
        );
        assert!(v.question().is_none(), "any other key is no");
    }

    #[test]
    fn a_snapshot_is_taken_restored_after_a_question_and_deleted_after_one() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        v.key(Press::Right, &r, WIDE, Layout::Panes, &t);
        v.key(Press::NextTab, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.tab_shown(), Tab::Snapshots);
        // THE FIRST ROW TAKES ONE, its line holding the next free name.
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Ask(SNAPSHOT, Some("snap1".to_owned()))
        );
        assert_eq!(
            v.answered(SNAPSHOT, "before-upgrade", &r, None, &t),
            Errand::Do(
                Deed::Snapshot {
                    name: "web".to_owned(),
                    snapshot: "before-upgrade".to_owned()
                },
                "web/before-upgrade".to_owned()
            )
        );
        assert_eq!(
            v.answered(SNAPSHOT, "again", &r, None, &t),
            Errand::Nothing,
            "asked once"
        );
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Nothing
        );
        assert!(matches!(v.question(), Some(Question::Restore { .. })));
        v.key(Press::Escape, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.key(Press::Delete, &r, WIDE, Layout::Panes, &t),
            Errand::Nothing
        );
        assert!(matches!(
            v.question(),
            Some(Question::DeleteSnapshot { .. })
        ));
        v.key(Press::Right, &r, WIDE, Layout::Panes, &t);
        assert!(matches!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Do(Deed::DeleteSnapshot { .. }, _)
        ));
    }

    /// FAILS IF: a limit's line does not hold what is set, a change sends
    /// more than the one limit asked (a profile's limit copied into the
    /// instance would stop following the profile), nothing typed does not
    /// take the instance's own away — or says "done" over a profile's that
    /// stays — a size with no unit is taken, or a VM is given a limit of
    /// processes.
    #[test]
    fn a_limit_is_typed_alone_and_an_empty_answer_takes_only_the_instance_s_own() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        v.key(Press::Right, &r, WIDE, Layout::Panes, &t);
        v.key(Press::NextTab, &r, WIDE, Layout::Panes, &t);
        v.key(Press::NextTab, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.tab_shown(), Tab::Limits);
        // WHERE A LIMIT COMES FROM is drawn: the memory is a profile's.
        let said = texts_of(&v.lines(&r, WIDE, Layout::Panes, ROOM, &t));
        let profile_s = t.t_with(
            crate::key::INCUS_LIMIT_FROM_PROFILE,
            &[(
                "value",
                &limit_text(Limit::Memory, r.instance("web").unwrap().limits, &t),
            )],
        );
        assert!(said.contains(&profile_s), "{said}");
        assert!(
            !said.contains(&t.t_with(
                crate::key::INCUS_LIMIT_FROM_PROFILE,
                &[(
                    "value",
                    &limit_text(Limit::Cpu, r.instance("web").unwrap().limits, &t)
                )],
            )),
            "the processors are its own: {said}"
        );
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Ask(MEMORY, Some("2GiB".to_owned()))
        );
        assert!(matches!(
            v.answered(MEMORY, "2", &r, None, &t),
            Errand::Say(_)
        ));
        // NOTHING TYPED on a profile's limit: nothing of its own to take
        // away, and the screen says why rather than "done".
        v.key(Press::Enter, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.answered(MEMORY, "", &r, None, &t),
            Errand::Say(t.t_with(
                crate::key::INCUS_REFUSED_LIMIT_PROFILE,
                &[
                    ("limit", &t.t(words::limit_key(Limit::Memory))),
                    ("name", "web")
                ],
            ))
        );
        // A VALUE: that limit, and nothing else, goes to the daemon.
        v.key(Press::Enter, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.answered(MEMORY, "4GiB", &r, None, &t),
            Errand::Do(
                Deed::Limit {
                    name: "web".to_owned(),
                    change: LimitChange::MemoryBytes(Some(4 << 30)),
                },
                "web".to_owned()
            )
        );
        // NOTHING TYPED on its own limit takes that one away.
        v.key(Press::Up, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Ask(CPU, Some("2".to_owned()))
        );
        assert_eq!(
            v.answered(CPU, "", &r, None, &t),
            Errand::Do(
                Deed::Limit {
                    name: "web".to_owned(),
                    change: LimitChange::Cpu(None),
                },
                "web".to_owned()
            )
        );
        // A VM HAS NO ROW FOR PROCESSES, and says why.
        v.key(Press::Left, &r, WIDE, Layout::Panes, &t);
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        let said = flat(&texts_of(&v.lines(&r, WIDE, Layout::Panes, ROOM, &t)));
        assert!(
            said.contains(&flat(&t.t(crate::key::INCUS_LIMITS_VM_PROCESSES))),
            "{said}"
        );
    }

    /// FAILS IF: the draft does not open on its name, a name Incus refuses
    /// or one taken is accepted, a remote's image is launched before its
    /// hosts are let, the draft is lost when it is put away, or the launch
    /// does not reach the queue whole.
    #[test]
    fn a_new_instance_is_drafted_kept_and_launched_on_the_queue() {
        let (t, mut r) = (texts(), readings());
        let mut v = View::fresh();
        assert_eq!(
            v.key(Press::Launch, &r, WIDE, Layout::Panes, &t),
            Errand::Nothing
        );
        assert!(v.drafting());
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Ask(NAME, None),
            "the cursor is born on the name"
        );
        assert!(matches!(
            v.answered(NAME, "1web", &r, None, &t),
            Errand::Say(_)
        ));
        v.key(Press::Enter, &r, WIDE, Layout::Panes, &t);
        assert!(
            matches!(v.answered(NAME, "web", &r, None, &t), Errand::Say(_)),
            "taken"
        );
        v.key(Press::Enter, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.answered(NAME, "box", &r, None, &t), Errand::Nothing);
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Ask(IMAGE, None)
        );
        assert!(matches!(
            v.answered(IMAGE, "nowhere:x", &r, None, &t),
            Errand::Say(_)
        ));
        v.key(Press::Enter, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.answered(IMAGE, "images:alpine/3.22", &r, None, &t),
            Errand::Nothing
        );
        // PUT AWAY AND OPENED AGAIN: kept.
        v.key(Press::Escape, &r, WIDE, Layout::Panes, &t);
        assert!(!v.drafting());
        v.key(Press::Launch, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.draft().unwrap().name, "box");
        // THE CONSENT ROW, before the row that launches; refused until let.
        let said = texts_of(&v.lines(&r, WIDE, Layout::Panes, ROOM, &t));
        assert!(said.contains("images.linuxcontainers.org"), "{said}");
        let rows = v.detail_rows(&r, detail_width(WIDE, Layout::Panes), &t);
        let create = rows
            .iter()
            .position(|row| row.target == Target::Create)
            .unwrap();
        v.at = create;
        assert!(matches!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Say(_)
        ));
        r.consented.insert("images.linuxcontainers.org".to_owned());
        let rows = v.detail_rows(&r, detail_width(WIDE, Layout::Panes), &t);
        v.at = rows
            .iter()
            .position(|row| row.target == Target::Create)
            .unwrap();
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Queue(
                Job::Launch {
                    create: Create {
                        name: "box".to_owned(),
                        kind: Machine::Container,
                        source: Source::Remote {
                            server: "https://images.linuxcontainers.org".to_owned(),
                            protocol: Protocol::SimpleStreams,
                            alias: "alpine/3.22".to_owned(),
                        },
                        profiles: Vec::new(),
                        limits: Limits::default(),
                    }
                },
                "box".to_owned()
            )
        );
        assert!(
            v.draft().is_none() && !v.drafting(),
            "launched, the draft is done with"
        );
    }

    #[test]
    fn a_local_image_is_named_by_its_alias_or_its_fingerprint() {
        let r = readings();
        let fp = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
        for said in ["debian/13", "A1B2C3D4E5F6"] {
            assert_eq!(
                image_of(said, &r),
                Some((
                    Source::Local {
                        fingerprint: fp.to_owned()
                    },
                    Some(Machine::Container)
                )),
                "{said}"
            );
        }
        assert_eq!(image_of("a1b2", &r), None, "too short to name one");
        assert!(matches!(
            image_of("docker:nginx", &r),
            Some((
                Source::Remote {
                    protocol: Protocol::Oci,
                    ..
                },
                None
            ))
        ));
    }

    /// FAILS IF: a text console or a shell is opened anywhere but in OUR
    /// terminal, or the screen is put in a terminal (remote-viewer is a
    /// window of its own).
    #[test]
    fn a_console_and_a_shell_open_in_our_terminal_and_the_screen_by_itself() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        assert_eq!(
            v.key(Press::Console, &r, WIDE, Layout::Panes, &t),
            Errand::Open(Use::Console, "web".to_owned())
        );
        assert_eq!(
            v.key(Press::Shell, &r, WIDE, Layout::Panes, &t),
            Errand::Open(Use::Shell, "web".to_owned())
        );
        assert!(
            matches!(
                v.key(Press::Screen, &r, WIDE, Layout::Panes, &t),
                Errand::Say(_)
            ),
            "a container has no screen"
        );
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        assert!(
            matches!(
                v.key(Press::Console, &r, WIDE, Layout::Panes, &t),
                Errand::Say(_)
            ),
            "not running"
        );
        let line: Vec<String> = ["/nix/store/x-incus/bin/incus", "console", "web"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(
            launched(Use::Console.launch().unwrap(), "/me", &line),
            Some((
                "/me".to_owned(),
                vec![
                    "terminal".to_owned(),
                    "/nix/store/x-incus/bin/incus".to_owned(),
                    "console".to_owned(),
                    "web".to_owned()
                ]
            ))
        );
        assert_eq!(Use::Shell.launch(), Some(Launch::InTheTerminal));
        assert_eq!(
            launched(Use::Screen.launch().unwrap(), "/me", &line).map(|(p, _)| p),
            Some("/nix/store/x-incus/bin/incus".to_owned())
        );
        assert_eq!(launched(Launch::InTheTerminal, "/me", &[]), None);
    }

    #[test]
    fn a_file_is_sent_in_and_taken_out_in_two_questions_never_over_one() {
        let (t, r) = (texts(), readings());
        let dir = std::env::temp_dir().join(format!("ronoctua-incus-view-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let mut v = View::fresh();
        assert_eq!(
            v.key(Press::Push, &r, WIDE, Layout::Panes, &t),
            Errand::Ask(PUSH, None)
        );
        assert!(matches!(
            v.answered(PUSH, "~/absent", &r, Some(&dir), &t),
            Errand::Say(_)
        ));
        v.key(Press::Push, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.answered(PUSH, "~/notes.txt", &r, Some(&dir), &t),
            Errand::Ask(PUSH_TO, Some("/root/notes.txt".to_owned()))
        );
        assert!(
            matches!(v.answered(PUSH_TO, "root/x", &r, None, &t), Errand::Say(_)),
            "relative"
        );
        v.key(Press::Push, &r, WIDE, Layout::Panes, &t);
        v.answered(PUSH, "~/notes.txt", &r, Some(&dir), &t);
        assert_eq!(
            v.answered(PUSH_TO, "/root/notes.txt", &r, None, &t),
            Errand::Queue(
                Job::Push {
                    name: "web".to_owned(),
                    from: dir.join("notes.txt"),
                    to: "/root/notes.txt".to_owned()
                },
                "web".to_owned()
            )
        );
        v.key(Press::Pull, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.answered(PULL, "/etc/hosts", &r, None, &t),
            Errand::Ask(PULL_TO, Some("~/hosts".to_owned()))
        );
        // NEVER OVER A FILE that is there.
        assert!(matches!(
            v.answered(PULL_TO, "~/notes.txt", &r, Some(&dir), &t),
            Errand::Say(_)
        ));
        v.key(Press::Export, &r, WIDE, Layout::Panes, &t);
        assert!(matches!(
            v.answered(EXPORT, "~/web.tar.gz", &r, Some(&dir), &t),
            Errand::Queue(Job::Export { .. }, _)
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_panes_are_three_columns_and_a_click_takes_the_column_it_is_in() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        let lines = v.lines(&r, WIDE, Layout::Panes, ROOM, &t);
        let at = places(WIDE);
        let panes: Vec<Pane> = lines[0].zones.iter().map(|z| z.pane).collect();
        assert!(
            panes.contains(&Pane::Kinds)
                && panes.contains(&Pane::List)
                && panes.contains(&Pane::Detail)
        );
        let cell = at.list.0 + 1;
        assert!(!v.clicked(&lines, 1, Some(cell), Layout::Panes, &r));
        assert_eq!(v.picked(&r).as_deref(), Some("desk"));
        assert!(
            v.clicked(&lines, 1, Some(cell), Layout::Panes, &r),
            "the second click is RET"
        );
        let snapshots = lines[0]
            .zones
            .iter()
            .find(|z| z.target == Target::Tab(Tab::Snapshots))
            .unwrap();
        v.clicked(&lines, 0, Some(snapshots.from), Layout::Panes, &r);
        assert_eq!(v.tab_shown(), Tab::Snapshots);
        let (from, to) = View::hover_span(&lines, 1, cell).unwrap();
        assert_eq!((from, to), (at.list.0, at.list.0 + at.list.1 - BAR_CELLS));
    }

    #[test]
    fn every_tab_is_drawn_and_clicked_at_the_narrowest_three_columns() {
        let r = readings();
        for language in ["en-GB", "pt-BR"] {
            let t = texts_in(language);
            let mut v = View::fresh();
            let columns = tokens::space::CONTAINERS_PANESFROM;
            v.key(Press::Right, &r, columns, Layout::Panes, &t);
            let lines = v.lines(&r, columns, Layout::Panes, ROOM, &t);
            let tabs: Vec<Tab> = lines
                .iter()
                .flat_map(|l| l.zones.iter())
                .filter_map(|z| match z.target {
                    Target::Tab(tab) => Some(tab),
                    _ => None,
                })
                .collect();
            assert_eq!(tabs, Tab::ALL, "{language}");
            for tab in Tab::ALL {
                let word = t.t(words::tab_key(tab));
                assert!(
                    lines
                        .iter()
                        .flat_map(|l| l.fields.iter())
                        .any(|f| f.text == word),
                    "{language}: `{word}` is not drawn whole"
                );
            }
        }
    }

    /// FAILS IF: two rows of one detail say the same label — two addresses,
    /// two aliases.
    #[test]
    fn two_rows_of_a_detail_never_say_the_same_label() {
        let (t, mut r) = (texts(), readings());
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.images[0].aliases.push("debian/trixie".to_owned());
        }
        for kind in [Kind::Instances, Kind::Images] {
            let mut v = View::on(kind);
            v.key(Press::Right, &r, WIDE, Layout::Panes, &t);
            let at = places(WIDE);
            let labels: Vec<String> = v
                .lines(&r, WIDE, Layout::Panes, 60, &t)
                .iter()
                .filter_map(|l| l.fields.iter().find(|f| f.at == at.detail.0))
                .map(|f| f.text.clone())
                .collect();
            let mut seen = BTreeSet::new();
            for label in &labels {
                assert!(
                    seen.insert(label),
                    "{kind:?}: `{label}` twice in {labels:?}"
                );
            }
        }
    }

    /// FAILS IF: a refusal is not said in words, or a key that would ask a
    /// daemon that does not answer is taught.
    #[test]
    fn a_refusal_is_a_sentence_and_no_incus_says_so() {
        let t = texts();
        let r = Readings {
            listing: Some(Err(Refusal::of(Why::NoIncus))),
            remotes: Some(Err(Refusal::of(Why::NoIncus))),
            ..Readings::default()
        };
        let said = texts_of(&View::fresh().lines(&r, WIDE, Layout::Stacked, ROOM, &t));
        assert!(said.contains(&t.t(words::why_key(Why::NoIncus))), "{said}");
        let hint: String = View::fresh()
            .hint(&r, WIDE, Layout::Panes, &t)
            .iter()
            .map(|p| p.text.clone())
            .collect();
        assert!(!hint.contains("C-l") && !hint.contains("C-i"), "{hint}");
    }

    #[test]
    fn a_question_in_the_stacked_layout_gives_back_what_was_there() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        v.key(Press::Delete, &r, NARROW, Layout::Stacked, &t);
        assert!(v.detail_shown(&r, Layout::Stacked));
        v.key(Press::Enter, &r, NARROW, Layout::Stacked, &t);
        assert_eq!(v.focus(), Pane::List);
        assert!(!v.detail_shown(&r, Layout::Stacked));
        assert_eq!(
            v.key(Press::Escape, &r, NARROW, Layout::Stacked, &t),
            Errand::Close
        );
    }

    #[test]
    fn every_command_of_the_table_has_a_press_and_every_question_an_asking() {
        for (notation, id, _) in OF_THE_VIEW {
            assert!(press_of(id).is_some(), "{id}");
            assert!(ronoctua_keymap::Key::parse(notation).is_ok(), "{notation}");
            assert_eq!(pressed(notation), press_of(id));
        }
        assert_eq!(pressed("ESC"), Some(Press::Escape));
        assert_eq!(pressed("C-a"), Some(Press::Allow));
        let ids: BTreeSet<&str> = reached().collect();
        for q in QUESTIONS {
            assert!(ids.contains(q), "{q}");
        }
    }

    /// FAILS IF: the stacked layout (the footer's region, a narrow window)
    /// says the draft's title twice — the detail's title line over the
    /// draft's own heading — or the panes lose it.
    #[test]
    fn the_draft_says_its_title_once_in_both_layouts() {
        let (t, r) = (texts(), readings());
        let title = t.t(crate::key::INCUS_NEW_TITLE);
        for (columns, layout) in [(NARROW, Layout::Stacked), (WIDE, Layout::Panes)] {
            let mut v = View::fresh();
            v.key(Press::Launch, &r, columns, layout, &t);
            let times = v
                .lines(&r, columns, layout, ROOM, &t)
                .iter()
                .flat_map(|l| l.fields.iter())
                .filter(|f| f.text == title)
                .count();
            assert_eq!(times, 1, "{layout:?}");
        }
    }

    /// FAILS IF: turning the open view to another kind (`LEADER s x i`, or
    /// `LEADER s x n` from the images) throws away what was typed in the
    /// draft, or the row chosen in a kind.
    #[test]
    fn turning_the_open_view_to_a_kind_keeps_the_draft_and_the_rows() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.picked(&r).as_deref(), Some("desk"));
        v.key(Press::Launch, &r, WIDE, Layout::Panes, &t);
        v.key(Press::Enter, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.answered(NAME, "box", &r, None, &t), Errand::Nothing);
        // SHOWN, the draft is put away by the turn — and kept.
        v.show(Kind::Images);
        assert_eq!(v.kind(), Kind::Images);
        assert!(!v.drafting());
        v.show(Kind::Instances);
        assert_eq!(v.picked(&r).as_deref(), Some("desk"), "the row chosen");
        v.key(Press::Launch, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.draft().map(|d| d.name.as_str()), Some("box"));
        // A QUESTION UP is taken down by the turn, as by any other key.
        v.key(Press::Escape, &r, WIDE, Layout::Panes, &t);
        v.key(Press::Delete, &r, WIDE, Layout::Panes, &t);
        assert!(v.question().is_some());
        v.show(Kind::Remotes);
        assert!(v.question().is_none());
        assert_eq!(v.draft().map(|d| d.name.as_str()), Some("box"));
    }

    /// FAILS IF: the detail's cursor is kept by its row's INDEX — a snapshot
    /// that expires above it (an event's listing) would move it onto the
    /// next one, which `RET` would then restore.
    #[test]
    fn the_detail_s_cursor_stays_on_its_snapshot_when_the_list_moves() {
        let (t, mut r) = (texts(), readings());
        let snapshots: Vec<Snapshot> = (0..5)
            .map(|n| Snapshot {
                name: format!("snap{n}"),
                created_at: format!("2026-10-0{}T06:00:00Z", n + 1),
            })
            .collect();
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.instances[0].snapshots.clone_from(&snapshots);
        }
        let mut v = View::fresh();
        v.key(Press::Right, &r, WIDE, Layout::Panes, &t);
        v.key(Press::NextTab, &r, WIDE, Layout::Panes, &t);
        v.settle(&r, WIDE, Layout::Panes, &t);
        while v.detail_target(&r, WIDE, Layout::Panes, &t)
            != Some(Target::Snapshot("snap3".to_owned()))
        {
            v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
            v.settle(&r, WIDE, Layout::Panes, &t);
        }
        // SNAP0 EXPIRES: the listing comes without it.
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.instances[0].snapshots.remove(0);
        }
        v.settle(&r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.detail_target(&r, WIDE, Layout::Panes, &t),
            Some(Target::Snapshot("snap3".to_owned()))
        );
        // A NEW ONE FIRST: still on it.
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.instances[0].snapshots.insert(0, snapshots[0].clone());
            l.instances[0].snapshots.insert(
                0,
                Snapshot {
                    name: "early".to_owned(),
                    created_at: "2026-09-30T06:00:00Z".to_owned(),
                },
            );
        }
        v.settle(&r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.detail_target(&r, WIDE, Layout::Panes, &t),
            Some(Target::Snapshot("snap3".to_owned()))
        );
        // AND A KEY STILL MOVES IT: what the person moves is not pulled back.
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        v.settle(&r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.detail_target(&r, WIDE, Layout::Panes, &t),
            Some(Target::Snapshot("snap4".to_owned()))
        );
        // THE ONE IT IS ON GOES: the row that took its place, never a jump.
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.instances[0].snapshots.retain(|s| s.name != "snap4");
        }
        v.settle(&r, WIDE, Layout::Panes, &t);
        assert!(matches!(
            v.detail_target(&r, WIDE, Layout::Panes, &t),
            Some(Target::Snapshot(_))
        ));
    }

    /// FAILS IF: data that arrives moves a column the person scrolled — the
    /// settle that follows a listing brought the list back to its cursor.
    #[test]
    fn data_that_arrives_never_scrolls_a_column_the_person_scrolled() {
        let (t, mut r) = (texts(), readings());
        if let Some(Ok(l)) = r.listing.as_mut() {
            for n in 0..60 {
                l.instances.push(instance(
                    &format!("box{n:02}"),
                    Machine::Container,
                    Status::Stopped,
                ));
            }
        }
        let room = 10;
        let mut v = View::fresh();
        v.settle(&r, WIDE, Layout::Panes, &t);
        v.keep_in_sight(&r, WIDE, Layout::Panes, room, &t);
        v.scroll_to(Pane::List, 30);
        let offset = |v: &View, r: &Readings| {
            v.scrolls(r, WIDE, room, &t)
                .iter()
                .find(|s| s.pane == Pane::List)
                .map(|s| s.offset)
        };
        // AN EVENT'S LISTING ARRIVES: another instance changed.
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.instances[5].status = Status::Running;
        }
        v.settle(&r, WIDE, Layout::Panes, &t);
        assert_eq!(offset(&v, &r), Some(30), "the person's scroll is kept");
        // A KEY of the person's brings the cursor back into sight.
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        v.settle(&r, WIDE, Layout::Panes, &t);
        v.keep_in_sight(&r, WIDE, Layout::Panes, room, &t);
        assert_eq!(offset(&v, &r), Some(1), "the cursor's row, desk's");
    }

    /// FAILS IF: no state is read again for what no event reports — an
    /// address that comes with the lease after the start, the Overview's
    /// figures — or the clock is armed for what is not on screen.
    #[test]
    fn a_state_is_read_again_only_while_the_screen_needs_it() {
        let (t, mut r) = (texts(), readings());
        let mut fresh = instance("fresh", Machine::Container, Status::Running);
        if let Some(s) = fresh.state.as_mut() {
            s.ipv4.clear();
        }
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.instances.push(fresh);
        }
        let mut v = View::fresh();
        // THE PANES: the list shows `fresh` with no address, and web's
        // Overview is beside it.
        assert_eq!(
            v.wants(&r, Layout::Panes).states,
            ["fresh".to_owned(), "web".to_owned()]
        );
        v.key(Press::Right, &r, WIDE, Layout::Panes, &t);
        v.key(Press::NextTab, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.wants(&r, Layout::Panes).states,
            ["fresh".to_owned()],
            "the Snapshots tab shows no figure that moves"
        );
        // STACKED, the detail over the list: only its Overview's.
        let mut v = View::fresh();
        v.key(Press::Right, &r, NARROW, Layout::Stacked, &t);
        assert_eq!(v.wants(&r, Layout::Stacked).states, ["web".to_owned()]);
        // A STOPPED ONE, another kind, or every address here: nothing.
        let mut v = View::fresh();
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.instances.retain(|i| i.name != "fresh");
        }
        assert!(
            v.wants(&r, Layout::Panes).states.is_empty(),
            "desk is stopped"
        );
        v.show(Kind::Images);
        assert!(v.wants(&r, Layout::Panes).states.is_empty());
    }

    /// FAILS IF: the view draws nothing when the daemon's events stopped —
    /// what it shows would pass for current.
    #[test]
    fn lost_events_are_said_above_the_list() {
        let (t, mut r) = (texts(), readings());
        let lost = flat(&t.t(crate::key::INCUS_EVENTS_LOST));
        let list = |r: &Readings, kind: Kind| {
            flat(
                &View::on(kind)
                    .list_rows(r, 60, &t)
                    .iter()
                    .flat_map(|row| row.fields.iter().map(|f| f.text.clone()))
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        };
        assert!(!list(&r, Kind::Instances).contains(&lost));
        r.events = Following::Lost;
        for kind in Kind::ALL {
            assert!(list(&r, kind).contains(&lost), "{kind:?}");
        }
        // A DAEMON THAT ANSWERS NOTHING says its refusal, and that is all.
        r.listing = Some(Err(Refusal::of(Why::NoIncus)));
        assert!(!list(&r, Kind::Instances).contains(&lost));
    }

    #[test]
    fn the_next_snapshot_name_is_the_first_free_one() {
        let s = |n: &str| Snapshot {
            name: n.to_owned(),
            created_at: String::new(),
        };
        assert_eq!(next_snapshot(&[]), "snap0");
        assert_eq!(next_snapshot(&[s("snap0"), s("snap2")]), "snap1");
    }
}
