//! INCUS IN THE FOOTER'S REGION (`LEADER s x x`, `s x i`, `s x n`) — the view
//! as a [`crate::region::Region`] the footer hosts by its id, beside its
//! window (`LEADER s x w`, [`crate::incus_window`]).
//!
//! # One view, two doors, one errand handler
//!
//! The region holds the view open ([`Open`]) as the window holds it, in the
//! stacked layout the footer's width draws, and hands it every key, click,
//! command, answer and thread's answer; what the view asks for after them
//! goes through [`crate::incus_open::carry_out`] in both doors, over each
//! door's [`Doorway`].
//!
//! # What only a region has to say
//!
//! - A row of several stretches — a kind on the strip, a tab — takes the
//!   click under the pointer's cell, and the pointer lights that stretch and
//!   nothing else ([`Region::clicked_at`], [`Region::lit`], one answer).
//! - A deed outlives its door: closed with one still out, the region QUIETS
//!   its feed (no events, no clock, no watch) and lingers until the answer
//!   came ([`Region::lingers`]) — its sentence is still this door's to say.
//! - The queue's jobs it gave are what the footer heard of the queue
//!   ([`Region::heard`]), told to it when it opens and every time the list
//!   changes.
//!
//! # What a photograph's footer reads
//!
//! The scene its camera's environment names (`incus_feed::SCENE_VARIABLE`,
//! the window camera's `--scene`), answered on the spot, every date in UTC —
//! and what it would queue or let stays in the scene.

use std::cell::Cell;

use crate::incus::{Kind, Press, QUESTIONS, View, press_of, press_of_key};
use crate::incus_feed::{Arrival, Feed, Scene};
use crate::incus_open::Open;
use crate::places::Layout;
use crate::region::{Closed, Doorway, Entry, Lit, Region, Row, Seen, Taken};

/// The id the footer ranks the view under (`surface::PRECEDENCE`).
pub const ID: &str = "incus";

/// The view on the instances (`LEADER s x x`).
const OPEN_COMMAND: &str = "incus-open";
/// The view on the images (`LEADER s x i`).
const IMAGES_COMMAND: &str = "incus-images";
/// A new instance's draft (`LEADER s x n`).
const NEW_COMMAND: &str = "incus-new";
/// Its window (`LEADER s x w`).
const WINDOW_COMMAND: &str = "incus-window";

/// The region the binary registers (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The footer draws the stacked layout, and pages it itself.
const LAYOUT: Layout = Layout::Stacked;

/// What a command of the view does while it is CLOSED: the view on a kind,
/// a new instance's draft and every key by name open it — what a key acts on
/// is the view's; its window opens without it; a question's answer with no
/// view to answer answers nothing.
///
/// IN THIS ORDER, which is [`IncusRegion::command`]'s: four ids are both a
/// key of the view and the question that key asks (`incus-import`,
/// `incus-export`, `incus-push`, `incus-pull`). Typed, the id is an answer;
/// by name with nothing typed — `LEADER :`, `ronoctua-shell command` — it is
/// the key, which opens the view and asks; a question that is no key, by name
/// with nothing typed, answers nothing.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        OPEN_COMMAND | IMAGES_COMMAND | NEW_COMMAND => Closed::Opens,
        WINDOW_COMMAND => {
            way.open_window(vec![ID.to_owned()]);
            Closed::Taken
        }
        question if argument.is_some() && QUESTIONS.contains(&question) => Closed::Taken,
        other if press_of(other).is_some() => Closed::Opens,
        question if QUESTIONS.contains(&question) => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// The kind a command opens the view on.
fn kind_of(command: &str) -> Kind {
    if command == IMAGES_COMMAND {
        Kind::Images
    } else {
        Kind::Instances
    }
}

/// Opens the view on the command's kind: its threads and the daemon's events
/// alive while it is open, and never before — a person who never opens it
/// follows no events of the daemon's. A photograph's footer asks no daemon:
/// its feed answers from the scene its camera names, every date in UTC.
fn open(command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let queue_keys = crate::incus_words::queue_keys();
    let open = if way.photographing() {
        let named =
            std::env::var_os(crate::incus_feed::SCENE_VARIABLE).map(std::path::PathBuf::from);
        let scene = Scene::of_the_footer_camera(named.as_deref());
        Open::on(
            kind_of(command),
            Feed::detached(Some(scene)),
            ronoctua_files::when::Zone::default(),
            queue_keys,
        )
    } else {
        let wake = way.wake();
        Open::on(
            kind_of(command),
            Feed::new(move || wake.call()),
            ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
            queue_keys,
        )
    };
    Box::new(IncusRegion {
        open,
        cursor_at: Cell::new(None),
        closed: false,
    })
}

/// The view, open in the footer's region — or closed with a deed still out.
struct IncusRegion {
    open: Open,
    /// The drawn line the cursor is on — counted where the width is known
    /// (after every call, and whenever the rows are drawn), since the
    /// footer's page asks for it with no width in hand.
    cursor_at: Cell<Option<usize>>,
    /// Closed, its feed quieted: only its deeds' answers still arrive.
    closed: bool,
}

impl IncusRegion {
    /// The cursor's drawn line, counted again at `columns`.
    fn recount(&self, columns: u16, texts: &ronoctua_i18n::Texts) {
        let lines = self.open.lines(columns, LAYOUT, 0, texts);
        self.cursor_at
            .set(self.open.view.cursor_line(&lines, &self.open.readings));
    }

    /// The cursors in step, what is read made what is shown, and the
    /// cursor's line counted — never the page moved.
    fn settle(&mut self, way: &mut dyn Doorway) {
        self.open.settle(LAYOUT, way);
        self.recount(way.columns(), way.texts());
        way.repaint();
    }

    /// A key, pressed — and the cursor's line counted after it.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        self.open.press(press, LAYOUT, way);
        self.recount(way.columns(), way.texts());
    }

    /// A second opening keeps the live view and TURNS it to the kind: the
    /// draft, each kind's row and the tab are kept.
    fn show(&mut self, kind: Kind, way: &mut dyn Doorway) {
        self.open.view.show(kind);
        self.settle(way);
    }
}

impl Region for IncusRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// The stacked layout's lines, each clickable where it has something to
    /// take — which stretch, the click's cell says ([`Region::clicked_at`]).
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        let lines = self.open.lines(seen.columns, LAYOUT, 0, seen.texts);
        self.cursor_at
            .set(self.open.view.cursor_line(&lines, &self.open.readings));
        lines
            .into_iter()
            .map(|l| Row {
                takes: l.zones.iter().any(|z| z.target.acts()),
                fields: l.fields,
            })
            .collect()
    }

    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.open
            .view
            .hint(&self.open.readings, seen.columns, LAYOUT, seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        self.cursor_at.get()
    }

    /// Every key read by its whole notation out of the view's table (and the
    /// consent row's), through the reader the window calls too
    /// (`incus::press_of_key`).
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        match press_of_key(key) {
            Some(press) => {
                self.press(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    /// The region's door on the instances or the images (a second opening
    /// turns the live view to the kind), a new instance's draft, the
    /// window's, the view's keys by name — and a typed question's answer by
    /// name. A command it takes is a gesture by name, after which the page
    /// follows the cursor, as after a key (`Doorway::follow`).
    ///
    /// CLOSED — kept hidden while a deed answers (`Region::lingers`) — it
    /// takes none: a closed view has no page to turn, and the footer steps
    /// it aside so its opening command opens the view afresh
    /// (`region::Slots::set_aside`), while it still says the deed's answer.
    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        if self.closed {
            return Taken::No;
        }
        match id {
            OPEN_COMMAND | IMAGES_COMMAND => self.show(kind_of(id), way),
            NEW_COMMAND => {
                self.show(Kind::Instances, way);
                self.press(Press::Launch, way);
            }
            WINDOW_COMMAND => way.open_window(vec![ID.to_owned()]),
            // A QUESTION'S ID WITH WHAT WAS TYPED: its answer — before the
            // keys, since the ids of four keys are the questions they ask
            // (`closed` says which).
            question if argument.is_some() && QUESTIONS.contains(&question) => {
                if let Some(said) = argument {
                    self.open.answered(question, said, LAYOUT, way);
                    self.recount(way.columns(), way.texts());
                }
            }
            other => match press_of(other) {
                Some(press) => self.press(press, way),
                // A QUESTION'S ID BY NAME, with nothing typed and no key of
                // that name: it answers nothing.
                None if QUESTIONS.contains(&other) => return Taken::Yes,
                None => return Taken::No,
            },
        }
        way.follow();
        Taken::Yes
    }

    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        self.clicked_at(row, None, way);
    }

    /// What is under the pointer's cell on line `row` — a kind on the strip,
    /// a row, a tab, an answer — and the second click is `RET` (invariant 19).
    fn clicked_at(&mut self, row: usize, cell: Option<u16>, way: &mut dyn Doorway) {
        let lines = self.open.lines(way.columns(), LAYOUT, 0, way.texts());
        let again = self
            .open
            .view
            .clicked(&lines, row, cell, LAYOUT, &self.open.readings);
        if again {
            self.press(Press::Enter, way);
        } else {
            self.settle(way);
        }
    }

    /// THE STRETCH A CLICK TAKES — the same zones the click reads
    /// (`View::hover_span`).
    fn lit(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> Lit {
        let lines = self.open.lines(seen.columns, LAYOUT, 0, seen.texts);
        match cell.and_then(|c| View::hover_span(&lines, row, c)) {
            Some((from, to)) => Lit::Cells(from, to),
            None => Lit::Nothing,
        }
    }

    /// The answer typed on the footer's line, under the id that asked — and
    /// the page follows the cursor after it. A closed view answers nothing.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        if self.closed {
            return;
        }
        self.open.answered(id, text, LAYOUT, way);
        self.recount(way.columns(), way.texts());
        way.follow();
    }

    /// The line was let go: what it was for goes with it.
    fn unanswered(&mut self, _id: &'static str, way: &mut dyn Doorway) {
        self.open.view.let_go();
        way.repaint();
    }

    /// The threads brought something: take all of it; a deed's end is said
    /// on the line. CLOSED, only the deeds' answers are taken, and said.
    fn woken(&mut self, way: &mut dyn Doorway) {
        if self.closed {
            for a in self.open.feed.take() {
                if matches!(a, Arrival::Done { .. })
                    && let Some(said) = crate::incus::Readings::default().took(a, way.texts())
                {
                    way.say(said, crate::echo::Echo::Normal);
                }
            }
            return;
        }
        if self.open.take(way) {
            self.settle(way);
        }
    }

    /// The queue's list changed — or is told as the region opens: Incus's
    /// jobs, and a listing again when one of them ended.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if self.closed {
            return;
        }
        if let Some(jobs) = news.downcast_ref::<Vec<ronoctua_transfers::Job>>()
            && self.open.jobs_arrived(jobs)
        {
            self.settle(way);
        }
    }

    /// Closing: the threads, the events and the watch go — and a deed still
    /// out keeps its feed, QUIETED, until it answered ([`Region::lingers`]).
    fn closing(&mut self, _way: &mut dyn Doorway) {
        self.closed = true;
        if self.open.feed.deeds_in_flight() {
            self.open.feed.quiet();
        }
    }

    fn lingers(&self) -> bool {
        self.closed && self.open.feed.deeds_in_flight()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB exists")
    }

    fn way() -> Recorder {
        Recorder::new(
            std::rc::Rc::new(texts()),
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

    /// The region over a scene, as a photograph's footer opens it.
    fn region(command: &str) -> IncusRegion {
        let mut w = way();
        let mut r = IncusRegion {
            open: Open::on(
                kind_of(command),
                Feed::detached(Some(scene())),
                ronoctua_files::when::Zone::default(),
                String::new(),
            ),
            cursor_at: Cell::new(None),
            closed: false,
        };
        assert_eq!(r.command(command, None, &mut w), Taken::Yes);
        r
    }

    /// FAILS IF: a command of the view does to the CLOSED view something
    /// other than what the footer did when it held the view itself.
    #[test]
    fn a_closed_view_opens_for_its_own_commands() {
        let mut w = way();
        for opens in [OPEN_COMMAND, IMAGES_COMMAND, NEW_COMMAND, "incus-down"] {
            assert_eq!(closed(opens, None, &mut w), Closed::Opens, "{opens}");
        }
        assert_eq!(closed(WINDOW_COMMAND, None, &mut w), Closed::Taken);
        assert_eq!(w.owed, vec![Effect::OpenWindow(vec!["incus".to_owned()])]);
        let question = QUESTIONS.first().copied().expect("a question");
        assert_eq!(closed(question, Some("x"), &mut w), Closed::Taken);
        assert_eq!(closed("stick-open", None, &mut w), Closed::NotMine);
    }

    /// FAILS IF: a key whose id is also the question it asks
    /// (`incus-import`, `incus-export`, `incus-push`, `incus-pull`), run by
    /// name with nothing typed, does not press the key — opening the closed
    /// view, acting in the open one — or a typed answer under that id is
    /// taken for the key, or a question that is no key, by name with nothing
    /// typed, does anything.
    #[test]
    fn a_key_named_as_its_question_is_the_key_until_something_is_typed() {
        use crate::incus::{EXPORT, IMPORT, NAME, PULL, PUSH};
        let mut w = way();
        for both in [IMPORT, EXPORT, PUSH, PULL] {
            assert!(
                QUESTIONS.contains(&both) && press_of(both).is_some(),
                "`{both}` is not a key and a question"
            );
            assert_eq!(closed(both, None, &mut w), Closed::Opens, "{both}");
            assert_eq!(closed(both, Some("x"), &mut w), Closed::Taken, "{both}");
        }
        assert_eq!(closed(NAME, None, &mut w), Closed::Taken);

        let mut r = region(OPEN_COMMAND);
        let mut w = way();
        assert_eq!(r.command(IMPORT, None, &mut w), Taken::Yes);
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Ask(l) if l.id == IMPORT)),
            "`{IMPORT}` by name did not ask what to import: {:?}",
            w.owed
        );
        for both in [EXPORT, PUSH, PULL] {
            let mut w = way();
            assert_eq!(r.command(both, None, &mut w), Taken::Yes, "{both}");
            assert!(
                w.owed
                    .iter()
                    .any(|e| matches!(e, Effect::Ask(_) | Effect::Say(..))),
                "`{both}` by name with the view open said nothing: {:?}",
                w.owed
            );
        }
        let mut w = way();
        assert_eq!(r.command(NAME, None, &mut w), Taken::Yes);
        assert!(
            w.owed.is_empty(),
            "a question by name with nothing typed did something: {:?}",
            w.owed
        );
    }

    /// FAILS IF: `incus-images` does not open on the images, or a second
    /// opening on another kind does not turn the live view to it.
    #[test]
    fn an_opening_turns_the_view_to_its_kind() {
        let mut r = region(IMAGES_COMMAND);
        assert_eq!(r.open.view.kind(), Kind::Images);
        let mut w = way();
        assert_eq!(r.command(OPEN_COMMAND, None, &mut w), Taken::Yes);
        assert_eq!(r.open.view.kind(), Kind::Instances);
        let r = region(NEW_COMMAND);
        assert!(r.open.view.drafting(), "`incus-new` opens the draft");
    }

    /// FAILS IF: the region closed with nothing out lingers, or one closed
    /// says the deeds' answers — and only those — anywhere but on the line.
    #[test]
    fn a_closed_region_lingers_only_for_a_deed() {
        let mut r = region(OPEN_COMMAND);
        let mut w = way();
        r.closing(&mut w);
        assert!(!r.lingers(), "a photograph's deeds answer on the spot");
        r.woken(&mut w);
        assert!(
            w.owed.iter().all(|e| matches!(e, Effect::Say(..))),
            "a closed region only says: {:?}",
            w.owed
        );
    }

    /// FAILS IF: the pointer lights over the strip something other than what
    /// a click there takes.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: 100,
            lines: 20,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let r = region(OPEN_COMMAND);
        let lines = r.open.lines(100, LAYOUT, 0, &t);
        let (row, stretch) = lines
            .iter()
            .enumerate()
            .find_map(|(n, l)| {
                l.zones
                    .iter()
                    .find(|z| matches!(z.target, crate::incus::Target::Kind(_)))
                    .map(|z| (n, (z.from, z.to)))
            })
            .expect("the strip of kinds is drawn");
        assert_eq!(
            r.lit(row, Some(stretch.0), &seen),
            Lit::Cells(stretch.0, stretch.1)
        );
        assert_eq!(r.lit(row, None, &seen), Lit::Nothing);
    }
}
