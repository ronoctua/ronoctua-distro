//! THE LLM ACCOUNTS' SHARED HALF: what the accounts' view and the module both
//! read of the same machine — the lines a mark changes colour at, the colour
//! of a window and of an API key's credits, and the sets the accounts and
//! their agents wear on the modeline.
//!
//! It is compiled into the brain's LIBRARY (`ronoctua-brain`), beside the
//! manifest, because the footer's accounts view (`agents.rs`, which this
//! module's `namedBy` lists) reads it; `module.rs`, the daemon's half, sees it
//! through `super::`. So the bars of the view and the marks of the modeline
//! are coloured by ONE function, and a line kept by one door is the line the
//! other reads. Nothing here watches or runs: that is `module.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ronoctua_llm::sessions::Status;
use ronoctua_llm::statusline::Limits;
use ronoctua_llm::{Account, Kind as Agent, Window, usage};
use ronoctua_terminals::registry::Registry;
use ronoctua_terminals::state::ProgramState;
use serde::{Deserialize, Serialize};

use crate::module::{Birth, Mark, MarkId, Tone, key_is_well_formed};
use crate::moment::Moment;

/// The shell's command a click on an account's lead runs: the accounts'
/// view, which the click hands the set's number — the account's place in the
/// list (`MarkId::opening`). The count of the hidden agents opens it too.
pub const OPENS: &str = "agents-open";

/// The shell's command a click on one AGENT runs: the agent itself — the
/// window it works in, brought forward, or opened when it has none — which
/// the click hands the agent's session (`MarkId::key`).
pub const SHOWS: &str = "agents-show";

/// The person's lines, in the module's configuration folder.
pub const LINES_FILE: &str = "lines.json";

/// The two lines, in per cent of a window: a mark turns to a warning from
/// the first and to danger from the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Lines {
    /// A warning from this much of a window used.
    pub warn_percent: u8,
    /// Danger from this much.
    pub danger_percent: u8,
}

impl Default for Lines {
    /// The distro's: 80 and 95.
    fn default() -> Self {
        Self {
            warn_percent: 80,
            danger_percent: 95,
        }
    }
}

impl Lines {
    /// Whether they are two lines: each a whole per cent from 1 to 100, the
    /// warning below the danger.
    #[must_use]
    pub const fn fine(self) -> bool {
        self.warn_percent >= 1
            && self.warn_percent < self.danger_percent
            && self.danger_percent <= 100
    }
}

/// The colour a window used this much wears, by the lines: quiet below the
/// warning, a warning, then danger. Nothing known is quiet — the mark is
/// there, and says nothing yet.
#[must_use]
pub fn tone_of(used_percent: Option<f32>, lines: Lines) -> Tone {
    match used_percent {
        Some(p) if p >= f32::from(lines.danger_percent) => Tone::Danger,
        Some(p) if p >= f32::from(lines.warn_percent) => Tone::Warning,
        _ => Tone::Quiet,
    }
}

/// The most used of `windows` that has not reset at `now_s` — the one an
/// account's colour follows.
#[must_use]
pub fn most_used(windows: &[Window], now_s: i64) -> Option<f32> {
    windows
        .iter()
        .filter(|w| w.resets_at_s > now_s && w.used_percent.is_finite())
        .map(|w| w.used_percent)
        .reduce(f32::max)
}

/// Every window an account's numbers say, reset or not: what its status
/// line (Claude Code) or its logs (Codex) last said, and the usage
/// endpoint's last answer with its weekly windows of a model each. Two
/// readings of one window are its two moments; [`most_used`] takes the
/// higher, since a window only fills until it resets. What the marks are
/// coloured from — and what the accounts' view hands this to, to prove its
/// bars wear the same colour.
#[must_use]
pub fn windows_said(limits: Option<Limits>, usage: Option<&usage::Kept>) -> Vec<Window> {
    let mut out = Vec::new();
    if let Some(limits) = limits {
        out.extend(limits.five_hour);
        out.extend(limits.seven_day);
    }
    if let Some(u) = usage.and_then(|k| k.usage) {
        out.extend(
            [
                u.five_hour,
                u.seven_day,
                u.seven_day_opus,
                u.seven_day_sonnet,
            ]
            .into_iter()
            .flatten(),
        );
    }
    out
}

/// The colour an account's mark wears at `now`: its most used window that
/// has not reset, by the person's lines.
#[must_use]
pub fn mark_tone(windows: &[Window], lines: Lines, now: Moment) -> Tone {
    tone_of(most_used(windows, now), lines)
}

/// The colour an API key's credits wear: danger while Claude Code says they
/// ran out, quiet otherwise — what the view's row and the mark both wear.
#[must_use]
pub const fn credits_tone(out_since: Option<Moment>) -> Tone {
    match out_since {
        Some(_) => Tone::Danger,
        None => Tone::Quiet,
    }
}

/// The colour an account's mark wears at `now`: an API key's by its credits
/// (it has no window), every other by its windows.
#[must_use]
pub fn account_tone(seen: &Seen, lines: Lines, now: Moment) -> Tone {
    if seen.agent.has_windows() {
        mark_tone(&seen.windows, lines, now)
    } else {
        credits_tone(seen.out_of_credits_since)
    }
}

/// The person's lines, as the file says them now — the distro's when there
/// is no file, or one that does not hold two lines.
#[must_use]
pub fn lines() -> Lines {
    read_lines(&lines_store(own_config()))
}

/// Where the person's lines are kept — what a door watches to repaint its
/// bars when they change.
#[must_use]
pub fn lines_file() -> Option<PathBuf> {
    lines_store(own_config()).path()
}

/// One of the two lines, changed alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// The warning's.
    Warn,
    /// The danger's.
    Danger,
}

/// Why a line was not kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unkept {
    /// The warning would not come before the danger.
    NotInOrder,
    /// The file is there and does not read: nothing is written over it.
    Unreadable,
    /// The file could not be written.
    Unwritten,
}

impl Unkept {
    /// The module's word for it — the page's refusal.
    #[must_use]
    pub const fn why(self) -> &'static str {
        match self {
            Self::NotInOrder => super::why::NOT_IN_ORDER,
            Self::Unreadable => super::why::UNREADABLE,
            Self::Unwritten => super::why::UNKEPT,
        }
    }
}

/// Keeps ONE of the person's lines, laid over what the file says NOW — the
/// footer's view, the window and the module's page all write it, and a copy
/// read earlier by one of them must not put back the line another changed.
///
/// # Errors
///
/// [`Unkept`]: the two lines would cross, the file does not read, or it could
/// not be written.
pub fn keep_line(which: Which, percent: u8) -> Result<Lines, Unkept> {
    write_line(&lines_store(own_config()), which, percent)
}

/// The mark an agent's accounts LEAD their sets with — its glyph token is
/// what the accounts' view draws them with.
#[must_use]
pub fn mark_of(agent: Agent) -> Option<&'static MarkId> {
    super::MANIFEST.mark(agent.id())
}

/// The module's configuration folder, under the machine's roots.
fn own_config() -> Option<PathBuf> {
    Birth {
        zone: Arc::new(crate::moment::Fixed(0)),
        roots: crate::prefs::Where::of_env(),
    }
    .config_dir(super::ID)
}

/// The lines' store, in a configuration folder.
#[must_use]
pub fn lines_store(dir: Option<PathBuf>) -> ronoctua_store::Store<Lines> {
    ronoctua_store::Store::at(dir, LINES_FILE)
}

/// The marks this module would put on the modeline now, read once from the
/// machine's files — what a photograph's mind is made of
/// (`photograph --door footer --accounts-marks`), so a picture never shows a
/// mark the module cannot draw. The module's own reading ([`accounts_now`],
/// [`windows_now`], [`agents_now`], [`lines`]) and its own [`marks_for`],
/// and no watch.
#[must_use]
pub fn marks_read_now(now: Moment) -> Vec<Mark> {
    let accounts = accounts_now().unwrap_or_default();
    marks_with_agents(&accounts, &agents_now(&accounts), now)
}

/// The same, with the agents PLANTED — a photograph's (`--agents <file>`,
/// [`planted`]): the accounts and their numbers are still read from the
/// machine's files, and the marks still made by [`marks_for`], so a picture
/// of agents is the module's drawing of them, never a mark written by hand.
/// The places are given as the module gives them at first sight: by when
/// each agent was first seen, then by its session.
#[must_use]
pub fn marks_read_with(now: Moment, agents: &[AgentSeen]) -> Vec<Mark> {
    marks_with_agents(&accounts_now().unwrap_or_default(), agents, now)
}

/// The accounts' marks with these agents, every account's numbers read now.
fn marks_with_agents(accounts: &[Account], agents: &[AgentSeen], now: Moment) -> Vec<Mark> {
    // A LIST THAT DOES NOT READ keeps no marks it cannot vouch for — the
    // module's own rule (`Kept::unreadable`): the caller hands none.
    let seen: Vec<Seen> = accounts
        .iter()
        .map(|a| Seen::of(a, windows_now(a), credits_now(a)))
        .collect();
    let mut places = Places::default();
    places.settle(agents);
    marks_for(&seen, agents, &places, lines(), now)
}

/// The agents a photograph plants (`--agents <file>`): a JSON list of
/// [`AgentSeen`], as this module reads them.
///
/// # Errors
///
/// The reason, in English, when the text is not such a list.
pub fn planted(json: &str) -> Result<Vec<AgentSeen>, String> {
    serde_json::from_str(json).map_err(|e| format!("not a list of the accounts' agents: {e}"))
}

/// What a store says, two lines or the distro's.
#[must_use]
pub fn read_lines(store: &ronoctua_store::Store<Lines>) -> Lines {
    Some(store.read()).filter(|l| l.fine()).unwrap_or_default()
}

/// Writes one line over what the file says now — `parsed`, never `read`:
/// this writes the whole file back, and a file that does not read is
/// refused, never replaced. A file that reads and does not hold two lines is
/// the distro's, as every reader takes it.
///
/// # Errors
///
/// [`Unkept`]: the two lines would cross, the file does not read, or it could
/// not be written.
pub fn write_line(
    store: &ronoctua_store::Store<Lines>,
    which: Which,
    percent: u8,
) -> Result<Lines, Unkept> {
    let now = store.parsed().ok_or(Unkept::Unreadable)?;
    let mut lines = if now.fine() { now } else { Lines::default() };
    match which {
        Which::Warn => lines.warn_percent = percent,
        Which::Danger => lines.danger_percent = percent,
    }
    if !lines.fine() {
        return Err(Unkept::NotInOrder);
    }
    store.write(&lines).map_err(|e| {
        tracing::warn!(error = %e, "the LLM accounts' lines could not be written");
        Unkept::Unwritten
    })?;
    Ok(lines)
}

// ---------------------------------------------------------------------------
// The accounts, as the marks need them — read by the module's watches and by
// a photograph alike.
// ---------------------------------------------------------------------------

/// One account, as the marks need it.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    /// The account's id in the person's list.
    pub id: String,
    /// Whose agent it is.
    pub agent: Agent,
    /// The person's word for it — the mark's label.
    pub name: String,
    /// Every window its numbers say, reset or not ([`windows_said`]).
    pub windows: Vec<Window>,
    /// Since when Claude Code says an API key's credits ran out
    /// ([`credits_now`]); `None` for every other account.
    pub out_of_credits_since: Option<Moment>,
}

impl Seen {
    /// An account, the windows read for it and its credits' word.
    #[must_use]
    pub fn of(
        account: &Account,
        windows: Vec<Window>,
        out_of_credits_since: Option<Moment>,
    ) -> Self {
        Self {
            id: account.id.clone(),
            agent: account.kind,
            name: account.name.clone(),
            windows,
            out_of_credits_since,
        }
    }
}

/// The person's accounts, as the one crate that reads the agents' state keeps
/// the list; `None` when the list is there and does not read.
#[must_use]
pub fn accounts_now() -> Option<Vec<Account>> {
    ronoctua_llm::Accounts::store()
        .parsed()
        .map(|list| list.accounts)
}

/// Every window an account's numbers say now: the status line's (Claude Code)
/// or the logs' (Codex), and the endpoint's last answer — put together by the
/// one function the view asks too ([`windows_said`]).
#[must_use]
pub fn windows_now(account: &Account) -> Vec<Window> {
    windows_said(
        ronoctua_llm::statusline::limits(account).ok(),
        ronoctua_llm::usage::kept(account).as_ref(),
    )
}

/// Since when an API key's credits ran out, as the one crate that keeps the
/// word says — `None` for every other account, and for a word that does not
/// read: the view says why, and the mark does not cry wolf.
#[must_use]
pub fn credits_now(account: &Account) -> Option<Moment> {
    ronoctua_llm::credits::out_since(account).ok().flatten()
}

/// How many accounts carry a set: the sets the leads declare, the same for
/// every agent.
#[must_use]
pub fn marked_most() -> usize {
    super::MANIFEST
        .marks
        .iter()
        .filter(|m| m.leads)
        .map(|m| usize::from(m.sets))
        .min()
        .unwrap_or(0)
}

/// How many places an account's set has for its agents: the instances the
/// agents' marks declare.
#[must_use]
pub fn places_most() -> u8 {
    super::MANIFEST
        .marks
        .iter()
        .filter(|m| m.sets != 0 && m.instances != 0)
        .map(|m| m.instances)
        .min()
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// The agents, in their account's set.
// ---------------------------------------------------------------------------

/// What an agent is doing, as its glyph and its tone say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Doing {
    /// It is working on what it was asked.
    Working,
    /// It waits for the person: a permission, a question, a login.
    Waiting,
    /// It finished, or it was started and has not been asked anything yet.
    Done,
    /// It stopped on an error.
    Failed,
}

impl Doing {
    /// Every one, for the words and glyphs a door draws them with.
    pub const ALL: [Self; 4] = [Self::Working, Self::Waiting, Self::Done, Self::Failed];

    /// A session's status as the agents' reader says it
    /// (`ronoctua_llm::agents::merged`).
    #[must_use]
    pub const fn of(status: Status) -> Self {
        match status {
            Status::Working => Self::Working,
            Status::Waiting => Self::Waiting,
            Status::Done => Self::Done,
            Status::Failed => Self::Failed,
        }
    }

    /// Its mark: one per state, all the same robot's head — the tone tells
    /// them apart.
    #[must_use]
    pub const fn mark(self) -> MarkId {
        match self {
            Self::Working => super::mark::AGENT_WORKING,
            Self::Waiting => super::mark::AGENT_WAITING,
            Self::Done => super::mark::AGENT_DONE,
            Self::Failed => super::mark::AGENT_FAILED,
        }
    }

    /// Its tone, the one thing that says the state on the modeline (every
    /// agent wears the same glyph): blue is for focus and nothing else,
    /// so working is the teal of a thing under way and well — never the
    /// watch's colour, which the owner read as an agent in trouble — and
    /// waiting the caution's orange.
    #[must_use]
    pub const fn tone(self) -> Tone {
        match self {
            Self::Working => Tone::Working,
            Self::Waiting => Tone::Warning,
            Self::Done => Tone::Quiet,
            Self::Failed => Tone::Danger,
        }
    }
}

/// One agent working in one of the person's accounts, as the marks need it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AgentSeen {
    /// Its account's id in the person's list (`Account::id`); an agent of an
    /// account the list does not have is not drawn.
    pub account: String,
    /// Its session — what a click hands [`SHOWS`] (`MarkId::key`).
    pub session: String,
    /// Where it works, as the person reads it (`~/src/distro`): its mark's
    /// label — data, drawn as it is.
    pub folder: String,
    /// What it is doing.
    pub doing: Doing,
    /// When it was first seen: the order agents first seen together are
    /// given their places in.
    #[serde(default)]
    pub since: Moment,
    /// Our terminal it runs in, when it runs in one — where a click shows
    /// it ([`Whereabouts::Terminal`]).
    #[serde(default)]
    pub terminal: Option<String>,
    /// Claude Code's process — what the window of an agent outside our
    /// terminals is found by ([`Whereabouts::Process`]); 0 for a
    /// photograph's, which has none.
    #[serde(default)]
    pub pid: u32,
}

/// Where a click on an agent takes the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Whereabouts {
    /// One of our terminals, by its id: the window that holds it, or the
    /// terminal's group restored in one.
    Terminal(String),
    /// A process outside our terminals — a Ghostty, an editor's terminal —
    /// whose window is the one it, or a process above it, opened.
    Process(u32),
}

impl AgentSeen {
    /// Where it can be shown; `None` for one with nowhere to be found (a
    /// photograph's, which has no process).
    #[must_use]
    pub fn whereabouts(&self) -> Option<Whereabouts> {
        match (&self.terminal, self.pid) {
            (Some(terminal), _) => Some(Whereabouts::Terminal(terminal.clone())),
            (None, 0) => None,
            (None, pid) => Some(Whereabouts::Process(pid)),
        }
    }

    /// Whether it runs outside our terminals: the one whose end no event
    /// may tell — a process killed with no `SessionEnd`, in a terminal that
    /// writes nothing — and so the one the module's liveness clock is for.
    #[must_use]
    pub const fn outside(&self) -> bool {
        self.terminal.is_none()
    }
}

/// THE AGENTS AT WORK NOW, per account — THE ONE SEAM their reading comes in
/// by, read the same by the module, the accounts' view and a click.
///
/// Claude Code tells its state twice: the hook the distro installs writes a
/// record per session ([`sessions_folder`]), and inside one of our terminals
/// the program's own status (OSC 7501) reaches the terminal's holder, which
/// keeps it in its state file ([`terminals_folder`]). One reader merges the
/// two (`ronoctua_llm::agents::merged`); this hands it what the folders hold
/// now, a process judged by `/proc` (its pid AND its start: a pid is handed
/// on) and a terminal held while its holder's file is there.
#[must_use]
pub fn agents_now(accounts: &[Account]) -> Vec<AgentSeen> {
    let Some(sessions) = sessions_folder() else {
        return Vec::new();
    };
    agents_in(
        accounts,
        &sessions,
        terminals_folder().as_deref(),
        ronoctua_llm::Roots::of_env().home.as_deref(),
        &ronoctua_llm::sessions::running,
    )
}

/// [`agents_now`], over given folders, a given home and a given answer to
/// "is this pid still the process that started then" — what a test plants.
#[must_use]
pub fn agents_in(
    accounts: &[Account],
    sessions: &Path,
    terminals: Option<&Path>,
    home: Option<&Path>,
    process: &dyn Fn(u32, u64) -> bool,
) -> Vec<AgentSeen> {
    let records = ronoctua_llm::sessions::read(sessions);
    // NO RECORD, NO AGENT: the terminals' files are not read for nothing.
    if records.is_empty() {
        return Vec::new();
    }
    let states = terminals
        .map(|t| Registry::at(t).states())
        .unwrap_or_default();
    let programs: Vec<ronoctua_llm::agents::Program> = states
        .iter()
        .flat_map(|state| {
            state
                .statuses
                .iter()
                .map(move |r| ronoctua_llm::agents::Program {
                    terminal: state.id.to_string(),
                    // THE PROGRAM ITSELF has no id: the record is the
                    // session's own, not a task of it.
                    id: (!r.id.is_empty()).then(|| r.id.clone()),
                    app: r.app.clone(),
                    state: program_state(r.state).to_owned(),
                })
        })
        .collect();
    let machine = Here {
        process,
        held: states.iter().map(|s| s.id.to_string()).collect(),
    };
    ronoctua_llm::agents::merged(accounts, &records, &programs, &machine)
        .into_iter()
        .map(|a| AgentSeen {
            account: a.account,
            session: a.session,
            folder: shown_folder(&a.folder, home),
            doing: Doing::of(a.status),
            since: i64::try_from(a.started_at_ns / 1_000_000_000).unwrap_or(i64::MAX),
            terminal: a.terminal,
            pid: a.pid,
        })
        .collect()
}

/// OSC 7501's own word for a state, as the terminals keep it — the words
/// the agents' reader knows (`ronoctua_llm::agents::status_of_program`).
const fn program_state(state: ProgramState) -> &'static str {
    match state {
        ProgramState::Idle => "idle",
        ProgramState::Working => "working",
        ProgramState::Done => "done",
        ProgramState::Blocked => "blocked",
        ProgramState::Error => "error",
    }
}

/// What the agents' reader asks of the machine: a process by the answer it
/// is handed, a terminal by its holder's file.
struct Here<'a> {
    process: &'a dyn Fn(u32, u64) -> bool,
    held: BTreeSet<String>,
}

impl ronoctua_llm::agents::Alive for Here<'_> {
    fn process(&self, pid: u32, start_ticks: u64) -> bool {
        (self.process)(pid, start_ticks)
    }

    fn terminal(&self, id: &str) -> bool {
        self.held.contains(id)
    }
}

/// A folder as the person reads it: under their home, `~/…` — an agent's
/// label, drawn as data.
#[must_use]
pub fn shown_folder(folder: &Path, home: Option<&Path>) -> String {
    // JOINED, not formatted: a separator spelled here would be a path of
    // the module's own, which the module's check refuses.
    match home.and_then(|h| folder.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => Path::new("~").join(rest).display().to_string(),
        None => folder.display().to_string(),
    }
}

/// THE AGENT A CLICK NAMES, read NOW by the reader the marks are made by
/// ([`agents_now`]) — so where a click takes the person is where the agent
/// is when it is clicked, not where it was when the mark was drawn. `None`
/// when it has ended, or its account is not listed (it would not have been
/// drawn).
#[must_use]
pub fn agent_of(session: &str) -> Option<AgentSeen> {
    // A SESSION NO MARK COULD HAND — not the word [`marks_for`] draws an
    // agent by — is no agent: nothing is read for it, the folders nor `/proc`.
    if !key_is_well_formed(session) {
        return None;
    }
    let accounts = accounts_now()?;
    agents_now(&accounts)
        .into_iter()
        .find(|a| a.session == session)
}

/// Where the hook keeps the sessions' records — this instance's
/// (`ronoctua_llm::sessions::folder`): what the module watches, and the view.
#[must_use]
pub fn sessions_folder() -> Option<PathBuf> {
    ronoctua_llm::sessions::folder()
}

/// Where our terminals' holders keep their state files — this instance's
/// (`ronoctua_terminals::registry`): what the module watches, and the view.
#[must_use]
pub fn terminals_folder() -> Option<PathBuf> {
    Registry::of_session().map(|r| r.dir().to_path_buf())
}

/// Makes the two folders the agents are read from, private (`0700`), before
/// anything watches them: a folder watched from inside is heard only once it
/// is there, and the hook and the holders write into what they find.
///
/// # Errors
///
/// The machine's words, when one cannot be made — or something that is not
/// a folder is in its place.
pub fn make_agents_folders(sessions: &Path, terminals: &Path) -> std::io::Result<()> {
    ronoctua_llm::sessions::make_folder(sessions)?;
    Registry::at(terminals).ensure()
}

/// Whether any agent the sets draw — standing in one, or counted in its
/// `agent-more` — runs outside our terminals ([`AgentSeen::outside`]): what
/// the module's liveness clock is armed for, and only then.
#[must_use]
pub fn outside_is_drawn(accounts: &[Seen], agents: &[AgentSeen]) -> bool {
    let marked: BTreeSet<&str> = accounts
        .iter()
        .take(marked_most())
        .filter(|s| mark_of(s.agent).is_some())
        .map(|s| s.id.as_str())
        .collect();
    agents.iter().any(|a| {
        a.outside() && key_is_well_formed(&a.session) && marked.contains(a.account.as_str())
    })
}

/// The places agents keep in their account's set: the lowest free one when
/// an agent is first seen, kept for as long as it lives — so a glyph never
/// changes its agent while the person looks, and a new one never pushes an
/// old one along.
///
/// An agent first seen with every place taken has none until one frees; the
/// set then draws the count of the hidden ones (`agent-more`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Places(BTreeMap<(String, String), u8>);

impl Places {
    /// Keeps the places of the agents still here, frees the rest, and gives
    /// each agent without one the lowest free place of its account — those
    /// first seen together by when each was first seen, then by session, so
    /// one reading gives one answer.
    ///
    /// AN AGENT THAT IS NEVER DRAWN TAKES NO PLACE: one whose session is not
    /// a key (`key_is_well_formed`) is left out of its set ([`marks_for`]),
    /// and holding a place it would leave a hole there and push a drawn one
    /// into the count.
    pub fn settle(&mut self, agents: &[AgentSeen]) {
        let agents: Vec<&AgentSeen> = agents
            .iter()
            .filter(|a| key_is_well_formed(&a.session))
            .collect();
        let here: BTreeSet<(&str, &str)> = agents
            .iter()
            .map(|a| (a.account.as_str(), a.session.as_str()))
            .collect();
        self.0
            .retain(|(account, session), _| here.contains(&(account.as_str(), session.as_str())));
        let mut new: Vec<&AgentSeen> = agents
            .iter()
            .copied()
            .filter(|a| !self.0.contains_key(&(a.account.clone(), a.session.clone())))
            .collect();
        new.sort_by(|a, b| (a.since, &a.session).cmp(&(b.since, &b.session)));
        for a in new {
            let key = (a.account.clone(), a.session.clone());
            if self.0.contains_key(&key) {
                continue;
            }
            let taken: BTreeSet<u8> = self
                .0
                .iter()
                .filter(|((account, _), _)| *account == a.account)
                .map(|(_, place)| *place)
                .collect();
            if let Some(free) = (1..=places_most()).find(|p| !taken.contains(p)) {
                self.0.insert(key, free);
            }
        }
    }

    /// The place an agent keeps, or `None` while every place was taken.
    #[must_use]
    pub fn of(&self, agent: &AgentSeen) -> Option<u8> {
        self.0
            .get(&(agent.account.clone(), agent.session.clone()))
            .copied()
    }
}

/// How loud a tone is, for the count of the hidden agents: it wears the
/// loudest of theirs, so one waiting for the person is still seen.
const fn loudness(tone: Tone) -> u8 {
    match tone {
        Tone::Quiet => 0,
        Tone::Working => 1,
        Tone::Watching => 2,
        Tone::Accent => 3,
        Tone::Warning => 4,
        Tone::Danger => 5,
    }
}

/// The marks: one SET per account, by its place in the list — its lead,
/// coloured by the person's lines (an API key's by its credits,
/// [`account_tone`]), then its agents by the places they keep, then the
/// count of the ones that did not fit.
///
/// A SET HOLDS NINE AGENTS, and a tenth takes the ninth place with it: the
/// first eight are drawn and the count stands where the ninth would, in the
/// loudest tone of the agents it hides. An agent whose session is not a word
/// a command line carries whole (`key_is_well_formed`) is left out: its click
/// could not say which agent it is.
#[must_use]
pub fn marks_for(
    accounts: &[Seen],
    agents: &[AgentSeen],
    places: &Places,
    lines: Lines,
    now: Moment,
) -> Vec<Mark> {
    let most = places_most();
    let mut out = Vec::new();
    for (n, seen) in accounts.iter().take(marked_most()).enumerate() {
        let (Some(lead), Ok(set)) = (mark_of(seen.agent), u8::try_from(n + 1)) else {
            continue;
        };
        out.push(Mark {
            id: lead.in_set(set, Some(seen.name.clone())).opening(OPENS),
            tone: account_tone(seen, lines, now),
            count: 0,
            act: None,
        });
        let mut mine: Vec<(Option<u8>, &AgentSeen)> = agents
            .iter()
            .filter(|a| a.account == seen.id && key_is_well_formed(&a.session))
            .map(|a| (places.of(a), a))
            .collect();
        mine.sort_by_key(|(place, _)| place.unwrap_or(u8::MAX));
        let crowded = mine.iter().any(|(place, _)| place.is_none());
        let (shown, hidden): (Vec<_>, Vec<_>) = mine
            .into_iter()
            .partition(|(place, _)| place.is_some_and(|p| !crowded || p < most));
        for (place, a) in shown {
            let Some(place) = place else { continue };
            out.push(Mark {
                id: a
                    .doing
                    .mark()
                    .in_set(set, Some(a.folder.clone()))
                    .numbered(place, None)
                    .keyed(a.session.clone())
                    .opening(SHOWS),
                tone: a.doing.tone(),
                count: 0,
                act: None,
            });
        }
        if let Some(loudest) = hidden
            .iter()
            .map(|(_, a)| a.doing.tone())
            .max_by_key(|t| loudness(*t))
        {
            out.push(Mark {
                id: super::mark::AGENT_MORE.in_set(set, None).opening(OPENS),
                tone: loudest,
                count: u32::try_from(hidden.len()).unwrap_or(u32::MAX),
                act: None,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Noon on 2026-10-05, UTC.
    const NOON: Moment = 1_791_201_600;

    fn window(used_percent: f32, resets_at_s: i64) -> Window {
        Window {
            used_percent,
            resets_at_s,
        }
    }

    fn seen(id: &str, agent: Agent, name: &str, windows: Vec<Window>) -> Seen {
        Seen {
            id: id.to_owned(),
            agent,
            name: name.to_owned(),
            windows,
            out_of_credits_since: None,
        }
    }

    /// An agent of `account`, its session made of `n`, first seen at `n`.
    fn agent(account: &str, n: u32, doing: Doing) -> AgentSeen {
        AgentSeen {
            account: account.to_owned(),
            session: format!("0000{n:04x}-7b44-4d1a-9c1e-0b5d2e6f8a90"),
            folder: format!("~/src/p{n}"),
            doing,
            since: NOON + i64::from(n),
            terminal: None,
            pid: 4000 + n,
        }
    }

    /// The marks with these agents, given their places as the module does.
    fn with(accounts: &[Seen], agents: &[AgentSeen]) -> Vec<Mark> {
        let mut places = Places::default();
        places.settle(agents);
        marks_for(accounts, agents, &places, Lines::default(), NOON)
    }

    /// The marks of one set.
    fn in_set(marks: &[Mark], set: u8) -> Vec<&Mark> {
        marks.iter().filter(|m| m.id.set == set).collect()
    }

    /// Every mark, as the core reads it: what this module publishes must be
    /// shown whole (`Manifest::refusal_of`), in the order it is published.
    fn all_shown(marks: &[Mark]) {
        let shown: Vec<&Mark> = marks.iter().collect();
        for (n, mark) in marks.iter().enumerate() {
            assert_eq!(
                super::super::MANIFEST.refusal_of(mark, &shown[..n]),
                None,
                "{mark:?}"
            );
        }
    }

    /// FAILS IF: an account does not lead a set of its own — its set the
    /// account's place in the list, its label the person's word, its click
    /// the accounts' view, no instance — the lead is coloured by a window
    /// that already reset, or a tenth account gets a set the modeline has no
    /// room for.
    #[test]
    fn every_account_leads_a_set_by_its_place() {
        let mut list = vec![
            seen(
                "claude-1",
                Agent::Claude,
                "Work",
                vec![window(96.0, NOON + 60)],
            ),
            seen(
                "codex-1",
                Agent::Codex,
                "Home",
                vec![window(99.0, NOON - 60), window(81.0, NOON + 60)],
            ),
        ];
        for n in 2..=10 {
            list.push(seen(
                &format!("codex-{n}"),
                Agent::Codex,
                &format!("C{n}"),
                Vec::new(),
            ));
        }
        let marks = with(&list, &[]);
        assert_eq!(marks.len(), marked_most(), "a set past the manifest's");
        assert!(marked_most() <= 9, "nine sets at most");
        let (first, second) = (&marks[0], &marks[1]);
        assert_eq!(
            (first.id.set, first.id.instance, first.id.label.as_deref()),
            (1, 0, Some("Work"))
        );
        assert!(first.id.leads, "the account's mark is not its set's lead");
        assert_eq!(first.id.opens.as_deref(), Some(OPENS));
        assert_eq!(first.tone, Tone::Danger);
        // THE RESET WINDOW IS GONE: 99 % that reset a minute ago, and 81 %
        // that has not, is a warning.
        assert_eq!((second.id.set, second.tone), (2, Tone::Warning));
        assert_eq!(
            second.id.name,
            mark_of(Agent::Codex).expect("codex has a mark").name
        );
        // AND A WINDOW NOBODY KNOWS is quiet: the lead is there, and says
        // nothing yet.
        assert_eq!(marks[2].tone, Tone::Quiet);
        all_shown(&marks);
    }

    /// FAILS IF: an agent is drawn outside its account's set, in any order
    /// but the places it keeps, with a click that does not name its session,
    /// in a tone that is not its state's — or an agent of an account the list
    /// does not have, or of one past the ninth, is drawn at all.
    #[test]
    fn the_agents_stand_in_their_accounts_set() {
        let mut list = vec![
            seen("claude-1", Agent::Claude, "Work", Vec::new()),
            seen("codex-1", Agent::Codex, "Home", Vec::new()),
            seen("claude-2", Agent::Claude, "Side", Vec::new()),
        ];
        for n in 4..=10 {
            list.push(seen(&format!("x-{n}"), Agent::Codex, "x", Vec::new()));
        }
        let agents = [
            agent("claude-2", 1, Doing::Waiting),
            agent("claude-1", 2, Doing::Working),
            agent("claude-2", 3, Doing::Done),
            agent("claude-2", 4, Doing::Failed),
            agent("nobody", 5, Doing::Working),
            agent("x-10", 6, Doing::Working),
        ];
        let marks = with(&list, &agents);
        all_shown(&marks);
        let third = in_set(&marks, 3);
        assert_eq!(
            third
                .iter()
                .map(|m| (m.id.name.as_ref(), m.id.instance, m.tone))
                .collect::<Vec<_>>(),
            [
                ("claude", 0, Tone::Quiet),
                ("agent-waiting", 1, Tone::Warning),
                ("agent-done", 2, Tone::Quiet),
                ("agent-failed", 3, Tone::Danger),
            ]
        );
        let waiting = third[1];
        assert_eq!(waiting.id.key.as_deref(), Some(agents[0].session.as_str()));
        assert_eq!(waiting.id.opens.as_deref(), Some(SHOWS));
        assert_eq!(waiting.id.label.as_deref(), Some("~/src/p1"));
        // EVERY AGENT IS THE SAME ROBOT'S HEAD, whatever it is doing: the
        // tone says the state, and the lead keeps the account's own glyph.
        assert!(third[2..].iter().all(|m| m.id.glyph == third[1].id.glyph));
        assert_ne!(third[1].id.glyph, third[0].id.glyph);
        assert_eq!(
            in_set(&marks, 1)
                .iter()
                .map(|m| (m.id.name.as_ref(), m.tone))
                .collect::<Vec<_>>(),
            [("claude", Tone::Quiet), ("agent-working", Tone::Working)]
        );
        assert_eq!(in_set(&marks, 2).len(), 1, "Codex's set has its lead alone");
        assert!(
            marks
                .iter()
                .all(|m| m.id.key.as_deref() != Some(agents[4].session.as_str())
                    && m.id.key.as_deref() != Some(agents[5].session.as_str())),
            "an agent of no listed account, or of the tenth, was drawn"
        );
        // A SESSION THAT IS NOT A WORD cannot be handed to its command, and
        // its agent is left out rather than drawn with a click to nowhere.
        let odd = AgentSeen {
            session: "Not A Word".to_owned(),
            ..agent("claude-1", 9, Doing::Working)
        };
        assert_eq!(in_set(&with(&list, &[odd]), 1).len(), 1);
    }

    /// FAILS IF: a glyph changes its agent while the person looks — an agent
    /// that ends moves the others along, or a new one is not given the
    /// lowest place free — or agents first seen together are placed by
    /// anything but when each was first seen.
    #[test]
    fn an_agent_keeps_its_place() {
        let (a, b, c, d) = (
            agent("claude-1", 1, Doing::Working),
            agent("claude-1", 2, Doing::Working),
            agent("claude-1", 3, Doing::Working),
            agent("claude-1", 4, Doing::Working),
        );
        let mut places = Places::default();
        // HANDED OUT OF ORDER, placed by when each was first seen.
        places.settle(&[c.clone(), a.clone(), b.clone()]);
        assert_eq!(
            [&a, &b, &c].map(|x| places.of(x)),
            [Some(1), Some(2), Some(3)]
        );
        places.settle(&[a.clone(), c.clone()]);
        assert_eq!([&a, &b, &c].map(|x| places.of(x)), [Some(1), None, Some(3)]);
        places.settle(&[a.clone(), c.clone(), d.clone()]);
        assert_eq!(places.of(&d), Some(2), "the lowest free place");
        assert_eq!(places.of(&c), Some(3), "an old agent moved");
        // AN AGENT OF ANOTHER ACCOUNT has places of its own.
        let other = agent("codex-1", 5, Doing::Working);
        places.settle(&[a, c, d, other.clone()]);
        assert_eq!(places.of(&other), Some(1));
    }

    /// FAILS IF: a tenth agent is drawn as a tenth glyph, the count of the
    /// hidden ones is not where the ninth would be — after the first eight,
    /// counting the ninth place's agent and the ones with no place — or it
    /// does not wear the loudest tone of the agents it hides, which is how
    /// one waiting for the person is still seen; or a set of nine draws one —
    /// counting an agent that is never drawn, which took a place.
    #[test]
    fn the_tenth_agent_becomes_more() {
        let list = [seen("claude-1", Agent::Claude, "Work", Vec::new())];
        let mut agents: Vec<AgentSeen> =
            (1..=9).map(|n| agent("claude-1", n, Doing::Done)).collect();
        let nine = with(&list, &agents);
        assert_eq!(nine.len(), 10, "the lead and nine agents");
        assert!(nine.iter().all(|m| m.id.name != "agent-more"));
        all_shown(&nine);
        // THE TENTH, waiting for the person: past the ninth place.
        agents.push(agent("claude-1", 10, Doing::Waiting));
        let ten = with(&list, &agents);
        all_shown(&ten);
        let names: Vec<&str> = ten.iter().map(|m| m.id.name.as_ref()).collect();
        assert_eq!(names.len(), 10, "{names:?}");
        assert_eq!(names[0], "claude");
        assert!(names[1..9].iter().all(|n| *n == "agent-done"), "{names:?}");
        let more = &ten[9];
        assert_eq!(more.id.name, "agent-more");
        assert_eq!((more.count, more.id.instance), (2, 0));
        assert_eq!(more.tone, Tone::Warning, "the waiting one went unseen");
        assert_eq!(more.id.opens.as_deref(), Some(OPENS));
        assert_eq!(more.id.set, 1);
        let places: Vec<u8> = ten[1..9].iter().map(|m| m.id.instance).collect();
        assert_eq!(places, (1..=8).collect::<Vec<u8>>());
        // ALL OF THEM DONE, the count is quiet — and still drawn, at rest.
        let done: Vec<AgentSeen> = (1..=12)
            .map(|n| agent("claude-1", n, Doing::Done))
            .collect();
        let more = with(&list, &done).pop().expect("a count");
        assert_eq!((more.count, more.tone), (4, Tone::Quiet));
        // AN AGENT NEVER DRAWN — its session no key — takes no place: nine
        // drawn ones and it are nine glyphs, and no count.
        let mut nine_and_odd: Vec<AgentSeen> =
            (1..=9).map(|n| agent("claude-1", n, Doing::Done)).collect();
        nine_and_odd.insert(
            0,
            AgentSeen {
                session: "Not A Word".to_owned(),
                since: 0,
                ..agent("claude-1", 99, Doing::Waiting)
            },
        );
        let drawn = with(&list, &nine_and_odd);
        assert_eq!(drawn.len(), 10, "the odd one took a place");
        assert!(drawn.iter().all(|m| m.id.name != "agent-more"));
    }

    /// FAILS IF: an API key's account has no lead of its own, its lead is
    /// not danger while Claude Code says its credits ran out (or stays danger
    /// once they are back), its agents put out the danger the lead says, or
    /// a window — which a key does not have — colours it.
    #[test]
    fn an_api_keys_lead_is_danger_while_its_credits_are_out() {
        let mut api = seen(
            "claude-api-1",
            Agent::ClaudeApi,
            "Console",
            vec![window(99.0, NOON + 60)],
        );
        let quiet = with(std::slice::from_ref(&api), &[]);
        assert_eq!(quiet.len(), 1, "an API key's account has no lead");
        assert_eq!(
            quiet[0].id.name,
            mark_of(Agent::ClaudeApi)
                .expect("claude-api has a mark")
                .name
        );
        assert_ne!(
            quiet[0].id.name,
            mark_of(Agent::Claude).expect("claude").name
        );
        assert_eq!(quiet[0].tone, Tone::Quiet, "a window coloured a key");
        api.out_of_credits_since = Some(NOON - 600);
        let working = [agent("claude-api-1", 1, Doing::Working)];
        let out = with(std::slice::from_ref(&api), &working);
        assert_eq!(out[0].tone, Tone::Danger, "the agents put out the danger");
        assert_eq!(out[1].tone, Tone::Working);
        all_shown(&out);
        assert_eq!(credits_tone(Some(NOON)), Tone::Danger);
        assert_eq!(credits_tone(None), Tone::Quiet);
        // AND A SUBSCRIPTION is coloured by its windows whatever else is
        // said of it.
        let mut work = seen(
            "claude-1",
            Agent::Claude,
            "Work",
            vec![window(10.0, NOON + 60)],
        );
        work.out_of_credits_since = Some(NOON);
        assert_eq!(with(std::slice::from_ref(&work), &[])[0].tone, Tone::Quiet);
    }

    /// FAILS IF: a photograph's planted agents do not read as the module
    /// reads them, or a file of anything else is taken for a list of them.
    #[test]
    fn a_photographs_agents_are_read_as_the_module_reads_them() {
        let read = planted(
            r#"[{"account": "claude-1", "session": "abc-1", "folder": "~/x", "doing": "waiting"}]"#,
        )
        .expect("a list of agents");
        assert_eq!(
            read,
            [AgentSeen {
                account: "claude-1".to_owned(),
                session: "abc-1".to_owned(),
                folder: "~/x".to_owned(),
                doing: Doing::Waiting,
                since: 0,
                terminal: None,
                pid: 0,
            }]
        );
        // A PLANTED AGENT HAS NO PROCESS: a click on it finds nowhere to go.
        assert_eq!(read[0].whereabouts(), None);
        assert!(planted(r#"[{"account": "claude-1"}]"#).is_err());
        assert!(
            planted(r#"[{"account": "a", "session": "b", "folder": "c", "doing": "napping"}]"#)
                .is_err()
        );
    }

    /// A scratch folder of this test's own.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-brain-agent-accounts-shared-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn listed(id: &str, kind: Agent, dir: &str) -> Account {
        Account {
            id: id.to_owned(),
            kind,
            name: id.to_owned(),
            dir: dir.into(),
            default: false,
        }
    }

    /// The hook's record of session `n`, its process `100 + n`, written
    /// where the hook writes it.
    fn record(folder: &Path, n: u32, account_dir: &str, terminal: Option<&str>, status: Status) {
        let session = ronoctua_llm::sessions::Session {
            format: ronoctua_llm::sessions::FORMAT,
            session: format!("5f0c1e2a-0000-4000-8000-{n:012x}"),
            account_dir: account_dir.into(),
            pid: 100 + n,
            pid_start_ticks: 7,
            terminal: terminal.map(str::to_owned),
            folder: format!("/home/p/src/p{n}").into(),
            transcript: None,
            status,
            said_at_ns: 1,
            started_at_ns: 3_000_000_000 + u64::from(n),
            ended: false,
        };
        std::fs::write(
            folder.join(format!("{}.json", session.session)),
            serde_json::to_vec(&session).expect("a record"),
        )
        .expect("written");
    }

    /// One of our terminals' state, its programs saying these of themselves.
    fn terminal(folder: &Path, id: &str, statuses: &[(&str, &str, ProgramState)]) {
        let state = ronoctua_terminals::state::TerminalState {
            protocol: ronoctua_terminals::PROTOCOL,
            id: id.parse().expect("an id"),
            group: "fedcba9876543210".parse().expect("an id"),
            order: 0,
            viewer: None,
            front: false,
            seen: false,
            title: String::new(),
            program: "zsh".to_owned(),
            cwd: None,
            started_s: 5,
            alone_since_s: None,
            attention: ronoctua_terminals::state::Attention::default(),
            notice: None,
            command: None,
            front_asked: 0,
            ended: None,
            statuses: statuses
                .iter()
                .map(
                    |(record, app, state)| ronoctua_terminals::state::StatusRecord {
                        id: (*record).to_owned(),
                        app: (*app).to_owned(),
                        state: *state,
                        kind: None,
                        title: String::new(),
                        msg: String::new(),
                        percent: None,
                        at_ms: 1,
                    },
                )
                .collect(),
            progress: None,
        };
        Registry::at(folder)
            .write(&state)
            .expect("the state written");
    }

    /// FAILS IF: an agent is not read as the hook and our terminals left it
    /// — its account by the folder it runs in, its folder as the person
    /// reads it, the moment it was first heard in seconds, its terminal and
    /// its process carried for a click — the program's own word (OSC 7501)
    /// does not beat the hook's inside our terminal (a task of it, another
    /// program, or its OWN record with no id each told apart), a dead
    /// process is drawn, or an agent whose terminal's holder left no file is.
    #[test]
    fn the_agents_are_read_as_the_hook_and_the_terminals_left_them() {
        const T1: &str = "0123456789abcdef";
        const T2: &str = "fedcba9876543210";
        let root = scratch("read");
        let (sessions, terms) = (root.join("agents"), root.join("terms"));
        make_agents_folders(&sessions, &terms).expect("the folders");
        let accounts = [
            listed("claude-1", Agent::Claude, "/home/p/.claude"),
            listed("codex-1", Agent::Codex, "/home/p/.codex"),
        ];
        // NOTHING YET: no agent, and the terminals' files not read for one.
        assert!(agents_in(&accounts, &sessions, Some(&terms), None, &|_, _| true).is_empty());
        record(&sessions, 1, "/home/p/.claude", Some(T1), Status::Working);
        record(&sessions, 2, "/home/p/.claude", None, Status::Waiting);
        record(&sessions, 3, "/home/p/.claude", None, Status::Working);
        record(&sessions, 4, "/home/p/.claude", Some(T2), Status::Working);
        record(&sessions, 5, "/srv/elsewhere/.claude", None, Status::Done);
        terminal(
            &terms,
            T1,
            &[
                ("task-1", "claude-code", ProgramState::Working),
                ("", "another", ProgramState::Error),
                ("", "claude-code", ProgramState::Blocked),
            ],
        );
        // T2's holder is gone: no file.
        let dead = 103;
        let read = agents_in(
            &accounts,
            &sessions,
            Some(&terms),
            Some(Path::new("/home/p")),
            &|pid, ticks| pid != dead && ticks == 7,
        );
        assert_eq!(
            read.iter()
                .map(|a| (a.session.as_str(), a.doing))
                .collect::<Vec<_>>(),
            [
                ("5f0c1e2a-0000-4000-8000-000000000001", Doing::Waiting),
                ("5f0c1e2a-0000-4000-8000-000000000002", Doing::Waiting),
            ],
            "{read:?}"
        );
        let first = &read[0];
        assert_eq!(first.account, "claude-1");
        assert_eq!(first.folder, "~/src/p1");
        assert_eq!(first.since, 3, "first heard, in seconds");
        assert_eq!(
            first.whereabouts(),
            Some(Whereabouts::Terminal(T1.to_owned()))
        );
        assert_eq!(read[1].whereabouts(), Some(Whereabouts::Process(102)));
        assert!(!first.outside() && read[1].outside());
        // WITH NO HOME TO SHORTEN BY, the folder as it is.
        let bare = agents_in(&accounts, &sessions, Some(&terms), None, &|_, _| true);
        assert!(bare.iter().any(|a| a.folder == "/home/p/src/p1"));
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: the liveness clock's question says an agent outside our
    /// terminals is drawn when none is — one in our terminal, one of an
    /// account past the ninth or of none listed, one whose session is no key
    /// — or says none is while one stands in a set, or is counted in one.
    #[test]
    fn only_a_drawn_agent_outside_our_terminals_wants_the_clock() {
        let mut list = vec![seen("claude-1", Agent::Claude, "Work", Vec::new())];
        for n in 2..=10 {
            list.push(seen(&format!("claude-{n}"), Agent::Claude, "x", Vec::new()));
        }
        let inside = AgentSeen {
            terminal: Some("0123456789abcdef".to_owned()),
            ..agent("claude-1", 1, Doing::Working)
        };
        assert!(!outside_is_drawn(&list, &[]));
        assert!(!outside_is_drawn(&list, std::slice::from_ref(&inside)));
        for undrawn in [
            agent("claude-10", 2, Doing::Working),
            agent("nobody", 3, Doing::Working),
            AgentSeen {
                session: "Not A Word".to_owned(),
                ..agent("claude-1", 4, Doing::Working)
            },
        ] {
            assert!(
                !outside_is_drawn(&list, &[inside.clone(), undrawn.clone()]),
                "{undrawn:?}"
            );
        }
        assert!(outside_is_drawn(
            &list,
            &[inside, agent("claude-2", 5, Doing::Done)]
        ));
        // COUNTED IN `agent-more`, it is drawn too: the count moves when it
        // goes.
        let crowd: Vec<AgentSeen> = (1..=9)
            .map(|n| AgentSeen {
                terminal: Some("0123456789abcdef".to_owned()),
                ..agent("claude-1", n, Doing::Done)
            })
            .chain([agent("claude-1", 10, Doing::Done)])
            .collect();
        assert!(outside_is_drawn(&list, &crowd));
    }

    /// FAILS IF: a folder under the home is not read as the person reads it
    /// — or one beside it (`/home/pa` for `/home/p`) is.
    #[test]
    fn a_folder_reads_from_the_home() {
        let home = Some(Path::new("/home/p"));
        assert_eq!(shown_folder(Path::new("/home/p/src/x"), home), "~/src/x");
        assert_eq!(shown_folder(Path::new("/home/p"), home), "~");
        assert_eq!(shown_folder(Path::new("/home/pa/x"), home), "/home/pa/x");
        assert_eq!(shown_folder(Path::new("/srv/x"), None), "/srv/x");
    }

    /// FAILS IF: a status the reader says is drawn as another state.
    #[test]
    fn every_status_is_the_state_it_means() {
        for (status, doing) in [
            (Status::Working, Doing::Working),
            (Status::Waiting, Doing::Waiting),
            (Status::Done, Doing::Done),
            (Status::Failed, Doing::Failed),
        ] {
            assert_eq!(Doing::of(status), doing);
        }
        assert_eq!(Status::ALL.map(Doing::of), Doing::ALL);
    }
}
