//! THE CONTAINERS' VIEW OPEN, in either door — the footer's region
//! ([`crate::containers_region`]) or the window ([`crate::containers_window`]):
//! the view, what it reads, its threads, and a login waiting for its
//! password — and the ONE function both doors carry the view's errands out
//! with ([`carry_out`]).
//!
//! # Why one function, and not one per door
//!
//! The owner's rule is that the two doors do the same things, and what a
//! door says is said in it. Two handlers of the same errand — one in the
//! footer, one in the window — drifted apart as two copies do: a shell the
//! footer could open with no feed of its own that the window could not, a
//! class of message here and another there. One handler over each door's
//! [`Doorway`] leaves nothing to drift: what differs between the doors is
//! what a door IS (the footer steps aside for a terminal, a window stays),
//! and that is the doorway's to answer.

use ronoctua_consent::keys::Secret;
use ronoctua_transfers::{Job, Stage, Work};

use crate::containers::{Errand, LOGIN_PASSWORD, Line, Login, Press, Readings, View, secret};
use crate::containers_feed::Feed;
use crate::echo::Echo;
use crate::places::Layout;
use crate::region::Doorway;

/// How many times a door's settling takes what arrived and follows it again,
/// at most — every round asks at most one inspect.
pub const SETTLE_ROUNDS: usize = 4;

/// The view open in a door.
pub struct Open {
    /// The view.
    pub view: View,
    /// What it draws from.
    pub readings: Readings,
    /// Its threads — podman's listing, events, logs and stats, alive exactly
    /// as long as the view; `None` when the door has no loop to hear them on
    /// (a photograph's footer, which asks podman nothing).
    pub feed: Option<Feed>,
    /// A registry's login waiting for its password
    /// ([`crate::containers::LOGIN`]).
    pub login: Option<Login>,
}

impl Open {
    /// A fresh view over `feed`, its dates in `zone`, its sentences teaching
    /// the queue's keys this door's leader teaches.
    #[must_use]
    pub fn new(feed: Option<Feed>, zone: ronoctua_files::when::Zone, queue_keys: String) -> Self {
        Self {
            view: View::fresh(),
            readings: Readings {
                zone,
                queue_keys,
                ..Readings::default()
            },
            feed,
            login: None,
        }
    }

    /// The drawn lines, at the door's width and room.
    #[must_use]
    pub fn lines(
        &self,
        columns: u16,
        layout: Layout,
        room: usize,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Line> {
        self.view
            .lines(&self.readings, columns, layout, room, texts)
    }

    /// The cursors put back in step with what is drawn, and the streams made
    /// what is shown — UNTIL NOTHING MORE ARRIVES: what arrived here was
    /// taken off the threads' list, so the streams are made what it shows
    /// before leaving (a listing taken now is a row whose inspect is asked
    /// now). A detached feed answers on the spot; a live one rarely has
    /// anything yet. What arrived is said on the door's line.
    pub fn settle(&mut self, layout: Layout, room: usize, way: &mut dyn Doorway) {
        for _ in 0..SETTLE_ROUNDS {
            self.view
                .settle(&self.readings, way.columns(), layout, room, way.texts());
            let wants = self.view.wants(&self.readings, layout);
            let Some(feed) = self.feed.as_mut() else {
                break;
            };
            feed.follow(&wants, &mut self.readings);
            let more = feed.take();
            if more.is_empty() {
                break;
            }
            for a in more {
                if let Some(said) = self.readings.took(a, way.texts()) {
                    way.say(said, Echo::Normal);
                }
            }
        }
        self.view
            .settle(&self.readings, way.columns(), layout, room, way.texts());
    }

    /// What one key means — from the keyboard, the hint, a second click or a
    /// command by name — carried out on the door it was pressed in.
    pub fn press(&mut self, press: Press, layout: Layout, room: usize, way: &mut dyn Doorway) {
        let errand = self
            .view
            .key(press, &self.readings, way.columns(), layout, way.texts());
        carry_out(errand, Some(self), layout, room, way);
    }

    /// The login's password arrived: to podman's stdin, off the loop, with
    /// the login it was asked for — or, with none waiting or no feed to hand
    /// it to, a sentence.
    pub fn password(&mut self, text: Option<&str>, way: &mut dyn Doorway) {
        match (self.login.take(), text) {
            (Some(login), Some(text)) if !text.is_empty() => match self.feed.as_ref() {
                Some(feed) => {
                    let said = way.texts().t_with(
                        crate::key::CONTAINERS_ECHO_LOGGING_IN,
                        &[("registry", &login.registry), ("user", &login.user)],
                    );
                    way.say(said, Echo::Normal);
                    feed.login(login, Secret::new(text));
                }
                None => refused_no_login(way),
            },
            (Some(_), _) => {}
            (None, _) => refused_no_login(way),
        }
    }

    /// The queue's list changed: the containers' jobs — and a listing again
    /// when one of them ended. Whether the view's jobs changed.
    pub fn jobs_arrived(&mut self, jobs: &[Job]) -> bool {
        let mine = jobs_of(jobs);
        if fewer_running(&mine, &self.readings.jobs)
            && let Some(feed) = self.feed.as_ref()
        {
            feed.relist();
        }
        if mine == self.readings.jobs {
            return false;
        }
        self.readings.jobs = mine;
        true
    }
}

/// No login waits for a password.
fn refused_no_login(way: &mut dyn Doorway) {
    let said = way.texts().t(crate::key::CONTAINERS_REFUSED_NO_LOGIN);
    way.say(said, Echo::Normal);
}

/// The queue's jobs that are the containers' — a pull, a build, compose.
#[must_use]
pub fn jobs_of(jobs: &[Job]) -> Vec<Job> {
    jobs.iter()
        .filter(|j| matches!(j.work, Work::Container { .. }))
        .cloned()
        .collect()
}

/// Whether fewer of the containers' jobs run now than before: one ended,
/// and what there is changed.
#[must_use]
pub fn fewer_running(now: &[Job], before: &[Job]) -> bool {
    let running = |js: &[Job]| {
        js.iter()
            .filter(|j| matches!(j.stage, Stage::Running | Stage::Weighing))
            .count()
    };
    running(now) < running(before)
}

/// What a key pressed BY NAME does to the CLOSED view, which has nothing to
/// act on: a typed question is asked all the same (an image to pull, a
/// folder to build), and nothing else is done.
#[must_use]
pub const fn closed_press(press: Press) -> Errand {
    match press {
        Press::Pull => Errand::Ask(crate::containers::PULL, None),
        Press::Build => Errand::Ask(crate::containers::BUILD, None),
        _ => Errand::Nothing,
    }
}

/// CARRIES OUT what the view asked for, on the line of the door it was asked
/// in — the footer's region and the window alike (`errands-are-handled-alike`
/// reads that both hand it the errand whole). `open` is `None` for the
/// CLOSED view's footer, whose typed questions are asked and answered all
/// the same (a pull, a build): what needs the view's threads says so.
pub fn carry_out(
    errand: Errand,
    mut open: Option<&mut Open>,
    layout: Layout,
    room: usize,
    way: &mut dyn Doorway,
) {
    match errand {
        Errand::Nothing => {}
        Errand::Close => {
            way.close();
            return;
        }
        Errand::Copy(text) => {
            let said = crate::chrome::copied(&text, way.texts());
            way.say(said, Echo::Normal);
        }
        Errand::Act(action, name) => {
            let what = way.texts().t(crate::containers_feed::action_key(&action));
            let said = way.texts().t_with(
                crate::key::CONTAINERS_ECHO_ASKED,
                &[("what", &what), ("name", &name)],
            );
            way.say(said, Echo::Normal);
            if let Some(feed) = open.as_deref().and_then(|o| o.feed.as_ref()) {
                feed.act(action, name);
            }
        }
        Errand::Queue(job, label) => {
            let keys = open
                .as_deref()
                .map_or_else(crate::queue::queue_keys, |o| o.readings.queue_keys.clone());
            let said = crate::containers_words::queued_said(&job, &label, &keys, way.texts());
            let work = Work::Container { job, label };
            match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
                Ok(()) => way.say(said, Echo::Normal),
                Err(e) => {
                    tracing::warn!(error = %e, "the transfer queue did not take the job");
                    let said = way.texts().t(crate::key::CONTAINERS_NO_QUEUE);
                    way.say(said, Echo::Normal);
                }
            }
        }
        // ASKED ON THE DOOR'S OWN LINE — the footer's minibuffer, the
        // window's header: the same `line::Line`.
        Errand::Ask(id, initial) => {
            way.ask(crate::line::Line::new(id, secret(id), initial.as_deref()));
        }
        // THE PASSWORD ON THE SECRET LINE, the login kept until it comes.
        Errand::Password(login) => {
            if let Some(o) = open.as_deref_mut() {
                o.login = Some(login);
            }
            way.ask(crate::line::Line::new(LOGIN_PASSWORD, true, None));
        }
        Errand::Shell(id) => {
            let line = open
                .as_deref()
                .and_then(|o| o.feed.as_ref())
                .and_then(|f| f.exec_line(&id))
                .or_else(|| ronoctua_containers::Podman::from_env().map(|p| p.exec_line(&id)));
            match line {
                // A SHELL IN THE TERMINAL — the door steps aside first (the
                // footer's region closes: a view left open over the terminal
                // keeps its keyboard).
                Some((podman, args)) => {
                    let mut words = vec!["terminal".to_owned(), podman];
                    words.extend(args);
                    way.step_aside();
                    way.open_window(words);
                    return;
                }
                None => {
                    let said = crate::containers_words::refused(
                        ronoctua_containers::Refusal {
                            why: ronoctua_containers::Why::NoPodman,
                        },
                        way.texts(),
                    );
                    way.say(said, Echo::Normal);
                }
            }
        }
        // ANOTHER VIEW — the firewall's rules: this one gives the region back
        // first, or the other would find it taken and say so.
        Errand::Run(id) => {
            way.step_aside();
            way.run(id, None);
            return;
        }
        Errand::Say(text) => way.say(text, Echo::Normal),
    }
    if let Some(o) = open {
        o.settle(layout, room, way);
    }
    way.repaint();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder};

    fn way() -> Recorder {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB exists");
        Recorder::new(
            std::rc::Rc::new(texts),
            100,
            true,
            crate::sudo::Gate::shut(),
            crate::wake::Wake::new(|| {}),
        )
    }

    fn job(stage: Stage, work: Work) -> Job {
        Job {
            stage,
            ..Job::fresh_one(1, work.engine(), work)
        }
    }

    /// FAILS IF: the containers' jobs are mixed with another view's, or a job
    /// that ended is not seen as fewer running — the listing would not be
    /// asked again after a pull.
    #[test]
    fn the_containers_jobs_are_theirs() {
        let pull = || Work::Container {
            job: ronoctua_containers::Job::Pull {
                image: "alpine".to_owned(),
            },
            label: "alpine".to_owned(),
        };
        let other = job(Stage::Running, Work::Delete { paths: Vec::new() });
        let running = job(Stage::Running, pull());
        let done = job(Stage::Done, pull());
        let (running, done) = (vec![running], vec![done]);
        assert_eq!(jobs_of(&[other, running[0].clone()]), running);
        assert!(fewer_running(&done, &running));
        assert!(!fewer_running(&running, &done));
        let mut open = Open::new(None, ronoctua_files::when::Zone::utc(), String::new());
        assert!(open.jobs_arrived(&running));
        assert!(!open.jobs_arrived(&running), "the same list again");
    }

    /// FAILS IF: a closed view's typed question is not asked on the door's
    /// line, a shell or another view does not make the door step aside
    /// first, or a password reaches nobody in silence.
    #[test]
    fn what_the_view_asks_is_done_on_the_door() {
        let mut w = way();
        carry_out(closed_press(Press::Pull), None, Layout::Stacked, 0, &mut w);
        assert!(
            matches!(w.owed.first(), Some(Effect::Ask(line)) if line.id == crate::containers::PULL),
            "{:?}",
            w.owed
        );
        let mut w = way();
        carry_out(
            Errand::Run(crate::containers::FIREWALL),
            None,
            Layout::Stacked,
            0,
            &mut w,
        );
        assert_eq!(
            w.owed,
            vec![
                Effect::StepAside,
                Effect::Run(crate::containers::FIREWALL, None)
            ]
        );
        let mut open = Open::new(None, ronoctua_files::when::Zone::utc(), String::new());
        let mut w = way();
        open.password(Some("hunter2"), &mut w);
        assert!(
            matches!(w.owed.as_slice(), [Effect::Say(..)]),
            "no login waits, and that is said: {:?}",
            w.owed
        );
        assert_eq!(closed_press(Press::Down), Errand::Nothing);
    }
}
