//! Emptying the trash of what has been in it long enough.
//!
//! # Why this is not the cache sweep, and must never become it
//!
//! Because nothing here can be made again. The cache sweep deletes what a
//! program can rebuild and deliberately never goes near `$XDG_DATA_HOME`; this
//! deletes the only copy. Same machinery, and deliberately different numbers:
//! four days rather than forty, and an age read out of the `DeletionDate` the
//! freedesktop spec writes into every note rather than guessed at from a
//! file's own timestamps — a document last written a year ago and thrown away
//! this morning has been in the trash for a morning.
//!
//! Everything else is the sweep's shape, and for the sweep's reasons: a unit so
//! the runner's thread is free and the deletes go at idle priority, two units
//! so `m e` can ask *"how much would this take?"* of something irreversible,
//! and no `needs` of its own because whether the machine is busy has one answer
//! in this tree. `why.md` beside this file tells the rest.

use std::sync::Arc;

use crate::module::{Birth, Manifest, Module, Run, Scheduled};
use crate::moment::{Moment, Offset};
use crate::prefs::TaskPrefs;
use crate::schedule::next_moment;
use crate::systemd;
use crate::task::{Due, Outcome, Sheet, Watched};

/// The entry: one emptying, in the machine's zone.
#[must_use]
pub fn new(birth: &Birth) -> impl Module + use<> {
    EmptyTrash {
        zone: Arc::clone(&birth.zone),
    }
}

/// The trash's emptying.
struct EmptyTrash {
    /// How far ahead of UTC this machine is — handed in, so
    /// [`Scheduled::due`] stays pure.
    zone: Arc<dyn Offset>,
}

/// The unit a run starts: the rehearsal's reads every note and deletes
/// nothing — a second unit and not a flag, for the sweep's reason.
const fn unit_for(rehearsal: bool) -> &'static str {
    if rehearsal {
        super::unit::REHEARSE
    } else {
        super::unit::RUN
    }
}

/// What an emptying that ended says about itself.
fn ended(sheet: Sheet) -> Outcome {
    if sheet.trouble.is_empty() {
        Outcome::Done(Vec::new())
    } else {
        Outcome::Failed {
            at: String::new(),
            why: sheet.trouble,
        }
    }
}

impl Module for EmptyTrash {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn scheduled(&self) -> Option<&dyn Scheduled> {
        Some(self)
    }
}

impl Scheduled for EmptyTrash {
    fn due(&self, now: Moment, mine: &TaskPrefs, last: Option<Moment>) -> Due {
        // A RUN THAT CANNOT ASK is not a run that is late: offering a moment
        // here would put a date on screen that nothing will ever honour.
        if !systemd::reachable() {
            return Due::Not {
                why: "no-systemctl",
            };
        }
        let at = next_moment(now, last, mine, self.zone.as_ref());
        if at <= now { Due::Now } else { Due::At(at) }
    }

    fn run(&self, run: &Run) -> Outcome {
        let unit = unit_for(run.is_rehearsal());
        // THE MARK BEFORE THE START: what is in the report now is the
        // PREVIOUS run's.
        let before = run.mark();
        if let Err(e) = systemd::start(unit) {
            return Outcome::Failed {
                at: String::new(),
                why: e.to_string(),
            };
        }
        match run.watch(before) {
            Watched::Ended(sheet) => ended(sheet),
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
                    tracing::warn!(error = %e, "could not stop emptying the trash");
                }
                Outcome::Failed {
                    at: String::new(),
                    why: "aborted".to_owned(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moment::Fixed;
    use crate::prefs::PartOfDay;

    const DAY: Moment = 24 * 3600;

    fn emptying() -> EmptyTrash {
        EmptyTrash {
            zone: Arc::new(Fixed(0)),
        }
    }

    #[test]
    fn it_empties_by_the_catalogues_calendar_and_announces_nothing() {
        let s = super::super::MANIFEST
            .schedule
            .expect("an emptying with no schedule");
        assert!(s.calendar, "an emptying that keeps no calendar");
        assert_eq!(s.part_of_day, PartOfDay::Night, "the trash left the night");
        // A FEW DAYS: the number that decides something irreversible is
        // small, and the cadence is no longer than it.
        assert!(
            (1..=7).contains(&s.interval_days),
            "the trash is emptied every {} days",
            s.interval_days
        );
        assert_eq!(s.countdown_seconds, 0, "a band in the way for no decision");
        assert_eq!(s.runaway_minutes, 30);
        assert!(super::super::MANIFEST.gated(), "the gate would not hold it");
    }

    #[test]
    fn a_rehearsal_starts_the_unit_that_deletes_nothing() {
        assert_eq!(unit_for(true), super::super::unit::REHEARSE);
        assert_eq!(unit_for(false), super::super::unit::RUN);
        assert_ne!(unit_for(true), unit_for(false), "the rehearsal deletes");
        let units = super::super::MANIFEST.units;
        assert_eq!(units.run, Some(unit_for(false)));
        assert_eq!(units.rehearse, Some(unit_for(true)));
    }

    #[test]
    fn it_comes_due_by_its_calendar_and_never_where_nothing_can_start_it() {
        let t = emptying();
        let mine = crate::prefs::Prefs::default().of_module(&super::super::MANIFEST);
        let now = 400 * DAY;
        if !systemd::reachable() {
            assert_eq!(
                t.due(now, &mine, None),
                Due::Not {
                    why: "no-systemctl"
                }
            );
            return;
        }
        // NEVER RUN IS A FIRST RUN, not an overdue one: the next window, and
        // not a moment in the past that would make it look late on a fresh
        // install.
        match t.due(now, &mine, None) {
            Due::At(at) => assert!(
                at > now && at <= now + DAY,
                "a trash never emptied waits past the next night: {at}"
            ),
            other => panic!("a trash never emptied is {other:?}"),
        }
        match t.due(now, &mine, Some(now)) {
            Due::At(at) => assert!(
                at > now && at <= now + Moment::from(mine.interval_days + 1) * DAY,
                "the next emptying is not one cadence away: {at}"
            ),
            other => panic!("an emptying that just ran is {other:?}"),
        }
    }

    #[test]
    fn an_emptying_that_ended_in_trouble_says_it_and_one_that_worked_is_done() {
        assert_eq!(ended(Sheet::default()), Outcome::Done(Vec::new()));
        assert_eq!(
            ended(Sheet {
                trouble: "a note could not be read".to_owned(),
                ..Sheet::default()
            }),
            Outcome::Failed {
                at: String::new(),
                why: "a note could not be read".to_owned(),
            }
        );
    }
}
