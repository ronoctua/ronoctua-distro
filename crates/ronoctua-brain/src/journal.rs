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

/// How many runs are kept.
///
/// Sixty-four, the same ceiling the transfer queue keeps its finished jobs to,
/// and for the same reason: it is a few screens of history — enough to answer
/// "did that one work?" weeks later — and small enough that the whole thing
/// crossing the socket on every change stays free. At one run a week it is more
/// than a year.
pub const HISTORY: usize = 64;

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
        self.latest(task).map(|r| r.began)
    }

    /// The last record of this task — what the screen draws.
    #[must_use]
    pub fn latest(&self, task: &str) -> Option<&RunRecord> {
        self.runs.iter().rev().find(|r| r.task == task)
    }

    /// Writes one down, and drops the oldest past the ceiling.
    pub fn put(&mut self, record: RunRecord) {
        self.runs.push(record);
        if self.runs.len() > HISTORY {
            let over = self.runs.len() - HISTORY;
            self.runs.drain(..over);
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
        std::fs::create_dir_all(dir)?;
        // COMPACT, unlike the preferences: nobody edits this by hand, and the
        // one thing it should not be is large.
        let text = serde_json::to_string(self).map_err(std::io::Error::other)?;
        std::fs::write(dir.join(JOURNAL_FILE), text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::OutcomeKind;

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
        j.put(ran("system-update", 100, OutcomeKind::Done));
        j.put(ran("system-update", 200, OutcomeKind::Skipped));
        assert_eq!(j.last("system-update"), Some(200));
        assert_eq!(j.latest("system-update").map(|r| r.began), Some(200));
    }

    #[test]
    fn a_failed_run_counts_so_it_does_not_retry_every_half_minute() {
        let mut j = Journal::fresh();
        j.put(ran("system-update", 100, OutcomeKind::Done));
        j.put(ran("system-update", 300, OutcomeKind::Failed));
        assert_eq!(j.last("system-update"), Some(300));
    }

    #[test]
    fn the_tasks_do_not_read_each_others_history() {
        let mut j = Journal::fresh();
        j.put(ran("system-update", 100, OutcomeKind::Done));
        assert_eq!(j.last("something-else"), None);
    }

    #[test]
    fn the_ceiling_drops_the_oldest_and_the_file_round_trips() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-brain-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut j = Journal::fresh();
        for n in 0..(HISTORY + 10) {
            j.put(ran("system-update", n as Moment, OutcomeKind::Done));
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
