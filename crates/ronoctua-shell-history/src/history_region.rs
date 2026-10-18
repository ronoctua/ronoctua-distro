//! THE SHELL HISTORY'S VIEW IN THE FOOTER'S REGION — the second door onto
//! [`crate::history`], beside its window ([`crate::history_window`]), and the
//! one errand handler both doors carry out ([`carry_out`]).
//!
//! # What it reads, and through what
//!
//! The footer's own Atuin thread and its last reading — the modeline's ring
//! reads them too — lent by type ([`crate::region::kept`]). The thread is
//! RAISED ON THE FIRST OPEN and kept: `atuin status` is a process, not a
//! socket, and a machine that never opens this view must not pay a fork
//! every thirty seconds for it. So the region asks the footer for it as it
//! opens ([`crate::region::want`]), and the footer keeps it for the session.

use std::rc::Rc;

use crate::history::{Errand, Facts, Press, Question, View, press_of, press_of_key};
use crate::region::{self, Closed, Doorway, Entry, Region, Row, Seen, Taken};
use ronoctua_atuin::{Hand, Order, Snapshot};

/// The region's id, as the footer ranks it (`surface::PRECEDENCE`).
pub const ID: &str = "history";

/// The view's region, as the binary registers it (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The view's own gestures, by name: they press a key of the open view, and
/// do nothing with the view shut.
const GESTURES: &[&str] = &[
    "atuin-down",
    "atuin-up",
    "atuin-enter",
    "atuin-more",
    "atuin-less",
    "atuin-back",
];

/// What a command does while the view is closed.
fn closed(command: &str, _: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "atuin-open" => Closed::Opens,
        "atuin-window" => {
            way.open_window(vec!["history".to_owned()]);
            Closed::Taken
        }
        "atuin-sync" => {
            let hand = region::kept::<Hand>(way);
            ask(Order::Sync, hand.as_deref(), way);
            Closed::Taken
        }
        // A GESTURE OF A CLOSED VIEW does nothing, and neither does an
        // answer to a question whose view is gone — taken, and not handed
        // on to be called a command nobody answers.
        id if GESTURES.contains(&id) || Question::named(id).is_some() => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// Opens the view on the footer's last reading — and asks the footer to
/// raise its Atuin thread, the first time.
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    region::want::<Hand>(way);
    // WHAT THE FILES SAY, before the thread has said anything: a door that
    // keeps none reads them now (`at_rest`), and never opens on "atuin is not
    // installed" a moment before the thread says otherwise.
    let shot = region::kept::<Snapshot>(way).unwrap_or_else(|| Rc::new(ronoctua_atuin::at_rest()));
    Box::new(History {
        view: View::fresh(),
        shot,
    })
}

/// One order to Atuin's thread — and said, when Atuin is not here.
fn ask(order: Order, hand: Option<&Hand>, way: &mut dyn Doorway) {
    match hand {
        Some(h) => h.send(order),
        None => {
            let said = way.texts().t(crate::key::ATUIN_ABSENT);
            way.say(said, crate::echo::Echo::Normal);
        }
    }
}

/// The view, open in the footer's region.
struct History {
    view: View,
    /// The footer's last reading — the same `Rc` the footer holds.
    shot: Rc<Snapshot>,
}

impl History {
    /// One press — a key, a gesture by name, a second click — and what it
    /// asks for, carried out.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = {
            let facts = Facts { shot: &self.shot };
            self.view.key(press, &facts, way.columns(), way.texts())
        };
        way.follow();
        self.carried(errand, way);
    }

    /// What the view asked for, carried out through the footer's thread.
    fn carried(&mut self, errand: Errand, way: &mut dyn Doorway) {
        let hand = region::kept::<Hand>(way);
        carry_out(errand, &mut self.view, hand.as_deref(), way);
    }
}

impl Region for History {
    fn id(&self) -> &'static str {
        ID
    }

    /// THE SHELL'S HISTORY, through its own view — its rows already resolved
    /// their own columns, so the region only places them; every row takes a
    /// click.
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        let facts = Facts { shot: &self.shot };
        self.view
            .rows(&facts, seen.columns, seen.texts)
            .into_iter()
            .map(|r| Row {
                takes: true,
                fields: r.fields,
            })
            .collect()
    }

    /// `s` syncs and `S-<arrows>` turn a value, and nothing on a row says
    /// so.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view.hint(seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// BOTH THE SHIFTED ARROWS AND THE BARE LETTERS THROUGH ONE TABLE, by
    /// the reader the window calls too: with the view open the whole
    /// keyboard is its, and `s` syncs rather than starting a leader
    /// sequence.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let Some(press) = press_of_key(key) else {
            return Taken::No;
        };
        self.press(press, way);
        Taken::Yes
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // ALREADY OPEN: left exactly as it is, and the page follows its
            // cursor.
            "atuin-open" => way.follow(),
            "atuin-window" => way.open_window(vec!["history".to_owned()]),
            "atuin-sync" => {
                let hand = region::kept::<Hand>(way);
                ask(Order::Sync, hand.as_deref(), way);
            }
            id if GESTURES.contains(&id) => {
                if let Some(press) = press_of(id) {
                    self.press(press, way);
                }
            }
            // WHAT WAS TYPED comes back under the question's id, and the VIEW
            // is what knows which question it answers — the login's four, one
            // at a time, or a setting's value.
            asked if Question::named(asked).is_some() => {
                let errand = self.view.answered(asked, argument.unwrap_or_default());
                self.carried(errand, way);
            }
            _ => return Taken::No,
        }
        Taken::Yes
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19: the first
    /// click on a ring does not turn it, and the first on "log out" does not
    /// log out.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let again = {
            let facts = Facts { shot: &self.shot };
            self.view.clicked(row, &facts, way.columns(), way.texts())
        };
        if again {
            self.press(Press::Enter, way);
        } else {
            way.repaint();
        }
    }

    /// What was typed for the view's question, under its id.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        let errand = self.view.answered(id, text);
        self.carried(errand, way);
    }

    /// The line was let go: what the view asked goes with it — and a
    /// password typed for the login.
    fn unanswered(&mut self, _: &'static str, way: &mut dyn Doorway) {
        self.view.let_go();
        way.repaint();
    }

    /// It says when its rows came or went (`Doorway::rows_changed`): a frame
    /// it asks for on news is only a frame.
    fn says_when_its_rows_change(&self) -> bool {
        true
    }

    /// What the footer heard: a fresh reading, or the key the thread was
    /// asked for.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(fresh) = news.downcast_ref::<Rc<Snapshot>>() {
            self.shot = Rc::clone(fresh);
            // A LOGIN OR A LOG-OUT changes which rows there are.
            way.rows_changed();
        } else if let Some(ronoctua_atuin::Event::Key(key)) =
            news.downcast_ref::<ronoctua_atuin::Event>()
        {
            // THE KEY, WHICH WAS ASKED FOR AND IS SHOWN ONCE — see
            // `history.rs` for why it is shown at all: its rows come.
            self.view.took_the_key(key.clone());
            way.rows_changed();
        }
    }
}

/// CARRIES OUT WHAT THE VIEW ASKED FOR, in either door — the footer's region
/// and the window — each with its view and the line it asks Atuin through
/// (`hand`: the footer's own thread, or the window's).
///
/// One handler, so the two doors cannot do one errand two ways: what is said
/// is said in the door it was asked in, for as long as its class says.
pub fn carry_out(errand: Errand, view: &mut View, hand: Option<&Hand>, way: &mut dyn Doorway) {
    match errand {
        Errand::Nothing => {}
        Errand::Close => way.close(),
        Errand::Order(order) => ask(order, hand, way),
        // THE KEY IS FETCHED HERE, because `atuin key` is a process and the
        // door's loop is one thread — but it is a single short call, not a
        // wait on a socket.
        Errand::ShowKey => match ronoctua_atuin::cli::key() {
            Ok(key) => view.took_the_key(key),
            Err(e) => {
                let said = way.texts().t_with(
                    crate::key::ECHO_ATUIN_REFUSED,
                    &[("reason", &e.to_string())],
                );
                way.say(said, crate::echo::Echo::Normal);
            }
        },
        // ON THIS DOOR'S LINE — the footer's minibuffer, the window's own —
        // hidden when the view says so, which also keeps the answer out of
        // the log; the answer comes back to `answered`.
        Errand::Ask(question) => {
            way.ask(crate::line::Line::new(question.id, question.secret, None));
        }
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

    /// FAILS IF: opening the view does not ask the footer to raise its Atuin
    /// thread — the region would open on a reading nobody refreshes, and
    /// the modeline's ring would never move — or a door that keeps no
    /// reading opens on "atuin is not installed".
    #[test]
    fn opening_the_view_asks_for_the_footers_thread() {
        let mut way = footer(Vec::new());
        let opened = open("atuin-open", None, &mut way);
        assert_eq!(opened.id(), ID);
        assert_eq!(
            way.owed,
            vec![region::Effect::Want(std::any::TypeId::of::<Hand>())]
        );
    }

    /// FAILS IF: one of the view's commands does something else with the
    /// view shut than the footer did — a sync with no Atuin said, a gesture
    /// and an answer to a question whose view is gone taken and nothing done
    /// — or a command that is not the view's is taken for it.
    #[test]
    fn a_closed_view_answers_its_own_commands_and_no_other() {
        let mut way = footer(Vec::new());
        assert_eq!(closed("atuin-open", None, &mut way), Closed::Opens);
        assert_eq!(closed("atuin-sync", None, &mut way), Closed::Taken);
        let said = texts().t(crate::key::ATUIN_ABSENT);
        assert_eq!(
            way.owed,
            vec![region::Effect::Say(said, crate::echo::Echo::Normal)]
        );
        way.owed.clear();
        for gesture in GESTURES {
            assert!(press_of(gesture).is_some(), "{gesture} presses nothing");
            assert_eq!(closed(gesture, None, &mut way), Closed::Taken);
        }
        for q in Question::ALL {
            assert_eq!(closed(q.id, Some("x"), &mut way), Closed::Taken);
        }
        assert!(way.owed.is_empty(), "{:?}", way.owed);
        assert_eq!(closed("atuin-window", None, &mut way), Closed::Taken);
        assert_eq!(
            way.owed,
            vec![region::Effect::OpenWindow(vec!["history".to_owned()])]
        );
        assert_eq!(closed("syncthing-open", None, &mut way), Closed::NotMine);
    }

    /// FAILS IF: a question the view asks is put on a door's line under
    /// another id, or shown when it is a secret — in the footer's door and
    /// in the window's alike.
    #[test]
    fn a_question_is_asked_alike_in_both_doors() {
        let mut view = View::fresh();
        let mut way = footer(Vec::new());
        carry_out(Errand::Ask(Question::PASSWORD), &mut view, None, &mut way);
        assert!(matches!(
            way.owed.first(),
            Some(region::Effect::Ask(line)) if line.id == Question::PASSWORD.id && line.secret
        ));
        let mut ui = crate::door::Ui::for_tests(texts());
        carry_out(Errand::Order(Order::Sync), &mut view, None, &mut ui);
        assert_eq!(
            ui.message.as_deref(),
            Some(texts().t(crate::key::ATUIN_ABSENT).as_str())
        );
    }

    /// FAILS IF: a reading or the key asked for, told by the type the footer
    /// tells each (`Rc<Snapshot>`, `ronoctua_atuin::Event`:
    /// `surface/mod.rs`), is not the one the region takes — the open view
    /// would freeze on the old login — or either does not say its rows may
    /// have come or gone, or a news of a type it does not read asks
    /// anything.
    #[test]
    fn what_the_footer_tells_is_heard_by_its_type() {
        let mut way = footer(Vec::new());
        let mut history = History {
            view: View::fresh(),
            shot: Rc::new(Snapshot::default()),
        };
        let fresh = Rc::new(Snapshot::default());
        Region::heard(&mut history, &fresh, &mut way);
        assert!(
            Rc::ptr_eq(&history.shot, &fresh),
            "the reading told is not the one drawn"
        );
        assert_eq!(way.owed, vec![region::Effect::RowsChanged]);
        way.owed.clear();
        Region::heard(
            &mut history,
            &ronoctua_atuin::Event::Key("the key".to_owned()),
            &mut way,
        );
        assert_eq!(way.owed, vec![region::Effect::RowsChanged]);
        way.owed.clear();
        Region::heard(&mut history, &(), &mut way);
        assert!(way.owed.is_empty(), "{:?}", way.owed);
    }
}
