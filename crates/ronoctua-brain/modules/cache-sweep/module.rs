//! Taking cached files nothing has used for a while.
//!
//! # Why the brain owns the WHEN and not the WHAT
//!
//! The walk is a unit (`module.nix`) and a binary of its own
//! (`ronoctua-cache-sweep`), for three reasons that were measured.
//! [`Scheduled::run`] blocks the runner's thread, and a walk of a quarter of a
//! million files there is a daemon that draws nothing and cannot be stopped
//! while it lasts. A unit gets `IOSchedulingClass=idle` for nothing, which
//! matters when the first run frees a hundred and fifty gibibytes. And a unit
//! stays reachable by hand.
//!
//! What is left for the brain is exactly what the brain is for: whether this is
//! a good moment. That question is already answered, once, by the runner's gate
//! the person configured — so this module has no `needs` of its own.
//!
//! # And the numbers are not this file's either
//!
//! Every one of them comes from `assets/cache.nix`: the cadence and the window
//! through the manifest, how old is too old through the unit's argument. How
//! often, when in the day and how old is too old are one declaration, because
//! the file manager reads two of the same four to decide when to remake a
//! thumbnail — and `freshDays < keepDays` is a promise neither crate can keep
//! alone. `why.md` beside this file tells the rest.

use std::sync::Arc;

use crate::module::{Birth, Manifest, Module, Run, Scheduled};
use crate::moment::{Moment, Offset};
use crate::prefs::TaskPrefs;
use crate::schedule::next_moment;
use crate::systemd;
use crate::task::{Due, Outcome, Sheet, Watched};

/// The entry: one sweep, in the machine's zone.
#[must_use]
pub fn new(birth: &Birth) -> impl Module + use<> {
    CacheSweep {
        zone: Arc::clone(&birth.zone),
    }
}

/// The sweep.
struct CacheSweep {
    /// How far ahead of UTC this machine is — handed in, so
    /// [`Scheduled::due`] stays pure.
    zone: Arc<dyn Offset>,
}

/// The unit a run starts: the rehearsal's walks and deletes nothing.
///
/// A SECOND UNIT AND NOT A FLAG: an argument that crossed from here would be an
/// argument somebody else could cross with. And it earns its keep here —
/// *"how much would this take?"* is exactly the question a person wants
/// answered before letting something delete a hundred and fifty gibibytes, and
/// the binary already answers it, because rehearsing is what it does when
/// nobody says `--execute`.
const fn unit_for(rehearsal: bool) -> &'static str {
    if rehearsal {
        super::unit::REHEARSE
    } else {
        super::unit::RUN
    }
}

/// What a walk that ended says about itself. Its steps are the binary's own
/// words, not the update's, so a run that worked records none.
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

impl Module for CacheSweep {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn scheduled(&self) -> Option<&dyn Scheduled> {
        Some(self)
    }
}

impl Scheduled for CacheSweep {
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

    // NOTHING OF ITS OWN in `needs`: busy, and on battery, are the runner's
    // questions, asked before it is reached and configured by the person in
    // one place.

    fn run(&self, run: &Run) -> Outcome {
        let unit = unit_for(run.is_rehearsal());
        // THE MARK BEFORE THE START. Whatever is in the report file now belongs
        // to the PREVIOUS run, and taking the mark afterwards means the first
        // look finds it, sees `ended` set, and reports a run that finished
        // before it began.
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
                // GIVING UP IS WHAT STOPS IT: an aborted sweep that went on
                // deleting would be a machine still losing files after the
                // person said to stop.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moment::Fixed;
    use crate::prefs::PartOfDay;

    const DAY: Moment = 24 * 3600;

    fn sweep() -> CacheSweep {
        CacheSweep {
            zone: Arc::new(Fixed(0)),
        }
    }

    #[test]
    fn it_sweeps_by_the_catalogues_calendar_and_announces_nothing() {
        let s = super::super::MANIFEST
            .schedule
            .expect("a sweep with no schedule");
        // THE CATALOGUE'S WINDOW: at night, when nobody is looking.
        assert!(s.calendar, "a sweep that keeps no calendar");
        assert_eq!(s.part_of_day, PartOfDay::Night, "the sweep left the night");
        assert!(s.interval_days > 0);
        // NO COUNTDOWN, and a patience of minutes.
        assert_eq!(s.countdown_seconds, 0, "a band in the way for no decision");
        assert_eq!(s.runaway_minutes, 30);
        assert!(super::super::MANIFEST.gated(), "the gate would not hold it");
        // AND WHAT THE PERSON HAS NOT CHOSEN IS WHAT IT DECLARED.
        let mine = crate::prefs::Prefs::default().of_module(&super::super::MANIFEST);
        assert_eq!(mine.interval_days, s.interval_days);
        assert_eq!(mine.countdown_seconds, 0);
    }

    #[test]
    fn a_rehearsal_starts_the_unit_that_deletes_nothing() {
        assert_eq!(unit_for(true), super::super::unit::REHEARSE);
        assert_eq!(unit_for(false), super::super::unit::RUN);
        assert_ne!(unit_for(true), unit_for(false), "the rehearsal deletes");
        // AND BOTH ARE THE MANIFEST'S, the units `module.nix` declares.
        let units = super::super::MANIFEST.units;
        assert_eq!(units.run, Some(unit_for(false)));
        assert_eq!(units.rehearse, Some(unit_for(true)));
    }

    #[test]
    fn it_comes_due_by_its_calendar_and_never_where_nothing_can_start_it() {
        let s = sweep();
        let mine = crate::prefs::Prefs::default().of_module(&super::super::MANIFEST);
        let now = 400 * DAY;
        if !systemd::reachable() {
            // NO DATE ON SCREEN THAT NOTHING WILL HONOUR.
            assert_eq!(
                s.due(now, &mine, None),
                Due::Not {
                    why: "no-systemctl"
                }
            );
            return;
        }
        // NEVER RUN IS A FIRST RUN, not an overdue one: the next window, and
        // not a moment in the past that would make it look late on a fresh
        // install.
        match s.due(now, &mine, None) {
            Due::At(at) => assert!(
                at > now && at <= now + DAY,
                "a sweep never run waits past the next night: {at}"
            ),
            other => panic!("a sweep never run is {other:?}"),
        }
        match s.due(now, &mine, Some(now)) {
            Due::At(at) => assert!(
                at > now && at <= now + Moment::from(mine.interval_days + 1) * DAY,
                "the next sweep is not one cadence away: {at}"
            ),
            other => panic!("a sweep that just ran is {other:?}"),
        }
    }

    #[test]
    fn a_walk_that_ended_in_trouble_says_it_and_one_that_worked_is_done() {
        assert_eq!(ended(Sheet::default()), Outcome::Done(Vec::new()));
        assert_eq!(
            ended(Sheet {
                trouble: "the walk could not read its root".to_owned(),
                ..Sheet::default()
            }),
            Outcome::Failed {
                at: String::new(),
                why: "the walk could not read its root".to_owned(),
            }
        );
    }
}
