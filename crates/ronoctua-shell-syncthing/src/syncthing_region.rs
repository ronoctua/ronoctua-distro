//! THE SYNCTHING VIEW IN THE FOOTER'S REGION — the second door onto
//! [`crate::syncthing`], beside its window ([`crate::syncthing_window`]), and
//! the one errand handler both doors carry out ([`carry_out`]).
//!
//! # What it reads, and through what
//!
//! The footer's own Syncthing thread — a long poll alive for the whole
//! session, because the modeline may draw a mark from it — its last reading,
//! and the view's preference the footer keeps for that mark
//! (`syncthing.json`): all lent by type ([`crate::region::kept`]). A fresh
//! reading and a preference changed in the other door reach the region as
//! news ([`Region::heard`]); the file picker's folder is watched through the
//! region's own wake.
//!
//! # The commands that work with the view shut
//!
//! `LEADER s s d` and `s s f` are leader keys of their own and work with
//! nothing on screen — the device's id asked on the minibuffer's line, the
//! folder chosen in the view's picker — and so do pausing and resuming every
//! folder, the one-line state and Syncthing's own page: each through the
//! same lines.

use std::cell::Cell;
use std::rc::Rc;

use crate::region::{self, Closed, Doorway, Entry, Region, Row, Seen, Taken};
use crate::syncthing::{
    Errand, Picking, Press, View, press_of, press_of_key, unasked, web_address,
};
use ronoctua_syncthing::prefs::Preferences;
use ronoctua_syncthing::{Handle, Order, Snapshot};

/// The region's id, as the footer ranks it (`surface::PRECEDENCE`).
pub const ID: &str = "syncthing";

/// The view's region, as the binary registers it (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The opener of a web page, by path (`RONOCTUA_XDG_OPEN`, `nix/rust.nix`).
const XDG_OPEN: Option<&str> = option_env!("RONOCTUA_XDG_OPEN");

/// The view's own gestures, by name: they press a key of the open view, and
/// do nothing with the view shut.
const GESTURES: &[&str] = &[
    "syncthing-up",
    "syncthing-down",
    "syncthing-less",
    "syncthing-more",
    "syncthing-enter",
    "syncthing-pause",
    "syncthing-rescan",
    "syncthing-add",
    "syncthing-forget",
];

/// What a command does while the view is closed.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match (command, argument) {
        ("syncthing-open", _) => Closed::Opens,
        ("syncthing-window", _) => {
            way.open_window(vec!["syncthing".to_owned()]);
            Closed::Taken
        }
        ("syncthing-web", _) => {
            let address = web_address(region::kept::<Snapshot>(way).as_deref());
            open_the_page(&address, way);
            Closed::Taken
        }
        ("syncthing-state", _) => {
            say_the_state(region::kept::<Snapshot>(way).as_deref(), way);
            Closed::Taken
        }
        ("syncthing-pause-all", _) => {
            ask(Order::Pause(None), way);
            Closed::Taken
        }
        ("syncthing-resume-all", _) => {
            ask(Order::Resume(None), way);
            Closed::Taken
        }
        // WHAT WAS TYPED comes back as the command's argument, and with no
        // view open the command is what knows which question it answers
        // (`syncthing::unasked`). AN EMPTY ANSWER IS STILL AN ANSWER.
        ("syncthing-add-device" | "syncthing-add-folder" | "syncthing-enter", Some(said)) => {
            if let Some(order) = unasked(command, said) {
                ask(order, way);
            }
            way.repaint();
            Closed::Taken
        }
        // A FOLDER IS CHOSEN, not typed: the view, and its picker up over it.
        ("syncthing-add-folder", None) => Closed::Opens,
        // THE LEADER'S OWN QUESTION, on the minibuffer's line.
        ("syncthing-add-device", None) => {
            way.ask(crate::line::Line::new("syncthing-add-device", false, None));
            Closed::Taken
        }
        // A GESTURE OF A CLOSED VIEW does nothing — taken, and not handed on
        // to be called a command nobody answers.
        (id, _) if GESTURES.contains(&id) => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// Opens the view on the footer's last reading and preference, with the file
/// picker's folder watched through the region's own wake.
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let prefs = region::kept::<Cell<Preferences>>(way)
        .map_or_else(|| ronoctua_syncthing::prefs::store().read(), |p| p.get());
    let snapshot = region::kept::<Snapshot>(way);
    let mut view = View::fresh(prefs);
    view.watch_with(way.wake());
    view.settle_in(snapshot.as_deref(), way.columns(), way.texts());
    Box::new(Syncthing { view, snapshot })
}

/// Ask Syncthing for something, through the door's line — and say so if
/// there is nobody to ask.
fn ask(order: Order, way: &mut dyn Doorway) {
    match region::kept::<Handle>(way) {
        Some(h) => h.send(order),
        None => not_running(way),
    }
}

/// That there is no Syncthing to ask.
fn not_running(way: &mut dyn Doorway) {
    let said = way.texts().t(crate::key::ECHO_SYNCTHING_NOT_RUNNING);
    way.say(said, crate::echo::Echo::Normal);
}

/// SYNCTHING'S ONE-LINE ECHO. The view says all of this and more, and the
/// echo stays because a line that appears and goes is cheaper than a view
/// when the question is only "is it fine?" — the argument `LEADER g` makes
/// for the glance.
fn say_the_state(snapshot: Option<&Snapshot>, way: &mut dyn Doorway) {
    let texts = way.texts();
    let said = match snapshot {
        Some(s) if s.service.is_up() => {
            let mark = texts.decimal_mark();
            texts.t_with(
                crate::key::ECHO_SYNCTHING_STATE,
                &[
                    ("connected", &s.connected().to_string()),
                    ("percent", &s.completion.to_string()),
                    ("up", &crate::sizes::readable_bytes(s.traffic.sent, mark)),
                    (
                        "down",
                        &crate::sizes::readable_bytes(s.traffic.received, mark),
                    ),
                ],
            )
        }
        _ => texts.t(crate::key::ECHO_SYNCTHING_STOPPED),
    };
    way.say(said, crate::echo::Echo::Normal);
}

/// Open Syncthing's own page in whatever opens links here.
///
/// ONE FUNCTION, THREE CALLERS: `LEADER s s a`, the view's `w`, and the
/// window's. The page is not chrome of ours (invariant 14 is about what THIS
/// system draws) and it stays because it has graphs and a history a list of
/// labels and values does not replace. In a scope of its own, off the loop,
/// by the path every door opens by; one that could not start is a sentence
/// in the door that asked.
fn open_the_page(address: &str, way: &mut dyn Doorway) {
    let Some(opener) = XDG_OPEN.filter(|p| !p.is_empty()) else {
        tracing::error!("RONOCTUA_XDG_OPEN was not set at build time");
        let said = way.texts().t(crate::key::ECHO_SYNCTHING_STOPPED);
        way.say(said, crate::echo::Echo::Normal);
        return;
    };
    if address.is_empty() {
        let said = way.texts().t(crate::key::ECHO_SYNCTHING_STOPPED);
        way.say(said, crate::echo::Echo::Normal);
        return;
    }
    way.launch(crate::door::Launching::Program {
        program: opener.to_owned(),
        args: vec![format!("http://{address}")],
        cwd: None,
    });
}

/// The view, open in the footer's region.
struct Syncthing {
    view: View,
    /// The footer's last reading — the same `Rc` the footer holds.
    snapshot: Option<Rc<Snapshot>>,
}

impl Syncthing {
    /// One press — a key, a gesture by name, a second click — and what it
    /// asks for, carried out.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = self
            .view
            .key(press, self.snapshot.as_deref(), way.columns(), way.texts());
        // THE SURFACE FIRST, THEN THE CURSOR — the order every view of this
        // shape keeps.
        way.follow();
        self.carried(errand, way);
    }

    /// What the view asked for, carried out through the footer's lines.
    fn carried(&mut self, errand: Errand, way: &mut dyn Doorway) {
        let orders = region::kept::<Handle>(way);
        let kept = region::kept::<Cell<Preferences>>(way);
        carry_out(
            errand,
            &mut self.view,
            orders.as_deref(),
            kept.as_deref(),
            way,
        );
    }

    /// What was typed, under the id it was asked by: the VIEW answers,
    /// because it is what knows which question was asked — a path may be a
    /// new folder's or an offered one's, and the two are different orders —
    /// through the same `View::answered` the window's line reaches.
    fn typed(&mut self, id: &str, said: &str, way: &mut dyn Doorway) {
        if let Some(order) = self.view.answered(id, said, self.snapshot.as_deref()) {
            ask(order, way);
        }
        way.repaint();
    }
}

impl Region for Syncthing {
    fn id(&self) -> &'static str {
        ID
    }

    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.view
            .rows(self.snapshot.as_deref(), seen.columns, seen.texts)
            .into_iter()
            .map(|r| Row {
                // A HEADING IS NOT A ROW TO CLICK ON.
                takes: !r.heading,
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

    /// BY THE WHOLE NOTATION, through the reader the window calls too
    /// (`syncthing::press_of_key`): with shift alone, `C-d` was `d` and
    /// forgot a folder, while the window's same chord did nothing. And the
    /// file picker, while it is up over the view, takes what it reads first
    /// — every letter, and its own keys.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        if self.view.picker_takes(key) {
            let errand =
                self.view
                    .pick_key(key, self.snapshot.as_deref(), way.columns(), way.texts());
            way.follow();
            self.carried(errand, way);
            return Taken::Yes;
        }
        let Some(press) = press_of_key(key) else {
            return Taken::No;
        };
        self.press(press, way);
        Taken::Yes
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match (id, argument) {
            // ALREADY OPEN: left exactly as it is, and the page follows its
            // cursor.
            ("syncthing-open", _) => way.follow(),
            ("syncthing-window", _) => way.open_window(vec!["syncthing".to_owned()]),
            ("syncthing-web", _) => {
                let address = web_address(self.snapshot.as_deref());
                open_the_page(&address, way);
            }
            ("syncthing-state", _) => say_the_state(self.snapshot.as_deref(), way),
            ("syncthing-pause-all", _) => ask(Order::Pause(None), way),
            ("syncthing-resume-all", _) => ask(Order::Resume(None), way),
            // `RET` CARRIES A TYPED VALUE when there is one, and is a key when
            // there is not — the shape `bluetooth-accept` has. AN EMPTY ANSWER
            // IS STILL AN ANSWER: taken for a key, it pressed `RET` on the row
            // under the cursor.
            ("syncthing-add-device" | "syncthing-add-folder" | "syncthing-enter", Some(said)) => {
                self.typed(id, said, way);
            }
            // A FOLDER IS CHOSEN, not typed: the picker, up over the view.
            ("syncthing-add-folder", None) => {
                self.view.choose(Picking::NewFolder);
                self.view
                    .settle_in(self.snapshot.as_deref(), way.columns(), way.texts());
                way.follow();
                way.repaint();
            }
            // THE LEADER'S OWN QUESTION, and not one the view asked: a
            // question of the view's still standing would take the answer as
            // its own.
            ("syncthing-add-device", None) => {
                self.view.let_go();
                way.ask(crate::line::Line::new("syncthing-add-device", false, None));
            }
            (id, _) if GESTURES.contains(&id) => {
                if let Some(press) = press_of(id) {
                    self.press(press, way);
                }
            }
            _ => return Taken::No,
        }
        Taken::Yes
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let (columns, snapshot) = (way.columns(), self.snapshot.clone());
        let rows = self.view.rows(snapshot.as_deref(), columns, way.texts());
        let again = self
            .view
            .clicked(row, &rows, snapshot.as_deref(), columns, way.texts());
        if again {
            self.press(Press::Enter, way);
        } else {
            way.repaint();
        }
    }

    /// The question the view asked on the minibuffer's line, answered.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        self.typed(id, text, way);
    }

    /// The line was let go, and the view forgets the question.
    fn unanswered(&mut self, _: &'static str, way: &mut dyn Doorway) {
        self.view.let_go();
        way.repaint();
    }

    /// The clipboard's text, for the file picker up over the view.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        self.view.picker_pasted(text);
        self.view
            .settle_in(self.snapshot.as_deref(), way.columns(), way.texts());
        way.repaint();
    }

    /// The file picker's folder moved, or its devices were read: its rows
    /// again, when they changed.
    fn woken(&mut self, way: &mut dyn Doorway) {
        if self.view.look() {
            way.repaint();
        }
    }

    /// It says when its rows came or went (`Doorway::rows_changed`): a frame
    /// it asks for on news is only a frame.
    fn says_when_its_rows_change(&self) -> bool {
        true
    }

    /// What the footer heard: a fresh reading, or the preference changed in
    /// the other door.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(fresh) = news.downcast_ref::<Rc<Snapshot>>() {
            self.snapshot = Some(Rc::clone(fresh));
            self.view
                .settle_in(self.snapshot.as_deref(), way.columns(), way.texts());
            // A FOLDER OR A DEVICE MAY HAVE COME OR GONE with the reading.
            way.rows_changed();
        } else if let Some(prefs) = news.downcast_ref::<Preferences>() {
            self.view.took_preferences(*prefs);
            way.repaint();
        }
    }

    /// The file picker's `PgUp` and `PgDn` walk half of what the region
    /// shows.
    fn page_is(&self, lines: usize) {
        self.view.page_is(lines);
    }
}

/// CARRIES OUT WHAT THE VIEW ASKED FOR, in either door — the footer's region
/// and the window — each with its view, the line it asks Syncthing through
/// (`orders`) and, in the footer's, the preference the footer keeps for the
/// modeline's mark (`kept`): the footer's copy is set before the file is
/// written, so its watch finds its own write and nothing new.
///
/// One handler, so the two doors cannot do one errand two ways: what is said
/// is said in the door it was asked in, for as long as its class says.
pub fn carry_out(
    errand: Errand,
    view: &mut View,
    orders: Option<&Handle>,
    kept: Option<&Cell<Preferences>>,
    way: &mut dyn Doorway,
) {
    match errand {
        Errand::Nothing => {}
        // AN INFORMATIVE ROW COPIES ITSELF — `chrome::information_of`, and
        // `chrome::copied` for the one place the doors share: a glance. It
        // earns its keep here more than anywhere: a machine id is 63
        // characters nobody types twice.
        Errand::Copy(text) => {
            let said = crate::chrome::copied(&text, way.texts());
            way.say(said, crate::echo::Echo::Brief);
        }
        Errand::Speak(key) => {
            let said = way.texts().t(key);
            way.say(said, crate::echo::Echo::Normal);
        }
        Errand::Order(order) => match orders {
            Some(h) => h.send(*order),
            None => not_running(way),
        },
        Errand::Keep(prefs) => {
            keep_settings(&ronoctua_syncthing::prefs::store(), prefs, kept, way);
        }
        Errand::Web(address) => open_the_page(&address, way),
        Errand::Said(said) => way.say(said, crate::echo::Echo::Normal),
        // THE CLIPBOARD, off the loop, for the file picker (`pasted`).
        Errand::Paste => way.paste(),
        // ON THIS DOOR'S LINE — the footer's minibuffer, the window's own —
        // and the view is told WHAT it is being asked, so that the answer
        // knows what it answers.
        Errand::Ask(question) => {
            let id = question.command();
            view.asked(question);
            way.ask(crate::line::Line::new(id, false, None));
        }
        Errand::Close => way.close(),
    }
    way.repaint();
}

/// Writes the view's preference to `store` — the footer's copy (`kept`)
/// first, so its watch finds its own write and nothing new — and says it when
/// the file could not be written.
///
/// `parsed` AND NOT `read`, through the crate's own `keep`: this writes the
/// whole value back, and a reader's default over a file that will not parse
/// is how somebody loses what they chose. The crate refuses; this says so,
/// as a refusal to be read to the end (`Echo::Long`).
fn keep_settings(
    store: &ronoctua_store::Store<Preferences>,
    prefs: Preferences,
    kept: Option<&Cell<Preferences>>,
    way: &mut dyn Doorway,
) {
    if let Some(kept) = kept {
        kept.set(prefs);
    }
    if let Err(e) = ronoctua_syncthing::prefs::keep(store, prefs) {
        tracing::warn!(error = %e, "could not write syncthing.json");
        let said = way.texts().t(crate::key::ECHO_SYNCTHING_WILL_NOT_PARSE);
        way.say(said, crate::echo::Echo::Long);
    }
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

    /// FAILS IF: one of the view's commands does something else with the
    /// view shut than the footer did — the leader's question asked on the
    /// line, its answer sent as the order the command names, a folder chosen
    /// in the view, the commands that need Syncthing said when there is none
    /// — or a command that is not the view's is taken for it.
    #[test]
    fn a_closed_view_answers_its_own_commands_and_no_other() {
        let mut way = footer(Vec::new());
        assert_eq!(closed("syncthing-open", None, &mut way), Closed::Opens);
        assert_eq!(
            closed("syncthing-add-folder", None, &mut way),
            Closed::Opens
        );
        assert!(way.owed.is_empty());
        assert_eq!(
            closed("syncthing-add-device", None, &mut way),
            Closed::Taken
        );
        assert!(matches!(
            way.owed.as_slice(),
            [region::Effect::Ask(line)] if line.id == "syncthing-add-device" && !line.secret
        ));
        way.owed.clear();
        assert_eq!(
            closed("syncthing-add-device", Some("ABC-DEF"), &mut way),
            Closed::Taken
        );
        let said = texts().t(crate::key::ECHO_SYNCTHING_NOT_RUNNING);
        assert_eq!(
            way.owed.first(),
            Some(&region::Effect::Say(
                said.clone(),
                crate::echo::Echo::Normal
            ))
        );
        way.owed.clear();
        assert_eq!(
            closed("syncthing-add-device", Some("  "), &mut way),
            Closed::Taken
        );
        assert!(
            !way.owed
                .iter()
                .any(|e| matches!(e, region::Effect::Say(..))),
            "an empty answer sent an order"
        );
        way.owed.clear();
        assert_eq!(closed("syncthing-pause-all", None, &mut way), Closed::Taken);
        assert_eq!(
            way.owed,
            vec![region::Effect::Say(said, crate::echo::Echo::Normal)]
        );
        for gesture in GESTURES {
            assert!(press_of(gesture).is_some(), "{gesture} presses nothing");
            assert_eq!(closed(gesture, None, &mut way), Closed::Taken);
        }
        assert_eq!(closed("power-open", None, &mut way), Closed::NotMine);
    }

    /// FAILS IF: the one-line state says something other than what the
    /// reading holds — a stopped Syncthing read as running, or nothing said.
    #[test]
    fn the_state_is_one_line_of_the_footers_reading() {
        let mut way = footer(Vec::new());
        assert_eq!(closed("syncthing-state", None, &mut way), Closed::Taken);
        let stopped = texts().t(crate::key::ECHO_SYNCTHING_STOPPED);
        assert_eq!(
            way.owed,
            vec![region::Effect::Say(stopped, crate::echo::Echo::Normal)]
        );
    }

    /// FAILS IF: a preference a door keeps is not the footer's copy before
    /// the file is written — the modeline's mark would change only when the
    /// file's watch caught up — or a file that will not parse is written
    /// over, or a refused write is not said.
    #[test]
    fn a_kept_preference_is_the_footers_first_and_a_broken_file_is_refused() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-syncthing-keep-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        let store = ronoctua_store::Store::at(Some(dir.clone()), ronoctua_syncthing::prefs::FILE);
        let changed = Preferences {
            mark: ronoctua_syncthing::prefs::Mark::Never,
        };
        let kept = Cell::new(Preferences::default());
        let mut way = footer(Vec::new());
        keep_settings(&store, changed, Some(&kept), &mut way);
        assert_eq!(kept.get(), changed);
        assert_eq!(store.read(), changed);
        assert!(way.owed.is_empty(), "{:?}", way.owed);
        std::fs::write(dir.join(ronoctua_syncthing::prefs::FILE), "{ not json")
            .expect("a broken file");
        keep_settings(&store, Preferences::default(), None, &mut way);
        assert!(matches!(
            way.owed.as_slice(),
            [region::Effect::Say(_, crate::echo::Echo::Long)]
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a question the view asks is put on a door's line under any
    /// id but the command that carries its answer, or the view is not told
    /// what it asked — in the footer's door and in the window's alike.
    #[test]
    fn a_question_is_asked_alike_in_both_doors() {
        let mut view = View::fresh(Preferences::default());
        let mut way = footer(Vec::new());
        carry_out(
            Errand::Ask(crate::syncthing::Ask::NewDevice),
            &mut view,
            None,
            None,
            &mut way,
        );
        assert!(matches!(
            way.owed.first(),
            Some(region::Effect::Ask(line)) if line.id == "syncthing-add-device"
        ));
        assert_eq!(view.asking(), Some(&crate::syncthing::Ask::NewDevice));
        let mut ui = crate::door::Ui::for_tests(texts());
        let mut other = View::fresh(Preferences::default());
        carry_out(
            Errand::Ask(crate::syncthing::Ask::NewDevice),
            &mut other,
            None,
            None,
            &mut ui,
        );
        assert_eq!(other.asking(), Some(&crate::syncthing::Ask::NewDevice));
    }

    /// FAILS IF: a reading or the mark's preference, told by the type the
    /// footer tells each (`Rc<Snapshot>`, `Preferences`: `surface/mod.rs`),
    /// is not the one the region takes — the open view would freeze — or a
    /// reading does not say its rows may have come or gone (a folder added),
    /// or a preference asks more than a frame, or a news of a type it does
    /// not read asks anything.
    #[test]
    fn what_the_footer_tells_is_heard_by_its_type() {
        let mut way = footer(Vec::new());
        let mut syncthing = Syncthing {
            view: View::fresh(Preferences::default()),
            snapshot: None,
        };
        let fresh = Rc::new(Snapshot::default());
        Region::heard(&mut syncthing, &fresh, &mut way);
        assert!(
            syncthing
                .snapshot
                .as_ref()
                .is_some_and(|s| Rc::ptr_eq(s, &fresh)),
            "the reading told is not the one drawn"
        );
        assert_eq!(way.owed, vec![region::Effect::RowsChanged]);
        way.owed.clear();
        let changed = Preferences {
            mark: ronoctua_syncthing::prefs::Mark::Never,
        };
        Region::heard(&mut syncthing, &changed, &mut way);
        assert_eq!(syncthing.view.preferences(), changed);
        assert_eq!(way.owed, vec![region::Effect::Repaint]);
        way.owed.clear();
        Region::heard(&mut syncthing, &(), &mut way);
        assert!(way.owed.is_empty(), "{:?}", way.owed);
    }
}
