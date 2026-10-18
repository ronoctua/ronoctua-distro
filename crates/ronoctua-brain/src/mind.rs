//! The whole state, as it crosses the socket.
//!
//! # A word about the word
//!
//! In this repository a *buffer* is a `wasm32-wasip2` component with a screen.
//! What is here is not that: [`Mind`] is the STATE, on the wire and in memory,
//! and `buffers/brain/` is the screen that draws it. The two are kept apart in
//! every comment, because "the brain's buffer" would otherwise mean either.
//!
//! The one lock everybody reads it through, and its bell, are the daemon's
//! (`ronoctua_brain_daemon::mind::Shared`): a reader of the socket holds a
//! copy, never the lock.
//!
//! # Why the whole mind goes out on every change
//!
//! A delta needs both sides to agree on what they last saw, and a client that
//! dropped one would draw a wrong answer for the rest of the session with
//! nothing to notice it by. Here that answer is *when will my machine update
//! itself*, which is exactly the kind of wrongness nobody double-checks. The
//! whole thing is a few short records.

use serde::{Deserialize, Serialize};

use crate::moment::Moment;
use crate::prefs::{PartOfDay, Prefs};
use crate::pulse::Pulse;
use crate::task::{Hold, RunRecord};

/// Where a task is right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Standing {
    /// Waiting for its moment.
    #[default]
    Idle,
    /// Its moment came; it is waiting for the one that is running, or for the
    /// countdown to end.
    Queued,
    /// It is happening.
    Running,
}

impl Standing {
    /// The word the screen looks up, as `brain-standing-<word>`.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Queued => "queued",
            Self::Running => "running",
        }
    }
}

/// One step of a task, as the screen draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct StepState {
    /// The step's id — `brain-step-<id>` on screen.
    pub id: String,
    /// What it really runs. DATA: a command line is not translated.
    pub command: String,
    /// Whether it needs root.
    pub needs_root: bool,
    /// Whether it will run.
    pub on: bool,
    /// Whether the distro ships it on.
    ///
    /// The DECLARED fact, and the only one of the two that is not derived —
    /// "changed" is `on != on_by_default`, which anybody holding both can
    /// work out. It travels because the screen wants to say what a step was
    /// changed FROM, and because the host has to know what flipping one means.
    pub on_by_default: bool,
    /// The text key of its name ([`crate::task::Step::name_key`]). Empty until
    /// its module declares one, and then no screen builds a key out of the id.
    #[serde(default)]
    pub name_key: String,
    /// The text key of the line under the cursor. Empty for the same reason.
    #[serde(default)]
    pub does_key: String,
}

/// Something a person can ask of a module by name, as the screen offers it.
///
/// The owned twin of [`crate::module::Action`]: the manifest's declaration is
/// `&'static`, and this is what crosses the socket.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ActionState {
    /// Its id inside the module — what `Ask::Act` hands back.
    pub id: String,
    /// The key of its label: `cmd-<module>-<action>`.
    pub label: String,
    /// Whether it acts on one row of the module's screen.
    pub row: bool,
    /// The key of the question it asks first, when it asks one.
    pub asks: Option<String>,
}

impl From<&crate::module::Action> for ActionState {
    fn from(a: &crate::module::Action) -> Self {
        Self {
            id: a.id.to_owned(),
            label: a.label.to_owned(),
            row: a.row,
            asks: a.asks.map(str::to_owned),
        }
    }
}

/// Something a module published, and whose it is.
///
/// A record and not a pair: `{"module": …, "what": …}` reads in `socat`, and a
/// field added later has somewhere to go.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Posted<T> {
    /// The module's id.
    pub module: String,
    /// What it published.
    pub what: T,
}

/// One thing a module told the footer, numbered so it is said once.
///
/// The footer says an event when its `seq` is past the last one it saw; the
/// first mind a footer sees is its baseline, so what was told before it
/// started is not news. It replaces `relieved`, which was this for one module.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Said {
    /// Counted from the daemon's start; only ever grows.
    pub seq: u64,
    /// The event's tag: `<module>-<event>`.
    pub tag: crate::module::Word,
    /// The sentence's values, formatted by the footer.
    pub args: crate::module::Args,
}

/// How many told events the mind keeps: a ring, the oldest dropped first.
///
/// Enough to cover every module telling something between two minds; a footer
/// that missed more than this was not listening, and its baseline is the next
/// mind.
pub const SAID_KEPT: usize = 32;

/// One task, as everybody draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TaskState {
    /// The task's id. `brain-task-<id>` names it on screen.
    pub id: String,
    /// Whether it repeats or happens once.
    pub kind: crate::task::Kind,
    /// Where it is.
    pub standing: Standing,
    /// Whether the person left it switched on.
    pub enabled: bool,
    /// Days between runs, as resolved for THIS task.
    ///
    /// Resolved here, and not left in the preferences map, because the screen
    /// has no map type to look one up in — and because a screen that had to
    /// join two lists to draw a row is a screen that draws the wrong row the
    /// first time one of them is filtered.
    pub interval_days: u16,
    /// Which part of the day it prefers.
    pub part_of_day: PartOfDay,
    /// The next moment it will be considered, in epoch seconds. `0` when there
    /// is none — it is switched off, or it only runs when somebody asks.
    pub next: Moment,
    /// What stopped it the last time its moment came, if anything.
    ///
    /// The screen draws it as *"waiting: the RAM is busy"*. A scheduler that
    /// silently does nothing is one nobody trusts.
    pub hold: Option<Hold>,
    /// Why it is not due at all, as a WORD — `off`, `no-schedule`. Empty when
    /// it is due or scheduled.
    pub why_not: String,
    /// The last run, whatever it said.
    pub last: Option<RunRecord>,
    /// The last few runs of THIS task, newest first.
    ///
    /// `last` is the first of them. Both are here for the reason the wire's own
    /// record gives: a row on the list draws one and a page draws all of them,
    /// and deriving either from the other would put a length check in front of
    /// every reader.
    #[serde(default)]
    pub history: Vec<RunRecord>,
    /// What it runs, in order.
    pub steps: Vec<StepState>,
    /// Whether it keeps a calendar (`Schedule::calendar`). A page draws the
    /// cadence and the window only when it does: a relief that comes due when
    /// the disk is short has neither, and drawing "every 7 days, afternoon"
    /// about it would be a lie. `true` in a mind that did not say.
    #[serde(default = "calendar_unless_said")]
    pub calendar: bool,
    /// The text key of its name (`Manifest::name_key`), so no screen builds a
    /// key out of the id. Empty only in a mind from before modules.
    #[serde(default)]
    pub name_key: String,
    /// The text key of the line under the cursor (`Manifest::does_key`).
    #[serde(default)]
    pub does_key: String,
    /// The text key of [`TaskState::why_not`]: `brain-why-<word>` for a core
    /// word, `<module>-why-<word>` for one of the module's. Empty when there
    /// is no reason.
    #[serde(default)]
    pub why_key: String,
    /// The generation of the module's screen (`Face::screen`): when it moves,
    /// a page that shows the screen asks for it again (`Ask::Screen`).
    #[serde(default)]
    pub screen_generation: u64,
    /// What a person can ask of it by name, in the manifest's order.
    #[serde(default)]
    pub actions: Vec<ActionState>,
}

/// What `calendar` is when a mind did not say: every task before modules
/// kept one.
const fn calendar_unless_said() -> bool {
    true
}

/// The warning that something is about to happen.
///
/// # Why the brain sends it ONCE and never a countdown
///
/// Because the footer already beats a one-second clock, and a daemon that
/// pushed a number every second would wake three processes to redraw a digit.
/// What crosses is the DEADLINE; whoever draws computes `deadline - now` on the
/// clock it already has, and reuses the countdown glyph the notification band
/// already draws with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Announcement {
    /// Which task is about to run.
    pub task: String,
    /// How long the whole warning lasts, in seconds — the denominator of the
    /// glyph, so it does not have to be inferred from a first sighting.
    pub total_seconds: u16,
    /// When it starts, in epoch seconds.
    pub deadline: Moment,
    /// Why it is running: `due` when its moment came, `asked` when somebody
    /// pressed a key. A word, so the screen says which of the two it is.
    pub why: String,
}

/// Everything there is to know, all at once.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Mind {
    /// The daemon's clock, in epoch seconds.
    ///
    /// It crosses because the screen has none: a component has no clock
    /// (invariant 11), so *"in three hours"* cannot be worked out on the other
    /// side without this number.
    pub now: Moment,
    /// How busy the machine is, as the gate last read it.
    pub pulse: Pulse,
    /// Every task the brain knows.
    pub tasks: Vec<TaskState>,
    /// What is waiting, in the order it will run.
    pub queue: Vec<String>,
    /// What is running, by id. Empty when nothing is.
    pub running: String,
    /// The warning in flight, if any.
    pub announcement: Option<Announcement>,
    /// The person's preferences, whole — what they chose, and only that: the
    /// rows carry what each module resolves them to (`Prefs::of_module`).
    pub prefs: Prefs,
    /// The task whose NEXT run is a rehearsal, if somebody asked for one.
    ///
    /// It lives in the state and not on the runner because the two are
    /// different threads: the ask arrives on a socket thread and the run
    /// happens on the runner's. One name and not a set — at most one thing
    /// runs, so at most one run can be a dry one — and it is cleared when the
    /// run starts, so a rehearsal asked for today cannot quietly make next
    /// week's scheduled run a dry one.
    pub rehearsing: Option<String>,
    /// A task somebody asked to put off, waiting for the runner to write it
    /// down.
    ///
    /// It crosses the threads the same way [`Mind::rehearsing`] does, and for
    /// the same reason: the ask arrives on a socket thread and the journal
    /// belongs to the runner's. It does NOT cross to the screen — by the time
    /// anything is drawn the runner has already turned it into a record and a
    /// new moment, and a field showing a decision in flight would be a field
    /// that is empty every time anybody looks.
    pub postponed: Option<String>,
    /// Everything the brain has done, OLDEST FIRST, as the journal keeps it.
    ///
    /// # Why the mind carries it at all
    ///
    /// Because the whole mind is what crosses the socket, and the home page
    /// shows one list of everything that has happened. The alternative was a
    /// second `Ask` for the journal, which would mean two answers that can
    /// disagree about the same moment — the argument this file's header makes
    /// about deltas, one level up.
    ///
    /// It is a COPY and not the journal itself: the journal belongs to the
    /// runner's thread and this is read by every screen. The ceiling is
    /// [`crate::journal::ACTIVITY`], so the copy is bounded however long the
    /// machine has been running.
    pub journal: Vec<crate::task::RunRecord>,
    /// Every notice every module raised, in the order the strip ranks them:
    /// something the brain wants said, without anybody having asked.
    ///
    /// The strip under the minibuffer shows the first, on the line the
    /// countdown already uses: one line is still one line (invariant 14), and
    /// the list is what the `+N` beside it opens.
    pub notices: Vec<Posted<crate::module::Notice>>,
    /// Every mark every module put on the modeline. Empty at rest.
    pub marks: Vec<Posted<crate::module::Mark>>,
    /// What modules told the footer, oldest first: a ring of at most
    /// [`SAID_KEPT`]. It replaced a field that carried this for one module
    /// alone — the disk's relief, which now tells its two events here like any
    /// other.
    pub events: Vec<Said>,
    /// Every one-shot row every module has (`Kind::Once`).
    pub items: Vec<Posted<crate::module::Item>>,
}

impl Mind {
    /// One task's row, by id.
    #[must_use]
    pub fn task(&self, id: &str) -> Option<&TaskState> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// One task's row, to change.
    pub fn task_mut(&mut self, id: &str) -> Option<&mut TaskState> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }
}
