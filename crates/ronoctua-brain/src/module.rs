//! WHAT A MODULE IS, AS IT CROSSES: what it declares and what it publishes.
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
//! `why.md` — and, for a module another crate names (its `namedBy`), a
//! `shared.rs`: the half those readers need, compiled into THIS crate beside
//! the manifest. The generator turns the manifest into a `manifest.rs`: `ID`,
//! `MANIFEST`, and the `key`, `mark`, `notice`, `action`, `event`, `why` and
//! `unit` constants this file's types are built from. `tests/modules/
//! pilot-sample/` is the hand-written sample of both.
//!
//! # Two halves, and which is which
//!
//! THIS crate holds what a module DECLARES (the [`Manifest`] and what it
//! names) and what it PUBLISHES (its [`Face`]: marks, notices, items, and its
//! [`Screen`]) — the vocabulary the socket carries, which the footer, the
//! brain's buffer and the daemon all speak. The traits a module implements,
//! and what it is handed while it answers (`Look`, `Ctx`, `Run`, what it
//! listens to), are the daemon's (`ronoctua_brain_daemon::module`), which
//! re-exports this whole: a `module.rs` names `crate::module::…` either way.
//! A module's code is compiled into the daemon alone; the footer links this.
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
//!   module would borrow its [`Birth`] and could not be kept in the registry.
//! - **`Acted::Refused` carries a word**, like `Due::Not`: a core word
//!   (`unknown`) or one of the module's `whys`.
//! - **The journal's `Match` and `Entry` are `ronoctua-journal`'s**, re-exported
//!   by the daemon's half so a module names the contract and nothing else.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::moment::{Moment, Offset};
use crate::prefs::{PartOfDay, TaskPrefs, Where};

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
/// Because the daemon's `Scheduled::due` is PURE — it takes no clock and
/// opens no file — and a schedule in a person's own day needs to know what
/// o'clock it is where they live. Handing the zone in at construction keeps the scheduling testable
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
        own_dir(self.roots.state.as_deref(), module)
    }

    /// `$XDG_CONFIG_HOME/ronoctua/brain/<module>`: what the person chose.
    #[must_use]
    pub fn config_dir(&self, module: &str) -> Option<PathBuf> {
        own_dir(self.roots.config.as_deref(), module)
    }
}

/// A module's own directory under one of the two roots: `<root>/brain/<id>`.
///
/// ONE RULE for both doors to it — a module's birth here, and the daemon's
/// `Ctx` while it answers — so the directory a module is born with is the one
/// it later writes into.
#[must_use]
pub fn own_dir(root: Option<&Path>, module: &str) -> Option<PathBuf> {
    root.map(|r| r.join("brain").join(module))
}

/// Everything the manifest declares, GENERATED from `manifest.nix` — never
/// written by hand in a module.
///
/// A module answers `Module::manifest` with `&super::MANIFEST`, the static
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
    /// Whether the distro ships it SWITCHED OFF — its row drawn and its switch
    /// at hand, but not running until the person turns it on. `false` for all
    /// but the few whose work is a choice to make rather than inherit
    /// (`journal-errors`, which reads the journal). Only the person's DEFAULT:
    /// a saved entry, on or off, still wins over it (see [`Self::defaults`]).
    pub starts_off: bool,
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
                enabled: !self.starts_off,
                interval_days: s.interval_days,
                part_of_day: s.part_of_day,
                countdown_seconds: s.countdown_seconds,
                runaway_minutes: s.runaway_minutes,
                every_minutes: s.every_minutes,
                steps_flipped: std::collections::BTreeSet::new(),
            },
            None => TaskPrefs {
                countdown_seconds: 0,
                enabled: !self.starts_off,
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

    /// One mark's declaration, by its name.
    #[must_use]
    pub fn mark(&self, name: &str) -> Option<&'static MarkId> {
        self.marks.iter().find(|m| m.name == name)
    }

    /// Why a mark this module published may NOT be shown, or `None` when its
    /// manifest lets it: what the core asks of every mark before the mind
    /// carries it (`ronoctua_brain_daemon::hub`). `earlier` is what this module already had
    /// shown in the same face.
    ///
    /// Read against the DECLARATION (by name), never against the bounds the
    /// published id carries: a module that built its own `MarkId` cannot give
    /// itself a tenth instance, or a rest the owner did not approve.
    #[must_use]
    pub fn refusal_of(&self, mark: &Mark, earlier: &[&Mark]) -> Option<&'static str> {
        let id = &mark.id;
        let Some(declared) = self.mark(&id.name) else {
            return Some("a mark its manifest does not declare");
        };
        if declared.instances == 0 && id.instance != 0 {
            return Some("an instance of a mark that is one");
        }
        if declared.instances != 0 && !(1..=declared.instances).contains(&id.instance) {
            return Some("an instance its manifest does not declare");
        }
        if id.label.is_some() && id.instance == 0 {
            return Some("a label on a mark that is one");
        }
        // AN INSTANCE IS OPENED, NEVER ACTED ON: `brain-act` names a module
        // and an action, and the instance clicked would not reach it — the
        // action would act on whichever one the module guessed.
        if id.instance != 0 && mark.act.is_some() {
            return Some("an action on one instance (an instance opens through `MarkId::opening`)");
        }
        // AN INSTANCE COUNTS NOTHING: its digit is drawn where a count would
        // be, a space after the glyph, and two numbers there would read as
        // one.
        if id.instance != 0 && mark.count != 0 {
            return Some("a count on one instance (its digit stands where a count would)");
        }
        // THE MODELINE AT REST IS SILENT: a quiet mark says only that all is
        // well, and only a mark declared `resting` may say that.
        if mark.tone == Tone::Quiet && !declared.resting {
            return Some("a mark at rest its manifest does not declare `resting`");
        }
        if earlier
            .iter()
            .any(|e| e.id.name == id.name && e.id.instance == id.instance)
        {
            return Some("the same mark twice");
        }
        None
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
    /// module that comes due only when the machine wants it (the daemon's
    /// `Scheduled::wanted_now`) or somebody asks, and its page does not draw a
    /// cadence it does not keep.
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

/// Which mark, as the generator declared it: its name, its glyph and its hint
/// — and, once published, which of its instances it is.
///
/// It crosses the socket inside a [`Mark`], which is why it holds [`Word`]s:
/// declared `&'static` (`MarkId::of`), read back owned.
///
/// # Why the instance is here and not on [`Mark`]
///
/// Because everything the footer makes a mark's SEGMENT out of is here: its
/// id (`brain:<module>/<name>.<n>`), its glyph and digit, the hint and what a
/// click runs. The instance is part of WHICH mark it is — the third account's
/// and the first account's are two segments — exactly as [`MarkId::opens`]
/// is part of where it leads, and both are set the same way, on the declared
/// constant ([`MarkId::numbered`], [`MarkId::opening`]). [`Mark`] keeps what a
/// publication says ABOUT the mark: its tone, its count and its action.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MarkId {
    /// The mark's name inside its module: `full`. The modeline segment is
    /// `brain:<module>/<name>`, and `brain:<module>/<name>.<n>` for one of
    /// its instances ([`MarkId::local`]).
    pub name: Word,
    /// The glyph's TOKEN name, measured into the brain's catalogue
    /// ([`crate::catalogue`]): `<module>-<glyph>`. Never a codepoint
    /// (invariant 10).
    pub glyph: Word,
    /// The key of the line that explains it under the pointer.
    pub hint: Word,
    /// A command of the SHELL's registry a click runs instead of opening the
    /// module's page — the mark of a module that has an app of its own (the
    /// running containers open the containers' view). `None`: the click is
    /// the mark's action, or the module's page. The shell runs it by name,
    /// so it reaches nothing the keyboard does not (`LEADER :`), and names
    /// no module to do so. For one of a mark's instances the click hands the
    /// command the instance's number as its argument.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opens: Option<Word>,
    /// DECLARED: how many of it a module may show at once (`instances = N`
    /// in the manifest, from 1 to 9 — `nix/brain-modules.nix` refuses any
    /// other); `0` is a mark that is one. The core reads it from the
    /// MANIFEST, never from what a module published (`ronoctua_brain_daemon::hub`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub instances: u8,
    /// DECLARED: whether it may be drawn AT REST — published in
    /// [`Tone::Quiet`], saying only that all is well. The modeline at rest is
    /// silent (`AGENTS.md`), so the core drops a quiet mark that does not
    /// declare this, and the loader lets only the marks the owner approved
    /// declare it (`resting-marks-are-the-approved-ones`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub resting: bool,
    /// Which of its instances this is, from 1; `0` for a mark that is one.
    /// Drawn as the digit after the glyph, where a count would be — so an
    /// instance counts nothing (`Manifest::refusal_of`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub instance: u8,
    /// The instance's name as the PERSON gave it ("work", an account's
    /// name) — data, like what a person typed in a reminder, so it is drawn
    /// as it is: the hint's `$label` (the instance's number when there is
    /// none). Only an instance has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<Word>,
}

/// `skip_serializing_if` for the counts a mark leaves at zero: a mind written
/// before they existed reads the same, and one that has none says nothing.
const fn is_zero(n: &u8) -> bool {
    *n == 0
}

/// The same, for the flag a mark leaves off.
const fn is_false(b: &bool) -> bool {
    !*b
}

impl MarkId {
    /// A declaration — what the generated `mark::*` constants are.
    #[must_use]
    pub const fn of(name: &'static str, glyph: &'static str, hint: &'static str) -> Self {
        Self::declared(name, glyph, hint, 0, false)
    }

    /// A declaration with its instances and whether it may rest — what the
    /// generator writes for a mark whose manifest says `instances` or
    /// `resting` ([`MarkId::of`] for every other).
    #[must_use]
    pub const fn declared(
        name: &'static str,
        glyph: &'static str,
        hint: &'static str,
        instances: u8,
        resting: bool,
    ) -> Self {
        Self {
            name: Cow::Borrowed(name),
            glyph: Cow::Borrowed(glyph),
            hint: Cow::Borrowed(hint),
            opens: None,
            instances,
            resting,
            instance: 0,
            label: None,
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

    /// The same mark as its instance `n` (from 1), named `label` by the
    /// person — the third account of nine, say. The core refuses an `n` its
    /// manifest does not declare (`ronoctua_brain_daemon::hub`), so a module cannot draw a
    /// tenth.
    ///
    /// A BLANK LABEL IS NO LABEL: an account the person left unnamed is
    /// named by its number in the hint, never by a hole in the sentence.
    #[must_use]
    pub fn numbered(&self, n: u8, label: Option<String>) -> Self {
        Self {
            instance: n,
            label: label.filter(|l| !l.trim().is_empty()).map(Cow::Owned),
            ..self.clone()
        }
    }

    /// Its name inside its module, as the segment carries it: `full`, or
    /// `claude.3` for an instance — the part after `brain:<module>/`.
    #[must_use]
    pub fn local(&self) -> Cow<'_, str> {
        if self.instance == 0 {
            Cow::Borrowed(self.name.as_ref())
        } else {
            Cow::Owned(format!("{}.{}", self.name, self.instance))
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
    /// The generation of `Module::screen`: it moves whenever the screen
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
    // THE 2026-10-05 WAVE: a mark's instance and its label, after its anchor.
    // ── anchor: L11 ──
    // (They live on the id — `MarkId::instance` and `MarkId::label`, set with
    // `MarkId::numbered` — beside everything else the segment is made of; see
    // `MarkId`. A mark with an instance names no `act`: the core refuses it,
    // since a module action would not know which instance was clicked.)
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
/// (`Module::screen`), never carried in the mind.
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
        starts_off: false,
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

        // A MODULE THAT SHIPS OFF is not enabled until the person turns it on —
        // the one default a saved entry still overrides, like every other.
        let off = Manifest {
            starts_off: true,
            ..FIXTURE
        };
        assert!(!off.defaults().enabled);
        let off_watch = Manifest {
            schedule: None,
            ..off
        };
        assert!(!off_watch.defaults().enabled);
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

    /// FAILS IF: an instance or its label is lost on the socket, a mark that
    /// is one starts saying `instance`, or a mind written before the fields
    /// existed stops reading.
    #[test]
    fn an_instance_crosses_with_its_label_and_a_plain_mark_says_nothing_of_it() {
        let declared = MarkId::declared("seat", "fixture-seat", "fixture-mark-seat", 9, true);
        let third = declared
            .opening("fixture-open")
            .numbered(3, Some("work".to_owned()));
        assert_eq!((third.instance, third.label.as_deref()), (3, Some("work")));
        assert_eq!((third.instances, third.resting), (9, true));
        assert_eq!(third.local(), "seat.3");
        // A BLANK NAME IS NONE: the hint then names the instance by its
        // number, never `Seat  is taken`.
        for blank in ["", "   ", "\t"] {
            assert_eq!(declared.numbered(2, Some(blank.to_owned())).label, None);
        }
        let wire = serde_json::to_string(&third).unwrap();
        assert_eq!(serde_json::from_str::<MarkId>(&wire).unwrap(), third);

        let plain = FIXTURE.marks[0].clone();
        assert_eq!(plain.local(), "full");
        let old = serde_json::to_string(&plain).unwrap();
        for absent in ["instance", "label", "resting"] {
            assert!(!old.contains(absent), "{absent} in {old}");
        }
        // AN OLD LINE, with none of the four: the mark that is one.
        let back: MarkId =
            serde_json::from_str(r#"{"name":"full","glyph":"g","hint":"h"}"#).unwrap();
        assert_eq!((back.instances, back.resting, back.instance), (0, false, 0));
        assert_eq!(back.label, None);
    }

    /// FAILS IF: the core would show a mark its manifest does not let it —
    /// an undeclared name, an instance past the declared number or on a mark
    /// that is one, a label with no instance, an action or a count on one
    /// instance, a quiet mark that was not declared to rest, or one mark
    /// twice — or if it refuses one the manifest does let it show.
    #[test]
    fn a_mark_is_shown_only_as_its_manifest_declares_it() {
        static MARKS: [MarkId; 2] = [
            MarkId::of("full", "fixture-full", "fixture-mark-full"),
            MarkId::declared("seat", "fixture-seat", "fixture-mark-seat", 9, true),
        ];
        let manifest = Manifest {
            marks: &MARKS,
            ..FIXTURE
        };
        let mark = |id: MarkId, tone: Tone| Mark {
            id,
            tone,
            count: 0,
            act: None,
        };
        let full = MARKS[0].clone();
        let seat = MARKS[1].clone();
        // SHOWN: a warning on the mark that is one, and each instance from
        // the first to the ninth — quiet ones included, since `seat` rests.
        assert_eq!(
            manifest.refusal_of(&mark(full.clone(), Tone::Warning), &[]),
            None
        );
        for n in 1..=9 {
            let one = mark(seat.numbered(n, Some(format!("n{n}"))), Tone::Quiet);
            assert_eq!(manifest.refusal_of(&one, &[]), None, "instance {n}");
        }
        // REFUSED, each for its own reason.
        let undeclared = mark(MarkId::of("gone", "fixture-gone", "h"), Tone::Danger);
        let tenth = mark(seat.numbered(10, None), Tone::Warning);
        let zeroth = mark(seat.clone(), Tone::Warning);
        let numbered_one = mark(full.numbered(1, None), Tone::Warning);
        let labelled_one = Mark {
            id: MarkId {
                label: Some("x".into()),
                ..full.clone()
            },
            ..mark(full.clone(), Tone::Warning)
        };
        let acted = Mark {
            act: Some("forget".into()),
            ..mark(seat.numbered(2, None), Tone::Warning)
        };
        let counted = Mark {
            count: 4,
            ..mark(seat.numbered(2, None), Tone::Warning)
        };
        let resting = mark(full, Tone::Quiet);
        // A MODULE THAT DECLARED ITS OWN BOUNDS on the id it published is
        // read against its manifest all the same.
        let forged = mark(
            MarkId::declared("full", "fixture-full", "fixture-mark-full", 0, true),
            Tone::Quiet,
        );
        for (what, refused) in [
            ("an undeclared mark", undeclared),
            ("a tenth instance", tenth),
            ("instance zero of a mark with instances", zeroth),
            ("an instance of a mark that is one", numbered_one),
            ("a label with no instance", labelled_one),
            ("an action on one instance", acted),
            ("a count on one instance", counted),
            ("a quiet mark that does not rest", resting),
            ("a rest the id gave itself", forged),
        ] {
            assert!(
                manifest.refusal_of(&refused, &[]).is_some(),
                "{what} was shown"
            );
        }
        let first = mark(seat.numbered(1, None), Tone::Warning);
        let again = mark(seat.numbered(1, Some("other".to_owned())), Tone::Danger);
        let second = mark(seat.numbered(2, None), Tone::Warning);
        assert!(manifest.refusal_of(&again, &[&first]).is_some(), "twice");
        assert_eq!(manifest.refusal_of(&second, &[&first]), None);
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
}
