//! The watch on the Emacs daemon.
//!
//! # What it is for
//!
//! The owner asked for it: check, from time to time, whether the Emacs
//! server is up, and say so on the brain's line with a way to start it. And
//! then the harder half — whether the Emacs that IS open is the client that
//! uses that server, or a second copy of the editor that loaded its own
//! configuration and holds buffers the session will never see.
//!
//! # Why it is a task and not a thread somewhere
//!
//! Because everything a task gets for free is what this needs: a cadence the
//! person can change, a row on a screen that says when it last looked, a
//! record of what it found, and a gate that keeps it off the machine while
//! somebody is compiling. A thread would be a second scheduler with none of
//! that.
//!
//! # Why its cadence is in MINUTES
//!
//! Because a daemon can stop at ten in the morning. Every other task here
//! thinks in days, which is right for a rebuild and useless for a watch: a
//! check that came round tomorrow would leave somebody without their editor's
//! session for a day while the screen said everything was in hand. See
//! [`crate::prefs::TaskPrefs::every_minutes`].
//!
//! # What it does NOT do
//!
//! Close anybody's editor. A standalone Emacs is somebody's work with unsaved
//! buffers in it; the screen says what it is and what to do about it, and the
//! person decides. The only thing this starts is the distro's own unit.

use crate::moment::Moment;
use crate::prefs::{PartOfDay, Prefs, TaskPrefs};
use crate::pulse::Pulse;
use crate::schedule::next_moment;
use crate::task::{Due, Kind, Notice, Outcome, Report, Task, Verdict};

/// The task's id — English, kebab-case, and the key of everything on screen.
pub const ID: &str = "emacs-daemon";

/// The unit the distro already ships.
///
/// `services.emacs.enable` is on in `nix/modules/emacs.nix`, which is the
/// upstream NixOS module and produces a USER unit running `emacs --fg-daemon`.
/// Starting a user unit goes through no polkit and asks nobody anything, which
/// is the whole reason this may offer a button at all — `AGENTS.md`: nothing
/// the system does by itself may ask.
pub const UNIT: &str = "emacs.service";

/// How often it looks.
///
/// SIX HOURS, by the owner's choice, against the five minutes it opened with.
/// Four looks a day instead of two hundred and eighty-eight.
///
/// Five minutes was argued from the watch's cost — one listing of `/proc` and
/// one short-lived `emacsclient`, which is nothing — and that argument left out
/// the only expensive thing a watch does: it writes a line in the journal every
/// time it runs, and the home page's activity log is a hundred lines long. At
/// five minutes this task alone filled it twice an hour, so the log answered
/// "what has this machine been doing" with two hundred and eighty-eight
/// repetitions of "nothing". The owner read exactly that and said the watch was
/// running all the time.
///
/// It still writes a line on every run — that was the owner's call too, and at
/// this cadence four a day sit in a hundred without crowding anything out.
const EVERY_MINUTES: u32 = 6 * 60;

/// AND IT STAYS RARE, checked by the COMPILER rather than by a test.
///
/// What this guards is not the number — that is the owner's — but the class of
/// change that put the activity log back under one task's heel. A build is
/// where that argument belongs: a test would let the tree be green for as long
/// as nobody ran it.
const _: () = assert!(
    EVERY_MINUTES >= 60,
    "a watch that runs more than once an hour drowns the activity log"
);

/// The watch.
pub struct EmacsDaemon {
    /// Where `/proc` is — handed in, so a test drives a planted tree.
    places: ronoctua_memory::emacs::Places,
}

impl Default for EmacsDaemon {
    fn default() -> Self {
        Self::new()
    }
}

impl EmacsDaemon {
    /// One, reading the real `/proc`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            places: ronoctua_memory::emacs::Places::default(),
        }
    }

    /// One reading somewhere else — what a test hands in.
    #[must_use]
    pub fn looking_in(places: ronoctua_memory::emacs::Places) -> Self {
        Self { places }
    }
}

impl Task for EmacsDaemon {
    fn id(&self) -> &'static str {
        ID
    }

    fn kind(&self) -> Kind {
        Kind::Routine
    }

    fn defaults(&self) -> TaskPrefs {
        TaskPrefs {
            every_minutes: Some(EVERY_MINUTES),
            // NO WINDOW AND NO COUNTDOWN. A watch is not an event: it reads
            // two things and says what it found, and warning somebody five
            // seconds before it does would be a countdown to nothing.
            part_of_day: PartOfDay::Any,
            countdown_seconds: 0,
            // A minute is already generous for a `/proc` walk and an
            // `emacsclient` with a deadline of its own.
            runaway_minutes: 1,
            ..TaskPrefs::default()
        }
    }

    fn due(&self, now: Moment, prefs: &Prefs, last: Option<Moment>) -> Due {
        let mine = prefs.of_task(self);
        if !mine.enabled {
            return Due::Not { why: "off" };
        }
        // NO `systemd::reachable()` GATE, unlike the tasks that start a unit
        // to do their work. This one's work is LOOKING, and looking is worth
        // doing on a machine where nothing can be started: the answer is then
        // "the daemon is not running and I cannot start it", which is more
        // than silence.
        // THE ZONE IS NEVER CONSULTED, and handing one in would say it was.
        // A cadence in minutes short-circuits the calendar in `next_moment`
        // before a zone is reached — see the first branch of it — so this is
        // the only task in the tree that does not carry one.
        let at = next_moment(now, last, &mine, &crate::moment::Fixed(0));
        if at <= now { Due::Now } else { Due::At(at) }
    }

    fn needs(&self, _pulse: &Pulse, _prefs: &Prefs) -> Verdict {
        Verdict::Go
    }

    fn countdown(&self, _prefs: &Prefs) -> u16 {
        0
    }

    fn run(&self, report: &Report) -> Outcome {
        let found = ronoctua_memory::emacs::look(&self.places);
        // A REHEARSAL IS THE SAME LOOK. Nothing here writes, starts or
        // deletes anything, so there is nothing for a rehearsal to hold back
        // — and a watch that refused to look when asked to rehearse would be
        // a watch nobody can test by hand.
        //
        // `say(None)` when all is well, and that is half the contract: a line
        // that had to be cancelled by a second call is a line that stays up
        // after the thing it was about has been fixed.
        report.say(if worth_saying(found) {
            notice_of(found)
        } else {
            // A MACHINE WITH NO EMACS AT ALL is not a machine with a problem.
            None
        });
        Outcome::Done(Vec::new())
    }
}

/// What the band should say, given what was found.
///
/// A pure function, which is what makes a year of Emacsen a millisecond of
/// test. The three cases are in the order somebody should hear them: the one
/// that stops them working, then the one that quietly costs them a session,
/// then nothing at all.
#[must_use]
pub fn notice_of(found: ronoctua_memory::emacs::Emacsen) -> Option<Notice> {
    if !found.daemon {
        // THE ONE WITH A BUTTON. Starting a user unit asks nobody anything.
        return Some(Notice {
            task: ID.to_owned(),
            says: "emacs-no-daemon".to_owned(),
            act: Some("brain-start-emacs".to_owned()),
        });
    }
    if found.standalone > 0 {
        // AND THE ONE WITHOUT ONE. What is open is somebody's work, with
        // buffers in it that nothing here knows whether they have saved. The
        // screen says what it is; the person closes it or does not.
        return Some(Notice {
            task: ID.to_owned(),
            says: "emacs-not-the-client".to_owned(),
            act: None,
        });
    }
    None
}

/// A machine with no Emacs at all is not a machine with a problem.
///
/// Without this the watch would raise a line on every machine that has never
/// installed the editor — which is a notice about the absence of something
/// nobody asked for.
#[must_use]
pub const fn worth_saying(found: ronoctua_memory::emacs::Emacsen) -> bool {
    found.daemon || found.clients > 0 || found.standalone > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_memory::emacs::Emacsen;

    #[test]
    fn a_dead_daemon_gets_a_button_and_a_wrong_emacs_does_not() {
        let down = Emacsen {
            daemon: false,
            clients: 0,
            standalone: 0,
        };
        let n = notice_of(down).expect("a daemon that is not running says so");
        assert_eq!(n.says, "emacs-no-daemon");
        assert!(n.act.is_some(), "there is no way to start it");

        let wrong = Emacsen {
            daemon: true,
            clients: 0,
            standalone: 1,
        };
        let n = notice_of(wrong).expect("a second copy of the editor says so");
        assert_eq!(n.says, "emacs-not-the-client");
        assert!(
            n.act.is_none(),
            "the screen offered to close somebody's editor"
        );

        let well = Emacsen {
            daemon: true,
            clients: 2,
            standalone: 0,
        };
        assert!(notice_of(well).is_none(), "a notice with nothing wrong");
    }

    /// A MACHINE WITH NO EMACS IS NOT A MACHINE WITH A PROBLEM.
    #[test]
    fn nothing_running_is_nothing_to_say() {
        assert!(!worth_saying(Emacsen::default()));
        assert!(worth_saying(Emacsen {
            daemon: true,
            ..Emacsen::default()
        }));
        assert!(worth_saying(Emacsen {
            standalone: 1,
            ..Emacsen::default()
        }));
    }

    /// It comes round in MINUTES, and never through the calendar.
    ///
    /// Written against the constant and not against the number it happens to
    /// hold: the first version said `1_000 + 300`, and the day the owner asked
    /// for a rarer watch the test failed while the code was right. What this
    /// is about is that a cadence in minutes short-circuits the day-based
    /// schedule — not how many minutes.
    #[test]
    fn it_comes_round_in_minutes() {
        let task = EmacsDaemon::new();
        let mine = task.defaults();
        assert_eq!(mine.every_minutes, Some(EVERY_MINUTES));
        let gap = Moment::from(EVERY_MINUTES) * 60;
        let prefs = Prefs::default();
        // Never run: due now.
        assert!(matches!(task.due(1_000, &prefs, None), Due::Now));
        // Just run: due one cadence later, to the second.
        assert!(matches!(
            task.due(1_000, &prefs, Some(1_000)),
            Due::At(at) if at == 1_000 + gap
        ));
        assert!(matches!(
            task.due(1_000 + gap, &prefs, Some(1_000)),
            Due::Now
        ));
    }
}
