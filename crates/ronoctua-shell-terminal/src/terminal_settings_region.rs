//! THE TERMINAL'S SETTINGS IN THE FOOTER'S REGION (`RET s s`) — the second
//! door onto [`crate::terminal_settings`], beside its window
//! ([`crate::terminal_settings_window`]), and the one errand handler both
//! doors carry out ([`carry_out`]).
//!
//! # What it reads, and what it keeps
//!
//! `terminal.json`, read when the region opens and again whenever the file
//! moves while it is open — its own watch (`Notice::on_file`), raised as it
//! opens and dropped as it closes, which wakes the footer's loop: the window
//! writes the same file, and the file is the broadcast. What the region
//! keeps is the cursor and the reading its rows draw, with what of the file
//! that reading could not take (`terminal_settings::read`), said under it.

use std::sync::mpsc;

use crate::region::{Closed, Doorway, Entry, Region, Row, Seen, Taken};
use crate::terminal_settings::{
    Changed, Errand, OPEN, Press, Settings, View, WINDOW, change_the_file, judged, press_of,
    press_of_key, read,
};

/// The region's id, as the footer ranks it (`surface::PRECEDENCE`).
pub const ID: &str = "terminal-settings";

/// The view's region, as the binary registers it (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The window's words after the binary's name.
pub const SETTINGS_DOOR: &str = "terminal-settings";

/// The commands that are the view's own gestures, by name.
const GESTURES: &[&str] = &[
    "terminal-settings-up",
    "terminal-settings-down",
    "terminal-settings-less",
    "terminal-settings-more",
    "terminal-settings-enter",
];

/// What a command does while the view is closed.
fn closed(command: &str, _: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        OPEN => Closed::Opens,
        WINDOW => {
            way.open_window(vec![SETTINGS_DOOR.to_owned()]);
            Closed::Taken
        }
        id if GESTURES.contains(&id) => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// Opens the view on the file as it is now — and, outside a photograph, a
/// watch on it while the region is open.
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let dir = (!way.photographing())
        .then(crate::terminal_settings::dir)
        .flatten();
    let store = crate::terminal_settings::store_in(dir.clone());
    let (settings, unread) = read(&store);
    let mut view = View::fresh(&settings, way.columns(), way.texts());
    view.read_in_part(unread);
    let (moved, heard) = mpsc::channel();
    let wake = way.wake();
    let watch = dir.as_deref().and_then(|dir| {
        crate::terminal_settings::watch_in(dir, ronoctua_files::notice::Pace::AtOnce, move || {
            let sent = moved.send(()).is_ok();
            if sent {
                wake.call();
            }
            sent
        })
    });
    Box::new(TerminalSettings {
        view,
        settings,
        store,
        heard,
        _watch: watch,
    })
}

/// The settings, open in the footer.
struct TerminalSettings {
    view: View,
    /// The file as it was read last.
    settings: Settings,
    store: ronoctua_store::Store<Settings>,
    /// That the file moved.
    heard: mpsc::Receiver<()>,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl TerminalSettings {
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = self
            .view
            .press(press, &self.settings, way.columns(), way.texts());
        way.follow();
        if let Some(now) = carry_out(errand, &self.store, way) {
            self.settings = now;
            // THE FILE JUDGED AGAIN: a write keeps the places that did not
            // read as the file had them, and the sentence stays with them.
            self.view.read_in_part(judged(&self.store));
        }
    }
}

impl Region for TerminalSettings {
    fn id(&self) -> &'static str {
        ID
    }

    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.view
            .rows(&self.settings, seen.columns, seen.texts)
            .into_iter()
            .map(|r| Row {
                takes: r.takes(),
                fields: r.fields,
            })
            .collect()
    }

    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view.hint(seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let Some(press) = press_of_key(key) else {
            return Taken::No;
        };
        self.press(press, way);
        Taken::Yes
    }

    fn command(&mut self, id: &str, _: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            OPEN => way.follow(),
            WINDOW => way.open_window(vec![SETTINGS_DOOR.to_owned()]),
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
        let rows = self.view.rows(&self.settings, way.columns(), way.texts());
        if self.view.clicked(row, &rows) {
            self.press(Press::Enter, way);
        } else {
            way.repaint();
        }
    }

    /// The file moved — the window's write, or a hand's: read again.
    fn woken(&mut self, way: &mut dyn Doorway) {
        let mut moved = false;
        while self.heard.try_recv().is_ok() {
            moved = true;
        }
        if moved {
            let (settings, unread) = read(&self.store);
            self.settings = settings;
            self.view.read_in_part(unread);
            way.repaint();
        }
    }
}

/// CARRIES OUT WHAT THE VIEW ASKED FOR, in either door, over `store`: a
/// change written — and the file as it is after, for the door to draw — or
/// its refusal said where the key was pressed, to be read to the end.
pub fn carry_out(
    errand: Errand,
    store: &ronoctua_store::Store<Settings>,
    way: &mut dyn Doorway,
) -> Option<Settings> {
    let now = match errand {
        Errand::Nothing => None,
        Errand::Close => {
            way.close();
            None
        }
        Errand::Change(setting, forward) => match change_the_file(store, setting, forward) {
            Changed::Written(now) => Some(now),
            refused => {
                if let Some(said) = refused.refusal(way.texts()) {
                    tracing::warn!(?refused, "terminal.json was not changed");
                    way.say(said, crate::echo::Echo::Long);
                }
                None
            }
        },
    };
    way.repaint();
    now
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region;
    use crate::terminal_settings::{Setting, store_in};
    use std::rc::Rc;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    fn footer() -> region::Recorder {
        region::Recorder::new(
            Rc::new(texts()),
            100,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-terminal-settings-region-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A region over `dir`'s file, no watch.
    fn opened(dir: &std::path::Path) -> TerminalSettings {
        let store = store_in(Some(dir.to_path_buf()));
        let (settings, unread) = read(&store);
        let (_, heard) = mpsc::channel();
        let mut view = View::fresh(&settings, 100, &texts());
        view.read_in_part(unread);
        TerminalSettings {
            view,
            settings,
            store,
            heard,
            _watch: None,
        }
    }

    /// FAILS IF: a command of the view does something else with the view
    /// closed than it did in the footer, or a command that is not the
    /// view's is taken for it.
    #[test]
    fn a_closed_view_answers_its_own_commands_and_no_other() {
        let mut way = footer();
        assert_eq!(closed(OPEN, None, &mut way), Closed::Opens);
        assert_eq!(closed(WINDOW, None, &mut way), Closed::Taken);
        assert_eq!(
            way.owed,
            vec![region::Effect::OpenWindow(vec![SETTINGS_DOOR.to_owned()])]
        );
        for gesture in GESTURES {
            assert!(press_of(gesture).is_some(), "{gesture} presses nothing");
            assert_eq!(closed(gesture, None, &mut way), Closed::Taken);
        }
        assert_eq!(closed("terminals-open", None, &mut way), Closed::NotMine);
        assert_eq!(way.owed.len(), 1);
    }

    /// FAILS IF: a key on a setting does not write the file and draw what
    /// it says now, or a refusal — a file that does not parse — is not said
    /// in the door the key was pressed in, to be read to the end; in the
    /// footer's door and the window's alike.
    #[test]
    fn a_change_is_written_and_a_refusal_said_in_both_doors() {
        let dir = scratch("doors");
        let mut r = opened(&dir);
        let mut way = footer();
        Region::key(
            &mut r,
            ronoctua_keymap::Key::parse("RET").unwrap(),
            &mut way,
        );
        assert_eq!(r.settings.scrollback_bytes, 50_000_000);
        assert_eq!(store_in(Some(dir.clone())).read(), r.settings);
        std::fs::write(dir.join(crate::terminal_settings::FILE), "{ not json").unwrap();
        Region::key(
            &mut r,
            ronoctua_keymap::Key::parse("RET").unwrap(),
            &mut way,
        );
        assert!(
            way.owed
                .iter()
                .any(|e| matches!(e, region::Effect::Say(_, crate::echo::Echo::Long))),
            "{:?}",
            way.owed
        );
        let mut ui = crate::door::Ui::for_tests(texts());
        let store = store_in(Some(dir.clone()));
        assert!(carry_out(Errand::Change(Setting::Copy, true), &store, &mut ui).is_none());
        assert!(
            ui.message
                .as_deref()
                .is_some_and(|m| m.contains("terminal.json"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the region keeps drawing the file as it opened on after
    /// another door wrote it — it reads it again when its watch wakes it —
    /// or a wake with nothing moved reads it again.
    #[test]
    fn the_region_reads_the_file_again_when_it_moves() {
        let dir = scratch("moved");
        let (moved, heard) = mpsc::channel();
        let mut r = TerminalSettings {
            heard,
            ..opened(&dir)
        };
        let other = store_in(Some(dir.clone()));
        let mut written = Settings::default();
        Setting::Cursor.change(&mut written, true);
        other.write(&written).unwrap();
        let mut way = footer();
        Region::woken(&mut r, &mut way);
        assert_ne!(r.settings, written, "read with nothing heard");
        moved.send(()).unwrap();
        Region::woken(&mut r, &mut way);
        assert_eq!(r.settings, written);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the region opens on a file read only in part and does not
    /// say so, stops saying it after a write of another row (which keeps the
    /// place that did not read as the file had it), keeps saying it after a
    /// write of that very place (the file is the person's there now), or is
    /// not judged again when its watch wakes it — broken, or mended, by
    /// another hand.
    #[test]
    fn a_part_not_read_is_said_and_judged_again() {
        let dir = scratch("partly");
        let file = dir.join(crate::terminal_settings::FILE);
        std::fs::write(&file, "{\"cursor\": \"triangle\"}").unwrap();
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let says = |r: &TerminalSettings| -> bool {
            let seen = Seen {
                texts: &t,
                columns: 100,
                lines: 40,
                zone: &zone,
                photographing: true,
                lent: region::Lent::none(),
            };
            r.rows(&seen)
                .iter()
                .flat_map(|row| row.fields.iter())
                .any(|f| f.text.contains("/cursor"))
        };
        let (moved, heard) = mpsc::channel();
        let mut r = TerminalSettings {
            heard,
            ..opened(&dir)
        };
        assert!(says(&r), "opened on a part not read, and silent");
        let mut way = footer();
        Region::key(
            &mut r,
            ronoctua_keymap::Key::parse("RET").unwrap(),
            &mut way,
        );
        assert_eq!(r.settings.scrollback_bytes, 50_000_000, "nothing written");
        assert!(
            std::fs::read_to_string(&file).unwrap().contains("triangle"),
            "the write lost the file's own word"
        );
        assert!(says(&r), "the region's own write took the sentence away");
        // THE PLACE ITSELF TURNED: the person's value is the file's now, and
        // it reads whole.
        for key in ["<down>", "RET"] {
            Region::key(&mut r, ronoctua_keymap::Key::parse(key).unwrap(), &mut way);
        }
        assert!(
            !std::fs::read_to_string(&file).unwrap().contains("triangle"),
            "the cursor's row did not write the cursor"
        );
        assert!(
            !says(&r),
            "a file its own write mended is still said not to read"
        );
        std::fs::write(&file, "{\"cursor\": \"triangle\"}").unwrap();
        moved.send(()).unwrap();
        Region::woken(&mut r, &mut way);
        assert!(
            says(&r),
            "a file broken by another hand is not said when the watch wakes"
        );
        std::fs::write(&file, "{\"cursor\": \"bar\"}").unwrap();
        moved.send(()).unwrap();
        Region::woken(&mut r, &mut way);
        assert!(!says(&r), "a mended file is still said not to read");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a resting pointer lights a row a click takes nothing on — a
    /// title, a note, the air — or leaves unlit a setting.
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
            lent: region::Lent::none(),
        };
        let dir = scratch("lit");
        let fresh = || (opened(&dir), footer());
        let held = region::held_to_the_click(fresh, &seen, |r, w: &region::Recorder| {
            region::seen(r, &w.owed, &seen)
        });
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        let unheld = region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
