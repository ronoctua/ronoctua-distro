//! THE TERMINALS NO WINDOW SHOWS, IN THE FOOTER'S REGION (`RET b b`, and the
//! modeline's mark) — the second door onto [`crate::terminals`], beside its
//! window ([`crate::terminals_window`]), and the one errand handler both
//! doors carry out ([`carry_out`]).
//!
//! # What it reads, and through what
//!
//! The footer's census of the held terminals, lent by its type
//! (`ronoctua_terminals::registry::Census`, `held_by_the_footer!`) — the
//! table the modeline's mark counts, read by the footer's own thread, so
//! the region opens nothing of the machine and the rows are the mark's,
//! one for one. A fresh reading reaches it as news ([`Region::heard`]), and
//! the cursor stays on its terminal by identity: the region keeps no copy of
//! the table, only what the cursor is on.

use std::path::PathBuf;

use crate::region::{Closed, Doorway, Entry, Region, Row, Seen, Taken};
use crate::terminals::{Errand, OPEN, Press, Reading, SHOW, Target, View, WINDOW};
use ronoctua_terminals::TerminalId;
use ronoctua_terminals::registry::{Census, Registry};

/// The region's id, as the footer ranks it (`surface::PRECEDENCE`).
pub const ID: &str = "terminals";

/// The view's region, as the binary registers it (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The window's words after the binary's name (`ronoctua-shell terminals`).
pub const BACKGROUND_DOOR: &str = "terminals";

/// The commands that are the view's own gestures, by name: they press a key
/// of the open view, and do nothing with the view shut.
const GESTURES: &[&str] = &[
    "terminals-up",
    "terminals-down",
    "terminals-enter",
    "terminals-end",
];

/// What a command does while the view is closed.
fn closed(command: &str, _: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        OPEN => Closed::Opens,
        WINDOW => {
            way.open_window(vec![BACKGROUND_DOOR.to_owned()]);
            Closed::Taken
        }
        // A GESTURE OF A CLOSED VIEW does nothing, which is what it means —
        // taken, and not handed on to be called a command nobody answers.
        id if GESTURES.contains(&id) => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// The table the footer lends: its census, or nothing (a window, a door
/// that keeps none).
fn lent_census<'a>(
    lent: &crate::region::Lent<'a>,
) -> &'a [ronoctua_terminals::state::TerminalState] {
    lent.get::<Census>().map_or(&[], |c| c.terminals.as_slice())
}

/// Now, in seconds since the epoch — what "how long without a window" is
/// counted to.
#[must_use]
pub fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The person's home, a folder is shown under.
#[must_use]
pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|h| h.is_absolute())
}

/// Opens the view on the footer's census.
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let home = home();
    let lent = way.lent();
    let r = Reading {
        census: lent_census(&lent),
        now_s: now_s(),
        home: home.as_deref(),
    };
    let view = View::fresh(&r, way.columns(), way.texts());
    let on = view.on(&r, way.columns(), way.texts());
    Box::new(Terminals { view, on, home })
}

/// The view, open in the footer's region.
struct Terminals {
    view: View,
    /// What the cursor is on — what it is kept on when a reading moves the
    /// rows under it.
    on: Option<Target>,
    home: Option<PathBuf>,
}

impl Terminals {
    /// One press — a key, a gesture by name, a second click — and what it
    /// asks for, carried out.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let (errand, on) = {
            let lent = way.lent();
            let r = Reading {
                census: lent_census(&lent),
                now_s: now_s(),
                home: self.home.as_deref(),
            };
            let errand = self.view.key(press, &r, way.columns(), way.texts());
            (errand, self.view.on(&r, way.columns(), way.texts()))
        };
        self.on = on;
        way.follow();
        carry_out(errand, way);
    }
}

impl Region for Terminals {
    fn id(&self) -> &'static str {
        ID
    }

    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        let r = Reading {
            census: lent_census(&seen.lent),
            now_s: now_s(),
            home: self.home.as_deref(),
        };
        self.view
            .rows(&r, seen.columns, seen.texts)
            .into_iter()
            .map(|row| Row {
                // A HEADING IS NOT A ROW TO CLICK ON — the row's own answer
                // (`terminals::Row::takes`), which the window gives too.
                takes: row.takes(),
                fields: row.fields,
            })
            .collect()
    }

    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        let r = Reading {
            census: lent_census(&seen.lent),
            now_s: now_s(),
            home: self.home.as_deref(),
        };
        self.view.hint(&r, seen.columns, seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// BOTH DOORS THROUGH ONE READER — `terminals::press_of_key`.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let Some(press) = crate::terminals::press_of_key(key) else {
            return Taken::No;
        };
        self.press(press, way);
        Taken::Yes
    }

    fn command(&mut self, id: &str, _: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // ALREADY OPEN: left exactly as it is, and the page follows its
            // cursor — a door onto a room you are in does not rearrange it.
            OPEN => way.follow(),
            WINDOW => way.open_window(vec![BACKGROUND_DOOR.to_owned()]),
            id if GESTURES.contains(&id) => {
                if let Some(press) = crate::terminals::press_of(id) {
                    self.press(press, way);
                }
            }
            _ => return Taken::No,
        }
        Taken::Yes
    }

    /// THE MOUSE AIMS, AND THE SECOND CLICK ACTS — invariant 19.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let again = {
            let lent = way.lent();
            let r = Reading {
                census: lent_census(&lent),
                now_s: now_s(),
                home: self.home.as_deref(),
            };
            let rows = self.view.rows(&r, way.columns(), way.texts());
            let again = self.view.clicked(row, &rows);
            self.on = self.view.on(&r, way.columns(), way.texts());
            again
        };
        if again {
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

    /// A fresh reading of the footer's census (`Footer::terminals_heard`).
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        let Some(fresh) = news.downcast_ref::<Census>() else {
            return;
        };
        let r = Reading {
            census: &fresh.terminals,
            now_s: now_s(),
            home: self.home.as_deref(),
        };
        // THE CURSOR STAYS ON ITS TERMINAL BY IDENTITY — one put away adds a
        // row above it, and an index would drift.
        self.view
            .follow(self.on.as_ref(), &r, way.columns(), way.texts());
        self.on = self.view.on(&r, way.columns(), way.texts());
        // A TERMINAL PUT AWAY OR SHOWN brings or takes its row.
        way.rows_changed();
    }
}

/// CARRIES OUT WHAT THE VIEW ASKED FOR, in either door — the footer's region
/// and the window. One handler, so the two doors cannot do one errand two
/// ways: what is said is said in the door it was asked in, for as long as
/// its class says.
pub fn carry_out(errand: Errand, way: &mut dyn Doorway) {
    match errand {
        Errand::Nothing => {}
        Errand::Close => way.close(),
        // THE TERMINAL IS THE FOOTER'S TO SHOW (`terminal-show`): its window
        // focused, or one opened for its group with it in front. The view
        // steps aside first — the footer gives its region back, so the
        // window that comes takes the keyboard; a window of this view stays.
        Errand::Show(id) => {
            way.step_aside();
            way.run(SHOW, Some(id.to_string()));
        }
        Errand::End(id, name) => end(&id, &name, way),
    }
    way.repaint();
}

/// Ends a terminal: an order to its holder, on a thread — the holder answers
/// from its engine's thread, and `Registry::end` waits up to its timeout for
/// it. What the person sees is the row leaving, when the census reads the
/// holder gone; what is said now is that it was asked. A holder that does
/// not answer is a dead one or a lost one, and the census is asked to sweep:
/// a dead holder's files go, and its row with them.
fn end(id: &TerminalId, name: &str, way: &mut dyn Doorway) {
    let said = way
        .texts()
        .t_with(crate::key::TERMINALS_ENDING, &[("name", name)]);
    // A PHOTOGRAPH REACHES NOTHING OF THE MACHINE: no holder is told.
    if way.photographing() {
        way.say(said, crate::echo::Echo::Brief);
        return;
    }
    let Some(registry) = Registry::of_session() else {
        let said = way.texts().t(crate::key::TERMINALS_NO_REGISTRY);
        way.say(said, crate::echo::Echo::Normal);
        return;
    };
    let id = id.clone();
    let asked = std::thread::Builder::new()
        .name("ronoctua-terminal-end".to_owned())
        .spawn(move || {
            if let Err(e) = registry.end(&id) {
                tracing::warn!(error = %e, terminal = %id, "the terminal's holder did not take its end; sweeping");
                let _ = registry.census();
            }
        });
    match asked {
        Ok(_) => way.say(said, crate::echo::Echo::Brief),
        Err(e) => {
            tracing::warn!(error = %e, "could not ask a terminal to end");
            let said = way
                .texts()
                .t_with(crate::key::TERMINALS_NOT_ENDED, &[("name", name)]);
            way.say(said, crate::echo::Echo::Normal);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region;
    use crate::terminals::planted;
    use std::rc::Rc;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    /// The footer's doorway, a photograph's: nothing of the machine reached.
    fn footer() -> region::Recorder {
        region::Recorder::new(
            Rc::new(texts()),
            100,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
    }

    /// `n` terminals each alone, the first the newest.
    fn census_of(n: usize) -> Census {
        Census {
            terminals: (0..n)
                .map(|i| {
                    planted(
                        "zsh",
                        &ronoctua_terminals::GroupId::fresh().unwrap(),
                        1 << 16,
                        now_s() - 60 * i64::try_from(i + 1).unwrap(),
                    )
                })
                .collect(),
        }
    }

    /// FAILS IF: a command of the view does something else with the view
    /// closed than it did in the footer — the open command opens, the window
    /// opens a window, a gesture is taken and does nothing — or a command
    /// that is not the view's is taken for it.
    #[test]
    fn a_closed_view_answers_its_own_commands_and_no_other() {
        let mut way = footer();
        assert_eq!(closed(OPEN, None, &mut way), Closed::Opens);
        assert_eq!(closed(WINDOW, None, &mut way), Closed::Taken);
        assert_eq!(
            way.owed,
            vec![region::Effect::OpenWindow(vec![BACKGROUND_DOOR.to_owned()])]
        );
        for gesture in GESTURES {
            assert!(
                crate::terminals::press_of(gesture).is_some(),
                "{gesture} presses nothing"
            );
            assert_eq!(closed(gesture, None, &mut way), Closed::Taken);
        }
        assert_eq!(closed("pointers-open", None, &mut way), Closed::NotMine);
        assert_eq!(
            closed(SHOW, None, &mut way),
            Closed::NotMine,
            "the footer's own"
        );
        assert_eq!(
            way.owed.len(),
            1,
            "a gesture of a closed view asked something"
        );
    }

    /// FAILS IF: showing a terminal does not step the region aside before
    /// running the footer's `terminal-show` with its id (the region left open
    /// over the window that comes would keep the keyboard), or a yes to the
    /// question sends no order and says nothing — in the footer's door and
    /// the window's alike.
    #[test]
    fn an_errand_is_carried_out_alike_in_both_doors() {
        let id = TerminalId::fresh().unwrap();
        let mut way = footer();
        carry_out(Errand::Show(id.clone()), &mut way);
        let at = |e: &region::Effect| way.owed.iter().position(|x| x == e);
        let aside = at(&region::Effect::StepAside).expect("no step aside");
        let shown = at(&region::Effect::Run(SHOW, Some(id.to_string()))).expect("not shown");
        assert!(aside < shown, "{:?}", way.owed);
        carry_out(Errand::End(id.clone(), "zsh".to_owned()), &mut way);
        assert!(
            way.owed
                .iter()
                .any(|e| matches!(e, region::Effect::Say(s, crate::echo::Echo::Brief) if s.contains("zsh"))),
            "{:?}",
            way.owed
        );
        let mut ui = crate::door::Ui::for_tests(texts());
        carry_out(Errand::Show(id.clone()), &mut ui);
        assert_eq!(ui.orders(), &[format!("command {SHOW} {id}")]);
        assert!(!ui.quit, "showing a terminal closed the window");
    }

    /// FAILS IF: the region opens on something other than the footer's
    /// census (the rows would not be the mark's), or a census told by the
    /// type the footer tells it (`Census`: `surface/terminals.rs`) is not
    /// the one the region follows — the cursor left on an index while a new
    /// terminal comes in above it — or news of a type it does not read asks
    /// anything.
    #[test]
    fn the_region_follows_the_footers_census_by_identity() {
        let census = census_of(2);
        let held: Rc<Census> = Rc::new(census.clone());
        let mut way = footer().lending(region::Lent::of(vec![&*held as &dyn std::any::Any]));
        let mut opened = open(OPEN, None, &mut way);
        assert_eq!(opened.id(), ID);
        // THE SECOND ROW, the older terminal.
        opened.key(ronoctua_keymap::Key::parse("<down>").unwrap(), &mut way);
        let rows_before = {
            let seen = Seen {
                texts: way.texts(),
                columns: 100,
                lines: 20,
                zone: &ronoctua_files::when::Zone::utc(),
                photographing: true,
                lent: way.lent(),
            };
            opened.rows(&seen)
        };
        let older = opened.cursor().unwrap();
        assert!(rows_before[older].takes);
        let mut newer = census;
        let first = planted(
            "vim",
            &ronoctua_terminals::GroupId::fresh().unwrap(),
            1,
            now_s(),
        );
        newer.terminals.insert(0, first);
        opened.heard(&newer, &mut way);
        assert_eq!(
            opened.cursor(),
            Some(older + 1),
            "the cursor stayed on an index"
        );
        let before = way.owed.len();
        opened.heard(&(), &mut way);
        assert_eq!(way.owed.len(), before, "news of no type asked something");
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a heading, the air between sections — or
    /// leaves unlit one it takes. THE LIGHT IS THE CLICK: every row clicked
    /// once, on a view of its own over the same census, and lit exactly
    /// where something happened.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let census = census_of(3);
        let held: Rc<Census> = Rc::new(census);
        let seen = Seen {
            texts: &t,
            columns: 100,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: region::Lent::of(vec![&*held as &dyn std::any::Any]),
        };
        let fresh = || {
            let home = home();
            let r = Reading {
                census: &held.terminals,
                now_s: now_s(),
                home: home.as_deref(),
            };
            let view = View::fresh(&r, 100, &t);
            let on = view.on(&r, 100, &t);
            let way = footer().lending(region::Lent::of(vec![&*held as &dyn std::any::Any]));
            (Terminals { view, on, home }, way)
        };
        let held_rows = region::held_to_the_click(fresh, &seen, |r, w: &region::Lending<'_>| {
            region::seen(r, &w.owed, &seen)
        });
        assert!(
            held_rows.iter().any(|h| h.lit) && held_rows.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held_rows:?}"
        );
        let unheld = region::unheld(&held_rows);
        assert!(unheld.is_empty(), "{unheld:?}");
    }
}
