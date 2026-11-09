//! THE LLM ACCOUNTS IN THE FOOTER'S REGION (`LEADER / a a`) — the view as a
//! [`crate::region::Region`] the footer hosts by its id, beside its window
//! (`LEADER / a w`, [`crate::agents_window`]).
//!
//! # One view, two doors, one errand handler
//!
//! The region holds the view [`Open`] as the window holds it, and hands it
//! every key, click, command, answer and thread's answer; what the view asks
//! for after them goes through [`crate::agents::carry_out`] in both doors,
//! over each door's [`Doorway`]. A question is asked on the door's own line
//! (`Doorway::ask`) and its answer comes back to the region that asked.
//!
//! # Two columns, one page
//!
//! The view's rows are one list, each knowing its column; the region draws
//! them side by side through the view's [`crate::agents::Page`], and reads
//! the light and the click of a line at the cell the pointer is on — the
//! half of a line, a tab — through the same page. `TAB` crosses the
//! columns, so it walks no sections here.
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
    LOGIN_COMMAND, OPEN_COMMAND, Open, Reach, Reader, SESSION_COMMAND, SHOW_COMMAND, View,
    is_command, place_of, show_agent, window_words,
};
use crate::region::{Closed, Doorway, Entry, Lit, Region, RegionEasel, Row, Seen, Taken};

/// The id the footer ranks the view under (`surface::PRECEDENCE`).
pub const ID: &str = "agents";

/// The command that opens the view's window (`LEADER / a w`).
const WINDOW_COMMAND: &str = "agents-window";

/// The region the binary registers (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// What a command of the view does while it is CLOSED: the view (a mark's
/// click hands it the account's place) and every command of the view open
/// it — then the open region takes the command; a session and a login open
/// it on the file picker, the folder chosen before the terminal (a mark's
/// right click hands them the account's place); its window opens without
/// it, and an agent named by its session (a click on its glyph on the
/// modeline) is gone to without the view: the window it works in comes
/// forward. With no session, the view opens on the agents' rows' way to it.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        OPEN_COMMAND => Closed::Opens,
        WINDOW_COMMAND => {
            way.open_window(window_words(argument));
            Closed::Taken
        }
        SHOW_COMMAND => match argument.map(str::trim).filter(|s| !s.is_empty()) {
            Some(session) => {
                show_agent(session, way);
                Closed::Taken
            }
            None => Closed::Opens,
        },
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
        let (rows, page) = self.open.page(seen.columns, seen.texts);
        page.lines
            .iter()
            .enumerate()
            .map(|(n, fields)| Row {
                takes: page.takes(n, &rows),
                fields: fields.clone(),
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

    /// THE LINE THE CURSOR'S ROW IS DRAWN ON — side by side, a row of the
    /// accounts' column and one of the options' share a line.
    fn cursor_in(&self, seen: &Seen<'_>) -> Option<usize> {
        let (_, page) = self.open.page(seen.columns, seen.texts);
        page.line_of(self.open.view.cursor())
    }

    /// What a row does — in one list. Side by side a line is two rows, and
    /// only the cell says which ([`Self::about_at`]).
    fn about(&self, row: usize, seen: &Seen<'_>) -> Option<String> {
        let (rows, page) = self.open.page(seen.columns, seen.texts);
        if page.split.is_some() {
            return None;
        }
        View::about(&rows, page.row_at(row, None)?, seen.texts)
    }

    /// What the row under the pointer does — on a line side by side, the
    /// half the pointer is on; on the tabs, the tab.
    fn about_at(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> Option<String> {
        let (rows, page) = self.open.page(seen.columns, seen.texts);
        View::about_at(&rows, page.row_at(row, cell)?, cell, seen.texts)
    }

    /// THE POINTER LIGHTS WHAT THE CLICK TAKES, cell by cell: the half of a
    /// line side by side, one tab, the whole line in one list
    /// (`Page::lit`, which the click reads too).
    fn lit(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> Lit {
        let (rows, page) = self.open.page(seen.columns, seen.texts);
        page.lit(row, cell, &rows)
    }

    /// `TAB` crosses the columns — or reaches the file picker.
    fn walks_sections(&self) -> bool {
        false
    }

    /// Read by the whole notation through the reader the window calls too
    /// (`Open::key`): the file picker's, while it is up, then the view's.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        if self.open.key(key, way) {
            Taken::Yes
        } else {
            Taken::No
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
                self.open
                    .choose_at(place_of(argument), id == LOGIN_COMMAND, way);
            }
            other if is_command(other) => self.open.command(other, argument, way),
            _ => return Taken::No,
        }
        way.follow();
        Taken::Yes
    }

    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        self.clicked_at(row, None, way);
    }

    /// The row of the view at that cell of the line: the half side by side,
    /// the tab.
    fn clicked_at(&mut self, row: usize, cell: Option<u16>, way: &mut dyn Doorway) {
        self.open.clicked_at(row, cell, way);
    }

    /// The clipboard's text, for the file picker's `C-v`.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        self.open.pasted(text, way);
    }

    /// The file picker's `PgUp` and `PgDn` walk half of what the region
    /// shows.
    fn page_is(&self, lines: usize) {
        self.open.view.page_is(lines);
    }

    /// THE RULE BETWEEN THE TWO COLUMNS, side by side — lit while the
    /// accounts' has the keyboard, the file manager's focus mark.
    fn paint(&self, easel: &mut RegionEasel<'_>) {
        let columns =
            ronoctua_tokens::metrics::columns(easel.area.width, ronoctua_tokens::text::size());
        let (rows, page) = self.open.page(columns, easel.texts);
        let Some(split) = page.split else {
            return;
        };
        let f = easel.area.frame();
        let at = split.saturating_sub(1);
        let x = f32::from(crate::chrome::FIELDS_LEAD + at) * f.cell() + f.cell() / 2.0;
        let lit = rows
            .get(self.open.view.cursor())
            .is_some_and(|r| r.pane == crate::agents::Pane::Accounts);
        let top = f.y_of(easel.area.first);
        easel.under.push(if lit {
            crate::chrome::divider_lit(x, top, f.bottom())
        } else {
            crate::chrome::divider(x, top, f.bottom())
        });
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
    /// place a mark gave), an agent named by nothing opens the view where
    /// its rows are, one named by a session is gone to without the view (a
    /// click on its glyph opens no view over the window it brings forward),
    /// and another view's command is not its.
    #[test]
    fn a_closed_view_opens_for_its_own_commands() {
        let mut w = way();
        assert_eq!(closed(OPEN_COMMAND, Some("2"), &mut w), Closed::Opens);
        assert_eq!(closed(SHOW_COMMAND, None, &mut w), Closed::Opens);
        assert_eq!(closed(SHOW_COMMAND, Some("  "), &mut w), Closed::Opens);
        // A SESSION NO MARK COULD HAND (`agent_of` reads nothing for it):
        // the same verdict on every machine — taken, and said.
        assert_eq!(
            closed(SHOW_COMMAND, Some("NOT-A-SESSION"), &mut w),
            Closed::Taken
        );
        assert_eq!(
            std::mem::take(&mut w.owed),
            vec![Effect::Say(
                texts().t(crate::key::AGENTS_AGENT_ENDED),
                crate::echo::Echo::Long
            )]
        );
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

    /// The footer's width at which the two columns stand side by side.
    const WIDE: u16 = 160;

    /// Accounts to draw — never this machine's.
    fn with_accounts(_: &ronoctua_files::when::Zone) -> crate::agents::Readings {
        crate::agents::tests::readings()
    }

    /// FAILS IF: side by side, the pointer over one half of a line lights
    /// what a click there does not take — the blank beside a row of the
    /// other column, a title, the tab shown — or leaves unlit a half whose
    /// click takes it, or lights across both columns; or a click where
    /// nothing lights does something. The light is read at the cell, as the
    /// click is (`Page::lit`, `Page::row_at`).
    #[test]
    fn side_by_side_the_pointer_lights_the_half_a_click_takes() {
        use crate::region::Lit;
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: WIDE,
            lines: 80,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let fresh = || AgentsRegion {
            open: Open::new(
                ronoctua_files::when::Zone::utc(),
                Reader::Here(with_accounts),
                Reach::Nowhere(Vec::new()),
                None,
                WIDE,
                &texts(),
            ),
        };
        let wide_way = || {
            Recorder::new(
                std::rc::Rc::new(texts()),
                WIDE,
                true,
                crate::sudo::Gate::shut(),
                crate::wake::Wake::new(|| {}),
            )
        };
        let split = crate::agents::split_of(WIDE).expect("side by side");
        let first = fresh();
        let rows = first.rows(&seen);
        assert!(rows.len() >= 10, "{} lines", rows.len());
        let (mut lit_left, mut lit_right, mut dark) = (false, false, false);
        for (n, row) in rows.iter().enumerate() {
            for cell in [2, split + 2] {
                let lit = first.lit(n, Some(cell), &seen);
                let lights = row.takes && lit != Lit::Nothing;
                if let Lit::Cells(from, to) = lit {
                    assert!(
                        (from..to).contains(&cell) && (to <= split || from >= split),
                        "line {n} at {cell} lights {from}..{to}, across the columns"
                    );
                }
                let mut r = fresh();
                let mut w = wide_way();
                let before = crate::region::seen(&r, &w.owed, &seen);
                r.clicked_at(n, Some(cell), &mut w);
                let did = crate::region::seen(&r, &w.owed, &seen) != before;
                // THE ROW THE CURSOR STANDS ON is the click's whatever its
                // `RET` does (`region::Held::here`): born on the account.
                let here = first.cursor_in(&seen) == Some(n) && cell >= split;
                assert!(
                    lights == did || (lights && here),
                    "line {n} at cell {cell} is lit {lights} and a click there did {did}"
                );
                lit_left |= lights && cell < split;
                lit_right |= lights && cell >= split;
                dark |= !lights;
            }
        }
        assert!(
            lit_left && lit_right && dark,
            "the columns were not both reached"
        );
    }

    /// FAILS IF: a session or a login named by a mark's place — its right
    /// click — does anything with the view closed but open it, or the open
    /// view does not put the folder's question up for that account.
    #[test]
    fn a_marks_right_click_opens_the_view_on_the_folder_for_its_account() {
        let mut w = way();
        assert_eq!(closed(SESSION_COMMAND, Some("2"), &mut w), Closed::Opens);
        assert_eq!(closed(LOGIN_COMMAND, None, &mut w), Closed::Opens);
        assert!(w.owed.is_empty(), "a closed view did {:?}", w.owed);
        let mut r = AgentsRegion {
            open: Open::new(
                ronoctua_files::when::Zone::utc(),
                Reader::Here(with_accounts),
                Reach::Nowhere(Vec::new()),
                None,
                100,
                &texts(),
            ),
        };
        assert_eq!(r.command(SESSION_COMMAND, Some("2"), &mut w), Taken::Yes);
        assert_eq!(
            r.open.view.picking().map(|p| (p.account.as_str(), p.login)),
            Some(("codex-1", false))
        );
        assert!(
            r.open.done().is_empty(),
            "something opened before the folder"
        );
    }

    /// FAILS IF: `LEADER / a n` with the view closed — no place, and the
    /// first reading not here yet — says there is no account instead of
    /// choosing the folder for the list's default one; or chooses one for a
    /// list with no account.
    #[test]
    fn a_session_before_the_first_reading_is_the_lists_default() {
        let fresh = |read: fn(&ronoctua_files::when::Zone) -> crate::agents::Readings| {
            let mut open = Open::new(
                ronoctua_files::when::Zone::utc(),
                Reader::Here(read),
                Reach::Nowhere(Vec::new()),
                None,
                100,
                &texts(),
            );
            open.readings.read = false;
            open
        };
        let mut none = fresh(|_| crate::agents::Readings::default());
        let mut w = way();
        none.choose_at(None, false, &mut w);
        assert!(none.view.picking().is_none());
        assert!(
            w.owed.iter().any(|e| matches!(e, Effect::Say(..))),
            "nothing was said: {:?}",
            w.owed
        );
        let mut listed = fresh(with_accounts);
        listed.choose_at(None, true, &mut way());
        assert_eq!(
            listed.view.picking().map(|p| (p.account.as_str(), p.login)),
            Some(("claude-1", true))
        );
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a title, a sentence — or leaves unlit one it
    /// takes, the rest of a wrapped label among them. THE LIGHT IS THE CLICK: every
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
