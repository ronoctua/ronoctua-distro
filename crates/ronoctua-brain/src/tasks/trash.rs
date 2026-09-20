//! Emptying the trash of what has been in it long enough.
//!
//! # Why this is not the cache sweep, and must never become it
//!
//! Because nothing here can be made again. `ronoctua-cache-sweep` deletes what
//! a program can rebuild and deliberately never goes near `$XDG_DATA_HOME`;
//! this deletes the only copy. Same machinery, and deliberately different
//! numbers: four days rather than forty, and an age read out of the
//! `DeletionDate` the freedesktop spec writes into every note rather than
//! guessed at from a file's own timestamps — a document last written a year ago
//! and thrown away this morning has been in the trash for a morning.
//!
//! Everything else is the sweep's shape, and for the sweep's reasons: a unit so
//! the runner's thread is free and the deletes go at idle priority, two units
//! so `m e` can ask *"how much would this take?"* of something irreversible,
//! and no `needs` of its own because whether the machine is busy has one answer
//! in this tree.

use std::sync::Arc;

use crate::moment::{Moment, Offset};
use crate::prefs::{PartOfDay, Prefs, TaskPrefs};
use crate::pulse::Pulse;
use crate::schedule::next_moment;
use crate::systemd;
use crate::task::{Due, Outcome, Report, Task, Verdict, Watched};

/// The task's id — English, kebab-case, and the key of everything the person
/// sees about it (`brain-task-trash-expire`).
pub const ID: &str = "trash-expire";

/// The unit the flake ships, and the only thing this task starts.
///
/// A CONSTANT for `update.rs`'s reason: a unit name that came out of a
/// configuration file would be a configuration file that can start units.
pub const UNIT: &str = "ronoctua-empty-trash.service";

/// The same walk, deleting nothing — what `m e` asks for.
///
/// A SECOND UNIT AND NOT A FLAG, for the reason the update's rehearsal gives:
/// an argument that crossed from here would be an argument somebody else could
/// cross with. And it earns its keep here more than there — *"how much would
/// this take?"* is exactly the question a person wants answered before letting
/// something delete a hundred and fifty gibibytes, and the binary already
/// answers it, because rehearsing is what it does when nobody says `--execute`.
pub const REHEARSAL_UNIT: &str = "ronoctua-empty-trash-rehearsal.service";

/// The policy, generated from `assets/cache.nix` by `nix/cache.nix`.
///
/// `TRASH_DAYS` is not read here on purpose — the unit is told it, because the
/// unit is what deletes. What the brain takes from this file is the cadence and
/// the window.
#[allow(dead_code)]
mod policy {
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// The sweep.
pub struct EmptyTrash {
    /// How far ahead of UTC this machine is — handed in, so [`Task::due`] stays
    /// pure. See [`crate::tasks::tasks`].
    zone: Arc<dyn Offset>,
}

impl EmptyTrash {
    /// One, in a given zone.
    #[must_use]
    pub fn with(zone: Arc<dyn Offset>) -> Self {
        Self { zone }
    }
}

impl Task for EmptyTrash {
    fn id(&self) -> &'static str {
        ID
    }

    fn defaults(&self) -> TaskPrefs {
        TaskPrefs {
            // EVERY FOUR DAYS AT NIGHT, from the catalogue. `PartOfDay::Night`
            // is 02:00 to 05:00, which is where the owner's "four in the
            // morning" falls — and a window is what this vocabulary has,
            // because an hour is a decision about a machine and `at night` is a
            // decision about a life.
            interval_days: policy::TRASH_EVERY_DAYS,
            part_of_day: PartOfDay::from_word(policy::PART_OF_DAY),
            // NO COUNTDOWN. It interrupts nothing, nothing it does is visible
            // while it happens, and a minute of warning about deleting files
            // nobody has opened in three weeks would be a band in the way for
            // no decision. `task.rs` says it: zero is the right answer for
            // anything that interrupts nothing.
            countdown_seconds: 0,
            // AND A PATIENCE OF MINUTES, NOT HOURS. A trash is a handful of
            // notes and what they name; measured on the owner's, ten entries
            // and 362 MiB answered in a tenth of a second. Two hours of
            // patience belongs to a task that compiles a system.
            runaway_minutes: 30,
            ..TaskPrefs::default()
        }
    }

    fn due(&self, now: Moment, prefs: &Prefs, last: Option<Moment>) -> Due {
        let mine = prefs.of_task(self);
        if !mine.enabled {
            return Due::Not { why: "off" };
        }
        // A TASK THAT CANNOT ASK is not a task that is late. Same shape, and
        // the same reason, as the update's: offering a moment here would put a
        // date on screen that nothing will ever honour.
        if !systemd::reachable() {
            return Due::Not {
                why: "no-systemctl",
            };
        }
        let at = next_moment(now, last, &mine, self.zone.as_ref());
        if at <= now { Due::Now } else { Due::At(at) }
    }

    fn needs(&self, _pulse: &Pulse, _prefs: &Prefs) -> Verdict {
        // NOTHING OF ITS OWN, for the reason the sweep's own empty one gives.
        Verdict::Go
    }

    fn countdown(&self, prefs: &Prefs) -> u16 {
        prefs.of_task(self).countdown_seconds
    }

    fn run(&self, report: &Report) -> Outcome {
        let unit = if report.is_rehearsal() {
            REHEARSAL_UNIT
        } else {
            UNIT
        };
        // THE MARK BEFORE THE START. Whatever is in the report file now belongs
        // to the PREVIOUS run, and taking the mark afterwards means the first
        // look finds it, sees `ended` set, and reports a run that finished
        // before it began.
        let before = report.mark();
        if let Err(e) = systemd::start(unit) {
            return Outcome::Failed {
                at: String::new(),
                why: e.to_string(),
            };
        }
        match report.watch(before) {
            Watched::Ended(sheet) => {
                if sheet.trouble.is_empty() {
                    Outcome::Done(Vec::new())
                } else {
                    Outcome::Failed {
                        at: String::new(),
                        why: sheet.trouble,
                    }
                }
            }
            Watched::Runaway(_) => Outcome::Failed {
                at: String::new(),
                why: "runaway".to_owned(),
            },
            Watched::Aborted(_) => {
                // GIVING UP IS WHAT STOPS IT, and it matters most of all here:
                // this is the only thing in the tree deleting something that
                // cannot be made again, so a run that went on after the person
                // said stop would be files lost with permission withdrawn.
                if let Err(e) = systemd::stop(unit) {
                    tracing::warn!(error = %e, "could not stop the cache sweep");
                }
                Outcome::Failed {
                    at: String::new(),
                    why: "aborted".to_owned(),
                }
            }
        }
    }
}
