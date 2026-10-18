//! INCUS'S VIEW OPEN, in either door — the footer's region
//! ([`crate::incus_region`]) or the window ([`crate::incus_window`]): the
//! view, what it reads and its threads — and the ONE function both doors
//! carry the view's errands out with ([`carry_out`]).
//!
//! # Why one function, and not one per door
//!
//! The owner's rule is that the two doors do the same things, and what a
//! door says is said in it. Two handlers of the same errand drift apart as
//! two copies do; one over each door's [`Doorway`] leaves nothing to drift.
//! What differs between the doors is what a door IS — the footer steps aside
//! for a console, a window stays — and that is the doorway's to answer.

use crate::echo::Echo;
use crate::incus::{Errand, Kind, Line, Press, Readings, View};
use crate::incus_feed::Feed;
use crate::places::Layout;
use crate::region::Doorway;

/// How many times a door's settling takes what arrived and follows it again,
/// at most — a photograph's feed answers on the spot, and what it answered
/// can be what the view wants next (a catalogue once its hosts are let).
pub const SETTLE_ROUNDS: usize = 4;

/// The view open in a door.
pub struct Open {
    /// The view.
    pub view: View,
    /// What it draws from.
    pub readings: Readings,
    /// Its threads — the daemon's listing, events, state clock, catalogues
    /// and deeds, alive exactly as long as the view (and a deed's answer
    /// after it, `Feed::quiet`); a photograph's answers on the spot.
    pub feed: Feed,
}

impl Open {
    /// The view on `kind` over `feed`, its dates in `zone`, its sentences
    /// teaching the queue's keys this door's leader teaches.
    #[must_use]
    pub fn new(
        view: View,
        feed: Feed,
        zone: ronoctua_files::when::Zone,
        queue_keys: String,
    ) -> Self {
        Self {
            view,
            readings: Readings {
                zone,
                queue_keys,
                ..Readings::default()
            },
            feed,
        }
    }

    /// The view on `kind`, the feed started.
    #[must_use]
    pub fn on(
        kind: Kind,
        mut feed: Feed,
        zone: ronoctua_files::when::Zone,
        queue_keys: String,
    ) -> Self {
        feed.start();
        Self::new(View::on(kind), feed, zone, queue_keys)
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

    /// Everything the threads brought, taken — a deed's end said on the
    /// door's line. Whether anything arrived.
    pub fn take(&mut self, way: &mut dyn Doorway) -> bool {
        let arrived = self.feed.take();
        if arrived.is_empty() {
            return false;
        }
        for a in arrived {
            if let Some(said) = self.readings.took(a, way.texts()) {
                way.say(said, Echo::Normal);
            }
        }
        true
    }

    /// The cursors put back in step with what is drawn, and what is read
    /// made what is shown — WHAT ARRIVED WHILE FOLLOWING taken here, a few
    /// rounds at most: a photograph's feed answers on the spot, and what it
    /// answered can be what the view wants next. A live feed's threads ring
    /// the bell as well.
    pub fn settle(&mut self, layout: Layout, way: &mut dyn Doorway) {
        for _ in 0..SETTLE_ROUNDS {
            self.view
                .settle(&self.readings, way.columns(), layout, way.texts());
            let wants = self.view.wants(&self.readings, layout);
            self.feed.follow(&wants, &mut self.readings);
            let more = self.feed.take();
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
            .settle(&self.readings, way.columns(), layout, way.texts());
    }

    /// What one key means — from the keyboard, the hint, a second click or a
    /// command by name — carried out on the door it was pressed in.
    pub fn press(&mut self, press: Press, layout: Layout, way: &mut dyn Doorway) {
        let errand = self
            .view
            .key(press, &self.readings, way.columns(), layout, way.texts());
        carry_out(errand, self, layout, way);
    }

    /// The answer typed on the door's line, under the id that asked — to the
    /// view, which knows what it was for. A path is read under the person's
    /// home.
    pub fn answered(&mut self, id: &str, said: &str, layout: Layout, way: &mut dyn Doorway) {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let errand = self
            .view
            .answered(id, said, &self.readings, home.as_deref(), way.texts());
        carry_out(errand, self, layout, way);
    }

    /// The queue's list changed: Incus's jobs — and a listing again when one
    /// of them ended. Whether the view's jobs changed.
    pub fn jobs_arrived(&mut self, jobs: &[ronoctua_transfers::Job]) -> bool {
        let mine = crate::incus_feed::incus_jobs(jobs);
        if crate::incus_feed::fewer_running(&mine, &self.readings.jobs) {
            self.feed.relist();
        }
        if mine == self.readings.jobs {
            return false;
        }
        self.readings.jobs = mine;
        true
    }
}

/// CARRIES OUT what the view asked for, on the line of the door it was asked
/// in — the footer's region and the window alike (`errands-are-handled-alike`
/// reads that both hand it the errand whole) — and settles the view after.
pub fn carry_out(errand: Errand, open: &mut Open, layout: Layout, way: &mut dyn Doorway) {
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
        Errand::Do(deed, name) => {
            let what = way.texts().t(deed.key());
            let said = way.texts().t_with(
                crate::key::INCUS_ECHO_ASKED,
                &[("what", &what), ("name", &name)],
            );
            way.say(said, Echo::Normal);
            open.feed.deed(deed, name);
        }
        Errand::Queue(job, label) => {
            let said = crate::incus_words::queued_said(
                &job,
                &label,
                &open.readings.queue_keys,
                way.texts(),
            );
            let work = ronoctua_transfers::Work::Incus {
                job: Box::new(job),
                label,
            };
            match open.feed.queue(work) {
                Ok(()) => way.say(said, Echo::Normal),
                Err(crate::incus_feed::QueueDown) => {
                    let said = way.texts().t(crate::key::INCUS_NO_QUEUE);
                    way.say(said, Echo::Normal);
                }
            }
        }
        // ASKED ON THE DOOR'S OWN LINE — the footer's minibuffer, the
        // window's header: the same `line::Line`.
        Errand::Ask(id, initial) => {
            way.ask(crate::line::Line::new(id, false, initial.as_deref()));
        }
        // A CONSOLE, THE SCREEN OR A SHELL: the program `incus::launched`
        // says — our terminal for the first and the last — started exactly
        // as `Feed::opening` answered, the door stepping aside first (the
        // footer's region closes: a view left open over the terminal keeps
        // its keyboard).
        Errand::Open(what, name) => {
            let me = std::env::current_exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            match open.feed.opening(what, &name, &me) {
                Ok((program, words)) => {
                    way.step_aside();
                    way.launch(crate::door::Launching::Program {
                        program,
                        args: words,
                        cwd: None,
                    });
                    return;
                }
                Err(refusal) => {
                    let said = crate::incus_words::refused(refusal, way.texts());
                    way.say(said, Echo::Normal);
                }
            }
        }
        Errand::Allow(hosts) => {
            if let Err(said) = open.feed.allow(&hosts, way.texts()) {
                way.say(said, Echo::Normal);
            }
        }
        Errand::Say(text) => way.say(text, Echo::Normal),
    }
    open.settle(layout, way);
    way.repaint();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::incus_feed::Scene;
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

    fn scene() -> Scene {
        let r = crate::incus::tests::readings();
        let l = r.listed().cloned().expect("a listing");
        Scene {
            instances: l.instances,
            images: l.images,
            profiles: l.profiles,
            networks: l.networks,
            pools: l.pools,
            projects: l.projects,
            remotes: r.remotes_read().to_vec(),
            cli: Some("/nix/store/x-incus/bin/incus".to_owned()),
            ..Scene::default()
        }
    }

    /// FAILS IF: a console is started anywhere but in our terminal, or the
    /// door does not step aside for it first — the footer's region would
    /// keep the keyboard over the terminal.
    #[test]
    fn a_console_steps_aside_into_our_terminal() {
        let mut w = way();
        let mut open = Open::on(
            Kind::Instances,
            Feed::detached(Some(scene())),
            ronoctua_files::when::Zone::utc(),
            String::new(),
        );
        open.settle(Layout::Stacked, &mut w);
        let mut w = way();
        open.press(Press::Console, Layout::Stacked, &mut w);
        let at = w
            .owed
            .iter()
            .position(|e| *e == Effect::StepAside)
            .expect("the door steps aside");
        match w.owed.get(at + 1) {
            Some(Effect::Launch(crate::door::Launching::Program { args, .. })) => {
                assert_eq!(
                    args.first().map(String::as_str),
                    Some("terminal"),
                    "{args:?}"
                );
            }
            other => panic!("no console after stepping aside: {other:?}"),
        }
    }

    /// FAILS IF: Incus's jobs are not told apart from the queue's others,
    /// or the same list again reads as a change.
    #[test]
    fn the_queues_jobs_are_incuss() {
        let mut open = Open::new(
            View::fresh(),
            Feed::detached(None),
            ronoctua_files::when::Zone::utc(),
            String::new(),
        );
        let other = ronoctua_transfers::Work::Delete { paths: Vec::new() };
        let jobs = vec![ronoctua_transfers::Job::fresh_one(1, other.engine(), other)];
        assert!(!open.jobs_arrived(&jobs), "no job of Incus's");
        assert!(open.readings.jobs.is_empty());
    }
}
