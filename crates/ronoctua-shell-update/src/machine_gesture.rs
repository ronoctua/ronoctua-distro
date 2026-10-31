//! THE MACHINE'S OWN GESTURES ON THE UPDATE'S PAGE (owner, 2026-10-31): the
//! system whose boot trial failed, tried again at the next update; the
//! machine held on the distro revision it runs, and let go. The records the
//! view draws above the steps offer each where its sentence is — the
//! failed trial's, the hold's — in both doors.
//!
//! # Asked twice, the no first
//!
//! Each is root's, so a click on its row asks first, there, under the hand
//! that clicked: the question's `no` drawn first and the cursor born on it
//! ([`Question`], `aim::Aim`: the first click picks, a click on the pick
//! answers). A yes is asked again in the sudo bar, above the modeline —
//! through the door's gate, on a thread, never the loop — unless the
//! passwordless window answers it. What came of it is said in the door that
//! asked ([`Came`]): done, the door's refusal (cancelled, a wrong password,
//! no footer), or the machine's own sentence for why it refused
//! (`machine-refused-<word>`), carried back by the helper as a progress word.
//!
//! # No word of the person's reaches root
//!
//! The helper's verbs take none: the revision held is the one the machine
//! reads as running, and the system tried again is the one its record says
//! failed (`ronoctua_security::verb::RootVerb::MachineHold`…). What this
//! page shows is the reason to ask, never what root acts on.

use ronoctua_security::verb::{RootVerb, Said};
use ronoctua_shell_kit::aim::Aim;

use crate::key;

/// A gesture of the machine's the update's records offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gesture {
    /// Try again, at the next update, the system whose boot trial failed.
    TryAgain,
    /// Hold the machine on the distro revision it runs.
    Hold,
    /// Let go of the hold.
    LetGo,
}

impl Gesture {
    /// Every one.
    pub const ALL: [Self; 3] = [Self::TryAgain, Self::Hold, Self::LetGo];

    /// The sudo door's verb, which takes no word.
    #[must_use]
    pub const fn verb(self) -> RootVerb {
        match self {
            Self::TryAgain => RootVerb::MachineTryAgain,
            Self::Hold => RootVerb::MachineHold,
            Self::LetGo => RootVerb::MachineUnhold,
        }
    }

    /// The command that asks it by name (`LEADER :`).
    #[must_use]
    pub const fn command(self) -> &'static str {
        match self {
            Self::TryAgain => TRY_AGAIN,
            Self::Hold => HOLD,
            Self::LetGo => LET_GO,
        }
    }

    /// The gesture a command asks.
    #[must_use]
    pub fn of_command(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.command() == id)
    }

    /// The question asked before the sudo bar.
    #[must_use]
    pub const fn question_key(self) -> &'static str {
        match self {
            Self::TryAgain => key::MACHINE_TRY_AGAIN_QUESTION,
            Self::Hold => key::MACHINE_HOLD_QUESTION,
            Self::LetGo => key::MACHINE_LET_GO_QUESTION,
        }
    }

    /// Its answer: `no` (`false`), drawn first, or `yes`.
    #[must_use]
    pub const fn answer_key(self, yes: bool) -> &'static str {
        match (self, yes) {
            (Self::TryAgain, false) => key::MACHINE_TRY_AGAIN_NO,
            (Self::TryAgain, true) => key::MACHINE_TRY_AGAIN_YES,
            (Self::Hold, false) => key::MACHINE_HOLD_NO,
            (Self::Hold, true) => key::MACHINE_HOLD_YES,
            (Self::LetGo, false) => key::MACHINE_LET_GO_NO,
            (Self::LetGo, true) => key::MACHINE_LET_GO_YES,
        }
    }

    /// What is said when it was done.
    #[must_use]
    pub const fn done_key(self) -> &'static str {
        match self {
            Self::TryAgain => key::MACHINE_SAID_TRY_AGAIN,
            Self::Hold => key::MACHINE_SAID_HOLD,
            Self::LetGo => key::MACHINE_SAID_LET_GO,
        }
    }

    /// What it is, in words for a photograph's record of what it would have
    /// asked.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::TryAgain => "to try the system that did not start again",
            Self::Hold => "to hold the machine on the revision it runs",
            Self::LetGo => "to let go of the machine's hold",
        }
    }
}

/// The command that asks to try again the system whose trial failed.
pub const TRY_AGAIN: &str = "machine-try-again";
/// The command that asks to hold the machine on the revision it runs.
pub const HOLD: &str = "machine-hold";
/// The command that asks to let go of the hold.
pub const LET_GO: &str = "machine-let-go";

/// The question a gesture asks on the page, before the sudo bar does: its
/// `no` first, and the cursor born there — the screen's, so the first click
/// on it only picks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// The gesture asked about.
    pub what: Gesture,
    /// Which answer the cursor is on (`true` is yes), and whether a person
    /// put it there.
    aim: Aim<bool>,
}

impl Question {
    /// The question about `what`, its cursor born on `no`.
    #[must_use]
    pub const fn of(what: Gesture) -> Self {
        Self {
            what,
            aim: Aim::born(false),
        }
    }

    /// The answer the cursor is on — `true` is yes.
    #[must_use]
    pub fn yes(&self) -> bool {
        self.aim.on().copied().unwrap_or(false)
    }

    /// A key put the cursor on an answer.
    pub fn moved(&mut self, yes: bool) {
        self.aim.moved(yes);
    }

    /// A click on an answer: `true` when it answers — the person's own pick,
    /// clicked again.
    pub fn clicked(&mut self, yes: bool) -> bool {
        self.aim.clicked(yes)
    }
}

/// What came of a gesture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Came {
    /// The machine did it.
    Done(Gesture),
    /// The machine refused it, in its own word (`ronoctua-machine`'s
    /// `refused <word>`).
    Refused(String),
    /// The door refused it: cancelled, a wrong password, no footer — or a
    /// helper that failed with no word of the machine's.
    Door(ronoctua_security::Why),
}

/// Runs `what` through the door's `gesture`, blocking — a door's thread,
/// never its loop.
pub fn run(what: Gesture, mut gesture: crate::sudo::Gesture) -> Came {
    let mut word = None;
    let ran = gesture.run(what.verb().into(), &mut |step| {
        if let Some(said) = Said::machine_refusal(step) {
            word = Some(said.to_owned());
        }
    });
    match (ran, word) {
        (Ok(()), _) => Came::Done(what),
        (Err(_), Some(word)) => Came::Refused(word),
        (Err(why), None) => Came::Door(why),
    }
}

/// The sentence of what came of a gesture: the machine's own sentence for
/// its refusal — a word it never says is the door's `failed`, never a
/// reason invented.
#[must_use]
pub fn said(came: &Came, texts: &ronoctua_i18n::Texts) -> String {
    match came {
        Came::Done(what) => texts.t(what.done_key()),
        Came::Refused(word) => texts.t(crate::update_window::refusal_key(word).unwrap_or(
            crate::security_words::refusal(ronoctua_security::Why::Failed),
        )),
        Came::Door(why) => texts.t(crate::security_words::refusal(*why)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a language")
    }

    /// FAILS IF: a gesture runs another verb of the helper's than its own,
    /// or one that carries a word; or what came of it is said in no words,
    /// or the machine's refusal is said as anything but the machine's own
    /// sentence.
    #[test]
    fn each_gesture_is_its_own_wordless_verb_and_says_what_came() {
        for what in Gesture::ALL {
            let log = Arc::new(Mutex::new(Vec::new()));
            let gate = crate::sudo::Gate::fake(Arc::clone(&log), Ok(()));
            assert_eq!(
                run(what, gate.gesture(crate::sudo::Asker::Update)),
                Came::Done(what)
            );
            let asked = log.lock().expect("log").clone();
            assert_eq!(asked, vec![(crate::sudo::Asker::Update, what.verb())]);
            assert_eq!(what.verb().arguments().len(), 1, "{what:?}");
            assert_eq!(Gesture::of_command(what.command()), Some(what));
        }
        let gate = crate::sudo::Gate::fake(
            Arc::new(Mutex::new(Vec::new())),
            Err(ronoctua_security::Why::Cancelled),
        );
        assert_eq!(
            run(Gesture::Hold, gate.gesture(crate::sudo::Asker::Update)),
            Came::Door(ronoctua_security::Why::Cancelled)
        );
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for what in Gesture::ALL {
                for k in [
                    what.question_key(),
                    what.answer_key(false),
                    what.answer_key(true),
                    what.done_key(),
                ] {
                    assert!(!t.t(k).starts_with('['), "{language}: {k}");
                }
                assert_ne!(t.t(what.answer_key(false)), t.t(what.answer_key(true)));
            }
            assert_eq!(
                said(&Came::Refused("busy".to_owned()), &t),
                t.t(key::MACHINE_REFUSED_BUSY)
            );
            assert_eq!(
                said(&Came::Refused("no-failed-trial".to_owned()), &t),
                t.t(key::MACHINE_REFUSED_NO_FAILED_TRIAL)
            );
            assert_eq!(
                said(
                    &Came::Refused("a-word-the-machine-never-says".to_owned()),
                    &t
                ),
                t.t(crate::security_words::refusal(
                    ronoctua_security::Why::Failed
                ))
            );
        }
    }

    /// FAILS IF: the question's cursor is not born on `no`, a first click
    /// on it answers, or a key that moved it is not the person's pick.
    #[test]
    fn the_question_is_born_on_no_and_a_click_picks_first() {
        let mut q = Question::of(Gesture::TryAgain);
        assert!(!q.yes());
        assert!(!q.clicked(false), "the born cursor answered one click");
        assert!(q.clicked(false));
        let mut q = Question::of(Gesture::Hold);
        assert!(!q.clicked(true), "the first click on yes answered");
        assert!(q.yes());
        assert!(q.clicked(true));
        let mut q = Question::of(Gesture::LetGo);
        q.moved(true);
        assert!(q.clicked(true), "a key's pick is the person's");
    }
}
