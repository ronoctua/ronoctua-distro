//! Containers (`LEADER s o`) — the person's Podman: containers, pods,
//! compose projects, images, volumes and networks. One view, two doors.
//!
//! # What is here
//!
//! Everything the screen DECIDES: which lines each layout draws, what a key
//! or a click does to them, what has to be asked of podman as a result, and
//! which streams have to live while something is shown ([`View::wants`]).
//! Nothing here runs a program: `ronoctua-containers` is the one crate that
//! runs podman, `containers_feed.rs` asks it on threads, the queue
//! (`Work::Container`) pulls, builds and runs compose, and the footer's
//! region (`containers_region.rs`) and `containers_window.rs` are the two
//! doors — which hold the view open and carry its errands out with one
//! function (`containers_open.rs`).
//!
//! # Two layouts, one list of lines
//!
//! - **Panes** (a window from `space.containers.panesFrom` cells): three
//!   columns side by side — the kinds with their counts, the list of the
//!   kind, and the detail of the row chosen, with its tabs. Each column keeps
//!   its own selection and scrolls on its own (invariant 15); `←`/`→` move the
//!   keyboard between them, and the divider before the focused one is lit.
//! - **Stacked** (the footer's region, and a narrow window): the kinds on a
//!   strip at the top, `TAB` to the next; the list under it; `RET` opens the
//!   detail OVER the list and `Esc` comes back.
//!
//! Both are [`Line`]s: fields placed in cells and ZONES saying what each
//! stretch of a line is — a kind, a row of the list, a tab, a row of the
//! detail, an answer. The drawing and the pointer read the same zones, so a
//! control is taken where it is drawn.
//!
//! # The view holds no readings
//!
//! The listing, an inspect, a log's lines, a round of stats, the firewall's
//! rules and the queue's jobs arrive in [`Readings`], which the door owns and
//! hands in. The view keeps WHERE it is: the kind, the focused column, the
//! row chosen in each kind (by identity, never by index), the tab, the
//! detail's cursor and a question. Data arriving never moves the cursor.
//!
//! # A question opens on "no"
//!
//! Removing and pruning — and compose's `down`, which removes — ask first, in
//! the detail's place, and the cursor is born on "no".

use std::collections::VecDeque;
use std::path::PathBuf;

use ronoctua_containers::{
    Action, ComposeProject, Container, Details, Image, Job, Listing, Network, Pod, Port, Prune,
    Refusal, State, Usage, Volume,
};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::containers_words as words;
use crate::fields::{Field, Hit as FilesHit, Ink};
use crate::places::{BAR_CELLS, Layout, Pane, PaneScroll, extent, places};

/// The keys this view teaches, and where each one applies.
///
/// EVERY COMMAND IS `C-`, the rule of every view with a detail to act on,
/// and `M-` never: `M-` is the compositor's family.
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "containers-down", Where::Anywhere),
    ("<up>", "containers-up", Where::Anywhere),
    ("RET", "containers-enter", Where::Anywhere),
    ("TAB", "containers-next-kind", Where::Anywhere),
    ("S-TAB", "containers-previous-kind", Where::Anywhere),
    ("<left>", "containers-left", Where::Leftward),
    ("<right>", "containers-right", Where::Rightward),
    ("S-<left>", "containers-previous-tab", Where::OnADetail),
    ("S-<right>", "containers-next-tab", Where::OnADetail),
    ("C-s", "containers-start-stop", Where::OnStartable),
    ("C-r", "containers-restart", Where::OnRestartable),
    ("C-d", "containers-remove", Where::OnRemovable),
    ("C-l", "containers-logs", Where::OnAContainer),
    ("C-t", "containers-shell", Where::OnARunning),
    ("C-i", "containers-inspect", Where::OnInspectable),
    ("C-u", "containers-pull", Where::Anywhere),
    ("C-b", "containers-build", Where::Anywhere),
    ("C-k", "containers-login", Where::Anywhere),
    ("C-x", "containers-prune", Where::OnPrunable),
    ("C-e", "containers-reveal", Where::OnTheEnvironment),
];

/// The typed questions, by the command that asks and answers: an image to
/// pull, a folder to build.
pub const PULL: &str = "containers-pull";
/// See [`PULL`].
pub const BUILD: &str = "containers-build";
/// A registry's login: first the registry and the user, on one line, then
/// the password on a SECRET line ([`LOGIN_PASSWORD`]).
pub const LOGIN: &str = "containers-login";
/// See [`LOGIN`]: the password, masked on the screen and in the log, which
/// reaches `podman login --password-stdin` and nothing else.
pub const LOGIN_PASSWORD: &str = "containers-login-password";

/// Whether the typed question `id` is a secret one — masked as it is typed,
/// and its answer never logged — in either door.
#[must_use]
pub fn secret(id: &str) -> bool {
    id == LOGIN_PASSWORD
}

/// A registry's login on its way: what the first question answered, kept
/// by the door until the password is typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Login {
    /// The registry, as podman names it (`quay.io`, `ghcr.io`, a host:port).
    pub registry: String,
    /// The user on it.
    pub user: String,
}

/// The security centre's firewall, where a blocked port's row leads.
pub const FIREWALL: &str = "security-firewall";

/// Where a key is worth teaching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// When there is a column to the left to go to.
    Leftward,
    /// When there is a column to the right to go to.
    Rightward,
    /// While a detail with tabs is shown.
    OnADetail,
    /// On what can be started or stopped.
    OnStartable,
    /// On what can be restarted.
    OnRestartable,
    /// On what can be removed.
    OnRemovable,
    /// On a container.
    OnAContainer,
    /// On a running container.
    OnARunning,
    /// On what has an `inspect`.
    OnInspectable,
    /// On a kind podman can prune.
    OnPrunable,
    /// On what shows environment values: a container's environment, or an
    /// `inspect` that holds some.
    OnTheEnvironment,
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
    /// One level back, out of a question, and out.
    Escape,
    /// The column to the left.
    Left,
    /// The column to the right.
    Right,
    /// The next kind.
    NextKind,
    /// The previous kind.
    PreviousKind,
    /// The next tab of the detail.
    NextTab,
    /// The previous tab.
    PreviousTab,
    /// Start it, or stop it.
    StartStop,
    /// Restart it.
    Restart,
    /// Remove it, after a question.
    Remove,
    /// Its log.
    Logs,
    /// A shell inside it, in the terminal.
    Shell,
    /// Its `inspect`.
    Inspect,
    /// Pull an image.
    Pull,
    /// Build an image.
    Build,
    /// Log in to a registry: two questions, the second a secret.
    Login,
    /// Prune, after a question.
    Prune,
    /// Show the environment's values, or hide them again.
    Reveal,
}

/// Which key a command id is — the ONE map both doors reach.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "containers-up" => Press::Up,
        "containers-down" => Press::Down,
        "containers-enter" => Press::Enter,
        "containers-left" => Press::Left,
        "containers-right" => Press::Right,
        "containers-next-kind" => Press::NextKind,
        "containers-previous-kind" => Press::PreviousKind,
        "containers-next-tab" => Press::NextTab,
        "containers-previous-tab" => Press::PreviousTab,
        "containers-start-stop" => Press::StartStop,
        "containers-restart" => Press::Restart,
        "containers-remove" => Press::Remove,
        "containers-logs" => Press::Logs,
        "containers-shell" => Press::Shell,
        "containers-inspect" => Press::Inspect,
        "containers-pull" => Press::Pull,
        "containers-build" => Press::Build,
        "containers-login" => Press::Login,
        "containers-prune" => Press::Prune,
        "containers-reveal" => Press::Reveal,
        _ => return None,
    })
}

/// Which key a notation is, through [`OF_THE_VIEW`]. `ESC` is the way back,
/// which every view has and no table teaches (`chrome::with_the_way_back`).
#[must_use]
pub fn pressed(notation: &str) -> Option<Press> {
    if notation == "ESC" {
        return Some(Press::Escape);
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

/// Every command id this view's table reaches — for the registry's census.
pub fn reached() -> impl Iterator<Item = &'static str> {
    OF_THE_VIEW.iter().map(|(_, id, _)| *id)
}

/// A kind of the person's things.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Kind {
    /// Containers, running or not.
    #[default]
    Containers,
    /// Pods.
    Pods,
    /// Compose projects, grouped from their containers' labels.
    Compose,
    /// Images.
    Images,
    /// Volumes.
    Volumes,
    /// Networks.
    Networks,
}

impl Kind {
    /// The six, in the order drawn.
    pub const ALL: [Self; 6] = [
        Self::Containers,
        Self::Pods,
        Self::Compose,
        Self::Images,
        Self::Volumes,
        Self::Networks,
    ];

    /// Where it is in [`Kind::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }

    /// The tabs its detail has.
    #[must_use]
    pub const fn tabs(self) -> &'static [Tab] {
        match self {
            Self::Containers => &Tab::ALL,
            Self::Compose => &[Tab::Overview],
            Self::Pods | Self::Images | Self::Volumes | Self::Networks => {
                &[Tab::Overview, Tab::Inspect]
            }
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

/// A tab of the detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tab {
    /// What it is, in facts.
    #[default]
    Overview,
    /// Its log, as it grows — a child only while shown.
    Logs,
    /// Its use of the machine — a child only while shown.
    Stats,
    /// `podman inspect`, highlighted.
    Inspect,
    /// Its published ports, and what the firewall does with each.
    Ports,
    /// Its mounts.
    Mounts,
    /// Its environment, the values hidden until shown.
    Env,
}

impl Tab {
    /// The seven, in the order drawn.
    pub const ALL: [Self; 7] = [
        Self::Overview,
        Self::Logs,
        Self::Stats,
        Self::Inspect,
        Self::Ports,
        Self::Mounts,
        Self::Env,
    ];
}

// ---------------------------------------------------------------------------
// Readings: what the door holds and hands in.
// ---------------------------------------------------------------------------

/// The most log lines a detail keeps. A log is a stream, and a container
/// that writes a line a millisecond would otherwise grow the door without
/// end; what scrolled past is still in `podman logs`.
pub const LOG_LINES: usize = 2000;

/// How many lines of the log `podman logs` sends before following it.
pub const LOG_TAIL: u32 = 500;

/// How often a round of stats comes, in seconds — podman's own interval.
pub const STATS_SECONDS: u32 = 2;

/// One `inspect`, answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspected {
    /// What was inspected: an id or a name.
    pub key: String,
    /// What podman said — JSON, for the Inspect tab — or why not.
    pub answer: Result<String, Refusal>,
    /// The same answer, read for the other tabs.
    pub details: Option<Details>,
}

/// A container's log, as it streams.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Log {
    /// Whose it is.
    pub id: String,
    /// Its last [`LOG_LINES`] lines.
    pub lines: VecDeque<String>,
    /// The stream ended (the container went, podman stopped following).
    pub ended: bool,
    /// Why it could not start.
    pub refused: Option<Refusal>,
}

impl Log {
    /// One more line, the oldest dropped past [`LOG_LINES`].
    pub fn push(&mut self, line: String) {
        if self.lines.len() == LOG_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }
}

/// What the firewall's two files said, for the Ports tab.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Firewall {
    /// `firewall.json`; `None` when it was never written (the defaults).
    pub settings: Option<ronoctua_security::firewall::Settings>,
    /// Which profile each interface is on; `None` before the helper ran.
    pub status: Option<ronoctua_security::firewall::Status>,
    /// The settings exist and could not be read.
    pub unreadable: bool,
}

/// Everything the view draws from, handed in by the door.
#[derive(Debug, Clone, Default)]
pub struct Readings {
    /// The listing; `None` while the first one is on its way.
    pub listing: Option<Result<Listing, Refusal>>,
    /// The last `inspect`.
    pub inspected: Option<Inspected>,
    /// The log of the container whose Logs tab is shown.
    pub log: Option<Log>,
    /// The last round of stats; `None` before the first.
    pub stats: Option<Result<Vec<Usage>, Refusal>>,
    /// The firewall, for the Ports tab.
    pub firewall: Firewall,
    /// The queue's jobs that are the containers' — a pull, a build, compose.
    pub jobs: Vec<ronoctua_transfers::Job>,
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

    /// How many things of a kind there are — the kinds column's counts.
    #[must_use]
    pub fn count(&self, kind: Kind) -> usize {
        self.listed().map_or(0, |l| match kind {
            Kind::Containers => l.containers.len(),
            Kind::Pods => l.pods.len(),
            Kind::Compose => l.compose.len(),
            Kind::Images => l.images.len(),
            Kind::Volumes => l.volumes.len(),
            Kind::Networks => l.networks.len(),
        })
    }

    /// The identities of a kind's things, in the order drawn.
    #[must_use]
    pub fn keys(&self, kind: Kind) -> Vec<String> {
        self.listed().map_or_else(Vec::new, |l| match kind {
            Kind::Containers => l.containers.iter().map(|c| c.id.clone()).collect(),
            Kind::Pods => l.pods.iter().map(|p| p.id.clone()).collect(),
            Kind::Compose => l.compose.iter().map(|p| p.name.clone()).collect(),
            Kind::Images => l.images.iter().map(|i| i.id.clone()).collect(),
            Kind::Volumes => l.volumes.iter().map(|v| v.name.clone()).collect(),
            Kind::Networks => l.networks.iter().map(|n| n.name.clone()).collect(),
        })
    }

    fn container(&self, id: &str) -> Option<&Container> {
        self.listed()?.containers.iter().find(|c| c.id == id)
    }

    fn pod(&self, id: &str) -> Option<&Pod> {
        self.listed()?.pods.iter().find(|p| p.id == id)
    }

    fn project(&self, name: &str) -> Option<&ComposeProject> {
        self.listed()?.compose.iter().find(|p| p.name == name)
    }

    fn image(&self, id: &str) -> Option<&Image> {
        self.listed()?.images.iter().find(|i| i.id == id)
    }

    fn volume(&self, name: &str) -> Option<&Volume> {
        self.listed()?.volumes.iter().find(|v| v.name == name)
    }

    fn network(&self, name: &str) -> Option<&Network> {
        self.listed()?.networks.iter().find(|n| n.name == name)
    }

    /// The details of `key`, when the last inspect was of it.
    fn details(&self, key: &str) -> Option<&Details> {
        self.inspected
            .as_ref()
            .filter(|i| i.key == key)
            .and_then(|i| i.details.as_ref())
    }
}

/// A container's name: its first, without compose's own prefix games.
#[must_use]
pub fn name_of(c: &Container) -> String {
    c.names.first().cloned().unwrap_or_else(|| short(&c.id))
}

/// podman's short form of an id.
#[must_use]
pub fn short(id: &str) -> String {
    id.chars().take(12).collect()
}

/// An image's name: its first, or its short id when nothing names it.
fn image_name(i: &Image) -> String {
    i.names.first().cloned().unwrap_or_else(|| short(&i.id))
}

/// Whether a container is up — running, or paused inside a run.
const fn is_up(state: State) -> bool {
    matches!(state, State::Running | State::Paused)
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
    /// A blocked port's way to the firewall.
    Firewall,
    /// The environment's values, shown or hidden.
    Reveal,
    /// An answer to the question.
    Answer(bool),
}

/// One row of one column, its fields counted from the column's first cell.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// What is drawn.
    pub fields: Vec<Field>,
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
    pub fields: Vec<Field>,
    /// Its stretches, left to right.
    pub zones: Vec<Zone>,
}

impl Line {
    /// Whether a click takes something on it — some stretch of it is a
    /// target — and so whether a resting pointer may light it: ONE ANSWER
    /// for both doors (the region's `Row::takes`, the window's
    /// `Content::takes`). Which stretch lights, and which a click takes, is
    /// [`View::hover_span`]'s and [`View::clicked`]'s, by the cell.
    #[must_use]
    pub fn takes(&self) -> bool {
        self.zones.iter().any(|z| z.target != Target::Nothing)
    }
}

// ---------------------------------------------------------------------------
// Questions and errands.
// ---------------------------------------------------------------------------

/// A question, drawn in the detail's place, "no" first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Question {
    /// Remove this.
    Remove {
        /// What podman is told.
        action: Action,
        /// Its name, for the sentence.
        name: String,
        /// What the sentence says besides.
        why: Option<&'static str>,
    },
    /// Prune.
    Prune(Prune),
    /// Take a compose project down.
    Down {
        /// The project.
        name: String,
        /// Its compose files.
        files: Vec<PathBuf>,
    },
}

/// What the door has to do about a key or a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Draw again, and nothing else.
    Nothing,
    /// Shut the view.
    Close,
    /// Put this on the clipboard.
    Copy(String),
    /// Ask podman this, on a thread; the name is for the sentence.
    Act(Action, String),
    /// Put this on the queue, under this label.
    Queue(Job, String),
    /// Open a typed question — [`PULL`], [`BUILD`] or [`LOGIN`] — with what
    /// its line holds to begin with (an image's own name, to pull it again).
    Ask(&'static str, Option<String>),
    /// Keep this login and ask its password, on a secret line
    /// ([`LOGIN_PASSWORD`]); the door hands both to the feed, off the loop.
    Password(Login),
    /// A shell inside this container, in the terminal.
    Shell(String),
    /// Run a command of the registry — the firewall.
    Run(&'static str),
    /// Say this; it is already a sentence.
    Say(String),
}

// ---------------------------------------------------------------------------
// The view.
// ---------------------------------------------------------------------------

/// What streams have to live, and what has to be asked, for what is shown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wants {
    /// Inspect this, if the last inspect was of something else.
    pub inspect: Option<String>,
    /// Follow this container's log.
    pub logs: Option<String>,
    /// Follow the stats.
    pub stats: bool,
}

/// The view's state.
#[derive(Debug, Clone, Default)]
pub struct View {
    kind: Kind,
    focus: Pane,
    /// The row chosen in each kind, by identity.
    picked: [Option<String>; 6],
    /// Stacked: the detail is open over the list.
    open: bool,
    tab: Tab,
    /// The detail's cursor, a row of the detail.
    at: usize,
    /// The detail follows the log's end, as `tail -f` does, until a key
    /// moves the cursor off it.
    following: bool,
    /// Each column's first row on screen, in the panes.
    offsets: [usize; 3],
    question: Option<Question>,
    /// Where the keyboard was, and whether the detail was open, when the
    /// question came: an answer puts both back.
    before_question: (Pane, bool),
    /// The question's cursor: `true` on "yes".
    yes: bool,
    /// The environment's values are shown.
    revealed: bool,
}

/// A lean row builder for one column.
struct Col<'a> {
    rows: Vec<Row>,
    room: u16,
    texts: &'a ronoctua_i18n::Texts,
}

fn piece(at: u16, text: String, color: tokens::Color) -> Field {
    Field {
        at,
        text,
        color,
        hit: FilesHit::Nothing,
        explain: None,
        ink: Ink::Grid,
    }
}

impl<'a> Col<'a> {
    fn new(room: u16, texts: &'a ronoctua_i18n::Texts) -> Self {
        Self {
            rows: Vec::new(),
            room,
            texts,
        }
    }

    fn push(&mut self, fields: Vec<Field>, target: Target, heading: bool) {
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

    /// A fact by its label's key.
    fn fact_of(&mut self, key: &str, value: &str, focused: bool) {
        let label = self.texts.t(key);
        self.fact(&label, value, focused, None);
    }
}

impl View {
    /// A fresh view, on the containers.
    #[must_use]
    pub fn fresh() -> Self {
        Self {
            following: true,
            ..Self::default()
        }
    }

    /// The kind shown.
    #[must_use]
    #[cfg(test)]
    pub const fn kind(&self) -> Kind {
        self.kind
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

    /// Whether the detail is on screen: always beside the list in the panes
    /// once there is a row, and over the list when opened when stacked.
    #[must_use]
    pub fn detail_shown(&self, r: &Readings, layout: Layout) -> bool {
        self.picked(r).is_some()
            && match layout {
                Layout::Panes => true,
                Layout::Stacked => self.open,
            }
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

    /// What has to be asked and followed for what is on screen. The door
    /// hands it to the feed after every change: a log or a stats stream
    /// exists only while its tab is shown.
    #[must_use]
    pub fn wants(&self, r: &Readings, layout: Layout) -> Wants {
        if !self.detail_shown(r, layout) {
            return Wants::default();
        }
        let Some(key) = self.picked(r) else {
            return Wants::default();
        };
        let tab = self.tab_shown();
        let inspectable = self.kind != Kind::Compose;
        let running = self.kind == Kind::Containers
            && r.container(&key).is_some_and(|c| c.state == State::Running);
        Wants {
            inspect: inspectable.then(|| key.clone()),
            logs: (self.kind == Kind::Containers && tab == Tab::Logs).then(|| key.clone()),
            stats: running && tab == Tab::Stats,
        }
    }

    /// The hint line: the keys that work where the cursor is.
    #[must_use]
    pub fn hint(
        &self,
        r: &Readings,
        layout: Layout,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        if self.question.is_some() {
            // A QUESTION: the arrows between the two answers, `RET` to give
            // one, `Esc` for no.
            let table: Vec<(&str, &str, bool)> = OF_THE_VIEW
                .iter()
                .filter(|(_, id, _)| {
                    matches!(
                        *id,
                        "containers-down" | "containers-up" | "containers-enter"
                    )
                })
                .map(|(k, id, _)| (*k, *id, false))
                .collect();
            return crate::chrome::with_the_way_back(
                crate::chrome::hint_of_marked(&table, |_| None, texts),
                texts,
            );
        }
        let here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| self.shown(*w, r, layout))
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        crate::chrome::with_the_way_back(
            crate::chrome::hint_of_marked(&here, |_| None, texts),
            texts,
        )
    }

    /// Whether a key of the table works now.
    fn shown(&self, w: Where, r: &Readings, layout: Layout) -> bool {
        let picked = self.picked(r);
        let container = picked
            .as_deref()
            .filter(|_| self.kind == Kind::Containers)
            .and_then(|k| r.container(k));
        match w {
            Where::Anywhere => true,
            Where::Leftward => match layout {
                Layout::Panes => self.focus != Pane::Kinds,
                Layout::Stacked => self.open,
            },
            Where::Rightward => {
                picked.is_some()
                    && match layout {
                        Layout::Panes => self.focus != Pane::Detail,
                        Layout::Stacked => !self.open,
                    }
            }
            Where::OnADetail => self.detail_shown(r, layout) && self.kind.tabs().len() > 1,
            Where::OnStartable => {
                picked.is_some()
                    && matches!(self.kind, Kind::Containers | Kind::Pods | Kind::Compose)
            }
            Where::OnRestartable => {
                picked.is_some() && matches!(self.kind, Kind::Containers | Kind::Pods)
            }
            Where::OnRemovable => picked.is_some(),
            Where::OnAContainer => container.is_some(),
            Where::OnARunning => container.is_some_and(|c| c.state == State::Running),
            Where::OnInspectable => picked.is_some() && self.kind != Kind::Compose,
            Where::OnPrunable => true,
            Where::OnTheEnvironment => {
                self.detail_shown(r, layout)
                    && match self.tab_shown() {
                        Tab::Env => container.is_some(),
                        Tab::Inspect => picked
                            .as_deref()
                            .is_some_and(|k| inspect_hides_values(r, k)),
                        _ => false,
                    }
            }
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
                let count = r
                    .listed()
                    .map(|_| r.count(kind).to_string())
                    .unwrap_or_default();
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

    /// The list of the kind shown.
    fn list_rows(&self, r: &Readings, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let mut col = Col::new(room, texts);
        // WHAT THE QUEUE IS DOING FOR THIS KIND, above the list: a pull or a
        // build under Images, compose under Compose — podman's own last
        // line, which is data.
        for job in &r.jobs {
            let ronoctua_transfers::Work::Container { job: which, label } = &job.work else {
                continue;
            };
            let mine = match which {
                Job::Pull { .. } | Job::Build { .. } => self.kind == Kind::Images,
                Job::ComposeUp { .. } | Job::ComposeDown { .. } | Job::ComposePull { .. } => {
                    self.kind == Kind::Compose
                }
            };
            if !mine || matches!(job.stage, ronoctua_transfers::Stage::Done) {
                continue;
            }
            let what = texts.t_with(
                crate::key::CONTAINERS_WORKING,
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
        let Some(listing) = &r.listing else {
            col.note(
                &texts.t(crate::key::CONTAINERS_READING),
                tokens::color::TEXT_DIM,
            );
            return col.rows;
        };
        let listing = match listing {
            Ok(l) => l,
            Err(refusal) => {
                col.note(&words::refused(*refusal, texts), tokens::color::STATE_ERROR);
                return col.rows;
            }
        };
        let picked = self.picked(r);
        let focused_list = self.focus == Pane::List;
        let chosen = |key: &str| picked.as_deref() == Some(key);
        // EVERY ROW FIRST, then where the values go: one column for the
        // whole list, after its longest name (`chrome::section_column`), so
        // the values read down one line whatever the names are.
        let mut items: Vec<(String, String, String, Option<tokens::Color>)> = Vec::new();
        let item = |items: &mut Vec<_>, key: String, name: String, value: String, tint| {
            items.push((key, name, value, tint));
        };
        let empty = match self.kind {
            Kind::Containers => {
                for c in &listing.containers {
                    let tint = match c.state {
                        State::Running => Some(tokens::color::STATE_SUCCESS),
                        State::Paused => Some(tokens::color::STATE_WARNING),
                        _ => None,
                    };
                    item(
                        &mut items,
                        c.id.clone(),
                        name_of(c),
                        texts.t(words::state_key(c.state)),
                        tint,
                    );
                }
                listing.containers.is_empty()
            }
            Kind::Pods => {
                for p in &listing.pods {
                    let value = texts.t_with(
                        crate::key::CONTAINERS_POD_VALUE,
                        &[
                            ("status", &p.status),
                            ("count", &p.containers.len().to_string()),
                        ],
                    );
                    item(&mut items, p.id.clone(), p.name.clone(), value, None);
                }
                listing.pods.is_empty()
            }
            Kind::Compose => {
                for p in &listing.compose {
                    let up = p
                        .containers
                        .iter()
                        .filter(|id| r.container(id).is_some_and(|c| c.state == State::Running))
                        .count();
                    let value = texts.t_args(
                        crate::key::CONTAINERS_PROJECT_VALUE,
                        &[
                            ("up", ronoctua_i18n::Arg::Number(count(up))),
                            (
                                "count",
                                ronoctua_i18n::Arg::Number(count(p.containers.len())),
                            ),
                        ],
                    );
                    let tint = (up > 0).then_some(tokens::color::STATE_SUCCESS);
                    item(&mut items, p.name.clone(), p.name.clone(), value, tint);
                }
                listing.compose.is_empty()
            }
            Kind::Images => {
                for i in &listing.images {
                    let name = if i.dangling {
                        texts.t_with(
                            crate::key::CONTAINERS_IMAGE_UNNAMED,
                            &[("id", &short(&i.id))],
                        )
                    } else {
                        image_name(i)
                    };
                    let size = crate::sizes::readable_bytes(i.size_bytes, texts.decimal_mark());
                    item(&mut items, i.id.clone(), name, size, None);
                }
                listing.images.is_empty()
            }
            Kind::Volumes => {
                for v in &listing.volumes {
                    item(
                        &mut items,
                        v.name.clone(),
                        v.name.clone(),
                        v.driver.clone(),
                        None,
                    );
                }
                listing.volumes.is_empty()
            }
            Kind::Networks => {
                for n in &listing.networks {
                    let value = n
                        .subnets
                        .first()
                        .cloned()
                        .unwrap_or_else(|| n.driver.clone());
                    item(&mut items, n.name.clone(), n.name.clone(), value, None);
                }
                listing.networks.is_empty()
            }
        };
        let pairs: Vec<(u16, u16, u16)> = items
            .iter()
            .map(|(_, name, value, _)| (0, layout::width(name), layout::width(value)))
            .collect();
        let column = crate::chrome::section_column(0, room, &pairs);
        for (key, name, value, tint) in items {
            let on = chosen(&key);
            let (label_tone, value_tone) = crate::chrome::Kind::ListedSetting.tones(on);
            let fields = vec![
                piece(
                    0,
                    layout::elide_end(&name, column.saturating_sub(2)),
                    label_tone,
                ),
                piece(
                    column,
                    layout::elide_end(&value, room.saturating_sub(column)),
                    value_tone,
                ),
            ];
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
        col.rows
    }

    /// The detail's tabs laid out in `room` cells: each line's tabs as
    /// `(first cell, cells, tab)`.
    ///
    /// A TAB THAT DOES NOT FIT GOES TO THE NEXT LINE, never away: `S-→`
    /// reaches every tab, so every tab is drawn and clicked (invariant 19) —
    /// at three columns' narrowest the detail holds four of seven. The ONE
    /// layout the strip's drawing and its zones both read.
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
            // TWO CELLS OF AIR between two tabs, the air `field_pair` keeps
            // between a label and its value.
            at += w + 2;
        }
        lines
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
                // A LABEL THAT ACTS, in the tone its kind says.
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

    /// How many lines the panes' detail pins above its rows: its strip, when
    /// it shows one.
    fn pinned_in_panes(&self, r: &Readings, columns: u16, texts: &ronoctua_i18n::Texts) -> usize {
        let shown = self.detail_shown(r, Layout::Panes) && self.question.is_none();
        if shown {
            self.tab_lines(detail_width(columns, Layout::Panes), texts)
                .len()
        } else {
            0
        }
    }

    /// The detail's rows under its strip: the tab shown, or the question.
    fn detail_rows(&self, r: &Readings, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let mut col = Col::new(room, texts);
        if let Some(q) = &self.question {
            self.question_rows(&mut col, q);
            return col.rows;
        }
        let Some(key) = self.picked(r) else {
            return col.rows;
        };
        let focused = |col: &Col<'_>| self.focus == Pane::Detail && col.rows.len() == self.at;
        match (self.kind, self.tab_shown()) {
            (_, Tab::Inspect) => self.inspect_rows(&mut col, r, &key),
            (Kind::Containers, Tab::Logs) => self.log_rows(&mut col, r, &key),
            (Kind::Containers, Tab::Stats) => self.stats_rows(&mut col, r, &key),
            (Kind::Containers, Tab::Ports) => self.ports_rows(&mut col, r, &key),
            (Kind::Containers, Tab::Mounts) => {
                let Some(d) = r.details(&key) else {
                    col.note(
                        &texts.t(crate::key::CONTAINERS_READING),
                        tokens::color::TEXT_DIM,
                    );
                    return col.rows;
                };
                if d.mounts.is_empty() {
                    col.note(
                        &texts.t(crate::key::CONTAINERS_MOUNTS_NONE),
                        tokens::color::TEXT_DIM,
                    );
                }
                for m in &d.mounts {
                    let how = texts.t(if m.writable {
                        crate::key::CONTAINERS_MOUNT_WRITABLE
                    } else {
                        crate::key::CONTAINERS_MOUNT_READ_ONLY
                    });
                    let value = texts.t_with(
                        crate::key::CONTAINERS_MOUNT_VALUE,
                        &[("source", &m.source), ("how", &how), ("kind", &m.kind)],
                    );
                    let on = focused(&col);
                    col.fact(&m.destination, &value, on, None);
                }
            }
            (Kind::Containers, Tab::Env) => self.env_rows(&mut col, r, &key),
            (Kind::Containers, _) => self.container_overview(&mut col, r, &key),
            (Kind::Pods, _) => {
                let Some(p) = r.pod(&key) else {
                    return col.rows;
                };
                let rows: Vec<(&str, String)> = vec![
                    (crate::key::CONTAINERS_ROW_NAME, p.name.clone()),
                    (crate::key::CONTAINERS_ROW_STATUS, p.status.clone()),
                    (
                        crate::key::CONTAINERS_ROW_CREATED,
                        crate::chrome::when_text(p.created_s, &r.zone, false, texts),
                    ),
                    (crate::key::CONTAINERS_ROW_ID, p.id.clone()),
                ];
                for (label, value) in rows {
                    let on = focused(&col);
                    col.fact_of(label, &value, on);
                }
                col.heading(crate::key::CONTAINERS_HEADING_MEMBERS);
                self.members(&mut col, r, &p.containers);
            }
            (Kind::Compose, _) => {
                let Some(p) = r.project(&key) else {
                    return col.rows;
                };
                let mut rows: Vec<(String, String)> =
                    vec![(texts.t(crate::key::CONTAINERS_ROW_NAME), p.name.clone())];
                if let Some(dir) = &p.working_dir {
                    rows.push((
                        texts.t(crate::key::CONTAINERS_ROW_FOLDER),
                        dir.display().to_string(),
                    ));
                }
                for (n, f) in p.config_files.iter().enumerate() {
                    rows.push((
                        nth_label(texts, crate::key::CONTAINERS_ROW_FILE, n),
                        f.display().to_string(),
                    ));
                }
                for (label, value) in rows {
                    let on = focused(&col);
                    col.fact(&label, &value, on, None);
                }
                col.heading(crate::key::CONTAINERS_HEADING_MEMBERS);
                self.members(&mut col, r, &p.containers);
            }
            (Kind::Images, _) => {
                let Some(i) = r.image(&key) else {
                    return col.rows;
                };
                // EACH NAME OF AN IMAGE is a row of its own, numbered: two
                // rows of one detail never say the same thing.
                for (n, name) in i.names.iter().enumerate() {
                    let on = focused(&col);
                    let label = nth_label(texts, crate::key::CONTAINERS_ROW_NAME, n);
                    col.fact(&label, name, on, None);
                }
                let mut rows: Vec<(&str, String)> = Vec::new();
                rows.push((
                    crate::key::CONTAINERS_ROW_SIZE,
                    crate::sizes::readable_bytes(i.size_bytes, texts.decimal_mark()),
                ));
                rows.push((
                    crate::key::CONTAINERS_ROW_CREATED,
                    crate::chrome::when_text(i.created_s, &r.zone, false, texts),
                ));
                rows.push((
                    crate::key::CONTAINERS_ROW_USED_BY,
                    texts.t_args(
                        crate::key::CONTAINERS_USED_BY,
                        &[("count", ronoctua_i18n::Arg::Number(i64::from(i.containers)))],
                    ),
                ));
                if let Some(d) = r.details(&key) {
                    if let Some(p) = &d.platform {
                        rows.push((crate::key::CONTAINERS_ROW_PLATFORM, p.clone()));
                    }
                    if !d.command.is_empty() {
                        rows.push((crate::key::CONTAINERS_ROW_COMMAND, d.command.join(" ")));
                    }
                }
                rows.push((crate::key::CONTAINERS_ROW_ID, i.id.clone()));
                for (label, value) in rows {
                    let on = focused(&col);
                    col.fact_of(label, &value, on);
                }
            }
            (Kind::Volumes, _) => {
                let Some(v) = r.volume(&key) else {
                    return col.rows;
                };
                let rows: Vec<(&str, String)> = vec![
                    (crate::key::CONTAINERS_ROW_NAME, v.name.clone()),
                    (crate::key::CONTAINERS_ROW_DRIVER, v.driver.clone()),
                    (
                        crate::key::CONTAINERS_ROW_MOUNTPOINT,
                        v.mountpoint.display().to_string(),
                    ),
                    (
                        crate::key::CONTAINERS_ROW_CREATED,
                        crate::chrome::when_text(v.created_s, &r.zone, false, texts),
                    ),
                ];
                for (label, value) in rows {
                    let on = focused(&col);
                    col.fact_of(label, &value, on);
                }
            }
            (Kind::Networks, _) => {
                let Some(n) = r.network(&key) else {
                    return col.rows;
                };
                let mut rows: Vec<(String, String)> = vec![
                    (texts.t(crate::key::CONTAINERS_ROW_NAME), n.name.clone()),
                    (texts.t(crate::key::CONTAINERS_ROW_DRIVER), n.driver.clone()),
                ];
                for (k, s) in n.subnets.iter().enumerate() {
                    rows.push((
                        nth_label(texts, crate::key::CONTAINERS_ROW_SUBNET, k),
                        s.clone(),
                    ));
                }
                rows.push((
                    texts.t(crate::key::CONTAINERS_ROW_DNS),
                    texts.t(if n.dns_enabled {
                        crate::key::CONTAINERS_ON
                    } else {
                        crate::key::CONTAINERS_OFF
                    }),
                ));
                rows.push((texts.t(crate::key::CONTAINERS_ROW_ID), n.id.clone()));
                for (label, value) in rows {
                    let on = focused(&col);
                    col.fact(&label, &value, on, None);
                }
            }
        }
        col.rows
    }

    /// The containers of a pod or a project, each with its state.
    fn members(&self, col: &mut Col<'_>, r: &Readings, ids: &[String]) {
        for id in ids {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            let (name, state) = r.container(id).map_or_else(
                || (short(id), String::new()),
                |c| (name_of(c), col.texts.t(words::state_key(c.state))),
            );
            let tint = r
                .container(id)
                .filter(|c| c.state == State::Running)
                .map(|_| tokens::color::STATE_SUCCESS);
            col.fact(&name, &state, on, tint);
        }
    }

    fn container_overview(&self, col: &mut Col<'_>, r: &Readings, key: &str) {
        let Some(c) = r.container(key) else {
            return;
        };
        let texts = col.texts;
        let d = r.details(key);
        let when = |s: i64| crate::chrome::when_text(s, &r.zone, false, texts);
        // EACH NETWORK is a row of its own, numbered after the first.
        let mut networks: Vec<(usize, String)> = Vec::new();
        let mut rows: Vec<(&str, String, Option<tokens::Color>)> = vec![
            (crate::key::CONTAINERS_ROW_NAME, name_of(c), None),
            (crate::key::CONTAINERS_ROW_IMAGE, c.image.clone(), None),
            (
                crate::key::CONTAINERS_ROW_STATE,
                texts.t(words::state_key(c.state)),
                (c.state == State::Running).then_some(tokens::color::STATE_SUCCESS),
            ),
            // podman's own status line — data.
            (crate::key::CONTAINERS_ROW_STATUS, c.status.clone(), None),
            (crate::key::CONTAINERS_ROW_CREATED, when(c.created_s), None),
        ];
        if let Some(d) = d {
            if let Some(s) = d.started_s {
                rows.push((crate::key::CONTAINERS_ROW_STARTED, when(s), None));
            }
            if !is_up(c.state)
                && let Some(s) = d.finished_s
            {
                rows.push((crate::key::CONTAINERS_ROW_FINISHED, when(s), None));
            }
            if let Some(code) = d.exit_code.filter(|_| !is_up(c.state)) {
                rows.push((
                    crate::key::CONTAINERS_ROW_EXIT,
                    code.to_string(),
                    (code != 0).then_some(tokens::color::STATE_ERROR),
                ));
            }
            if !d.command.is_empty() {
                rows.push((
                    crate::key::CONTAINERS_ROW_COMMAND,
                    d.command.join(" "),
                    None,
                ));
            }
            if let Some(p) = &d.restart_policy {
                rows.push((crate::key::CONTAINERS_ROW_RESTART, p.clone(), None));
            }
            if d.restarts > 0 {
                rows.push((
                    crate::key::CONTAINERS_ROW_RESTARTS,
                    d.restarts.to_string(),
                    None,
                ));
            }
            for (k, (network, address)) in d.networks.iter().enumerate() {
                let value = if address.is_empty() {
                    network.clone()
                } else {
                    texts.t_with(
                        crate::key::CONTAINERS_NETWORK_VALUE,
                        &[("network", network), ("address", address)],
                    )
                };
                networks.push((k, value));
            }
        }
        if let Some(pod) = c.pod.as_deref() {
            let name = r.pod(pod).map_or_else(|| short(pod), |p| p.name.clone());
            rows.push((crate::key::CONTAINERS_ROW_POD, name, None));
        }
        if let Some(compose) = &c.compose {
            rows.push((
                crate::key::CONTAINERS_ROW_PROJECT,
                compose.project.clone(),
                None,
            ));
            if let Some(s) = &compose.service {
                rows.push((crate::key::CONTAINERS_ROW_SERVICE, s.clone(), None));
            }
        }
        rows.push((crate::key::CONTAINERS_ROW_ID, c.id.clone(), None));
        let networks_after = rows
            .iter()
            .position(|(k, _, _)| {
                matches!(
                    *k,
                    crate::key::CONTAINERS_ROW_POD
                        | crate::key::CONTAINERS_ROW_PROJECT
                        | crate::key::CONTAINERS_ROW_ID
                )
            })
            .unwrap_or(rows.len());
        let mut labelled: Vec<(String, String, Option<tokens::Color>)> = rows
            .into_iter()
            .map(|(k, v, t)| (texts.t(k), v, t))
            .collect();
        for (n, (k, value)) in networks.into_iter().enumerate() {
            labelled.insert(
                networks_after + n,
                (
                    nth_label(texts, crate::key::CONTAINERS_ROW_NETWORK, k),
                    value,
                    None,
                ),
            );
        }
        for (label, value, tint) in labelled {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            col.fact(&label, &value, on, tint);
        }
    }

    fn inspect_rows(&self, col: &mut Col<'_>, r: &Readings, key: &str) {
        let texts = col.texts;
        let Some(i) = r.inspected.as_ref().filter(|i| i.key == key) else {
            col.note(
                &texts.t(crate::key::CONTAINERS_READING),
                tokens::color::TEXT_DIM,
            );
            return;
        };
        let text = match &i.answer {
            Ok(t) => t,
            Err(refusal) => {
                col.note(&words::refused(*refusal, texts), tokens::color::STATE_ERROR);
                return;
            }
        };
        // THE ENVIRONMENT'S VALUES ARE HIDDEN HERE TOO, by the same switch as
        // on its own tab: `inspect` holds every one of them (`Config.Env`, and
        // the `-e` of the command that made it), and a secret the
        // Environment tab hides must not be read, or copied, one tab over.
        let (hidden, hid) = masked(text, &texts.t(crate::key::CONTAINERS_ENV_MASK));
        if hid {
            self.reveal_switch(col, crate::key::CONTAINERS_INSPECT_ENV);
            col.push(crate::chrome::AIR, Target::Nothing, true);
        }
        let shown_text = if self.revealed {
            text.as_str()
        } else {
            hidden.as_str()
        };
        // HIGHLIGHTED BY THE ONE HIGHLIGHTER, its roles coloured by the one
        // map (`chrome::syntax_color`): the file manager's preview and this
        // colour the same JSON the same way. THE CURSOR IS A COLOUR, the
        // keyboard's (`text.here`).
        let json = ronoctua_syntax::of_tag("json");
        for line in shown_text.lines() {
            let shown = layout::elide_end(line, col.room);
            let focused = self.focus == Pane::Detail && col.rows.len() == self.at;
            let fields = if focused {
                vec![piece(0, shown, tokens::color::TEXT_HERE_READ)]
            } else {
                highlighted(&shown, json)
            };
            col.push(fields, Target::Copy(line.trim().to_owned()), false);
        }
    }

    fn log_rows(&self, col: &mut Col<'_>, r: &Readings, key: &str) {
        let texts = col.texts;
        let log = r.log.as_ref().filter(|l| l.id == key);
        let Some(log) = log else {
            col.note(
                &texts.t(crate::key::CONTAINERS_LOGS_WAITING),
                tokens::color::TEXT_DIM,
            );
            return;
        };
        if let Some(refusal) = log.refused {
            col.note(&words::refused(refusal, texts), tokens::color::STATE_ERROR);
            return;
        }
        if log.lines.is_empty() {
            col.note(
                &texts.t(if log.ended {
                    crate::key::CONTAINERS_LOGS_EMPTY
                } else {
                    crate::key::CONTAINERS_LOGS_WAITING
                }),
                tokens::color::TEXT_DIM,
            );
            return;
        }
        // A LOG LINE IS DATA — what the container wrote — drawn as it came
        // (invariant 8's exception, the brain's log line's), one row a line,
        // cut at the column's end; `RET` copies it whole.
        for line in &log.lines {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            // THE CURSOR IS A COLOUR, the keyboard's (`text.here`).
            let color = if on {
                tokens::color::TEXT_HERE_READ
            } else {
                tokens::color::TEXT_PRIMARY
            };
            col.push(
                vec![piece(0, layout::elide_end(line, col.room), color)],
                Target::Copy(line.clone()),
                false,
            );
        }
        if log.ended {
            col.note(
                &texts.t(crate::key::CONTAINERS_LOGS_ENDED),
                tokens::color::TEXT_DIM,
            );
        }
    }

    fn stats_rows(&self, col: &mut Col<'_>, r: &Readings, key: &str) {
        let texts = col.texts;
        if !r.container(key).is_some_and(|c| c.state == State::Running) {
            col.note(
                &texts.t(crate::key::CONTAINERS_STATS_NOT_RUNNING),
                tokens::color::TEXT_DIM,
            );
            return;
        }
        let round = match &r.stats {
            None => {
                col.note(
                    &texts.t(crate::key::CONTAINERS_STATS_WAITING),
                    tokens::color::TEXT_DIM,
                );
                return;
            }
            Some(Err(refusal)) => {
                col.note(&words::refused(*refusal, texts), tokens::color::STATE_ERROR);
                return;
            }
            Some(Ok(round)) => round,
        };
        let Some(u) = round
            .iter()
            .find(|u| key.starts_with(&u.id) || u.id.starts_with(key))
        else {
            col.note(
                &texts.t(crate::key::CONTAINERS_STATS_WAITING),
                tokens::color::TEXT_DIM,
            );
            return;
        };
        // podman's own figures, as it words them: data.
        let rows = [
            (crate::key::CONTAINERS_STAT_CPU, &u.cpu),
            (crate::key::CONTAINERS_STAT_MEMORY, &u.memory),
            (crate::key::CONTAINERS_STAT_MEMORY_SHARE, &u.memory_share),
            (crate::key::CONTAINERS_STAT_NETWORK, &u.network),
            (crate::key::CONTAINERS_STAT_DISK, &u.disk),
            (crate::key::CONTAINERS_STAT_PIDS, &u.pids),
        ];
        for (label, value) in rows {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            col.fact_of(label, value, on);
        }
        col.note(
            &texts.t_args(
                crate::key::CONTAINERS_STATS_EVERY,
                &[(
                    "seconds",
                    ronoctua_i18n::Arg::Number(i64::from(STATS_SECONDS)),
                )],
            ),
            tokens::color::TEXT_DIM,
        );
    }

    fn ports_rows(&self, col: &mut Col<'_>, r: &Readings, key: &str) {
        let texts = col.texts;
        let Some(c) = r.container(key) else {
            return;
        };
        if c.ports.is_empty() {
            col.note(
                &texts.t(crate::key::CONTAINERS_PORTS_NONE),
                tokens::color::TEXT_DIM,
            );
            return;
        }
        // EACH PORT: where it leads on the label, the firewall's word on the
        // value — every value in one column after the longest label
        // (`chrome::section_column`), and a word that does not fit WRAPS under
        // itself rather than being cut: at three columns' narrowest the
        // detail is forty cells, and "open: the firewall is off" cut short is
        // the one place that said so.
        let judged: Vec<(&Port, String, words::Verdict)> = c
            .ports
            .iter()
            .map(|p| {
                let label = texts.t_with(
                    crate::key::CONTAINERS_PORT_LABEL,
                    &[
                        ("published", &port_label(p)),
                        ("inside", &p.container_port.to_string()),
                    ],
                );
                let v = words::verdict(
                    p,
                    r.firewall.settings.as_ref(),
                    r.firewall.status.as_ref(),
                    r.firewall.unreadable,
                );
                (p, label, v)
            })
            .collect();
        let pairs: Vec<(u16, u16, u16)> = judged
            .iter()
            .map(|(_, label, v)| (0, layout::width(label), layout::width(&texts.t(v.key()))))
            .collect();
        let column = crate::chrome::section_column(0, col.room, &pairs);
        for (p, label, v) in &judged {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            let tint = v.blocked().then_some(tokens::color::STATE_WARNING);
            let lines = wrapped_pair(
                column,
                col.room,
                label,
                &texts.t(v.key()),
                crate::chrome::Kind::Fact,
                on,
                tint,
            );
            for (n, fields) in lines.into_iter().enumerate() {
                if n == 0 {
                    // `RET` COPIES THE PORT, what one types somewhere else.
                    col.push(fields, Target::Copy(port_label(p)), false);
                } else {
                    col.push(fields, Target::Nothing, true);
                }
            }
        }
        // WHAT EACH VERDICT MEANS AND WHAT HELPS, said once each, in the order
        // the ports first said them: under shields no rule lets anything in,
        // so their sentence is their own.
        let mut said: Vec<&'static str> = Vec::new();
        for (_, _, v) in &judged {
            let Some(key) = v.explained() else {
                continue;
            };
            if said.contains(&key) {
                continue;
            }
            said.push(key);
            col.push(crate::chrome::AIR, Target::Nothing, true);
            let color = match v {
                words::Verdict::Open | words::Verdict::Unknown => tokens::color::STATE_WARNING,
                _ => tokens::color::TEXT_DIM,
            };
            col.note(&texts.t(key), color);
        }
        if judged.iter().any(|(_, _, v)| v.leads_to_the_firewall()) {
            // THE WAY TO THE FIREWALL, a row that acts: the security centre's
            // rules, where a port is let in and the firewall switched on.
            col.push(crate::chrome::AIR, Target::Nothing, true);
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            let label = texts.t(crate::key::CONTAINERS_PORT_FIREWALL);
            let value = texts.t(crate::key::CONTAINERS_OPEN);
            let fields = crate::chrome::field_pair(
                0,
                col.room,
                &label,
                &value,
                crate::chrome::Kind::Setting,
                on,
            );
            col.push(fields, Target::Firewall, false);
        }
    }

    fn env_rows(&self, col: &mut Col<'_>, r: &Readings, key: &str) {
        let texts = col.texts;
        let Some(d) = r.details(key) else {
            col.note(
                &texts.t(crate::key::CONTAINERS_READING),
                tokens::color::TEXT_DIM,
            );
            return;
        };
        if d.env.is_empty() {
            col.note(
                &texts.t(crate::key::CONTAINERS_ENV_NONE),
                tokens::color::TEXT_DIM,
            );
            return;
        }
        // THE SWITCH FIRST, a row that acts: values stay hidden until asked
        // for, because a token in the environment is a secret on a screen.
        self.reveal_switch(col, crate::key::CONTAINERS_ENV_VALUES);
        col.push(crate::chrome::AIR, Target::Nothing, true);
        for (name, value) in &d.env {
            let on = self.focus == Pane::Detail && col.rows.len() == self.at;
            if self.revealed {
                col.fact(name, value, on, None);
            } else {
                // HIDDEN, AND NOTHING TO COPY: `RET` on a hidden value copies
                // nothing a screen does not show.
                let fields = crate::chrome::field_pair(
                    0,
                    col.room,
                    name,
                    &texts.t(crate::key::CONTAINERS_ENV_MASK),
                    crate::chrome::Kind::Fact,
                    on,
                );
                col.push(fields, Target::Copy(name.clone()), false);
            }
        }
    }

    /// The row that shows the environment's values, or hides them again —
    /// the Environment tab's, and the Inspect tab's over the same values.
    fn reveal_switch(&self, col: &mut Col<'_>, label: &str) {
        let on = self.focus == Pane::Detail && col.rows.len() == self.at;
        let texts = col.texts;
        let value = texts.t(if self.revealed {
            crate::key::CONTAINERS_ENV_SHOWN
        } else {
            crate::key::CONTAINERS_ENV_HIDDEN
        });
        let fields = crate::chrome::field_pair(
            0,
            col.room,
            &texts.t(label),
            &value,
            crate::chrome::Kind::Setting,
            on,
        );
        col.push(fields, Target::Reveal, false);
    }

    fn question_rows(&self, col: &mut Col<'_>, q: &Question) {
        let texts = col.texts;
        let sentence = match q {
            Question::Remove { name, why, .. } => {
                let mut s = texts.t_with(crate::key::CONTAINERS_ASK_REMOVE, &[("name", name)]);
                if let Some(why) = why {
                    s.push(' ');
                    s.push_str(&texts.t(why));
                }
                s
            }
            Question::Prune(what) => texts.t(match what {
                Prune::System => crate::key::CONTAINERS_ASK_PRUNE_SYSTEM,
                Prune::Images => crate::key::CONTAINERS_ASK_PRUNE_IMAGES,
                Prune::Volumes => crate::key::CONTAINERS_ASK_PRUNE_VOLUMES,
            }),
            Question::Down { name, .. } => {
                texts.t_with(crate::key::CONTAINERS_ASK_DOWN, &[("name", name)])
            }
        };
        col.note(&sentence, tokens::color::TEXT_PRIMARY);
        col.push(crate::chrome::AIR, Target::Nothing, true);
        // "NO" FIRST, and the cursor is born on it. "Yes" is in the colour
        // of danger: what it does cannot be undone.
        for (yes, key) in [
            (false, crate::key::CONTAINERS_ANSWER_NO),
            (true, crate::key::CONTAINERS_ANSWER_YES),
        ] {
            let on = self.yes == yes;
            // THE CURSOR IS A COLOUR, the keyboard's: an answer acts.
            let color = if on {
                tokens::color::TEXT_HERE_ACTS
            } else if yes {
                tokens::color::STATE_ERROR
            } else {
                // A LABEL THAT ACTS, in the tone its kind says.
                crate::chrome::Kind::Setting.tones(false).0
            };
            col.push(
                vec![piece(0, layout::elide_end(&texts.t(key), col.room), color)],
                Target::Answer(yes),
                false,
            );
        }
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
        let shown = self.detail_shown(r, Layout::Panes) || self.question.is_some();
        let strip = if shown && self.question.is_none() {
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
                line.fields.extend(r.fields.iter().map(|f| Field {
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
                .map(|f| Field {
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
        if self.open && (self.detail_shown(r, Layout::Stacked) || self.question.is_some()) {
            // THE DETAIL OVER THE LIST: whose it is, its tabs, its rows.
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
            if self.question.is_none() {
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
            let count = r
                .listed()
                .map(|_| r.count(kind).to_string())
                .unwrap_or_default();
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
                // A LABEL THAT ACTS, in the tone its kind says.
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
            Kind::Containers => r.container(key).map_or_else(|| short(key), name_of),
            Kind::Pods => r.pod(key).map_or_else(|| short(key), |p| p.name.clone()),
            Kind::Images => r.image(key).map_or_else(|| short(key), image_name),
            Kind::Compose | Kind::Volumes | Kind::Networks => key.to_owned(),
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

    // -- keys ---------------------------------------------------------------

    /// Keeps every cursor on a row that exists, after new readings: the row
    /// chosen by identity, the detail's by index, the offsets inside their
    /// columns. A log being followed keeps its end in sight.
    pub fn settle(
        &mut self,
        r: &Readings,
        columns: u16,
        layout: Layout,
        room: usize,
        texts: &ronoctua_i18n::Texts,
    ) {
        let keys = r.keys(self.kind);
        if let Some(p) = &self.picked[self.kind.index()]
            && !keys.contains(p)
            && r.listed().is_some()
        {
            self.picked[self.kind.index()] = None;
        }
        if !self.detail_shown(r, layout) && self.question.is_none() && self.focus == Pane::Detail {
            self.focus = Pane::List;
            self.open = false;
        }
        let rows = self.detail_rows(r, detail_width(columns, layout), texts);
        // FOLLOWING: the cursor on the last line the log wrote — never on the
        // sentence after it, which the cursor skips like any heading.
        if self.following && self.tab_shown() == Tab::Logs && self.question.is_none() {
            self.at = rows
                .iter()
                .rposition(|row| !row.heading)
                .unwrap_or_else(|| rows.len().saturating_sub(1));
        }
        self.at = self.at.min(rows.len().saturating_sub(1));
        self.keep_in_sight(r, columns, layout, room, texts);
    }

    /// The offsets of the panes follow their cursors. Only in the panes: a
    /// stacked door scrolls its whole region to the cursor line.
    fn keep_in_sight(
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
        // A LOG FOLLOWED keeps its very end in sight, the sentence after
        // the last line included.
        let target = if self.following && self.tab_shown() == Tab::Logs {
            detail.len().saturating_sub(1).max(self.at)
        } else {
            self.at
        };
        self.offsets[2] = follow(self.offsets[2], target, detail_room, detail.len());
    }

    /// The panes' scroll, as the door's bars and wheel read it: for each
    /// scrolling column, `(pane, first line of its rows, lines, total,
    /// offset)`.
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
        let shown = self.detail_shown(r, Layout::Panes) || self.question.is_some();
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
            Pane::Detail => {
                self.offsets[2] = offset;
                self.following = false;
            }
        }
    }

    /// The pane's offset, as drawn.
    #[must_use]
    #[cfg(test)]
    pub const fn offset(&self, pane: Pane) -> usize {
        match pane {
            Pane::Kinds => 0,
            Pane::List => self.offsets[1],
            Pane::Detail => self.offsets[2],
        }
    }

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
                    // THE STRIP: a kind clicked there is the kind shown, and
                    // the list is where the keyboard already is.
                    Layout::Stacked => {
                        self.focus = Pane::List;
                        false
                    }
                }
            }
            Target::Tab(tab) => {
                self.tab = *tab;
                self.at = 0;
                self.following = *tab == Tab::Logs;
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
            Target::Copy(_) | Target::Firewall | Target::Reveal => {
                let again = self.focus == Pane::Detail && self.at == zone.row;
                self.focus = Pane::Detail;
                self.at = zone.row;
                self.following = false;
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
        (!matches!(z.target, Target::Nothing)).then_some((z.from, z.to))
    }

    fn pick(&mut self, key: String) {
        let i = self.kind.index();
        if self.picked[i].as_ref() != Some(&key) {
            self.picked[i] = Some(key);
            self.at = 0;
            self.offsets[2] = 0;
            self.following = self.tab == Tab::Logs;
            self.revealed = false;
        }
    }

    fn choose_kind(&mut self, kind: Kind) {
        if self.kind != kind {
            self.kind = kind;
            self.at = 0;
            self.offsets[1] = 0;
            self.offsets[2] = 0;
            self.open = false;
            self.revealed = false;
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
                self.at = i;
                break;
            }
        }
        self.following = self.tab_shown() == Tab::Logs && self.at + 1 >= rows.len();
    }

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
                self.choose_kind(self.kind.turned(press == Press::NextKind));
                Errand::Nothing
            }
            Press::NextTab | Press::PreviousTab => {
                let tabs = self.kind.tabs();
                if tabs.len() > 1 && self.detail_shown(r, layout) {
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
                    self.at = 0;
                    self.offsets[2] = 0;
                    self.following = self.tab == Tab::Logs;
                }
                Errand::Nothing
            }
            Press::Escape => match (self.focus, layout) {
                (Pane::Detail, _) => {
                    self.focus = Pane::List;
                    self.open = false;
                    Errand::Nothing
                }
                _ => Errand::Close,
            },
            Press::Left => {
                match (self.focus, layout) {
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
                    Some(Target::Copy(text)) if !text.is_empty() => Errand::Copy(text),
                    Some(Target::Firewall) => Errand::Run(FIREWALL),
                    Some(Target::Reveal) => {
                        self.revealed = !self.revealed;
                        Errand::Nothing
                    }
                    _ => Errand::Nothing,
                },
            },
            Press::Reveal => {
                // ON INSPECT the same values are shown where they are; on any
                // other tab of a container, its Environment.
                if self.detail_shown(r, layout) && self.tab_shown() == Tab::Inspect {
                    self.revealed = !self.revealed;
                } else if self.kind == Kind::Containers && self.detail_shown(r, layout) {
                    self.tab = Tab::Env;
                    self.revealed = !self.revealed;
                }
                Errand::Nothing
            }
            Press::Logs | Press::Inspect => {
                let tab = if press == Press::Logs {
                    Tab::Logs
                } else {
                    Tab::Inspect
                };
                if self.kind.tabs().contains(&tab) && self.picked(r).is_some() {
                    self.tab = tab;
                    self.at = 0;
                    self.offsets[2] = 0;
                    self.following = tab == Tab::Logs;
                    self.enter_detail(r, layout);
                    Errand::Nothing
                } else {
                    self.cannot(texts)
                }
            }
            Press::Shell => match self.picked_container(r) {
                Some(c) if c.state == State::Running => Errand::Shell(c.id.clone()),
                Some(_) => Errand::Say(texts.t(crate::key::CONTAINERS_REFUSED_NOT_RUNNING)),
                None => self.cannot(texts),
            },
            Press::StartStop => self.start_stop(r, texts),
            Press::Restart => self.restart(r, texts),
            Press::Remove => self.remove(r, texts),
            Press::Pull => self.pull(r),
            Press::Build => Errand::Ask(BUILD, None),
            Press::Login => Errand::Ask(LOGIN, None),
            Press::Prune => {
                self.ask(Question::Prune(match self.kind {
                    Kind::Images => Prune::Images,
                    Kind::Volumes => Prune::Volumes,
                    _ => Prune::System,
                }));
                Errand::Nothing
            }
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
                let q = self.unask();
                match (q, yes) {
                    (Some(Question::Remove { action, name, .. }), true) => {
                        Errand::Act(action, name)
                    }
                    (Some(Question::Prune(what)), true) => {
                        Errand::Act(Action::Prune { what }, String::new())
                    }
                    (Some(Question::Down { name, files }), true) => Errand::Queue(
                        Job::ComposeDown {
                            project: name.clone(),
                            files,
                        },
                        name,
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
        if self.question.is_none() {
            self.before_question = (self.focus, self.open);
        }
        self.question = Some(q);
        self.yes = false;
        self.open = true;
        self.focus = Pane::Detail;
    }

    /// The question taken down, whatever the answer: the keyboard goes back
    /// where it was and the detail is open again only if it was — asked from
    /// the stacked list, the list comes back, and `Esc` is one level back
    /// again rather than out of the view.
    fn unask(&mut self) -> Option<Question> {
        let q = self.question.take();
        self.yes = false;
        if q.is_some() {
            (self.focus, self.open) = self.before_question;
        }
        q
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
            crate::key::CONTAINERS_REFUSED_NOT_HERE,
            &[("kind", &texts.t(words::kind_key(self.kind)))],
        ))
    }

    fn picked_container<'r>(&self, r: &'r Readings) -> Option<&'r Container> {
        (self.kind == Kind::Containers)
            .then(|| self.picked(r))
            .flatten()
            .and_then(|k| r.container(&k))
    }

    fn start_stop(&mut self, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(key) = self.picked(r) else {
            return self.cannot(texts);
        };
        match self.kind {
            Kind::Containers => {
                let Some(c) = r.container(&key) else {
                    return Errand::Nothing;
                };
                let action = if is_up(c.state) {
                    Action::Stop { id: c.id.clone() }
                } else {
                    Action::Start { id: c.id.clone() }
                };
                Errand::Act(action, name_of(c))
            }
            Kind::Pods => {
                let Some(p) = r.pod(&key) else {
                    return Errand::Nothing;
                };
                let running = p.status.eq_ignore_ascii_case("running")
                    || p.status.eq_ignore_ascii_case("degraded");
                let action = if running {
                    Action::StopPod { id: p.id.clone() }
                } else {
                    Action::StartPod { id: p.id.clone() }
                };
                Errand::Act(action, p.name.clone())
            }
            Kind::Compose => {
                let Some(p) = r.project(&key) else {
                    return Errand::Nothing;
                };
                let Some(files) = compose_files(p) else {
                    return Errand::Say(texts.t(crate::key::CONTAINERS_REFUSED_NO_FILE));
                };
                let up = p
                    .containers
                    .iter()
                    .any(|id| r.container(id).is_some_and(|c| is_up(c.state)));
                if up {
                    // DOWN REMOVES the project's containers: asked first.
                    self.ask(Question::Down {
                        name: p.name.clone(),
                        files,
                    });
                    Errand::Nothing
                } else {
                    Errand::Queue(
                        Job::ComposeUp {
                            project: p.name.clone(),
                            files,
                        },
                        p.name.clone(),
                    )
                }
            }
            Kind::Images | Kind::Volumes | Kind::Networks => self.cannot(texts),
        }
    }

    fn restart(&self, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(key) = self.picked(r) else {
            return self.cannot(texts);
        };
        match self.kind {
            Kind::Containers => r.container(&key).map_or(Errand::Nothing, |c| {
                Errand::Act(Action::Restart { id: c.id.clone() }, name_of(c))
            }),
            Kind::Pods => r.pod(&key).map_or(Errand::Nothing, |p| {
                Errand::Act(Action::RestartPod { id: p.id.clone() }, p.name.clone())
            }),
            _ => self.cannot(texts),
        }
    }

    fn remove(&mut self, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(key) = self.picked(r) else {
            return self.cannot(texts);
        };
        let q = match self.kind {
            Kind::Containers => r.container(&key).map(|c| {
                let running = is_up(c.state);
                Question::Remove {
                    action: Action::Remove {
                        id: c.id.clone(),
                        force: running,
                    },
                    name: name_of(c),
                    why: running.then_some(crate::key::CONTAINERS_ASK_REMOVE_RUNNING),
                }
            }),
            Kind::Pods => r.pod(&key).map(|p| Question::Remove {
                action: Action::RemovePod {
                    id: p.id.clone(),
                    force: true,
                },
                name: p.name.clone(),
                why: Some(crate::key::CONTAINERS_ASK_REMOVE_POD),
            }),
            Kind::Compose => r.project(&key).and_then(|p| {
                compose_files(p).map(|files| Question::Down {
                    name: p.name.clone(),
                    files,
                })
            }),
            Kind::Images => r.image(&key).map(|i| {
                let used = i.containers > 0;
                Question::Remove {
                    action: Action::RemoveImage {
                        id: i.id.clone(),
                        force: used,
                    },
                    name: image_name(i),
                    why: used.then_some(crate::key::CONTAINERS_ASK_REMOVE_USED),
                }
            }),
            Kind::Volumes => r.volume(&key).map(|v| Question::Remove {
                action: Action::RemoveVolume {
                    name: v.name.clone(),
                },
                name: v.name.clone(),
                why: None,
            }),
            Kind::Networks => r.network(&key).map(|n| Question::Remove {
                action: Action::RemoveNetwork {
                    name: n.name.clone(),
                },
                name: n.name.clone(),
                why: None,
            }),
        };
        match q {
            Some(q) => {
                self.ask(q);
                Errand::Nothing
            }
            None if self.kind == Kind::Compose => {
                Errand::Say(texts.t(crate::key::CONTAINERS_REFUSED_NO_FILE))
            }
            None => Errand::Nothing,
        }
    }

    fn pull(&self, r: &Readings) -> Errand {
        let key = self.picked(r);
        match self.kind {
            // A PULL ALWAYS ASKS WHICH IMAGE — the hint says "pull an image" —
            // and on an image with a name its line holds that name, so `RET`
            // pulls it again and a new name is typed over it.
            Kind::Images => Errand::Ask(
                PULL,
                key.and_then(|k| r.image(&k).cloned())
                    .filter(|i| !i.dangling)
                    .map(|i| image_name(&i)),
            ),
            // A PROJECT'S PULL is compose's: the images its files name.
            Kind::Compose => key
                .and_then(|k| r.project(&k).cloned())
                .and_then(|p| compose_files(&p).map(|files| (p.name, files)))
                .map_or(Errand::Ask(PULL, None), |(project, files)| {
                    Errand::Queue(
                        Job::ComposePull {
                            project: project.clone(),
                            files,
                        },
                        project,
                    )
                }),
            _ => Errand::Ask(PULL, None),
        }
    }

    /// Whether the detail follows a log at its end now — what the person
    /// turned on by opening the Log tab, or by walking to its last line. A
    /// door keeps its page on the end while this holds and the end was in
    /// sight: the one place where data arriving moves the page, because the
    /// person asked for the tail.
    #[must_use]
    pub fn follows_log(&self, r: &Readings, layout: Layout) -> bool {
        self.following
            && self.question.is_none()
            && self.kind == Kind::Containers
            && self.tab_shown() == Tab::Logs
            && self.detail_shown(r, layout)
    }
}

/// A project's files as compose is given them — every one, absolute
/// (`ronoctua_containers::ComposeLabels`) — or `None` when its labels did not
/// say where they are.
fn compose_files(p: &ComposeProject) -> Option<Vec<PathBuf>> {
    (!p.config_files.is_empty() && p.config_files.iter().all(|f| f.is_absolute()))
        .then(|| p.config_files.clone())
}

/// What was typed on a typed question, as an errand — one call for both
/// doors: the footer's region through `Region::answered` (or, closed, the
/// command of that id), the window through `Content::answered`. `home`
/// expands a leading `~`.
#[must_use]
pub fn answered(
    id: &str,
    said: &str,
    home: Option<&std::path::Path>,
    texts: &ronoctua_i18n::Texts,
) -> Errand {
    let said = said.trim();
    if said.is_empty() {
        return Errand::Nothing;
    }
    match id {
        PULL => Errand::Queue(
            Job::Pull {
                image: said.to_owned(),
            },
            said.to_owned(),
        ),
        BUILD => {
            let context = match (said.strip_prefix('~'), home) {
                (Some(rest), Some(home)) => home.join(rest.trim_start_matches('/')),
                _ => PathBuf::from(said),
            };
            if !context.is_dir() {
                return Errand::Say(
                    texts.t_with(crate::key::CONTAINERS_REFUSED_NO_FOLDER, &[("path", said)]),
                );
            }
            if ronoctua_containers::containerfile_in(&context).is_none() {
                return Errand::Say(texts.t_with(
                    crate::key::CONTAINERS_REFUSED_NO_CONTAINERFILE,
                    &[("path", said)],
                ));
            }
            let Some(tag) = ronoctua_containers::tag_of_folder(&context) else {
                return Errand::Say(
                    texts.t_with(crate::key::CONTAINERS_REFUSED_NO_FOLDER, &[("path", said)]),
                );
            };
            Errand::Queue(
                Job::Build {
                    context,
                    file: None,
                    tag: tag.clone(),
                },
                tag,
            )
        }
        LOGIN => match login_of(said) {
            Some(login) => Errand::Password(login),
            None => Errand::Say(texts.t(crate::key::CONTAINERS_REFUSED_LOGIN_WORDS)),
        },
        _ => Errand::Nothing,
    }
}

/// The registry and the user of a login's first answer — two words, a
/// space between them — or `None` for anything else: a scheme, a password
/// typed where the user goes, a third word.
#[must_use]
pub fn login_of(said: &str) -> Option<Login> {
    let mut words = said.split_whitespace();
    let (registry, user) = (words.next()?, words.next()?);
    if words.next().is_some()
        || registry.contains("://")
        || registry.starts_with('-')
        || user.starts_with('-')
    {
        return None;
    }
    Some(Login {
        registry: registry.to_owned(),
        user: user.to_owned(),
    })
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

/// One `label: value` row in which NEITHER is cut: the label wraps in the
/// room before `column`, the value wraps under itself from `column`, and the
/// row is as many lines as the longer of the two — in the tones of its kind.
/// `tint` colours the value as `chrome::tinted` does, every line of it, when
/// the cursor is elsewhere.
fn wrapped_pair(
    column: u16,
    room: u16,
    label: &str,
    value: &str,
    kind: crate::chrome::Kind,
    focused: bool,
    tint: Option<tokens::Color>,
) -> Vec<Vec<Field>> {
    let (label_tone, value_tone) = kind.tones(focused);
    let value_tone = tint.filter(|_| !focused).unwrap_or(value_tone);
    // TWO CELLS OF AIR before the value, as `field_pair` keeps.
    let labels = crate::chrome::note_lines(label, column.saturating_sub(2).max(1));
    let values = crate::chrome::note_lines(value, room.saturating_sub(column).max(1));
    (0..labels.len().max(values.len()).max(1))
        .map(|n| {
            let mut line = Vec::with_capacity(2);
            if let Some(l) = labels.get(n).filter(|l| !l.is_empty()) {
                line.push(piece(0, l.clone(), label_tone));
            }
            if let Some(v) = values.get(n).filter(|v| !v.is_empty()) {
                line.push(piece(column, v.clone(), value_tone));
            }
            line
        })
        .collect()
}

/// Numbered when it is not the first: two rows of one detail never say the
/// same thing (`Name`, `Name 2`), in the words of the person's language.
fn nth_label(texts: &ronoctua_i18n::Texts, key: &str, n: usize) -> String {
    if n == 0 {
        texts.t(key)
    } else {
        texts.t_args(
            crate::key::CONTAINERS_ROW_NTH,
            &[
                ("label", ronoctua_i18n::Arg::Text(&texts.t(key))),
                ("n", ronoctua_i18n::Arg::Number(count(n + 1))),
            ],
        )
    }
}

/// Whether the `inspect` of `key` holds environment values the screen hides.
fn inspect_hides_values(r: &Readings, key: &str) -> bool {
    r.inspected
        .as_ref()
        .filter(|i| i.key == key)
        .and_then(|i| i.answer.as_ref().ok())
        .is_some_and(|text| masked(text, "").1)
}

/// What a JSON array in podman's `inspect` holds, for [`masked`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Held {
    /// Nothing hidden here.
    Nothing,
    /// `NAME=value` entries: `Config.Env`.
    Environment,
    /// A command's words, where `-e NAME=value` may be: `CreateCommand`.
    Command,
}

/// podman's `inspect` with every environment VALUE replaced by `mask` —
/// `Config.Env`'s `NAME=value`, and the `-e NAME=value` (or `--env`) of the
/// command that made the container, `CreateCommand` — and whether anything
/// was. Pure.
///
/// Line by line, as podman writes it: Go's indented JSON puts every entry
/// of a non-empty array on a line of its own, and the names stay, so the
/// person still sees WHICH variables there are.
#[must_use]
pub fn masked(text: &str, mask: &str) -> (String, bool) {
    let mut out = String::with_capacity(text.len());
    let mut held = Held::Nothing;
    // The command's last word was `-e` or `--env`: this one is a value.
    let mut after_flag = false;
    let mut hid = false;
    for line in text.lines() {
        let t = line.trim();
        let shown = match held {
            Held::Nothing => {
                let opens = |name: &str| {
                    t.starts_with(&format!("\"{name}\": ["))
                        && !t.ends_with(']')
                        && !t.ends_with("],")
                };
                if opens("Env") {
                    held = Held::Environment;
                } else if opens("CreateCommand") {
                    held = Held::Command;
                    after_flag = false;
                }
                None
            }
            _ if t.starts_with(']') => {
                held = Held::Nothing;
                None
            }
            Held::Environment => hide_after(line, 1, mask),
            Held::Command => {
                let word = entry_of(line).unwrap_or_default();
                let hidden = if after_flag {
                    hide_after(line, 1, mask)
                } else if word.starts_with("--env=") || word.starts_with("-e=") {
                    hide_after(line, 2, mask)
                } else {
                    None
                };
                after_flag = matches!(word, "-e" | "--env");
                hidden
            }
        };
        hid |= shown.is_some();
        out.push_str(shown.as_deref().unwrap_or(line));
        out.push('\n');
    }
    (out, hid)
}

/// The text inside a line's JSON string, as written.
fn entry_of(line: &str) -> Option<&str> {
    let from = line.find('"')? + 1;
    let to = line.rfind('"')?;
    (to >= from).then(|| &line[from..to])
}

/// The line with what follows its string's `nth` `=` replaced by `mask`;
/// `None` when the string has no such `=`, or no value after it.
fn hide_after(line: &str, nth: usize, mask: &str) -> Option<String> {
    let from = line.find('"')? + 1;
    let to = line.rfind('"')?;
    if to < from {
        return None;
    }
    let at = from + line[from..to].match_indices('=').nth(nth - 1)?.0 + 1;
    (at < to).then(|| format!("{}{mask}{}", &line[..at], &line[to..]))
}

/// A published port's name: `8080/tcp`, and the address when it is one.
fn port_label(p: &Port) -> String {
    match p
        .host_ip
        .as_deref()
        .filter(|ip| !ip.is_empty() && *ip != "0.0.0.0")
    {
        Some(ip) => format!("{ip}:{}/{}", p.host_port, p.protocol),
        None => format!("{}/{}", p.host_port, p.protocol),
    }
}

/// A count as Fluent takes it.
fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// One line of JSON, in the highlighter's colours.
fn highlighted(line: &str, language: Option<ronoctua_syntax::Language>) -> Vec<Field> {
    let Some(l) = language else {
        return vec![piece(0, line.to_owned(), tokens::color::TEXT_PRIMARY)];
    };
    let mut out = Vec::new();
    let mut cursor = 0usize;
    let mut push = |text: &str, from: usize, color| {
        if !text.is_empty() {
            out.push(piece(layout::width(&line[..from]), text.to_owned(), color));
        }
    };
    for range in ronoctua_syntax::ranges(line, l) {
        push(
            &line[cursor..range.start],
            cursor,
            tokens::color::TEXT_PRIMARY,
        );
        push(
            &line[range.start..range.end],
            range.start,
            crate::chrome::syntax_color(range.role),
        );
        cursor = range.end;
    }
    push(&line[cursor..], cursor, tokens::color::TEXT_PRIMARY);
    out
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "containers",
    commands: &[
        // THE PERSON'S CONTAINERS, `LEADER s o` (`crate::containers`): the
        // two doors, and the view's own keys (`containers::OF_THE_VIEW`).
        ronoctua_cmd::Command {
            id: "containers-open",
            label: crate::key::CMD_CONTAINERS_OPEN,
        },
        ronoctua_cmd::Command {
            id: "containers-window",
            label: crate::key::CMD_CONTAINERS_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "containers-down",
            label: crate::key::CMD_CONTAINERS_DOWN,
        },
        ronoctua_cmd::Command {
            id: "containers-up",
            label: crate::key::CMD_CONTAINERS_UP,
        },
        ronoctua_cmd::Command {
            id: "containers-enter",
            label: crate::key::CMD_CONTAINERS_ENTER,
        },
        ronoctua_cmd::Command {
            id: "containers-left",
            label: crate::key::CMD_CONTAINERS_LEFT,
        },
        ronoctua_cmd::Command {
            id: "containers-right",
            label: crate::key::CMD_CONTAINERS_RIGHT,
        },
        ronoctua_cmd::Command {
            id: "containers-next-kind",
            label: crate::key::CMD_CONTAINERS_NEXT_KIND,
        },
        ronoctua_cmd::Command {
            id: "containers-previous-kind",
            label: crate::key::CMD_CONTAINERS_PREVIOUS_KIND,
        },
        ronoctua_cmd::Command {
            id: "containers-next-tab",
            label: crate::key::CMD_CONTAINERS_NEXT_TAB,
        },
        ronoctua_cmd::Command {
            id: "containers-previous-tab",
            label: crate::key::CMD_CONTAINERS_PREVIOUS_TAB,
        },
        ronoctua_cmd::Command {
            id: "containers-start-stop",
            label: crate::key::CMD_CONTAINERS_START_STOP,
        },
        ronoctua_cmd::Command {
            id: "containers-restart",
            label: crate::key::CMD_CONTAINERS_RESTART,
        },
        ronoctua_cmd::Command {
            id: "containers-remove",
            label: crate::key::CMD_CONTAINERS_REMOVE,
        },
        ronoctua_cmd::Command {
            id: "containers-logs",
            label: crate::key::CMD_CONTAINERS_LOGS,
        },
        ronoctua_cmd::Command {
            id: "containers-shell",
            label: crate::key::CMD_CONTAINERS_SHELL,
        },
        ronoctua_cmd::Command {
            id: "containers-inspect",
            label: crate::key::CMD_CONTAINERS_INSPECT,
        },
        ronoctua_cmd::Command {
            id: "containers-pull",
            label: crate::key::CMD_CONTAINERS_PULL,
        },
        ronoctua_cmd::Command {
            id: "containers-build",
            label: crate::key::CMD_CONTAINERS_BUILD,
        },
        ronoctua_cmd::Command {
            id: "containers-login",
            label: crate::key::CMD_CONTAINERS_LOGIN,
        },
        ronoctua_cmd::Command {
            id: "containers-login-password",
            label: crate::key::CMD_CONTAINERS_LOGIN_PASSWORD,
        },
        ronoctua_cmd::Command {
            id: "containers-prune",
            label: crate::key::CMD_CONTAINERS_PRUNE,
        },
        ronoctua_cmd::Command {
            id: "containers-reveal",
            label: crate::key::CMD_CONTAINERS_REVEAL,
        },
    ],
    bindings: &[
        // `s o` — THE PERSON'S CONTAINERS, on a letter of `s` still free: the
        // doubled key is the footer's door, `w` the window's.
        ("s o o", "containers-open", None),
        ("s o w", "containers-window", None),
    ],
    groups: &[("s o", crate::key::GROUP_CONTAINERS)],
    by_name: &[
        // THE REGISTRY LOGIN'S SECOND QUESTION (`containers::LOGIN_PASSWORD`):
        // the secret line the first answer opens, whose answer arrives as this
        // command's argument. A key of its own would ask for a password with no
        // login to give it to.
        "containers-login-password",
    ],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::containers::reached().collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::places::layout_of;
    use ronoctua_containers::{ComposeLabels, Mount};
    use std::collections::BTreeMap;

    const WIDE: u16 = 140;
    const ROOM: usize = 20;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn container(id: &str, name: &str, state: State, project: Option<&str>) -> Container {
        Container {
            id: id.to_owned(),
            names: vec![name.to_owned()],
            image: "docker.io/library/nginx:latest".to_owned(),
            state,
            status: String::new(),
            created_s: 1_790_747_250,
            ports: vec![Port {
                host_ip: None,
                host_port: 8080,
                container_port: 80,
                protocol: "tcp".to_owned(),
            }],
            pod: None,
            compose: project.map(|p| ComposeLabels {
                project: p.to_owned(),
                working_dir: Some(PathBuf::from("/home/someone/site")),
                config_files: vec![PathBuf::from("/home/someone/site/compose.yaml")],
                service: Some("web".to_owned()),
            }),
            labels: BTreeMap::new(),
        }
    }

    /// Three containers (two of a compose project) and an image, listed.
    pub(crate) fn readings() -> Readings {
        let containers = vec![
            container("aaaa1111", "web", State::Running, Some("site")),
            container("bbbb2222", "db", State::Exited, Some("site")),
            container("cccc3333", "scratch", State::Created, None),
        ];
        let compose = ronoctua_containers::compose_of(&containers);
        let listing = Listing {
            containers,
            pods: Vec::new(),
            images: vec![Image {
                id: "img1".to_owned(),
                names: vec!["docker.io/library/nginx:latest".to_owned()],
                size_bytes: 190_000_000,
                created_s: 1_790_000_000,
                dangling: false,
                containers: 1,
            }],
            volumes: Vec::new(),
            networks: Vec::new(),
            compose,
        };
        Readings {
            listing: Some(Ok(listing)),
            ..Readings::default()
        }
    }

    /// The cells line `line` is probed at, counted as its fields are: its
    /// first, where each field begins, and each stretch's first and last
    /// cell and the cell on either side of it — wherever a light and a click
    /// could part.
    pub(crate) fn probes(line: &Line) -> Vec<u16> {
        let mut cells = vec![0];
        cells.extend(line.fields.iter().map(|f| f.at));
        for z in &line.zones {
            cells.extend([
                z.from.saturating_sub(1),
                z.from,
                z.to.saturating_sub(1),
                z.to,
            ]);
        }
        cells.sort_unstable();
        cells.dedup();
        cells
    }

    /// Whether cell `cell` of `line` is the cursor's own place on a strip —
    /// the kind shown, the tab shown — which lights, and where a click has
    /// nothing left to do.
    pub(crate) fn own_place(view: &View, line: &Line, cell: u16) -> bool {
        line.zones
            .iter()
            .filter(|z| crate::chrome::in_cell(cell, z.from, z.to.saturating_sub(z.from)))
            .any(|z| match &z.target {
                Target::Kind(k) => *k == view.kind(),
                Target::Tab(t) => *t == view.tab_shown(),
                _ => false,
            })
    }

    fn details() -> Details {
        Details {
            command: vec!["nginx".to_owned()],
            env: vec![
                ("PATH".to_owned(), "/usr/bin".to_owned()),
                ("API_TOKEN".to_owned(), "s3cr3t".to_owned()),
            ],
            mounts: vec![Mount {
                kind: "bind".to_owned(),
                source: "/home/someone/site/html".to_owned(),
                destination: "/usr/share/nginx/html".to_owned(),
                writable: false,
            }],
            ..Details::default()
        }
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
        assert_eq!(v.picked(&r).as_deref(), Some("aaaa1111"));
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.picked(&r).as_deref(), Some("bbbb2222"));
        // A LISTING THAT REORDERS keeps the row, not the index.
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.containers.reverse();
        }
        assert_eq!(v.picked(&r).as_deref(), Some("bbbb2222"));
        // AND ONE WHERE IT IS GONE falls back to the first.
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.containers.retain(|c| c.id != "bbbb2222");
        }
        v.settle(&r, WIDE, Layout::Panes, ROOM, &t);
        assert_eq!(v.picked(&r).as_deref(), Some("cccc3333"));
    }

    #[test]
    fn the_kinds_turn_with_tab_and_count_what_there_is() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        v.key(Press::NextKind, &r, WIDE, Layout::Stacked, &t);
        assert_eq!(v.kind(), Kind::Pods);
        v.key(Press::PreviousKind, &r, WIDE, Layout::Stacked, &t);
        v.key(Press::PreviousKind, &r, WIDE, Layout::Stacked, &t);
        assert_eq!(v.kind(), Kind::Networks, "round");
        let lines = v.lines(&r, WIDE, Layout::Stacked, ROOM, &t);
        let strip = texts_of(&lines[..1]);
        assert!(strip.contains("Containers 3"), "{strip}");
        assert!(strip.contains("Compose 1"), "{strip}");
        assert!(
            texts_of(&lines).contains("No networks"),
            "{}",
            texts_of(&lines)
        );
    }

    #[test]
    fn a_stream_lives_only_while_its_tab_is_shown() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        // STACKED AND CLOSED: nothing is followed, nothing inspected.
        assert_eq!(v.wants(&r, Layout::Stacked), Wants::default());
        // OPEN: the overview inspects, and follows nothing.
        v.key(Press::Enter, &r, WIDE, Layout::Stacked, &t);
        let w = v.wants(&r, Layout::Stacked);
        assert_eq!(w.inspect.as_deref(), Some("aaaa1111"));
        assert_eq!((w.logs, w.stats), (None, false));
        v.key(Press::Logs, &r, WIDE, Layout::Stacked, &t);
        assert_eq!(
            v.wants(&r, Layout::Stacked).logs.as_deref(),
            Some("aaaa1111")
        );
        v.key(Press::NextTab, &r, WIDE, Layout::Stacked, &t);
        let w = v.wants(&r, Layout::Stacked);
        assert_eq!(
            (w.logs, w.stats),
            (None, true),
            "the stats of a running one"
        );
        // A CONTAINER THAT IS NOT RUNNING has no stats to follow.
        v.key(Press::Escape, &r, WIDE, Layout::Stacked, &t);
        v.key(Press::Down, &r, WIDE, Layout::Stacked, &t);
        v.key(Press::Enter, &r, WIDE, Layout::Stacked, &t);
        assert!(!v.wants(&r, Layout::Stacked).stats);
        // CLOSED AGAIN: every stream goes.
        v.key(Press::Escape, &r, WIDE, Layout::Stacked, &t);
        assert_eq!(v.wants(&r, Layout::Stacked), Wants::default());
    }

    #[test]
    fn removing_asks_first_and_the_cursor_is_born_on_no() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        assert_eq!(
            v.key(Press::Remove, &r, WIDE, Layout::Panes, &t),
            Errand::Nothing
        );
        assert!(v.question().is_some());
        let lines = v.lines(&r, WIDE, Layout::Panes, ROOM, &t);
        let said = texts_of(&lines);
        assert!(said.contains("web"), "{said}");
        // `RET` AT ONCE is "no".
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Nothing
        );
        assert!(v.question().is_none());
        // Asked again and answered yes: a RUNNING one is removed by force.
        v.key(Press::Remove, &r, WIDE, Layout::Panes, &t);
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Act(
                Action::Remove {
                    id: "aaaa1111".to_owned(),
                    force: true
                },
                "web".to_owned()
            )
        );
        // ANY OTHER KEY is no.
        v.key(Press::Prune, &r, WIDE, Layout::Panes, &t);
        assert!(matches!(v.question(), Some(Question::Prune(Prune::System))));
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.key(Press::NextKind, &r, WIDE, Layout::Panes, &t),
            Errand::Nothing
        );
        assert!(v.question().is_none());
    }

    #[test]
    fn compose_goes_up_on_the_queue_and_down_after_a_question() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        v.key(Press::NextKind, &r, WIDE, Layout::Panes, &t);
        v.key(Press::NextKind, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.kind(), Kind::Compose);
        // ONE OF ITS CONTAINERS RUNS: `C-s` takes it down, after a question.
        assert_eq!(
            v.key(Press::StartStop, &r, WIDE, Layout::Panes, &t),
            Errand::Nothing
        );
        v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Queue(
                Job::ComposeDown {
                    project: "site".to_owned(),
                    files: vec![PathBuf::from("/home/someone/site/compose.yaml")]
                },
                "site".to_owned()
            )
        );
        // AND A PULL of a project is compose's.
        assert!(matches!(
            v.key(Press::Pull, &r, WIDE, Layout::Panes, &t),
            Errand::Queue(Job::ComposePull { .. }, _)
        ));
    }

    #[test]
    fn a_blocked_port_says_so_and_leads_to_the_firewall() {
        let (t, mut r) = (texts(), readings());
        r.inspected = Some(Inspected {
            key: "aaaa1111".to_owned(),
            answer: Ok("[]".to_owned()),
            details: Some(details()),
        });
        let mut v = View::fresh();
        v.key(Press::Right, &r, WIDE, Layout::Panes, &t);
        for _ in 0..4 {
            v.key(Press::NextTab, &r, WIDE, Layout::Panes, &t);
        }
        assert_eq!(v.tab_shown(), Tab::Ports);
        let said = texts_of(&v.lines(&r, WIDE, Layout::Panes, ROOM, &t));
        assert!(said.contains("8080/tcp"), "{said}");
        assert!(said.contains(&t.t(words::Verdict::Refused.key())), "{said}");
        // THE LAST ROW is the way to the firewall.
        for _ in 0..3 {
            v.key(Press::Down, &r, WIDE, Layout::Panes, &t);
        }
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Run(FIREWALL)
        );
    }

    #[test]
    fn the_environment_is_hidden_until_shown() {
        let (t, mut r) = (texts(), readings());
        r.inspected = Some(Inspected {
            key: "aaaa1111".to_owned(),
            answer: Ok("[]".to_owned()),
            details: Some(details()),
        });
        let mut v = View::fresh();
        v.key(Press::Right, &r, WIDE, Layout::Panes, &t);
        v.key(Press::PreviousTab, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.tab_shown(), Tab::Env);
        let said = texts_of(&v.lines(&r, WIDE, Layout::Panes, ROOM, &t));
        assert!(!said.contains("s3cr3t"), "a secret on the screen: {said}");
        // `RET` ON THE SWITCH shows them.
        assert_eq!(
            v.key(Press::Enter, &r, WIDE, Layout::Panes, &t),
            Errand::Nothing
        );
        let said = texts_of(&v.lines(&r, WIDE, Layout::Panes, ROOM, &t));
        assert!(said.contains("s3cr3t"), "{said}");
        // AND `C-e` HIDES THEM AGAIN.
        v.key(Press::Reveal, &r, WIDE, Layout::Panes, &t);
        assert!(!texts_of(&v.lines(&r, WIDE, Layout::Panes, ROOM, &t)).contains("s3cr3t"));
    }

    #[test]
    fn the_panes_are_three_columns_and_a_click_takes_the_column_it_is_in() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        let lines = v.lines(&r, WIDE, Layout::Panes, ROOM, &t);
        let at = places(WIDE);
        assert!(at.kinds.0 + at.kinds.1 < at.list.0 && at.list.0 + at.list.1 < at.detail.0);
        // LINE 0: the first kind, the first container and the tabs.
        let panes: Vec<Pane> = lines[0].zones.iter().map(|z| z.pane).collect();
        assert!(
            panes.contains(&Pane::Kinds)
                && panes.contains(&Pane::List)
                && panes.contains(&Pane::Detail)
        );
        // A CLICK ON THE LIST'S SECOND ROW chooses it, and a second click is
        // `RET`.
        let cell = at.list.0 + 1;
        assert!(!v.clicked(&lines, 1, Some(cell), Layout::Panes, &r));
        assert_eq!(v.picked(&r).as_deref(), Some("bbbb2222"));
        assert!(v.clicked(&lines, 1, Some(cell), Layout::Panes, &r));
        // A CLICK ON A TAB changes the tab.
        let logs = lines[0]
            .zones
            .iter()
            .find(|z| z.target == Target::Tab(Tab::Logs))
            .unwrap();
        v.clicked(&lines, 0, Some(logs.from), Layout::Panes, &r);
        assert_eq!(v.tab_shown(), Tab::Logs);
        // THE HOVER covers the column under the pointer, never the line.
        let (from, to) = View::hover_span(&lines, 1, cell).unwrap();
        // …and stops where the column's bar begins (`crate::bars`).
        assert_eq!((from, to), (at.list.0, at.list.0 + at.list.1 - BAR_CELLS));
    }

    #[test]
    fn a_log_is_followed_at_its_end_until_a_key_moves_off_it() {
        let (t, mut r) = (texts(), readings());
        let mut v = View::fresh();
        v.key(Press::Logs, &r, WIDE, Layout::Panes, &t);
        let mut log = Log {
            id: "aaaa1111".to_owned(),
            ..Log::default()
        };
        for n in 0..50 {
            log.push(format!("GET /{n} 200"));
        }
        r.log = Some(log.clone());
        v.settle(&r, WIDE, Layout::Panes, ROOM, &t);
        let said = texts_of(&v.lines(&r, WIDE, Layout::Panes, ROOM, &t));
        assert!(said.contains("GET /49 200"), "the end is in sight: {said}");
        // A KEY UP stops following, and new lines no longer move the page.
        v.key(Press::Up, &r, WIDE, Layout::Panes, &t);
        let before = v.offset(Pane::Detail);
        log.push("GET /50 200".to_owned());
        r.log = Some(log);
        v.settle(&r, WIDE, Layout::Panes, ROOM, &t);
        assert_eq!(v.offset(Pane::Detail), before);
        // AND THE RING has a ceiling.
        let mut long = Log::default();
        for n in 0..(LOG_LINES + 10) {
            long.push(n.to_string());
        }
        assert_eq!(long.lines.len(), LOG_LINES);
    }

    /// FAILS IF: a registry's login is not reachable from the view's keys,
    /// its first answer is not read as a registry and a user (and refused in
    /// words otherwise), or the password's line is not a secret one.
    #[test]
    fn a_login_asks_the_registry_and_then_a_secret_password() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        assert_eq!(
            v.key(Press::Login, &r, WIDE, Layout::Panes, &t),
            Errand::Ask(LOGIN, None)
        );
        assert_eq!(pressed("C-k"), Some(Press::Login));
        assert_eq!(press_of("containers-login"), Some(Press::Login));
        assert_eq!(
            answered(LOGIN, " quay.io  alice ", None, &t),
            Errand::Password(Login {
                registry: "quay.io".to_owned(),
                user: "alice".to_owned(),
            })
        );
        for wrong in [
            "quay.io",
            "https://quay.io alice",
            "quay.io alice hunter2",
            "quay.io -p",
        ] {
            assert!(
                matches!(answered(LOGIN, wrong, None, &t), Errand::Say(_)),
                "{wrong} was taken for a login"
            );
        }
        assert!(secret(LOGIN_PASSWORD));
        assert!(!secret(LOGIN) && !secret(PULL));
    }

    #[test]
    fn a_typed_question_is_answered_the_same_in_both_doors() {
        let t = texts();
        assert_eq!(
            answered(PULL, " nginx ", None, &t),
            Errand::Queue(
                Job::Pull {
                    image: "nginx".to_owned()
                },
                "nginx".to_owned()
            )
        );
        assert_eq!(answered(PULL, "  ", None, &t), Errand::Nothing);
        assert!(matches!(
            answered(BUILD, "/nonexistent/app", None, &t),
            Errand::Say(_)
        ));
        let dir =
            std::env::temp_dir().join(format!("ronoctua-containers-build-{}", std::process::id()));
        let app = dir.join("App");
        std::fs::create_dir_all(&app).unwrap();
        assert!(
            matches!(
                answered(BUILD, &app.to_string_lossy(), None, &t),
                Errand::Say(_)
            ),
            "no Containerfile"
        );
        std::fs::write(app.join("Containerfile"), "FROM scratch\n").unwrap();
        match answered(BUILD, "~/App", Some(&dir), &t) {
            Errand::Queue(Job::Build { context, tag, .. }, _) => {
                assert_eq!(context, app);
                assert_eq!(tag, "localhost/app:latest");
            }
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_refusal_is_a_sentence_and_no_podman_says_so() {
        let t = texts();
        let r = Readings {
            listing: Some(Err(Refusal {
                why: ronoctua_containers::Why::NoPodman,
            })),
            ..Readings::default()
        };
        let said = texts_of(&View::fresh().lines(&r, WIDE, Layout::Stacked, ROOM, &t));
        assert!(
            said.contains(&t.t(words::why_key(ronoctua_containers::Why::NoPodman))),
            "{said}"
        );
    }

    /// A width the stacked layout draws — a narrow window, as the footer's
    /// region is.
    const NARROW: u16 = 80;

    fn texts_in(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).unwrap()
    }

    /// podman's `inspect` of a container, as its JSON is indented: the
    /// environment, and the command that made it with `-e` and `--env=`.
    const INSPECTED: &str = r#"[
     {
          "Id": "aaaa1111",
          "Config": {
               "Env": [
                    "PATH=/usr/bin",
                    "API_TOKEN=s3cr3t"
               ],
               "CreateCommand": [
                    "podman",
                    "run",
                    "-e",
                    "API_TOKEN=s3cr3t",
                    "--env=DB_PASS=hunter2",
                    "--name",
                    "web"
               ],
               "Labels": {}
          },
          "Mounts": []
     }
]"#;

    #[test]
    fn a_question_in_the_stacked_layout_gives_back_what_was_there() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        // ASKED FROM THE LIST, answered "no": the list again, the keyboard on
        // it, and the strip of kinds on top.
        v.key(Press::Remove, &r, NARROW, Layout::Stacked, &t);
        let said = texts_of(&v.lines(&r, NARROW, Layout::Stacked, ROOM, &t));
        assert!(said.contains("Remove web?"), "{said}");
        assert_eq!(
            v.key(Press::Enter, &r, NARROW, Layout::Stacked, &t),
            Errand::Nothing
        );
        assert_eq!(v.focus(), Pane::List);
        assert!(!v.detail_shown(&r, Layout::Stacked));
        let lines = v.lines(&r, NARROW, Layout::Stacked, ROOM, &t);
        assert!(texts_of(&lines[..1]).contains("Containers 3"));
        assert!(
            v.cursor_line(&lines, &r).is_some(),
            "a row of the list is in the cursor's colour again"
        );
        // ASKED FROM THE OPEN DETAIL, and walked away from with `Esc`: the
        // detail again; `Esc` is the way back to the list, and only then out.
        v.key(Press::Enter, &r, NARROW, Layout::Stacked, &t);
        assert!(v.detail_shown(&r, Layout::Stacked));
        v.key(Press::Remove, &r, NARROW, Layout::Stacked, &t);
        assert_eq!(
            v.key(Press::Escape, &r, NARROW, Layout::Stacked, &t),
            Errand::Nothing
        );
        assert!(v.question().is_none());
        assert_eq!(v.focus(), Pane::Detail);
        assert!(v.detail_shown(&r, Layout::Stacked));
        assert_eq!(
            v.key(Press::Escape, &r, NARROW, Layout::Stacked, &t),
            Errand::Nothing
        );
        assert!(!v.detail_shown(&r, Layout::Stacked));
        assert_eq!(
            v.key(Press::Escape, &r, NARROW, Layout::Stacked, &t),
            Errand::Close
        );
    }

    #[test]
    fn a_click_that_takes_something_else_is_no() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        v.key(Press::Remove, &r, WIDE, Layout::Panes, &t);
        let lines = v.lines(&r, WIDE, Layout::Panes, ROOM, &t);
        let at = places(WIDE);
        v.clicked(&lines, 1, Some(at.list.0 + 1), Layout::Panes, &r);
        assert!(v.question().is_none());
        assert_eq!(v.picked(&r).as_deref(), Some("bbbb2222"));
    }

    #[test]
    fn every_tab_is_drawn_and_clicked_at_the_narrowest_three_columns() {
        let r = readings();
        for language in ["en-GB", "pt-BR"] {
            let t = texts_in(language);
            let mut v = View::fresh();
            let columns = tokens::space::CONTAINERS_PANESFROM;
            assert_eq!(layout_of(columns, false), Layout::Panes);
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
            assert_eq!(tabs, Tab::ALL, "{language}: every tab, each once");
            // AND EACH WORD WHOLE, never cut.
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
            // A CLICK ON THE LAST takes it, wherever it wrapped to.
            let (line, zone) = lines
                .iter()
                .enumerate()
                .find_map(|(n, l)| {
                    l.zones
                        .iter()
                        .find(|z| z.target == Target::Tab(Tab::Env))
                        .map(|z| (n, z.clone()))
                })
                .unwrap();
            v.clicked(&lines, line, Some(zone.from), Layout::Panes, &r);
            assert_eq!(v.tab_shown(), Tab::Env, "{language}");
            // THE DETAIL'S BAR starts under the whole strip.
            let pinned = lines
                .iter()
                .take_while(|l| l.zones.iter().any(|z| matches!(z.target, Target::Tab(_))))
                .count();
            let scrolls = v.scrolls(&r, columns, ROOM, &t);
            assert_eq!(scrolls[1].first, pinned, "{language}");
        }
    }

    #[test]
    fn the_inspect_hides_what_the_environment_hides() {
        let (t, mut r) = (texts(), readings());
        r.inspected = Some(Inspected {
            key: "aaaa1111".to_owned(),
            answer: Ok(INSPECTED.to_owned()),
            details: Some(details()),
        });
        let mut v = View::fresh();
        v.key(Press::Inspect, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.tab_shown(), Tab::Inspect);
        let lines = v.lines(&r, WIDE, Layout::Panes, 60, &t);
        let said = texts_of(&lines);
        assert!(
            !said.contains("s3cr3t") && !said.contains("hunter2"),
            "{said}"
        );
        assert!(said.contains("API_TOKEN=••••••"), "the names stay: {said}");
        assert!(said.contains("PATH=••••••"), "{said}");
        // NOTHING HIDDEN IS COPIED either.
        for line in &lines {
            for z in &line.zones {
                if let Target::Copy(text) = &z.target {
                    assert!(!text.contains("s3cr3t"), "copies {text}");
                }
            }
        }
        // `C-e` IS TAUGHT HERE, and shows them where they are.
        let hint: String = v
            .hint(&r, Layout::Panes, &t)
            .iter()
            .map(|p| p.text.clone())
            .collect();
        assert!(hint.contains("C-e"), "{hint}");
        v.key(Press::Reveal, &r, WIDE, Layout::Panes, &t);
        assert_eq!(v.tab_shown(), Tab::Inspect, "the same tab");
        assert!(texts_of(&v.lines(&r, WIDE, Layout::Panes, 60, &t)).contains("s3cr3t"));
        // AND THE SWITCH ROW, first, does the same on `RET`.
        v.key(Press::Right, &r, WIDE, Layout::Panes, &t);
        v.key(Press::Enter, &r, WIDE, Layout::Panes, &t);
        assert!(!texts_of(&v.lines(&r, WIDE, Layout::Panes, 60, &t)).contains("s3cr3t"));
    }

    #[test]
    fn a_mask_hides_values_and_keeps_names() {
        let (hidden, hid) = masked(INSPECTED, "***");
        assert!(hid);
        assert!(hidden.contains(r#""PATH=***","#), "{hidden}");
        assert!(hidden.contains(r#""API_TOKEN=***""#), "{hidden}");
        assert!(hidden.contains(r#""--env=DB_PASS=***","#), "{hidden}");
        assert!(hidden.contains(r#""--name","#) && hidden.contains(r#""web""#));
        assert!(!hidden.contains("s3cr3t") && !hidden.contains("hunter2"));
        // NOTHING TO HIDE, nothing hidden.
        let bare = "{\n  \"Config\": {\n    \"Env\": [],\n    \"Cmd\": null\n  }\n}";
        assert_eq!(masked(bare, "***"), (format!("{bare}\n"), false));
    }

    #[test]
    fn a_port_says_its_whole_verdict_at_the_narrowest_three_columns() {
        let mut r = readings();
        let columns = tokens::space::CONTAINERS_PANESFROM;
        for (settings, verdict) in [
            (
                ronoctua_security::firewall::Settings {
                    enabled: false,
                    ..Default::default()
                },
                words::Verdict::Open,
            ),
            (
                ronoctua_security::firewall::Settings {
                    shields: true,
                    ..Default::default()
                },
                words::Verdict::Shields,
            ),
            (Default::default(), words::Verdict::Refused),
        ] {
            r.firewall.settings = Some(settings);
            for language in ["en-GB", "pt-BR"] {
                let t = texts_in(language);
                let mut v = View::fresh();
                v.key(Press::Right, &r, columns, Layout::Panes, &t);
                for _ in 0..4 {
                    v.key(Press::NextTab, &r, columns, Layout::Panes, &t);
                }
                assert_eq!(v.tab_shown(), Tab::Ports);
                let at = places(columns);
                let detail: Vec<String> = v
                    .lines(&r, columns, Layout::Panes, 60, &t)
                    .iter()
                    .map(|l| {
                        l.fields
                            .iter()
                            .filter(|f| f.at >= at.detail.0)
                            .map(|f| f.text.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect();
                let flat = detail
                    .join(" ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(!flat.contains('…'), "{language}: cut: {flat}");
                let word = t.t(verdict.key());
                assert!(
                    flat.contains(&word),
                    "{language}: `{word}` is not whole in {flat}"
                );
                let why = t.t(verdict.explained().unwrap());
                let why = why.split_whitespace().collect::<Vec<_>>().join(" ");
                assert!(
                    flat.contains(&why),
                    "{language}: {why} is not said in {flat}"
                );
                // SHIELDS ARE NOT A MISSING RULE.
                if verdict == words::Verdict::Shields {
                    let rule = t.t(crate::key::CONTAINERS_PORTS_BLOCKED);
                    let rule = rule.split_whitespace().collect::<Vec<_>>().join(" ");
                    assert!(!flat.contains(&rule), "{language}: {flat}");
                }
            }
        }
    }

    #[test]
    fn a_pull_on_images_asks_and_holds_the_name() {
        let (t, r) = (texts(), readings());
        let mut v = View::fresh();
        for _ in 0..3 {
            v.key(Press::NextKind, &r, WIDE, Layout::Panes, &t);
        }
        assert_eq!(v.kind(), Kind::Images);
        assert_eq!(
            v.key(Press::Pull, &r, WIDE, Layout::Panes, &t),
            Errand::Ask(PULL, Some("docker.io/library/nginx:latest".to_owned()))
        );
        // ELSEWHERE, an empty line.
        v.key(Press::NextKind, &r, WIDE, Layout::Panes, &t);
        assert_eq!(
            v.key(Press::Pull, &r, WIDE, Layout::Panes, &t),
            Errand::Ask(PULL, None)
        );
    }

    #[test]
    fn two_rows_of_a_detail_never_say_the_same_label() {
        let (t, mut r) = (texts(), readings());
        if let Some(Ok(l)) = r.listing.as_mut() {
            l.images[0].names.push("localhost/web:1".to_owned());
            l.networks.push(Network {
                id: "net1".to_owned(),
                name: "site_default".to_owned(),
                driver: "bridge".to_owned(),
                subnets: vec!["10.89.0.0/24".to_owned(), "fd00::/64".to_owned()],
                dns_enabled: true,
            });
        }
        let mut v = View::fresh();
        for kind in [Kind::Images, Kind::Networks] {
            while v.kind() != kind {
                v.key(Press::NextKind, &r, WIDE, Layout::Panes, &t);
            }
            let at = places(WIDE);
            let labels: Vec<String> = v
                .lines(&r, WIDE, Layout::Panes, 60, &t)
                .iter()
                .filter_map(|l| l.fields.iter().find(|f| f.at == at.detail.0))
                .map(|f| f.text.clone())
                .collect();
            let mut seen = std::collections::BTreeSet::new();
            for label in &labels {
                assert!(
                    seen.insert(label),
                    "{kind:?}: `{label}` twice in {labels:?}"
                );
            }
        }
    }

    #[test]
    fn every_command_of_the_table_has_a_press_and_every_press_a_command() {
        for (notation, id, _) in OF_THE_VIEW {
            assert!(press_of(id).is_some(), "{id}");
            assert!(ronoctua_keymap::Key::parse(notation).is_ok(), "{notation}");
            assert_eq!(pressed(notation), press_of(id));
        }
        assert_eq!(pressed("ESC"), Some(Press::Escape));
    }
}
