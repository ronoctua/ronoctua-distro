//! THE SECURITY CENTRE'S VAULT FLOW, AS A DOOR HOLDS IT (`vault::Flow`) —
//! what the file manager's window runs for `m` and `l` on its sidebar's
//! `/private` row, so the passphrase is asked on ITS line and what came back
//! is said on it: a door never sends the person to another one.
//!
//! # Why the binary
//!
//! The flow is the security centre's (`security::View`, its private section)
//! and the window is the file manager's, and neither stands on the
//! other: the binary, which names every feature, hands the manager's window
//! the centre's flow as a `vault::Flow` when it opens one, and that is the
//! whole of their meeting.

use std::sync::mpsc;

use ronoctua_shell_door::region::Doorway;
use ronoctua_shell_kit::echo::Echo;
use ronoctua_shell_security::security::{Came, Errand, Job, Scene, View};
use ronoctua_shell_work::vault::{self, Flow, Walk};

/// The centre's flow for a door that is not the centre's.
#[derive(Default)]
struct InADoor {
    /// The questions and the work in flight.
    view: View,
    /// The vault's part of the scene, read at each gesture.
    scene: Scene,
    /// How the work's answers come back — hung at the first work, and only
    /// then, on the door's wake.
    came: Option<mpsc::Receiver<Came>>,
    /// Its other end, for the next work.
    send: Option<mpsc::Sender<Came>>,
    /// The status a camera planted (`--private`): read in place of the file,
    /// and no work is ever started from it.
    planted: Option<Option<ronoctua_private::Status>>,
}

/// The flow a window of the file manager holds.
#[must_use]
pub fn flow() -> Box<dyn Flow> {
    Box::new(InADoor::default())
}

impl InADoor {
    /// What the flow asked for, done in the door that holds it.
    fn errand(&mut self, errand: Errand, way: &mut dyn Doorway) -> Walk {
        match errand {
            Errand::Ask {
                id,
                secret,
                initial,
            } => way.ask(ronoctua_shell_kit::line::Line::new(
                id,
                secret,
                initial.as_deref(),
            )),
            Errand::Door(work) => {
                // THE CAMERA starts no work: it draws.
                if self.planted.is_some() {
                    return Walk::Stay;
                }
                let send = self.channel(way);
                self.view.sent(&work);
                way.hold(way.texts().t(crate::key::SECURITY_PRIVATE_BUSY));
                // ROOT, ASKED IN THE FOOTER'S SUDO BAR — as this door's.
                let gesture = way.sudo().gesture(ronoctua_shell_door::sudo::Asker::Files);
                let wake = way.wake();
                ronoctua_shell_security::security::start(Job::Door(work, gesture), move |came| {
                    let _ = send.send(came);
                    wake.call();
                });
            }
            Errand::Say(said) => way.say(said, Echo::Long),
            // THE OPEN VAULT, walked into: this door is the file manager.
            Errand::Command("private-show") => {
                way.repaint();
                return Walk::Into(std::path::PathBuf::from(ronoctua_private::MOUNT_POINT));
            }
            // NO VAULT any more (its row went in that instant): said here.
            Errand::Show(_) => way.say(
                way.texts().t(vault::refusal(ronoctua_private::Why::Absent)),
                Echo::Long,
            ),
            _ => {}
        }
        way.repaint();
        Walk::Stay
    }

    /// The channel the work answers on, hung the first time it is wanted.
    fn channel(&mut self, _way: &mut dyn Doorway) -> mpsc::Sender<Came> {
        if let Some(send) = &self.send {
            return send.clone();
        }
        let (send, came) = mpsc::channel::<Came>();
        self.came = Some(came);
        self.send = Some(send.clone());
        send
    }
}

impl Flow for InADoor {
    /// `m` or `l` on the vault's row: the security centre's two leader keys
    /// (`private_open`, `private_lock`), over the vault as it is now — or as
    /// the camera planted it.
    fn gesture(
        &mut self,
        open: bool,
        planted: Option<Option<ronoctua_private::Status>>,
        way: &mut dyn Doorway,
    ) -> Walk {
        self.planted = planted;
        self.scene = match planted {
            Some(status) => ronoctua_shell_security::security::vault_scene_of(status),
            None => ronoctua_shell_security::security::vault_scene(),
        };
        let errand = if open {
            self.view.private_open(&self.scene, way.texts())
        } else {
            self.view.private_lock(&self.scene, way.texts())
        };
        self.errand(errand, way)
    }

    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) -> Walk {
        let errand = self.view.answered(id, text, &self.scene, way.texts());
        self.errand(errand, way)
    }

    fn unanswered(&mut self, id: &str) {
        self.view.unanswered(id);
    }

    /// What the vault's work came back with: the sentence said here (the
    /// touch it waits for, held while it waits), and the next question — the
    /// vault's passphrase — asked here; root is asked in the sudo bar.
    fn woken(&mut self, way: &mut dyn Doorway) -> Walk {
        let mut walk = Walk::Stay;
        while let Some(came) = self.came.as_ref().and_then(|c| c.try_recv().ok()) {
            let waits = matches!(came, Came::Step(_));
            let out = self.view.came(came, &mut self.scene, way.texts());
            if let Some(said) = out.say {
                if waits {
                    way.hold(said);
                } else {
                    way.say(said, Echo::Long);
                }
            }
            if let Walk::Into(path) = self.errand(out.errand, way) {
                walk = Walk::Into(path);
            }
        }
        walk
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_shell_door::region::{Effect, Recorder};

    fn way() -> Recorder {
        Recorder::new(
            std::rc::Rc::new(
                ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language"),
            ),
            100,
            true,
            ronoctua_shell_door::sudo::Gate::shut(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
    }

    fn planted(state: ronoctua_private::State) -> Option<Option<ronoctua_private::Status>> {
        Some(Some(ronoctua_private::Status {
            state,
            ..ronoctua_private::Status::absent()
        }))
    }

    /// FAILS IF: `m` on an open vault does not walk the door into it, or a
    /// gone vault is not said on the door's line — the centre's flow, run in
    /// a door that is not the centre's, sending the person nowhere else.
    #[test]
    fn the_flow_walks_into_an_open_vault_and_says_a_gone_one() {
        let mut flow = flow();
        let mut w = way();
        assert_eq!(
            flow.gesture(true, planted(ronoctua_private::State::Open), &mut w),
            Walk::Into(std::path::PathBuf::from(ronoctua_private::MOUNT_POINT))
        );
        let mut w = way();
        assert_eq!(
            flow.gesture(true, planted(ronoctua_private::State::Absent), &mut w),
            Walk::Stay
        );
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Say(_, Echo::Long))),
            "a gone vault said nothing: {:?}",
            w.owed
        );
    }

    /// FAILS IF: `m` on a locked vault does not ask its passphrase on the
    /// door's own line, or a camera's planted vault starts work.
    #[test]
    fn a_locked_vault_is_asked_for_on_the_doors_line() {
        let mut flow = flow();
        let mut w = way();
        let _ = flow.gesture(true, planted(ronoctua_private::State::Locked), &mut w);
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Ask(l) if vault::QUESTIONS.contains(&l.id))),
            "the passphrase was not asked here: {:?}",
            w.owed
        );
        let mut w = way();
        let _ = flow.answered(vault::PASSPHRASE, "a passphrase", &mut w);
        assert!(
            !w.owed.iter().any(|e| matches!(e, Effect::Hold(_))),
            "the camera's vault started work: {:?}",
            w.owed
        );
    }
}
