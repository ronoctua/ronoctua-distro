//! THE SOUND VIEW IN THE FOOTER'S REGION — the second door onto
//! [`crate::audio`], beside its window ([`crate::audio_window`]), the one
//! errand handler both doors carry out ([`carry_out`]) and the one meter
//! both open the microphone through ([`Meter`]).
//!
//! # What it reads, and through what
//!
//! The footer's own PipeWire threads — alive for the whole session, because
//! the modeline draws its two marks from them and the volume keys act with
//! no view open — their last reading, and the person's sound settings the
//! footer keeps (`audio.json`, which the volume keys step by): all lent by
//! type ([`crate::region::kept`]). A fresh reading, a level of the open
//! microphone and a setting changed in the other door reach the region as
//! news ([`Region::heard`]).
//!
//! # The microphone opens and closes with the cursor
//!
//! The view says which source belongs open (`View::listening`); the door
//! asks for it after a key, after a click, after a reading moved the cursor,
//! and closes it when the view closes — through the line it asks PipeWire
//! through, the footer's here and the window's own there.

use std::cell::Cell;
use std::rc::Rc;

use crate::audio::{Errand, Press, View, press_of, press_of_key};
use crate::region::{self, Closed, Doorway, Entry, Region, Row, Seen, Taken};
use ronoctua_audio::prefs::Preferences;
use ronoctua_audio::{Handle, Order, Snapshot};

/// The region's id, as the footer ranks it (`surface::PRECEDENCE`).
pub const ID: &str = "audio";

/// The view's region, as the binary registers it (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The commands that are the view's own gestures, by name. They are commands
/// so the registry can name them, the hint line can be generated from them
/// and `LEADER :` can reach them; with the view shut they do nothing, which
/// is what a gesture of a closed view means.
const GESTURES: &[&str] = &[
    "audio-up",
    "audio-down",
    "audio-enter",
    "audio-less",
    "audio-more",
    "audio-mute",
    "audio-default",
];

/// What a command does while the view is closed.
fn closed(command: &str, _: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "audio-open" => Closed::Opens,
        "audio-window" => {
            way.open_window(vec!["audio".to_owned()]);
            Closed::Taken
        }
        id if GESTURES.contains(&id) => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// Opens the view on the footer's last reading and settings — and opens the
/// microphone under the cursor, if the cursor landed on one: opening the view
/// IS the first move of the cursor.
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    Box::new(opened(way))
}

/// The view [`open`] opens.
fn opened(way: &mut dyn Doorway) -> Sound {
    let prefs = region::kept::<Cell<Preferences>>(way)
        .map_or_else(|| ronoctua_audio::prefs::store().read(), |p| p.get());
    let snapshot = region::kept::<Snapshot>(way);
    let mut view = View::fresh(prefs);
    view.settle_in(snapshot.as_deref(), way.columns(), way.texts());
    let mut meter = Meter::default();
    meter.follow_the_microphone(&view, region::kept::<Handle>(way).as_deref());
    Sound {
        view,
        snapshot,
        meter,
    }
}

/// The view, open in the footer's region: the reading it draws and the
/// microphone it asked for.
struct Sound {
    view: View,
    /// The footer's last reading — the same `Rc` the footer holds.
    snapshot: Option<Rc<Snapshot>>,
    meter: Meter,
}

impl Sound {
    /// One press — a key, a gesture by name, a second click — and what it
    /// asks for, carried out.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = self
            .view
            .key(press, self.snapshot.as_deref(), way.columns(), way.texts());
        // THE SURFACE FIRST, THEN THE CURSOR — the order every view of this
        // shape keeps.
        way.follow();
        let orders = region::kept::<Handle>(way);
        // THE METER FOLLOWS THE CURSOR, and a key is a way of moving it.
        self.meter
            .follow_the_microphone(&self.view, orders.as_deref());
        let wish = region::kept::<Cell<Preferences>>(way);
        carry_out(
            errand,
            &mut self.meter,
            orders.as_deref(),
            wish.as_deref(),
            way,
        );
    }
}

impl Region for Sound {
    fn id(&self) -> &'static str {
        ID
    }

    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.view
            .rows(self.snapshot.as_deref(), seen.columns, seen.texts)
            .into_iter()
            .map(|r| Row {
                // A HEADING IS NOT A ROW TO CLICK ON — the row's own answer
                // (`audio::Row::takes`), which the window gives too.
                takes: r.takes(),
                fields: r.fields,
            })
            .collect()
    }

    /// SOUND: the keys it teaches depend on the row, so a cursor on a
    /// setting is not told about `m`.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view
            .hint(self.snapshot.as_deref(), seen.columns, seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// BOTH DOORS THROUGH ONE READER. The footer's key and the window's
    /// (`door::keys::key_of`) are the same `Key`, and `audio::press_of_key`
    /// reads it — so a key written in one place is answered in both, a key
    /// written nowhere is answered in neither, and `C-m` is not `m`.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let Some(press) = press_of_key(key) else {
            return Taken::No;
        };
        self.press(press, way);
        Taken::Yes
    }

    fn command(&mut self, id: &str, _: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // ALREADY OPEN: left exactly as it is. The way in here is the
            // MOUSE — a click on the mute mark runs `audio-open` — and
            // rebuilding the view would move a cursor somebody had put
            // somewhere. The page only follows it.
            "audio-open" => way.follow(),
            "audio-window" => way.open_window(vec!["audio".to_owned()]),
            id if GESTURES.contains(&id) => {
                if let Some(press) = press_of(id) {
                    self.press(press, way);
                }
            }
            _ => return Taken::No,
        }
        Taken::Yes
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19: a click on
    /// the row the cursor is already on is that row's `RET`. And a click
    /// moves a microphone too: the meter follows the cursor, and a click is
    /// a way of moving the cursor.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let (columns, snapshot) = (way.columns(), self.snapshot.clone());
        let rows = self.view.rows(snapshot.as_deref(), columns, way.texts());
        let again = self
            .view
            .clicked(row, &rows, snapshot.as_deref(), columns, way.texts());
        self.meter
            .follow_the_microphone(&self.view, region::kept::<Handle>(way).as_deref());
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

    /// What the footer heard: a fresh reading, a level of the open
    /// microphone, or the settings changed in the other door.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(fresh) = news.downcast_ref::<Rc<Snapshot>>() {
            self.snapshot = Some(Rc::clone(fresh));
            self.view
                .settle_in(self.snapshot.as_deref(), way.columns(), way.texts());
            // A DEVICE CAN BE UNPLUGGED WHILE ITS ROW IS UNDER THE CURSOR,
            // and then the microphone this door asked for is a source that
            // is gone. `settle_in` moved the cursor; this tells the meter.
            self.meter
                .follow_the_microphone(&self.view, region::kept::<Handle>(way).as_deref());
            // AND THE ROWS MAY HAVE COME OR GONE with the device.
            way.rows_changed();
        } else if let Some(ronoctua_audio::Event::Level(level)) =
            news.downcast_ref::<ronoctua_audio::Event>()
        {
            // A LEVEL IS NOT A SNAPSHOT: a frame only when the bar moved a
            // cell, and never the rows counted — it is ten times a second.
            if self.view.took_level(*level, way.columns()) {
                way.repaint();
            }
        } else if let Some(prefs) = news.downcast_ref::<Preferences>() {
            self.view.took_preferences(*prefs);
            way.repaint();
        }
    }

    /// THE MICROPHONE CLOSES WITH THE VIEW: the footer's thread lives on past
    /// the view, so nothing else would ever send the order.
    fn closing(&mut self, way: &mut dyn Doorway) {
        self.meter
            .stop_listening(region::kept::<Handle>(way).as_deref());
    }
}

/// THE MICROPHONE A DOOR ASKED TO HAVE OPEN — both doors', the one place the
/// order to open or close one is sent from.
///
/// It shadows `View::listening` so the order is sent when it CHANGES and not
/// on every keystroke. Repeating `Listen(Some(61))` would drop the open child
/// and build another one, and a microphone that closed and reopened on every
/// arrow is a meter that flickers.
#[derive(Debug, Default)]
pub struct Meter {
    listening: Option<u32>,
}

impl Meter {
    /// Tells the meter which source the cursor is on, when that changed —
    /// through `orders`, the door's line to PipeWire.
    pub fn follow_the_microphone(&mut self, view: &View, orders: Option<&Handle>) {
        let want = view.listening();
        if want == self.listening {
            return;
        }
        self.listening = want;
        if let Some(h) = orders {
            h.send(Order::Listen(want));
        }
    }

    /// Closes whatever microphone this door has open — sent while there is
    /// still a thread to receive it, which is the difference between "it
    /// closed" and "it closed when the process happened to exit".
    pub fn stop_listening(&mut self, orders: Option<&Handle>) {
        if self.listening.is_none() {
            return;
        }
        self.listening = None;
        if let Some(h) = orders {
            h.send(Order::Listen(None));
        }
    }

    /// The source this door asked to have open.
    #[must_use]
    pub const fn listening(&self) -> Option<u32> {
        self.listening
    }
}

/// CARRIES OUT WHAT THE VIEW ASKED FOR, in either door — the footer's region
/// and the window — each with its own microphone (`meter`), the line it asks
/// PipeWire through (`orders`) and, in the footer's, the settings the footer
/// keeps for the volume keys (`kept`): the footer's copy is set before the
/// file is written, so its watch finds its own write and nothing new.
///
/// One handler, so the two doors cannot do one errand two ways: what is said
/// is said in the door it was asked in, for as long as its class says.
pub fn carry_out(
    errand: Errand,
    meter: &mut Meter,
    orders: Option<&Handle>,
    kept: Option<&Cell<Preferences>>,
    way: &mut dyn Doorway,
) {
    match errand {
        Errand::Nothing => {}
        // AN INFORMATIVE ROW COPIES ITSELF — `chrome::information_of`, and
        // `chrome::copied` for the one place the doors share: a glance.
        Errand::Copy(text) => {
            let said = crate::chrome::copied(&text, way.texts());
            way.say(said, crate::echo::Echo::Brief);
        }
        Errand::Speak(key) => {
            let said = way.texts().t(key);
            way.say(said, crate::echo::Echo::Normal);
        }
        Errand::Order(order) => match orders {
            Some(h) => h.send(order),
            None => {
                let said = way.texts().t(crate::key::AUDIO_NO_PIPEWIRE);
                way.say(said, crate::echo::Echo::Normal);
            }
        },
        // A PHOTOGRAPH'S DOOR, OR A TEST'S, WRITES NOTHING OF THE PERSON'S
        // (`Doorway::photographing`): the setting turns in the view and in
        // the footer's copy, and `audio.json` — under the home of whoever
        // runs the door — is left as it is.
        Errand::Keep(prefs) if way.photographing() => {
            if let Some(kept) = kept {
                kept.set(prefs);
            }
        }
        Errand::Keep(prefs) => {
            keep_settings(&ronoctua_audio::prefs::store(), prefs, kept, way);
        }
        // THE MICROPHONE CLOSES BEFORE THE VIEW DOES.
        Errand::Close => {
            meter.stop_listening(orders);
            way.close();
        }
    }
    way.repaint();
}

/// Writes the sound settings to `store` — the footer's copy (`kept`) first,
/// so its watch finds its own write and nothing new — and says it when the
/// file could not be written.
///
/// `parsed` AND NOT `read`, through the crate's own `keep`: this writes the
/// whole value back, and a reader's default over a file that will not parse
/// is how somebody loses what they chose — one press of `S-<right>` over a
/// broken `audio.json` and it was gone. Both doors write the file, and the
/// footer watches it. Not written is a refusal to be read to the end
/// (`Echo::Long`).
fn keep_settings(
    store: &ronoctua_store::Store<Preferences>,
    prefs: Preferences,
    kept: Option<&Cell<Preferences>>,
    way: &mut dyn Doorway,
) {
    if let Some(kept) = kept {
        kept.set(prefs);
    }
    if let Err(e) = ronoctua_audio::prefs::keep(store, prefs) {
        tracing::warn!(error = %e, "could not write audio.json");
        let said = way.texts().t(crate::key::ECHO_AUDIO_WILL_NOT_PARSE);
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

    /// FAILS IF: a command of the view does something else with the view
    /// closed than it did in the footer, or a command that is not the
    /// view's is taken for it.
    #[test]
    fn a_closed_view_answers_its_own_commands_and_no_other() {
        let mut way = footer(Vec::new());
        assert_eq!(closed("audio-open", None, &mut way), Closed::Opens);
        assert_eq!(closed("audio-window", None, &mut way), Closed::Taken);
        assert_eq!(
            way.owed,
            vec![region::Effect::OpenWindow(vec!["audio".to_owned()])]
        );
        for gesture in GESTURES {
            assert!(press_of(gesture).is_some(), "{gesture} presses nothing");
            assert_eq!(closed(gesture, None, &mut way), Closed::Taken);
        }
        assert_eq!(closed("pointers-open", None, &mut way), Closed::NotMine);
        assert_eq!(way.owed.len(), 1);
    }

    /// FAILS IF: the region opens on settings of its own instead of the ones
    /// the footer keeps — a step chosen in the window would not be the one
    /// drawn.
    #[test]
    fn the_region_opens_on_the_footers_settings() {
        let changed = Preferences {
            flash: !Preferences::default().flash,
            ..Preferences::default()
        };
        let mut way = footer(vec![Rc::new(Cell::new(changed))]);
        assert_eq!(opened(&mut way).view.preferences(), changed);
    }

    /// FAILS IF: a setting a door keeps is not the footer's copy before the
    /// file is written — the volume keys would step by the old one, and the
    /// footer's watch would take its own write for news — or is written
    /// over a file that will not parse, or a write refused is not said.
    #[test]
    fn a_kept_setting_is_the_footers_first_and_a_broken_file_is_refused() {
        let dir = std::env::temp_dir().join(format!("ronoctua-sound-keep-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        let store = ronoctua_store::Store::at(Some(dir.clone()), ronoctua_audio::prefs::FILE);
        let changed = Preferences {
            flash: !Preferences::default().flash,
            ..Preferences::default()
        };
        let kept = Cell::new(Preferences::default());
        let mut way = footer(Vec::new());
        keep_settings(&store, changed, Some(&kept), &mut way);
        assert_eq!(kept.get(), changed);
        assert_eq!(store.read(), changed);
        assert!(way.owed.is_empty(), "{:?}", way.owed);
        std::fs::write(dir.join(ronoctua_audio::prefs::FILE), "{ not json").expect("a broken file");
        keep_settings(&store, Preferences::default(), None, &mut way);
        assert!(matches!(
            way.owed.as_slice(),
            [region::Effect::Say(_, crate::echo::Echo::Long)]
        ));
        assert_eq!(
            std::fs::read_to_string(dir.join(ronoctua_audio::prefs::FILE))
                .ok()
                .as_deref(),
            Some("{ not json"),
            "a file that will not parse was written over"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: an order with no PipeWire to carry it is swallowed, a copy
    /// is not said as the word it is, or a microphone that is not open is
    /// "closed" with an order — in the footer's door and in the window's
    /// alike.
    #[test]
    fn an_errand_is_carried_out_alike_in_both_doors() {
        let mut way = footer(Vec::new());
        let mut meter = Meter::default();
        carry_out(
            Errand::Order(Order::Listen(None)),
            &mut meter,
            None,
            None,
            &mut way,
        );
        let said = texts().t(crate::key::AUDIO_NO_PIPEWIRE);
        assert_eq!(
            way.owed.first(),
            Some(&region::Effect::Say(
                said.clone(),
                crate::echo::Echo::Normal
            ))
        );
        carry_out(Errand::Close, &mut meter, None, None, &mut way);
        assert!(way.owed.contains(&region::Effect::Close));
        assert_eq!(meter.listening(), None);
        let mut ui = crate::door::Ui::for_tests(texts());
        carry_out(
            Errand::Order(Order::Listen(None)),
            &mut Meter::default(),
            None,
            None,
            &mut ui,
        );
        assert_eq!(ui.message.as_deref(), Some(said.as_str()));
        carry_out(Errand::Close, &mut Meter::default(), None, None, &mut ui);
        assert!(ui.quit);
    }

    /// FAILS IF: a reading, a level or the settings, told by the type the
    /// footer tells each (`Rc<Snapshot>`, `ronoctua_audio::Event`,
    /// `Preferences`: `surface/mod.rs`), is not the one the region takes —
    /// the open view would freeze, the meter stop — or a reading does not
    /// say its rows may have come or gone, or a level asks more than a frame
    /// when its bar moved (ten a second, every row would be built for it),
    /// or a news of a type it does not read asks anything.
    #[test]
    fn what_the_footer_tells_is_heard_by_its_type() {
        let mut way = footer(vec![Rc::new(Cell::new(Preferences::default()))]);
        let mut sound = opened(&mut way);
        way.owed.clear();
        let fresh = Rc::new(Snapshot::default());
        Region::heard(&mut sound, &fresh, &mut way);
        assert!(
            sound
                .snapshot
                .as_ref()
                .is_some_and(|s| Rc::ptr_eq(s, &fresh)),
            "the reading told is not the one drawn"
        );
        assert_eq!(way.owed, vec![region::Effect::RowsChanged]);
        way.owed.clear();
        Region::heard(&mut sound, &ronoctua_audio::Event::Level(100), &mut way);
        assert_eq!(
            way.owed,
            vec![region::Effect::Repaint],
            "a level is a frame and nothing more"
        );
        way.owed.clear();
        Region::heard(&mut sound, &ronoctua_audio::Event::Level(100), &mut way);
        assert!(
            way.owed.is_empty(),
            "a bar that did not move asked {:?}",
            way.owed
        );
        let changed = Preferences {
            flash: !Preferences::default().flash,
            ..Preferences::default()
        };
        Region::heard(&mut sound, &changed, &mut way);
        assert_eq!(sound.view.preferences(), changed);
        assert_eq!(way.owed, vec![region::Effect::Repaint]);
        way.owed.clear();
        Region::heard(&mut sound, &(), &mut way);
        assert!(way.owed.is_empty(), "{:?}", way.owed);
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a title, the air between sections, a
    /// sentence about a whole section — or leaves unlit one it takes. THE
    /// LIGHT IS THE CLICK: every row clicked once, on a view of its own over
    /// the same machine (`audio::a_machine`), and lit exactly where something
    /// happened.
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
        let fresh = || {
            let mut way = footer(vec![
                Rc::new(Cell::new(Preferences::default())) as region::Kept,
                Rc::new(crate::audio::a_machine()) as region::Kept,
            ]);
            let sound = opened(&mut way);
            way.owed.clear();
            (sound, way)
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
