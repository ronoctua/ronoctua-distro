//! The report a unit writes, and the patience the daemon watches it with.
//!
//! What a run is made of — when it is due, what may hold it, what it did, the
//! [`Sheet`] the unit writes — is the library's (`ronoctua_brain::task`),
//! re-exported here whole, so `crate::task::…` names both halves. What is here
//! is the WATCHING, which only the daemon does: a [`Report`] and how it
//! ended ([`Watched`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub use ronoctua_brain::task::*;

/// The file, and the patience to watch it with.
#[derive(Debug, Clone)]
pub struct Report {
    path: PathBuf,
    patience: Duration,
    beat: Duration,
    stop: Arc<AtomicBool>,
    rehearsal: bool,
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
    /// # Why it is on the REPORT and not on the module
    ///
    /// Because it is a property of this run and not of the feature. A module is a
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
}
