//! The watch on the machine's own clock, and its switch.
//!
//! # What it is for
//!
//! The owner asked for automatic time-keeping to be the way this distro runs,
//! for the switch to be on the date-and-time screen, and for the same switch
//! to be reachable from the brain: *"deve existir a atualização automática do
//! relógio/etc do sistema/ronoctua-distro via internet, sendo essa a
//! configuração padrão em vez do ajuste manual"*.
//!
//! It reads whether automatic time is on, turns it on when it is not, and SAYS
//! when the clock is unattended or has never managed to agree with anybody.
//! Switching this module off is how somebody says they want the clock to be
//! theirs. Why it does not fetch the time itself is in `why.md`.
//!
//! # What it says, and the one thing it offers
//!
//! What it found is its [`Module::face`]: a notice while nothing keeps the
//! clock — with the action `keep` beside it, which is what the watch itself
//! does on its schedule — or while the clock is kept and has never agreed
//! with a server.
//!
//! A PRESS IS DONE NOW, and not queued: a queued run waits for the gate (held
//! on battery) and behind whatever else is running (a two-hour update), which
//! is a button that answers "taken" and then does nothing for hours. It is
//! not done on the socket's thread either — turning the clock on is two calls
//! on the system bus, and timedated may have to be started first — so it runs
//! on a thread of its own, and the press waits a moment for it
//! ([`PATIENCE`]). In time, the answer is the press's: done, or refused in
//! this module's own sentence. Late, the press is taken and the answer lands
//! where a late answer can be read: the strip's line says it was refused, and
//! the activity log keeps the machine's words.

use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use crate::module::{
    Act, Acted, Arg, Birth, Ctx, Face, Look, Manifest, Module, Noted, Notice, NoticeKind, Run,
    Schedule, Scheduled,
};
use crate::moment::Moment;
use crate::prefs::TaskPrefs;
use crate::schedule::next_moment;
use crate::task::{Due, Outcome};

/// IT STAYS RARE, checked by the COMPILER: every look is a line in the
/// activity log, and a watch that ran every few minutes answered "what has
/// this machine been doing" with hundreds of repetitions of "nothing".
const _: () = assert!(
    matches!(
        super::MANIFEST.schedule,
        Some(Schedule {
            every_minutes: Some(minutes),
            ..
        }) if minutes >= 60
    ),
    "a watch that runs more than once an hour drowns the activity log"
);

/// How long a press waits for the bus before it answers "taken".
///
/// Under the half second a client waits on the socket, with room for the
/// rest of the answer: the lock, the face published again and the line
/// written back. A bus that answers in time — almost always — gives the press
/// its real answer; one that does not still gets its answer said, later, on
/// the strip and in the activity log.
const PATIENCE: Duration = Duration::from_millis(300);

/// The entry: one watch, asking the real `timedated`.
#[must_use]
pub fn new(_birth: &Birth) -> impl Module + use<> {
    ClockSync::asking(None, turn_on_for_real)
}

/// Turns the machine's automatic time-keeping on, or says what the machine
/// said instead — in its own words, which are the ones worth keeping.
fn turn_on_for_real() -> Result<(), String> {
    ronoctua_clock::set_automatic(true).map_err(|e| format!("{e:#}"))
}

/// What the watch found, which is the part with no bus in it.
///
/// A copy of the fields the rule below reads, so that a year of clocks is a
/// millisecond of test. `ronoctua_clock::Standing` is what fills it on a real
/// machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Found {
    /// Did timedated answer at all?
    reachable: bool,
    /// Is a network time client running?
    automatic: bool,
    /// Can one be? `false` where none is installed.
    can_be_automatic: bool,
    /// Has the clock actually been set by it?
    synchronised: bool,
}

impl Found {
    /// What `ronoctua_clock` said, in the fields this rule reads.
    const fn of(said: &ronoctua_clock::Standing) -> Self {
        Self {
            reachable: said.reachable,
            automatic: said.automatic,
            can_be_automatic: said.can_be_automatic,
            synchronised: said.synchronised,
        }
    }
}

/// What the watch and the presses touch: the bus, or what a test planted.
///
/// `Copy`, because a press hands it to a thread of its own.
#[derive(Debug, Clone, Copy)]
struct Keeper {
    /// What a test planted in place of the bus; `None` asks the real daemon.
    planted: Option<Found>,
    /// What turns the keeping on — the bus, or a test's stand-in.
    turn_on: fn() -> Result<(), String>,
}

impl Keeper {
    /// What the machine says, or what the test planted.
    fn look(self) -> Found {
        if let Some(planted) = self.planted {
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

    /// Looks, and — when `acts` — turns the keeping on where it is off.
    ///
    /// What it found, and the machine's words when turning it on was refused.
    fn keep(self, acts: bool) -> (Found, Option<String>) {
        let mut found = self.look();
        if acts && should_turn_on(found) {
            match (self.turn_on)() {
                Ok(()) => found.automatic = true,
                Err(said) => {
                    tracing::warn!(said = %said, "automatic time could not be turned on");
                    return (found, Some(said));
                }
            }
        }
        (found, None)
    }
}

/// What the last look found, since when its line has been up, and whether
/// the last try at turning the keeping on was refused.
#[derive(Debug, Default)]
struct Seen {
    last: Option<(Found, Moment)>,
    refused: bool,
}

impl Seen {
    /// Writes down what was found, keeping `since` while its line is the same.
    fn found(&mut self, found: Found, refused: bool, now: Moment) {
        let since = match self.last {
            Some((was, since)) if said(was) == said(found) => since,
            _ => now,
        };
        self.last = Some((found, since));
        self.refused = refused;
    }
}

/// Where a press and its thread meet: the thread's answer, or the press
/// having stopped waiting for it.
#[derive(Debug)]
enum Meeting {
    Waiting,
    /// The machine's words when it refused, `None` when it took it.
    Answered(Option<String>),
    GaveUp,
}

/// The watch.
#[derive(Debug)]
struct ClockSync {
    keeper: Keeper,
    /// Shared with a press's thread, which writes what it found.
    seen: Arc<Mutex<Seen>>,
}

impl ClockSync {
    /// One that asks the real daemon (`None`), or is handed what it would
    /// have found — what a test drives — and what turns the keeping on.
    fn asking(planted: Option<Found>, turn_on: fn() -> Result<(), String>) -> Self {
        Self {
            keeper: Keeper { planted, turn_on },
            seen: Arc::new(Mutex::new(Seen::default())),
        }
    }

    /// What the last look found.
    fn seen(&self) -> std::sync::MutexGuard<'_, Seen> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A press: the keeping turned on, on a thread of its own, and the
    /// answer while it is still worth waiting for.
    fn press(&self, ctx: &Ctx) -> Acted {
        let meeting = Arc::new((Mutex::new(Meeting::Waiting), Condvar::new()));
        let (keeper, seen, later, met) = (
            self.keeper,
            Arc::clone(&self.seen),
            ctx.detached(),
            Arc::clone(&meeting),
        );
        let started = std::thread::Builder::new()
            .name("clock-sync-keep".into())
            .spawn(move || {
                let (found, refused) = keeper.keep(true);
                seen.lock().unwrap_or_else(PoisonError::into_inner).found(
                    found,
                    refused.is_some(),
                    later.now(),
                );
                {
                    let (lock, woken) = &*met;
                    let mut m = lock.lock().unwrap_or_else(PoisonError::into_inner);
                    if matches!(*m, Meeting::Waiting) {
                        *m = Meeting::Answered(refused.clone());
                        woken.notify_one();
                    }
                }
                // THE FACE MOVED either way, and a refusal is KEPT: the press
                // may have stopped waiting, and the log is where the
                // machine's own words stay to be read.
                later.changed();
                if let Some(said) = refused {
                    later.record(
                        Noted::of(super::key::CLOCK_SYNC_NOTED_REFUSED)
                            .with("said", Arg::Text(said)),
                    );
                }
            });
        if let Err(e) = started {
            // NO THREAD, and still an answer that does the work: the watch's
            // own run is the same look and the same switch.
            tracing::warn!(error = %e, "no thread for the clock's press; queued instead");
            return Acted::Queue;
        }
        let (lock, woken) = &*meeting;
        let waiting = lock.lock().unwrap_or_else(PoisonError::into_inner);
        let (mut m, _) = woken
            .wait_timeout_while(waiting, PATIENCE, |m| matches!(m, Meeting::Waiting))
            .unwrap_or_else(PoisonError::into_inner);
        match std::mem::replace(&mut *m, Meeting::GaveUp) {
            Meeting::Answered(Some(_)) => Acted::Refused(super::why::REFUSED),
            // TAKEN, in time or not: late, what came of it is the strip's
            // line and the log's, which the thread writes when it knows.
            Meeting::Answered(None) | Meeting::Waiting | Meeting::GaveUp => Acted::Done,
        }
    }
}

impl Module for ClockSync {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn scheduled(&self) -> Option<&dyn Scheduled> {
        Some(self)
    }

    fn face(&self, _look: &Look<'_>) -> Face {
        let (last, refused) = {
            let seen = self.seen();
            (seen.last, seen.refused)
        };
        let mut face = Face::default();
        if let Some((found, since)) = last
            && let Some(notice) = notice_of(found, refused, since)
        {
            face.notices.push(notice);
        }
        face
    }

    fn act(&self, act: &Act, ctx: &Ctx) -> Acted {
        if act.action != super::action::KEEP.id {
            return Acted::Refused("unknown");
        }
        // A REHEARSING BRAIN changes no clock: the development loop's copy
        // would otherwise reach the real machine's time keeping.
        if ctx.rehearsing() {
            return Acted::Refused("rehearsing");
        }
        // NOW, on a thread of its own — see the module's documentation.
        self.press(ctx)
    }
}

impl Scheduled for ClockSync {
    fn due(&self, now: Moment, mine: &TaskPrefs, last: Option<Moment>) -> Due {
        // THE ZONE IS NEVER CONSULTED, and handing one in would say it was: a
        // cadence in minutes short-circuits the calendar in `next_moment`
        // before a zone is reached.
        let at = next_moment(now, last, mine, &crate::moment::Fixed(0));
        if at <= now { Due::Now } else { Due::At(at) }
    }

    fn run(&self, run: &Run) -> Outcome {
        // TURNING IT BACK ON IS THE WORK, and it is the only thing here that
        // writes. A rehearsal must not do it — `m e` asks what would happen,
        // and a rehearsal that changed the machine's clock-keeping would be
        // the one question in this system that cannot be asked safely.
        let (found, refused) = self.keeper.keep(!run.is_rehearsal());
        self.seen().found(found, refused.is_some(), run.ctx().now());
        match refused {
            None => Outcome::Done(Vec::new()),
            // A REFUSAL IS A FAILED RUN, in the machine's words: the row and
            // the activity log say why, and the strip's line says it was
            // refused. It was recorded as "worked" once, with the line left up
            // and no reason anywhere.
            Some(said) => Outcome::Failed {
                at: String::new(),
                why: as_trouble(said),
            },
        }
    }
}

/// The machine's words as a failed run's trouble.
///
/// A run's `why` with no space in it is read as a WORD of the brain's
/// (`RunRecord::of`), and drawn as a key that does not exist; the machine's
/// words always have one, and this makes sure of it.
fn as_trouble(said: String) -> String {
    if said.contains(' ') {
        said
    } else {
        format!("timedated: {said}")
    }
}

/// Is there something to turn on?
///
/// Only where there is a client to run and it is not running. A machine with
/// no NTP client cannot be asked to start one, and a daemon that did not
/// answer has not said anything to act on.
const fn should_turn_on(found: Found) -> bool {
    found.reachable && found.can_be_automatic && !found.automatic
}

/// A machine that cannot keep its own clock is not a machine with a problem.
///
/// TWO CASES, and neither is worth a line. One: `timedated` could not be
/// asked at all — a notice about the absence of something nobody asked for.
/// Two, and this is the one a photograph found: there is no time client
/// INSTALLED. Nothing here can start one that is not there, so the line would
/// have no button and would never go away — a permanent sentence on a footer
/// whose whole job is to be empty. What that machine is stays written on the
/// date-and-time page, where somebody looking at the clock will read it.
const fn worth_saying(found: Found) -> bool {
    found.reachable && found.can_be_automatic
}

/// Which line a look makes, if any: the clock left to itself, then a clock
/// that is looked after and has never agreed with anybody, then nothing.
fn said(found: Found) -> Option<&'static NoticeKind> {
    if !worth_saying(found) {
        return None;
    }
    if !found.automatic {
        return Some(&super::notice::NOT_AUTOMATIC);
    }
    (!found.synchronised).then_some(&super::notice::NOT_SYNCHRONISED)
}

/// What the strip should say, given what was found, whether the last try at
/// turning the keeping on was refused, and since when.
fn notice_of(found: Found, refused: bool, since: Moment) -> Option<Notice> {
    let kind = said(found)?;
    Some(if kind.id == super::notice::NOT_AUTOMATIC.id {
        // THE ONE WITH A BUTTON, and the button is the same thing this watch
        // does on its own — which is the point: somebody who switched the
        // watch off still gets one press rather than a sentence about a
        // setting. A REFUSED TRY says so, and where to look; the button stays
        // for the next try.
        let says = if refused {
            super::key::CLOCK_SYNC_SAYS_REFUSED
        } else {
            super::key::CLOCK_SYNC_SAYS_NOT_AUTOMATIC
        };
        Notice::of(kind, says, since).control(&super::action::KEEP)
    } else {
        // AND THE ONE WITHOUT ONE. The client is running and has not been
        // able to reach anybody; what is missing is a network, and there is
        // no button for that.
        Notice::of(kind, super::key::CLOCK_SYNC_SAYS_NOT_SYNCHRONISED, since)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Level;
    use crate::pulse::Pulse;

    const WELL: Found = Found {
        reachable: true,
        automatic: true,
        can_be_automatic: true,
        synchronised: true,
    };

    /// A bus that takes it.
    #[allow(clippy::unnecessary_wraps)]
    const fn takes() -> Result<(), String> {
        Ok(())
    }

    /// A bus that refuses, in the machine's words.
    fn refuses() -> Result<(), String> {
        Err("Access denied by the fixture".to_owned())
    }

    /// A bus that refuses, after the press stopped waiting.
    fn refuses_late() -> Result<(), String> {
        std::thread::sleep(PATIENCE * 3);
        refuses()
    }

    /// A clock nobody keeps.
    const OFF: Found = Found {
        automatic: false,
        ..WELL
    };

    /// The keep action, pressed.
    fn keep() -> Act {
        Act {
            action: super::super::action::KEEP.id.to_owned(),
            row: None,
            answer: None,
        }
    }

    /// The face's one notice, if any: what it says.
    fn says(watch: &ClockSync) -> Option<std::borrow::Cow<'static, str>> {
        let (pulse, mine) = (Pulse::default(), super::super::MANIFEST.defaults());
        watch
            .face(&Look {
                now: 50,
                pulse: &pulse,
                mine: &mine,
                last: None,
            })
            .notices
            .first()
            .map(|n| n.says.clone())
    }

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
        let n = notice_of(off, false, 3).expect("an unattended clock says so");
        assert_eq!(n.says, super::super::key::CLOCK_SYNC_SAYS_NOT_AUTOMATIC);
        assert_eq!(
            n.controls,
            vec![super::super::action::KEEP.id],
            "there is no way to turn it on"
        );
        assert_eq!(n.level, Level::Warn);
        assert!(should_turn_on(off));

        // A MACHINE WITH NO TIME CLIENT SAYS NOTHING, because it can do
        // nothing: the line would have no button and would never go away.
        let none_installed = Found {
            automatic: false,
            can_be_automatic: false,
            ..WELL
        };
        assert!(!worth_saying(none_installed));
        assert!(!should_turn_on(none_installed));
        assert!(notice_of(none_installed, false, 3).is_none());

        let adrift = Found {
            synchronised: false,
            ..WELL
        };
        let n = notice_of(adrift, false, 3).expect("a clock that never agreed says so");
        assert_eq!(n.says, super::super::key::CLOCK_SYNC_SAYS_NOT_SYNCHRONISED);
        assert_eq!(n.level, Level::Info);
        assert!(
            n.controls.is_empty(),
            "there is no button for a missing network"
        );
        assert!(!should_turn_on(adrift));

        assert_eq!(
            notice_of(WELL, false, 3),
            None,
            "a clock in hand raised a line"
        );
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
        let mine = crate::prefs::Prefs::default().of_module(&super::super::MANIFEST);
        assert!(mine.every_minutes.is_some_and(|m| m >= 60));
        assert_eq!(mine.countdown_seconds, 0);
        assert_eq!(mine.part_of_day, crate::prefs::PartOfDay::Any);
        let watch = ClockSync::asking(Some(WELL), takes);
        let gap = Moment::from(mine.every_minutes.unwrap_or(0)) * 60;
        assert_eq!(watch.due(1_000, &mine, None), Due::Now);
        assert_eq!(watch.due(1_000, &mine, Some(1_000)), Due::At(1_000 + gap));
    }

    /// A press does it NOW, and says what came of it.
    ///
    /// THE DEFECT THIS CATCHES: a press that was queued — held by the gate on
    /// battery, waiting behind a two-hour update — and a refusal recorded as a
    /// run that worked, with the line left up and no reason anywhere. `asking`
    /// plants the bus: nothing here reaches the machine's real timedated.
    #[test]
    fn a_press_does_it_now_and_says_when_it_was_refused() {
        // TAKEN: the line goes, and nothing is queued.
        let watch = ClockSync::asking(Some(OFF), takes);
        let (ctx, kept) = Ctx::kept(super::super::ID, 40);
        assert_eq!(watch.act(&keep(), &ctx), Acted::Done);
        assert_eq!(says(&watch), None, "a clock turned on still raised a line");
        assert!(
            kept.records().is_empty(),
            "a press that worked wrote a record"
        );

        // REFUSED IN TIME: the press says so in the module's own sentence, the
        // line says it was refused — with the button, for the next try — and
        // the log keeps the machine's words.
        let watch = ClockSync::asking(Some(OFF), refuses);
        let (ctx, kept) = Ctx::kept(super::super::ID, 40);
        assert_eq!(
            watch.act(&keep(), &ctx),
            Acted::Refused(super::super::why::REFUSED)
        );
        assert_eq!(
            super::super::MANIFEST.why_key(super::super::why::REFUSED),
            Some(super::super::key::CLOCK_SYNC_WHY_REFUSED)
        );
        assert_eq!(
            says(&watch).as_deref(),
            Some(super::super::key::CLOCK_SYNC_SAYS_REFUSED)
        );
        let noted = wait_for_a_record(&kept);
        assert_eq!(noted.says, super::super::key::CLOCK_SYNC_NOTED_REFUSED);
        assert_eq!(
            noted.args,
            vec![(
                std::borrow::Cow::Borrowed("said"),
                Arg::Text("Access denied by the fixture".to_owned())
            )]
        );

        // REFUSED LATE: the press was taken, and the refusal still arrives —
        // on the strip's line and in the log, with the face published again.
        let watch = ClockSync::asking(Some(OFF), refuses_late);
        let (ctx, kept) = Ctx::kept(super::super::ID, 40);
        assert_eq!(watch.act(&keep(), &ctx), Acted::Done);
        let noted = wait_for_a_record(&kept);
        assert_eq!(noted.says, super::super::key::CLOCK_SYNC_NOTED_REFUSED);
        assert!(
            !kept.changes().is_empty(),
            "the late answer was not published"
        );
        assert_eq!(
            says(&watch).as_deref(),
            Some(super::super::key::CLOCK_SYNC_SAYS_REFUSED)
        );
    }

    /// The first record a press's thread writes, waited for.
    fn wait_for_a_record(kept: &crate::module::Kept) -> Noted {
        for _ in 0..200 {
            if let Some((_, _, noted)) = kept.records().into_iter().next() {
                return noted;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the press's thread wrote nothing");
    }

    #[test]
    fn a_rehearsing_brain_and_an_unknown_action_are_refused() {
        let watch = ClockSync::asking(Some(OFF), refuses);
        let (ctx, kept) = Ctx::kept(super::super::ID, 40);
        // A REHEARSING BRAIN declines, and says why.
        let rehearsing = Ctx::new(
            super::super::ID,
            40,
            crate::prefs::Where::default(),
            true,
            std::sync::Arc::new(crate::module::Kept::default()),
        );
        assert_eq!(
            watch.act(&keep(), &rehearsing),
            Acted::Refused("rehearsing")
        );
        assert_eq!(
            watch.act(
                &Act {
                    action: "other".to_owned(),
                    row: None,
                    answer: None
                },
                &ctx
            ),
            Acted::Refused("unknown")
        );
        assert!(kept.records().is_empty() && kept.changes().is_empty());
        // THE ACTION IS ITS MANIFEST'S, under the command the leader lists.
        assert_eq!(super::super::action::KEEP.command, "clock-sync-keep");
    }

    /// A scheduled run that was refused FAILED, in the machine's words.
    #[test]
    fn a_refused_run_failed_and_says_why() {
        let watch = ClockSync::asking(Some(OFF), refuses);
        let (ctx, _) = Ctx::kept(super::super::ID, 40);
        let report = crate::task::Report::at(
            std::path::PathBuf::from("/nonexistent/clock.json"),
            std::time::Duration::ZERO,
            std::time::Duration::ZERO,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert_eq!(
            watch.run(&Run::new(report, ctx)),
            Outcome::Failed {
                at: String::new(),
                why: "Access denied by the fixture".to_owned()
            }
        );
        assert_eq!(
            says(&watch).as_deref(),
            Some(super::super::key::CLOCK_SYNC_SAYS_REFUSED)
        );
        // A WORD WOULD BE READ AS THE BRAIN'S: the machine's words keep a space.
        assert_eq!(as_trouble("denied".to_owned()), "timedated: denied");
    }

    #[test]
    fn what_a_rehearsed_look_found_is_its_face() {
        // A REHEARSAL LOOKS and changes nothing, and what it found is still
        // what the strip says.
        let watch = ClockSync::asking(
            Some(Found {
                synchronised: false,
                ..WELL
            }),
            refuses,
        );
        let (ctx, _) = Ctx::kept(super::super::ID, 40);
        let report = crate::task::Report::at(
            std::path::PathBuf::from("/nonexistent/clock.json"),
            std::time::Duration::ZERO,
            std::time::Duration::ZERO,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .rehearsing();
        assert_eq!(watch.run(&Run::new(report, ctx)), Outcome::Done(Vec::new()));
        let (pulse, mine) = (Pulse::default(), super::super::MANIFEST.defaults());
        let face = watch.face(&Look {
            now: 50,
            pulse: &pulse,
            mine: &mine,
            last: None,
        });
        assert_eq!(face.notices.len(), 1);
        assert_eq!(
            face.notices[0].id,
            super::super::notice::NOT_SYNCHRONISED.id
        );
        assert_eq!(face.notices[0].since, 40);
    }
}
