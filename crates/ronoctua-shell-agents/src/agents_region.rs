//! THE LLM ACCOUNTS IN THE FOOTER'S REGION (`LEADER m a a`) — the view as a
//! [`crate::region::Region`] the footer hosts by its id, beside its window
//! (`LEADER m a w`, [`crate::agents_window`]).
//!
//! # One view, two doors, one errand handler
//!
//! The region holds the view [`Open`] as the window holds it, and hands it
//! every key, click, command, answer and thread's answer; what the view asks
//! for after them goes through [`crate::agents::carry_out`] in both doors,
//! over each door's [`Doorway`]. A question is asked on the door's own line
//! (`Doorway::ask`) and its answer comes back to the region that asked.
//!
//! # The one clock
//!
//! A window that resets is "not known" from then on, and no file says so:
//! the region reads again once the nearest reset shown is past, asked on the
//! second the footer already beats ([`Region::tick`]) — no timer of its own,
//! and none at all once it is closed.
//!
//! # What a photograph's footer reads
//!
//! The accounts on the spot (`agents::read`), and no thread, no watch and no
//! refresh: a region whose door is a photograph's reaches no network.

use crate::agents::{
    LOGIN_COMMAND, OPEN_COMMAND, Open, Reach, Reader, SESSION_COMMAND, View, is_command, place_of,
    press_of_key, session_from_anywhere, window_words,
};
use crate::region::{Closed, Doorway, Entry, Region, Row, Seen, Taken};

/// The id the footer ranks the view under (`surface::PRECEDENCE`).
pub const ID: &str = "agents";

/// The command that opens the view's window (`LEADER m a w`).
const WINDOW_COMMAND: &str = "agents-window";

/// The region the binary registers (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// What a command of the view does while it is CLOSED: the view (a mark's
/// click hands it the account's place) and every command of the view open
/// it — then the open region takes the command; its window opens without
/// it, and a session and a login from anywhere open in the terminal.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        OPEN_COMMAND => Closed::Opens,
        WINDOW_COMMAND => {
            way.open_window(window_words(argument));
            Closed::Taken
        }
        SESSION_COMMAND | LOGIN_COMMAND => {
            session_from_anywhere(command == LOGIN_COMMAND, place_of(argument), way);
            Closed::Taken
        }
        other if is_command(other) => Closed::Opens,
        _ => Closed::NotMine,
    }
}

/// Opens the view: what it shows read off the loop, and a refresh asked at
/// once — the numbers are fresh when the person looks; a photograph's read
/// on the spot. A place (a mark's digit, `agents-open 3`) puts the cursor on
/// that account once the first reading is here.
fn open(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let place = if command == OPEN_COMMAND {
        place_of(argument)
    } else {
        None
    };
    let reader = if way.photographing() {
        Reader::Here(crate::agents::read)
    } else {
        Reader::Thread(way.wake())
    };
    let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
    Box::new(AgentsRegion {
        open: Open::new(
            zone,
            reader,
            Reach::Machine,
            place,
            way.columns(),
            way.texts(),
        ),
    })
}

/// The view, open in the footer's region.
struct AgentsRegion {
    open: Open,
}

impl Region for AgentsRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// Every row a click that selects and a second that acts, each saying
    /// under the pointer what its `RET` does.
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.open
            .rows(seen.columns, seen.texts)
            .into_iter()
            .map(|r| Row {
                takes: r.takes(),
                fields: r.fields,
            })
            .collect()
    }

    /// The keys that work where the cursor is — an account's only on one,
    /// the question's while it is up.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.open.view.hint(
            &self.open.readings,
            &self.open.zone,
            seen.columns,
            seen.texts,
        )
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.open.view.cursor())
    }

    fn about(&self, row: usize, seen: &Seen<'_>) -> Option<String> {
        let rows = self.open.rows(seen.columns, seen.texts);
        View::about(&rows, row, seen.texts)
    }

    /// Read by the whole notation through the reader the window calls too
    /// (`agents::press_of_key`).
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        match press_of_key(key) {
            Some(press) => {
                self.open.press(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    /// The view (a place aims at that account), its window, a session and a
    /// login — of the account under the cursor, or of the place given — and
    /// every command of the view: a key, a row, a question's answer. A
    /// command it takes is a gesture by name, after which the page follows
    /// the cursor, as after a key (`Doorway::follow`).
    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            OPEN_COMMAND => {
                if let Some(place) = place_of(argument) {
                    self.open.aim_at(place, way.columns(), way.texts());
                    way.repaint();
                }
            }
            WINDOW_COMMAND => way.open_window(window_words(argument)),
            SESSION_COMMAND | LOGIN_COMMAND => {
                let login = id == LOGIN_COMMAND;
                match place_of(argument) {
                    None => self.open.press(
                        if login {
                            crate::agents::Press::Login
                        } else {
                            crate::agents::Press::Session
                        },
                        way,
                    ),
                    place => session_from_anywhere(login, place, way),
                }
            }
            other if is_command(other) => self.open.command(other, argument, way),
            _ => return Taken::No,
        }
        way.follow();
        Taken::Yes
    }

    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        self.open.clicked(row, way);
    }

    /// What was typed on the footer's line: the same `View::answered` the
    /// window's line reaches — and the page follows the cursor after it.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        self.open.answered(id, text, way);
        way.follow();
    }

    /// The line was let go, and the view forgets the question.
    fn unanswered(&mut self, _id: &'static str, way: &mut dyn Doorway) {
        self.open.view.let_go();
        way.repaint();
    }

    /// What a thread or a watch brought: a reading, a refresh, a stir.
    fn woken(&mut self, way: &mut dyn Doorway) {
        self.open.heard(way);
    }

    /// The footer's second: a reading asked once the nearest reset shown is
    /// past. What it reads arrives on a thread, so nothing changed yet.
    fn tick(&mut self, _now: std::time::Instant, _way: &mut dyn Doorway) -> bool {
        self.open.ring_if_due(crate::when::now_s());
        false
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

    /// A machine with no account yet — never this machine's.
    fn planted(_: &ronoctua_files::when::Zone) -> crate::agents::Readings {
        crate::agents::Readings {
            read: true,
            ..crate::agents::Readings::default()
        }
    }

    fn region() -> AgentsRegion {
        AgentsRegion {
            open: Open::new(
                ronoctua_files::when::Zone::utc(),
                Reader::Here(planted),
                Reach::Nowhere(Vec::new()),
                None,
                100,
                &texts(),
            ),
        }
    }

    /// FAILS IF: a command of the view does to the CLOSED view something
    /// other than what the footer did when it held the view itself — the
    /// view and its commands open it, its window opens without it (with the
    /// place a mark gave), and another view's command is not its.
    #[test]
    fn a_closed_view_opens_for_its_own_commands() {
        let mut w = way();
        assert_eq!(closed(OPEN_COMMAND, Some("2"), &mut w), Closed::Opens);
        assert_eq!(closed("agents-refresh", None, &mut w), Closed::Opens);
        assert_eq!(closed("agents-add-codex", None, &mut w), Closed::Opens);
        assert_eq!(closed(WINDOW_COMMAND, Some("2"), &mut w), Closed::Taken);
        assert_eq!(
            w.owed,
            vec![Effect::OpenWindow(vec![
                "agents".to_owned(),
                "2".to_owned()
            ])]
        );
        assert_eq!(closed("stick-open", None, &mut w), Closed::NotMine);
    }

    /// FAILS IF: a question the region asks is not asked on the footer's
    /// line under the command that answers it, its answer does not reach the
    /// view, or a refresh reaches past a photograph.
    #[test]
    fn a_question_is_asked_on_the_footers_line_and_answered_there() {
        let mut r = region();
        let mut w = way();
        assert_eq!(r.command("agents-add-codex", None, &mut w), Taken::Yes);
        let line = w
            .owed
            .iter()
            .find_map(|e| match e {
                Effect::Ask(line) => Some(line.clone()),
                _ => None,
            })
            .expect("the name is asked on the door's line");
        assert_eq!(line.id, "agents-add-codex");
        assert!(r.open.view.asking().is_some());
        r.unanswered(line.id, &mut w);
        assert!(r.open.view.asking().is_none(), "the question was let go");
        let mut w = way();
        assert_eq!(r.command("agents-refresh", None, &mut w), Taken::Yes);
        assert_eq!(r.open.done(), ["refresh"]);
        assert!(
            w.owed.iter().any(|e| matches!(e, Effect::Say(..))),
            "nothing was said"
        );
    }

    /// FAILS IF: the region's rows are not the view's, a heading takes a
    /// click, or `Esc` does not close the region through the footer's door.
    #[test]
    fn the_region_is_the_view() {
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
        let mut r = region();
        let rows = r.rows(&seen);
        let view = r.open.rows(100, &t);
        assert_eq!(rows.len(), view.len());
        assert!(rows.len() >= 3, "{} rows", rows.len());
        for (drawn, row) in rows.iter().zip(&view) {
            assert_eq!(drawn.takes, row.takes());
        }
        let mut w = way();
        let esc = ronoctua_keymap::Key::parse("ESC").expect("a key");
        assert_eq!(r.key(esc, &mut w), Taken::Yes);
        assert!(w.owed.contains(&Effect::Close), "{:?}", w.owed);
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a title, a sentence, the rest of a wrapped
    /// label — or leaves unlit one it takes. THE LIGHT IS THE CLICK: every
    /// row clicked once, on a view of its own, and lit exactly where
    /// something happened.
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
        let held = crate::region::held_to_the_click(
            || (region(), way()),
            &seen,
            |r, w: &Recorder| crate::region::seen(r, &w.owed, &seen),
        );
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }
}
