//! THE UPDATE IN EITHER DOOR: what the view keeps while a door shows it
//! ([`Open`]), the one handler of what it asks a door to do ([`carry_out`]),
//! and the footer's region ([`UpdateRegion`], registered as [`ENTRY`]).
//!
//! # One handler, two doors
//!
//! The footer's region (`LEADER s u u`, one list) and the window
//! (`LEADER s u w`, two halves) hold the same [`Open`] and hand every errand
//! of the view to [`carry_out`], over the door they are
//! (`crate::region::Doorway`): a run started, answered or said the same way,
//! the binary cache's page changed the same way, root's netrc asked through
//! that door's own sudo gate and its verdict said on that door's line. What
//! moves a PAGE — the step cursor, the output a page up or down — is each
//! door's, because their pages are not the same shape: the window pages its
//! two halves, the footer its one list.
//!
//! # A run followed with the page closed
//!
//! `LEADER s u n` starts a run with no page open, and the run the brain took
//! is followed in its mind: held for a metered way out, the page opens on the
//! question. So that command brings the footer's region to life HIDDEN
//! (`Closed::Behind`), and the region lingers while it follows a run
//! ([`Open::lingers`]) — and so does a region whose page closed meanwhile.

use crate::binary_cache;
use crate::region::{
    Closed, Doorway, Entry, Inbox, Lit, Paging, Post, Region, Row, Seen, Taken, Told,
};
use crate::update_window::{Errand, Followed, Heard, Order, Output, Press, RunHere, Started, View};
use crate::update_window_list::{Item, ListRow};
use ronoctua_brain::compiled::system_update as module;
use ronoctua_shell_kit::mind_wire::Answer;

/// The region's id, as the footer's precedence ranks it.
pub const ID: &str = "update";

/// The words that open the update's window, after the binary's name.
const WINDOW_WORDS: &str = "update";

/// The footer's command that opens the brain's page — on a module's, given
/// its id: where the update's schedule is changed.
const BRAIN_OPEN: &str = "brain-open";

/// What the door says when the brain took a step's switch: the step, by its
/// name, and what the next update does with it.
fn switched(id: &str, on: bool, texts: &ronoctua_i18n::Texts) -> String {
    let name =
        crate::update_window::step_of(id).map_or_else(|| id.to_owned(), |s| texts.t(s.name_key));
    texts.t_with(
        module::key::SYSTEM_UPDATE_SWITCHED,
        &[("step", &name), ("on", if on { "on" } else { "off" })],
    )
}

/// What the update's threads hand back to a door's loop.
#[derive(Debug)]
pub enum Back {
    /// A line of either journal.
    Line(crate::output::OutputLine),
    /// What came of root's netrc, asked through the door's sudo gate.
    Netrc(binary_cache::Came),
    /// What came of a gesture of the parts' page (`crate::parts::carry`).
    Parts(crate::parts::Came),
    /// What came of a gesture of the machine's, asked through the door's
    /// sudo gate (`crate::machine_gesture`).
    Machine(crate::machine_gesture::Came),
    /// What came of an approval of a brain module of the person's, asked
    /// through the door's sudo gate (`crate::approval`).
    Approve(crate::approval::Came),
    /// What GitHub said of the branches the machine's flake follows
    /// (`ronoctua_software::heads`), or the lock's refusal — with the number
    /// of the question it answers (`Open::heads_asked`).
    Heads(
        u64,
        Result<Vec<ronoctua_software::heads::Heard>, ronoctua_software::heads::Unread>,
    ),
    /// What the brain said to what this door asked of it.
    Brain(Asked, Answer),
    /// A run this door started straight through systemd ended — or systemd
    /// refused it (`update_window::start_unit`).
    Ended,
}

/// What a door asked of the brain, beside its answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    /// A step switched to `on`.
    Switch {
        /// The step.
        id: &'static str,
        /// Whether it is to run.
        on: bool,
    },
    /// A rehearsal.
    Rehearse,
}

/// Where a key or a click of a door reaches past it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    /// The machine's: the brain's socket, and systemd when the brain is not
    /// listening (`update_window::begin`); the brain again for the countdown's
    /// answers.
    Machine,
    /// Nobody's: what it would have asked is KEPT, in order, and the
    /// photograph prints it. A photograph whose `u` started an update on the
    /// machine taking it would be a camera that rebuilds the system.
    Kept(Vec<String>),
}

/// THE UPDATE WHILE A DOOR SHOWS IT — or, in the footer, while it still
/// follows a run with its page closed.
pub struct Open {
    /// The steps, their verdicts, the cursor and the output.
    pub view: View,
    /// The brain's mind, as it last said — `None` while it is not answering.
    mind: Option<ronoctua_brain::Mind>,
    /// A run this door asked of the brain, followed until the brain says
    /// what became of it ([`Followed`]).
    followed: Option<Followed>,
    /// Where a key that starts or answers something goes.
    reach: Reach,
    /// A photograph's brain holds the run a `u` asks for, for a metered way
    /// out — the scene's; nothing on a screen.
    pub held_metered: bool,
    /// A photograph's machine has no brain answering — the scene's: what a
    /// key would have done is what a door with no brain does. Nothing on a
    /// screen, where whether the brain answers is its mind.
    pub no_brain: bool,
    /// The two `journalctl -f`, while the page is shown, so they can be
    /// killed — `-f` never ends by itself.
    children: Vec<std::process::Child>,
    /// What the journals and the netrc's thread hand back; `None`: a
    /// photograph's door, which follows nothing.
    inbox: Option<Inbox<Back>>,
    /// Root's netrc was asked for, and its verdict has not come.
    netrc_waiting: bool,
    /// Gestures of the parts' page carried out, whose verdict has not come.
    parts_waiting: usize,
    /// Gestures of the machine's asked, whose verdict has not come.
    machine_waiting: usize,
    /// Approvals asked, whose verdict has not come.
    approve_waiting: usize,
    /// Asks of the brain's, whose answer has not come.
    brain_waiting: usize,
    /// How many times GitHub was asked for the heads: each answer comes back
    /// with the number of its question, and one older than the last asked is
    /// dropped — a slow answer never says over a newer one.
    heads_asked: u64,
}

impl std::fmt::Debug for Open {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Open")
            .field("view", &self.view)
            .field("reach", &self.reach)
            .finish_non_exhaustive()
    }
}

impl Drop for Open {
    /// WHAT I STARTED, I STOP. `journalctl -f` never ends on its own, and a
    /// door that leaves two of them behind every time it is closed is a door
    /// that leaks processes.
    fn drop(&mut self) {
        crate::update_window::stop_following(&mut self.children);
    }
}

impl Open {
    /// The update over `view`, its keys reaching `reach`; with a `wake`, its
    /// threads (the journals, the netrc's) hand back through it.
    #[must_use]
    pub fn new(mut view: View, reach: Reach, wake: Option<crate::wake::Wake>) -> Self {
        // THE FILE PICKER the parts' page brings up is watched through the
        // same wake: its folder, and the devices that are its places.
        if let Some(w) = &wake {
            view.watch_with(w.clone());
        }
        Self {
            view,
            mind: None,
            followed: None,
            reach,
            held_metered: false,
            no_brain: false,
            children: Vec::new(),
            inbox: wake.map(Inbox::new),
            netrc_waiting: false,
            parts_waiting: 0,
            machine_waiting: 0,
            approve_waiting: 0,
            brain_waiting: 0,
            heads_asked: 0,
        }
    }

    /// Where a thread posts what it hands back.
    fn post(&self) -> Option<Post<Back>> {
        self.inbox.as_ref().map(Inbox::post)
    }

    /// The two journals, followed on threads of their own — once.
    pub fn follow_the_journals(&mut self) {
        if !self.children.is_empty() {
            return;
        }
        if let Some(post) = self.post() {
            self.children = crate::update_window::follow_both(&post);
        }
    }

    /// Whether the journals are followed — for a test of what a door
    /// starts.
    #[cfg(test)]
    pub(crate) const fn follows_the_journals(&self) -> bool {
        !self.children.is_empty()
    }

    /// The journals stopped.
    pub fn stop_the_journals(&mut self) {
        crate::update_window::stop_following(&mut self.children);
    }

    /// Whether it still follows something with its page closed: a run it
    /// asked of the brain, root's netrc, a gesture of the parts' page, one
    /// of the machine's, or an approval.
    #[must_use]
    pub const fn lingers(&self) -> bool {
        self.followed.is_some()
            || self.netrc_waiting
            || self.parts_waiting > 0
            || self.machine_waiting > 0
            || self.approve_waiting > 0
            || self.brain_waiting > 0
    }

    /// WHETHER THE OUTPUT IS SHOWN LIVE, under the list: while this update
    /// runs — the brain's run or its rehearsal, or one this door started
    /// straight through systemd, until it ends (`View::started_here`).
    /// Hidden otherwise: nothing running, nothing to watch.
    #[must_use]
    pub const fn output_live(&self) -> bool {
        matches!(self.view.run(), crate::update_window::Run::Running)
    }

    /// What the paging keys and arrows asked are what a photograph kept.
    #[must_use]
    pub fn would_have(&self) -> Vec<String> {
        match &self.reach {
            Reach::Machine => Vec::new(),
            Reach::Kept(asked) => asked.clone(),
        }
    }

    /// What its threads handed back, taken: the journals' lines into the
    /// output (whether any came), root's netrc said in this door and the
    /// page read again.
    pub fn woken(&mut self, way: &mut dyn Doorway) -> bool {
        // THE FILE PICKER'S FOLDER OR DEVICES moved: drawn again.
        if self.view.look() {
            way.repaint();
        }
        let came = self.inbox.as_ref().map(Inbox::take).unwrap_or_default();
        let mut lines = false;
        for back in came {
            match back {
                Back::Line(line) => {
                    // NOTHING IS PAINTED HERE: every line lands in the
                    // scrollback, and the frame is owed once.
                    self.view.heard(line);
                    lines = true;
                }
                Back::Netrc(came) => {
                    self.netrc_waiting = false;
                    let said = binary_cache::came_said(came, way.texts());
                    way.say(said, crate::echo::Echo::Long);
                    self.view.cache_reread();
                    way.repaint();
                }
                Back::Parts(came) => {
                    self.parts_waiting = self.parts_waiting.saturating_sub(1);
                    let said = crate::parts::said_of(&came, way.texts());
                    way.say(said, crate::echo::Echo::Long);
                    self.view.parts_reread();
                    way.repaint();
                }
                // WHAT CAME OF A GESTURE OF THE MACHINE'S, said in this door:
                // done, the door's refusal, or the machine's own sentence.
                // The records are read again on the beat, their files moved.
                Back::Machine(came) => {
                    self.machine_waiting = self.machine_waiting.saturating_sub(1);
                    self.view.gesture_heard();
                    let said = crate::machine_gesture::said(&came, way.texts());
                    way.say(said, crate::echo::Echo::Long);
                    way.repaint();
                }
                // WHAT CAME OF AN APPROVAL, said in this door: approved, the
                // door's refusal, or root's own reason. The records are read
                // again now: an approval recorded moved root's file.
                Back::Approve(came) => {
                    self.approve_waiting = self.approve_waiting.saturating_sub(1);
                    self.view.approval_heard();
                    let said = crate::approval::said(&came, way.texts());
                    way.say(said, crate::echo::Echo::Long);
                    way.repaint();
                }
                // WHAT GITHUB SAID: the status line says it. Data, so it moves
                // no page — and only the answer to the last question asked.
                Back::Heads(asked, came) => {
                    if asked == self.heads_asked {
                        self.view.heads_came(came);
                        way.repaint();
                    }
                }
                // WHAT THE BRAIN SAID to a switch or a rehearsal: taken, the
                // step's new state said with its name — the row itself
                // changes with the brain's next mind, published before it
                // answered —; refused or unheard, why.
                Back::Brain(asked, answer) => {
                    self.brain_waiting = self.brain_waiting.saturating_sub(1);
                    let said = match (&answer, asked) {
                        (Answer::Taken(_), Asked::Switch { id, on }) => {
                            Some(switched(id, on, way.texts()))
                        }
                        (other, _) => other.sentence(way.texts()),
                    };
                    if let Some(said) = said {
                        way.say(said, crate::echo::Echo::Long);
                    }
                    way.repaint();
                }
                // A RUN THIS DOOR STARTED ENDED: the list says what the brain
                // says again, and the live output goes.
                Back::Ended => {
                    self.view.started_here = None;
                    way.repaint();
                }
            }
        }
        lines
    }

    /// Asks the brain for `ask` on a named thread, its answer handed back to
    /// this door as `asked`'s — or, with no inbox (a test's door), dropped.
    /// Whether the thread started.
    fn ask_the_brain(
        &mut self,
        ask: ronoctua_brain::Ask,
        asked: Asked,
        taken: Option<&'static str>,
    ) -> bool {
        let post = self.post();
        let born = crate::brainlink::put_then(ask, taken, move |answer| {
            if let Some(post) = post {
                let _ = post.send(Back::Brain(asked, answer));
            }
        });
        if born {
            self.brain_waiting += 1;
        }
        born
    }

    /// Whether an update is there, asked of GitHub on a NAMED thread — the
    /// heads of the branches the machine's lock follows, as the machine's
    /// own updater, no consent asked (`ronoctua_software::heads`) — whose
    /// answer comes back to this door, numbered: a question asked after it
    /// makes it stale. `again`: the person asked, or the lock moved, and a
    /// head kept longer than a minute is asked again. A photograph's door
    /// asks nothing; nothing asks on a clock.
    pub fn check_the_heads(&mut self, again: bool) {
        let Some(post) = self.post().filter(|_| self.reach == Reach::Machine) else {
            return;
        };
        let Ok(roots) = ronoctua_software::Roots::of_session() else {
            tracing::warn!("no home: whether an update is there is not asked");
            return;
        };
        let cache = ronoctua_software::appimage::github_cache(&roots);
        let lock = crate::update_window::record_places()
            .machine_dir
            .join(ronoctua_software::heads::LOCK);
        let asked = self.heads_asked + 1;
        let born = std::thread::Builder::new()
            .name("update-heads".to_owned())
            .spawn(move || {
                let heard = if again {
                    ronoctua_software::heads::look_again(&lock, &cache)
                } else {
                    ronoctua_software::heads::look(&lock, &cache)
                };
                let _ = post.send(Back::Heads(asked, heard));
            });
        match born {
            Ok(_) => {
                self.heads_asked = asked;
                self.view.heads_asked();
            }
            Err(e) => tracing::error!(error = %e, "could not start the heads' thread"),
        }
    }

    /// The reports, the records and the pages' values read again when a
    /// file moved — and GitHub asked again when the machine's lock moved
    /// under what it said, past the copy kept a quarter of an hour (the
    /// person's own re-check): a head kept from before the update is judged
    /// against the lock the update moved. Whether something did.
    pub fn reread_if_moved(&mut self) -> bool {
        let moved = self.view.reread_if_moved();
        if self.view.heads_are_stale() {
            self.check_the_heads(true);
        }
        moved
    }

    /// What the files read again brought that the door says once: a
    /// rehearsal that ended, and what it measured.
    pub fn say_the_news(&mut self, way: &mut dyn Doorway) {
        if let Some(said) = self.view.rehearsal_news(way.texts()) {
            way.say(said, crate::echo::Echo::Long);
        }
    }

    /// The brain spoke: its mind is kept — the countdown's answers read it —
    /// what it says of this task drawn (the status, the actions, the keys),
    /// and a run this door asked for is followed in it. Whether that asked
    /// the question on a metered way out — what the brain says of the task
    /// is data, and moves no page.
    pub fn heard_the_mind(&mut self, mind: ronoctua_brain::Mind, way: &mut dyn Doorway) -> bool {
        if self.view.heard_the_brain(&mind) {
            way.repaint();
        }
        self.mind = Some(mind);
        self.heard_the_brain(way)
    }

    /// What the brain's mind says of the run this door asked for: held for a
    /// metered way out, the question — the no first — and the page asked for.
    fn heard_the_brain(&mut self, way: &mut dyn Doorway) -> bool {
        let (Some(followed), Some(mind)) = (self.followed.as_mut(), self.mind.as_ref()) else {
            return false;
        };
        match followed.heard(mind) {
            Heard::Waiting => false,
            Heard::Over => {
                self.followed = None;
                false
            }
            Heard::Metered => {
                self.followed = None;
                self.view.ask_metered();
                way.show();
                way.repaint();
                true
            }
        }
    }

    /// `LEADER s u n` — and `u` in the view, in either door: the whole update,
    /// asked for by hand. BELOW THE BATTERY FLOOR it asks first — the no
    /// first — on the page, which is asked for; otherwise it starts.
    /// Whether it asked.
    pub fn start_by_hand(&mut self, way: &mut dyn Doorway) -> bool {
        if self.view.ask_on_battery() {
            way.show();
            way.repaint();
            return true;
        }
        carry_out(Errand::Start, self, way);
        false
    }

    /// Root's netrc made to agree with the person's choice, on a thread — a
    /// root gesture of the update's, asked in the sudo bar through this door's
    /// gate (`sudo -n` first: an open window or sudo's timestamp asks
    /// nothing) — whose verdict comes back to this door. A photograph's door
    /// asks nothing.
    pub fn ask_for_the_netrc(&mut self, places: binary_cache::Places, way: &mut dyn Doorway) {
        let Some(post) = self.post() else {
            return;
        };
        if self.reach != Reach::Machine {
            return;
        }
        self.netrc_waiting = true;
        let gesture = way.sudo().gesture(crate::sudo::Asker::Update);
        binary_cache::door(places, gesture, move |came| {
            let _ = post.send(Back::Netrc(came));
        });
    }

    /// A gesture of the machine's, said yes to on the page, asked in the sudo
    /// bar through this door's gate (`sudo -n` first: an open window or
    /// sudo's timestamp asks nothing) on a NAMED thread — the helper waits
    /// for the machine's unit — whose verdict comes back to this door. A
    /// photograph's door asks nothing.
    pub fn ask_the_machine(
        &mut self,
        what: crate::machine_gesture::Gesture,
        way: &mut dyn Doorway,
    ) {
        let Some(post) = self.post().filter(|_| self.reach == Reach::Machine) else {
            self.view.gesture_heard();
            return;
        };
        let gesture = way.sudo().gesture(crate::sudo::Asker::Update);
        let born = std::thread::Builder::new()
            .name("machine-gesture".to_owned())
            .spawn(move || {
                let came = crate::machine_gesture::run(what, gesture);
                let _ = post.send(Back::Machine(came));
            });
        match born {
            Ok(_) => {
                self.machine_waiting += 1;
                let said = way.texts().t(crate::key::MACHINE_SAID_ASKED);
                way.say(said, crate::echo::Echo::Long);
            }
            Err(e) => {
                tracing::error!(error = %e, "could not start the machine gesture's thread");
                self.view.gesture_heard();
                let said = way.texts().t(crate::security_words::refusal(
                    ronoctua_security::Why::Unstartable,
                ));
                way.say(said, crate::echo::Echo::Long);
            }
        }
    }

    /// A brain module of the person's said yes to on the page: its request
    /// staged and asked in the sudo bar through this door's gate (`sudo -n`
    /// first: an open window or sudo's timestamp asks nothing) on a NAMED
    /// thread — root's unit asks Nix for the hash — whose verdict comes back
    /// to this door. A photograph's door asks nothing.
    pub fn ask_to_approve(&mut self, what: crate::approval::Waiting, way: &mut dyn Doorway) {
        let Some(post) = self.post().filter(|_| self.reach == Reach::Machine) else {
            self.view.approval_heard();
            return;
        };
        let gesture = way.sudo().gesture(crate::sudo::Asker::Update);
        let places = self
            .view
            .approval_places()
            .cloned()
            .unwrap_or_else(crate::approval::Places::here);
        let born = std::thread::Builder::new()
            .name("approval".to_owned())
            .spawn(move || {
                let came = crate::approval::run(&what, &places, gesture);
                let _ = post.send(Back::Approve(came));
            });
        match born {
            Ok(_) => {
                self.approve_waiting += 1;
                let said = way.texts().t(crate::key::MACHINE_SAID_ASKED);
                way.say(said, crate::echo::Echo::Long);
            }
            Err(e) => {
                tracing::error!(error = %e, "could not start the approval's thread");
                self.view.approval_heard();
                let said = way.texts().t(crate::security_words::refusal(
                    ronoctua_security::Why::Unstartable,
                ));
                way.say(said, crate::echo::Echo::Long);
            }
        }
    }

    /// A gesture of the parts' page carried out on a NAMED thread — a
    /// hand-in and a stick wait for root's oneshot —, whose verdict comes
    /// back to this door. A photograph's door does nothing.
    pub fn carry_parts(&mut self, act: crate::parts::Act, way: &mut dyn Doorway) {
        let Some(post) = self.post() else {
            return;
        };
        let places = self
            .view
            .parts_places()
            .cloned()
            .unwrap_or_else(crate::parts::Places::here);
        let born = std::thread::Builder::new()
            .name("parts".to_owned())
            .spawn(move || {
                let came = crate::parts::carry(&act, &places);
                let _ = post.send(Back::Parts(came));
            });
        match born {
            Ok(_) => self.parts_waiting += 1,
            Err(e) => {
                tracing::error!(error = %e, "could not start the parts' thread");
                let said = way.texts().t(crate::key::DISTRO_PARTS_SAID_NOT_STARTED);
                way.say(said, crate::echo::Echo::Long);
            }
        }
    }
}

/// CARRIES OUT what the view asked for, in the door `way` is — the ONE
/// handler both doors call, with the same sentences. A page's own move (the
/// step cursor, the output) is the door's, done before it hands the rest
/// here.
pub fn carry_out(errand: Errand, open: &mut Open, way: &mut dyn Doorway) {
    let say = |way: &mut dyn Doorway, key: &'static str| {
        let said = way.texts().t(key);
        way.say(said, crate::echo::Echo::Normal);
    };
    match errand {
        Errand::Nothing => return,
        // EACH DOOR'S OWN PAGE — the window's, the footer's one list — moved
        // by the door itself.
        Errand::Moved | Errand::Output(_) | Errand::Leap(_) => {}
        // A STEP SWITCHED: asked of the brain — the one place the switches
        // are written, so the run reads them — and its answer said when it
        // comes. With no brain answering, nothing is written by hand: why
        // is said instead.
        Errand::Switch { id, on } => match &mut open.reach {
            Reach::Machine if open.mind.is_none() => {
                say(way, module::key::SYSTEM_UPDATE_SWITCH_NO_BRAIN);
            }
            Reach::Machine => {
                let ask = ronoctua_brain::Ask::Set {
                    task: module::ID.to_owned(),
                    knob: ronoctua_brain::Knob::Step {
                        id: id.to_owned(),
                        on,
                    },
                };
                if !open.ask_the_brain(ask, Asked::Switch { id, on }, None) {
                    say(way, crate::key::ECHO_BRAIN_UNREACHABLE);
                }
            }
            // A PHOTOGRAPH'S MACHINE WITH NO BRAIN says what that door says.
            Reach::Kept(_) if open.no_brain => {
                say(way, module::key::SYSTEM_UPDATE_SWITCH_NO_BRAIN);
            }
            Reach::Kept(asked) => asked.push(format!(
                "would have switched {id} {}",
                if on { "on" } else { "off" }
            )),
        },
        // A REHEARSAL: through the brain, which counts down to it as to a
        // run and says why when it will not; with no brain, its unit
        // straight through systemd.
        Errand::Rehearse => match &mut open.reach {
            Reach::Machine if open.mind.is_none() => {
                let started = crate::update_window::rehearse_directly(open.post());
                if started == Started::Systemd {
                    open.view.started_here = Some(RunHere::Rehearsal);
                }
                say(
                    way,
                    match started {
                        Started::Nowhere => module::key::ECHO_SYSTEM_UPDATE_NOWHERE,
                        _ => module::key::ECHO_SYSTEM_UPDATE_REHEARSE_STARTED,
                    },
                );
            }
            Reach::Machine => {
                let ask = ronoctua_brain::Ask::Run {
                    task: module::ID.to_owned(),
                    rehearsal: true,
                };
                if !open.ask_the_brain(
                    ask,
                    Asked::Rehearse,
                    Some(module::key::SYSTEM_UPDATE_REHEARSE_ASKED),
                ) {
                    say(way, crate::key::ECHO_BRAIN_UNREACHABLE);
                }
            }
            Reach::Kept(asked) if open.no_brain => {
                asked.push(format!(
                    "would have started the rehearsal of {} through systemd",
                    module::ID
                ));
            }
            Reach::Kept(asked) => {
                asked.push(format!(
                    "would have asked for a rehearsal of {}",
                    module::ID
                ));
            }
        },
        // THE SCHEDULE: the brain's own page, on this task — the footer's
        // command, which a window asks the footer for.
        Errand::OpenBrain => match &mut open.reach {
            Reach::Machine => way.run(BRAIN_OPEN, Some(module::ID.to_owned())),
            Reach::Kept(asked) => asked.push(format!(
                "would have asked the footer to open the brain at {}",
                module::ID
            )),
        },
        // STARTED, through the door the brain starts it through, and said in
        // this door's words — and FOLLOWED, when the brain took it: held for
        // a metered way out, the question comes (`heard_the_brain`) — at once
        // when the brain already holds a scheduled run that way.
        Errand::Start => match &mut open.reach {
            Reach::Machine => {
                let started = crate::update_window::begin(open.post());
                say(way, crate::update_window::said_of(started));
                if started == Started::Systemd {
                    open.view.started_here = Some(RunHere::Run);
                }
                if started == Started::Brain {
                    open.followed = Some(Followed::now());
                    open.heard_the_brain(way);
                }
            }
            Reach::Kept(asked) => {
                asked.push(format!("would have asked for a run of {}", module::ID));
                // THE SCENE'S BRAIN HOLDS IT for a metered way out.
                if open.held_metered {
                    open.view.ask_metered();
                }
            }
        },
        // THE PERSON SAID YES TO WHAT IT SPENDS: started here, the brain's
        // queued run taken out first, the yes to a run on battery for this
        // run only.
        Errand::StartHere { on_battery } => match &mut open.reach {
            Reach::Machine => {
                let started =
                    crate::update_window::begin_anyway(open.mind.as_ref(), on_battery, open.post());
                say(way, crate::update_window::said_anyway(started, on_battery));
                if started == Started::Systemd {
                    open.view.started_here = Some(RunHere::Run);
                }
            }
            Reach::Kept(asked) => asked.push(format!(
                "would have started a run of {} here, {}",
                module::ID,
                if on_battery {
                    "on battery"
                } else {
                    "on this connection"
                }
            )),
        },
        Errand::NotStarted => say(way, crate::update_window::NOT_STARTED),
        Errand::KeptWaiting => say(way, crate::update_window::KEPT_WAITING),
        // THE BELONGINGS, whose conflicts the line counts, in this door's kind
        // of door: the footer leaves the update's region for theirs; a window
        // asks the footer for the belongings' window, and stays.
        Errand::OpenBelongings => match &mut open.reach {
            Reach::Machine => way.give_way(
                crate::belongings::OPEN_COMMAND,
                crate::belongings::WINDOW_COMMAND,
            ),
            Reach::Kept(asked) => asked.push(format!(
                "would have asked the footer for {}",
                crate::belongings::WINDOW_COMMAND
            )),
        },
        // THE COUNTDOWN'S ANSWERS, for THIS update only: the brain answers
        // whatever it announced, and a key of the update's must never skip
        // or stop another task's — with nothing of this one counting down or
        // running, the footer's sentence that nothing is about to happen.
        // A REHEARSAL STOPPED OR LET GO changed nothing, and is said so; A
        // RUN THIS DOOR STARTED is stopped through systemd, which holds it.
        Errand::Answer(proceeding) => match &mut open.reach {
            Reach::Machine => {
                let ours = open
                    .mind
                    .as_ref()
                    .filter(|m| crate::update_window::answers_this_update(m, proceeding));
                let rehearsal = open.view.rehearsal_now();
                let said = match (ours, open.view.started_here) {
                    (Some(mind), _) => crate::brainlink::answer(Some(mind), proceeding),
                    (None, Some(here)) if !proceeding => {
                        if crate::update_window::stop_here(here) {
                            crate::key::ECHO_BRAIN_STOPPED
                        } else {
                            module::key::ECHO_SYSTEM_UPDATE_NOWHERE
                        }
                    }
                    (None, _) => crate::key::ECHO_BRAIN_IDLE,
                };
                let said = if rehearsal
                    && !proceeding
                    && [
                        crate::key::ECHO_BRAIN_STOPPED,
                        crate::key::ECHO_BRAIN_POSTPONED,
                        crate::key::ECHO_BRAIN_REHEARSAL_LET_GO,
                    ]
                    .contains(&said)
                {
                    module::key::ECHO_SYSTEM_UPDATE_REHEARSAL_STOPPED
                } else {
                    said
                };
                say(way, said);
            }
            Reach::Kept(asked) => asked.push(format!(
                "would have answered the countdown: {}",
                if proceeding { "proceed" } else { "abort" }
            )),
        },
        // THE STATUS LINE'S SECOND CLICK: GitHub asked again.
        Errand::Recheck => match &mut open.reach {
            Reach::Machine => open.check_the_heads(true),
            Reach::Kept(asked) => asked.push("would have asked GitHub for the heads".to_owned()),
        },
        Errand::Cache(e) => cache_errand(e, open, way),
        Errand::Follow(e) => follow_errand(e, open, way),
        Errand::Parts(e) => parts_errand(e, open, way),
        // A GESTURE OF THE MACHINE'S, said yes to: asked again in the sudo bar,
        // and what came of it said here when it comes.
        Errand::Machine(what) => match &mut open.reach {
            Reach::Machine => open.ask_the_machine(what, way),
            Reach::Kept(asked) => {
                asked.push(format!("would have asked root {}", what.word()));
                open.view.gesture_heard();
            }
        },
        // A BRAIN MODULE OF THE PERSON'S, said yes to: its request staged and
        // asked again in the sudo bar, and what came of it said here.
        Errand::Approve(what) => match &mut open.reach {
            Reach::Machine => open.ask_to_approve(what, way),
            Reach::Kept(asked) => {
                asked.push(format!(
                    "would have asked root to approve {} at {}",
                    what.id, what.hash
                ));
                open.view.approval_heard();
            }
        },
        Errand::Say(k) => {
            let said = way.texts().t(k);
            way.say(said, crate::echo::Echo::Long);
        }
        Errand::Close => way.close(),
    }
    way.repaint();
}

/// What the binary cache's page asked of the door: the question on its
/// line, a sentence, a change — and root's netrc, a root gesture asked in
/// the sudo bar through the door's gate, whose verdict the door says when it
/// comes.
fn cache_errand(e: binary_cache::Errand, open: &mut Open, way: &mut dyn Doorway) {
    use binary_cache::Errand as Page;
    match e {
        Page::Ask(line) => way.ask(line),
        Page::Say(k) => {
            let said = way.texts().t(k);
            way.say(said, crate::echo::Echo::Long);
        }
        Page::Edit(edit) => match &mut open.reach {
            Reach::Machine => {
                let (Some(places), Some(page)) =
                    (open.view.cache_places().cloned(), open.view.cache_page())
                else {
                    return;
                };
                let readings = page.readings.clone();
                let applied = binary_cache::apply(&edit, &places, &readings, way.texts());
                open.view.cache_reread();
                let mut said = applied.said;
                if applied.netrc {
                    open.ask_for_the_netrc(places, way);
                    let footer = way.texts().t(crate::key::BINARY_CACHE_SAID_NETRC_FOOTER);
                    said = if said.is_empty() {
                        footer
                    } else {
                        format!("{said} {footer}")
                    };
                }
                if !said.is_empty() {
                    way.say(said, crate::echo::Echo::Long);
                }
            }
            Reach::Kept(asked) => {
                asked.push(format!(
                    "would have changed the binary cache: {}",
                    edit.word()
                ));
            }
        },
        Page::Nothing | Page::Moved | Page::Close => {}
    }
}

/// What the page of the apps that follow their releases asked of the door:
/// a sentence, or a turn of the switch — the person's copy of their options
/// — said on the door's line.
fn follow_errand(e: crate::follow::Errand, open: &mut Open, way: &mut dyn Doorway) {
    use crate::follow::Errand as Page;
    match e {
        Page::Say(k) => {
            let said = way.texts().t(k);
            way.say(said, crate::echo::Echo::Long);
        }
        Page::Turn(on) => match &mut open.reach {
            Reach::Machine => {
                let Some(places) = open.view.follow_places().cloned() else {
                    return;
                };
                let said = crate::follow::apply(on, &places, way.texts());
                open.view.follow_reread();
                way.say(said, crate::echo::Echo::Long);
            }
            Reach::Kept(asked) => asked.push(format!(
                "would have turned following the distro's releases {}",
                if on { "on" } else { "off" }
            )),
        },
        Page::Nothing | Page::Moved | Page::Close => {}
    }
}

/// What the parts' page asked of the door: the question on its line, a
/// sentence, or a gesture carried out on a thread whose verdict the door
/// says when it comes.
fn parts_errand(e: crate::parts::Errand, open: &mut Open, way: &mut dyn Doorway) {
    use crate::parts::Errand as Page;
    match e {
        Page::Ask(line) => way.ask(line),
        Page::Say(k) => {
            let said = way.texts().t(k);
            way.say(said, crate::echo::Echo::Long);
        }
        Page::Explain(e) => {
            let said = way.texts().t_with(e.key, &e.borrowed());
            way.say(said, crate::echo::Echo::Long);
        }
        // `C-v` IN THE PICKER: the door reads the clipboard and hands it to
        // the view (`View::picker_pasted`).
        Page::Paste => way.paste(),
        Page::Pick(_) => {}
        Page::Run(act) => match &mut open.reach {
            Reach::Machine => open.carry_parts(act, way),
            Reach::Kept(asked) => {
                asked.push(format!("would have asked the machine to {}", act.word()))
            }
        },
        // THE PERSON'S MIRRORS: their copy of their options, and their
        // consent for the hosts — a file of theirs, written here.
        Page::Mirrors(named) => match &mut open.reach {
            Reach::Machine => {
                let places = open
                    .view
                    .parts_places()
                    .cloned()
                    .unwrap_or_else(crate::parts::Places::here);
                let said = crate::parts::name_mirrors(&named, &places, way.texts());
                open.view.parts_reread();
                way.say(said, crate::echo::Echo::Long);
            }
            Reach::Kept(asked) => asked.push(format!("would have named the mirrors {named:?}")),
        },
        // `Update` IS THE VIEW'S: it became the update's own start there.
        Page::Nothing | Page::Moved | Page::Close | Page::Update => {}
    }
}

/// THE UPDATE IN THE FOOTER'S REGION (`LEADER s u u`): the window's twin —
/// the title, the list ([`View::list`]) and, while a run goes, its output —
/// ONE list, which the region's scroll, bar and wheel read, and only the
/// rows on the page built. The footer has no foot: what the thing under the
/// pointer is, and the cursor's, is said on its explanation line, joined
/// into one sentence (`door::Explained::joined`).
pub struct UpdateRegion {
    open: Open,
    /// Whether the page follows the cursor: after an arrow it does, after
    /// `C-j`, `C-k` or `G` it stays on the output the person moved to.
    on_the_step: bool,
    /// The row the pointer rests on: what it is on is said, not the
    /// cursor's, until a key — the window's foot's rule. On a row about
    /// nothing (air, a fact) the cursor's thing is said, as the foot says it.
    pointed: Option<usize>,
}

impl UpdateRegion {
    /// The title's rows, placed from `left`.
    fn title(&self, left: u16, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<ListRow> {
        let mut rows = self.open.view.status_lines(room, None, texts);
        for row in &mut rows {
            for f in &mut row.fields {
                f.at += left;
            }
        }
        rows
    }

    /// Whether the live output is shown under the list — the window's rule
    /// ([`Open::output_live`]) — while the view is on no page.
    fn live(&self) -> bool {
        self.open.output_live() && !self.open.view.page_open()
    }

    /// How many rows there are — counted, the output never built.
    fn len(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> usize {
        let (left, room) = crate::chrome::extent(columns);
        let live = if self.live() {
            2 + self.open.view.scrollback.last_run().count()
        } else {
            0
        };
        self.title(left, room, texts).len() + self.open.view.list_len(room, texts) + live
    }

    /// Rows `skip..skip + take`: the title, the list, then — while a run
    /// goes — a line of air, the output's heading and its lines.
    ///
    /// ON THE FOOTER'S GROUND, NOT A TERMINAL'S: a program's colours — the
    /// output's page's too — are folded to the chrome's inks
    /// ([`crate::output::on_the_chrome`]), the only ones measured there.
    fn window(
        &self,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
        skip: usize,
        take: usize,
    ) -> Vec<ListRow> {
        let (left, room) = crate::chrome::extent(columns);
        let title = self.title(left, room, texts);
        let mut out: Vec<ListRow> = title.iter().skip(skip).take(take).cloned().collect();
        let skip_list = skip.saturating_sub(title.len());
        let list =
            self.open
                .view
                .list_from(left, room, texts, skip_list, take.saturating_sub(out.len()));
        out.extend(list);
        if self.live() && out.len() < take {
            let before = title.len() + self.open.view.list_len(room, texts);
            let from = skip.max(before) - before;
            let lines = [
                ListRow::plain_air(),
                self.open.view.live_heading(left, room, texts),
            ]
            .into_iter()
            .chain(
                self.open
                    .view
                    .scrollback
                    .last_run()
                    .map(|l| ListRow::of_output(l, left, room)),
            );
            let rest = take - out.len();
            out.extend(lines.skip(from).take(rest));
        }
        for row in &mut out {
            for f in &mut row.fields {
                f.color = crate::output::on_the_chrome(f.color);
            }
        }
        out
    }

    /// The row the cursor is on, as drawn: the title above the list.
    fn cursor_row(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> Option<usize> {
        let (left, room) = crate::chrome::extent(columns);
        let title = self.title(left, room, texts);
        if let Some(n) = title.iter().position(|r| {
            r.item == Some(Item::Status) && self.open.view.focus() == Some(Item::Status)
        }) {
            return Some(n);
        }
        let rows = self.open.view.list(left, room, texts);
        self.open.view.cursor_row(&rows).map(|n| title.len() + n)
    }

    /// What the view asked: the page's own move done here — the cursor
    /// followed, the output a page up or down or at its end — and the rest
    /// carried out as both doors carry it out.
    fn errand(&mut self, errand: Errand, way: &mut dyn Doorway) {
        let errand = match errand {
            Errand::Leap(leap) => {
                let page = way.page_shown().lines;
                self.open.view.leap(leap, page)
            }
            other => other,
        };
        if errand == Errand::Moved {
            self.on_the_step = true;
        }
        if let Errand::Output(how) = errand {
            self.on_the_step = false;
            way.page(match how {
                Output::Up => Paging::Pages(-1),
                Output::Down => Paging::Pages(1),
                Output::Start => Paging::Top(0),
                Output::End => Paging::End,
                Output::Lines(n) => Paging::Lines(n),
            });
        }
        carry_out(errand, &mut self.open, way);
    }

    /// A key of the view — from the keyboard, the hint or the wheel.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        self.pointed = None;
        let errand = self.open.view.key(press);
        self.errand(errand, way);
    }

    /// The page asked for, while it is hidden.
    fn shown_on_the_step(&mut self, way: &mut dyn Doorway) {
        self.on_the_step = true;
        way.show();
        way.page(Paging::Follow);
    }

    /// What the pointer on `row` is told, `cell` along it: a step's box
    /// switches at once; elsewhere what a click takes it with — the
    /// window's foot's answer ([`View::gesture_for`]) —, and a row no click
    /// takes (a heading, the output, a row a question dims) promises none.
    fn explained_at(
        &self,
        row: &ListRow,
        cell: Option<u16>,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<String> {
        let item = row.item.as_ref()?;
        let how = if cell.is_some_and(|c| row.on_the_box(c)) {
            crate::door::Gesture::AtOnce
        } else {
            self.open.view.gesture_for(item)
        };
        let mut e = self.open.view.explained(item, how, texts);
        if !row.takes {
            e.gesture = None;
        }
        e.joined()
    }
}

impl Region for UpdateRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// NOT `TAB` FROM SECTION TO SECTION (`section_ring`): it walks its sections itself.
    fn walks_sections(&self) -> bool {
        false
    }

    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        let len = self.row_count(seen);
        self.rows_from(seen, 0, len)
    }

    /// COUNTED, NEVER BUILT: the output keeps up to two thousand lines.
    fn row_count(&self, seen: &Seen<'_>) -> usize {
        self.len(seen.columns, seen.texts)
    }

    /// ONLY THE ROWS ON THE PAGE are built.
    fn rows_from(&self, seen: &Seen<'_>, skip: usize, take: usize) -> Vec<Row> {
        self.window(seen.columns, seen.texts, skip, take)
            .into_iter()
            .map(|r| Row {
                fields: r.fields,
                takes: r.takes,
            })
            .collect()
    }

    /// EACH ROW SAYS WHAT IT IS under the pointer — the door's explanation
    /// of it in one line — and a sentence wrapped over several rows is one
    /// control. A step's row says it at the cell ([`Region::about_at`]): its
    /// box switches at once, its name selects first.
    fn told(&self, seen: &Seen<'_>, skip: usize, take: usize) -> Vec<Told> {
        let rows = self.window(seen.columns, seen.texts, skip, take);
        // The first row of the control being told, while it is among them.
        let mut control = None;
        rows.into_iter()
            .enumerate()
            .map(|(k, r)| {
                let n = skip + k;
                if !r.continues {
                    control = Some(n);
                }
                // A ROW ABOUT SOMETHING says it, whether a click takes it or
                // not — a heading, the output —, as the window's foot does;
                // a step's says it at the cell ([`Region::about_at`]).
                let about = if r.boxed.is_some() {
                    None
                } else {
                    self.explained_at(&r, None, seen.texts)
                };
                // THE ROW A CLICK LANDS ON: its control's first — unknown when
                // the control began above the rows asked for, and then the
                // footer asks (`Region::control_of`).
                Told {
                    about,
                    continues: r.continues,
                    control: if !r.takes {
                        None
                    } else if r.continues {
                        control
                    } else {
                        Some(n)
                    },
                    ..Told::from(Row {
                        fields: r.fields,
                        takes: r.takes,
                    })
                }
            })
            .collect()
    }

    /// A SENTENCE WRAPPED OVER SEVERAL ROWS IS ONE CONTROL: every row of it
    /// lights with the one under the pointer, as the window lights it — a
    /// click on any of them takes the sentence.
    fn control_of(&self, row: usize, seen: &Seen<'_>) -> usize {
        let mut first = row;
        while first > 0
            && self
                .window(seen.columns, seen.texts, first, 1)
                .first()
                .is_some_and(|r| r.continues)
        {
            first -= 1;
        }
        first
    }

    /// THE POINTER LIGHTS WHAT THE CLICK TAKES: a step's box alone, where a
    /// click switches it at once; the row elsewhere, where a click selects
    /// it; nothing on a row a click takes nothing on.
    fn lit(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> Lit {
        let Some(r) = self.window(seen.columns, seen.texts, row, 1).pop() else {
            return Lit::Nothing;
        };
        match (r.boxed, cell) {
            _ if !r.takes => Lit::Nothing,
            (Some((from, to)), Some(c)) if (from..to).contains(&c) => Lit::Cells(from, to),
            _ => Lit::Row,
        }
    }

    /// What the pointer on a step says, at the cell it is on.
    fn about_at(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> Option<String> {
        let r = self
            .window(seen.columns, seen.texts, row, 1)
            .pop()
            .filter(|r| r.boxed.is_some())?;
        self.explained_at(&r, cell, seen.texts)
    }

    /// WHAT THE CURSOR IS ON, said on the explanation line — while the
    /// pointer rests on no row of the region, or after a key: the pointer
    /// outranks the cursor until the next key, as in the window's foot.
    fn note(&self, seen: &Seen<'_>) -> Option<String> {
        if self.pointed.is_some_and(|row| {
            self.window(seen.columns, seen.texts, row, 1)
                .pop()
                .is_some_and(|r| r.item.is_some())
        }) {
            return None;
        }
        let item = self.open.view.cursor_item()?;
        self.open
            .view
            .explained(&item, crate::door::Gesture::Ret, seen.texts)
            .joined()
    }

    /// The pointer moved: whether it rests on a row of the region — what
    /// the explanation line says then is that row, not the cursor's.
    fn hovered(
        &mut self,
        under: &crate::region::Under,
        _: &mut Option<Box<dyn std::any::Any>>,
        _: &mut dyn Doorway,
    ) -> bool {
        std::mem::replace(&mut self.pointed, under.row) != under.row
    }

    /// The pointer left: the cursor's thing is said again.
    fn left(&mut self, _: &mut Option<Box<dyn std::any::Any>>, _: &mut dyn Doorway) {
        self.pointed = None;
    }

    /// The table its window's header teaches too.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.open.view.hint(seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        None
    }

    /// THE CURSOR'S ROW — after an arrow. After `C-j`, `C-k` or `G` the page
    /// is on the output the person moved it to, and following the cursor
    /// would take it back.
    fn cursor_in(&self, seen: &Seen<'_>) -> Option<usize> {
        if !self.on_the_step {
            return None;
        }
        self.cursor_row(seen.columns, seen.texts)
    }

    /// The cursor, the output, and the things one does here — through the
    /// reader its window calls too.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        // THE FILE PICKER, while it is up over the parts' page, reads its
        // keys before the view does.
        if self.open.view.picker_takes(key) {
            self.pointed = None;
            let errand = self.open.view.pick_key(key);
            self.errand(errand, way);
            return Taken::Yes;
        }
        match crate::update_window::press_of_key(key) {
            Some(press) => {
                self.press(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    /// The update's commands and its binary cache page's, which its window
    /// names — asked by [`crate::update_window::order_of`], never spelled.
    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        let Some(order) = crate::update_window::order_of(id) else {
            return Taken::No;
        };
        match order {
            Order::Start => {
                if self.open.start_by_hand(way) {
                    self.on_the_step = true;
                }
            }
            Order::Open => self.shown_on_the_step(way),
            Order::Watch => way.open_window(vec![WINDOW_WORDS.to_owned()]),
            Order::Cache(binary_cache::OPEN) => {
                self.open.view.open_cache();
                self.shown_on_the_step(way);
            }
            Order::Follow => {
                self.open.view.open_follow();
                self.shown_on_the_step(way);
            }
            Order::Parts(crate::parts::OPEN) => {
                self.open.view.open_parts();
                self.shown_on_the_step(way);
            }
            // A QUESTION OF THE PARTS' PAGE: its answer, or — run by its name
            // with nothing typed — the question asked.
            Order::Parts(question) => {
                way.show();
                let errand = match argument {
                    Some(typed) => self.open.view.parts_typed(question, typed),
                    None => self.open.view.parts_ask_for(question),
                };
                self.errand(errand, way);
            }
            // A GESTURE OF THE MACHINE'S, by its name: the page, on its
            // question — the no first — or why the records do not offer it.
            Order::Machine(what) => {
                self.shown_on_the_step(way);
                let errand = self.open.view.ask_gesture(what);
                self.errand(errand, way);
            }
            // A BRAIN MODULE OF THE PERSON'S THAT WAITS, by the name: the
            // page, on its question — the one the argument names, or the
            // first — or the sentence that none waits.
            Order::Approve => {
                self.shown_on_the_step(way);
                let errand = self.open.view.ask_approval(argument);
                self.errand(errand, way);
            }
            // ROOT'S NETRC, by its name: a root gesture, asked in the sudo
            // bar above the footer.
            Order::Cache(binary_cache::NETRC) => {
                self.open
                    .ask_for_the_netrc(binary_cache::Places::here(), way);
            }
            // A QUESTION OF THE PAGE: its answer, or — run by its name with
            // nothing typed — the question asked.
            Order::Cache(question) => {
                way.show();
                let errand = match argument {
                    Some(typed) => self.open.view.cache_answered(question, typed),
                    None => self.open.view.cache_ask_for(question),
                };
                self.errand(errand, way);
            }
        }
        way.repaint();
        Taken::Yes
    }

    /// A click on a row with no cell said: what the row is takes it.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        self.clicked_at(row, None, way);
    }

    /// A CLICK ON A STEP'S BOX switches it at once; anywhere else on a row
    /// that takes a click, the first selects and the second does what `RET`
    /// does — the window's click, through the same `View::clicked`.
    fn clicked_at(&mut self, row: usize, cell: Option<u16>, way: &mut dyn Doorway) {
        let Some(r) = self.window(way.columns(), way.texts(), row, 1).pop() else {
            return;
        };
        let errand = match (r.item, cell) {
            (Some(Item::Step(id)), Some(c))
                if r.boxed.is_some_and(|(f, t)| (f..t).contains(&c)) =>
            {
                self.open.view.clicked_box(id)
            }
            (Some(item), _) if r.takes => self.open.view.clicked(item),
            _ => return,
        };
        self.errand(errand, way);
    }

    /// THE PAGES' QUESTIONS — the binary cache's, the parts' — typed on the
    /// minibuffer: the window's line answers into the same
    /// `View::page_answered`.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        let errand = self.open.view.page_answered(id, text);
        self.errand(errand, way);
    }

    fn unanswered(&mut self, _: &'static str, way: &mut dyn Doorway) {
        self.open.view.page_unanswered();
        way.repaint();
    }

    /// What was copied, for the file picker.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        self.open.view.picker_pasted(text);
        way.repaint();
    }

    /// A line of either journal, an answer of the brain's, or root's netrc.
    /// DATA DOES NOT MOVE THE PAGE, with the one exception a log has: a page
    /// the person took to the output's END stays on the end as the end
    /// moves — the window's "zero follows". A page on the list stays there.
    fn woken(&mut self, way: &mut dyn Doorway) {
        let before = self.len(way.columns(), way.texts());
        if self.open.woken(way) {
            let after = self.len(way.columns(), way.texts());
            if !self.on_the_step {
                way.page(Paging::Tail(after.saturating_sub(before)));
            }
            let _ = self.open.view.owes_a_frame();
            way.repaint();
        }
    }

    /// The brain's mind: a run asked for here, followed whatever the page
    /// does.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(mind) = news.downcast_ref::<ronoctua_brain::Mind>()
            && self.open.heard_the_mind(mind.clone(), way)
        {
            self.on_the_step = true;
        }
    }

    /// ITS REPORTS LOOKED AT, one `stat` per file, on the footer's second —
    /// the window's beat.
    fn tick(&mut self, _: std::time::Instant, way: &mut dyn Doorway) -> bool {
        let moved = self.open.reread_if_moved();
        if moved {
            self.open.say_the_news(way);
            way.repaint();
        }
        moved
    }

    /// THE TWO JOURNALS, each on a thread of its own, for as long as the page
    /// is shown — and whether an update is there, asked once each time it
    /// is (a head GitHub said in the last quarter of an hour is said from
    /// its copy). A photograph's footer follows and asks nothing.
    fn shown(&mut self, way: &mut dyn Doorway) {
        self.on_the_step = true;
        if !way.photographing() {
            self.open.follow_the_journals();
            self.open.check_the_heads(false);
        }
    }

    /// The journals stop with the page — `-f` never ends by itself. A page
    /// closed while it follows a run starts over, read again, when it is
    /// shown next: its output is the journals' backlog then.
    fn closing(&mut self, _: &mut dyn Doorway) {
        self.open.stop_the_journals();
        if self.open.lingers() {
            self.open.view = View::read();
        }
    }

    fn lingers(&self) -> bool {
        self.open.lingers()
    }
}

/// What a command does to the update while its region is closed: its window
/// opens on its own; its view and its page's open the region; a run asked
/// for by hand, and root's netrc, act WITHOUT the page — the region comes to
/// life hidden, and follows them.
fn closed(command: &str, _: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match crate::update_window::order_of(command) {
        None => Closed::NotMine,
        Some(Order::Watch) => {
            way.open_window(vec![WINDOW_WORDS.to_owned()]);
            Closed::Taken
        }
        Some(Order::Start | Order::Cache(binary_cache::NETRC)) => Closed::Behind,
        Some(
            Order::Open
            | Order::Cache(_)
            | Order::Follow
            | Order::Parts(_)
            | Order::Machine(_)
            | Order::Approve,
        ) => Closed::Opens,
    }
}

/// The update, for the footer's slot host: the declared steps and what the
/// machine's reports and records say — a photograph's footer reaches nothing
/// past the door, and follows no journal.
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let photographing = way.photographing();
    let reach = if photographing {
        Reach::Kept(Vec::new())
    } else {
        Reach::Machine
    };
    Box::new(UpdateRegion {
        open: Open::new(View::read(), reach, (!photographing).then(|| way.wake())),
        on_the_step: true,
        pointed: None,
    })
}

/// The update, as the binary registers it in the footer's slot host.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::Effect;
    use std::rc::Rc;

    fn way() -> crate::region::Recorder {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        crate::region::Recorder::new(
            Rc::new(texts),
            100,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
    }

    fn region() -> UpdateRegion {
        let view = View::of(crate::update_window::Verdicts::of(&[]), Vec::new());
        UpdateRegion {
            open: Open::new(view, Reach::Kept(Vec::new()), None),
            on_the_step: true,
            pointed: None,
        }
    }

    /// FAILS IF: a run asked for by hand with the page closed opens the
    /// page over whatever the region holds, or the view and its page's
    /// stop opening it, or its window stops being its own.
    #[test]
    fn what_a_command_does_to_the_closed_update() {
        assert_eq!(
            closed(crate::update_window::START, None, &mut way()),
            Closed::Behind
        );
        assert_eq!(
            closed(binary_cache::NETRC, None, &mut way()),
            Closed::Behind
        );
        assert_eq!(
            closed(crate::update_window::OPEN, None, &mut way()),
            Closed::Opens
        );
        assert_eq!(closed(binary_cache::OPEN, None, &mut way()), Closed::Opens);
        assert_eq!(closed(binary_cache::NAME, None, &mut way()), Closed::Opens);
        let mut w = way();
        assert_eq!(
            closed(crate::update_window::WATCH, None, &mut w),
            Closed::Taken
        );
        assert_eq!(w.owed, vec![Effect::OpenWindow(vec!["update".to_owned()])]);
        assert_eq!(closed("stick-open", None, &mut way()), Closed::NotMine);
    }

    /// FAILS IF: the output's keys stop moving the footer's page — a page up,
    /// a page down, the end — or stop leaving the step cursor's page, so the
    /// next arrow would not bring it back.
    #[test]
    fn the_outputs_keys_page_the_footer() {
        let mut r = region();
        for (key, paging) in [
            ("C-j", Paging::Pages(1)),
            ("C-k", Paging::Pages(-1)),
            ("G", Paging::End),
        ] {
            let mut w = way();
            assert!(
                r.key(ronoctua_keymap::Key::parse(key).expect("a key"), &mut w)
                    .took()
            );
            assert!(
                w.owed.contains(&Effect::Page(paging)),
                "{key}: {:?}",
                w.owed
            );
            assert!(!r.on_the_step, "{key}");
        }
        let _ = r.key(
            ronoctua_keymap::Key::parse("<down>").expect("a key"),
            &mut way(),
        );
        assert!(
            r.on_the_step,
            "an arrow did not bring the page back to the step"
        );
    }

    /// FAILS IF: the belongings' line asks the footer for anything but the
    /// belongings' own view (the window asks for their window), or a
    /// photograph's reaches anything.
    #[test]
    fn the_belongings_line_gives_way_to_the_belongings() {
        let mut open = Open::new(
            View::of(crate::update_window::Verdicts::of(&[]), Vec::new()),
            Reach::Machine,
            None,
        );
        let mut w = way();
        carry_out(Errand::OpenBelongings, &mut open, &mut w);
        assert_eq!(
            w.owed.first(),
            Some(&Effect::GiveWay(crate::belongings::OPEN_COMMAND))
        );
        let mut r = region();
        carry_out(Errand::OpenBelongings, &mut r.open, &mut way());
        assert_eq!(r.open.would_have().len(), 1);
    }

    /// FAILS IF: the journals' lines posted to either door's inbox do not
    /// reach the output, the footer's region shows the output while nothing
    /// runs or hides it while this update runs, or a page the person took
    /// to the output's end is not kept there as the end moves — or one on
    /// the list is moved.
    #[test]
    fn the_journals_reach_the_output_shown_only_while_a_run_goes() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = seen_at(&t, &zone);
        let mut r = region();
        r.open.inbox = Some(Inbox::new(crate::wake::Wake::new(|| {})));
        let post = r.open.post().expect("an inbox");
        let said =
            |at_us, text| Back::Line(crate::output::OutputLine::written(at_us, false, "", text));
        assert!(post.send(said(1, "one")));
        assert!(post.send(said(2, "two")));
        let mut w = way();
        r.woken(&mut w);
        assert_eq!(r.open.view.scrollback().amount(), 2);
        let text = |r: &UpdateRegion| -> String {
            r.rows(&seen)
                .iter()
                .flat_map(|row| row.fields.iter().map(|f| f.text.clone()))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert!(
            !text(&r).contains("two"),
            "the output shows while nothing runs"
        );
        assert_eq!(r.rows(&seen).len(), r.row_count(&seen));
        r.open.view = r.open.view.clone().with_brain(crate::update_window::Brain {
            running: true,
            ..crate::update_window::Brain::default()
        });
        assert!(
            text(&r).ends_with("two"),
            "the run's output is not under the list"
        );
        assert_eq!(r.rows(&seen).len(), r.row_count(&seen));
        let all = r.rows(&seen);
        let cut = r.rows_from(&seen, all.len() - 2, 2);
        assert_eq!(
            cut.iter().map(|x| x.fields.clone()).collect::<Vec<_>>(),
            all[all.len() - 2..]
                .iter()
                .map(|x| x.fields.clone())
                .collect::<Vec<_>>()
        );
        assert!(
            !w.owed.iter().any(|e| matches!(e, Effect::Page(_))),
            "a page on the list moved with the output: {:?}",
            w.owed
        );
        let _ = r.key(ronoctua_keymap::Key::parse("G").expect("a key"), &mut way());
        assert!(post.send(said(3, "three")));
        let mut w = way();
        r.woken(&mut w);
        assert!(
            w.owed.contains(&Effect::Page(Paging::Tail(1))),
            "the end did not follow: {:?}",
            w.owed
        );
    }

    /// FAILS IF: a run this door started straight through systemd is not
    /// one while it goes — its output under the list, the run's keys taught
    /// (the rehearsal's for a rehearsal), the status saying it — or is still
    /// one after systemd said it ended, its output staying for as long as
    /// the door is open.
    #[test]
    fn a_run_started_here_goes_until_systemd_says_it_ended() {
        use crate::update_window::{OF_THE_REHEARSAL, OF_THE_RUN, OF_THE_VIEW, Status};
        let mut r = region();
        r.open.inbox = Some(Inbox::new(crate::wake::Wake::new(|| {})));
        let post = r.open.post().expect("an inbox");
        for (here, keys, status) in [
            (RunHere::Run, OF_THE_RUN, Status::Running),
            (RunHere::Rehearsal, OF_THE_REHEARSAL, Status::Rehearsing),
        ] {
            r.open.view.started_here = Some(here);
            assert!(r.open.output_live(), "{here:?}: no live output");
            assert_eq!(r.open.view.taught(), keys, "{here:?}");
            assert_eq!(r.open.view.status(), status);
            assert!(post.send(Back::Ended));
            r.woken(&mut way());
            assert_eq!(r.open.view.started_here, None);
            assert!(!r.open.output_live(), "{here:?}: the output stayed");
            assert_eq!(r.open.view.taught(), OF_THE_VIEW);
        }
    }

    /// FAILS IF: what GitHub said, handed back to either door's inbox, does
    /// not reach the status line and draw it again — or moves the page —, or
    /// a photograph's door asks GitHub at all, its re-check kept instead.
    #[test]
    fn what_github_said_reaches_the_status_and_a_photograph_asks_nothing() {
        use crate::update_window::{Status, Unchecked};
        let mut r = region();
        r.open.check_the_heads(false);
        assert_eq!(r.open.view.status(), Status::Unasked, "a photograph asked");
        carry_out(Errand::Recheck, &mut r.open, &mut way());
        assert_eq!(
            r.open.would_have(),
            vec!["would have asked GitHub for the heads".to_owned()]
        );
        r.open.inbox = Some(Inbox::new(crate::wake::Wake::new(|| {})));
        let post = r.open.post().expect("an inbox");
        assert!(post.send(Back::Heads(
            0,
            Err(ronoctua_software::heads::Unread::NoLock)
        )));
        let mut w = way();
        r.woken(&mut w);
        assert_eq!(r.open.view.status(), Status::Unchecked(Unchecked::NoLock));
        assert!(w.owed.contains(&Effect::Repaint), "{:?}", w.owed);
        assert!(
            !w.owed.iter().any(|e| matches!(e, Effect::Page(_))),
            "what GitHub said moved the page: {:?}",
            w.owed
        );
    }

    /// FAILS IF: an answer to an older question of the heads says over the
    /// answer to a newer one — the slow first look that times out offline
    /// after the person's re-check came back up to date.
    #[test]
    fn an_older_answer_of_the_heads_never_says_over_a_newer_one() {
        use crate::update_window::{Status, Unchecked};
        let mut r = region();
        r.open.inbox = Some(Inbox::new(crate::wake::Wake::new(|| {})));
        let post = r.open.post().expect("an inbox");
        // TWO QUESTIONS ASKED; the second's answer comes first.
        r.open.heads_asked = 2;
        assert!(post.send(Back::Heads(2, Ok(Vec::new()))));
        assert!(post.send(Back::Heads(
            1,
            Err(ronoctua_software::heads::Unread::Unreadable)
        )));
        r.woken(&mut way());
        assert_eq!(r.open.view.status(), Status::Unchecked(Unchecked::Nothing));
    }

    /// FAILS IF: what came of root's netrc is not said in the door that asked,
    /// or a door still waiting for it lets the update go.
    #[test]
    fn the_netrcs_verdict_is_said_in_the_door() {
        let mut r = region();
        r.open.inbox = Some(Inbox::new(crate::wake::Wake::new(|| {})));
        r.open.netrc_waiting = true;
        assert!(r.lingers(), "a door waiting for root's netrc let it go");
        let post = r.open.post().expect("an inbox");
        assert!(post.send(Back::Netrc(binary_cache::Came::Removed)));
        let mut w = way();
        r.woken(&mut w);
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Say(_, crate::echo::Echo::Long))),
            "{:?}",
            w.owed
        );
        assert!(!r.lingers());
    }

    /// FAILS IF: a gesture of the machine's said yes to does not go through
    /// the door's gate as the helper's own wordless verb, the door lets the
    /// update go before what came of it is said there — done, or the
    /// machine's own sentence for a refusal — or a photograph's reaches
    /// root instead of keeping what it would have asked.
    #[test]
    fn a_machine_gesture_is_asked_through_the_gate_and_said_in_the_door() {
        use crate::machine_gesture::{Came, Gesture};
        use std::sync::{Arc, Mutex};
        let texts = Rc::new(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        let says = |w: &crate::region::Recorder, key: &str| {
            let wanted = texts.t(key);
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Say(said, _) if *said == wanted))
        };
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut w = crate::region::Recorder::new(
            Rc::clone(&texts),
            100,
            false,
            crate::sudo::Gate::fake(Arc::clone(&log), Ok(())),
            crate::wake::Wake::new(|| {}),
        );
        let mut open = Open::new(
            View::of(crate::update_window::Verdicts::of(&[]), Vec::new()),
            Reach::Machine,
            Some(crate::wake::Wake::new(|| {})),
        );
        carry_out(Errand::Machine(Gesture::Hold), &mut open, &mut w);
        assert!(says(&w, crate::key::MACHINE_SAID_ASKED), "{:?}", w.owed);
        // THE VERDICT, waited for in a bounded regime: the gate's thread.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut heard = way();
        while open.lingers() && std::time::Instant::now() < until {
            heard = way();
            open.woken(&mut heard);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!open.lingers(), "the gesture's verdict never came");
        assert!(
            says(&heard, crate::key::MACHINE_SAID_HOLD),
            "{:?}",
            heard.owed
        );
        assert_eq!(
            *log.lock().expect("log"),
            vec![(
                crate::sudo::Asker::Update,
                ronoctua_security::verb::RootVerb::MachineHold
            )]
        );
        // THE MACHINE'S REFUSAL is the machine's sentence, in the door.
        let post = open.post().expect("an inbox");
        open.machine_waiting = 1;
        assert!(post.send(Back::Machine(Came::Refused("busy".to_owned()))));
        let mut heard = way();
        open.woken(&mut heard);
        assert!(
            says(
                &heard,
                module::machine_refusal_said(module::MachineRefusal::Busy)
            ),
            "{:?}",
            heard.owed
        );
        assert!(!open.lingers());
        // A PHOTOGRAPH'S keeps what it would have asked.
        let mut r = region();
        carry_out(Errand::Machine(Gesture::TryAgain), &mut r.open, &mut way());
        let kept = r.open.would_have();
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert!(kept[0].contains(Gesture::TryAgain.word()), "{kept:?}");
        // AND BY NAME, the closed update opens on the question.
        for what in Gesture::ALL {
            assert_eq!(closed(what.command(), None, &mut way()), Closed::Opens);
        }
    }

    /// FAILS IF: an approval said yes to is not staged in the person's
    /// approval folder and asked through the door's gate as the helper's
    /// `person-module-approve` with the module and the hash shown; the door
    /// lets the update go before what came of it is said there; root's own
    /// reason is not what is said of a refusal; or a photograph's reaches
    /// root instead of keeping what it would have asked.
    #[test]
    fn an_approval_is_staged_asked_through_the_gate_and_said_in_the_door() {
        use std::sync::{Arc, Mutex};
        let texts = Rc::new(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        let room =
            std::env::temp_dir().join(format!("ronoctua-approve-door-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&room);
        std::fs::create_dir_all(room.join("approve")).expect("a room");
        let places = crate::approval::Places {
            applied: room.join("person-applied.json"),
            approved: room.join("approved.json"),
            verdict: room.join("approve-verdict.json"),
            requests: room.join("approve"),
        };
        let tea = crate::approval::Waiting {
            id: "my-tea".to_owned(),
            hash: "sha256-tea".to_owned(),
        };
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut w = crate::region::Recorder::new(
            Rc::clone(&texts),
            100,
            false,
            crate::sudo::Gate::fake(Arc::clone(&log), Ok(())),
            crate::wake::Wake::new(|| {}),
        );
        let view = View::of(crate::update_window::Verdicts::of(&[]), Vec::new())
            .with_approval_places(places.clone());
        let mut open = Open::new(view, Reach::Machine, Some(crate::wake::Wake::new(|| {})));
        carry_out(Errand::Approve(tea.clone()), &mut open, &mut w);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut heard = way();
        while open.lingers() && std::time::Instant::now() < until {
            heard = way();
            open.woken(&mut heard);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!open.lingers(), "the approval's verdict never came");
        let said = texts.t_with(
            crate::key::MACHINE_APPROVE_SAID_APPROVED,
            &[("id", "my-tea")],
        );
        assert!(
            heard
                .owed
                .iter()
                .any(|e| matches!(e, Effect::Say(s, _) if *s == said)),
            "{:?}",
            heard.owed
        );
        assert_eq!(
            *log.lock().expect("log"),
            vec![(
                crate::sudo::Asker::Update,
                ronoctua_security::verb::RootVerb::PersonModuleApprove {
                    id: "my-tea".to_owned(),
                    hash: "sha256-tea".to_owned(),
                }
            )]
        );
        let staged: ronoctua_person::approve::Request = ronoctua_person::parse(
            &std::fs::read(places.requests.join(ronoctua_person::approve::REQUEST))
                .expect("the request was staged in the room"),
        )
        .expect("a request");
        assert_eq!(
            staged,
            ronoctua_person::approve::Request::new("my-tea", "sha256-tea")
        );
        // ROOT'S REASON, in the door.
        let post = open.post().expect("an inbox");
        open.approve_waiting = 1;
        assert!(post.send(Back::Approve(crate::approval::Came::Refused(
            "my-tea".to_owned(),
            ronoctua_person::approve::Denied::Changed
        ))));
        let mut heard = way();
        open.woken(&mut heard);
        let said = texts.t_with(
            crate::key::MACHINE_APPROVE_SAID_CHANGED,
            &[("id", "my-tea")],
        );
        assert!(
            heard
                .owed
                .iter()
                .any(|e| matches!(e, Effect::Say(s, _) if *s == said)),
            "{:?}",
            heard.owed
        );
        assert!(!open.lingers());
        let _ = std::fs::remove_dir_all(&room);
        // A PHOTOGRAPH'S keeps what it would have asked.
        let mut r = region();
        carry_out(Errand::Approve(tea), &mut r.open, &mut way());
        let kept = r.open.would_have();
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert!(kept[0].contains("my-tea"), "{kept:?}");
        // AND BY NAME, the closed update opens on the question.
        assert_eq!(
            closed(crate::approval::APPROVE, None, &mut way()),
            Closed::Opens
        );
    }

    /// The update turned to the binary cache's page over a public cache with
    /// a token kept — and, `grave`, with the removal's question asked: its
    /// row clicked twice, as a person asks it.
    fn on_the_page(grave: bool) -> (UpdateRegion, crate::region::Recorder) {
        use crate::binary_cache::{Readings, Row as PageRow, TokenState, rows_of};
        let readings = Readings {
            named: ronoctua_cachix::settings::Named::Cache(ronoctua_person::options::BinaryCache {
                name: "someone".to_owned(),
                public_key: "someone.cachix.org-1:QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ="
                    .to_owned(),
                visibility: ronoctua_person::options::Visibility::Public,
            }),
            token: TokenState::Kept,
            ..Readings::default()
        };
        let remove = rows_of(&readings)
            .iter()
            .position(|r| *r == PageRow::Remove)
            .expect("a cache is removable");
        let view =
            View::of(crate::update_window::Verdicts::of(&[]), Vec::new()).with_cache(readings);
        let mut r = UpdateRegion {
            open: Open::new(view, Reach::Kept(Vec::new()), None),
            on_the_step: true,
            pointed: None,
        };
        let mut w = way();
        if grave {
            let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
            let zone = ronoctua_files::when::Zone::utc();
            // THE ROW, found again before each click: the first moves the
            // cursor, and what the head says above the page moves with it.
            let removal = |r: &UpdateRegion| {
                (0..r.row_count(&seen_at(&t, &zone)))
                    .find(|n| {
                        r.window(100, &t, *n, 1)
                            .first()
                            .and_then(|x| x.item.clone())
                            == Some(Item::Row(remove))
                    })
                    .expect("the removal is drawn")
            };
            let row = removal(&r);
            r.clicked(row, &mut w);
            let row = removal(&r);
            r.clicked(row, &mut w);
            assert!(
                r.open
                    .view
                    .cache_page()
                    .is_some_and(|c| c.page.confirm().is_some()),
                "the removal asked nothing"
            );
            w.owed.clear();
        }
        (r, w)
    }

    fn seen_at<'a>(t: &'a ronoctua_i18n::Texts, zone: &'a ronoctua_files::when::Zone) -> Seen<'a> {
        Seen {
            texts: t,
            columns: 100,
            lines: 60,
            zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        }
    }

    /// FAILS IF: on the binary cache's page a resting pointer lights a row a
    /// click takes nothing on — the heading, the air before it, and every
    /// row of the page while its grave question is asked, when the click is
    /// the question's — or leaves unlit one it takes: a row, an answer.
    /// THE LIGHT IS THE CLICK: every row clicked once, on a page of its
    /// own, and lit exactly where something happened. The rows stayed lit
    /// under the question.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = seen_at(&t, &zone);
        for grave in [false, true] {
            let held = crate::region::held_to_the_click(
                || on_the_page(grave),
                &seen,
                |r, w: &crate::region::Recorder| crate::region::seen(r, &w.owed, &seen),
            );
            assert!(
                held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
                "grave {grave}: no row lit, or every row lit: {held:?}"
            );
            let unheld = crate::region::unheld(&held);
            assert!(unheld.is_empty(), "grave {grave}: {unheld:?}");
            if grave {
                let r = on_the_page(true).0;
                let rows = r.rows(&seen);
                let page_rows = (0..rows.len())
                    .filter(|n| {
                        r.window(100, &t, *n, 1)
                            .first()
                            .is_some_and(|x| matches!(x.item, Some(Item::Row(_))))
                            && !rows[*n].takes
                    })
                    .count();
                assert!(page_rows >= 3, "the page's rows lit under the question");
            }
        }
    }

    /// FAILS IF: the footer's twin does not say what the row under the
    /// pointer is, with what a click there does — a step's box at once, its
    /// name the first click selects — or does not say the cursor's thing on
    /// the explanation line, or says the cursor's over the pointer's before
    /// a key.
    #[test]
    fn the_twin_says_the_pointers_row_and_the_cursors_thing() {
        use crate::door::Gesture;
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = seen_at(&t, &zone);
        let mut r = region();
        let rows = r.window(seen.columns, &t, 0, usize::MAX);
        let schedule = rows
            .iter()
            .position(|x| x.item == Some(Item::Schedule))
            .expect("the schedule's row");
        let told = r.told(&seen, schedule, 1);
        let about = told[0].about.clone().expect("the schedule says nothing");
        let selects = crate::door::foot::gesture(
            &t,
            Gesture::Selects,
            &t.t(module::key::SYSTEM_UPDATE_ACT_OPEN_BRAIN),
        );
        assert!(about.starts_with(&selects), "{about}");
        assert!(
            about.contains(&t.t(module::key::SYSTEM_UPDATE_SCHEDULE_DOES)),
            "{about}"
        );
        // THE CURSOR'S THING, on the explanation line, with `RET`.
        let note = r.note(&seen).expect("the cursor says nothing");
        assert!(
            note.contains(&t.t(module::key::SYSTEM_UPDATE_START_DOES)),
            "{note}"
        );
        // A STEP: its box at once, its name selects first.
        let off = module::steps()
            .iter()
            .find(|s| !s.on_by_default && !s.needs_root)
            .expect("a step that ships off");
        let at = rows
            .iter()
            .position(|x| x.item == Some(Item::Step(off.id)))
            .expect("the step's row");
        let (from, to) = rows[at].boxed.expect("a box");
        let does = t.t(module::key::SYSTEM_UPDATE_ACT_SWITCH_ON);
        let on_the_box = r
            .about_at(at, Some(from), &seen)
            .expect("the box says nothing");
        assert!(
            on_the_box.starts_with(&crate::door::foot::gesture(&t, Gesture::AtOnce, &does)),
            "{on_the_box}"
        );
        let on_the_name = r
            .about_at(at, Some(to + 4), &seen)
            .expect("the name says nothing");
        assert!(
            on_the_name.starts_with(&crate::door::foot::gesture(&t, Gesture::Selects, &does)),
            "{on_the_name}"
        );
        // THE POINTER OUTRANKS THE CURSOR until a key.
        let under = crate::region::Under {
            at: (0.0, 0.0),
            line: Some(at),
            row: Some(at),
            cell: Some(from),
            base: 0,
            ctrl: false,
            shift: false,
            frame: None,
        };
        let mut w = way();
        assert!(
            r.hovered(&under, &mut None, &mut w),
            "no frame for the pointer's row"
        );
        assert_eq!(
            r.note(&seen),
            None,
            "the cursor's thing said over the pointer's"
        );
        let _ = r.key(
            ronoctua_keymap::Key::parse("<down>").expect("a key"),
            &mut way(),
        );
        assert!(
            r.note(&seen).is_some(),
            "a key did not bring the cursor's thing back"
        );
    }

    /// FAILS IF: the footer's twin says less than the window's foot — a
    /// section's heading, or the live output, says nothing under the pointer,
    /// and the output has no heading or never says the administrator's half
    /// is missing —, draws a program's colour on the footer's ground, where
    /// only the chrome's inks are measured, or, under a question, lights a
    /// step's box or promises a click that does nothing.
    #[test]
    fn the_twin_says_what_the_window_says_and_promises_only_what_a_click_does() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = seen_at(&t, &zone);
        let mut r = region();
        r.open.view = r.open.view.clone().with_brain(crate::update_window::Brain {
            running: true,
            ..crate::update_window::Brain::default()
        });
        r.open.view.system_readable = false;
        r.open.view.heard(crate::output::OutputLine::written(
            1,
            false,
            "",
            "\u{1b}[30mdim\u{1b}[0m and \u{1b}[31mred\u{1b}[0m",
        ));
        let rows = r.window(seen.columns, &t, 0, usize::MAX);
        let text = |x: &ListRow| x.fields.iter().map(|f| f.text.as_str()).collect::<String>();
        let heading = rows
            .iter()
            .position(|x| x.item == Some(Item::Output))
            .expect("the output's heading");
        assert!(text(&rows[heading]).contains(&t.t(module::key::SYSTEM_UPDATE_OUTPUT_LIVE)));
        assert!(
            text(&rows[heading])
                .contains(&t.t(module::key::SYSTEM_UPDATE_OUTPUT_SYSTEM_HIDDEN_SHORT)),
            "the missing half is not said"
        );
        let about = r.told(&seen, heading, 1)[0].about.clone();
        assert!(
            about.is_some_and(|a| a.contains(&t.t(module::key::SYSTEM_UPDATE_OUTPUT_LIVE_DOES))),
            "the output says nothing under the pointer"
        );
        let steps = rows
            .iter()
            .position(|x| x.item == Some(Item::Heading(crate::update_window_list::Section::Steps)))
            .expect("the steps' heading");
        let about = r.told(&seen, steps, 1)[0].about.clone();
        assert!(
            about.is_some_and(
                |a| a.contains(&t.t(crate::update_window_list::Section::Steps.about()))
            ),
            "a heading says nothing under the pointer"
        );
        let sixteen = ronoctua_shell_kit::ansi::SIXTEEN;
        assert!(
            rows.iter()
                .flat_map(|x| x.fields.iter())
                .all(|f| !sixteen.contains(&f.color)),
            "a terminal's ink on the footer's ground"
        );
        assert!(
            rows.iter()
                .flat_map(|x| x.fields.iter())
                .any(|f| f.text == "red" && f.color == ronoctua_tokens::color::STATE_ERROR),
            "a program's red is not the error's"
        );
        // UNDER A QUESTION: a step's box lights nothing, and promises nothing.
        let mut r = region();
        r.open.view.ask_metered();
        let rows = r.window(seen.columns, &t, 0, usize::MAX);
        let at = rows
            .iter()
            .position(|x| matches!(x.item, Some(Item::Step(_))))
            .expect("a step's row");
        let (from, _) = rows[at].boxed.expect("a box");
        assert_eq!(r.lit(at, Some(from), &seen), Lit::Nothing);
        let said = r
            .about_at(at, Some(from), &seen)
            .expect("the step says nothing");
        for how in [crate::door::Gesture::AtOnce, crate::door::Gesture::Selects] {
            let promise = crate::door::foot::gesture(&t, how, "");
            assert!(!said.starts_with(promise.trim_end()), "{said}");
        }
    }

    /// FAILS IF: a step's box is not lit and clicked on its own — at once,
    /// the step switched to the other state — while the rest of the row is
    /// lit whole and its first click only selects it; or a row a click
    /// takes nothing on is lit.
    #[test]
    fn a_steps_box_is_lit_and_clicked_alone() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = seen_at(&t, &zone);
        let mut r = region();
        let rows = r.window(seen.columns, &t, 0, usize::MAX);
        let off = module::steps()
            .iter()
            .find(|s| !s.on_by_default && !s.needs_root)
            .expect("a step that ships off");
        let at = rows
            .iter()
            .position(|x| x.item == Some(Item::Step(off.id)))
            .expect("the step's row");
        let (from, to) = rows[at].boxed.expect("a box");
        assert_eq!(r.lit(at, Some(from), &seen), Lit::Cells(from, to));
        assert_eq!(r.lit(at, Some(to + 3), &seen), Lit::Row);
        let heading = rows
            .iter()
            .position(|x| x.item == Some(Item::Heading(crate::update_window_list::Section::Steps)))
            .expect("the steps' heading");
        assert_eq!(r.lit(heading, Some(from), &seen), Lit::Nothing);
        // THE NAME: the first click selects, and switches nothing.
        r.clicked_at(at, Some(to + 3), &mut way());
        assert!(r.open.would_have().is_empty(), "{:?}", r.open.would_have());
        assert_eq!(r.open.view.focus(), Some(Item::Step(off.id)));
        // THE BOX: at once.
        let mut r = region();
        r.clicked_at(at, Some(from), &mut way());
        assert_eq!(
            r.open.would_have(),
            vec![format!("would have switched {} on", off.id)]
        );
    }

    /// FAILS IF: a switch, a rehearsal or the schedule's link reaches past a
    /// photograph's door, a switch with no brain answering is sent anywhere
    /// instead of said, or the schedule's link is not the footer's command
    /// that opens the brain on this task.
    #[test]
    fn the_brains_errands_reach_the_brain_or_say_why() {
        let mut r = region();
        for errand in [
            Errand::Switch {
                id: module::steps()[0].id,
                on: false,
            },
            Errand::Rehearse,
            Errand::OpenBrain,
        ] {
            carry_out(errand, &mut r.open, &mut way());
        }
        let kept = r.open.would_have();
        assert_eq!(kept.len(), 3, "{kept:?}");
        assert!(
            kept[0].ends_with(" off") && kept[1].contains("rehearsal"),
            "{kept:?}"
        );
        assert!(kept[2].contains(module::ID), "{kept:?}");
        // ON THE MACHINE: the schedule asks the footer for the brain's page
        // of this task.
        let mut open = Open::new(
            View::of(crate::update_window::Verdicts::of(&[]), Vec::new()),
            Reach::Machine,
            None,
        );
        let mut w = way();
        carry_out(Errand::OpenBrain, &mut open, &mut w);
        assert_eq!(
            w.owed.first(),
            Some(&Effect::Run(BRAIN_OPEN, Some(module::ID.to_owned())))
        );
        // NO BRAIN: the switch is said not to be possible, and nothing goes.
        let mut w = way();
        carry_out(
            Errand::Switch {
                id: module::steps()[0].id,
                on: false,
            },
            &mut open,
            &mut w,
        );
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let no_brain = t.t(module::key::SYSTEM_UPDATE_SWITCH_NO_BRAIN);
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Say(s, _) if *s == no_brain)),
            "{:?}",
            w.owed
        );
        assert!(
            !open.lingers(),
            "a switch with no brain waits for an answer"
        );
    }

    /// FAILS IF: what the brain answered a switch or a rehearsal is not
    /// said in the door — the step's new state by its name when taken, the
    /// brain's reason when refused, that nobody answered when nobody did —
    /// or the door lets go before the answer comes.
    #[test]
    fn the_brains_answer_is_said_in_the_door() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let step = &module::steps()[0];
        for (asked, answer, wanted) in [
            (
                Asked::Switch {
                    id: step.id,
                    on: false,
                },
                Answer::Taken(None),
                switched(step.id, false, &t),
            ),
            (
                Asked::Rehearse,
                Answer::Refused("brain-why-quiet".to_owned()),
                t.t("brain-why-quiet"),
            ),
            (
                Asked::Rehearse,
                Answer::Unreachable,
                t.t(crate::key::ECHO_BRAIN_UNREACHABLE),
            ),
        ] {
            let mut r = region();
            r.open.inbox = Some(Inbox::new(crate::wake::Wake::new(|| {})));
            r.open.brain_waiting = 1;
            assert!(r.lingers(), "a door waiting for the brain let it go");
            let post = r.open.post().expect("an inbox");
            assert!(post.send(Back::Brain(asked, answer)));
            let mut w = way();
            r.woken(&mut w);
            assert!(
                w.owed
                    .iter()
                    .any(|e| matches!(e, Effect::Say(s, _) if *s == wanted)),
                "{wanted}: {:?}",
                w.owed
            );
            assert!(!r.lingers());
        }
        assert!(switched(step.id, true, &t).contains(&t.t(step.name_key)));
    }
}
