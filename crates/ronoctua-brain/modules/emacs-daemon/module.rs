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
//! # What it says, and the one thing it offers
//!
//! What it found is its [`Module::face`]: a notice while the daemon is down —
//! with the action `start` beside it — or while an Emacs that is not its
//! client is open. Nothing at all when everything is in hand, or when there is
//! no Emacs on the machine.
//!
//! # What it does NOT do
//!
//! Close anybody's editor. A standalone Emacs is somebody's work with unsaved
//! buffers in it; the notice says what it is and what to do about it, and the
//! person decides. The only thing this starts is the distro's own unit.
//!
//! Why it is a scheduled module and not a thread, and why its cadence is in
//! hours and not minutes, is in `why.md` beside this file.

use std::sync::Mutex;

use ronoctua_memory::emacs::{Emacsen, Places};

use crate::module::{
    Act, Acted, Birth, Ctx, Face, Look, Manifest, Module, Notice, NoticeKind, Run, Schedule,
    Scheduled,
};
use crate::moment::Moment;
use crate::prefs::TaskPrefs;
use crate::schedule::next_moment;
use crate::task::{Due, Outcome};

/// IT STAYS RARE, checked by the COMPILER rather than by a test.
///
/// What this guards is not the number — that is the owner's, in the manifest
/// — but the class of change that put the activity log back under one watch's
/// heel: every look writes a line, and the home page's log is a hundred lines
/// long. A build is where that argument belongs: a test would let the tree be
/// green for as long as nobody ran it.
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

/// The entry: one watch, reading the real `/proc`.
#[must_use]
pub fn new(_birth: &Birth) -> impl Module + use<> {
    EmacsDaemon::looking_in(Places::default())
}

/// What the last look found, and since when it has been saying it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Found {
    /// What was found.
    emacsen: Emacsen,
    /// Since when the line it makes has been up — kept across looks that
    /// find the same thing.
    since: Moment,
}

/// The watch.
struct EmacsDaemon {
    /// Where `/proc` is — handed in, so a test drives a planted tree.
    places: Places,
    /// What the last look found; `None` before the first, and after the
    /// daemon was started from here.
    found: Mutex<Option<Found>>,
}

impl EmacsDaemon {
    /// One reading this `/proc`.
    const fn looking_in(places: Places) -> Self {
        Self {
            places,
            found: Mutex::new(None),
        }
    }

    /// Writes down what a look found, keeping `since` while the line it makes
    /// is the same line.
    fn found(&self, emacsen: Emacsen, now: Moment) {
        let mut found = self
            .found
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let since = match *found {
            Some(was) if said(was.emacsen) == said(emacsen) => was.since,
            _ => now,
        };
        *found = Some(Found { emacsen, since });
    }
}

impl Module for EmacsDaemon {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn scheduled(&self) -> Option<&dyn Scheduled> {
        Some(self)
    }

    fn face(&self, _look: &Look<'_>) -> Face {
        let found = *self
            .found
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut face = Face::default();
        if let Some(f) = found
            && let Some(notice) = notice_of(f.emacsen, f.since)
        {
            face.notices.push(notice);
        }
        face
    }

    fn act(&self, act: &Act, ctx: &Ctx) -> Acted {
        if act.action != super::action::START.id {
            return Acted::Refused("unknown");
        }
        // A REHEARSING BRAIN starts nothing: the development loop's copy
        // would otherwise start the real session's daemon.
        if ctx.rehearsing() {
            return Acted::Refused("rehearsing");
        }
        // A USER UNIT, so `systemctl --user start` authorises through nothing
        // and asks nobody — and it returns once systemd took the job, which is
        // quick enough for the socket's thread.
        match crate::systemd::start(super::unit::WATCHED) {
            Ok(()) => {
                // THE LINE GOES WITH THE PRESS: systemd took the job, and the
                // next look — at its own cadence — says what came of it.
                *self
                    .found
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
                ctx.changed();
                Acted::Done
            }
            Err(crate::systemd::Trouble::Missing(_)) => Acted::Refused("no-systemctl"),
            Err(crate::systemd::Trouble::Refused(said)) => {
                tracing::warn!(said = %said, "the Emacs daemon could not be started");
                Acted::Refused(super::why::REFUSED)
            }
        }
    }
}

impl Scheduled for EmacsDaemon {
    fn due(&self, now: Moment, mine: &TaskPrefs, last: Option<Moment>) -> Due {
        // NO `systemd::reachable()` GATE, unlike the modules that start a unit
        // to do their work. This one's work is LOOKING, and looking is worth
        // doing on a machine where nothing can be started: the answer is then
        // "the daemon is not running", which is more than silence.
        //
        // THE ZONE IS NEVER CONSULTED, and handing one in would say it was:
        // a cadence in minutes short-circuits the calendar in `next_moment`
        // before a zone is reached.
        let at = next_moment(now, last, mine, &crate::moment::Fixed(0));
        if at <= now { Due::Now } else { Due::At(at) }
    }

    fn run(&self, run: &Run) -> Outcome {
        // A REHEARSAL IS THE SAME LOOK. Nothing here writes, starts or
        // deletes anything, so there is nothing for a rehearsal to hold back.
        self.found(ronoctua_memory::emacs::look(&self.places), run.ctx().now());
        Outcome::Done(Vec::new())
    }
}

/// Which line a look makes, if any — in the order somebody should hear them:
/// the one that stops them working, then the one that quietly costs them a
/// session, then nothing at all.
fn said(found: Emacsen) -> Option<&'static NoticeKind> {
    // A MACHINE WITH NO EMACS AT ALL is not a machine with a problem: a
    // notice about the absence of something nobody asked for.
    if !worth_saying(found) {
        return None;
    }
    if !found.daemon {
        return Some(&super::notice::NO_DAEMON);
    }
    (found.standalone > 0).then_some(&super::notice::NOT_THE_CLIENT)
}

/// What the strip should say, given what was found and since when.
///
/// A pure function, which is what makes a year of Emacsen a millisecond of
/// test.
fn notice_of(found: Emacsen, since: Moment) -> Option<Notice> {
    let kind = said(found)?;
    Some(if kind.id == super::notice::NO_DAEMON.id {
        // THE ONE WITH A BUTTON. Starting a user unit asks nobody anything.
        Notice::of(kind, super::key::EMACS_DAEMON_SAYS_NO_DAEMON, since)
            .control(&super::action::START)
    } else {
        // AND THE ONE WITHOUT ONE. What is open is somebody's work, with
        // buffers in it that nothing here knows whether they have saved.
        Notice::of(kind, super::key::EMACS_DAEMON_SAYS_NOT_THE_CLIENT, since)
    })
}

/// A machine with no Emacs at all is not a machine with a problem.
const fn worth_saying(found: Emacsen) -> bool {
    found.daemon || found.clients > 0 || found.standalone > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Level;
    use crate::pulse::Pulse;

    #[test]
    fn a_dead_daemon_gets_a_button_and_a_wrong_emacs_does_not() {
        let down = Emacsen {
            daemon: false,
            clients: 0,
            standalone: 1,
        };
        let n = notice_of(down, 5).expect("a daemon that is not running says so");
        assert_eq!(n.says, super::super::key::EMACS_DAEMON_SAYS_NO_DAEMON);
        assert_eq!(
            n.controls,
            vec![super::super::action::START.id],
            "there is no way to start it"
        );
        assert_eq!((n.level, n.since), (Level::Warn, 5));

        let wrong = Emacsen {
            daemon: true,
            clients: 0,
            standalone: 1,
        };
        let n = notice_of(wrong, 5).expect("a second copy of the editor says so");
        assert_eq!(n.says, super::super::key::EMACS_DAEMON_SAYS_NOT_THE_CLIENT);
        assert_eq!(n.level, Level::Info);
        assert!(
            n.controls.is_empty(),
            "the screen offered to close somebody's editor"
        );

        let well = Emacsen {
            daemon: true,
            clients: 2,
            standalone: 0,
        };
        assert!(notice_of(well, 5).is_none(), "a notice with nothing wrong");
    }

    /// A MACHINE WITH NO EMACS IS NOT A MACHINE WITH A PROBLEM.
    #[test]
    fn nothing_running_is_nothing_to_say() {
        assert!(!worth_saying(Emacsen::default()));
        assert!(notice_of(Emacsen::default(), 1).is_none());
        assert!(worth_saying(Emacsen {
            daemon: true,
            ..Emacsen::default()
        }));
        assert!(worth_saying(Emacsen {
            standalone: 1,
            ..Emacsen::default()
        }));
    }

    #[test]
    fn what_a_look_found_is_its_face_and_the_line_keeps_its_age() {
        let watch = EmacsDaemon::looking_in(Places::default());
        let (pulse, mine) = (Pulse::default(), super::super::MANIFEST.defaults());
        let look = Look {
            now: 100,
            pulse: &pulse,
            mine: &mine,
            last: None,
        };
        assert_eq!(watch.face(&look), Face::default(), "a face before any look");
        let down = Emacsen {
            daemon: false,
            clients: 1,
            standalone: 0,
        };
        watch.found(down, 10);
        watch.found(down, 20);
        let face = watch.face(&look);
        assert_eq!(face.notices.len(), 1);
        assert_eq!(face.notices[0].since, 10, "the same line lost its age");
        // A DIFFERENT LINE is a new one.
        watch.found(
            Emacsen {
                daemon: true,
                clients: 0,
                standalone: 1,
            },
            30,
        );
        assert_eq!(watch.face(&look).notices[0].since, 30);
    }

    /// It comes round in MINUTES, and never through the calendar.
    #[test]
    fn it_comes_round_in_minutes_of_its_own() {
        let watch = EmacsDaemon::looking_in(Places::default());
        let mine = crate::prefs::Prefs::default().of_module(&super::super::MANIFEST);
        let every = mine.every_minutes.expect("a watch with no cadence");
        assert!(every >= 60);
        assert_eq!(mine.countdown_seconds, 0, "a countdown to a look");
        assert!(!super::super::MANIFEST.schedule.is_some_and(|s| s.calendar));
        let gap = Moment::from(every) * 60;
        // Never run: due now.
        assert_eq!(watch.due(1_000, &mine, None), Due::Now);
        // Just run: due one cadence later, to the second.
        assert_eq!(watch.due(1_000, &mine, Some(1_000)), Due::At(1_000 + gap));
        assert_eq!(watch.due(1_000 + gap, &mine, Some(1_000)), Due::Now);
    }

    #[test]
    fn a_rehearsing_brain_starts_no_daemon_and_an_unknown_action_is_refused() {
        let watch = EmacsDaemon::looking_in(Places::default());
        let (ctx, _) = Ctx::kept(super::super::ID, 1);
        let rehearsing = Ctx::new(
            super::super::ID,
            1,
            crate::prefs::Where::default(),
            true,
            std::sync::Arc::new(crate::module::Kept::default()),
        );
        let start = Act {
            action: super::super::action::START.id.to_owned(),
            ..Act::default()
        };
        assert_eq!(watch.act(&start, &rehearsing), Acted::Refused("rehearsing"));
        assert_eq!(
            watch.act(
                &Act {
                    action: "fly".to_owned(),
                    ..Act::default()
                },
                &ctx
            ),
            Acted::Refused("unknown")
        );
        // THE ACTION IS ITS MANIFEST'S, under the command the leader lists.
        assert_eq!(super::super::action::START.command, "emacs-daemon-start");
        assert_eq!(
            super::super::MANIFEST.why_key(super::super::why::REFUSED),
            Some(super::super::key::EMACS_DAEMON_WHY_REFUSED),
            "a refused start is said in the core's words, not its own"
        );
    }
}
