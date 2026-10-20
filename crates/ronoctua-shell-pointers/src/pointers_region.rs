//! THE POINTERS' VIEW IN THE FOOTER'S REGION — the second door onto
//! [`crate::pointers`], beside its window ([`crate::pointers_window`]), and
//! the one errand handler both doors carry out ([`carry_out`]).
//!
//! # What it reads, and through what
//!
//! The footer's own pointer thread — alive for the whole session, the one
//! that DETECTS and so gives a mouse plugged in with no view open a cursor of
//! its own — and that thread's last reading, both lent by their type
//! ([`crate::region::kept`]). The region asks through the thread that already
//! detects and never starts a second one: two threads numbering one new
//! mouse would race. A fresh reading reaches it as news
//! ([`Region::heard`]), and the cursor stays on its row by identity.

use std::rc::Rc;

use crate::pointers::{Errand, Press, View, press_of, press_of_key};
use crate::region::{self, Closed, Doorway, Entry, Region, Row, Seen, Taken};
use ronoctua_pointers::{Handle, Order, Snapshot};

/// The region's id, as the footer ranks it (`surface::PRECEDENCE`).
pub const ID: &str = "pointers";

/// The view's region, as the binary registers it (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The commands that are the view's own gestures, by name: they press a key
/// of the open view, and do nothing with the view shut.
const GESTURES: &[&str] = &[
    "pointers-up",
    "pointers-down",
    "pointers-enter",
    "pointers-less",
    "pointers-more",
    "pointers-forget",
];

/// What a command does while the view is closed.
fn closed(command: &str, _: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "pointers-open" => Closed::Opens,
        "pointers-window" => {
            way.open_window(vec!["pointers".to_owned()]);
            Closed::Taken
        }
        // A GESTURE OF A CLOSED VIEW does nothing, which is what it means —
        // taken, and not handed on to be called a command nobody answers.
        id if GESTURES.contains(&id) => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// Opens the view on the footer's last reading — and asks the thread for a
/// fresh one, so the view does not open on what it saw minutes ago.
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let snapshot = region::kept::<Snapshot>(way);
    let mut view = View::fresh();
    view.settle_in(snapshot.as_deref(), way.columns(), way.texts());
    if let Some(h) = region::kept::<Handle>(way) {
        h.send(Order::Look);
    }
    Box::new(Pointers { view, snapshot })
}

/// The view, open in the footer's region, and the reading it draws.
struct Pointers {
    view: View,
    /// The footer's last reading — the same `Rc` the footer holds.
    snapshot: Option<Rc<Snapshot>>,
}

impl Pointers {
    /// One press — a key, a gesture by name, a second click — and what it
    /// asks for, carried out.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = self
            .view
            .key(press, self.snapshot.as_deref(), way.columns(), way.texts());
        way.follow();
        // THE REGION IS THE VIEW'S WHILE IT IS OPEN, so a row that runs a
        // command hands it back first, as a notification's command does:
        // what the command opens would find the region taken. A window is no
        // region, and keeps its window open (`carry_out`).
        if matches!(errand, Errand::Run(_)) {
            way.close();
        }
        let orders = region::kept::<Handle>(way);
        carry_out(errand, orders.as_deref(), way);
    }
}

impl Region for Pointers {
    fn id(&self) -> &'static str {
        ID
    }

    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.view
            .rows(self.snapshot.as_deref(), seen.columns, seen.texts)
            .into_iter()
            .map(|r| Row {
                // A HEADING IS NOT A ROW TO CLICK ON — the row's own answer
                // (`pointers::Row::takes`), which the window gives too.
                takes: r.takes(),
                fields: r.fields,
            })
            .collect()
    }

    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view
            .hint(self.snapshot.as_deref(), seen.columns, seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// BOTH DOORS THROUGH ONE READER — `pointers::press_of_key`.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let Some(press) = press_of_key(key) else {
            return Taken::No;
        };
        self.press(press, way);
        Taken::Yes
    }

    fn command(&mut self, id: &str, _: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // ALREADY OPEN: left exactly as it is, and the page follows its
            // cursor — a door onto a room you are in does not rearrange it.
            "pointers-open" => way.follow(),
            "pointers-window" => way.open_window(vec!["pointers".to_owned()]),
            id if GESTURES.contains(&id) => {
                if let Some(press) = press_of(id) {
                    self.press(press, way);
                }
            }
            _ => return Taken::No,
        }
        Taken::Yes
    }

    /// THE MOUSE AIMS, AND THE SECOND CLICK ACTS — invariant 19.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let rows = self
            .view
            .rows(self.snapshot.as_deref(), way.columns(), way.texts());
        if self.view.clicked(row, &rows) {
            self.press(Press::Enter, way);
        } else {
            way.repaint();
        }
    }

    /// It says when its rows came or went (`Doorway::rows_changed`): a frame
    /// it asks for on news is only a frame.
    fn says_when_its_rows_change(&self) -> bool {
        true
    }

    /// A fresh reading of the footer's thread.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        let Some(fresh) = news.downcast_ref::<Rc<Snapshot>>() else {
            return;
        };
        let columns = way.columns();
        // THE CURSOR STAYS ON ITS ROW BY IDENTITY — a device plugged in adds
        // rows above it, and an index would drift.
        let spot = self
            .view
            .rows(self.snapshot.as_deref(), columns, way.texts())
            .get(self.view.cursor())
            .map(|r| r.spot.clone());
        self.snapshot = Some(Rc::clone(fresh));
        if let Some(spot) = spot {
            self.view
                .follow(&spot, self.snapshot.as_deref(), columns, way.texts());
        }
        self.view
            .settle_in(self.snapshot.as_deref(), columns, way.texts());
        // A DEVICE PLUGGED IN OR PULLED OUT brings or takes its rows.
        way.rows_changed();
    }
}

/// CARRIES OUT WHAT THE VIEW ASKED FOR, in either door — the footer's region
/// and the window — each with the line it asks the pointer thread through
/// (`orders`: the footer's own thread, or the window's).
///
/// One handler, so the two doors cannot do one errand two ways: what is said
/// is said in the door it was asked in, for as long as its class says.
pub fn carry_out(errand: Errand, orders: Option<&Handle>, way: &mut dyn Doorway) {
    match errand {
        Errand::Nothing => {}
        // AN INFORMATIVE ROW COPIES ITSELF — `chrome::information_of`, and
        // the word that says so is a glance (`Echo::Brief`).
        Errand::Copy(text) => {
            let said = crate::chrome::copied(&text, way.texts());
            way.say(said, crate::echo::Echo::Brief);
        }
        Errand::Order(order) => match orders {
            Some(h) => h.send(order),
            None => {
                let said = way.texts().t(crate::key::POINTERS_NO_COMPOSITOR);
                way.say(said, crate::echo::Echo::Normal);
            }
        },
        Errand::Close => way.close(),
        // A COMMAND OF THE REGISTRY, by the one door every command enters:
        // the footer runs it, and a window sends it down the control socket
        // (the keyboards' buffer, which the footer opens in either door) — a
        // footer that is not there is said.
        Errand::Run(id) => way.run(id, None),
    }
    way.repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    /// The footer's doorway, lending `kept`.
    fn footer(kept: Vec<region::Kept>) -> region::Recorder {
        region::Recorder::new(
            Rc::new(texts()),
            100,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
        .keeping(kept)
    }

    /// FAILS IF: a command of the view does something else with the view
    /// closed than it did in the footer — the open command opens, the window
    /// opens a window, a gesture is taken and does nothing — or a command
    /// that is not the view's is taken for it.
    #[test]
    fn a_closed_view_answers_its_own_commands_and_no_other() {
        let mut way = footer(Vec::new());
        assert_eq!(closed("pointers-open", None, &mut way), Closed::Opens);
        assert_eq!(closed("pointers-window", None, &mut way), Closed::Taken);
        assert_eq!(
            way.owed,
            vec![region::Effect::OpenWindow(vec!["pointers".to_owned()])]
        );
        for gesture in GESTURES {
            assert!(press_of(gesture).is_some(), "{gesture} presses nothing");
            assert_eq!(closed(gesture, None, &mut way), Closed::Taken);
        }
        assert_eq!(closed("audio-open", None, &mut way), Closed::NotMine);
        assert_eq!(
            way.owed.len(),
            1,
            "a gesture of a closed view asked something"
        );
    }

    /// FAILS IF: the region opens on something other than the footer's last
    /// reading, or does not ask the footer's thread for a fresh one.
    #[test]
    fn the_region_opens_on_the_footers_reading_and_asks_for_a_fresh_one() {
        let snapshot: Rc<Snapshot> = Rc::new(Snapshot::default());
        let mut way = footer(vec![snapshot.clone()]);
        let opened = open("pointers-open", None, &mut way);
        assert_eq!(opened.id(), ID);
        let seen = Seen {
            texts: way.texts(),
            columns: 100,
            lines: 20,
            zone: &ronoctua_files::when::Zone::utc(),
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let drawn: Vec<String> = opened
            .rows(&seen)
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        let mut alone = View::fresh();
        alone.settle_in(Some(&snapshot), 100, way.texts());
        let wanted: Vec<String> = alone
            .rows(Some(&snapshot), 100, way.texts())
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert_eq!(drawn, wanted);
    }

    /// FAILS IF: an order with no thread to carry it is swallowed, a copy is
    /// not said as the word it is, or a row's command is not run through the
    /// door — in the footer's door and in the window's alike.
    #[test]
    fn an_errand_is_carried_out_alike_in_both_doors() {
        let mut way = footer(Vec::new());
        carry_out(Errand::Order(Order::Look), None, &mut way);
        carry_out(Errand::Run("keyboard-open"), None, &mut way);
        assert!(matches!(
            way.owed.first(),
            Some(region::Effect::Say(_, crate::echo::Echo::Normal))
        ));
        assert!(
            way.owed
                .contains(&region::Effect::Run("keyboard-open", None))
        );
        let mut ui = crate::door::Ui::for_tests(texts());
        carry_out(Errand::Order(Order::Look), None, &mut ui);
        assert_eq!(
            ui.message.as_deref(),
            Some(texts().t(crate::key::POINTERS_NO_COMPOSITOR).as_str())
        );
        carry_out(Errand::Run("keyboard-open"), None, &mut ui);
        assert_eq!(ui.orders(), &["command keyboard-open".to_owned()]);
        assert!(!ui.quit, "running a row's command closed the window");
    }

    /// FAILS IF: a reading told by the type the footer tells it
    /// (`Rc<Snapshot>`: `surface/mod.rs`) is not the one the region takes —
    /// an unplugged mouse would keep its row — or it does not say its rows
    /// may have come or gone, or a news of a type it does not read asks
    /// anything.
    #[test]
    fn what_the_footer_tells_is_heard_by_its_type() {
        let mut way = footer(Vec::new());
        let mut pointers = Pointers {
            view: View::fresh(),
            snapshot: None,
        };
        let fresh = Rc::new(Snapshot::default());
        Region::heard(&mut pointers, &fresh, &mut way);
        assert!(
            pointers
                .snapshot
                .as_ref()
                .is_some_and(|s| Rc::ptr_eq(s, &fresh)),
            "the reading told is not the one drawn"
        );
        assert_eq!(way.owed, vec![region::Effect::RowsChanged]);
        way.owed.clear();
        Region::heard(&mut pointers, &(), &mut way);
        assert!(way.owed.is_empty(), "{:?}", way.owed);
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a title, the air between sections, a
    /// sentence, a cursor's or a device's name — or leaves unlit one it
    /// takes. THE LIGHT IS THE CLICK: every row clicked once, on a view of
    /// its own over the same laptop (`pointers::a_laptop`), and lit exactly
    /// where something happened.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: 100,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        // AS `open` OPENS IT on the footer's reading — a view of its own.
        let laptop = Rc::new(crate::pointers::a_laptop());
        let fresh = || {
            let mut view = View::fresh();
            view.settle_in(Some(&*laptop), 100, &t);
            let opened = Pointers {
                view,
                snapshot: Some(Rc::clone(&laptop)),
            };
            (opened, footer(Vec::new()))
        };
        let held = crate::region::held_to_the_click(fresh, &seen, |r, w: &region::Recorder| {
            crate::region::seen(r, &w.owed, &seen)
        });
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }
}
