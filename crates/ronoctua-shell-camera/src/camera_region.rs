//! THE CAMERA'S SETTINGS IN THE FOOTER'S REGION (`LEADER s c c`) — the
//! settings' view as a [`Region`] the footer hosts by its id, and the camera
//! app's keys from anywhere, told to the studio with no window open.
//!
//! # What it reads, and what it keeps
//!
//! The studio's state is the footer's: its thread hears it (the modeline's
//! recording mark reads the same state), and it is LENT to the region when it
//! opens and when it is asked a closed command, and TOLD to it as it changes
//! ([`Region::heard`]). THE CAMERAS ARE THE REGION'S OWN, read while it is
//! shown and never before ([`CameraWatch`]): reading a camera opens it, and a
//! footer that read them as it started blinked the camera's light with nobody
//! having asked. The window reads them the same way. What the region keeps is
//! the view's cursor, the cameras, and the snapshot its rows read — the two
//! files beside what it was told, read again after every change, as the
//! window reads them. The cameras come after the view opened, so a cursor
//! nobody moved goes where the view opens once they are read, and the page
//! after it (`View::reread`).

use crate::camera_settings::{CameraWatch, Carried, Press, Snapshot, View};
use crate::region::{Closed, Doorway, Entry, Paging, Region, Row, Seen, Taken};
use ronoctua_camera::Camera;
use ronoctua_studio::wire::State;

/// The id the footer ranks the camera's settings under (`PRECEDENCE`).
pub const ID: &str = "camera";

/// The camera's settings, as the binary registers them (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The camera's settings, open in the footer.
struct Settings {
    view: View,
    /// What the rows read: the two files, the cameras and the studio's state.
    snap: Snapshot,
    /// The cameras as the region's watch last read them — `None` until its
    /// first reading.
    cameras: Option<Vec<Camera>>,
    /// The watch, while the region is shown — never in a photograph, which
    /// reaches nothing of the machine.
    watch: Option<CameraWatch>,
}

/// The studio's state the footer lends the call.
fn lent_of(way: &dyn Doorway) -> Option<State> {
    way.lent().get::<Option<State>>().cloned().flatten()
}

/// Tells the studio something, saying on the door's line what refused.
fn studio_ask(ask: &ronoctua_studio::wire::Ask, way: &mut dyn Doorway) {
    let said = match ronoctua_studio::client::ask(ask) {
        Ok(ronoctua_studio::wire::Say::Refused(t)) => {
            crate::camera::trouble_sentence(&t, way.texts())
        }
        Ok(_) => return,
        Err(_) => way.texts().t(crate::key::STUDIO_NOT_RUNNING),
    };
    way.say(said, crate::echo::Echo::Normal);
}

/// THE CAMERA APP'S KEYS, from anywhere: the same press the app makes, told
/// to the studio — a photo or a recording does not need the window open.
/// `true` when `command` was one.
fn remote(command: &str, way: &mut dyn Doorway) -> bool {
    if !matches!(
        command,
        "camera-photo"
            | "camera-record"
            | "camera-effect-next"
            | "camera-effect-previous"
            | "camera-background-next"
            | "camera-param-less"
            | "camera-param-more"
            | "camera-camera-next"
            | "camera-mirror"
            | "camera-scene-next"
            | "camera-scene-previous"
            | "camera-script"
            | "camera-next-cue"
            | "camera-virtual"
            | "camera-stream"
    ) {
        return false;
    }
    // A PHOTOGRAPH REACHES NOTHING OF THE MACHINE: its studio is not asked.
    if way.photographing() {
        return true;
    }
    if let Some(press) = crate::camera::press_of(command) {
        let studio = lent_of(way);
        let errand = crate::camera::View::default().key(press, studio.as_ref());
        if let crate::camera::Errand::Tell(ask) = errand {
            studio_ask(&ask, way);
        }
    }
    true
}

/// The commands that do the same with the settings open or closed: the
/// app's keys, the two windows, and the questions. `true` when `command` was
/// one of them. An answer is carried out against `snap`.
fn anywhere(command: &str, argument: Option<&str>, snap: &Snapshot, way: &mut dyn Doorway) -> bool {
    if remote(command, way) {
        return true;
    }
    match command {
        "camera-open" => way.open_window(vec!["camera".to_owned()]),
        "camera-settings-window" => way.open_window(vec!["camera-settings".to_owned()]),
        // THE CAMERA'S QUESTIONS: with nothing typed this opens the question;
        // with an argument it IS the answer (`s s d`'s shape) — and an empty
        // one is never mind, as it is in the window.
        _ => match crate::camera_settings::question(command) {
            Some(q) => match argument {
                Some(said) => {
                    let studio = lent_of(way);
                    let errand = crate::camera_settings::answered(q.id, said, studio.as_ref());
                    let _ = crate::camera_settings::carry_in(errand, snap, way);
                }
                None => way.ask(crate::line::Line::new(q.id, q.secret, None)),
            },
            None => return false,
        },
    }
    true
}

/// What a command does with the settings CLOSED.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    if command == "camera-settings-open" {
        return Closed::Opens;
    }
    // THE VIEW'S OWN PRESSES, by name, with no view: nothing to press.
    if crate::camera_settings::press_of(command).is_some() {
        return Closed::Taken;
    }
    // AN ANSWER NEEDS NO CAMERA — a scene's name, a script, the stream's
    // address or key — and none is read for it.
    let snap = if crate::camera_settings::question(command).is_some() && argument.is_some() {
        Snapshot::read(None, lent_of(way))
    } else {
        Snapshot::default()
    };
    if anywhere(command, argument, &snap, way) {
        Closed::Taken
    } else {
        Closed::NotMine
    }
}

/// The settings, opened on what the footer lends — the cameras still to be
/// read, by the watch the region raises once it is shown.
fn open(_command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    Box::new(Settings::opened(way))
}

impl Settings {
    /// [`open`]'s region, as it is before it is shown: the cameras being
    /// read, the cursor on the first row that does something meanwhile.
    fn opened(way: &mut dyn Doorway) -> Self {
        let snap = Snapshot::read(None, lent_of(way));
        let mut view = View::fresh();
        // THE FILE PICKER'S FOLDER, watched through the region's own wake —
        // and not in a photograph, which reaches nothing of the machine.
        if !way.photographing() {
            view.watch_with(way.wake());
        }
        view.settle_in(&snap, way.columns(), way.texts());
        Self {
            view,
            snap,
            cameras: None,
            watch: None,
        }
    }

    /// The page follows the cursor — after a person's key or click. The
    /// footer finds the cursor as drawn (`Region::cursor`); the window
    /// follows its own after every key and click.
    fn follow(way: &mut dyn Doorway) {
        way.page(Paging::Follow);
    }

    /// Rebuilds what the rows read — the cursor kept on its row, or put
    /// where the view opens when nobody moved it, and the page after it
    /// (`View::reread`, the window's too).
    fn reread(&mut self, studio: Option<State>, way: &mut dyn Doorway) {
        let now = Snapshot::read(self.cameras.clone(), studio);
        self.view.reread(&self.snap, &now, way);
        self.snap = now;
        way.repaint();
    }

    /// THE CAMERAS ARRIVED — the watch's reading, or a photograph's planted
    /// one — and the rows are read again over them.
    fn arrived(&mut self, cameras: Vec<Camera>, way: &mut dyn Doorway) {
        self.cameras = Some(cameras);
        let _ = self.view.look();
        self.reread_lent(way);
    }

    /// The same, from what the footer lends.
    fn reread_lent(&mut self, way: &mut dyn Doorway) {
        let studio = lent_of(way);
        self.reread(studio, way);
    }

    /// A press — of a key, of a command by name, or `RET` from a second
    /// click on the row the cursor is on (invariant 19).
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = self.view.key(press, &self.snap, way.columns(), way.texts());
        Self::follow(way);
        match crate::camera_settings::carry_in(errand, &self.snap, way) {
            Carried::Said | Carried::Quiet => self.reread_lent(way),
            Carried::Handed | Carried::Left => {}
        }
    }
}

impl Region for Settings {
    fn id(&self) -> &'static str {
        ID
    }

    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.view
            .rows(&self.snap, seen.columns, seen.texts)
            .into_iter()
            .map(|r| Row {
                takes: r.takes(),
                fields: r.fields,
            })
            .collect()
    }

    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view.hint(&self.snap, seen.columns, seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// BOTH DOORS THROUGH ONE READER — `camera_settings::press_of_key`, which
    /// reads a letter by its whole notation: `C-d` is not `d`. THE FILE
    /// PICKER, while it is up over the view, takes what it reads — every
    /// letter, and its own keys.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        if self.view.picker_takes(key) {
            self.press(Press::Pick(key), way);
            return Taken::Yes;
        }
        match crate::camera_settings::press_of_key(key) {
            Some(press) => {
                self.press(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        if id == "camera-settings-open" {
            // OPEN ALREADY, the page goes to the cursor; just opened, it
            // starts on its first page with the cursor in sight.
            Self::follow(way);
            return Taken::Yes;
        }
        if let Some(press) = crate::camera_settings::press_of(id) {
            self.press(press, way);
            return Taken::Yes;
        }
        let answers = crate::camera_settings::question(id).is_some() && argument.is_some();
        if !anywhere(id, argument, &self.snap, way) {
            return Taken::No;
        }
        if answers {
            self.reread_lent(way);
        }
        Taken::Yes
    }

    /// THE MOUSE AIMS, AND THE SECOND CLICK ACTS — invariant 19.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let rows = self.view.rows(&self.snap, way.columns(), way.texts());
        if self.view.clicked(row, &rows) {
            self.press(Press::Enter, way);
        } else {
            way.repaint();
        }
    }

    /// What was typed for one of the camera's questions — handed to the SAME
    /// `camera_settings::answered` a window's line reaches, and carried out
    /// by the same handler.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        let studio = lent_of(way);
        let errand = crate::camera_settings::answered(id, text, studio.as_ref());
        let _ = crate::camera_settings::carry_in(errand, &self.snap, way);
        self.reread_lent(way);
    }

    /// The clipboard's text, for the file picker up over the view.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        self.view.picker_pasted(text);
        Self::follow(way);
        way.repaint();
    }

    /// THE CAMERAS ARE READ WHILE IT IS SHOWN, and from no earlier moment:
    /// the watch raised here is the first time this footer opens a camera.
    ///
    /// A PHOTOGRAPH OPENS NONE: its watch is planted with the cameras of the
    /// scene its camera's environment names
    /// (`camera_settings_window::planted_watch`, none by default), and with
    /// no loop to wake, the reading is taken here by the handler a wake
    /// reaches.
    fn shown(&mut self, way: &mut dyn Doorway) {
        if self.watch.is_some() {
            return;
        }
        if way.photographing() {
            let watch = crate::camera_settings_window::planted_watch();
            if let Some(cameras) = watch.arrived() {
                self.arrived(cameras, way);
            }
            self.watch = Some(watch);
        } else {
            self.watch = Some(CameraWatch::raise(way.wake()));
        }
    }

    /// And the watch goes with the view: a camera plugged after it closed
    /// is not read.
    fn closing(&mut self, _way: &mut dyn Doorway) {
        self.watch = None;
    }

    /// The file picker's folder moved, or its devices were read — or the
    /// region's watch read the cameras.
    fn woken(&mut self, way: &mut dyn Doorway) {
        if let Some(cameras) = self.watch.as_ref().and_then(CameraWatch::arrived) {
            self.arrived(cameras, way);
        } else if self.view.look() {
            way.repaint();
        }
    }

    /// The studio's new state, as the footer heard it.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(studio) = news.downcast_ref::<Option<State>>() {
            self.reread(studio.clone(), way);
        }
    }

    /// How many lines the region shows, for the file picker's `PgUp` and
    /// `PgDn`.
    fn page_is(&self, lines: usize) {
        self.view.page_is(lines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera_settings::Target;
    use crate::region::{Effect, Lent, Recorder};

    fn texts() -> std::rc::Rc<ronoctua_i18n::Texts> {
        std::rc::Rc::new(
            ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language"),
        )
    }

    fn way(texts: &std::rc::Rc<ronoctua_i18n::Texts>) -> Recorder {
        Recorder::new(
            std::rc::Rc::clone(texts),
            120,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
    }

    fn a_studio() -> Option<State> {
        Some(State {
            scenes: vec!["camera".into(), "screen".into()],
            scene: "camera".into(),
            ..State::default()
        })
    }

    /// The settings as a photograph's footer opens and shows them: no camera
    /// read, the studio answering, and nothing of the machine reached.
    fn shown_settings(t: &std::rc::Rc<ronoctua_i18n::Texts>) -> (Settings, Recorder) {
        let mut w = way(t);
        let mut s = Settings::opened(&mut w);
        s.shown(&mut w);
        s.heard(&a_studio(), &mut w);
        w.owed.clear();
        (s, w)
    }

    /// FAILS IF: a question of the camera stops being asked on the door's
    /// line with the view left on screen — the stream's key masked — or one
    /// asked by name with the view closed is not asked at all.
    #[test]
    fn a_question_is_asked_on_the_doors_line_open_or_closed() {
        let t = texts();
        let studio = a_studio();
        let mut w = way(&t).lending(Lent::of(vec![&studio]));
        assert_eq!(
            closed(crate::camera_settings::STREAM_KEY, None, &mut w),
            Closed::Taken
        );
        assert!(matches!(
            &w.owed[..],
            [Effect::Ask(line)] if line.id == crate::camera_settings::STREAM_KEY && line.secret
        ));
        w.owed.clear();
        let mut s = open("camera-settings-open", None, &mut w);
        assert_eq!(s.command("camera-settings-open", None, &mut w), Taken::Yes);
        w.owed.clear();
        assert_eq!(
            s.command(crate::camera_settings::SCENE_ADD, None, &mut w),
            Taken::Yes
        );
        assert!(matches!(
            &w.owed[..],
            [Effect::Ask(line)] if line.id == crate::camera_settings::SCENE_ADD && !line.secret
        ));
    }

    /// FAILS IF: the settings open on nothing the footer lends — no state of
    /// the studio's — or a press of the view does not ask the page to follow
    /// its cursor.
    #[test]
    fn the_settings_open_on_what_the_footer_lends() {
        let t = texts();
        let studio = a_studio();
        let mut w = way(&t).lending(Lent::of(vec![&studio]));
        let mut s = open("camera-settings-open", None, &mut w);
        let _ = s.command("camera-settings-open", None, &mut w);
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: 120,
            lines: 12,
            zone: &zone,
            photographing: true,
            lent: Lent::none(),
        };
        let rows = s.rows(&seen);
        assert!(!rows.is_empty());
        assert!(rows.iter().any(|r| !r.takes), "no heading among the rows");
        w.owed.clear();
        assert_eq!(s.command("camera-settings-down", None, &mut w), Taken::Yes);
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Page(Paging::Follow))),
            "{:?}",
            w.owed
        );
    }

    /// FAILS IF: a closed command of the view's own presses is lost to the
    /// footer's "does nothing yet", the app's windows are not opened, or a
    /// command that is not the camera's is taken.
    #[test]
    fn the_closed_settings_answer_their_own_commands_only() {
        let t = texts();
        let mut w = way(&t);
        assert_eq!(closed("camera-settings-open", None, &mut w), Closed::Opens);
        assert_eq!(closed("camera-settings-down", None, &mut w), Closed::Taken);
        assert!(w.owed.is_empty());
        assert_eq!(closed("camera-open", None, &mut w), Closed::Taken);
        assert_eq!(w.owed, vec![Effect::OpenWindow(vec!["camera".to_owned()])]);
        assert_eq!(closed("stick-open", None, &mut w), Closed::NotMine);
    }

    /// FAILS IF: what the footer hears about the studio does not reach the
    /// open settings, or a reading of another kind moves them.
    #[test]
    fn the_studio_is_heard() {
        let t = texts();
        let mut w = way(&t);
        let mut s = open("camera-settings-open", None, &mut w);
        w.owed.clear();
        s.heard(&a_studio(), &mut w);
        assert_eq!(w.owed, vec![Effect::Repaint]);
        w.owed.clear();
        s.heard(&7_u8, &mut w);
        assert!(
            w.owed.is_empty(),
            "a reading of another kind moved the settings"
        );
    }

    /// FAILS IF: a photographed region — which reaches nothing of the
    /// machine, its cameras least of all — draws the cameras as still being
    /// read rather than a machine with none, the picture the baseline holds.
    #[test]
    fn a_photograph_reads_no_camera_and_draws_none() {
        let t = texts();
        let mut w = way(&t);
        let mut s = open("camera-settings-open", None, &mut w);
        s.shown(&mut w);
        s.woken(&mut w);
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: 120,
            lines: 12,
            zone: &zone,
            photographing: true,
            lent: Lent::none(),
        };
        let says = |key: &str| {
            s.rows(&seen)
                .iter()
                .any(|r| r.fields.iter().any(|f| f.text == t.t(key)))
        };
        assert!(says(crate::key::STUDIO_TROUBLE_NO_CAMERA));
        assert!(!says(crate::key::CAMERA_READING));
    }

    /// The target of the row the cursor is on, and the line it is drawn on.
    fn at_the_cursor(s: &Settings, t: &ronoctua_i18n::Texts) -> (Target, usize) {
        let rows = s.view.rows(&s.snap, 120, t);
        let at = s.view.cursor();
        (rows[at].target.clone(), at)
    }

    /// The line the row with `target` is drawn on.
    fn line_of(s: &Settings, t: &ronoctua_i18n::Texts, target: &Target) -> usize {
        s.view
            .rows(&s.snap, 120, t)
            .iter()
            .position(|r| &r.target == target)
            .unwrap_or_else(|| panic!("no row for {target:?}"))
    }

    /// FAILS IF: the cameras arriving after the view opened leave a cursor
    /// nobody moved where it was put while they were being read — the first
    /// row that turned then, "Effect" — instead of where the view opens on
    /// them, the camera the app opens; or the page is not asked to follow
    /// it. With one webcam of fourteen controls "Effect" went to line 27 of
    /// a page of 15, and `RET` turned an effect nobody saw. And a cursor
    /// somebody moved stays on its row, the page after it.
    #[test]
    fn the_cameras_arriving_put_a_cursor_nobody_moved_where_the_view_opens() {
        use crate::camera_settings::{CameraWatch, planted_camera};
        let t = texts();
        let studio = a_studio();
        let mut w = way(&t).lending(Lent::of(vec![&studio]));
        let mut s = Settings::opened(&mut w);
        // THE RACE: the studio answers, the cameras are still being read.
        s.heard(&studio, &mut w);
        let (on, was) = at_the_cursor(&s, &t);
        assert_eq!(on, Target::Effect, "the race this catches is not set up");
        w.owed.clear();
        s.watch = Some(CameraWatch::planted(Some(vec![planted_camera()])));
        s.woken(&mut w);
        let (on, at) = at_the_cursor(&s, &t);
        assert_eq!(on, Target::DefaultCamera, "the cursor stayed on {on:?}");
        let page = 15;
        assert!(
            line_of(&s, &t, &Target::Effect) >= page,
            "the webcam's rows no longer push \"Effect\" off a page"
        );
        assert!(at < page, "the cursor is on line {at}, off the first page");
        assert!(
            w.owed.contains(&Effect::FollowIfSeen(was)),
            "the page was not asked to follow the cursor: {:?}",
            w.owed
        );
        // A CURSOR SOMEBODY MOVED, while the cameras were being read, stays
        // on its row, and the page follows it there.
        let mut s = Settings::opened(&mut w);
        s.heard(&studio, &mut w);
        assert_eq!(s.command("camera-settings-down", None, &mut w), Taken::Yes);
        let (on, was) = at_the_cursor(&s, &t);
        assert_eq!(on, Target::Background);
        w.owed.clear();
        s.watch = Some(CameraWatch::planted(Some(vec![planted_camera()])));
        s.woken(&mut w);
        let (now, at) = at_the_cursor(&s, &t);
        assert_eq!(now, Target::Background, "a cursor somebody moved was moved");
        assert_ne!(at, was);
        assert!(w.owed.contains(&Effect::FollowIfSeen(was)), "{:?}", w.owed);
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a title, the air between sections, a
    /// sentence — or leaves unlit one it takes. THE LIGHT IS THE CLICK: every
    /// row clicked once, on settings of their own, and lit exactly where
    /// something happened.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: 120,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: Lent::none(),
        };
        let held = crate::region::held_to_the_click(
            || shown_settings(&t),
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
