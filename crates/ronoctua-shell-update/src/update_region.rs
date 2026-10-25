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
use crate::region::{Closed, Doorway, Entry, Inbox, Paging, Post, Region, Row, Seen, Taken};
use crate::update_window::{Errand, Followed, Heard, Order, Output, Press, Started, View};
use ronoctua_brain::compiled::system_update as module;

/// The region's id, as the footer's precedence ranks it.
pub const ID: &str = "update";

/// The words that open the update's window, after the binary's name.
const WINDOW_WORDS: &str = "update";

/// What the update's threads hand back to a door's loop.
#[derive(Debug)]
pub enum Back {
    /// A line of either journal.
    Line(String),
    /// What came of root's netrc, asked through the door's sudo gate.
    Netrc(binary_cache::Came),
    /// What came of a gesture of the parts' page (`crate::parts::carry`).
    Parts(crate::parts::Came),
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
            children: Vec::new(),
            inbox: wake.map(Inbox::new),
            netrc_waiting: false,
            parts_waiting: 0,
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
    /// asked of the brain, root's netrc, or a gesture of the parts' page.
    #[must_use]
    pub const fn lingers(&self) -> bool {
        self.followed.is_some() || self.netrc_waiting || self.parts_waiting > 0
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
            }
        }
        lines
    }

    /// The brain spoke: its mind is kept — the countdown's answers read it —
    /// and a run this door asked for is followed in it. Whether that asked
    /// the question on a metered way out.
    pub fn heard_the_mind(&mut self, mind: ronoctua_brain::Mind, way: &mut dyn Doorway) -> bool {
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
        // EACH DOOR'S OWN PAGE — the window's two halves, the footer's one
        // list — moved by the door itself.
        Errand::Moved | Errand::Output(_) => {}
        // STARTED, through the door the brain starts it through, and said in
        // this door's words — and FOLLOWED, when the brain took it: held for
        // a metered way out, the question comes (`heard_the_brain`) — at once
        // when the brain already holds a scheduled run that way.
        Errand::Start => match &mut open.reach {
            Reach::Machine => {
                let started = crate::update_window::begin();
                say(way, crate::update_window::said_of(started));
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
                let started = crate::update_window::begin_anyway(open.mind.as_ref(), on_battery);
                say(way, crate::update_window::said_anyway(started, on_battery));
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
        Errand::Answer(proceeding) => match &mut open.reach {
            Reach::Machine => {
                say(
                    way,
                    crate::brainlink::answer(open.mind.as_ref(), proceeding),
                );
            }
            Reach::Kept(asked) => asked.push(format!(
                "would have answered the countdown: {}",
                if proceeding { "proceed" } else { "abort" }
            )),
        },
        Errand::Cache(e) => cache_errand(e, open, way),
        Errand::Follow(e) => follow_errand(e, open, way),
        Errand::Parts(e) => parts_errand(e, open, way),
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
/// and, turning it on, their consent for GitHub — said on the door's line.
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

/// THE UPDATE IN THE FOOTER'S REGION (`LEADER s u u`): the second register,
/// the steps and the output — ONE list, which the region's scroll, bar and
/// wheel read, and only the rows on the page built.
pub struct UpdateRegion {
    open: Open,
    /// Whether the page follows the step cursor: after an arrow it does,
    /// after `C-j`, `C-k` or `G` it stays on the output the person moved to.
    on_the_step: bool,
}

impl UpdateRegion {
    /// The rows from `skip`, at most `take`.
    fn region_rows(&self, seen: &Seen<'_>, skip: usize, take: usize) -> Vec<Row> {
        self.open
            .view
            .region_rows(skip, take, seen.columns, seen.texts)
            .into_iter()
            .map(|r| Row {
                fields: r.fields,
                takes: r.target.is_some(),
            })
            .collect()
    }

    /// What row `row` takes, at the door's width.
    fn target_of(&self, row: usize, way: &dyn Doorway) -> Option<crate::update_window::Target> {
        self.open
            .view
            .region_rows(row, 1, way.columns(), way.texts())
            .first()
            .and_then(|r| r.target)
    }

    /// What the view asked: the page's own move done here — the step cursor
    /// followed, the output a page up or down or at its end — and the rest
    /// carried out as both doors carry it out.
    fn errand(&mut self, errand: Errand, way: &mut dyn Doorway) {
        if errand == Errand::Moved {
            self.on_the_step = true;
        }
        if let Errand::Output(how) = errand {
            self.on_the_step = false;
            way.page(match how {
                Output::Up => Paging::Pages(-1),
                Output::Down => Paging::Pages(1),
                Output::End => Paging::End,
            });
        }
        carry_out(errand, &mut self.open, way);
    }

    /// A key of the view — from the keyboard, the hint or the wheel.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = self.open.view.key(press);
        self.errand(errand, way);
    }

    /// The page asked for, while it is hidden.
    fn shown_on_the_step(&mut self, way: &mut dyn Doorway) {
        self.on_the_step = true;
        way.show();
        way.page(Paging::Follow);
    }
}

impl Region for UpdateRegion {
    fn id(&self) -> &'static str {
        ID
    }

    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        let len = self.row_count(seen);
        self.region_rows(seen, 0, len)
    }

    /// COUNTED, NEVER BUILT: the output keeps up to two thousand lines.
    fn row_count(&self, seen: &Seen<'_>) -> usize {
        self.open.view.region_len(seen.columns, seen.texts)
    }

    /// ONLY THE ROWS ON THE PAGE are built.
    fn rows_from(&self, seen: &Seen<'_>, skip: usize, take: usize) -> Vec<Row> {
        self.region_rows(seen, skip, take)
    }

    /// A SENTENCE WRAPPED OVER SEVERAL ROWS IS ONE CONTROL: every row of it
    /// lights with the one under the pointer, as the window lights it — a
    /// click on any of them takes the sentence.
    fn control_of(&self, row: usize, seen: &Seen<'_>) -> usize {
        let view = &self.open.view;
        let target = |n: usize| {
            view.region_rows(n, 1, seen.columns, seen.texts)
                .first()
                .and_then(|r| r.target)
        };
        let Some(mine) = target(row) else {
            return row;
        };
        let mut first = row;
        while first > 0 && target(first - 1) == Some(mine) {
            first -= 1;
        }
        first
    }

    /// The table its window's header teaches too.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.open.view.hint(seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        None
    }

    /// THE STEP'S ROW — after an arrow. After `C-j`, `C-k` or `G` the page is
    /// on the output the person moved it to, and following the step would
    /// take it back.
    fn cursor_in(&self, seen: &Seen<'_>) -> Option<usize> {
        self.on_the_step
            .then(|| self.open.view.region_cursor(seen.columns, seen.texts))
    }

    /// The step cursor, the output, and the three things one does here —
    /// through the reader its window calls too.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        // THE FILE PICKER, while it is up over the parts' page, reads its
        // keys before the view does.
        if self.open.view.picker_takes(key) {
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

    /// THE CURSOR GOES TO THE STEP, and what it is is said — the window's
    /// click, through the same `View::clicked`.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        if let Some(target) = self.target_of(row, way) {
            let errand = self.open.view.clicked(target);
            self.errand(errand, way);
        }
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

    /// A line of either journal, or root's netrc. DATA DOES NOT MOVE THE
    /// PAGE, with the one exception a log has: a page the person took to the
    /// output's END stays on the end as the end moves — the window's "zero
    /// follows". A page on the steps stays on the steps.
    fn woken(&mut self, way: &mut dyn Doorway) {
        let before = self.open.view.region_len(way.columns(), way.texts());
        if self.open.woken(way) {
            let after = self.open.view.region_len(way.columns(), way.texts());
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
        let moved = self.open.view.reread_if_moved();
        if moved {
            way.repaint();
        }
        moved
    }

    /// THE TWO JOURNALS, each on a thread of its own, for as long as the page
    /// is shown — a photograph's footer follows nothing.
    fn shown(&mut self, way: &mut dyn Doorway) {
        self.on_the_step = true;
        if !way.photographing() {
            self.open.follow_the_journals();
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
        Some(Order::Open | Order::Cache(_) | Order::Follow | Order::Parts(_)) => Closed::Opens,
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
        let view = View::of(
            module::steps(),
            crate::update_window::Verdicts::of(&[]),
            Vec::new(),
        );
        UpdateRegion {
            open: Open::new(view, Reach::Kept(Vec::new()), None),
            on_the_step: true,
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
            View::of(
                module::steps(),
                crate::update_window::Verdicts::of(&[]),
                Vec::new(),
            ),
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
    /// reach the output, or a page the person took to the output's end is
    /// not kept there as the end moves — or one on the steps is moved.
    #[test]
    fn the_journals_reach_the_output_and_the_end_follows() {
        let mut r = region();
        r.open.inbox = Some(Inbox::new(crate::wake::Wake::new(|| {})));
        let post = r.open.post().expect("an inbox");
        assert!(post.send(Back::Line("one".to_owned())));
        assert!(post.send(Back::Line("two".to_owned())));
        let mut w = way();
        r.woken(&mut w);
        assert_eq!(r.open.view.scrollback().amount(), 2);
        assert!(
            !w.owed.iter().any(|e| matches!(e, Effect::Page(_))),
            "a page on the steps moved with the output: {:?}",
            w.owed
        );
        let _ = r.key(ronoctua_keymap::Key::parse("G").expect("a key"), &mut way());
        assert!(post.send(Back::Line("three".to_owned())));
        let mut w = way();
        r.woken(&mut w);
        assert!(
            w.owed.contains(&Effect::Page(Paging::Tail(1))),
            "the end did not follow: {:?}",
            w.owed
        );
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
        let view = View::of(
            module::steps(),
            crate::update_window::Verdicts::of(&[]),
            Vec::new(),
        )
        .with_cache(readings);
        let mut r = UpdateRegion {
            open: Open::new(view, Reach::Kept(Vec::new()), None),
            on_the_step: true,
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
                        r.open
                            .view
                            .region_rows(*n, 1, 100, &t)
                            .first()
                            .and_then(|x| x.target)
                            == Some(crate::update_window::Target::Page(remove))
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
                        r.open
                            .view
                            .region_rows(*n, 1, 100, &t)
                            .first()
                            .is_some_and(|x| x.fields.iter().any(|f| !f.text.is_empty()))
                            && !rows[*n].takes
                    })
                    .count();
                assert!(page_rows >= 3, "the page's rows lit under the question");
            }
        }
    }
}
