//! Taking cached files nothing has used for a while.
//!
//! # Why the brain owns the WHEN and not the WHAT
//!
//! The walk is a unit (`nix/modules/cache.nix`) and a binary of its own
//! (`ronoctua-cache-sweep`), for three reasons that were measured. [`Task::run`]
//! blocks the runner's thread, and a walk of a quarter of a million files there
//! is a daemon that draws nothing and cannot be stopped while it lasts. A unit
//! gets `IOSchedulingClass=idle` for nothing, which matters when the first run
//! frees a hundred and fifty gibibytes. And a unit stays reachable by hand.
//!
//! What is left for the brain is exactly what the brain is for: whether this is
//! a good moment. That question is already answered, once, by
//! [`crate::runner::judge`] against a gate the person configured — so this task
//! has no `needs` of its own, for the reason `update.rs` writes out beside its
//! own empty one.
//!
//! # And the numbers are not this file's either
//!
//! Every one of them comes from `assets/cache.nix` through `nix/cache.nix`. How
//! often, when in the day and how old is too old are one declaration, because
//! the file manager reads two of the same four to decide when to remake a
//! thumbnail — and `freshDays < keepDays` is a promise neither crate can keep
//! alone.

use std::sync::Arc;

use crate::moment::{Moment, Offset};
use crate::prefs::{PartOfDay, Prefs, TaskPrefs};
use crate::pulse::Pulse;
use crate::schedule::next_moment;
use crate::systemd;
use crate::task::{Due, Outcome, Report, Task, Verdict, Watched};

/// The task's id — English, kebab-case, and the key of everything the person
/// sees about it (`brain-task-cache-sweep`).
pub const ID: &str = "cache-sweep";

/// The unit the flake ships, and the only thing this task starts.
///
/// A CONSTANT for `update.rs`'s reason: a unit name that came out of a
/// configuration file would be a configuration file that can start units.
pub const UNIT: &str = "ronoctua-cache-sweep.service";

/// The same walk, deleting nothing — what `m e` asks for.
///
/// A SECOND UNIT AND NOT A FLAG, for the reason the update's rehearsal gives:
/// an argument that crossed from here would be an argument somebody else could
/// cross with. And it earns its keep here more than there — *"how much would
/// this take?"* is exactly the question a person wants answered before letting
/// something delete a hundred and fifty gibibytes, and the binary already
/// answers it, because rehearsing is what it does when nobody says `--execute`.
pub const REHEARSAL_UNIT: &str = "ronoctua-cache-sweep-rehearsal.service";

/// The policy, generated from `assets/cache.nix` by `nix/cache.nix`.
///
/// `KEEP_DAYS` is not read here on purpose — the unit is told it, because the
/// unit is what deletes. What the brain takes from this file is the cadence and
/// the window.
#[allow(dead_code)]
mod policy {
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// The sweep.
pub struct CacheSweep {
    /// How far ahead of UTC this machine is — handed in, so [`Task::due`] stays
    /// pure. See [`crate::tasks::tasks`].
    zone: Arc<dyn Offset>,
}

impl CacheSweep {
    /// One, in a given zone.
    #[must_use]
    pub fn with(zone: Arc<dyn Offset>) -> Self {
        Self { zone }
    }
}

impl Task for CacheSweep {
    fn id(&self) -> &'static str {
        ID
    }

    fn defaults(&self) -> TaskPrefs {
        TaskPrefs {
            // EVERY FIVE DAYS AT NIGHT, from the catalogue. `PartOfDay::Night`
            // is 02:00 to 05:00, which is where the owner's "four in the
            // morning" falls — and a window is what this vocabulary has,
            // because an hour is a decision about a machine and `at night` is a
            // decision about a life.
            interval_days: policy::EVERY_DAYS,
            part_of_day: PartOfDay::from_word(policy::PART_OF_DAY),
            // NO COUNTDOWN. It interrupts nothing, nothing it does is visible
            // while it happens, and a minute of warning about deleting files
            // nobody has opened in three weeks would be a band in the way for
            // no decision. `task.rs` says it: zero is the right answer for
            // anything that interrupts nothing.
            countdown_seconds: 0,
            // AND A PATIENCE OF MINUTES, NOT HOURS. Measured on the owner's own
            // cache — 424 000 files, 150 GiB — the walk takes ten seconds and
            // the deletes are what the disk can do at idle priority. The
            // default of two hours belongs to a task that compiles a system;
            // here it would mean a sweep that died silently is not noticed
            // until the afternoon.
            runaway_minutes: 30,
            ..TaskPrefs::default()
        }
    }

    fn due(&self, now: Moment, prefs: &Prefs, last: Option<Moment>) -> Due {
        let mine = prefs.of_task(self);
        if !mine.enabled {
            return Due::Not { why: "off" };
        }
        // A SWEEP THAT CANNOT ASK is not a sweep that is late. Same shape, and
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
        // NOTHING OF ITS OWN. Busy, and on battery, are the runner's questions,
        // asked before this is reached and configured by the person in one
        // place — which is exactly what the owner asked for when he said the
        // RAM and CPU check should be shared with the other routines.
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
                // GIVING UP IS WHAT STOPS IT — the same as the update's, and it
                // matters more here: an aborted sweep that went on deleting
                // would be a machine still losing files after the person said
                // to stop.
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
