//! THE DOOR: what a feature of the brain is, and what it has to answer.
//!
//! # Five methods, and four of them are pure
//!
//! [`Task::due`], [`Task::needs`], [`Task::countdown`] and [`Task::steps`] take
//! no clock, open no file and spawn nothing. That is not tidiness — it is what
//! lets a whole year of scheduling be tested in milliseconds, and it is the
//! reason a bug in "when" can be found without a machine that is busy. Only
//! [`Task::run`] touches the world, and it runs on the runner's thread, alone,
//! where it is allowed to block.
//!
//! # What a task does NOT decide
//!
//! - **Whether anything else is running.** At most one task runs at a time and
//!   what comes due meanwhile waits in the queue. That is a property of
//!   [`crate::runner`], so every feature added later inherits it without
//!   writing a line — a task that implemented its own lock would be one that
//!   forgot to.
//! - **Whether the machine can afford it.** RAM, processor and battery are the
//!   general gate, asked by the runner before [`Task::needs`] is even reached.
//!   A task's own `needs` is for what only IT knows.
//! - **How the person is warned.** The announcement and its countdown belong to
//!   the runner; the task says how many seconds it wants and nothing else.
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

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::moment::Moment;
use crate::prefs::Prefs;
use crate::pulse::Pulse;

/// One thing the machine does by itself.
///
/// The five places to touch when adding one are listed in this crate's own
/// header, and they are five because of this trait: everything else — the
/// queue, the gate, the announcement, the journal, the socket and the screen —
/// is already written and already watching.
pub trait Task: Send + Sync {
    /// The id: English, kebab-case, stable.
    ///
    /// It is the key of everything a person sees (`brain-task-<id>`), the key
    /// of the entry in the preferences file, and the name in the journal.
    /// Renaming one is renaming all three, which is why the registry refuses a
    /// malformed one at construction rather than letting it reach a screen.
    fn id(&self) -> &'static str;

    /// When it wants to run, given what the person chose and when it last did.
    ///
    /// PURE: no clock — `now` is handed in — and no file. `last` is `None` when
    /// it has never run, which is a first run and not an overdue one.
    fn due(&self, now: Moment, prefs: &Prefs, last: Option<Moment>) -> Due;

    /// Whether it may run right now, beyond the general gate.
    ///
    /// For what only this task knows. The general refusals — a busy machine, a
    /// battery — are the runner's and are not repeated here; a task that
    /// repeated them would be a second copy of a policy the person configured
    /// in one place.
    fn needs(&self, pulse: &Pulse, prefs: &Prefs) -> Verdict;

    /// How many seconds the screen warns before it starts.
    ///
    /// `0` means it just runs, which is the right answer for anything that
    /// interrupts nothing.
    fn countdown(&self, prefs: &Prefs) -> u16;

    /// Whether it repeats, or happens once and is then gone.
    ///
    /// # Why this exists before there is a one-shot task
    ///
    /// Because the list has to be able to SAY which is which before the first
    /// one arrives. Add it the day a one-shot lands and every row on the page
    /// changes meaning at once — a list that never distinguished two kinds is
    /// a list a person has read as one kind. A glyph says it, and a colour
    /// says it again, from the day there is nothing to distinguish.
    ///
    /// Every task today is a routine, which is why this has a default.
    fn kind(&self) -> Kind {
        Kind::Routine
    }

    /// What the DISTRO chose for it, when the person has chosen nothing.
    ///
    /// # Why a task gets a say in its own cadence
    ///
    /// Because [`crate::prefs::TaskPrefs::default`] is one struct for every
    /// task — weekly, in the afternoon, with a minute of countdown — and that
    /// is right for the one that rebuilds the system and wrong for a sweep that
    /// interrupts nothing and wants a night every five days.
    ///
    /// Answering it inside `due` instead would be a LIE ON SCREEN: the brain's
    /// row draws `interval_days` and `part_of_day` as `Prefs` resolved them, so
    /// it would say "every seven days, afternoon" about a task running every
    /// five at night. One answer, asked here, and the person's file still wins
    /// over it — [`crate::prefs::Prefs::of_task`].
    ///
    /// The default is the distro's own default, which is what every task that
    /// has no opinion should have.
    fn defaults(&self) -> crate::prefs::TaskPrefs {
        crate::prefs::TaskPrefs::default()
    }

    /// What it runs, in order — DATA, so the screen can draw it and the person
    /// can switch one off.
    ///
    /// The default is empty: a task with nothing to show is the ordinary case.
    /// What the steps MEAN is the flake's, not this crate's: the brain names
    /// them and reads what came back (invariant 5).
    fn steps(&self) -> &'static [Step] {
        &[]
    }

    /// Do it. On the runner's thread, alone, and it may block.
    ///
    /// The [`Report`] is how it learns what happened: it is the file the unit
    /// writes as it goes, and watching it is a `stat` on a beat — not
    /// `journalctl`, not `systemctl show` in a loop.
    fn run(&self, report: &Report) -> Outcome;
}

/// Something the brain wants said, without anybody having asked.
///
/// # Why a task can raise one at all
///
/// Because some tasks are WATCHES. A sweep and a rebuild report what they did
/// and are done with it; a watch's whole output is a sentence, and a sentence
/// that only appeared on a screen somebody had to open would be a watch nobody
/// hears. The owner asked for the Emacs one to reach the band.
///
/// It is not a notification: it does not go to the bus, it does not queue, it
/// does not expire on a clock. It is one line on the footer's own surface —
/// the same line the countdown uses, which is already there and already
/// clickable (invariant 14: there is no second chrome).
///
/// # Why it carries keys and not sentences
///
/// A notice crosses to a component and to the footer, and neither of them is
/// allowed to invent words: `says` and `act` are text keys, translated by
/// whoever draws. The same rule every other string on this path follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Notice {
    /// Whose it is.
    pub task: String,
    /// What to say, as a text key.
    pub says: String,
    /// The COMMAND a button runs, when there is something to offer.
    ///
    /// `None` is a notice that only tells. That is not a lesser kind: the
    /// standalone-Emacs one has no button on purpose, because what is open is
    /// somebody's work and nothing here knows whether it is saved.
    pub act: Option<String>,
}

/// Whether a task repeats, or happens once and is then gone.
///
/// A `Copy` enum with no payload, for the reason [`OutcomeKind`] gives: what
/// the screen wants is a glyph and a colour, and a shape that carried anything
/// more would be a shape the WIT has to carry too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// It comes round again.
    #[default]
    Routine,
    /// It happens once and then it is gone.
    Once,
}

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

/// One thing a task runs, as the screen draws it.
///
/// A `&'static` table on the task, and never something read from the
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
}

impl RunRecord {
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

/// The file, and the patience to watch it with.
#[derive(Debug, Clone)]
pub struct Report {
    path: PathBuf,
    patience: Duration,
    beat: Duration,
    stop: Arc<AtomicBool>,
    rehearsal: bool,
    /// What the run wants said on the brain's line, if anything.
    ///
    /// # Why it comes back through here
    ///
    /// Because `Task::run` takes `&self` and returns an `Outcome`, and a
    /// notice is neither: it is not what the run DID. The two other ways out
    /// were a fourth `Outcome` variant, which would make every reader of an
    /// outcome learn about notices, and a second method on the trait, which a
    /// task could not answer because it has nowhere to remember what it just
    /// found.
    ///
    /// The report is already the channel a run talks back through, and it is
    /// already shared across the thread that watches. One more slot on it
    /// costs nothing and adds no vocabulary anywhere else.
    said: Arc<std::sync::Mutex<Option<Notice>>>,
}

/// How watching ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Watched {
    /// The unit said it finished.
    Ended(Sheet),
    /// Nothing changed for as long as the patience lasted.
    ///
    /// NOT the same as failed: the unit is systemd's and may well still be
    /// going. What the brain knows is that it stopped hearing, and the record
    /// says exactly that rather than inventing an outcome.
    Runaway(Sheet),
    /// Somebody asked for it to stop.
    Aborted(Sheet),
}

impl Report {
    /// A report at a path, watched on a beat, with a patience and a switch.
    #[must_use]
    pub fn at(path: PathBuf, patience: Duration, beat: Duration, stop: Arc<AtomicBool>) -> Self {
        Self {
            path,
            patience,
            beat,
            stop,
            rehearsal: false,
            said: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// The same run, as a rehearsal.
    #[must_use]
    pub fn rehearsing(mut self) -> Self {
        self.rehearsal = true;
        self
    }

    /// Whether this run is a rehearsal — a dry run that changes nothing.
    ///
    /// # Why it is on the REPORT and not on the task
    ///
    /// Because it is a property of this run and not of the feature. A task is a
    /// stateless description of something the machine does; putting a mode on
    /// it would mean two runs of the same feature writing to the same field,
    /// and the second one reading whatever the first left there. The report is
    /// what describes THIS run — where it writes, how long it is given, and
    /// whether it is the real thing.
    ///
    /// It is the idiom the installer already has: `nix run .#install` rehearses
    /// and only `--execute` acts.
    #[must_use]
    pub const fn is_rehearsal(&self) -> bool {
        self.rehearsal
    }

    /// Where the file is — what the unit is told, and what a person can `cat`.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Says something on the brain's line when this run is over.
    ///
    /// `None` clears it, which is how a watch that finds nothing wrong takes
    /// its own line down — a notice that had to be cancelled by a second call
    /// would be a line that stayed up after the thing it was about was fixed.
    pub fn say(&self, notice: Option<Notice>) {
        if let Ok(mut slot) = self.said.lock() {
            *slot = notice;
        }
    }

    /// What it said, for the runner to publish.
    #[must_use]
    pub fn said(&self) -> Option<Notice> {
        self.said.lock().ok().and_then(|s| s.clone())
    }

    /// What the file says now, or `None` when it is not there or is half
    /// written.
    ///
    /// A half-written file is a normal sight here: the unit rewrites it as each
    /// step ends, and a read can land in the middle. `None` is the honest
    /// answer, and the next beat has the whole of it — which is why nothing
    /// here treats a parse failure as a failure of the run.
    #[must_use]
    pub fn sheet(&self) -> Option<Sheet> {
        let raw = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// What the file looks like RIGHT NOW, to be handed to [`Report::watch`].
    ///
    /// # Why the previous report is marked and not deleted
    ///
    /// Because the file is not ours to delete: the unit runs as root and the
    /// brain runs as the person. Deleting was the first shape of this, and it
    /// works only for as long as the unit is a user one — the day it is not,
    /// `remove_file` fails with a permission error and a run that was about to
    /// work is reported as a failure to clean up.
    ///
    /// Marking answers the same question — *is what I am reading THIS run's?* —
    /// without touching anything: whatever is there now belongs to the previous
    /// run, and the first change after the mark is the new one. Without either,
    /// the first look finds the old file, sees that it ended, and reports a run
    /// that finished before it began.
    #[must_use]
    pub fn mark(&self) -> Option<std::time::SystemTime> {
        std::fs::metadata(&self.path)
            .ok()
            .and_then(|m| m.modified().ok())
    }

    /// Watches until the unit says it ended, the patience runs out, or somebody
    /// aborts.
    ///
    /// `since` is what [`Report::mark`] gave before the unit was asked to
    /// start. The patience is measured from the LAST CHANGE and not from the
    /// beginning: a rebuild that is still writing steps is a rebuild that is
    /// alive, however long it takes. What the ceiling catches is silence.
    #[must_use]
    pub fn watch(&self, since: Option<std::time::SystemTime>) -> Watched {
        let mut last_change = Instant::now();
        let mut seen = since;
        let mut sheet = Sheet::default();
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return Watched::Aborted(sheet);
            }
            let mtime = std::fs::metadata(&self.path)
                .ok()
                .and_then(|m| m.modified().ok());
            if mtime != seen {
                seen = mtime;
                last_change = Instant::now();
                if let Some(fresh) = self.sheet() {
                    sheet = fresh;
                    if sheet.ended != 0 {
                        return Watched::Ended(sheet);
                    }
                }
            }
            if last_change.elapsed() >= self.patience {
                return Watched::Runaway(sheet);
            }
            std::thread::sleep(self.beat);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-brain-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn a_report(path: PathBuf, stop: Arc<AtomicBool>) -> Report {
        // Milliseconds, because this is a test and not a rebuild: what is being
        // proved is the shape of the watching, not how long it waits.
        Report::at(
            path,
            Duration::from_millis(300),
            Duration::from_millis(5),
            stop,
        )
    }

    #[test]
    fn a_report_left_from_last_time_is_not_this_runs_answer() {
        // THE DEFECT THIS CATCHES: the first look finding the PREVIOUS run's
        // file, seeing `ended` already set, and reporting a run that finished
        // before it started — which on screen is an update that "worked" and
        // never happened. The mark is what tells the two apart WITHOUT
        // deleting a file that belongs to root.
        let dir = scratch("stale");
        let path = dir.join("report.json");
        std::fs::write(&path, r#"{"began":1,"ended":2,"steps":[],"trouble":""}"#).expect("write");
        let report = a_report(path, Arc::new(AtomicBool::new(false)));
        let before = report.mark();
        assert!(
            before.is_some(),
            "the mark did not see the file that exists"
        );
        // The old file says it ended, and it is NOT taken as this run's answer:
        // nothing changed since the mark, so the watch waits and gives up
        // saying it heard nothing.
        assert!(matches!(report.watch(before), Watched::Runaway(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn silence_is_runaway_and_says_so_instead_of_inventing_an_outcome() {
        let dir = scratch("runaway");
        let report = a_report(dir.join("report.json"), Arc::new(AtomicBool::new(false)));
        // Nothing ever writes the file. The unit may well still be going — what
        // the brain knows is that it stopped hearing, and that is what it says.
        assert_eq!(report.watch(None), Watched::Runaway(Sheet::default()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_run_that_ends_comes_back_with_what_each_step_did() {
        let dir = scratch("ended");
        let path = dir.join("report.json");
        let report = a_report(path.clone(), Arc::new(AtomicBool::new(false)));
        let writing = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            std::fs::write(
                &path,
                r#"{"began":10,"ended":0,"steps":[{"id":"channels","ok":true,"seconds":3,"trouble":""}],"trouble":""}"#,
            )
            .expect("middle");
            std::thread::sleep(Duration::from_millis(20));
            std::fs::write(
                &path,
                r#"{"began":10,"ended":40,"steps":[{"id":"channels","ok":true,"seconds":3,"trouble":""}],"trouble":""}"#,
            )
            .expect("the end");
        });
        let Watched::Ended(sheet) = report.watch(None) else {
            panic!("it did not see the end");
        };
        writing.join().expect("thread");
        assert_eq!(sheet.ended, 40);
        assert_eq!(sheet.steps.len(), 1);
        assert_eq!(sheet.steps[0].id, "channels");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn aborting_comes_back_at_once_and_keeps_what_it_had_seen() {
        let dir = scratch("abort");
        let stop = Arc::new(AtomicBool::new(true));
        let report = a_report(dir.join("report.json"), stop);
        assert!(matches!(report.watch(None), Watched::Aborted(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_word_of_ours_and_a_machines_sentence_do_not_end_up_in_the_same_field() {
        // The screen draws one through the bundle and the other verbatim.
        // Putting both in one field is how `brain-why-runaway` ends up printed
        // in front of somebody, in brackets.
        let ours = RunRecord::of(
            "system-update",
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
            "system-update",
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
