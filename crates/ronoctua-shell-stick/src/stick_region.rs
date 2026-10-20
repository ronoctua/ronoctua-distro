//! THE INSTALLER STICK IN THE FOOTER'S REGION (`LEADER s z z`) — the view as
//! a [`crate::region::Region`] the footer hosts by its id, beside its window
//! (`LEADER s z w`, [`crate::stick_window`]).
//!
//! # One view, two doors, one errand handler
//!
//! The region holds the view [`Open`] as the window holds it, and hands it
//! every key, click, command and thread's answer; what the view asks for
//! after them goes through [`crate::stick::carry_out`] in both doors, over
//! each door's [`Doorway`] — the footer's writes down what is asked and does
//! it once the region is back in its slot, the window's is the window's
//! `Ui`. So the two doors cannot carry an errand out differently: there is
//! one function to do it.
//!
//! # What a photograph's footer reads
//!
//! No GitHub, no udisks and no thread: the readings are the scene the
//! footer camera's environment names ([`crate::stick_window::SCENE_VARIABLE`],
//! the window camera's `--scene`), or the machine's first reading with no
//! feed behind it — a region whose door is a photograph's reaches nothing.

use crate::region::{Closed, Doorway, Entry, Region, Row, Seen, Taken};
use crate::stick::{Feed, Open, Readings, View, press_of, press_of_key};

/// The id the footer ranks the view under (`surface::PRECEDENCE`).
pub const ID: &str = "stick";

/// The region the binary registers (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// What a command of the view does while it is CLOSED: `stick-open` and the
/// view's keys by name open it — then the open region takes the command, a
/// key by name pressed on it; its window opens without it.
fn closed(command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "stick-open" => Closed::Opens,
        "stick-window" => {
            way.open_window(vec![ID.to_owned()]);
            Closed::Taken
        }
        other if press_of(other).is_some() => Closed::Opens,
        _ => Closed::NotMine,
    }
}

/// Opens the view: the machine's readings and the threads behind them — or,
/// a photograph's, the scene its camera names and no thread at all.
fn open(_command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let photographing = way.photographing();
    let readings = if photographing {
        crate::stick_window::readings_of_the_footer_camera()
            .unwrap_or_else(Readings::of_the_machine)
    } else {
        Readings::of_the_machine()
    };
    // THE THREADS, only with a loop to hear them on — never a photograph's.
    let feed = (!photographing).then(|| {
        let feed = Feed::start(way.wake(), readings.cache.as_deref());
        if readings.consent {
            feed.ask_latest();
        }
        feed
    });
    let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
    Box::new(StickRegion {
        open: Open::over(readings, feed, None, zone, way.columns(), way.texts()),
    })
}

/// The view, open in the footer's region.
struct StickRegion {
    open: Open,
}

impl Region for StickRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// A click selects and a second acts; a line of the consent row's note
    /// is that row's click too (`View::clicked`). A row takes a click, and
    /// so lights under a resting pointer, wherever the click does something
    /// — a fact the cursor stands on included (`View::takes`, the window's
    /// answer too).
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        let rows = self.open.rows(seen.columns, seen.texts);
        let takes = View::takes(&rows);
        rows.into_iter()
            .zip(takes)
            .map(|(r, takes)| Row {
                takes,
                fields: r.fields,
            })
            .collect()
    }

    /// THE CONSENT ROW'S NOTE LIGHTS WITH IT: its lines are one control
    /// (`View::control_of`).
    fn control_of(&self, row: usize, seen: &Seen<'_>) -> usize {
        View::control_of(row, &self.open.rows(seen.columns, seen.texts))
    }

    /// The rows laid out once, each with its control (`Told::control`):
    /// asked row by row, [`Self::control_of`] lays them out again for each.
    fn told(&self, seen: &Seen<'_>, skip: usize, take: usize) -> Vec<crate::region::Told> {
        let rows = self.open.rows(seen.columns, seen.texts);
        let takes = View::takes(&rows);
        (0..rows.len())
            .map(|n| View::control_of(n, &rows))
            .zip(rows.iter().zip(takes))
            .map(|(control, (r, takes))| crate::region::Told {
                row: Row {
                    takes,
                    fields: r.fields.clone(),
                },
                control: Some(control),
                ..crate::region::Told::default()
            })
            .skip(skip)
            .take(take)
            .collect()
    }

    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.open.view.hint(&self.open.readings, seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.open.view.cursor())
    }

    fn about(&self, row: usize, seen: &Seen<'_>) -> Option<String> {
        let rows = self.open.rows(seen.columns, seen.texts);
        View::about(&rows, row, seen.texts)
    }

    /// Read by the whole notation through the reader the window calls too
    /// (`stick::press_of_key`).
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        match press_of_key(key) {
            Some(press) => {
                self.open.press(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    /// The view, its window and its keys by name; root is asked in the sudo
    /// bar. A second opening keeps the live view. A command it takes is a
    /// gesture by name, after which the page follows the cursor, as after a
    /// key (`Doorway::follow`).
    fn command(&mut self, id: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        let taken = match id {
            "stick-open" => Taken::Yes,
            "stick-window" => {
                way.open_window(vec![ID.to_owned()]);
                Taken::Yes
            }
            other => match press_of(other) {
                Some(press) => {
                    self.open.press(press, way);
                    Taken::Yes
                }
                None => Taken::No,
            },
        };
        if taken.took() {
            way.follow();
        }
        taken
    }

    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        self.open.clicked(row, way);
    }

    /// A thread answered: the drives, GitHub, the writer, the cache, the door.
    fn woken(&mut self, way: &mut dyn Doorway) {
        self.open.heard(way);
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

    /// The region over planted readings — the hosts not let, no drive yet,
    /// no thread — as the footer camera opens it with a scene.
    fn region() -> StickRegion {
        let t = texts();
        let readings = Readings::default();
        StickRegion {
            open: Open::over(
                readings,
                None,
                None,
                ronoctua_files::when::Zone::utc(),
                100,
                &t,
            ),
        }
    }

    /// FAILS IF: a command of the view does to the CLOSED view something
    /// other than what the footer did when it held the view itself — the
    /// view and its keys by name open it, its window opens without it, and
    /// another view's command is not its.
    #[test]
    fn a_closed_view_opens_for_its_own_commands() {
        let mut w = way();
        assert_eq!(closed("stick-open", None, &mut w), Closed::Opens);
        assert_eq!(closed("stick-down", None, &mut w), Closed::Opens);
        assert_eq!(closed("stick-reread", None, &mut w), Closed::Opens);
        assert_eq!(closed("stick-window", None, &mut w), Closed::Taken);
        assert_eq!(w.owed, vec![Effect::OpenWindow(vec!["stick".to_owned()])]);
        assert_eq!(closed("incus-open", None, &mut w), Closed::NotMine);
    }

    /// FAILS IF: the region's rows are not the view's at the door's width,
    /// a row that does nothing takes a click, the cursor is not the view's,
    /// or `Esc` does not close the region through the footer's door.
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
        floor_of(rows.len(), 3);
        // A ROW TAKES A CLICK WHEREVER THE CLICK DOES SOMETHING — a fact
        // the cursor stands on included — and never on a heading
        // (`the_pointer_lights_what_the_click_takes` holds it to the click).
        for (drawn, (row, takes)) in rows.iter().zip(view.iter().zip(View::takes(&view))) {
            assert_eq!(drawn.takes, takes);
            assert!(!row.heading || row.act != crate::stick::Act::Nothing || !takes);
            assert_eq!(drawn.fields.len(), row.fields.len());
        }
        assert_eq!(r.cursor(), Some(r.open.view.cursor()));
        let mut w = way();
        let esc = ronoctua_keymap::Key::parse("ESC").expect("a key");
        assert_eq!(r.key(esc, &mut w), Taken::Yes);
        assert!(w.owed.contains(&Effect::Close), "{:?}", w.owed);
        assert_eq!(
            r.command("files-open", None, &mut w),
            Taken::No,
            "another view's command"
        );
    }

    /// FAILS IF: the same errand is said differently in the two doors —
    /// a sentence of the view's on the footer's line and another on the
    /// window's, or one class in one and another in the other.
    #[test]
    fn both_doors_carry_an_errand_out_alike() {
        let t = texts();
        let mut footer = way();
        let mut ui = crate::door::Ui::for_tests(t);
        for errand in [
            crate::stick::Errand::Speak(crate::key::STICK_DRIVE_LEFT),
            crate::stick::Errand::Door(ronoctua_security::Why::Cancelled),
        ] {
            let mut a = region();
            crate::stick::carry_out(errand.clone(), &mut a.open, &mut footer);
            let mut b = region();
            crate::stick::carry_out(errand, &mut b.open, &mut ui);
            let said = footer.owed.iter().rev().find_map(|e| match e {
                Effect::Say(s, _) => Some(s.clone()),
                _ => None,
            });
            assert_eq!(said.as_deref(), ui.message.as_deref());
        }
    }

    fn floor_of(n: usize, least: usize) {
        assert!(n >= least, "{n} rows, fewer than {least}");
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a title, the air between sections, a
    /// sentence — or leaves unlit one it takes. THE LIGHT IS THE CLICK:
    /// every row clicked once, on a view of its own, and lit exactly where
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
            |r, w: &crate::region::Recorder| crate::region::seen(r, &w.owed, &seen),
        );
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }
}
