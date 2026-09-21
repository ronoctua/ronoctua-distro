//! The watch on the machine's own clock.
//!
//! # What it is for
//!
//! The owner asked for automatic time-keeping to be the way this distro runs,
//! for the switch to be on the date-and-time screen, and for the same switch
//! to be reachable from the brain: *"deve existir a atualização automática do
//! relógio/etc do sistema/ronoctua-distro via internet, sendo essa a
//! configuração padrão em vez do ajuste manual"*.
//!
//! # Why it does not fetch the time itself
//!
//! Because something already does it properly. `systemd-timesyncd` is an NTP
//! client: it disciplines the clock continuously, corrects for drift rather
//! than stepping, and knows how far it can trust an answer. A task that asked
//! a web server for the time every six hours would be a SECOND thing setting
//! this machine's clock, less accurate than the first and disagreeing with it.
//!
//! So this is the watch and the switch. It reads whether automatic time is on,
//! turns it on when it is not, and SAYS when the clock is unattended or has
//! never managed to agree with anybody. That is the whole of what "automatic"
//! means on a machine that has an NTP client, and switching this task off is
//! how somebody says they want the clock to be theirs.
//!
//! # Why a task and not a thread somewhere
//!
//! The `emacs-daemon` watch's answer, and the same one: a task gets a cadence
//! the person can change, a row that says when it last looked, a record of
//! what it found and a gate that keeps it off a busy machine. A thread would
//! be a second scheduler with none of that.

use crate::moment::Moment;
use crate::prefs::{PartOfDay, Prefs, TaskPrefs};
use crate::pulse::Pulse;
use crate::schedule::next_moment;
use crate::task::{Due, Kind, Notice, Outcome, Report, Task, Verdict};

/// The task's id — English, kebab-case, and the key of everything on screen.
pub const ID: &str = "clock-sync";

/// How often it looks.
///
/// SIX HOURS, which is the `emacs-daemon` watch's number and is chosen for its
/// reason: every run writes a line in the journal, and the home page's
/// activity log is a hundred lines long. A watch that ran every few minutes
/// answered "what has this machine been doing" with hundreds of repetitions of
/// "nothing" — the owner read exactly that once, and it is not worth learning
/// twice.
///
/// It is also enough for what this watches. The clock going unattended is a
/// unit that stopped or a switch somebody turned off; neither happens between
/// one look and the next without somebody knowing.
const EVERY_MINUTES: u32 = 6 * 60;

/// AND IT STAYS RARE, checked by the COMPILER rather than by a test.
///
/// What this guards is not the number — that is the owner's — but the class of
/// change that put the activity log under one task's heel. A build is where
/// that argument belongs: a test would let the tree be green for as long as
/// nobody ran it.
const _: () = assert!(
    EVERY_MINUTES >= 60,
    "a watch that runs more than once an hour drowns the activity log"
);

/// What the watch found, which is the part with no bus in it.
///
/// A copy of the three fields the rule below reads, so that a year of clocks
/// is a millisecond of test. `ronoctua_clock::Standing` is what fills it on a
/// real machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Found {
    /// Did timedated answer at all?
    pub reachable: bool,
    /// Is a network time client running?
    pub automatic: bool,
    /// Can one be? `false` where none is installed.
    pub can_be_automatic: bool,
    /// Has the clock actually been set by it?
    pub synchronised: bool,
}

impl Found {
    /// What `ronoctua_clock` said, in the three fields this rule reads.
    #[must_use]
    pub const fn of(said: &ronoctua_clock::Standing) -> Self {
        Self {
            reachable: said.reachable,
            automatic: said.automatic,
            can_be_automatic: said.can_be_automatic,
            synchronised: said.synchronised,
        }
    }
}

/// The watch.
#[derive(Debug, Default)]
pub struct ClockSync {
    /// Whether this may actually touch the machine's clock.
    ///
    /// A REHEARSAL reads and says and changes nothing, which is what `m e`
    /// asks and what makes this testable by hand. It travels on the report,
    /// not here — this field exists so a test can drive the whole task with
    /// no bus at all.
    looking: Option<Found>,
}

impl ClockSync {
    /// One that asks the real daemon.
    #[must_use]
    pub const fn new() -> Self {
        Self { looking: None }
    }

    /// One that is handed what it would have found — what a test drives.
    #[must_use]
    pub const fn finding(found: Found) -> Self {
        Self {
            looking: Some(found),
        }
    }

    /// What the machine says, or what the test planted.
    fn look(&self) -> Found {
        if let Some(planted) = self.looking {
            return planted;
        }
        ronoctua_clock::read().map_or_else(
            |e| {
                tracing::debug!(error = %e, "timedated could not be asked about the clock");
                Found::default()
            },
            |said| Found::of(&said),
        )
    }
}

impl Task for ClockSync {
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
            // three properties and says what it found, and warning somebody
            // five seconds before it does would be a countdown to nothing.
            part_of_day: PartOfDay::Any,
            countdown_seconds: 0,
            // A minute is generous for three D-Bus properties.
            runaway_minutes: 1,
            ..TaskPrefs::default()
        }
    }

    fn due(&self, now: Moment, prefs: &Prefs, last: Option<Moment>) -> Due {
        let mine = prefs.of_task(self);
        if !mine.enabled {
            return Due::Not { why: "off" };
        }
        // THE ZONE IS NEVER CONSULTED, and handing one in would say it was: a
        // cadence in minutes short-circuits the calendar in `next_moment`
        // before a zone is reached. The `emacs-daemon` watch is the other
        // task in this tree that carries none.
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
        let found = self.look();
        // TURNING IT BACK ON IS THE WORK, and it is the only thing here that
        // writes. A rehearsal must not do it — `m e` asks what would happen,
        // and a rehearsal that changed the machine's clock-keeping would be
        // the one question in this system that cannot be asked safely.
        let mut found = found;
        if should_turn_on(found) && !report.is_rehearsal() {
            match ronoctua_clock::set_automatic(true) {
                Ok(()) => found.automatic = true,
                Err(e) => tracing::warn!(error = %e, "automatic time could not be turned on"),
            }
        }
        // `say(None)` WHEN ALL IS WELL, and that is half the contract: a line
        // that has to be cancelled by a second call is a line that stays up
        // after the thing it was about has been put right.
        report.say(if worth_saying(found) {
            notice_of(found)
        } else {
            None
        });
        Outcome::Done(Vec::new())
    }
}

/// Is there something to turn on?
///
/// Only where there is a client to run and it is not running. A machine with
/// no NTP client cannot be asked to start one, and a daemon that did not
/// answer has not said anything to act on.
#[must_use]
pub const fn should_turn_on(found: Found) -> bool {
    found.reachable && found.can_be_automatic && !found.automatic
}

/// A machine that cannot keep its own clock is not a machine with a problem.
///
/// TWO CASES, and neither is worth a line. One: `timedated` could not be
/// asked at all — a notice about the absence of something nobody asked for.
/// Two, and this is the one a photograph found: there is no time client
/// INSTALLED. Nothing here can start one that is not there, so the line would
/// have no button and would never go away — a permanent sentence on a footer
/// whose whole job is to be empty. What that machine is stays written in
/// `LEADER s l`, where somebody looking at the clock will read it.
///
/// It is the `emacs-daemon` watch's rule, which asks the same question before
/// it builds a sentence: a machine with no Emacs is not a machine with a
/// problem.
#[must_use]
pub const fn worth_saying(found: Found) -> bool {
    found.reachable && found.can_be_automatic
}

/// What the band should say, given what was found.
///
/// A pure function, which is what makes the whole of this a millisecond of
/// test. The cases are in the order somebody should hear them: the clock left
/// to itself, then a clock that is being looked after and has never managed to
/// agree with anybody, then nothing.
#[must_use]
pub fn notice_of(found: Found) -> Option<Notice> {
    if !found.automatic {
        // THE ONE WITH A BUTTON, and the button is the same thing this task
        // does on its own — which is the point: somebody who switched the task
        // off still gets one press rather than a sentence about a setting.
        return Some(Notice {
            task: ID.to_owned(),
            says: "clock-not-automatic".to_owned(),
            // THE BUTTON IS ALWAYS THERE when the line is: `worth_saying`
            // has already refused the machine that has nothing to start.
            act: Some("brain-clock-automatic".to_owned()),
        });
    }
    if !found.synchronised {
        // AND THE ONE WITHOUT ONE. The client is running and has not been able
        // to reach anybody; what is missing is a network, and there is no
        // button for that.
        return Some(Notice {
            task: ID.to_owned(),
            says: "clock-not-synchronised".to_owned(),
            act: None,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const WELL: Found = Found {
        reachable: true,
        automatic: true,
        can_be_automatic: true,
        synchronised: true,
    };

    /// A clock nobody is keeping gets a BUTTON; one that is kept and cannot
    /// reach anybody gets a sentence and no button.
    ///
    /// THE DEFECT THIS CATCHES: offering to turn on a client that is not
    /// installed. The press would do nothing and the refusal would arrive from
    /// the bus, which is a control that lies about what it can do.
    #[test]
    fn a_clock_left_to_itself_gets_a_button_and_a_missing_network_does_not() {
        let off = Found {
            automatic: false,
            ..WELL
        };
        let n = notice_of(off).expect("an unattended clock says so");
        assert_eq!(n.says, "clock-not-automatic");
        assert!(n.act.is_some(), "there is no way to turn it on");
        assert!(should_turn_on(off));

        // A MACHINE WITH NO TIME CLIENT SAYS NOTHING, because it can do
        // nothing: the line would have no button and would never go away.
        // What that machine is stays written in `LEADER s l`.
        let none_installed = Found {
            automatic: false,
            can_be_automatic: false,
            ..WELL
        };
        assert!(!worth_saying(none_installed));
        assert!(!should_turn_on(none_installed));

        let adrift = Found {
            synchronised: false,
            ..WELL
        };
        let n = notice_of(adrift).expect("a clock that never agreed says so");
        assert_eq!(n.says, "clock-not-synchronised");
        assert!(n.act.is_none(), "there is no button for a missing network");
        assert!(!should_turn_on(adrift));

        assert_eq!(notice_of(WELL), None, "a clock in hand raised a line");
    }

    /// A machine that could not be asked is not a machine with a problem.
    #[test]
    fn a_machine_that_cannot_keep_its_clock_is_not_a_machine_with_a_problem() {
        let nobody = Found::default();
        assert!(!worth_saying(nobody));
        assert!(
            !should_turn_on(nobody),
            "an unreachable daemon was written to"
        );
        assert!(worth_saying(WELL));
    }

    /// The watch's own cadence, and not the general default.
    ///
    /// THE DEFECT THIS CATCHES: `TaskPrefs::default()` is weekly, in the
    /// afternoon, with a minute of countdown — right for a rebuild and a lie
    /// on a watch's row, because the screen draws what the preferences resolve
    /// to.
    #[test]
    fn the_watch_chooses_its_own_cadence() {
        let mine = ClockSync::new().defaults();
        assert_eq!(mine.every_minutes, Some(EVERY_MINUTES));
        assert_eq!(mine.countdown_seconds, 0);
        assert_eq!(mine.part_of_day, PartOfDay::Any);
    }
}
