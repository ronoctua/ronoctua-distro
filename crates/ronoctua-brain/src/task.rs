//! What a run is made of: when it is due, what may hold it, what it did, and
//! the sheet its unit writes.
//!
//! # Where the trait went
//!
//! This file held THE DOOR — the `Task` trait every feature implemented. The
//! door is now the daemon's contract (`ronoctua_brain_daemon::module`): a
//! module answers `Module`, and a module that runs on a schedule answers
//! `Scheduled`, which is the old `Task` minus what the manifest declares (its
//! id, its defaults, its countdown, its kind and its gate). What stayed here
//! is the vocabulary both halves speak — [`Due`], [`Verdict`], [`Hold`],
//! [`Step`], [`Outcome`], [`RunRecord`] — and the [`Sheet`] a unit writes;
//! the watching of that file is the daemon's (`ronoctua_brain_daemon::task`).
//!
//! # What a module does NOT decide
//!
//! - **Whether anything else is running.** At most one run at a time, and what
//!   comes due meanwhile waits in the queue. That is a property of the
//!   daemon's runner, so every module inherits it without writing a line.
//! - **Whether the machine can afford it.** RAM, processor and battery are the
//!   general gate, asked by the runner before `Scheduled::needs` is reached.
//! - **How the person is warned.** The announcement and its countdown belong to
//!   the runner; the manifest says how many seconds and nothing else.
//! - **What the person reads.** Everything that leaves here is a WORD — an id,
//!   a hold's name, a step's id — and whoever has the bundle makes a sentence
//!   out of it (invariant 8).
//!
//! # A hold is recorded and published, never swallowed
//!
//! When the gate refuses, the reason goes on the screen: *"waiting: the RAM is
//! busy"*. A scheduler that silently does nothing is a scheduler nobody trusts,
//! and the first thing somebody does with one is turn it off and do the job by
//! hand. That is why [`Verdict::Hold`] carries a [`Hold`] and not a `bool`.

use serde::{Deserialize, Serialize};

use crate::moment::Moment;

/// Whether a module repeats, happens once, or only watches — the contract's
/// [`crate::module::Kind`], named here too so every `task::Kind` keeps
/// meaning what it meant.
pub use crate::module::Kind;

/// Whether a task wants to run, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Due {
    /// Not at all, and the WORD says why — `off`, `no-schedule`. The screen
    /// draws it as `brain-why-<word>`, so a task that goes quiet always has a
    /// line explaining itself.
    Not {
        /// The word, English and kebab-case.
        why: &'static str,
    },
    /// At this instant, and not before it.
    At(Moment),
    /// Now — it is overdue, or somebody asked.
    Now,
}

/// The answer to "can this run right now".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Go ahead.
    Go,
    /// Not yet, and this is what is in the way.
    Hold(Hold),
}

/// What is in the way — a NAME, never a sentence.
///
/// The screen draws `brain-hold-<word>` and the person reads their own
/// language (invariant 8). An enum and not a string for the reason the rest of
/// this repository gives: a typo in a string compiles and fails in silence in
/// front of somebody.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Hold {
    /// The machine's memory is spoken for.
    Ram,
    /// Something is waiting for a processor.
    Cpu,
    /// It is running off the battery, and this task compiles.
    Battery,
    /// Something of the brain is already running.
    ///
    /// The queue's own answer, and the one every future feature inherits: this
    /// is decided by the runner, not by any task.
    Busy,
    /// The network is not up, or is metered.
    ///
    /// NOTHING PRODUCES THIS YET, and the omission is deliberate rather than
    /// forgotten: what knows about connections is `ronoctua-network`, the only
    /// crate that speaks NetworkManager, and the brain will ask it rather than
    /// grow a second answer. The case is here so the day it does, the screen,
    /// the journal and the two `.ftl` already have a word for it.
    Network,
    /// The machine is going to sleep, or has just come back.
    ///
    /// Nothing produces this yet either; it would come from logind, through
    /// whoever already holds that connection.
    Asleep,
}

impl Hold {
    /// The word the screen looks up, as `brain-hold-<word>`.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Ram => "ram",
            Self::Cpu => "cpu",
            Self::Battery => "battery",
            Self::Busy => "busy",
            Self::Network => "network",
            Self::Asleep => "asleep",
        }
    }
}

/// One thing a module's run does, as the screen draws it.
///
/// A `&'static` table on the module (`Module::steps`), and never something read from the
/// preferences: the person may switch a step OFF, which is a set of ids, and
/// nothing they can write introduces a command. A configuration file that could
/// name what runs would be a configuration file that can run anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    /// English, kebab-case. `brain-step-<id>` names it on screen and
    /// `brain-step-<id>-does` is the dim line under the cursor.
    pub id: &'static str,
    /// What it really is, for the line under the cursor. DATA — it is a command
    /// line, and a command line is not translated.
    pub command: &'static str,
    /// Whether it needs root. Drawn, because "what is about to happen to my
    /// machine" is a question with a different weight when the answer is yes.
    pub needs_root: bool,
    /// Whether the distro ships it switched on. A step the person turned off
    /// differs from this, and the screen says which of the two it is looking at.
    pub on_by_default: bool,
    /// The text key of its name, so no screen builds one out of the id: the
    /// module that owns the step declares it (a generated table, for the one
    /// module with steps today) and it crosses the wire beside the id.
    pub name_key: &'static str,
    /// The text key of the line under the cursor, declared the same way.
    pub does_key: &'static str,
}

/// How one step ended, as the unit reported it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct StepOutcome {
    /// Which step. The same id [`Step::id`] carries.
    pub id: String,
    /// Whether it worked.
    pub ok: bool,
    /// How long it took, in seconds.
    pub seconds: u32,
    /// What it said when it failed, verbatim. Empty when nothing did.
    pub trouble: String,
}

/// What became of a whole run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It ran, and here is what each step did.
    Done(Vec<StepOutcome>),
    /// It stopped somewhere.
    Failed {
        /// The step it failed at, by id. Empty when the failure was not a
        /// step's — the unit would not start, the run went runaway.
        at: String,
        /// Why, verbatim when it came from the machine, and a WORD of ours
        /// (`runaway`, `aborted`, `no-systemctl`) when the brain decided. The
        /// screen tells them apart because the word never contains a space.
        why: String,
    },
    /// It did not run, and this says why.
    Skipped {
        /// A word of ours: `off`, `held`, `postponed`.
        why: String,
    },
}

/// The three shapes of an outcome, as the wire and the screen carry them.
///
/// A flat enum beside [`Outcome`] because a record cannot hold a Rust enum with
/// payloads across the WIT, and because the screen wants exactly this: three
/// colours and three words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutcomeKind {
    /// It did not run.
    ///
    /// THE DEFAULT, and deliberately the harmless one: a record built from a
    /// file that lost a field must not read as "it worked".
    #[default]
    Skipped,
    /// It ran and everything worked.
    Done,
    /// It ran and something did not.
    Failed,
    /// Nothing ran: the machine was in no state for it.
    ///
    /// Not a failure — the moment came, the gate said no, and the next moment
    /// will be along. It is written ONCE per episode and not once per tick:
    /// the runner looks every thirty seconds, so a line per look would make
    /// the activity log a wall of "RAM is busy" with everything that actually
    /// happened buried in it.
    Held,
    /// Nothing ran: a module wrote down something it NOTICED (the daemon's
    /// `Ctx::record`) — a process the kernel killed, an error in the journal.
    ///
    /// Like `Held`, it is not a run and moves no schedule
    /// ([`crate::journal::schedules`]). What it says is in
    /// [`RunRecord::says`], with its values.
    Noted,
}

impl OutcomeKind {
    /// The word the screen looks up, as `brain-outcome-<word>`.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Held => "held",
            Self::Noted => "noted",
        }
    }
}

/// One run, as the journal keeps it and the screen draws it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct RunRecord {
    /// Which task.
    pub task: String,
    /// When it started.
    pub began: Moment,
    /// When it ended. Equal to `began` for something that never started.
    pub ended: Moment,
    /// Which of the three shapes.
    pub kind: OutcomeKind,
    /// The step it failed at, by id. Empty when there is none.
    pub at: String,
    /// A word of OURS when the brain decided — `runaway`, `aborted`, `off`.
    /// Empty when the reason came from the machine, in which case
    /// [`RunRecord::trouble`] carries it.
    pub why: String,
    /// What the machine said, verbatim. Empty when it said nothing.
    pub trouble: String,
    /// What each step did.
    pub steps: Vec<StepOutcome>,
    /// What a `Noted` record says, as a text key. Empty for every other kind.
    pub says: String,
    /// The values of that sentence, formatted by whoever draws it.
    pub args: crate::module::Args,
}

impl RunRecord {
    /// A record of something a module noticed, at `at`.
    #[must_use]
    pub fn noted(module: &str, at: Moment, noted: crate::module::Noted) -> Self {
        Self {
            task: module.to_owned(),
            began: at,
            ended: at,
            kind: OutcomeKind::Noted,
            says: noted.says.into_owned(),
            args: noted.args,
            ..Self::default()
        }
    }

    /// A record of an outcome that already happened.
    #[must_use]
    pub fn of(task: &str, began: Moment, ended: Moment, outcome: Outcome) -> Self {
        let mut me = Self {
            task: task.to_owned(),
            began,
            ended,
            ..Self::default()
        };
        match outcome {
            Outcome::Done(steps) => {
                me.kind = OutcomeKind::Done;
                me.steps = steps;
            }
            Outcome::Failed { at, why } => {
                me.kind = OutcomeKind::Failed;
                me.at = at;
                // A word of ours has no space in it; anything else came from
                // the machine and is drawn as it came. One rule, applied here,
                // so the screen does not have to guess twice.
                if why.contains(' ') {
                    me.trouble = why;
                } else {
                    me.why = why;
                }
            }
            Outcome::Skipped { why } => {
                me.kind = OutcomeKind::Skipped;
                me.why = why;
            }
        }
        me
    }
}

// ---------------------------------------------------------------------------
// The report file — how the brain learns what the unit did.
// ---------------------------------------------------------------------------

/// What the unit writes as it goes, and the only thing the brain reads back.
///
/// # Why a file, and not the journal
///
/// Because `journalctl` on a tick is a process spawned every beat to parse
/// prose that upstream is free to reword, and `systemctl show` in a loop is the
/// same cost with a smaller answer. A file the unit writes is one `stat` per
/// beat, it is structured, and re-reading it only when the mtime MOVES is the
/// device the file manager already uses to notice a directory changed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Sheet {
    /// When the unit started, as it saw it.
    pub began: Moment,
    /// When it finished. **Zero while it is still going** — a moment in 1970 is
    /// not a run that ended, and the alternative is a second field saying
    /// whether the first one means anything.
    pub ended: Moment,
    /// What each step did, appended as they end.
    pub steps: Vec<StepOutcome>,
    /// What went wrong overall, verbatim. Empty when nothing did.
    pub trouble: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_is_a_record_that_ran_nothing_and_says_what_it_noticed() {
        let r = RunRecord::noted(
            "fixture",
            500,
            crate::module::Noted::of("fixture-killed")
                .with("unit", crate::module::Arg::Text("a.service".to_owned())),
        );
        assert_eq!(r.kind, OutcomeKind::Noted);
        assert_eq!((r.began, r.ended), (500, 500));
        assert_eq!(r.says, "fixture-killed");
        assert_eq!(r.args.len(), 1);
        assert!(r.steps.is_empty() && r.why.is_empty() && r.trouble.is_empty());
        assert_eq!(OutcomeKind::Noted.word(), "noted");
    }

    #[test]
    fn a_word_of_ours_and_a_machines_sentence_do_not_end_up_in_the_same_field() {
        // The screen draws one through the bundle and the other verbatim.
        // Putting both in one field is how `brain-why-runaway` ends up printed
        // in front of somebody, in brackets.
        let ours = RunRecord::of(
            "made-up",
            1,
            2,
            Outcome::Failed {
                at: String::new(),
                why: "runaway".to_owned(),
            },
        );
        assert_eq!(ours.why, "runaway");
        assert!(ours.trouble.is_empty());

        let theirs = RunRecord::of(
            "made-up",
            1,
            2,
            Outcome::Failed {
                at: "build".to_owned(),
                why: "error: build of drv failed".to_owned(),
            },
        );
        assert!(theirs.why.is_empty());
        assert_eq!(theirs.at, "build");
        assert_eq!(theirs.trouble, "error: build of drv failed");
    }
}
