//! THE PLAYER IN THE FOOTER'S REGION (`LEADER p p`) — the player's view
//! (`crate::player`) as a `region::Region` of the player's own crate, and
//! what the footer keeps of the player for the whole session
//! (`region::Resident`).
//!
//! # What this door is
//!
//! The view's rows beside the PANEL — the picture, or its title card, with
//! the transport and the time bar at its foot (`crate::player_media`), laid
//! out at each frame and written down for the pointer — or the picture alone
//! on the region's stage (`o`, up to `expansion.playerStagePct`), a deck and
//! a schematic and a globe among the rows, covers over them. What a key, a
//! click, a press on the panel or a drag on a deck does is the view's
//! (`player::View::key`, `clicked_px`, `player_rows::slid`), carried out
//! through the ONE function every door calls (`player::carry_out`); what is
//! left is what only this door can do — its stage, the column and the window
//! raised from here, what plays on every screen it knows.
//!
//! # What outlives the region
//!
//! The player's last state and the queue's outline, which a command run with
//! the view closed reads (`player-toggle` is the ambient sounds' while they
//! are all that plays); a pairing between its two questions; the picture
//! folded away; the column this footer raised last; and how an ask told off
//! the loop ended — the resident ([`Kept`]), made when the footer starts.
//! While the region is open it hears the player's watch and its threads; the
//! resident hears them while it is closed.

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};

use ronoctua_player::queue::Outline;
use ronoctua_player::wire::{Ask, State};
use ronoctua_tokens as tokens;

use crate::player::{Errand, Heard, Hearing, Press, Show, Told, Unseen, View};
use crate::region::{
    Closed, Doorway, Entry, Laid, Region, RegionEasel, Residence, Residing, Row, Seen, Shape,
    Taken, Under,
};

/// The region's id, as the footer ranks it.
pub const ID: &str = "player";

/// The region, as the binary registers it.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// What the footer keeps of the player for the session, as the binary
/// registers it.
pub const RESIDING: Residing = Residing { id: ID, make };

/// What the player's threads leave for the footer's loop, taken when its
/// wake rings.
#[derive(Default)]
struct Mail {
    /// How an ask told off the loop ended (`player::tell_then`).
    told: Vec<Told>,
    /// How a pairing ended (`player_machines::pair_then`).
    paired: Vec<crate::player_machines::Paired>,
    /// The globe's pictures (`globe::Stage`).
    beats: Vec<crate::globe::Beat>,
    /// A read the view started off the loop is done — the folder beside the
    /// film (`View::waking`).
    view: bool,
}

/// WHAT THE FOOTER KEEPS OF THE PLAYER, the region open or closed.
pub struct Kept {
    /// The player's last state — `None` while nothing answers its socket.
    state: Option<State>,
    /// The queue's outline, as the player's watch last said it — handed to
    /// the view when it opens.
    outline: Option<Box<Outline>>,
    /// A pairing's name, between its two questions — the footer's and not
    /// the view's: `player-pair` (by its name) asks it with no view open.
    pairing: crate::player_machines::Pairing,
    /// THE PICTURE FOLDED AWAY in the panel beside the rows (`z`): kept from
    /// one opening to the next, and never written.
    folded: bool,
    /// The column this footer raised last, until it has taken its slot
    /// ([`column_coming`]).
    column_child: Option<crate::foreign::Waited>,
    /// What the player's threads left.
    mail: Arc<Mutex<Mail>>,
}

impl Kept {
    /// Where an ask told off the loop says how it ended: this door's mail,
    /// and its wake.
    fn told(&self, way: &dyn Doorway) -> crate::player::ToldSender {
        let mail = Arc::clone(&self.mail);
        let wake = way.wake();
        crate::player::ToldSender::new(move |answer| {
            mail.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .told
                .push(answer);
            wake.call();
        })
    }

    /// What the player's watch heard, kept: a state, an outline, its
    /// farewell.
    fn hear(&mut self, heard: &ronoctua_player::client::Heard) {
        match crate::player::hearing(heard.clone()) {
            Hearing::Shows(s) => {
                if s.is_none() {
                    self.outline = None;
                }
                self.state = s.map(|s| *s);
            }
            Hearing::Outline(o) => self.outline = Some(o),
            Hearing::Closes => {
                self.outline = None;
                self.state = None;
            }
        }
    }

    /// What the threads left, taken.
    fn take_the_mail(&self) -> Mail {
        std::mem::take(&mut *self.mail.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl crate::region::Resident for Kept {
    /// Only the player's watch.
    fn hears(&self, news: &dyn Any) -> bool {
        news.is::<ronoctua_player::client::Heard>()
    }

    fn heard(&mut self, news: &dyn Any, _: &mut dyn Doorway) {
        if let Some(heard) = news.downcast_ref::<ronoctua_player::client::Heard>() {
            self.hear(heard);
        }
    }

    /// A question asked with the view closed was let go: a pairing between
    /// its two questions goes with it, as it goes with the view open
    /// ([`Region::unanswered`]).
    fn let_go(&mut self, _: &str) {
        self.pairing.let_go();
    }

    /// WITH THE VIEW CLOSED, an answer off the loop is only said — a draft
    /// opened in the editor — and a pairing says how it ended.
    fn woken(&mut self, way: &mut dyn Doorway) {
        let mail = self.take_the_mail();
        let older = self.state.as_ref().is_some_and(crate::player::older);
        for answer in mail.told {
            if let Told::Draft(path) = &answer {
                open_editor(path, way);
                way.repaint();
                continue;
            }
            if let Heard::Said(Some(said)) =
                crate::player::heard(false, &answer, older, way.texts())
            {
                way.say(said, crate::echo::Echo::Normal);
            }
        }
        for p in mail.paired {
            if let Some(said) = crate::player_machines::paired_said(&p, way.texts()) {
                way.say(said, crate::echo::Echo::Normal);
            }
        }
    }
}

/// The resident, made when the footer starts.
fn make() -> Residence {
    Residence::of(Kept {
        state: None,
        outline: None,
        pairing: crate::player_machines::Pairing::default(),
        folded: false,
        column_child: None,
        mail: Arc::default(),
    })
}

/// The resident behind a doorway — the footer's always has one; a test's
/// is made here.
fn kept_of(way: &dyn Doorway) -> Rc<RefCell<Kept>> {
    way.residence()
        .and_then(|k| k.as_a::<Kept>())
        .or_else(|| make().as_a::<Kept>())
        .expect("a resident of the player's is a `Kept`")
}

/// A FILE OPENED IN THE EDITOR — the draft the daemon wrote — or, with no
/// editor named, said so: a file that opens nothing has to say why.
fn open_editor(file: &std::path::Path, way: &mut dyn Doorway) {
    let client = std::env::var("RONOCTUA_EMACSCLIENT").ok();
    match crate::launch::editor_command(client.as_deref(), file) {
        Some((program, args, cwd)) => {
            tracing::info!(file = %file.display(), "opening in emacs");
            way.launch(crate::door::Launching::Program { program, args, cwd });
        }
        None => {
            let said = way.texts().t(crate::key::FILES_NO_EDITOR);
            way.say(said, crate::echo::Echo::Normal);
        }
    }
}

/// The commands a key of the view runs — `true` while the view is open.
const OF_THE_OPEN_VIEW: &[&str] = &[
    "player-up",
    "player-down",
    "player-less",
    "player-more",
    "player-enter",
    "player-remove",
    "player-add",
    "player-only",
    "player-maximize",
    "player-play-now",
    "player-clear",
    "player-quieter",
    "player-louder",
    "player-pip",
    "player-mute",
    "player-stop",
    "player-swap",
    "player-fold",
    "player-also",
    "player-lane-shown",
    "player-earlier",
    "player-later",
    // A DIRECTORY'S PAGE'S KEYS: its search, a favourite, its globe.
    "player-search",
    "player-favourite",
    "player-nearer",
    "player-further",
    "player-world",
    // THE VIEW'S NEW KEYS by name: the seeks, the picture folded, the
    // queue saved again, the countries' order, the lyrics translated.
    "player-rewind",
    "player-forward",
    "player-picture-fold",
    "player-save-queue-again",
    "player-countries-order",
    "player-lyrics-translate",
    // A LISTING OF THE CATALOGUE'S KEYS.
    "player-listing-earlier",
    "player-listing-later",
    "player-listing-first",
    "player-listing-last",
    "player-listing-full",
    "player-listing-layout",
    "player-listing-source",
    "player-listing-letter",
    "player-listing-order",
];

/// The commands that open a PAGE of the view — the view opened on it, or
/// turned to it when it is already open.
const PAGES: &[&str] = &[
    "player-browse",
    "player-machines",
    "player-tracks",
    // AND THE SUBTITLES OVER THE PICTURE, AND THE AMBIENT SOUNDS.
    "player-subtitles",
    "player-ambience",
    // AND THE TIMELINE: every lane on one clock.
    "player-timeline",
    // AND HOW THE PICTURES SHARE A SCREEN.
    "player-layout",
    // AND THE METADATA: the home's last section, the cursor on it.
    "player-metadata",
    // AND WHAT THE INTERNET SAYS OF WHAT PLAYS: the suggestions for its
    // tags, subtitles and lyrics — and the page of what asks it.
    "player-suggestions",
    "player-find-subtitles",
    "player-find-lyrics",
    "player-internet",
    // AND THE DIRECTORIES: the world's radio and TV, the themes.
    "player-radio",
    "player-tv",
    "player-channels",
    // THE WEBSITE PAGE — `LEADER p h`.
    "player-website",
    // THE RANKING — `LEADER p k`.
    "player-ranking",
];

/// The view's own questions — a new name, when a lane begins and ends, its
/// title — and, typed from no question, a name for what plays or saving.
const VIEW_QUESTIONS: &[&str] = &[
    "player-rename",
    "player-save-set",
    "player-save-queue",
    "player-lane-start",
    "player-lane-until",
    "player-lane-title",
];

/// Whether `id` is the answer to one of the questions the view's pages ask
/// — a field of the file the Metadata shows, the lookups', the directories',
/// the website's, the catalogue's: the view's to answer when it is open.
fn a_page_asks(id: &str) -> bool {
    crate::player_metadata::asks(id)
        || crate::player_find::asks(id)
        || crate::player_directory::asks(id)
        || crate::player_website::asks(id)
        || crate::player_seam::asks(id)
}

/// What a command does with the region closed: a page opens it, and its
/// questions; what the transport, the doors, the questions of the player
/// and every screen do needs no view (the resident's state is enough); a key
/// of the open view is no command of a closed one.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    if command == "player-open" || PAGES.contains(&command) {
        return Closed::Opens;
    }
    if VIEW_QUESTIONS.contains(&command) {
        if argument.is_none() {
            return Closed::Opens;
        }
    } else if OF_THE_OPEN_VIEW.contains(&command)
        && !matches!(command, "player-stop" | "player-rewind" | "player-forward")
    {
        return Closed::NotMine;
    }
    let kept = kept_of(way);
    let mut door = Door {
        kept: &kept,
        open: None,
    };
    if door.command(command, argument, way) {
        Closed::Taken
    } else {
        Closed::NotMine
    }
}

/// The region, opened for `command`: the view in the footer, waking this
/// door, with the outline and the screens it has and settled on the state.
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    Box::new(opened(way))
}

/// [`open`], unboxed.
fn opened(way: &mut dyn Doorway) -> Player {
    let kept = kept_of(way);
    let view_wake = {
        let mail = Arc::clone(&kept.borrow().mail);
        let wake = way.wake();
        crate::wake::Wake::new(move || {
            mail.lock().unwrap_or_else(PoisonError::into_inner).view = true;
            wake.call();
        })
    };
    let (known, here) = known_of(way);
    let mut view = View::fresh().in_the_footer();
    // A DOOR WITH NO LOOP — a photograph's footer, a test's — reads in
    // place: a wake there would wake nothing, and a read off the loop would
    // never land (`View::waking`).
    let looping = way.has_a_loop();
    if looping {
        view = view.waking(view_wake);
    }
    {
        let k = kept.borrow();
        view.outline_arrived(k.outline.clone());
        view.screens_are(known.clone(), here.clone());
    }
    let mut me = Player {
        kept,
        view,
        only: false,
        sinks: None,
        pictures: Vec::new(),
        drawn: crate::player_media::Drawn::default(),
        dragged: None,
        globe_drawn: RefCell::new(Vec::new()),
        globe_stage: None,
        controls_placed: RefCell::new(Vec::new()),
        control_under: None,
        globe: false,
        known,
        here,
        screens: Vec::new(),
        wants: RefCell::new(Vec::new()),
        globe_largest: RefCell::new(None),
        frame_wake: way.frame_wake(),
        globe_wake: way.wake(),
        looping,
        screen_columns: way.columns(),
    };
    // THE FIRST SETTLE AT THE SCREEN'S WIDTH: the panel beside the rows is
    // the open view's, and there was none before it opened.
    let columns = way.columns();
    {
        let k = me.kept.borrow();
        me.view.settle_in(k.state.as_ref(), columns, way.texts());
    }
    // AND THE PANEL STANDS BESIDE THE ROWS before the command that opened
    // the region reaches them — what the footer's first frame told the view
    // before, when a page's command opened it: a directory's globe goes in
    // the picture's place, and a page pushed now places its cursor among
    // the rows the panel leaves (with the globe still among them, the
    // cursor fell rows below where the frame drew it).
    me.room_follows(way.texts());
    me
}

/// The screens a door knows by name, as the player's view takes them.
fn known_of(way: &dyn Doorway) -> (Vec<crate::player_lane::Known>, Option<String>) {
    let (known, here) = way.screens();
    (
        known
            .into_iter()
            .map(|(connector, description)| crate::player_lane::Known {
                connector,
                description,
            })
            .collect(),
        here,
    )
}

/// Where the player's globe was drawn on each screen this frame: the
/// screen's connector, the picture's box and its size in pixels.
type GlobesDrawn = Vec<(Option<String>, ronoctua_render::image::Area, (u32, u32))>;

/// A press on the player's globe, held: where it was pressed, where the hand
/// was last, and whether it has become a drag.
type GlobeHeld = ((f64, f64), (f64, f64), bool);

/// WHAT ONE POINTER HOLDS over the player's region — kept with the pointer
/// by the footer (`region::Region::hovered`).
#[derive(Debug, Default)]
struct Hand {
    /// Which ROW of the view has its bar held — the position's or the
    /// volume's (`player_rows::slider_at`): the drag keeps turning it.
    dragging: Option<usize>,
    /// The clicks on the picture, one after the other: a second in time is
    /// a double click.
    picture_clicked: crate::player_media::PictureClicks,
    /// The button of the panel under the pointer: lit, and said on the
    /// message line.
    button_under: Option<&'static str>,
    /// The panel's time bar is held.
    dragging_track: bool,
    /// What a press on a line of a DECK holds until it is let go, the
    /// deck's first cell in the row and the row.
    deck: Option<(crate::player_timeline::Grip, f64, usize)>,
    /// The control of a deck under the pointer.
    deck_under: Option<(usize, usize, crate::player_timeline::Hit)>,
    /// A press on the GLOBE held.
    globe: Option<GlobeHeld>,
}

/// The pointer's hand, as this region keeps it — a hand of another region
/// is none of this one's.
fn hand_of(hand: &mut Option<Box<dyn Any>>) -> &mut Hand {
    if !hand.as_ref().is_some_and(|h| h.is::<Hand>()) {
        *hand = Some(Box::new(Hand::default()));
    }
    hand.as_mut()
        .and_then(|h| h.downcast_mut::<Hand>())
        .expect("a hand was just put")
}

/// THE PLAYER'S REGION.
pub struct Player {
    /// What the footer keeps of the player, shared with the resident.
    kept: Rc<RefCell<Kept>>,
    /// The view.
    view: View,
    /// Only the picture, over the whole region — the window's `Only`, in
    /// this door (`o`, and `Esc` back): the region is the film's stage.
    only: bool,
    /// The sound outputs, held while a row of them is on screen in the view
    /// (`player::SinksHeld`).
    sinks: Option<crate::player::SinksHeld>,
    /// The pictures the view shows beside its rows, a lane each — every lane
    /// routed to one of the footer's screens — and the threads that bring
    /// their frames: only while the view is shown and something with a
    /// picture plays.
    pictures: Vec<crate::player_picture::Picture>,
    /// The panel as the LAST FRAME laid it out — the picture's lines, the
    /// transport's buttons and the time bar: a press, a drag and a hover
    /// answer what was DRAWN.
    drawn: crate::player_media::Drawn,
    /// A band of a deck being DRAGGED — where its ghost would land — until
    /// it is let go, or `Esc` calls it off.
    dragged: Option<crate::player_timeline::Held>,
    /// WHERE THE GLOBE was drawn this frame ON EACH SCREEN: what a pointer on
    /// it is read back through, on the screen the pointer is on. A cell,
    /// written by the paint.
    globe_drawn: RefCell<GlobesDrawn>,
    /// The globe's drawing thread, raised the first time a page shows one.
    globe_stage: Option<crate::globe::Stage>,
    /// Where the frame PLACED the controls of the rows: the line of the page,
    /// the cell, its width and its sentence's id — written by the paint, read
    /// by the pointer.
    controls_placed: RefCell<Vec<(usize, u16, u16, &'static str)>>,
    /// The CONTROL of a row under the pointer — a title's glyph — by the
    /// command whose sentence the message line says.
    control_under: Option<&'static str>,
    /// WHETHER A WORLD PAGE'S GLOBE stands in the panel, in the picture's
    /// place: no picture there to fold, and the fold kept for when one comes
    /// back.
    globe: bool,
    /// The screens the footer knows by name…
    known: Vec<crate::player_lane::Known>,
    /// …and the one the pointer is on.
    here: Option<String>,
    /// The screens the frame is drawn on, as it was laid out.
    screens: Vec<crate::region::Screen>,
    /// THE SIZE EACH PICTURE IS ASKED FOR, one per lane: the largest any
    /// screen that shows it draws it at, said once after every screen.
    wants: RefCell<Vec<(ronoctua_player::own::LaneId, (u32, u32))>>,
    /// The globe's largest box this frame, measured once after every screen.
    globe_largest: RefCell<Option<(ronoctua_render::image::Area, u32)>>,
    /// The wake a picture's thread rings: only a paint is owed.
    frame_wake: crate::wake::Wake,
    /// The wake the globe's thread rings.
    globe_wake: crate::wake::Wake,
    /// The door runs a loop its threads wake (`Doorway::has_a_loop`): with
    /// none, no thread is raised for a picture or the globe — nothing would
    /// hear what they bring.
    looping: bool,
    /// The footer's columns, as its last call said them.
    screen_columns: u16,
}

/// THE PLAYER'S DOOR IN THE FOOTER — what a command, an errand and an answer
/// do here, the view open (`open`) or closed: one set of answers for both.
struct Door<'a> {
    /// What the footer keeps.
    kept: &'a Rc<RefCell<Kept>>,
    /// The region, when it is open.
    open: Option<&'a mut Player>,
}

impl Door<'_> {
    /// The view's commands — the registry's, the leader's, `LEADER :`'s, a
    /// window's order, a question's answer: `false` when `id` is none of
    /// them.
    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> bool {
        match id {
            "player-open" => {
                // A SECOND OPENING follows the cursor, the view as it is.
                way.follow();
                way.repaint();
            }
            id if PAGES.contains(&id) => self.page(id, way),
            // WHAT PLAYS ON EVERY SCREEN: the errand every door carries out
            // the same way — no view needed to open it.
            "player-every-screen" => self.errand(Errand::EveryScreen, way),
            // THE WINDOW, and THE COLUMN, which toggles: one that is up is
            // closed, else one is raised — and with `raise` only raised.
            "player-window" | "player-column" => self.door(id, argument, way),
            // THE PLAYER CLOSED EVERYWHERE: the daemon, and every door.
            "player-quit" => self.quit(way),
            // THE VIEW'S OWN QUESTIONS, answered — and, typed from no
            // question, a name for what plays or saving: the view asked.
            id if VIEW_QUESTIONS.contains(&id) => match argument {
                Some(said) => self.answered(id, said, way),
                None => {
                    if let Some(press) = crate::player::press_of(id) {
                        self.press(press, way);
                    }
                }
            },
            // A FIELD OF THE FILE THE METADATA SHOWS, or a picture for its
            // cover, typed on the minibuffer: the view's to answer — and the
            // lookups', the directories', the website's, the catalogue's.
            id if a_page_asks(id) => {
                if let Some(said) = argument {
                    self.answered(id, said, way);
                }
            }
            // A KEY OF THE OPEN VIEW, by name.
            id if self.open.is_some()
                && (OF_THE_OPEN_VIEW.contains(&id)
                    || crate::player_seam::RankingKey::of_command(id).is_some()) =>
            {
                if let Some(press) = crate::player::press_of(id) {
                    self.press(press, way);
                }
            }
            // THE TRANSPORT, from anywhere: told to the player straight away.
            "player-toggle" | "player-next" | "player-previous" | "player-stop"
            | "player-shuffle" | "player-repeat" => {
                let ask = match id {
                    // THE AMBIENT SOUNDS' while they are all that plays — the
                    // modeline's glyph runs this too (`player::toggle_ask`).
                    "player-toggle" => crate::player::toggle_ask(self.kept.borrow().state.as_ref()),
                    "player-next" => Ask::Next,
                    "player-previous" => Ask::Previous,
                    "player-stop" => Ask::Stop,
                    "player-shuffle" => Ask::Shuffle,
                    _ => Ask::Repeat,
                };
                self.errand(Errand::Tell(ask), way);
            }
            // EVERY AMBIENT SOUND OFF, from anywhere — by name only.
            "player-ambience-silence" => {
                let said = way.texts().t(crate::key::PLAYER_AMBIENCE_SILENCED);
                self.errand(
                    Errand::TellSaying(ronoctua_player::wire::Ask::AmbientSilence, said),
                    way,
                );
            }
            // THE PLAYER'S QUESTIONS: with nothing typed this opens the
            // question — on the minibuffer, and an open view stays open under
            // it; with an argument it IS the answer.
            "player-play-file" | "player-enqueue" | "player-together" | "player-pair"
            | "player-pair-code" => match argument {
                Some(said) => self.answered(id, said, way),
                None => {
                    if let Some(q) = crate::player_question::question(id) {
                        ask(q, way);
                    }
                }
            },
            // THE SEEKS WITH NO VIEW OPEN are the transport's.
            "player-rewind" | "player-forward" => {
                let errand = crate::player_transport::button_errand(
                    id,
                    self.kept.borrow().state.as_ref(),
                    way.texts(),
                );
                self.errand(errand, way);
            }
            _ => return false,
        }
        true
    }

    /// A page of the view, by its command — the view opened on it (the
    /// region opened for it), or turned to it.
    fn page(&mut self, id: &str, way: &mut dyn Doorway) {
        if let Some(press) = crate::player::press_of(id) {
            self.press(press, way);
        }
    }

    /// A key of the view.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let Some(open) = self.open.as_deref_mut() else {
            return;
        };
        open.press(press, way);
    }

    /// What was typed for one of the player's questions — the view's to
    /// answer while it is open, else the one function every question
    /// reaches (`player_question::answered_saying`).
    fn answered(&mut self, id: &str, said: &str, way: &mut dyn Doorway) {
        let errand = match self.open.as_deref_mut() {
            Some(open) => {
                let columns = open.player_columns();
                let mut kept = self.kept.borrow_mut();
                let Kept { state, pairing, .. } = &mut *kept;
                let errand =
                    open.view
                        .answered(id, said, pairing, state.as_ref(), columns, way.texts());
                drop(kept);
                way.resized();
                way.follow();
                errand
            }
            None => crate::player_question::answered_saying(
                id,
                said,
                &mut self.kept.borrow_mut().pairing,
                way.texts(),
            ),
        };
        self.errand(errand, way);
    }

    /// One of the player's doors, opened from the footer: the window, or the
    /// column — which toggles, and which `player::RAISE` raises instead.
    fn door(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) {
        match (id, argument) {
            ("player-window", None) => way.open_window(vec!["player".to_owned()]),
            // THE COLUMN OF THIS WORKSPACE: closed when it is here, and
            // brought here from wherever it is otherwise.
            // A PHOTOGRAPH'S DOOR raises no column and ends none
            // (`Doorway::photographing`).
            ("player-column", None | Some(crate::player::RAISE)) if way.photographing() => {}
            ("player-column", None) => {
                let here = way.workspace();
                if crate::right_column::toggle(crate::right_column::Tenant::Player, here.as_deref())
                    == crate::right_column::Asked::Raise
                {
                    self.raise_the_column(here.as_deref(), way);
                }
            }
            // RAISED, NEVER CLOSED — what a window swapping itself for the
            // column asks: one already here stays, and one elsewhere is
            // brought here. Asked twice, it is asked once — the second time
            // too while the first is still coming up.
            ("player-column", Some(crate::player::RAISE)) => {
                let here = way.workspace();
                if !crate::right_column::is_here(
                    crate::right_column::Tenant::Player,
                    here.as_deref(),
                ) && !column_coming(&mut self.kept.borrow_mut())
                {
                    self.raise_the_column(here.as_deref(), way);
                }
            }
            // AN ARGUMENT THAT NAMES NOTHING is refused, never read as the
            // toggle — and refused IN WORDS.
            (id, Some(argument)) => {
                tracing::warn!(
                    id,
                    argument,
                    "a door of the player with an argument it does not take"
                );
                let said = way.texts().t_with(
                    crate::key::ECHO_ARGUMENT_REFUSED,
                    &[("command", id), ("argument", argument)],
                );
                way.say(said, crate::echo::Echo::Normal);
                way.repaint();
            }
            (other, None) => {
                tracing::warn!(id = other, "not a door of the player");
            }
        }
    }

    /// The player's column, started on workspace `on` — the one already up,
    /// wherever it is, ends as this one starts (`right_column::take`), so it
    /// is also how the column is brought from another workspace.
    fn raise_the_column(&mut self, on: Option<&str>, way: &mut dyn Doorway) {
        let _ = ronoctua_player::client::ensure();
        let words = crate::column::Opening {
            tenant: crate::right_column::Tenant::Player,
            on: on.map(ToOwned::to_owned),
            size: None,
        }
        .args();
        let started = std::env::current_exe().and_then(|me| {
            crate::watchdog::unshared(&mut std::process::Command::new(me))
                .args(words)
                .stdin(std::process::Stdio::null())
                .spawn()
        });
        match started {
            Ok(child) => {
                tracing::info!(pid = child.id(), "raising the player's column");
                self.kept.borrow_mut().column_child = Some(crate::foreign::waited(child));
            }
            Err(e) => {
                tracing::warn!(error = %e, "could not raise the player's column");
                let said = way.texts().t_with(
                    crate::key::PLAYER_COLUMN_REFUSED,
                    &[("why", &e.to_string())],
                );
                way.say(said, crate::echo::Echo::Normal);
            }
        }
    }

    /// CLOSES THE PLAYER EVERYWHERE: the daemon told only if it runs, never
    /// started to be told to go; this view closes; and the doors that are
    /// processes are ended whether or not a daemon was there to say it.
    fn quit(&mut self, way: &mut dyn Doorway) {
        // A PHOTOGRAPH'S DOOR tells no daemon and ends no door
        // (`Doorway::photographing`).
        let photographing = way.photographing();
        if !photographing {
            crate::player::tell_if_running(&ronoctua_player::wire::Ask::Quit);
        }
        if self.open.is_some() {
            way.close();
        }
        if !photographing {
            crate::player::end_the_doors();
        }
        way.repaint();
    }

    /// What the view asked of this door: what every door does the same
    /// (`player::carry_out`), and what only this one can.
    fn errand(&mut self, errand: Errand, way: &mut dyn Doorway) {
        // ON A PHOTOGRAPH'S DOORWAY nothing of the machine is reached
        // (`Doorway::photographing`: the footer's camera, a test): what would
        // have reached the daemon or the person's preferences is kept, and
        // said as a photograph's window says it (`player_window::Outbox`).
        let carried = if way.photographing() {
            crate::player_window::Outbox::default().carry_out(errand, way.texts())
        } else {
            let told = self.kept.borrow().told(&*way);
            crate::player::carry_out(errand, way.texts(), Some(&told))
        };
        // WHAT THE ERRAND WROTE — a switch, a consent — read back from its
        // file: a write that was refused is not shown as done.
        if let Some(open) = self.open.as_deref_mut() {
            open.view.errand_carried();
        }
        match carried {
            Ok(Some(said)) => way.say(said, crate::echo::Echo::Normal),
            Ok(None) => {}
            Err(Errand::Close) => {
                if self.open.is_some() {
                    way.close();
                }
            }
            Err(Errand::Ask(q)) => {
                ask(q, way);
                return;
            }
            // THE SWAP, from the footer: the view for the window — the view
            // closes as a door's row closes it, and the window opens.
            Err(Errand::Swap) => {
                if self.open.is_some() {
                    way.close();
                }
                self.door("player-window", None, way);
            }
            // THE PLAYER CLOSED EVERYWHERE, answered yes on the view's row.
            Err(Errand::QuitEverywhere) => self.quit(way),
            // THE CLIPBOARD, for the text being edited — off the loop.
            Err(Errand::Paste) => way.paste(),
            // A VIDEO OF THE RANKING IN THE CHROME KIOSK — this footer's own
            // `kiosk-open`.
            Err(Errand::Kiosk(url)) => way.run("kiosk-open", Some(url)),
            // THE PICTURE FOLDED AWAY beside the rows, or back: the view
            // told, the picture's thread following, and the region sized
            // again.
            Err(Errand::FoldPicture) => {
                {
                    let mut kept = self.kept.borrow_mut();
                    kept.folded = !kept.folded;
                }
                if let Some(open) = self.open.as_deref_mut() {
                    open.room_follows(way.texts());
                    open.pictures_follow(false);
                }
                way.resized();
                way.follow();
            }
            // WHAT PLAYS ON EVERY SCREEN something of it goes to — the
            // footer's screens, by name — or the windows up closed.
            // A PHOTOGRAPH'S DOOR opens no window (`Doorway::photographing`).
            Err(Errand::EveryScreen) if way.photographing() => {}
            Err(Errand::EveryScreen) => {
                let (known, _) = way.screens();
                let known: Vec<String> = known.into_iter().map(|(c, _)| c).collect();
                let routed: Vec<String> =
                    self.kept
                        .borrow()
                        .state
                        .as_ref()
                        .map_or_else(Vec::new, |s| {
                            crate::player_layout::screens_with_pictures(s, &known)
                                .into_iter()
                                .map(ToOwned::to_owned)
                                .collect()
                        });
                let said = match crate::player_stage::everywhere(known.len(), &routed) {
                    Ok(done) => crate::player_stage::everywhere_said(&done, way.texts()),
                    Err(e) => {
                        tracing::warn!(error = %e, "the windows for every screen could not be opened");
                        way.texts().t_with(
                            crate::key::PLAYER_EVERY_SCREEN_FAILED,
                            &[("why", &e.to_string())],
                        )
                    }
                };
                way.say(said, crate::echo::Echo::Normal);
            }
            // ONLY THE PICTURE is this door's own stage: the region is the
            // panel, up to `expansion.playerStagePct` of the screen, and `o`
            // or `Esc` gives the rows back. With nothing loaded there is
            // nothing to show alone, and it is the window's stage.
            Err(Errand::Show(Show::Only))
                if self.open.as_deref().is_some_and(|o| o.loaded() || o.only) =>
            {
                if let Some(open) = self.open.as_deref_mut() {
                    open.only = !open.only;
                    open.room_follows(way.texts());
                }
                way.resized();
                way.follow();
            }
            // THE WHOLE SCREEN AND THE CORNER are the WINDOW's — a strip of
            // the screen can neither fill it nor float. A SWITCH: pressed
            // again, the window it opened goes.
            Err(Errand::Show(_)) if way.photographing() => {}
            Err(Errand::Show(show)) => {
                if let Err(e) = crate::player_stage::toggle(show) {
                    tracing::warn!(error = %e, "could not open the player's window for its stage");
                }
            }
            // THE PAIRING, off the loop: the player answers only once the
            // other machine has, and this footer's one loop does not wait.
            Err(Errand::Pair { .. }) if way.photographing() => {}
            Err(Errand::Pair { to, code }) => {
                let mail = Arc::clone(&self.kept.borrow().mail);
                let wake = way.wake();
                crate::player_machines::pair_then(to, code, move |p| {
                    mail.lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .paired
                        .push(p);
                    wake.call();
                });
            }
            Err(_) => {}
        }
        way.repaint();
    }
}

/// CARRIES AN ERRAND OUT in the footer's door: what every door does the same
/// (`player::carry_out`) — or, on a photograph's doorway, which reaches
/// nothing of the machine (`Doorway::photographing`: the footer's camera, a
/// test), what a photograph's window keeps of it, said as that window says
/// it (`player_window::Outbox`). What stays in the door is handed back
/// either way.
fn carried(kept: &Kept, errand: Errand, way: &dyn Doorway) -> Result<Option<String>, Errand> {
    if way.photographing() {
        return crate::player_window::Outbox::default().carry_out(errand, way.texts());
    }
    let told = kept.told(way);
    crate::player::carry_out(errand, way.texts(), Some(&told))
}

/// Asks one of the player's questions on the door's line — masked, and left
/// out of the log, when it is a pairing's code — with the view, when it is
/// open, left open under it.
fn ask(q: crate::player_question::Question, way: &mut dyn Doorway) {
    way.ask(q.line());
    way.repaint();
}

/// Whether the column this footer started last is still COMING UP: alive,
/// and not yet holding its slot — a second raise in that time would start a
/// second column, which ends the first as it comes: a column flashing.
fn column_coming(kept: &mut Kept) -> bool {
    let Some(child) = kept.column_child.as_ref() else {
        return false;
    };
    if child.ended() {
        kept.column_child = None;
        return false;
    }
    let pid = child.pid;
    crate::right_column::holder(crate::right_column::Tenant::Player)
        .is_none_or(|h| u32::try_from(h.pid).ok() != Some(pid))
}

/// Where a pointer is across the player's rows on a line of the region
/// whose grid column starts `base` cells in — the ONE answer the band a
/// grid's cell lights with, the cursor's cell and the cell a click takes all
/// read: the rows are drawn `chrome::FIELDS_LEAD` after the column's first
/// cell.
fn across(base: u16) -> crate::chrome::Across {
    crate::chrome::Across {
        lead: crate::chrome::FIELDS_LEAD.saturating_add(base),
        cell_px: crate::chrome::cell_width(),
    }
}

/// Where `x` is among the player's rows' own cells — a field's `at`, in
/// fractions of a cell — which is what a bar of a row is measured in.
fn column_of(x: f64) -> f64 {
    x / f64::from(crate::chrome::cell_width()).max(1.0) - f64::from(crate::chrome::FIELDS_LEAD)
}

impl Player {
    /// The state the resident keeps.
    fn state(&self) -> std::cell::Ref<'_, Option<State>> {
        std::cell::Ref::map(self.kept.borrow(), |k| &k.state)
    }

    /// Whether something is loaded: what the panel stands for.
    fn loaded(&self) -> bool {
        self.state().as_ref().is_some_and(crate::player::loaded)
    }

    /// Where the panel is in a region `screen` cells wide, while something
    /// is loaded — a function of the width and of stored fields.
    fn panel_for(&self, screen: u16) -> Option<crate::player_media::Panel> {
        if !self.loaded() {
            return None;
        }
        crate::player_media::footer_panel(screen, self.only)
    }

    /// The panel at the footer's width, as its last call said it.
    fn panel(&self) -> Option<crate::player_media::Panel> {
        self.panel_for(self.screen_columns)
    }

    /// The width the rows are laid out at in a region `screen` cells wide:
    /// what the panel beside them leaves, or the whole width — and the whole
    /// width under Only, where the rows are not drawn but still answer the
    /// keys.
    fn columns_for(&self, screen: u16) -> u16 {
        match self.panel_for(screen) {
            Some(p) if !self.only => p.from,
            _ => screen,
        }
    }

    /// [`Player::columns_for`] the footer's width.
    fn player_columns(&self) -> u16 {
        self.columns_for(self.screen_columns)
    }

    /// The view's rows, at the width they are laid out at.
    fn rows_now(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::player_rows::Row> {
        self.view
            .rows(self.state().as_ref(), self.player_columns(), texts)
    }

    /// Whether the panel beside the rows is folded NOW: folded by `z`, not
    /// the picture alone, and no globe in the picture's place.
    fn folded_now(&self) -> bool {
        self.kept.borrow().folded && !self.only && !self.globe
    }

    /// Tells the view whether the region leaves the picture no room beside
    /// the rows, whether the panel stands, whether a globe stands in the
    /// picture's place, whether a picture stands to fold: `true` when the
    /// rows, and so the region, changed with it.
    fn room_follows(&mut self, texts: &ronoctua_i18n::Texts) -> bool {
        let refused = self.loaded()
            && !self.only
            && crate::player_media::footer_panel(self.screen_columns, false).is_none();
        let panel = self.panel().is_some();
        let columns = self.player_columns();
        let globe = panel && self.view.globe_here();
        let globe_moved = std::mem::replace(&mut self.globe, globe) != globe;
        let folded = self.kept.borrow().folded;
        let fold = (panel && !self.only && !globe).then_some(folded);
        let state = self.kept.borrow().state.clone();
        self.view.beside_the_panel(panel);
        let folding = self.view.picture_foldable(fold);
        // THE CURSOR STAYS ON ITS ROW: the refusal is lines of the home above
        // it, and coming or going they moved it by as many.
        let spot = self
            .view
            .rows(state.as_ref(), columns, texts)
            .get(self.view.cursor())
            .map(|r| r.spot.clone());
        let changed = self.view.picture_refused(refused) || folding;
        if changed && let Some(spot) = spot {
            self.view.follow(&spot, state.as_ref(), columns, texts);
        }
        changed || (globe_moved && folded && !self.only)
    }

    /// The lanes whose pictures the footer shows: every one routed to one of
    /// its screens, each once, in the order the screens come.
    fn lanes_shown(&self) -> Vec<ronoctua_player::own::LaneId> {
        self.state().as_ref().map_or_else(Vec::new, |s| {
            crate::player_media::lanes_shown(s, self.screens.iter().map(|t| t.connector.as_deref()))
        })
    }

    /// The picture's threads, started while the panel shows a picture and
    /// ended when it does not — and Only ends with the panel: nothing loaded,
    /// no stage.
    fn pictures_follow(&mut self, shown: bool) {
        let panel = shown && self.panel().is_some();
        if !panel {
            self.only = false;
        }
        // A DIRECTORY'S PAGE draws its globe in the film's place: no picture
        // is drawn there, and none is asked for.
        let globe = self.view.panel && self.view.globe_here();
        // FOLDED, no picture is drawn beside the rows, and none is asked.
        let folded = self.folded_now();
        let wanted = if panel && !globe && !folded {
            self.lanes_shown()
        } else {
            Vec::new()
        };
        self.pictures
            .retain(|p| p.of.is_some_and(|l| wanted.contains(&l)));
        for lane in wanted {
            if !self.looping || self.pictures.iter().any(|p| p.of == Some(lane)) {
                continue;
            }
            let wake = self.frame_wake.clone();
            self.pictures.push(crate::player_picture::Picture::start(
                Some(lane),
                move || {
                    wake.call();
                    true
                },
            ));
        }
    }

    /// Lays the panel out for this frame and WRITES IT DOWN: the painter
    /// draws from it, and a press, a drag and a hover read it back — so a
    /// control is taken where it was drawn.
    fn lay_out(&mut self, laid: &Laid, shown: bool, hand: &mut Hand) {
        let before = self.drawn.clone();
        self.drawn = match (shown, self.panel()) {
            (true, Some(panel)) => {
                let state = self.state();
                if self.folded_now() {
                    crate::player_media::drawn_folded(panel, laid.first, laid.lines, state.as_ref())
                } else {
                    crate::player_media::drawn(panel, laid.first, laid.lines, state.as_ref())
                }
            }
            _ => crate::player_media::Drawn::default(),
        };
        // THE BOX THE PICTURES ARE LAID OUT IN, as the paint places them,
        // told to the view: its Layout page draws what this door shows.
        let room = match (self.drawn.panel, self.drawn.picture) {
            (Some(panel), Some((_, lines))) => {
                let air = crate::sizes::picture_air();
                let cell = tokens::metrics::cell_width(tokens::text::size());
                let line = tokens::metrics::line_height(tokens::text::size());
                Some((
                    (f32::from(panel.cells) * cell - air - air).max(1.0),
                    (f32::from(lines) * line - air - air).max(1.0),
                ))
            }
            _ => None,
        };
        self.view.pictures_room_is(room);
        if self.drawn != before {
            // AND THE BUTTON UNDER A POINTER THAT HAS NOT MOVED, read again
            // against what was just placed.
            hand.button_under = laid
                .pointer
                .and_then(|(x, y)| self.button_at(laid.frame, x, y));
            let d = &self.drawn;
            tracing::debug!(
                from = d.panel.map(|p| p.from),
                cells = d.panel.map(|p| p.cells),
                picture = ?d.picture,
                transport = d.buttons.first().map(|b| b.0),
                buttons = d.buttons.len(),
                bar = d.track.map(|t| t.line),
                "the player's panel laid out"
            );
        }
    }

    /// The button of the panel's transport at `(x, y)`, where the last frame
    /// PLACED it.
    fn button_at(
        &self,
        frame: Option<crate::chrome::Frame>,
        x: f64,
        y: f64,
    ) -> Option<&'static str> {
        panel_button_at(&self.drawn, frame?, x, y)
    }

    /// Whether `(x, y)` is on the panel's time bar, where the last frame drew
    /// it.
    fn track_at(&self, frame: Option<crate::chrome::Frame>, x: f64, y: f64) -> bool {
        frame.is_some_and(|f| panel_track_at(&self.drawn, f, x, y))
    }

    /// Whether `(x, y)` is on the panel at all — its picture, its title card,
    /// its transport or its time bar.
    fn panel_at(&self, frame: Option<crate::chrome::Frame>, x: f64, y: f64) -> bool {
        let (Some(panel), Some(f)) = (self.drawn.panel, frame) else {
            return false;
        };
        let left = f64::from(f.x_of(panel.from));
        let right = left + f64::from(panel.cells) * f64::from(f.cell());
        (left..right).contains(&x) && y >= f64::from(f.y_of(f.first))
    }

    /// Whether `(x, y)` is on the picture — or on the title card in its
    /// place, which answers the click the picture would.
    fn picture_at(&self, frame: Option<crate::chrome::Frame>, x: f64, y: f64) -> bool {
        // A DIRECTORY'S PAGE draws its globe in the picture's place, and no
        // picture: a click there that missed the globe pauses nothing.
        if self.view.panel && self.view.globe_here() {
            return false;
        }
        let (Some(panel), Some((first, lines)), Some(f)) =
            (self.drawn.panel, self.drawn.picture, frame)
        else {
            return false;
        };
        let top = f64::from(f.y_of(first));
        let under = f64::from(f.y_of(first + lines));
        let left = f64::from(f.x_of(panel.from));
        let right = left + f64::from(panel.cells) * f64::from(f.cell());
        (left..right).contains(&x) && (top..under).contains(&y)
    }

    /// The point `at` of the surface in the GLOBE, as the last frame drew it
    /// — among the rows or in the panel — on the screen the pointer is on.
    fn globe_px(&self, at: (f64, f64)) -> Option<(f32, f32)> {
        let drawn_on = self.globe_drawn.borrow();
        let (_, drawn, size) = drawn_on
            .iter()
            .find(|(on, ..)| *on == self.here)
            .or_else(|| drawn_on.first())
            .cloned()?;
        #[allow(clippy::cast_possible_truncation)]
        crate::player_globe::picture_px(drawn, size, (at.0 as f32, at.1 as f32))
    }

    /// Asks the GLOBE of what the view wants drawn, after the frame that
    /// measured it — its thread raised the first time a page shows one.
    fn follow_the_globe(&mut self) {
        let Some(ask) = self.view.wants_globe() else {
            return;
        };
        if !self.looping {
            return;
        }
        if self.globe_stage.is_none() {
            let mail = Arc::clone(&self.kept.borrow().mail);
            let wake = self.globe_wake.clone();
            self.globe_stage = crate::globe::stage(crate::wake::Out::new(move |beat| {
                mail.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .beats
                    .push(beat);
                wake.call();
                true
            }));
        }
        if let Some(stage) = self.globe_stage.as_mut() {
            stage.tell(ask);
        }
    }

    /// A key in the view.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        // `Esc` CALLS A DECK'S DRAG OFF first: the band stays where it was.
        if press == Press::Escape && self.dragged.take().is_some() {
            way.repaint();
            return;
        }
        // UNDER ONLY THE ROWS ARE NOT DRAWN: a key that needs them gives them
        // back first — and one that opens a page opens it there.
        if self.only && press.unseen() != Unseen::Acts {
            self.only = false;
            self.room_follows(way.texts());
            way.resized();
            way.follow();
            if press.unseen() == Unseen::ShowsTheRows {
                way.repaint();
                return;
            }
        }
        let columns = self.player_columns();
        let state = self.kept.borrow().state.clone();
        let errand = self.view.key(press, state.as_ref(), columns, way.texts());
        way.resized();
        way.follow();
        self.carry(errand, way);
    }

    /// An errand of the view, carried out in this door.
    fn carry(&mut self, errand: Errand, way: &mut dyn Doorway) {
        let kept = Rc::clone(&self.kept);
        let mut door = Door {
            kept: &kept,
            open: Some(self),
        };
        door.errand(errand, way);
    }

    /// What the player's watch heard: a state, or — the player closed
    /// everywhere — its farewell, which closes the view. A player that is
    /// merely GONE leaves the view open, saying it is not running.
    fn player_heard(&mut self, heard: &ronoctua_player::client::Heard, way: &mut dyn Doorway) {
        match crate::player::hearing(heard.clone()) {
            Hearing::Shows(s) => {
                // GONE, and the queue it had with it — and a deck's drag,
                // whose rows went with the state.
                if s.is_none() {
                    self.dragged = None;
                    self.outline_arrived(None, way);
                    // AND WHAT THE VIEW HELD of its sheets and desks: a player
                    // that comes back counts them from zero again.
                    self.view.player_gone();
                }
                self.state_arrived(s.map(|s| *s), way);
            }
            Hearing::Outline(o) => self.outline_arrived(Some(o), way),
            Hearing::Closes => {
                self.kept.borrow_mut().outline = None;
                self.kept.borrow_mut().state = None;
                way.close();
            }
        }
    }

    /// The queue's outline: kept, and handed to the view — whose cursor
    /// stays on the row it was on, by identity.
    fn outline_arrived(&mut self, outline: Option<Box<Outline>>, way: &mut dyn Doorway) {
        let columns = self.player_columns();
        self.kept.borrow_mut().outline.clone_from(&outline);
        let state = self.kept.borrow().state.clone();
        self.view
            .outline_heard(outline, state.as_ref(), columns, way.texts());
        way.resized();
        way.repaint();
    }

    /// The player's new state: the view, the cursor on its row.
    fn state_arrived(&mut self, s: Option<State>, way: &mut dyn Doorway) {
        // A HOLD ON THE OUTPUTS WHOSE PLAYER WENT is no hold.
        if self
            .sinks
            .as_ref()
            .is_some_and(crate::player::SinksHeld::ended)
        {
            self.sinks = None;
        }
        let columns = self.player_columns();
        let spot = {
            let state = self.state();
            self.view
                .rows(state.as_ref(), columns, way.texts())
                .get(self.view.cursor())
                .map(|r| r.spot.clone())
        };
        self.kept.borrow_mut().state = s;
        // THE ROWS ARE LAID OUT BESIDE A PICTURE THAT MAY HAVE COME OR GONE.
        let columns = self.player_columns();
        let state = self.kept.borrow().state.clone();
        if let Some(spot) = spot {
            self.view
                .follow(&spot, state.as_ref(), columns, way.texts());
        }
        self.view.settle_in(state.as_ref(), columns, way.texts());
        way.resized();
        way.repaint();
    }

    /// The line of a DECK among the rows under `under`: the row, the deck's
    /// line, the column from the deck's own first cell, that cell in the
    /// surface, and the deck.
    fn deck_at(
        &self,
        under: &Under,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<(usize, usize, f64, f64, Rc<crate::player_timeline::Deck>)> {
        let n = under.row?;
        under.line?;
        let rows = self.rows_now(texts);
        if !rows
            .get(n)
            .is_some_and(|r| !r.heading || !r.cells.is_empty())
        {
            return None;
        }
        let (d, column) = crate::player_rows::deck_at(&rows, n, column_of(under.at.0))?;
        let origin = f64::from(crate::chrome::FIELDS_LEAD + d.left);
        Some((n, d.line, column, origin, d.deck.clone()))
    }

    /// The row whose BAR is under the pointer, when a press lands on one.
    fn bar_at(&self, under: &Under, texts: &ronoctua_i18n::Texts) -> Option<usize> {
        let n = under.row?;
        under.line?;
        let rows = self.rows_now(texts);
        if !rows
            .get(n)
            .is_some_and(|r| !r.heading || !r.cells.is_empty())
        {
            return None;
        }
        crate::player_rows::slider_at(&rows, n, column_of(under.at.0)).map(|_| n)
    }

    /// Turns the bar of row `n` to where `x` is along it — the press, and
    /// every motion of the drag after it.
    fn slide(&mut self, n: usize, x: f64, way: &mut dyn Doorway) {
        let rows = self.rows_now(way.texts());
        let Some(bar) = rows.get(n).and_then(|r| r.slider) else {
            return;
        };
        let errand =
            crate::player_rows::slid(bar.of, bar.along(column_of(x)), self.state().as_ref());
        self.tell_now(errand, way);
    }

    /// An errand told straight away, its sentence said: a seek, a volume —
    /// what a drag asks at every motion.
    fn tell_now(&mut self, errand: Errand, way: &mut dyn Doorway) {
        if let Ok(Some(said)) = carried(&self.kept.borrow(), errand, &*way) {
            way.say(said, crate::echo::Echo::Normal);
        }
        way.repaint();
    }

    /// Seeks to where `x` is along the panel's time bar.
    fn seek_at(&mut self, x: f64, frame: Option<crate::chrome::Frame>, way: &mut dyn Doorway) {
        let (Some(track), Some(f)) = (self.drawn.track, frame) else {
            return;
        };
        let errand = crate::player_rows::slid(
            crate::player_rows::Slid::Position,
            track.along(x, f.cell()),
            self.state().as_ref(),
        );
        self.tell_now(errand, way);
    }

    /// A button of the panel's transport, pressed: a key of the view goes to
    /// the view, and every other button is what `player_transport` says.
    fn button_pressed(&mut self, id: &str, way: &mut dyn Doorway) {
        if let Some(press) = crate::player_transport::button_press(id) {
            self.press(press, way);
            return;
        }
        let errand = crate::player_transport::button_errand(id, self.state().as_ref(), way.texts());
        self.carry(errand, way);
    }

    /// A press on the picture: it plays or pauses — and a DOUBLE click puts
    /// it on the whole screen, as in the window.
    fn picture_pressed(
        &mut self,
        x: f64,
        frame: Option<crate::chrome::Frame>,
        hand: &mut Hand,
        way: &mut dyn Doorway,
    ) {
        use crate::player_media::PictureClick;
        let now = std::time::Instant::now();
        let zone = match (self.drawn.panel, frame) {
            (Some(panel), Some(f)) => {
                let left = f64::from(f.x_of(panel.from));
                let width = f64::from(panel.cells) * f64::from(f.cell());
                crate::player_media::picture_zone(width, x - left)
            }
            _ => crate::player_media::Zone::Middle,
        };
        let click = hand
            .picture_clicked
            .clicked(now, zone, crate::player_window::DOUBLE_CLICK);
        let toggle = || Errand::Tell(ronoctua_player::wire::Ask::Toggle);
        match click {
            PictureClick::Toggle => self.carry(toggle(), way),
            PictureClick::ToggleAndMaximize => {
                self.carry(toggle(), way);
                self.carry(Errand::Show(Show::Maximized), way);
            }
            PictureClick::ToggleAndSeek(seconds) => {
                self.carry(toggle(), way);
                let seek = crate::player_transport::seek_errand(seconds > 0.0, way.texts());
                self.carry(seek, way);
            }
            PictureClick::Seek(seconds) => {
                let seek = crate::player_transport::seek_errand(seconds > 0.0, way.texts());
                self.carry(seek, way);
            }
        }
    }

    /// A press on a line of a DECK: what `Deck::press` says it takes, done,
    /// and what it holds kept on the hand. `false` where no control of a
    /// deck is.
    fn deck_pressed(&mut self, under: &Under, hand: &mut Hand, way: &mut dyn Doorway) -> bool {
        let Some((n, line, column, origin, deck)) = self.deck_at(under, way.texts()) else {
            return false;
        };
        let hit = deck.hit(line, column);
        if hit.is_none() || matches!(hit, Some(crate::player_timeline::Hit::Name(_))) {
            return false;
        }
        if !matches!(hit, Some(crate::player_timeline::Hit::Band { .. })) {
            self.view.grabbed(n);
        }
        let (grip, act) = deck.press(line, column);
        // THE ROWS NEVER SCROLL THEIR LANES — every lane is a row.
        hand.deck = grip
            .filter(|g| !matches!(g, crate::player_timeline::Grip::Bar))
            .map(|g| (g, origin, n));
        self.dragged = None;
        self.deck_act(act, n, way);
        true
    }

    /// The pointer moved with a line of a deck HELD: the drag goes on.
    fn deck_dragged(
        &mut self,
        (grip, origin, n): (crate::player_timeline::Grip, f64, usize),
        x: f64,
        hand: &mut Hand,
        way: &mut dyn Doorway,
    ) {
        // NO PLAYER, NO DRAG: what the grip would ask is about rows that are
        // gone — and telling it would start a player again.
        if self.state().is_none() {
            hand.deck = None;
            self.dragged = None;
            return;
        }
        let mut grip = grip;
        let column = x / f64::from(crate::chrome::cell_width()).max(1.0) - origin;
        let was_a_drag = matches!(grip, crate::player_timeline::Grip::Band { moved: true, .. });
        let (act, ghost) = grip.moved(column);
        // A DRAG `Esc` CALLED OFF stays off.
        if let Some(h) = ghost
            && (!was_a_drag || self.dragged.is_some())
        {
            self.dragged = Some(h);
            way.repaint();
        }
        hand.deck = Some((grip, origin, n));
        match act {
            crate::player_timeline::Act::Tell(ask) => self.tell_now(Errand::Tell(ask), way),
            act => self.deck_act(act, n, way),
        }
    }

    /// What a deck on row `n` asked: the player told, a lane's page opened,
    /// or the row CHOSEN — the first click on a band brings the cursor to it,
    /// and the second opens the lane's page (invariant 19).
    fn deck_act(&mut self, act: crate::player_timeline::Act, n: usize, way: &mut dyn Doorway) {
        use crate::player_timeline::Act;
        match act {
            Act::Nothing => way.repaint(),
            Act::Tell(ask) => self.carry(Errand::Tell(ask), way),
            Act::Choose(lane) => {
                if self.view.band_clicked(n) {
                    self.press(Press::Go(crate::player::PageId::Lane(lane)), way);
                } else {
                    way.repaint();
                }
            }
        }
    }

    /// The pointer moved to `at`: the group of the globe under it lit, and a
    /// press held on the globe, past `atlas.clickSlack`, turning it.
    fn globe_moved(&mut self, at: (f64, f64), hand: &mut Hand, way: &mut dyn Doorway) {
        if let Some((pressed, last, dragging)) = hand.globe {
            let far = (at.0 - pressed.0).hypot(at.1 - pressed.1);
            let dragging = dragging || far > f64::from(tokens::space::ATLAS_CLICKSLACK);
            hand.globe = Some((pressed, if dragging { at } else { last }, dragging));
            #[allow(clippy::cast_possible_truncation)]
            if dragging
                && self
                    .view
                    .globe_panned((at.0 - last.0) as f32, (at.1 - last.1) as f32)
            {
                self.follow_the_globe();
                way.repaint();
            }
            return;
        }
        let px = self.globe_px(at);
        if self.view.globe_pointed(px) {
            way.repaint();
        }
    }

    /// A click on the globe at `at`: the first on a group brings the cursor
    /// to its row, the second does what `RET` does there.
    fn globe_clicked(&mut self, at: (f64, f64), way: &mut dyn Doorway) {
        let Some(px) = self.globe_px(at) else {
            return;
        };
        let columns = self.player_columns();
        let state = self.kept.borrow().state.clone();
        let errand = self
            .view
            .globe_clicked(px, state.as_ref(), columns, way.texts());
        let keeps = self.view.globe_keeps_the_scroll();
        way.resized();
        // A FIRST CLICK only brings the cursor: the globe stays under the
        // hand for the second.
        if !keeps {
            way.follow();
        }
        self.carry(errand, way);
    }

    /// The control of a row under `(line, cell)` — a title's glyph — read
    /// from what the last frame placed.
    fn control_at(&self, under: &Under) -> Option<&'static str> {
        let (i, cell) = (under.line?, under.cell?);
        self.controls_placed
            .borrow()
            .iter()
            .find(|&&(line, from, width, _)| line == i && crate::chrome::in_cell(cell, from, width))
            .map(|&(_, _, _, about)| about)
    }

    /// What a control of the region says under the pointer — THE SWAP SAYS
    /// WHAT IT DOES HERE: from the footer it opens the window in the view's
    /// place.
    fn control_sentence(id: &str, texts: &ronoctua_i18n::Texts) -> Option<String> {
        match id {
            "player-swap" => Some(texts.t(crate::key::PLAYER_SWAP_FOOTER)),
            id => crate::chrome::hint_sentence(id, texts),
        }
    }

    /// Whether a row of the sound outputs is among the view's rows on screen.
    fn sinks_in_sight(&self, laid: &Laid, shown: bool, texts: &ronoctua_i18n::Texts) -> bool {
        if !shown || self.only || !laid.shown {
            return false;
        }
        let rows = self.rows_now(texts);
        crate::player_lane::sinks_in_sight(&rows, laid.offset, laid.room)
    }

    /// The region's rows as told to the footer: the view's, beside the
    /// panel's lines — or blank rows over the whole stage while the picture
    /// is alone — each with what the footer's one painter draws beside its
    /// fields: the chips, the caret, the covers, what the pointer lights, a
    /// grid's cursor.
    fn told_whole(&self, seen: &Seen<'_>) -> Vec<crate::region::Told> {
        let width = self.columns_for(seen.columns);
        let panel = self.panel_for(seen.columns);
        let state = self.state();
        let rows = self.view.rows(state.as_ref(), width, seen.texts);
        let ceiling = usize::from(seen.lines);
        // ONLY THE PICTURE: the region is the panel's, as tall as the stage
        // lets it grow — the rows answer the keys and are not drawn.
        if self.only && panel.is_some() {
            return (0..ceiling)
                .map(|_| crate::region::Told::default())
                .collect();
        }
        // BESIDE THE ROWS the panel keeps the lines its picture needs, on
        // every page; FOLDED, only the transport and the time bar.
        let keep = panel.map_or(0, |panel| {
            let lines = if self.folded_now() {
                crate::player_media::FOLDED_LINES
            } else {
                crate::player_media::panel_lines(
                    panel,
                    tokens::metrics::cell_width(tokens::text::size()),
                    tokens::metrics::line_height(tokens::text::size()),
                )
            };
            usize::from(lines).min(ceiling)
        });
        let mut out: Vec<crate::region::Told> = rows
            .iter()
            .enumerate()
            .map(|(n, r)| {
                // WHAT THE POINTER LIGHTS: a line of a figure nothing — a
                // deck's, the schematic's, the globe's, a cover's but a
                // strip's — and otherwise what the rows say
                // (`player_rows::lighting`, the window's answer too).
                let lighting = crate::player_rows::lighting(&rows, n);
                let figure = r.deck.is_some()
                    || r.schematic.is_some()
                    || r.globe.is_some()
                    || (!r.covers.is_empty()
                        && !matches!(lighting, crate::player_rows::Lighting::From(_)));
                let lit = if figure {
                    crate::region::LineLights::Nothing
                } else {
                    match lighting {
                        crate::player_rows::Lighting::Cell(cells) => {
                            crate::region::LineLights::UnderX(cells)
                        }
                        crate::player_rows::Lighting::From(c) => crate::region::LineLights::From(c),
                        crate::player_rows::Lighting::Whole => crate::region::LineLights::Row,
                    }
                };
                // A GRID'S CURSOR CELL wears the selection, by the arithmetic
                // its click takes it with.
                let chosen = r
                    .focused
                    .then(|| {
                        r.cells
                            .iter()
                            .filter(|c| !c.control)
                            .find(|c| c.target == r.target)
                            .map(|c| (c.from, c.width))
                    })
                    .flatten();
                crate::region::Told {
                    row: Row {
                        fields: r.fields.clone(),
                        // A TITLE WITH A CONTROL takes a click too — on the
                        // control alone (`player_rows::takes`, the lighting's
                        // own cells).
                        takes: crate::player_rows::takes(&rows, n),
                    },
                    chips: r.chips.iter().map(|c| c.at).collect(),
                    caret: r.caret,
                    covers: r.covers.clone(),
                    continues: r.continues,
                    lights: lit,
                    chosen,
                    ..crate::region::Told::default()
                }
            })
            .collect();
        while out.len() < keep {
            out.push(crate::region::Told::default());
        }
        out
    }
}

impl Region for Player {
    fn id(&self) -> &'static str {
        ID
    }

    /// The region's rows — its fields, and whether each takes a click
    /// ([`Player::told_whole`]).
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.told_whole(seen).into_iter().map(|t| t.row).collect()
    }

    /// The region's rows from `skip`, at most `take`, as told to the footer:
    /// with what its one painter draws beside the fields.
    fn told(&self, seen: &Seen<'_>, skip: usize, take: usize) -> Vec<crate::region::Told> {
        self.told_whole(seen)
            .into_iter()
            .skip(skip)
            .take(take)
            .collect()
    }

    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view.hint(
            self.state().as_ref(),
            self.columns_for(seen.columns),
            seen.texts,
        )
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// A key of the view, while it holds the keyboard.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        self.screen_columns = way.columns();
        // `Esc` UNDER ONLY gives the rows back first — one level, as the
        // window's stage has it — and only then reaches the view.
        // The panel beside the rows is told at once, as every other way out of
        // Only tells it (`press`, the `Show(Only)` errand): a frame of the
        // door is not owed to come before what is drawn next.
        if key.sym == ronoctua_keymap::Sym::Esc && self.only {
            self.only = false;
            self.room_follows(way.texts());
            way.resized();
            way.follow();
            way.repaint();
            return Taken::Yes;
        }
        // EVERY DOOR THROUGH ONE READER — `player::press_of_key`, with the
        // modifiers — and every key the text's while one is being edited.
        match self.view.press_of_key(key) {
            Some(press) => {
                self.press(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        self.screen_columns = way.columns();
        let kept = Rc::clone(&self.kept);
        let mut door = Door {
            kept: &kept,
            open: Some(self),
        };
        Taken::of(door.command(id, argument, way))
    }

    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let under = Under {
            at: (0.0, 0.0),
            line: None,
            row: Some(row),
            cell: None,
            base: 0,
            ctrl: false,
            shift: false,
            frame: None,
        };
        self.clicked_under(row, &under, way);
    }

    /// A click on a line of the view: THE MOUSE AIMS, AND THE SECOND CLICK
    /// ACTS — by its column too, through the answer the band of a grid's
    /// cell is painted by. `Ctrl` marks a file of the picker, `Shift` every
    /// file up to it.
    fn clicked_under(&mut self, n: usize, under: &Under, way: &mut dyn Doorway) {
        self.screen_columns = way.columns();
        let width = self.player_columns();
        let across = across(under.base);
        #[allow(clippy::cast_possible_truncation)]
        let x = under.at.0 as f32;
        let state = self.kept.borrow().state.clone();
        let rows = self.view.rows(state.as_ref(), width, way.texts());
        if (under.ctrl || under.shift)
            && self.view.mark_clicked_px(n, x, across, &rows, under.shift)
        {
            way.repaint();
            return;
        }
        if self.view.clicked_px(n, x, across, &rows) {
            self.press(Press::Enter, way);
        } else {
            way.repaint();
        }
    }

    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        self.screen_columns = way.columns();
        let kept = Rc::clone(&self.kept);
        let mut door = Door {
            kept: &kept,
            open: Some(self),
        };
        door.answered(id, text, way);
    }

    /// The question was let go: what the view asked goes with it.
    fn unanswered(&mut self, _: &'static str, way: &mut dyn Doorway) {
        self.kept.borrow_mut().pairing.let_go();
        self.view.let_go();
        way.repaint();
    }

    /// The clipboard's text the view asked for — a paste that arrives after
    /// the text was left is nobody's.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        if self.view.text_pasted(text) {
            way.repaint();
        }
    }

    /// What the player's threads left: how an ask told off the loop ended —
    /// the view's share first, then what the door says — how a pairing
    /// ended, the globe's pictures, a read the view did off the loop.
    fn woken(&mut self, way: &mut dyn Doorway) {
        self.screen_columns = way.columns();
        let mail = self.kept.borrow().take_the_mail();
        for answer in mail.told {
            // A DRAFT, where the daemon wrote it: into the editor.
            if let Told::Draft(path) = &answer {
                open_editor(path, way);
                let said = way.texts().t(crate::key::PLAYER_DRAFT_OPENED);
                way.say(said, crate::echo::Echo::Normal);
                way.repaint();
                continue;
            }
            let columns = self.player_columns();
            let state = self.kept.borrow().state.clone();
            let took = self.view.told(&answer, way.texts());
            if took {
                self.view.settle_in(state.as_ref(), columns, way.texts());
                way.resized();
                way.repaint();
            }
            // A QUESTION THE VIEW ASKS is on the region, "no" first: what the
            // order put on the minibuffer is taken back — it was not done —
            // and under the picture alone the rows come back.
            let older = state.as_ref().is_some_and(crate::player::older);
            match crate::player::heard(took, &answer, older, way.texts()) {
                Heard::Asked => {
                    way.take_back();
                    if self.only {
                        self.only = false;
                        self.room_follows(way.texts());
                        way.resized();
                        way.follow();
                    }
                    way.repaint();
                }
                Heard::Said(Some(said)) => {
                    way.say(said, crate::echo::Echo::Normal);
                    way.repaint();
                }
                Heard::Said(None) => {}
            }
        }
        for p in mail.paired {
            if let Some(said) = crate::player_machines::paired_said(&p, way.texts()) {
                way.say(said, crate::echo::Echo::Normal);
                way.repaint();
            }
        }
        for b in mail.beats {
            if self.view.saw_globe(&b) {
                way.repaint();
            }
        }
        if mail.view {
            let columns = self.player_columns();
            let state = self.kept.borrow().state.clone();
            self.view.settle_in(state.as_ref(), columns, way.texts());
            way.resized();
            way.repaint();
        }
    }

    /// What the footer heard: the player's watch, and the screens it knows.
    fn heard(&mut self, news: &dyn Any, way: &mut dyn Doorway) {
        self.screen_columns = way.columns();
        if let Some(heard) = news.downcast_ref::<ronoctua_player::client::Heard>() {
            self.player_heard(heard, way);
        } else if let Some(screens) = news.downcast_ref::<crate::region::Screens>() {
            // THE SCREENS MOVED — described, gone, the pointer on another
            // footer: the view is told, and the pictures follow what is
            // routed where.
            let known: Vec<crate::player_lane::Known> = screens
                .known
                .iter()
                .map(|(connector, description)| crate::player_lane::Known {
                    connector: connector.clone(),
                    description: description.clone(),
                })
                .collect();
            self.known.clone_from(&known);
            self.here.clone_from(&screens.here);
            if self.view.screens_are(known, screens.here.clone()) {
                way.repaint();
            }
        }
    }

    fn shape(&self) -> Shape {
        if self.only { Shape::Stage } else { Shape::List }
    }

    fn types_capitals(&self) -> bool {
        self.view.editing_text()
    }

    /// A held button drags the player's own controls — a deck's band, a bar,
    /// the globe: no text is swept here.
    fn sweeps_text(&self) -> bool {
        false
    }

    /// The file picker of the view's pages walks half the page.
    fn page_is(&self, lines: usize) {
        self.view.page_is(lines);
    }

    /// The picture the view holds under `key` — a cover of the file the
    /// Metadata shows, a suggested one, a poster: the view's own, which the
    /// state only says which file it is of.
    fn cover_picture(
        &self,
        key: &str,
    ) -> Option<&(String, std::sync::Arc<ronoctua_render::image::Rgba>)> {
        let kept = self.kept.borrow();
        self.view.picture(kept.state.as_ref(), key)
    }

    fn owns_point(&self, at: (f64, f64), frame: Option<crate::chrome::Frame>) -> bool {
        // THE PANEL IS NO ROW: its picture, its transport and its time bar
        // are controls of their own.
        self.panel_at(frame, at.0, at.1)
    }

    fn hovered(
        &mut self,
        under: &Under,
        hand: &mut Option<Box<dyn Any>>,
        way: &mut dyn Doorway,
    ) -> bool {
        let hand = hand_of(hand);
        let before = (hand.button_under, hand.deck_under, self.control_under);
        hand.button_under = self.button_at(under.frame, under.at.0, under.at.1);
        // AND A CONTROL OF A DECK among the rows: said on the message line.
        hand.deck_under =
            self.deck_at(under, way.texts())
                .and_then(|(n, line, column, _, deck)| {
                    deck.hit(line, column).map(|hit| (n, line, hit))
                });
        self.control_under = self.control_at(under);
        // AND THE GLOBE: the group under the hand lit, and held, turned.
        self.globe_moved(under.at, hand, way);
        before != (hand.button_under, hand.deck_under, self.control_under)
    }

    fn dragged(
        &mut self,
        under: &Under,
        hand: &mut Option<Box<dyn Any>>,
        way: &mut dyn Doorway,
    ) -> bool {
        let hand = hand_of(hand);
        if let Some(held) = hand.deck {
            self.deck_dragged(held, under.at.0, hand, way);
        } else if let Some(n) = hand.dragging {
            self.slide(n, under.at.0, way);
        } else if hand.dragging_track {
            self.seek_at(under.at.0, under.frame, way);
        } else {
            return false;
        }
        true
    }

    fn pressed(
        &mut self,
        under: &Under,
        hand: &mut Option<Box<dyn Any>>,
        way: &mut dyn Doorway,
    ) -> bool {
        self.screen_columns = way.columns();
        let hand = hand_of(hand);
        let at = under.at;
        if let Some(id) = self.button_at(under.frame, at.0, at.1) {
            // A BUTTON OF THE PANEL, where the last frame placed it.
            self.button_pressed(id, way);
        } else if self.track_at(under.frame, at.0, at.1) {
            // THE PANEL'S TIME BAR: a press seeks there, and holding drags.
            hand.dragging_track = true;
            self.seek_at(at.0, under.frame, way);
        } else if self.globe_px(at).is_some() {
            // THE GLOBE: held, a drag turns it and let go where it was
            // pressed it is a click.
            hand.globe = Some((at, at, false));
        } else if self.picture_at(under.frame, at.0, at.1) {
            // THE PICTURE: a click plays or pauses, a double click puts it on
            // the whole screen.
            self.picture_pressed(at.0, under.frame, hand, way);
        } else if self.deck_pressed(under, hand, way) {
            // A LINE OF A DECK — its glyph, its band, its track, its meter.
        } else if let Some(n) = self.bar_at(under, way.texts()) {
            // A BAR OF THE VIEW — the position's, the volume's: a press sets
            // it there and holding drags it.
            hand.dragging = Some(n);
            self.view.grabbed(n);
            self.slide(n, at.0, way);
        } else {
            return false;
        }
        true
    }

    fn released(&mut self, hand: &mut Option<Box<dyn Any>>, way: &mut dyn Doorway) {
        let hand = hand_of(hand);
        hand.dragging = None;
        hand.dragging_track = false;
        // THE GLOBE LET GO where it was pressed: a click on the group there.
        if let Some((at, _, false)) = hand.globe.take() {
            self.globe_clicked(at, way);
        }
        // A DECK LET GO: a band dragged asks its new start, and one let go
        // where it was pressed chooses its row.
        if let Some((grip, _, n)) = hand.deck.take() {
            let act = grip.released(self.dragged.take());
            if matches!(grip, crate::player_timeline::Grip::Band { moved: true, .. }) {
                self.view.grabbed(n);
            }
            if self.state().is_some() {
                self.deck_act(act, n, way);
            }
        }
    }

    fn left(&mut self, hand: &mut Option<Box<dyn Any>>, way: &mut dyn Doorway) {
        let hand = hand_of(hand);
        hand.dragging = None;
        hand.dragging_track = false;
        hand.button_under = None;
        // A DECK'S DRAG THAT LEFT is called off: nothing is asked.
        if hand.deck.take().is_some() {
            self.dragged = None;
        }
        hand.deck_under = None;
        self.control_under = None;
        // AND THE GLOBE: nothing held, nothing lit by the hand.
        hand.globe = None;
        if self.view.globe_pointed(None) {
            way.repaint();
        }
    }

    fn takes_the_wheel(&self, under: &Under) -> bool {
        self.globe_px(under.at).is_some()
    }

    /// THE WHEEL ON THE GLOBE brings it nearer, or further.
    fn wheeled(
        &mut self,
        _: &Under,
        lines: i64,
        _: &mut Option<Box<dyn Any>>,
        way: &mut dyn Doorway,
    ) -> bool {
        if self.view.globe_wheeled(lines) {
            self.follow_the_globe();
            way.repaint();
            return true;
        }
        false
    }

    /// What the message line says under the pointer: a button of the panel
    /// — the sentence its window says too — a control of a title, a control
    /// of a deck.
    fn said_under(&self, hand: Option<&dyn Any>, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let hand = hand.and_then(|h| h.downcast_ref::<Hand>());
        hand.and_then(|h| h.button_under)
            .and_then(|id| Self::control_sentence(id, texts))
            .or_else(|| {
                self.control_under
                    .and_then(|id| Self::control_sentence(id, texts))
            })
            .or_else(|| {
                hand.and_then(|h| h.deck_under).and_then(|(_, _, hit)| {
                    crate::player_timeline::hover_sentence(hit, self.state().as_ref(), texts)
                })
            })
    }

    /// WHETHER THE PICTURE HAS ROOM BESIDE THE ROWS, before the shape is
    /// asked: when that changed, a line of the home came or went.
    fn before_measuring(&mut self, seen: &Seen<'_>) -> bool {
        self.screen_columns = seen.columns;
        self.room_follows(seen.texts)
    }

    /// The frame's layout: the picture's threads follow what the region
    /// shows, the panel is laid out and written down, the screens are told
    /// to the view, and the sound outputs held while a row of them is on
    /// screen.
    fn laid_out(&mut self, laid: &Laid, hand: &mut Option<Box<dyn Any>>, seen: &Seen<'_>) {
        self.screen_columns = laid.columns;
        self.screens.clone_from(&laid.screens);
        // WHERE THE GLOBE IS DRAWN is this frame's, screen by screen.
        self.globe_drawn.borrow_mut().clear();
        let shown = laid.ranked;
        self.pictures_follow(shown);
        let hand = hand_of(hand);
        self.lay_out(laid, shown, hand);
        let in_sight = self.sinks_in_sight(laid, shown, seen.texts);
        match (in_sight, self.sinks.is_some()) {
            (true, false) => self.sinks = Some(crate::player::hold_sinks()),
            (false, true) => self.sinks = None,
            _ => {}
        }
    }

    /// The panel, the deck, the schematic, the globe and the pictures, over
    /// the rows — the window's drawing, on this screen.
    #[allow(clippy::too_many_lines)]
    fn paint(&self, easel: &mut RegionEasel<'_>) {
        let hand = easel.hand.and_then(|h| h.downcast_ref::<Hand>());
        let cell = tokens::metrics::cell_width(tokens::text::size());
        let tall = tokens::metrics::line_height(tokens::text::size());
        let state = self.state();
        let rows = self.rows_now(easel.texts);
        let deck_sound = state.as_ref().map(|s| crate::player_timeline::Sound {
            volume_percent: s.volume,
            muted: s.muted,
        });
        let frame = easel.area.frame();
        let placed = easel.placed;
        let row_at = |i: usize| placed.get(i).and_then(|p| rows.get(p.row));
        // THE CONTROLS OF THE ROWS, where this frame puts them — for the
        // pointer: the line of the page, the cell, its width and its
        // sentence's id.
        {
            let mut controls = self.controls_placed.borrow_mut();
            controls.clear();
            for (i, p) in placed.iter().enumerate() {
                for (from, width, about) in crate::player_rows::controls_under(&rows, p.row) {
                    controls.push((i, from, width, about));
                }
            }
        }
        let mut globe_box: Option<(ronoctua_render::image::Area, u32)> = None;
        for (i, p) in placed.iter().enumerate() {
            let Some(row) = rows.get(p.row) else {
                continue;
            };
            let y = p.line;
            // THE DECK'S LINES, the window's drawing: each run of one deck's
            // lines on screen at once, so the playhead is one line across.
            if let Some(d) = &row.deck {
                let before = i
                    .checked_sub(1)
                    .and_then(row_at)
                    .and_then(|r| r.deck.as_ref());
                if before.is_none_or(|b| !Rc::ptr_eq(&b.deck, &d.deck) || b.line + 1 != d.line) {
                    let run = (i..placed.len())
                        .map_while(|k| row_at(k).and_then(|r| r.deck.as_ref()))
                        .take_while(|l| Rc::ptr_eq(&l.deck, &d.deck))
                        .count();
                    easel.under.extend(d.deck.holding(self.dragged).quads(
                        frame,
                        y,
                        p.base + crate::chrome::FIELDS_LEAD + d.left,
                        d.line..d.line + run,
                        deck_sound,
                    ));
                }
            }
            // AND THE LAYOUT'S SCHEMATIC, once for each run of its lines.
            if let Some(sc) = &row.schematic {
                let before = i
                    .checked_sub(1)
                    .and_then(row_at)
                    .and_then(|r| r.schematic.as_ref());
                if before.is_none_or(|b| !Rc::ptr_eq(&b.figure, &sc.figure)) {
                    let run = (i..placed.len())
                        .map_while(|k| row_at(k).and_then(|r| r.schematic.as_ref()))
                        .take_while(|l| Rc::ptr_eq(&l.figure, &sc.figure))
                        .count();
                    easel.under.extend(sc.figure.quads(
                        (
                            f32::from(p.base + crate::chrome::FIELDS_LEAD + sc.left) * cell,
                            f32::from(y) * tall,
                        ),
                        (sc.line, u16::try_from(run).unwrap_or(u16::MAX)),
                        (cell, tall),
                    ));
                }
            }
            // AND THE GLOBE'S BOX, once for its run of lines on screen.
            if let Some(g) = &row.globe
                && i.checked_sub(1)
                    .and_then(row_at)
                    .is_none_or(|r| r.globe.is_none())
            {
                let run = (i..placed.len())
                    .take_while(|&k| row_at(k).is_some_and(|r| r.globe.is_some()))
                    .count();
                globe_box = Some(crate::player_globe::box_of(
                    g,
                    run,
                    (
                        f32::from(p.base + crate::chrome::FIELDS_LEAD) * cell,
                        f32::from(y) * tall,
                    ),
                    (cell, tall),
                ));
            }
        }
        // THE PANEL, as this frame laid it out: the picture fitted into its
        // lines in the video's own proportion — or, with no frame to draw,
        // the TITLE CARD in its place — and at its foot the transport and the
        // time bar.
        let drawn = &self.drawn;
        let output = easel.screen.key.clone();
        match (drawn.panel, drawn.picture) {
            // A DIRECTORY'S PAGE: the GLOBE in the pictures' place.
            (Some(panel), Some((top, lines))) if self.view.panel && self.view.globe_here() => {
                globe_box = Some((
                    ronoctua_render::image::Area {
                        x: f32::from(panel.from) * cell,
                        y: f32::from(top) * tall,
                        width: f32::from(panel.cells) * cell,
                        height: f32::from(lines) * tall,
                    },
                    0,
                ));
            }
            (Some(panel), Some((top, lines))) => {
                let air = crate::sizes::picture_air();
                let room = ronoctua_player::arrange::Rect {
                    x: f32::from(panel.from) * cell + air,
                    y: f32::from(top) * tall + air,
                    width: (f32::from(panel.cells) * cell - air - air).max(1.0),
                    height: (f32::from(lines) * tall - air - air).max(1.0),
                };
                let connector = easel.screen.connector.as_deref();
                let placed = state.as_ref().map_or_else(Vec::new, |s| {
                    crate::player_media::placed_on_screen(s, connector, room)
                });
                #[allow(clippy::cast_precision_loss)]
                let scale = easel.screen.scale.max(1) as f32;
                let mut shown = 0;
                for (lane, rect) in &placed {
                    let Some(pic) = self.pictures.iter().find(|p| p.of == Some(*lane)) else {
                        continue;
                    };
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let size = ((rect.width * scale) as u32, (rect.height * scale) as u32);
                    {
                        let mut wants = self.wants.borrow_mut();
                        match wants.iter_mut().find(|(l, _)| l == lane) {
                            Some((_, wanted)) => {
                                *wanted = (wanted.0.max(size.0), wanted.1.max(size.1));
                            }
                            None => wants.push((*lane, size)),
                        }
                    }
                    let Some(f) = pic.frame.as_ref() else {
                        continue;
                    };
                    let fit = crate::chrome::picture_in(
                        rect.width,
                        rect.height,
                        (f.width, f.height),
                        false,
                    );
                    // A FRAME THAT CHANGED since it last went up goes up now;
                    // one that did not, only when its texture is missing.
                    let pixels = std::sync::Arc::clone(f);
                    easel.pictures.push(crate::region::Painted {
                        key: pic.key(&output),
                        area: ronoctua_render::image::Area {
                            x: rect.x + fit.x,
                            y: rect.y + fit.y,
                            width: fit.width,
                            height: fit.height,
                        },
                        source: if pic.uploaded {
                            crate::region::Source::Still(pixels)
                        } else {
                            crate::region::Source::Moving(pixels)
                        },
                    });
                    shown += 1;
                }
                // THE TITLE CARD while there is no frame to show here: what
                // plays, in words, in the middle of the pictures' place — on
                // the region's own ground.
                if shown == 0
                    && let Some(title) = state.as_ref().and_then(|s| {
                        crate::player::now_playing(
                            s,
                            self.kept.borrow().outline.as_deref(),
                            easel.texts,
                        )
                    })
                {
                    for (l, c, text) in crate::player_media::title_card(&title, panel.cells, lines)
                    {
                        easel.spans.push(ronoctua_render::Span {
                            size: ronoctua_render::Size::Grid,
                            text,
                            column: panel.from + c,
                            line: top + l,
                            color: tokens::color::TEXT_SECONDARY,
                            edge: false,
                            foot: false,
                        });
                    }
                }
            }
            _ => {}
        }
        // THE GLOBE, measured in the box its lines — or the panel — made this
        // frame, and its picture when one came back.
        if let Some((area, gone)) = globe_box {
            let mut largest = self.globe_largest.borrow_mut();
            if largest.is_none_or(|(a, _)| a.width * a.height < area.width * area.height) {
                *largest = Some((area, gone));
            }
            if let Some((key, rgba)) = self.view.globe_picture() {
                let drawn = crate::sizes::picture_in(area, rgba.width, rgba.height);
                self.globe_drawn.borrow_mut().push((
                    easel.screen.connector.clone(),
                    drawn,
                    (rgba.width, rgba.height),
                ));
                easel.pictures.push(crate::region::Painted {
                    key,
                    area: drawn,
                    source: crate::region::Source::Moving(std::sync::Arc::new(rgba)),
                });
            }
        }
        // THE TRANSPORT, where `player_media::buttons` placed it: the
        // window's glyphs, the one under the pointer and a switch that is on
        // in the accent — and the clock after them, where it fits.
        let stage = if self.only {
            crate::player_window::Stage::Only
        } else {
            crate::player_window::Stage::Off
        };
        easel.spans.extend(panel_glyphs(
            drawn,
            state.as_ref(),
            stage,
            hand.and_then(|h| h.button_under),
        ));
        if let (Some(clock), Some((line, at))) = (
            state.as_ref().and_then(crate::player_media::clock_text),
            drawn.clock,
        ) {
            easel.spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Grid,
                text: clock,
                column: at,
                line,
                color: tokens::color::TEXT_SECONDARY,
                edge: false,
                foot: false,
            });
        }
        // AND THE TIME BAR, the window's own drawing, on the track the
        // pointer takes — after the pictures.
        if let Some(track) = drawn.track {
            let (l, r) = track.px(cell);
            let played = state.as_ref().and_then(|s| {
                #[allow(clippy::cast_possible_truncation)]
                s.duration
                    .filter(|d| *d > 0.0)
                    .map(|d| (s.position / d).clamp(0.0, 1.0) as f32)
            });
            easel.over.extend(crate::chrome::time_bar(
                l,
                r,
                f32::from(track.line) * tall,
                tall,
                played,
            ));
        }
    }

    /// Every screen painted: the globe measured once, in the largest box,
    /// each picture asked for at the largest size a screen draws it, and the
    /// pictures that went up marked so.
    fn painted(&mut self, presented: bool, seen: &Seen<'_>) {
        if let Some((area, gone)) = self.globe_largest.borrow_mut().take() {
            let rows = self.rows_now(seen.texts);
            self.view.globe_measured(
                &rows,
                crate::fields::globe_texture(area),
                gone,
                tokens::theme::palette(),
                crate::globe::tongue(seen.texts),
            );
        }
        for (lane, size) in self.wants.borrow_mut().drain(..) {
            if let Some(pic) = self.pictures.iter().find(|p| p.of == Some(lane)) {
                pic.want(size);
            }
        }
        if presented {
            for p in &mut self.pictures {
                p.uploaded = true;
            }
        }
    }

    /// After the frame: the control under a pointer that has not moved, read
    /// again against what was just placed — one round, never a loop — and
    /// the globe asked for what the frame measured.
    fn after_the_frame(
        &mut self,
        under: Option<&Under>,
        _: &mut Option<Box<dyn Any>>,
        _: &Seen<'_>,
    ) -> bool {
        let now = under.and_then(|u| self.control_at(u));
        let moved = now != self.control_under;
        self.control_under = now;
        // AND THE GLOBE IS ASKED FOR AFTER THE FRAME: what the stage has to
        // draw is the box the layout measured, and the layout is this frame.
        self.follow_the_globe();
        moved
    }

    /// A frame of a picture arrived: one more paint is owed.
    fn framed(&mut self) -> bool {
        let mut took = false;
        for p in &mut self.pictures {
            took |= p.took_a_frame();
        }
        took
    }

    /// The view shuts: its picture threads, its hold on the outputs and a
    /// deck's drag go with it.
    fn closing(&mut self, _: &mut dyn Doorway) {
        self.pictures.clear();
        self.sinks = None;
        self.dragged = None;
        self.only = false;
    }
}

/// The glyphs of the player's panel transport, where `drawn` placed them —
/// the one under the pointer and a switch that is on in the accent. What the
/// region paints, and what [`panel_button_at`] answers for, read off the SAME
/// placement.
fn panel_glyphs(
    drawn: &crate::player_media::Drawn,
    state: Option<&ronoctua_player::wire::State>,
    stage: crate::player_window::Stage,
    under: Option<&'static str>,
) -> Vec<ronoctua_render::Span> {
    drawn
        .buttons
        .iter()
        .map(|(line, at, _, id)| {
            let color = if under == Some(*id) || crate::player_transport::lit(id, state, stage) {
                tokens::color::STATE_ACCENT
            } else {
                tokens::color::TEXT_PRIMARY
            };
            ronoctua_render::Span {
                size: ronoctua_render::Size::Grid,
                text: crate::player_transport::glyph_of(
                    id,
                    state,
                    stage,
                    crate::player_window::Door::Footer,
                )
                .to_string(),
                column: *at,
                line: *line,
                color,
                edge: false,
                foot: false,
            }
        })
        .collect()
}

/// The button of the player's panel at `(x, y)`, in a footer measured by
/// `f`: the cell and the line the pixel is in, asked of what `drawn` placed.
#[allow(clippy::cast_possible_truncation)]
fn panel_button_at(
    drawn: &crate::player_media::Drawn,
    f: crate::chrome::Frame,
    x: f64,
    y: f64,
) -> Option<&'static str> {
    crate::player_media::button_at(&drawn.buttons, f.line_at(y as f32)?, f.column_at(x as f32))
}

/// Whether `(x, y)` is on the player's panel time bar, in a footer measured
/// by `f`.
#[allow(clippy::cast_possible_truncation)]
fn panel_track_at(
    drawn: &crate::player_media::Drawn,
    f: crate::chrome::Frame,
    x: f64,
    y: f64,
) -> bool {
    let (Some(track), Some(line)) = (drawn.track, f.line_at(y as f32)) else {
        return false;
    };
    track.holds(x, line, f.cell())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn way() -> crate::region::Recorder {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        crate::region::Recorder::new(
            Rc::new(texts),
            120,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
    }

    /// FAILS IF: a question the region asks on the footer's line loses what
    /// its own line says — masked for a pairing's code, kept out of the log
    /// for the account's user name — so a name typed after the region closed
    /// (the daemon's farewell, `player-quit` from the window) reaches the
    /// journal as the command's argument: the footer asks the LINE, never the
    /// region, which may be gone by then.
    #[test]
    fn the_regions_questions_say_what_stays_out_of_the_log() {
        use crate::player_question::{ACCOUNT_USER, PAIR_CODE, Question, SEARCH_RADIO};
        let asked = |q: Question| {
            let mut w = way();
            ask(q, &mut w);
            w.owed
                .iter()
                .find_map(|e| match e {
                    crate::region::Effect::Ask(line) => Some(line.clone()),
                    _ => None,
                })
                .expect("asked on the footer's line")
        };
        let user = asked(Question::plain(ACCOUNT_USER, false));
        assert!(user.unlogged && !user.secret, "{user:?}");
        assert!(asked(Question::plain(PAIR_CODE, true)).secret);
        assert!(!asked(Question::plain(SEARCH_RADIO, false)).unlogged);
    }

    /// FAILS IF: a footer with NO LOOP — a photograph's, a test's — hands
    /// the view a wake: its reads off the loop would never land, and the
    /// photograph would draw "reading…" where the footer read in place
    /// (the ranking's, the subtitles beside the film, a directory's list);
    /// or a footer with one does not.
    #[test]
    fn a_footer_with_no_loop_reads_in_place() {
        let mut still = way();
        still.has_a_loop = false;
        let region = opened(&mut still);
        assert!(region.view.wake.is_none(), "a wake that wakes nothing");
        assert!(!region.looping);
        let mut live = way();
        let region = opened(&mut live);
        assert!(region.view.wake.is_some(), "a loop's footer reads off it");
        assert!(region.looping);
    }

    /// FAILS IF: a page's command that OPENS the region (`player-radio`
    /// with the view closed) lands the cursor on another row than the same
    /// command with the region already open and laid out — the footer drew
    /// a frame between the two before, and the frame put the panel beside
    /// the rows; a directory's globe among the rows moved the cursor rows
    /// below the one the page opens on.
    #[test]
    fn a_page_opened_by_its_command_is_the_page_a_frame_would_show() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let wide = || {
            let mut w = crate::region::Recorder::new(
                Rc::new(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")),
                160,
                true,
                crate::sudo::Gate::window(),
                crate::wake::Wake::new(|| {}),
            );
            w.has_a_loop = false;
            let resident = make();
            resident
                .as_a::<Kept>()
                .expect("the player's resident")
                .borrow_mut()
                .state = Some(crate::player::fixture::playing());
            w.residence = Some(resident);
            w
        };
        let seen = Seen {
            texts: &texts,
            columns: 160,
            lines: 12,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        // OPENED BY THE PAGE'S COMMAND, as the footer opens a closed region.
        let mut way = wide();
        let mut by_command = opened(&mut way);
        assert!(by_command.panel().is_some(), "no panel beside the rows");
        let _ = by_command.command("player-radio", None, &mut way);
        // AND THE SAME PAGE, the region open and laid out first.
        let mut way = wide();
        let mut laid = opened(&mut way);
        let _ = laid.before_measuring(&seen);
        let _ = laid.command("player-radio", None, &mut way);
        assert_eq!(laid.view.page(), crate::player::PageId::Radio);
        assert_eq!(by_command.view.page(), crate::player::PageId::Radio);
        assert_eq!(by_command.view.cursor(), laid.view.cursor());
    }

    /// FAILS IF: `Esc` under the picture alone gives the rows back without
    /// telling the panel beside them — the fold's glyph on "Now" waited for a
    /// frame of the door's own, and a door that only OWES its frames after a
    /// key (the footer's, its camera) drew the rows without it.
    #[test]
    fn esc_under_only_gives_the_rows_back_with_the_panel_told() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut way = crate::region::Recorder::new(
            Rc::new(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")),
            160,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        );
        way.has_a_loop = false;
        let resident = make();
        resident
            .as_a::<Kept>()
            .expect("the player's resident")
            .borrow_mut()
            .state = Some(crate::player::fixture::playing());
        way.residence = Some(resident);
        let mut player = opened(&mut way);
        let folds = |p: &Player| {
            p.rows_now(&texts).iter().any(|r| {
                r.cells
                    .iter()
                    .any(|c| c.about == Some("player-picture-fold"))
            })
        };
        assert!(
            folds(&player),
            "no fold on \"Now\" beside the panel to begin with"
        );
        let key = |k: &str| ronoctua_keymap::Key::parse(k).expect("a key");
        let _ = player.key(key("o"), &mut way);
        assert!(player.only, "`o` did not leave the picture alone");
        let _ = player.key(key("ESC"), &mut way);
        assert!(!player.only, "`Esc` did not give the rows back");
        assert!(
            folds(&player),
            "the rows came back with no fold on \"Now\": the panel was not told"
        );
    }

    /// FAILS IF: the resident says it hears what is not the player's watch
    /// — the footer would make a doorway for it at every reading of the
    /// machine, ten a second for the sound levels — or it misses the watch,
    /// and a command run with the view closed reads a state that is gone.
    #[test]
    fn the_resident_hears_the_players_watch_alone() {
        use crate::region::Resident as _;
        let resident = make();
        let kept = resident.as_a::<Kept>().expect("the player's resident");
        let state = crate::player_ambience::fixture::rain();
        let heard = ronoctua_player::client::Heard::State(Box::new(state.clone()));
        assert!(kept.borrow().hears(&heard));
        assert!(!kept.borrow().hears(&42_u32), "a reading of the machine");
        resident.hooks.borrow_mut().heard(&heard, &mut way());
        assert_eq!(kept.borrow().state.as_ref(), Some(&state));
    }

    /// FAILS IF: a pairing's name typed with the view closed outlives its
    /// question let go — a code typed later, under another question, would
    /// pair with that machine (the view open lets it go too, `unanswered`).
    #[test]
    fn a_pairing_asked_with_the_view_closed_goes_with_its_question() {
        let resident = make();
        let kept = resident.as_a::<Kept>().expect("the player's resident");
        kept.borrow_mut().pairing.name("the other machine");
        resident.hooks.borrow_mut().let_go("player-pair-code");
        assert_eq!(kept.borrow_mut().pairing.take(), None);
    }

    /// FAILS IF: with Caps Lock on, a text edited in the footer's region
    /// types small letters where the same text in the window types capitals
    /// (the owner's rule: both doors type the same), or the view whose
    /// letters are keys starts reading capitals.
    #[test]
    fn the_region_types_capitals_where_the_window_does() {
        let caps = smithay_client_toolkit::seat::keyboard::Modifiers {
            caps_lock: true,
            ..Default::default()
        };
        let mut way = way();
        let mut region = opened(&mut way);
        assert!(!region.types_capitals(), "a key of the view is no text");
        assert!(crate::door::keys::lock_undone(caps, false));
        region
            .view
            .edit_text(crate::text_area::TextArea::new("about", "words"));
        let typing = region.types_capitals();
        assert!(
            typing,
            "the player's text in the region does not type capitals"
        );
        assert!(
            !crate::door::keys::lock_undone(caps, typing),
            "the Lock was undone"
        );
        assert_eq!(crate::door::keys::without_caps_lock('A', false), 'A');
        // AND A CHORD is still a chord, typing or not.
        let chord = smithay_client_toolkit::seat::keyboard::Modifiers { ctrl: true, ..caps };
        assert!(crate::door::keys::lock_undone(chord, typing));
    }

    /// FAILS IF: a button of the player's media panel is taken anywhere but
    /// where the region paints its glyph — every glyph's cell, read back
    /// through the footer's own frame, is that button and no other — or the
    /// time bar is taken off the quads that draw it.
    #[test]
    fn the_panel_is_taken_where_the_footer_paints_it() {
        use crate::player::fixture::{alone, track};
        use ronoctua_player::wire::{State, TrackKind};
        let f = crate::chrome::Frame::of(1500.0, 900.0, 4);
        let panel = crate::player_media::footer_panel(f.columns(), false).expect("a panel");
        let state = State {
            duration: Some(100.0),
            position: 25.0,
            shuffle: true,
            ..alone(
                "/f/a.mkv",
                vec![
                    track(1, TrackKind::Audio, None, true),
                    track(2, TrackKind::Audio, None, false),
                ],
            )
        };
        let drawn = crate::player_media::drawn(panel, f.first, 12, Some(&state));
        let glyphs = super::panel_glyphs(
            &drawn,
            Some(&state),
            crate::player_window::Stage::Off,
            Some("player-next"),
        );
        assert_eq!(glyphs.len(), drawn.buttons.len());
        assert_eq!(glyphs.len(), crate::player_transport::BUTTONS.len());
        let (cell, line) = (f64::from(f.cell()), f64::from(f.line()));
        for (g, (_, _, _, id)) in glyphs.iter().zip(&drawn.buttons) {
            // THE GLYPH'S OWN CELL AND THE AIR AFTER IT, and the gap beyond.
            for (dx, answer) in [(0.5, Some(*id)), (1.5, Some(*id)), (2.5, None)] {
                let x = f64::from(f.x_of(g.column)) + dx * cell;
                let y = f64::from(f.y_of(g.line)) + line / 2.0;
                assert_eq!(
                    super::panel_button_at(&drawn, f, x, y),
                    answer,
                    "{id} +{dx}"
                );
            }
            // LIT UNDER THE POINTER, and a switch that is on.
            let lit = *id == "player-next" || *id == "player-shuffle";
            assert_eq!(g.color == tokens::color::STATE_ACCENT, lit, "{id}");
        }
        // THE TIME BAR: the quads it is drawn as are where it is taken.
        let t = drawn.track.expect("a time bar");
        let (l, r) = t.px(tokens::metrics::cell_width(tokens::text::size()));
        let quads = crate::chrome::time_bar(l, r, f.y_of(t.line), f.line(), Some(0.25));
        let q = quads[0];
        let y = f64::from(q.y + q.height / 2.0);
        assert!(super::panel_track_at(&drawn, f, f64::from(q.x) + 0.5, y));
        assert!(super::panel_track_at(
            &drawn,
            f,
            f64::from(q.x + q.width) - 0.5,
            y
        ));
        assert!(!super::panel_track_at(&drawn, f, f64::from(q.x) - 0.5, y));
        assert!(!super::panel_track_at(
            &drawn,
            f,
            f64::from(q.x + q.width) + 0.5,
            y
        ));
        assert!(!super::panel_track_at(
            &drawn,
            f,
            f64::from(q.x) + 5.0,
            y - line
        ));
        assert!(
            (quads[1].width - q.width * 0.25).abs() < 0.01,
            "the played part"
        );
    }
}
