//! What happened — the half that is state, not configuration.
//!
//! # Why it is rewritten whole
//!
//! Because there are a few dozen records of a few hundred bytes, written when
//! something happens — which on this machine is once a week. An append-only log
//! would need a second stream for the records that CHANGE (a run ends, and the
//! record of it grows a result), and rebuilding the state would mean replaying
//! it. Rewriting is a few kilobytes and no second format.
//!
//! # Why a failed run — and a postponement — still move the schedule
//!
//! Because the alternative is a loop. If only a success counted, a rebuild
//! that fails at half past two would be tried again at twenty-nine minutes
//! past, and again, for the rest of the night — and the thing it is failing on
//! is usually not going to change in thirty seconds. A postponement is the
//! same shape from the other side: *not now* has to mean something, and if it
//! left the next moment where it was, the tick half a minute later would ask
//! again.
//!
//! **A HOLD IS NOT WRITTEN DOWN AT ALL.** The machine being busy at two in the
//! morning is not an event in this task's history — it is a state of the
//! machine, it is published on the row, and it must not push a weekly update a
//! week away. That is a property of [`crate::runner`], which journals a run and
//! never a refusal, and there is a test on it there.
//!
//! # Where the database would go
//!
//! Here. The whole surface is [`Journal::last`] and [`Journal::put`], and the
//! day the brain keeps a measurement per minute instead of a record per run,
//! what changes is what is behind those two calls — not a single caller.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::moment::Moment;
use crate::task::RunRecord;

/// The file the journal is written to, under `$XDG_STATE_HOME/ronoctua`.
///
/// The same name as the preferences file, in a different directory — the shape
/// `ronoctua-notify` already has, and for the same reason: what tells them
/// apart is the lifetime, and the lifetime is the directory.
pub const JOURNAL_FILE: &str = "brain.json";

/// How many runs are kept, across every task.
///
/// It was sixty-four, matched to the transfer queue's ceiling on finished
/// jobs. Two things moved it. The home page now shows the last [`ACTIVITY`]
/// things the brain did, so the list has to be at least that long to fill it;
/// and a five-minute watch was added, which at sixty-four would have wrapped
/// the whole list inside six hours and taken every other task's history with
/// it. Two hundred and fifty-six is a few days of the watch and still years of
/// everything else — and what protects the rare tasks is not this number, it
/// is the floor in [`Journal::put`].
///
/// The whole list crosses the socket on every change. At this size that is a
/// few tens of kilobytes, and the argument for sending the whole mind is in
/// `crate::wire`.
pub const HISTORY: usize = 256;

/// How many of a task's OWN runs are kept whatever else happens, and shown.
///
/// Ten, because the owner asked for ten. It is a floor on [`put`]'s pruning as
/// well as the number the screen draws: a task's row promises its last ten, and
/// a ceiling that could take them is a promise the screen cannot keep.
pub const RECENT: usize = 10;

/// How many records the home page's activity log shows.
///
/// A hundred, because the owner asked for a hundred. It is smaller than
/// [`HISTORY`] on purpose — what is kept and what is shown are two questions,
/// and the log scrolls, so showing fewer than are kept is a choice about the
/// list and not about the disk.
pub const ACTIVITY: usize = 100;

/// Every run the brain remembers, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Journal {
    /// Oldest first, so appending is a push and the ceiling drops from the
    /// front — which is the order a person reads a history in anyway.
    pub runs: Vec<RunRecord>,
}

impl Journal {
    /// An empty one.
    #[must_use]
    pub fn fresh() -> Self {
        Self::default()
    }

    /// When this task last DID something, as scheduling counts it.
    ///
    /// Every record counts, because every record is an event somebody or
    /// something decided: a run that worked, one that failed, one the person
    /// postponed. What is not here is a hold — see this module's header.
    #[must_use]
    pub fn last(&self, task: &str) -> Option<Moment> {
        // A WAIT IS NOT A RUN, and this is the one place where confusing them
        // would be expensive. `due` is `last + interval_days`, so a `Held`
        // counted here would push a weekly task a whole week every time the
        // machine happened to be busy at its moment — a machine that is busy
        // every night would postpone itself for ever, and the screen would
        // show a date moving away each time anybody looked.
        //
        // It is `runs` and not `latest` for exactly that reason: `latest` is
        // what the SCREEN draws — the last thing that happened to this task,
        // whatever it was — and this is what the SCHEDULE counts from. Two
        // questions, and they were one function until a hold became a record.
        self.runs
            .iter()
            .rev()
            .find(|r| r.task == task && schedules(r))
            .map(|r| r.began)
    }

    /// Every record, oldest first.
    ///
    /// The home page's activity log is the tail of this, and the cut is the
    /// caller's for the reason [`Self::recent`] gives: what this knows is how
    /// to answer a question, and how many to answer with is a decision about
    /// a screen.
    #[must_use]
    pub fn all(&self) -> &[RunRecord] {
        &self.runs
    }

    /// The last record of this task — what the screen draws.
    #[must_use]
    pub fn latest(&self, task: &str) -> Option<&RunRecord> {
        self.runs.iter().rev().find(|r| r.task == task)
    }

    /// The last few records of this task, NEWEST FIRST.
    ///
    /// # Why a task's own history and not the journal's
    ///
    /// Because the journal is one list for every task, capped at
    /// [`HISTORY`] — so on a machine where one task runs ten times a week and
    /// another once a month, the second one's last run falls off the end while
    /// its row still has to show it. Asking per task is what makes "the last
    /// five of THIS" a question with an answer.
    ///
    /// The owner asked for five, and the number is the caller's: what this
    /// knows is how to answer the question.
    #[must_use]
    pub fn recent(&self, task: &str, most: usize) -> Vec<RunRecord> {
        self.runs
            .iter()
            .rev()
            .filter(|r| r.task == task)
            .take(most)
            .cloned()
            .collect()
    }

    /// Writes one down, and drops the oldest past the ceiling — unless
    /// dropping it would take a task's last few with it.
    ///
    /// # Why the ceiling has a floor under it
    ///
    /// Because a flat ceiling on one shared list is a ceiling that empties the
    /// rare tasks first. The update runs weekly, the sweep every ten days, the
    /// trash every four, and a watch every five minutes — and the watch would
    /// fill the whole list inside a day, taking the update's last run with it.
    /// The row that shows it would then say "never", about a task that ran on
    /// Tuesday.
    ///
    /// So the oldest record is dropped only while its own task still has more
    /// than [`RECENT`] left. Every task keeps its last few for as long as it
    /// exists, and everything above that is first-in-first-out. The list can
    /// therefore be longer than [`HISTORY`] — by at most `RECENT` per task,
    /// which is a handful of records and the price of the row being honest.
    ///
    /// # And a task's last RUN is never dropped
    ///
    /// Because the floor counts records of every kind, and a watcher's notes
    /// are records: a module that runs weekly and notes what it hears would
    /// have its run pushed out by its own eleventh note, and [`Self::last`]
    /// would answer "never" — a module that comes due as if it had never run.
    /// The newest record the schedule counts from stays, whatever is above it.
    pub fn put(&mut self, record: RunRecord) {
        self.runs.push(record);
        while self.runs.len() > HISTORY {
            // THE OLDEST THAT CAN GO. Walking from the front, the first record
            // whose task would still have more than `RECENT` left behind it,
            // and which is not that task's last run.
            let at = {
                let mut count: std::collections::HashMap<&str, usize> =
                    std::collections::HashMap::new();
                let mut last_run: std::collections::HashMap<&str, usize> =
                    std::collections::HashMap::new();
                for (i, r) in self.runs.iter().enumerate() {
                    *count.entry(r.task.as_str()).or_default() += 1;
                    if schedules(r) {
                        last_run.insert(r.task.as_str(), i);
                    }
                }
                self.runs.iter().enumerate().position(|(i, r)| {
                    count[r.task.as_str()] > RECENT && last_run.get(r.task.as_str()) != Some(&i)
                })
            };
            let Some(at) = at else {
                // Every task is down to its floor. The list stays longer than
                // the ceiling, which is the trade this floor exists to make.
                break;
            };
            self.runs.remove(at);
        }
    }

    /// Reads the journal from an explicit directory.
    ///
    /// A root that is a PARAMETER, for the reason `notify::store` writes down:
    /// functions that found `$XDG_STATE_HOME` for themselves are what let a
    /// test write into the real state of whoever ran `cargo test`.
    #[must_use]
    pub fn read_from(dir: &Path) -> Self {
        let path = dir.join(JOURNAL_FILE);
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Self::fresh();
        };
        match serde_json::from_str::<Self>(&raw) {
            Ok(j) => j,
            Err(e) => {
                // Loud, and then it carries on with nothing. A journal that
                // stopped parsing means the brain forgot when it last ran and
                // will run again sooner than the person asked — which is worth
                // a line in the log, and is not worth refusing to start over.
                tracing::error!(
                    path = %path.display(),
                    error = %e,
                    "the brain's journal is unreadable; starting with no history"
                );
                Self::fresh()
            }
        }
    }

    /// Writes it into an explicit directory.
    ///
    /// # Errors
    ///
    /// The directory cannot be created, or the file cannot be written.
    pub fn write_to(&self, dir: &Path) -> std::io::Result<()> {
        // COMPACT, unlike the preferences: nobody edits this by hand, and the
        // one thing it should not be is large.
        let text = serde_json::to_string(self).map_err(std::io::Error::other)?;
        // AND ATOMICALLY, like every other store in this tree — see the same
        // line in `prefs::write_to`.
        ronoctua_xdg::write_atomically(dir, JOURNAL_FILE, text.as_bytes())
    }
}

/// Whether the schedule counts from this record: a run that worked, failed or
/// was put off — never a hold (the machine was busy) and never a note (a
/// module wrote down what it noticed).
fn schedules(r: &RunRecord) -> bool {
    !matches!(
        r.kind,
        crate::task::OutcomeKind::Held | crate::task::OutcomeKind::Noted
    )
}

/// Consecutive records that say the same thing, folded into one row of the
/// activity log.
///
/// # Why the log folds at all
///
/// Because a watch writes the same line as often as the thing it watches
/// happens, and a hold is written once per episode but an episode can come back
/// every night. Forty rows of "RAM is busy" bury the one row that says the
/// update failed. Folded, they are one row — "Last: <when>", how many times,
/// and the first of them — that opens into all of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// The newest of them: what the row draws.
    pub newest: RunRecord,
    /// How many records the row folds; `1` is one that stands alone.
    pub repeats: u32,
    /// When the OLDEST of them began.
    pub first: Moment,
    /// All of them, NEWEST FIRST, `newest` included — what the row opens into.
    pub members: Vec<RunRecord>,
}

/// The records, NEWEST FIRST, with every run of consecutive ones that say the
/// same thing folded into one [`Group`].
///
/// `runs` is OLDEST FIRST, as the journal keeps it. "The same thing" is the
/// same task, kind, step, word and sentence — not the same trouble text, which
/// carries a machine's numbers and would split every fold in two. PURE: the
/// daemon and the footer fold the same list the same way, and the cut is the
/// caller's.
#[must_use]
pub fn grouped(runs: &[RunRecord]) -> Vec<Group> {
    let same = |a: &RunRecord, b: &RunRecord| {
        a.task == b.task && a.kind == b.kind && a.at == b.at && a.why == b.why && a.says == b.says
    };
    let mut out: Vec<Group> = Vec::new();
    for r in runs.iter().rev() {
        match out.last_mut() {
            Some(g) if same(&g.newest, r) => {
                g.repeats = g.repeats.saturating_add(1);
                g.first = r.began;
                g.members.push(r.clone());
            }
            _ => out.push(Group {
                newest: r.clone(),
                repeats: 1,
                first: r.began,
                members: vec![r.clone()],
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::OutcomeKind;

    #[test]
    fn a_note_does_not_move_the_schedule() {
        // A WATCH WRITING WHAT IT NOTICED is not the task running: counted
        // here, every note would push its module a whole interval away.
        let mut j = Journal::fresh();
        j.put(ran("fixture", 100, OutcomeKind::Done));
        j.put(ran("fixture", 200, OutcomeKind::Noted));
        j.put(ran("fixture", 300, OutcomeKind::Held));
        assert_eq!(j.last("fixture"), Some(100));
        assert_eq!(j.latest("fixture").map(|r| r.began), Some(300));
    }

    #[test]
    fn what_says_the_same_thing_in_a_row_is_one_row() {
        let held = |at: Moment| {
            let mut r = ran("fixture", at, OutcomeKind::Held);
            r.why = "ram".to_owned();
            r
        };
        let runs = vec![
            ran("fixture", 10, OutcomeKind::Done),
            held(20),
            held(30),
            held(40),
            ran("other", 50, OutcomeKind::Done),
            held(60),
        ];
        let rows = grouped(&runs);
        // NEWEST FIRST, and a fold never crosses another task's record: the
        // hold after `other` is its own row, not a fourth repeat.
        assert_eq!(rows.len(), 4);
        assert_eq!((rows[0].newest.began, rows[0].repeats), (60, 1));
        assert_eq!(rows[1].newest.task, "other");
        assert_eq!((rows[2].newest.began, rows[2].repeats), (40, 3));
        assert_eq!(rows[2].first, 20, "the fold forgot where it began");
        assert_eq!(
            rows[2].members.iter().map(|r| r.began).collect::<Vec<_>>(),
            vec![40, 30, 20]
        );
        assert_eq!((rows[3].newest.began, rows[3].repeats), (10, 1));
        // A DIFFERENT WORD IS A DIFFERENT ROW, however alike the rest is.
        let mut other_word = held(70);
        other_word.why = "cpu".to_owned();
        let rows = grouped(&[held(60), other_word]);
        assert_eq!(rows.len(), 2);
        assert!(grouped(&[]).is_empty());
    }

    fn ran(task: &str, at: Moment, kind: OutcomeKind) -> RunRecord {
        RunRecord {
            task: task.to_owned(),
            began: at,
            ended: at + 1,
            kind,
            ..RunRecord::default()
        }
    }

    #[test]
    fn a_postponement_moves_the_schedule_the_same_way_a_run_does() {
        // "NOT NOW" HAS TO MEAN SOMETHING. If a postponement left the next
        // moment where it was, the tick half a minute later would ask again —
        // and the person would be answering the same question until the window
        // closed. What is NOT in here is a hold: the runner never writes one,
        // and its own test says so.
        let mut j = Journal::fresh();
        j.put(ran("made-up", 100, OutcomeKind::Done));
        j.put(ran("made-up", 200, OutcomeKind::Skipped));
        assert_eq!(j.last("made-up"), Some(200));
        assert_eq!(j.latest("made-up").map(|r| r.began), Some(200));
    }

    #[test]
    fn a_failed_run_counts_so_it_does_not_retry_every_half_minute() {
        let mut j = Journal::fresh();
        j.put(ran("made-up", 100, OutcomeKind::Done));
        j.put(ran("made-up", 300, OutcomeKind::Failed));
        assert_eq!(j.last("made-up"), Some(300));
    }

    #[test]
    fn the_tasks_do_not_read_each_others_history() {
        let mut j = Journal::fresh();
        j.put(ran("made-up", 100, OutcomeKind::Done));
        assert_eq!(j.last("something-else"), None);
    }

    #[test]
    fn the_ceiling_drops_the_oldest_and_the_file_round_trips() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-brain-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut j = Journal::fresh();
        for n in 0..(HISTORY + 10) {
            j.put(ran("made-up", n as Moment, OutcomeKind::Done));
        }
        assert_eq!(j.runs.len(), HISTORY);
        assert_eq!(
            j.runs[0].began, 10,
            "the ceiling dropped one from the wrong side"
        );
        j.write_to(&dir).expect("write");
        assert_eq!(Journal::read_from(&dir), j);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_task_keeps_its_last_run_however_much_it_notes() {
        // THE DEFECT THIS CATCHES: the floor counts records of every kind, so
        // a weekly module that noted what it heard lost its run to its own
        // eleventh note, and came due as if it had never run.
        let mut j = Journal::fresh();
        j.put(ran("watcher", 100, OutcomeKind::Done));
        for n in 0..(HISTORY + RECENT + 5) {
            j.put(ran("watcher", 200 + n as Moment, OutcomeKind::Noted));
        }
        assert_eq!(j.last("watcher"), Some(100), "the run was pruned");
        assert!(j.runs.len() <= HISTORY, "the ceiling stopped holding");
        // AND ONLY THE LAST ONE is kept above the ceiling: an older run of
        // the same task goes, first-in-first-out.
        let mut j = Journal::fresh();
        j.put(ran("watcher", 50, OutcomeKind::Done));
        j.put(ran("watcher", 100, OutcomeKind::Failed));
        for n in 0..HISTORY {
            j.put(ran("watcher", 200 + n as Moment, OutcomeKind::Noted));
        }
        assert_eq!(j.last("watcher"), Some(100));
        assert!(!j.runs.iter().any(|r| r.began == 50));
    }

    #[test]
    fn a_journal_that_stopped_parsing_is_no_history_and_not_a_panic() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-brain-journal-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join(JOURNAL_FILE), "this is not json").expect("write");
        assert_eq!(Journal::read_from(&dir), Journal::fresh());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
