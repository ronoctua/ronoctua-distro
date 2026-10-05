//! THE CONTRACT: what a brain module is, and what it has to answer.
//!
//! # Why a contract and not a list of tasks
//!
//! Because a task was one shape — a calendar, a unit and a report — and the
//! brain grew things that are not that shape: a watch on the journal that runs
//! nothing, a reminder that rings once and is gone, a mark on the modeline that
//! is there only while the disk is nearly full. Each of those used to cost a
//! field on the mind, a word in the core texts and a branch in the shell that
//! named it. A module answers the same few questions instead, and the core, the
//! footer and the buffer draw every module the same way without naming any of
//! them.
//!
//! A module lives in `crates/ronoctua-brain/modules/<id>/`: `manifest.nix` (what
//! it is, its schedule, its marks, notices, actions and events), `module.rs`
//! (`pub fn new(birth: &crate::Birth) -> impl Module + use<>`, which may
//! `mod x;` siblings beside it), its NixOS module, its texts, its checks and its
//! `why.md`. The generator turns the manifest into a `manifest.rs` that is
//! included beside `module.rs`: `ID`, `MANIFEST`, and the `key`, `mark`,
//! `notice`, `action`, `event`, `why` and `unit` constants this file's types
//! are built from. `tests/modules/pilot-sample/` is the hand-written sample of both.
//!
//! # Three faces, all optional
//!
//! - [`Scheduled`] — the old `Task`: a run the runner queues, gates, announces
//!   and journals. It receives the person's preferences ALREADY RESOLVED over the
//!   module's own defaults ([`Manifest::defaults`]), so a module cannot read a
//!   default the screen does not show — the defect `Prefs::mine` had.
//! - [`Watcher`] — what the watch thread feeds: journal entries, a path that
//!   changed, lines of a long-lived child, and a clock armed only while the
//!   module asks for one ([`Watcher::next_wake`]).
//! - [`Module::face`] and [`Module::screen`] — what is published: marks on the
//!   modeline, notices on the brain's strip, one-shot items, and the rows of the
//!   module's own page. DATA, never sentences: keys, numbers and moments, and
//!   whoever draws translates and formats (invariants 8 and 11).
//!
//! # Deviations from the design (`brain.md` §2), each for a reason
//!
//! - **`Scheduled::kind` and `Scheduled::gated` are not methods.** The manifest
//!   declares both (`schedule.kind`, `schedule.gated`) and [`Manifest::kind`]
//!   answers; a trait default beside the declaration would be a second answer
//!   to one question, and the first time the two disagreed the page would draw
//!   one and the runner obey the other.
//! - **`Face` carries the screen's generation** (`Face::screen`), which is what
//!   the design's "a screen generation number" is: the mind says it moved, and
//!   the host asks for the screen off the loop.
//! - **Ids and keys are [`Word`]s** (`Cow<'static, str>`): a module writes its
//!   generated `&'static str` constants and pays nothing, and the SAME type
//!   crosses the socket and comes back owned. One type per idea instead of a
//!   static half and a wire half that have to be kept in step.
//! - **The entry is `-> impl Module + use<>`.** Rust 2024 makes an `impl
//!   Trait` capture every lifetime of its arguments; without `use<>` the
//!   module would borrow its `Birth` and could not be kept in the registry.
//! - **`Acted::Refused` carries a word**, like `Due::Not`: a core word
//!   (`unknown`) or one of the module's `whys`.
//! - **The journal's `Match` and `Entry` are `ronoctua-journal`'s**, re-exported
//!   here so a module names the contract and nothing else.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::moment::{Moment, Offset};
use crate::prefs::{PartOfDay, TaskPrefs, Where};
use crate::pulse::Pulse;
use crate::task::{Due, Outcome, Report, RunRecord, Sheet, Step, Verdict, Watched};

/// An id or a text key: `&'static` inside a module, owned once it has crossed
/// the socket.
///
/// A module writes the constants its manifest generated —
/// `Word::Borrowed(key::X)`, or `key::X.into()` — and the value that comes out
/// of a line of JSON on the footer's side is the same type, owned.
pub type Word = Cow<'static, str>;

/// Named values a sentence is made with: `("freed", Arg::Bytes(n))`.
///
/// The NAME is the Fluent variable; the value is what the host formats in the
/// person's language before the sentence is built.
pub type Args = Vec<(Word, Arg)>;

// ---------------------------------------------------------------------------
// Birth, and the manifest the generator writes.
// ---------------------------------------------------------------------------

/// Whether a module repeats, happens once, or only watches.
///
/// A `Copy` enum with no payload, for the reason `OutcomeKind` gives: what the
/// screen wants is a glyph and a colour, and a shape that carried anything more
/// would be a shape the WIT has to carry too.
///
/// # Why the list said so before there was anything to tell apart
///
/// Because a list that never distinguished two kinds is a list a person has
/// read as one kind; the day a one-shot lands, every row on the page would
/// change meaning at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// It comes round again.
    #[default]
    Routine,
    /// It happens once and then it is gone.
    Once,
    /// It runs nothing on a schedule: it listens (a journal, a path, a clock)
    /// and publishes what it heard. Its row says "watching" instead of a date.
    Watch,
}

/// What a module is given when it is made.
///
/// # Why the zone is here
///
/// Because [`Scheduled::due`] is PURE — it takes no clock and opens no file —
/// and a schedule in a person's own day needs to know what o'clock it is where
/// they live. Handing the zone in at construction keeps the scheduling testable
/// with a zone that jumps on a day the test chooses.
pub struct Birth {
    /// How far ahead of UTC this machine is.
    pub zone: Arc<dyn Offset>,
    /// `$XDG_CONFIG_HOME/ronoctua` and `$XDG_STATE_HOME/ronoctua`; `None` is
    /// nowhere, which is what a test hands in.
    pub roots: Where,
}

impl std::fmt::Debug for Birth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Birth")
            .field("roots", &self.roots)
            .finish_non_exhaustive()
    }
}

impl Birth {
    /// A birth in UTC with nowhere to read or write — what a test hands in.
    #[must_use]
    pub fn detached() -> Self {
        Self {
            zone: Arc::new(crate::moment::Fixed(0)),
            roots: Where::default(),
        }
    }

    /// `$XDG_STATE_HOME/ronoctua/brain/<module>`: what the module keeps.
    #[must_use]
    pub fn state_dir(&self, module: &str) -> Option<PathBuf> {
        under(self.roots.state.as_deref(), module)
    }

    /// `$XDG_CONFIG_HOME/ronoctua/brain/<module>`: what the person chose.
    #[must_use]
    pub fn config_dir(&self, module: &str) -> Option<PathBuf> {
        under(self.roots.config.as_deref(), module)
    }
}

/// A module's own directory under one of the two roots.
fn under(root: Option<&Path>, module: &str) -> Option<PathBuf> {
    root.map(|r| r.join("brain").join(module))
}

/// Everything the manifest declares, GENERATED from `manifest.nix` — never
/// written by hand in a module.
///
/// A module answers [`Module::manifest`] with `&super::MANIFEST`, the static
/// the generator wrote beside it. Everything here is `&'static`: nothing is
/// read from a file, so nothing a person can write names a unit or a command.
#[derive(Debug)]
pub struct Manifest {
    /// The id: the name of the module's folder, English and kebab-case.
    pub id: &'static str,
    /// What it does, in one English line — for `ronoctua-brain modules`.
    pub what: &'static str,
    /// `brain-task-<id>`: its name on screen.
    pub name_key: &'static str,
    /// `brain-task-<id>-does`: the line under the cursor.
    pub does_key: &'static str,
    /// What it needs from the machine.
    pub needs: Needs,
    /// Its schedule; `None` is a watch.
    pub schedule: Option<Schedule>,
    /// The units it starts, by name.
    pub units: Units,
    /// The words it may give as `Due::Not { why }` or `Acted::Refused`, with
    /// their keys (`<id>-why-<word>`).
    pub whys: &'static [Why],
    /// The marks it may put on the modeline.
    pub marks: &'static [MarkId],
    /// The notices it may raise.
    pub notices: &'static [NoticeKind],
    /// What a person can ask of it by name (`<id>-<action>`).
    pub actions: &'static [Action],
    /// What it may tell the footer, by tag.
    pub events: &'static [EventKind],
}

impl Manifest {
    /// Whether it repeats, happens once, or watches — from the schedule.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        match self.schedule {
            Some(s) => s.kind,
            None => Kind::Watch,
        }
    }

    /// Whether the general gate (memory, processor, battery) may hold its run.
    /// A watch has no run, so nothing to hold.
    #[must_use]
    pub const fn gated(&self) -> bool {
        match self.schedule {
            Some(s) => s.gated,
            None => false,
        }
    }

    /// What the DISTRO chose for it, when the person has chosen nothing.
    ///
    /// The person's file still wins over this: an entry there is something
    /// they changed, and this is only what fills the gap where they have not.
    /// A watch gets the shared defaults with no countdown — there is nothing
    /// of its own to announce.
    #[must_use]
    pub fn defaults(&self) -> TaskPrefs {
        match self.schedule {
            Some(s) => TaskPrefs {
                enabled: true,
                interval_days: s.interval_days,
                part_of_day: s.part_of_day,
                countdown_seconds: s.countdown_seconds,
                runaway_minutes: s.runaway_minutes,
                every_minutes: s.every_minutes,
                steps_flipped: std::collections::BTreeSet::new(),
            },
            None => TaskPrefs {
                countdown_seconds: 0,
                ..TaskPrefs::default()
            },
        }
    }

    /// One action, by its id.
    #[must_use]
    pub fn action(&self, id: &str) -> Option<&'static Action> {
        self.actions.iter().find(|a| a.id == id)
    }

    /// One notice's declaration, by its id.
    #[must_use]
    pub fn notice(&self, id: &str) -> Option<&'static NoticeKind> {
        self.notices.iter().find(|n| n.id == id)
    }

    /// The key a word of this module's is drawn with, or `None` when the word
    /// is not one of its `whys` (then it is a core word, `brain-why-<word>`).
    #[must_use]
    pub fn why_key(&self, word: &str) -> Option<&'static str> {
        self.whys.iter().find(|w| w.word == word).map(|w| w.key)
    }
}

/// What a module needs from the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Needs {
    /// Its unit runs as root (drawn: "what is about to happen to my machine"
    /// weighs differently when the answer is yes).
    pub root: bool,
    /// It talks to the network, so no network is a hold (`Hold::Network`).
    pub network: bool,
}

/// When a scheduled module runs, as the distro ships it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    /// `Routine` or `Once` — a watch has no schedule at all.
    pub kind: Kind,
    /// Whether it keeps a calendar (days and a part of the day). `false` is a
    /// module that comes due only when the machine wants it
    /// ([`Scheduled::wanted_now`]) or somebody asks, and its page does not
    /// draw a cadence it does not keep.
    pub calendar: bool,
    /// Days between runs.
    pub interval_days: u16,
    /// Which part of the day it prefers.
    pub part_of_day: PartOfDay,
    /// A cadence in minutes, for a module that cannot wait for tomorrow.
    pub every_minutes: Option<u32>,
    /// How long the strip warns before it starts, in seconds.
    pub countdown_seconds: u16,
    /// After how many silent minutes the run is called runaway.
    pub runaway_minutes: u16,
    /// Whether the general gate may hold it — `false` only for what exists to
    /// rescue a busy machine (a relief of the disk).
    pub gated: bool,
}

/// The units a module starts. Names, never command lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Units {
    /// The real run.
    pub run: Option<&'static str>,
    /// The same run, changing nothing.
    pub rehearse: Option<&'static str>,
}

/// A word a module may give, and the key it is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Why {
    /// English, kebab-case: `when-short`.
    pub word: &'static str,
    /// `<id>-why-<word>`.
    pub key: &'static str,
}

/// Which mark, as the generator declared it: its name, its glyph and its hint.
///
/// It crosses the socket inside a [`Mark`], which is why it holds [`Word`]s:
/// declared `&'static` (`MarkId::of`), read back owned.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MarkId {
    /// The mark's name inside its module: `full`. The modeline segment is
    /// `brain:<module>/<name>`.
    pub name: Word,
    /// The glyph's TOKEN name, merged into the measured icon table:
    /// `<module>-<glyph>`. Never a codepoint (invariant 10).
    pub glyph: Word,
    /// The key of the line that explains it under the pointer.
    pub hint: Word,
    /// A command of the SHELL's registry a click runs instead of opening the
    /// module's page — the mark of a module that has an app of its own (the
    /// running containers open the containers' view). `None`: the click is
    /// the mark's action, or the module's page. The shell runs it by name,
    /// so it reaches nothing the keyboard does not (`LEADER :`), and names
    /// no module to do so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opens: Option<Word>,
}

impl MarkId {
    /// A declaration — what the generated `mark::*` constants are.
    #[must_use]
    pub const fn of(name: &'static str, glyph: &'static str, hint: &'static str) -> Self {
        Self {
            name: Cow::Borrowed(name),
            glyph: Cow::Borrowed(glyph),
            hint: Cow::Borrowed(hint),
            opens: None,
        }
    }

    /// The same mark, clicked to the shell's command `command` — see
    /// [`MarkId::opens`].
    #[must_use]
    pub fn opening(&self, command: &'static str) -> Self {
        Self {
            opens: Some(Cow::Borrowed(command)),
            ..self.clone()
        }
    }
}

/// A notice a module may raise, as the manifest declared it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoticeKind {
    /// Its id inside the module: `due`.
    pub id: &'static str,
    /// How loud.
    pub level: Level,
    /// Whether a person may dismiss it.
    pub dismiss: bool,
    /// The snoozes offered, in minutes; empty is none. The first is the one
    /// the strip's control uses.
    pub snooze: &'static [u16],
}

/// Something a person can ask of a module by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    /// Its id inside the module: `new`.
    pub id: &'static str,
    /// The registry command: `<id>-<action>`.
    pub command: &'static str,
    /// The command's label: `cmd-<id>-<action>`.
    pub label: &'static str,
    /// Whether it acts on a ROW of the module's screen (`Act::row` is set)
    /// rather than on the module as a whole.
    pub row: bool,
    /// The key of the question it asks first, when it needs an answer; the
    /// answer arrives in `Act::answer`.
    pub asks: Option<&'static str>,
}

/// An event a module may tell the footer, as the manifest declared it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventKind {
    /// Its name inside the module: `relieved`.
    pub name: &'static str,
    /// The tag it travels and is filtered under: `<id>-<name>`.
    pub tag: &'static str,
}

// ---------------------------------------------------------------------------
// What a module publishes.
// ---------------------------------------------------------------------------

/// What a module may look at when it answers [`Module::face`] or
/// [`Module::screen`] — handed in, so both stay pure over the module's own
/// state and this.
#[derive(Debug, Clone, Copy)]
pub struct Look<'a> {
    /// Now, by the daemon's clock.
    pub now: Moment,
    /// The pulse the runner last took.
    pub pulse: &'a Pulse,
    /// The person's preferences for it, resolved over its defaults.
    pub mine: &'a TaskPrefs,
    /// Its last record in the journal, if any.
    pub last: Option<&'a RunRecord>,
}

/// Everything a module shows, all at once.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Face {
    /// Marks on the modeline. Empty at rest: a mark exists only when it says
    /// something (invariant 14).
    pub marks: Vec<Mark>,
    /// Notices on the brain's strip, with their controls.
    pub notices: Vec<Notice>,
    /// One-shot rows (`Kind::Once`): a reminder that rings once and leaves.
    pub items: Vec<Item>,
    /// The generation of [`Module::screen`]: it moves whenever the screen
    /// would answer differently, and the host asks for the screen only then.
    pub screen: u64,
}

/// A mark on the modeline.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Mark {
    /// Which, as declared (`mark::*`).
    pub id: MarkId,
    /// Its colour, by meaning.
    pub tone: Tone,
    /// A number drawn beside the glyph; `0` draws none.
    pub count: u32,
    /// The action of ITS MODULE a click runs; `None` opens the module's
    /// page. A mark that says "go there" — an app of the shell — says so in
    /// its id instead ([`MarkId::opens`]): ONE way for a mark to open a
    /// command, and it is typed, not a prefix on a string.
    pub act: Option<Word>,
}

/// The colour a mark or a row is drawn in — a MEANING, one token each in the
/// shell (`Watching` is the gestures' glyph colour).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tone {
    /// Nothing to worry about.
    #[default]
    Quiet,
    /// Something is being followed.
    Watching,
    /// Look soon.
    Warning,
    /// Look now.
    Danger,
    /// The module's own emphasis.
    Accent,
}

/// How loud a notice is. The ORDER is the strip's precedence, quietest first:
/// an alarm beats the countdown, then danger, warn and info.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// Worth saying.
    #[default]
    Info,
    /// Something is wrong.
    Warn,
    /// Something is wrong and costs something now.
    Danger,
    /// It rings, and it stays until a person answers it.
    Alarm,
}

/// Something a module wants said on the brain's strip.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Notice {
    /// Its id inside the module, as declared (`notice::*`).
    pub id: Word,
    /// What it says, as a text key.
    pub says: Word,
    /// The sentence's values.
    pub args: Args,
    /// How loud.
    pub level: Level,
    /// Since when it has been true.
    pub since: Moment,
    /// Actions offered beside it, by id.
    pub controls: Vec<Word>,
    /// Whether a person may dismiss it.
    pub dismiss: bool,
    /// The snoozes offered, in minutes.
    pub snooze: Cow<'static, [u16]>,
}

impl Notice {
    /// A notice as its declaration says: level, dismissal and snoozes come
    /// from the manifest, so a module cannot raise one louder than declared.
    #[must_use]
    pub fn of(kind: &NoticeKind, says: &'static str, since: Moment) -> Self {
        Self {
            id: Cow::Borrowed(kind.id),
            says: Cow::Borrowed(says),
            args: Vec::new(),
            level: kind.level,
            since,
            controls: Vec::new(),
            dismiss: kind.dismiss,
            snooze: Cow::Borrowed(kind.snooze),
        }
    }

    /// The same, with one more value for the sentence.
    #[must_use]
    pub fn with(mut self, name: &'static str, arg: Arg) -> Self {
        self.args.push((Cow::Borrowed(name), arg));
        self
    }

    /// The same, with an action offered beside it.
    #[must_use]
    pub fn control(mut self, action: &Action) -> Self {
        self.controls.push(Cow::Borrowed(action.id));
        self
    }
}

/// One value of a sentence. The SHELL formats it, in the person's language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Arg {
    /// Data, drawn as it is: a unit's name, what a person typed.
    Text(String),
    /// A text key, translated.
    Key(Word),
    /// A count.
    Number(i64),
    /// A size in bytes.
    Bytes(u64),
    /// A proportion, in per mille.
    PerMille(u16),
    /// An instant, in the person's zone.
    Moment(Moment),
}

/// A one-shot row: something that happens once and then leaves the list.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Item {
    /// Its id inside the module — DATA, stable while it exists.
    pub id: String,
    /// What it is: a key, or what the person typed (data, invariant 8).
    pub label: Cell,
    /// When it comes due.
    pub at: Moment,
    /// The action a press runs on it, by id.
    pub act: Option<Word>,
}

/// A module's own page, as DATA rows — built only when asked
/// ([`Module::screen`]), never carried in the mind.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Screen {
    /// In order.
    pub sections: Vec<Section>,
}

/// One titled block of a module's page.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Section {
    /// Its title, as a text key.
    pub heading: Word,
    /// Its rows.
    pub rows: Vec<Row>,
}

/// One row of a module's page.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Row {
    /// Its id inside the module — DATA, handed back in `Act::row`.
    pub id: String,
    /// What it is.
    pub label: Cell,
    /// Its value.
    pub value: Cell,
    /// A quieter line under it.
    pub note: Cell,
    /// Its colour, by meaning.
    pub tone: Tone,
    /// The action a press runs on it, by id; `None` is a row that only tells.
    pub act: Option<Word>,
    /// A SECOND action on it, by id, on a key of its own (the brain's page
    /// teaches it while the cursor is on the row): what a row offers beside
    /// its press — the journal's "ignore this type" on an error not yet seen,
    /// which is otherwise two steps. `None` is a row with one action or none.
    pub also: Option<Word>,
}

/// One cell of a row. The host formats moments and sizes; the component has no
/// zone and no decimal mark (invariant 11).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cell {
    /// Nothing.
    #[default]
    Empty,
    /// A text key.
    Key(Word),
    /// Data, drawn as it is.
    Data(String),
    /// An instant.
    Moment(Moment),
    /// A count.
    Count(u64),
    /// A size in bytes.
    Bytes(u64),
}

// ---------------------------------------------------------------------------
// What a person asks, and what a module tells.
// ---------------------------------------------------------------------------

/// A person asked for one of a module's actions (`Ask::Act`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Act {
    /// Which, by id — one of the manifest's `actions`.
    pub action: String,
    /// The row it was asked on, for an action with `row = true`.
    pub row: Option<String>,
    /// What the person typed, for an action that `asks` first.
    pub answer: Option<String>,
}

/// What became of an [`Act`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acted {
    /// Done; the module's face is published again.
    Done,
    /// Put the module's scheduled run in the queue — the gate, the countdown
    /// and the journal apply as to any run.
    Queue,
    /// Not done, and the WORD says why: a core word (`unknown`) or one of the
    /// module's `whys`.
    Refused(&'static str),
}

/// Something a module writes into the activity log — a record of kind
/// `Noted`, which is not a run and moves no schedule.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Noted {
    /// What it says, as a text key.
    pub says: Word,
    /// The sentence's values.
    pub args: Args,
}

impl Noted {
    /// A record that says this.
    #[must_use]
    pub fn of(says: &'static str) -> Self {
        Self {
            says: Cow::Borrowed(says),
            args: Vec::new(),
        }
    }

    /// The same, with one more value for the sentence.
    #[must_use]
    pub fn with(mut self, name: &'static str, arg: Arg) -> Self {
        self.args.push((Cow::Borrowed(name), arg));
        self
    }
}

/// Something a module tells the footer once — "room made: 12 GB freed". It
/// lands in the mind's ring of events and the footer says it when it is new.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Event {
    /// The tag, as declared (`event::*`): `<id>-<name>`.
    pub tag: Word,
    /// The sentence's values.
    pub args: Args,
}

impl Event {
    /// The event a declaration names.
    #[must_use]
    pub fn of(kind: &EventKind) -> Self {
        Self {
            tag: Cow::Borrowed(kind.tag),
            args: Vec::new(),
        }
    }

    /// The same, with one more value for the sentence.
    #[must_use]
    pub fn with(mut self, name: &'static str, arg: Arg) -> Self {
        self.args.push((Cow::Borrowed(name), arg));
        self
    }
}

/// Where what a module does goes: the journal, the mind, the footer.
///
/// A trait so the core writes the real one and a test keeps everything in a
/// [`Kept`]. Every call is quick and never blocks on a module.
pub trait Sink: Send + Sync {
    /// Write a record into the activity log.
    fn record(&self, module: &'static str, at: Moment, noted: Noted);
    /// Tell the footer something once.
    fn tell(&self, module: &'static str, event: Event);
    /// The module's face changed; publish it again.
    fn changed(&self, module: &'static str);
}

/// What a module may do while it answers: the one door out of it.
pub struct Ctx {
    module: &'static str,
    now: Moment,
    roots: Where,
    rehearsing: bool,
    sink: Arc<dyn Sink>,
}

impl std::fmt::Debug for Ctx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ctx")
            .field("module", &self.module)
            .field("now", &self.now)
            .field("roots", &self.roots)
            .field("rehearsing", &self.rehearsing)
            .finish_non_exhaustive()
    }
}

impl Ctx {
    /// One, for one module, at one moment.
    #[must_use]
    pub fn new(
        module: &'static str,
        now: Moment,
        roots: Where,
        rehearsing: bool,
        sink: Arc<dyn Sink>,
    ) -> Self {
        Self {
            module,
            now,
            roots,
            rehearsing,
            sink,
        }
    }

    /// One that keeps everything, nowhere on disk — what a test hands in —
    /// and the keeper to read it back from.
    #[must_use]
    pub fn kept(module: &'static str, now: Moment) -> (Self, Arc<Kept>) {
        let kept = Arc::new(Kept::default());
        let sink: Arc<dyn Sink> = kept.clone();
        (Self::new(module, now, Where::default(), false, sink), kept)
    }

    /// Whose.
    #[must_use]
    pub const fn module(&self) -> &'static str {
        self.module
    }

    /// Now, by the daemon's clock.
    #[must_use]
    pub const fn now(&self) -> Moment {
        self.now
    }

    /// Whether this is a rehearsal: a dry run that changes nothing, and whose
    /// records say so.
    #[must_use]
    pub const fn rehearsing(&self) -> bool {
        self.rehearsing
    }

    /// `$XDG_STATE_HOME/ronoctua/brain/<id>`, or `None` for nowhere.
    #[must_use]
    pub fn state_dir(&self) -> Option<PathBuf> {
        under(self.roots.state.as_deref(), self.module)
    }

    /// `$XDG_CONFIG_HOME/ronoctua/brain/<id>`, or `None` for nowhere.
    #[must_use]
    pub fn config_dir(&self) -> Option<PathBuf> {
        under(self.roots.config.as_deref(), self.module)
    }

    /// Writes a record into the activity log.
    pub fn record(&self, noted: Noted) {
        self.sink.record(self.module, self.now, noted);
    }

    /// Tells the footer something once.
    pub fn tell(&self, event: Event) {
        self.sink.tell(self.module, event);
    }

    /// Says the face changed, so it is published again.
    pub fn changed(&self) {
        self.sink.changed(self.module);
    }

    /// The same door, for work that outlives the call it was handed to.
    ///
    /// An `act` is answered on the socket's thread, which a client waits on
    /// for half a second at most; work that may take longer — a call on the
    /// system bus that has to start a daemon first — goes to a thread of its
    /// own, and this is how that thread still says what came of it: the face
    /// moved, and what it noticed goes into the activity log. `now` stays the
    /// moment of the call.
    #[must_use]
    pub fn detached(&self) -> Self {
        Self {
            module: self.module,
            now: self.now,
            roots: self.roots.clone(),
            rehearsing: self.rehearsing,
            sink: Arc::clone(&self.sink),
        }
    }
}

/// A [`Sink`] that keeps what it was given, for a test to read back.
#[derive(Debug, Default)]
pub struct Kept {
    records: Mutex<Vec<(String, Moment, Noted)>>,
    events: Mutex<Vec<(String, Event)>>,
    changes: Mutex<Vec<String>>,
}

impl Kept {
    /// Every record, in order, with whose it was and when.
    #[must_use]
    pub fn records(&self) -> Vec<(String, Moment, Noted)> {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Every event, in order, with whose it was.
    #[must_use]
    pub fn events(&self) -> Vec<(String, Event)> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Whose face changed, once per call.
    #[must_use]
    pub fn changes(&self) -> Vec<String> {
        self.changes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl Sink for Kept {
    fn record(&self, module: &'static str, at: Moment, noted: Noted) {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((module.to_owned(), at, noted));
    }

    fn tell(&self, module: &'static str, event: Event) {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((module.to_owned(), event));
    }

    fn changed(&self, module: &'static str) {
        self.changes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(module.to_owned());
    }
}

/// One run of a scheduled module: the old `Report` (the file the unit writes,
/// and the patience to watch it) and the [`Ctx`] (the door out).
///
/// `say` and `relieve` are gone: a notice is part of the module's [`Face`], and
/// a relief is an [`Event`] told through [`Ctx::tell`].
#[derive(Debug)]
pub struct Run {
    report: Report,
    ctx: Ctx,
}

impl Run {
    /// One run.
    #[must_use]
    pub const fn new(report: Report, ctx: Ctx) -> Self {
        Self { report, ctx }
    }

    /// The door out.
    #[must_use]
    pub const fn ctx(&self) -> &Ctx {
        &self.ctx
    }

    /// The report file and its watching.
    #[must_use]
    pub const fn report(&self) -> &Report {
        &self.report
    }

    /// Whether this run is a rehearsal.
    #[must_use]
    pub const fn is_rehearsal(&self) -> bool {
        self.report.is_rehearsal()
    }

    /// Where the unit writes — what it is told, and what a person can `cat`.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.report.path()
    }

    /// What the report file looks like now, before the unit is started.
    #[must_use]
    pub fn mark(&self) -> Option<std::time::SystemTime> {
        self.report.mark()
    }

    /// Watches until the unit says it ended, the patience runs out, or somebody
    /// aborts.
    #[must_use]
    pub fn watch(&self, since: Option<std::time::SystemTime>) -> Watched {
        self.report.watch(since)
    }

    /// What the report says now.
    #[must_use]
    pub fn sheet(&self) -> Option<Sheet> {
        self.report.sheet()
    }
}

// ---------------------------------------------------------------------------
// What a watcher listens to.
// ---------------------------------------------------------------------------

/// What a watcher listens to. Read ONCE, when the watch thread starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Journal entries that pass this match. One follower serves every module;
    /// it is started only if some module asked for one.
    Journal(Match),
    /// A path: something changed at it, or under it (inotify, never a poll).
    Path(PathBuf),
    /// The lines of a long-lived child, restarted with a backoff when it dies.
    /// `argv[0]` is a STORE PATH, never a name.
    Lines {
        /// The command line.
        argv: Vec<String>,
    },
}

/// What a watcher heard. `source` is the index into what
/// [`Watcher::sources`] returned.
#[derive(Debug, Clone, Copy)]
pub enum Heard<'a> {
    /// A journal entry that passed the source's match.
    Journal {
        /// Which source.
        source: usize,
        /// The entry.
        entry: &'a Entry,
    },
    /// Something changed at, or under, a watched path.
    Path {
        /// Which source.
        source: usize,
        /// What changed.
        path: &'a Path,
    },
    /// A line from a watched child.
    Line {
        /// Which source.
        source: usize,
        /// The line, without its newline.
        line: &'a str,
    },
    /// The clock [`Watcher::next_wake`] asked for.
    Wake,
    /// A source could not be followed — the journal is unreadable, the child
    /// keeps dying. Said, so the module's page can refuse with a sentence
    /// instead of going quiet.
    ///
    /// Said when a source is DOWN, not every time it restarts: a follower
    /// that ended after it had been following is started again from its
    /// cursor and nothing was missed. Said again when the words change.
    Lost {
        /// Which source.
        source: usize,
        /// What went wrong, in the machine's words.
        why: &'a str,
    },
    /// A source said [`Heard::Lost`] is followed again. Without it a page
    /// would go on refusing after the restart worked, until a matching line
    /// happened to arrive — which for a rare match is never.
    Back {
        /// Which source.
        source: usize,
    },
}

/// The journal's vocabulary, from the one crate that follows it.
///
/// `Match` is what a module asks for ([`Source::Journal`]), `Entry` what it
/// hears ([`Heard::Journal`]), and the rest is what those two are made of. They
/// were local copies of the same shapes while `ronoctua-journal` was being
/// written; nothing that names `module::Match` changed when they were swapped.
pub use ronoctua_journal::{Backfill, Cursor, Entry, Field, Match, Priority};

// ---------------------------------------------------------------------------
// The three traits.
// ---------------------------------------------------------------------------

/// One module of the brain.
///
/// Every method but [`Module::manifest`] has a default: a module answers only
/// the questions it has.
pub trait Module: Send + Sync {
    /// What the manifest declared — `&super::MANIFEST`, generated.
    fn manifest(&self) -> &'static Manifest;

    /// What its run does, in order — DATA, so the page can draw it and the
    /// person can switch one off.
    fn steps(&self) -> &'static [Step] {
        &[]
    }

    /// Its run, when it has a schedule.
    fn scheduled(&self) -> Option<&dyn Scheduled> {
        None
    }

    /// What it listens to, when it watches.
    fn watcher(&self) -> Option<&dyn Watcher> {
        None
    }

    /// What it shows now. PURE over its own state and `look`.
    fn face(&self, _look: &Look<'_>) -> Face {
        Face::default()
    }

    /// Its own page. Built only when the host asks (`Ask::Screen`), off the
    /// footer's loop.
    fn screen(&self, _look: &Look<'_>) -> Screen {
        Screen::default()
    }

    /// A person asked for one of its actions. MUST NOT BLOCK: what takes long
    /// is its run, which [`Acted::Queue`] puts in the queue.
    fn act(&self, _act: &Act, _ctx: &Ctx) -> Acted {
        Acted::Refused("unknown")
    }

    /// A person dismissed one of its notices, by id.
    fn dismissed(&self, _notice: &str, _ctx: &Ctx) {}

    /// A person put one of its notices off until `until`.
    fn snoozed(&self, _notice: &str, _until: Moment, _ctx: &Ctx) {}

    /// The person's preferences for it were written; a module that keeps a
    /// derived view of them for a unit (the update's steps file) writes it
    /// here.
    ///
    /// # Errors
    ///
    /// The view could not be written.
    fn prefs_saved(&self, _mine: &TaskPrefs, _dirs: &Where) -> std::io::Result<()> {
        Ok(())
    }

    /// `ronoctua-brain work <id> [args…]`: the work of a long unit, in the
    /// brain's binary, writing its report at `report`.
    ///
    /// # Errors
    ///
    /// The module has no work of its own, or the work failed.
    fn work(&self, _args: &[String], _report: &Path) -> anyhow::Result<()> {
        anyhow::bail!("the module `{}` has no work of its own", self.manifest().id)
    }
}

/// A module that runs on a schedule: the old `Task`, minus what the manifest
/// now declares (id, defaults, steps, countdown, kind, gate).
///
/// `mine` is ALWAYS the person's preferences resolved over the module's own
/// defaults: there is no way here to read a default the page does not show.
pub trait Scheduled: Send + Sync {
    /// When it wants to run. PURE: `now` is handed in and nothing is opened.
    /// `last` is `None` when it never ran — a first run, not an overdue one.
    fn due(&self, now: Moment, mine: &TaskPrefs, last: Option<Moment>) -> Due;

    /// Whether it may run now, beyond the general gate — for what only it
    /// knows.
    fn needs(&self, _pulse: &Pulse, _mine: &TaskPrefs) -> Verdict {
        Verdict::Go
    }

    /// Whether the MACHINE wants it now, whatever the calendar says (a disk
    /// running short). PURE, like `due`.
    fn wanted_now(
        &self,
        _now: Moment,
        _pulse: &Pulse,
        _mine: &TaskPrefs,
        _last: Option<Moment>,
    ) -> bool {
        false
    }

    /// Do it. On the runner's thread, alone, and it may block.
    fn run(&self, run: &Run) -> Outcome;
}

/// A module that listens.
pub trait Watcher: Send + Sync {
    /// What it listens to. Read once, when the watch thread starts.
    fn sources(&self) -> Vec<Source>;

    /// The watch thread started, or the module was switched on: catch up on
    /// what was missed while it was off. A module switched off hears nothing
    /// until then.
    fn started(&self, _now: Moment, _ctx: &Ctx) {}

    /// Something it listens to happened. On the watch thread: NEVER BLOCKS.
    fn heard(&self, heard: Heard<'_>, ctx: &Ctx);

    /// When it next wants [`Heard::Wake`]. `None` arms no clock at all — the
    /// watch thread blocks until something is heard.
    ///
    /// Asked again after everything the thread hands out, and whenever the
    /// module says [`Ctx::changed`] from any thread — so a wake armed by an
    /// action is seen at once.
    fn next_wake(&self, _now: Moment) -> Option<Moment> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FICTIONAL IDS, on purpose: the core's fixtures never name a real module
    // (the sealed check refuses a module's name outside its folder).
    static FIXTURE: Manifest = Manifest {
        id: "fixture",
        what: "A module that only exists in this test.",
        name_key: "brain-task-fixture",
        does_key: "brain-task-fixture-does",
        needs: Needs {
            root: false,
            network: false,
        },
        schedule: Some(Schedule {
            kind: Kind::Routine,
            calendar: true,
            interval_days: 5,
            part_of_day: PartOfDay::Night,
            every_minutes: None,
            countdown_seconds: 0,
            runaway_minutes: 30,
            gated: false,
        }),
        units: Units {
            run: Some("fixture.service"),
            rehearse: None,
        },
        whys: &[Why {
            word: "when-empty",
            key: "fixture-why-when-empty",
        }],
        marks: &[MarkId::of("full", "fixture-full", "fixture-mark-full")],
        notices: &[NoticeKind {
            id: "due",
            level: Level::Alarm,
            dismiss: true,
            snooze: &[5, 10],
        }],
        actions: &[Action {
            id: "forget",
            command: "fixture-forget",
            label: "cmd-fixture-forget",
            row: true,
            asks: None,
        }],
        events: &[EventKind {
            name: "emptied",
            tag: "fixture-emptied",
        }],
    };

    struct Bare;

    impl Module for Bare {
        fn manifest(&self) -> &'static Manifest {
            &FIXTURE
        }
    }

    #[test]
    fn a_module_that_answers_only_its_manifest_is_a_module() {
        // THE DEFAULTS ARE THE CONTRACT'S PROMISE: a module answers only the
        // questions it has, and every other one is a quiet no.
        let m: Box<dyn Module> = Box::new(Bare);
        assert!(m.scheduled().is_none());
        assert!(m.watcher().is_none());
        assert!(m.steps().is_empty());
        let pulse = Pulse::default();
        let mine = TaskPrefs::default();
        let look = Look {
            now: 1,
            pulse: &pulse,
            mine: &mine,
            last: None,
        };
        assert_eq!(m.face(&look), Face::default());
        assert_eq!(m.screen(&look), Screen::default());
        let (ctx, kept) = Ctx::kept("fixture", 1);
        assert_eq!(
            m.act(&Act::default(), &ctx),
            Acted::Refused("unknown"),
            "an action nobody declared was not refused"
        );
        assert!(kept.records().is_empty() && kept.changes().is_empty());
        assert!(m.work(&[], Path::new("/nonexistent")).is_err());
    }

    #[test]
    fn the_defaults_come_from_the_manifest_and_a_watch_announces_nothing() {
        // THE DEFECT THIS CLOSES: `Prefs::mine` and three other places reading
        // the SHARED default (weekly, afternoon, a minute of countdown) about a
        // task that declared its own — a watch switched off and on again came
        // back weekly with a countdown.
        let mine = FIXTURE.defaults();
        assert_eq!(mine.interval_days, 5);
        assert_eq!(mine.part_of_day, PartOfDay::Night);
        assert_eq!(mine.countdown_seconds, 0);
        assert_eq!(mine.runaway_minutes, 30);
        assert!(mine.enabled);
        assert_eq!(FIXTURE.kind(), Kind::Routine);
        assert!(!FIXTURE.gated());

        let watch = Manifest {
            schedule: None,
            ..FIXTURE
        };
        assert_eq!(watch.kind(), Kind::Watch);
        assert!(!watch.gated(), "a watch has no run for the gate to hold");
        assert_eq!(watch.defaults().countdown_seconds, 0);
    }

    #[test]
    fn a_word_of_the_module_has_its_key_and_a_core_word_does_not() {
        assert_eq!(
            FIXTURE.why_key("when-empty"),
            Some("fixture-why-when-empty")
        );
        assert_eq!(FIXTURE.why_key("off"), None);
        assert!(FIXTURE.action("forget").is_some_and(|a| a.row));
        assert!(FIXTURE.action("remember").is_none());
        assert!(FIXTURE.notice("due").is_some());
    }

    #[test]
    fn a_notice_is_as_loud_as_its_declaration_and_crosses_whole() {
        let Some(due) = FIXTURE.notice("due") else {
            panic!("the fixture declares `due`");
        };
        let notice = Notice::of(due, "fixture-says-due", 1_700_000_000)
            .with("freed", Arg::Bytes(12_345))
            .with("what", Arg::Text("a line somebody typed".to_owned()))
            .control(&FIXTURE.actions[0]);
        assert_eq!(notice.level, Level::Alarm);
        assert!(notice.dismiss);
        assert_eq!(&*notice.snooze, &[5, 10]);
        // THROUGH JSON AND BACK, which is what the socket does: the borrowed
        // constants come back owned and EQUAL.
        let face = Face {
            marks: vec![Mark {
                id: FIXTURE.marks[0].clone(),
                tone: Tone::Watching,
                count: 3,
                act: None,
            }],
            notices: vec![notice],
            items: vec![Item {
                id: "r-1".to_owned(),
                label: Cell::Data("call the plumber".to_owned()),
                at: 1_700_003_600,
                act: Some("forget".into()),
            }],
            screen: 7,
        };
        let line = serde_json::to_string(&face).expect("encode");
        let back: Face = serde_json::from_str(&line).expect("decode");
        assert_eq!(back, face);
    }

    /// A MARK THAT OPENS A COMMAND crosses the socket with it, and one that
    /// does not says nothing about it: a mind written before the field was
    /// there still reads, with no command.
    #[test]
    fn a_mark_that_opens_crosses_and_an_old_one_still_reads() {
        let plain = FIXTURE.marks[0].clone();
        let opening = plain.opening("fixture-open");
        assert_eq!(opening.opens.as_deref(), Some("fixture-open"));
        assert_eq!((&opening.name, &opening.glyph), (&plain.name, &plain.glyph));
        let wire = serde_json::to_string(&opening).unwrap();
        assert_eq!(serde_json::from_str::<MarkId>(&wire).unwrap(), opening);
        let old = serde_json::to_string(&plain).unwrap();
        assert!(!old.contains("opens"), "{old}");
        assert_eq!(serde_json::from_str::<MarkId>(&old).unwrap().opens, None);
    }

    #[test]
    fn a_screen_crosses_whole() {
        let screen = Screen {
            sections: vec![Section {
                heading: "fixture-heading".into(),
                rows: vec![Row {
                    id: "one".to_owned(),
                    label: Cell::Key("fixture-row".into()),
                    value: Cell::Bytes(1 << 30),
                    note: Cell::Moment(1_700_000_000),
                    tone: Tone::Danger,
                    act: Some("forget".into()),
                    also: Some("keep".into()),
                }],
            }],
        };
        let line = serde_json::to_string(&screen).expect("encode");
        assert_eq!(
            serde_json::from_str::<Screen>(&line).expect("decode"),
            screen
        );
    }

    #[test]
    fn the_level_order_is_the_strips_precedence() {
        // AN ALARM BEATS EVERYTHING the strip can show, then danger, warn and
        // info — the owner's decision, and `sort` is how the core applies it.
        let mut levels = vec![Level::Warn, Level::Alarm, Level::Info, Level::Danger];
        levels.sort_unstable();
        assert_eq!(
            levels,
            vec![Level::Info, Level::Warn, Level::Danger, Level::Alarm]
        );
    }

    #[test]
    fn the_door_out_goes_where_the_sink_says_and_the_dirs_are_the_modules() {
        let (ctx, kept) = Ctx::kept("fixture", 42);
        ctx.record(Noted::of("fixture-noted").with("n", Arg::Number(2)));
        ctx.tell(Event::of(&FIXTURE.events[0]).with("freed", Arg::Bytes(9)));
        ctx.changed();
        assert_eq!(kept.records().len(), 1);
        assert_eq!(kept.records()[0].1, 42, "the record was not stamped now");
        assert_eq!(kept.events()[0].0, "fixture");
        assert_eq!(kept.events()[0].1.tag, "fixture-emptied");
        assert_eq!(kept.changes(), vec!["fixture".to_owned()]);
        assert_eq!(ctx.state_dir(), None, "a test's context wrote somewhere");

        let rooted = Ctx::new(
            "fixture",
            0,
            Where {
                config: Some(PathBuf::from("/c/ronoctua")),
                state: Some(PathBuf::from("/s/ronoctua")),
            },
            true,
            Arc::new(Kept::default()),
        );
        assert_eq!(
            rooted.state_dir(),
            Some(PathBuf::from("/s/ronoctua/brain/fixture"))
        );
        assert_eq!(
            rooted.config_dir(),
            Some(PathBuf::from("/c/ronoctua/brain/fixture"))
        );
        assert!(rooted.rehearsing());
    }

    #[test]
    fn a_match_is_every_field_and_the_priority_and_the_text() {
        let wanted = Match {
            max_priority: Some(3),
            fields: vec![Field {
                name: "_SYSTEMD_UNIT".to_owned(),
                any_of: vec!["a.service".to_owned(), "b.service".to_owned()],
            }],
            message_contains: Some("Killed".to_owned()),
            backfill: Backfill::ThisBoot { most: 10 },
        };
        let mut entry = Entry {
            priority: Some(3),
            message: "Out of memory: Killed process 1".to_owned(),
            ..Entry::default()
        };
        entry
            .fields
            .insert("_SYSTEMD_UNIT".to_owned(), "b.service".to_owned());
        assert!(wanted.accepts(&entry));
        let quieter = Entry {
            priority: Some(6),
            ..entry.clone()
        };
        assert!(
            !wanted.accepts(&quieter),
            "an info line passed an err match"
        );
        let unsaid = Entry {
            priority: None,
            ..entry.clone()
        };
        assert!(
            !wanted.accepts(&unsaid),
            "a line with no priority read as an error"
        );
        let elsewhere = Entry {
            fields: std::collections::BTreeMap::new(),
            ..entry
        };
        assert!(!wanted.accepts(&elsewhere), "another unit's line passed");
    }

    /// THE `#[path]` PILOT (`brain.md` §3, "Step 1").
    ///
    /// `build.rs` writes the shim the generator will write for every module —
    /// an inline module that `include!`s the manifest and loads `module.rs` by
    /// an ABSOLUTE `#[path]` — and `module.rs` declares `mod rules;`. This
    /// compiling at all is half the proof (a `rules.rs` looked for anywhere
    /// else is a missing file); `file!()` is the other half.
    #[test]
    fn a_module_file_loaded_by_path_finds_its_siblings_beside_it() {
        use crate::pilot::pilot_sample;

        // BOXED AND OUTLIVING ITS BIRTH, which is what the registry does with
        // every module: the entry's `use<>` is what lets this compile.
        let m: Box<dyn Module> = Box::new(pilot_sample::new(&Birth::detached()));
        assert_eq!(m.manifest().id, "pilot-sample");
        assert_eq!(m.manifest().id, pilot_sample::ID);
        let beside = pilot_sample::rules_live_in();
        assert!(
            beside.ends_with("modules/pilot-sample/rules.rs"),
            "`mod rules;` resolved to {beside}, not beside module.rs"
        );

        // AND THE SAMPLE ANSWERS THROUGH THE CONTRACT, rules and all.
        let pulse = Pulse {
            disk_free_per_mille: Some(40),
            ..Pulse::default()
        };
        let mine = m.manifest().defaults();
        let look = Look {
            now: 100,
            pulse: &pulse,
            mine: &mine,
            last: None,
        };
        let face = m.face(&look);
        assert_eq!(face.marks.len(), 1, "a nearly full disk drew no mark");
        assert_eq!(face.marks[0].id, pilot_sample::mark::FULL);
        let calm = Pulse {
            disk_free_per_mille: Some(400),
            ..Pulse::default()
        };
        assert!(
            m.face(&Look {
                pulse: &calm,
                ..look
            })
            .marks
            .is_empty(),
            "a mark stayed up with the disk three-fifths free"
        );
        let Some(scheduled) = m.scheduled() else {
            panic!("the sample has a schedule");
        };
        assert_eq!(
            scheduled.due(100, &mine, None),
            Due::Not { why: "when-empty" }
        );
        assert_eq!(
            m.manifest().why_key("when-empty"),
            Some(pilot_sample::key::PILOT_SAMPLE_WHY_WHEN_EMPTY)
        );
    }
}
