//! The camera's settings in a window of its own — the second door onto
//! `crate::camera_settings`.
//!
//! A window and nothing else: which rows exist and what a key does is
//! `camera_settings.rs`, and both doors ask the same view the same questions.
//! The plumbing every window shares is `crate::door`.
//!
//! Two threads feed it: the camera watch (`camera_settings::CameraWatch`,
//! inotify on `/dev`, raised as the window opens and gone with it — no camera
//! is opened before somebody opens the settings) and the studio's state
//! (`ronoctua_studio::client::watch`). Every order is carried out here and
//! the file it wrote is reread — the footer's door does the same, and the two
//! never keep a copy between orders.
//!
//! # And what is typed is typed HERE
//!
//! A scene's name, a script, the stream's address and its key are asked on
//! this window's own line (`Ui::ask`), with the rows still under it, and
//! answered into `camera_settings::answered` — the call the footer's
//! minibuffer answers into. The key is masked, and this process writes it to
//! its file: it never goes to the footer.

use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.camera-settings";

/// Opens the camera's settings in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // THE CAMERAS, by event (inotify on `/dev`), read from now on — the
        // window opening is the person asking — through the door's wake
        // (`woken`); and THE STUDIO's state.
        let cameras = crate::camera_settings::CameraWatch::raise(w.wake());
        let (studio_sender, studio_events) =
            channel::channel::<Option<ronoctua_studio::wire::State>>();
        let studio = ronoctua_studio::client::watch(move |s| {
            let _ = studio_sender.send(s);
        });
        w.handle
            .insert_source(
                studio_events,
                |event, (), door: &mut Door<CameraSettingsWindow>| {
                    if let channel::Event::Msg(s) = event {
                        door.content.studio = s;
                        door.content.reread(&mut door.ui);
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the studio channel: {e}"))?;

        // THE FILE PICKER'S FOLDER, watched through the door's wake.
        let mut view = crate::camera_settings::View::fresh();
        view.watch_with(w.wake());
        Ok(CameraSettingsWindow {
            view,
            snapshot: crate::camera_settings::Snapshot {
                reading: true,
                ..crate::camera_settings::Snapshot::default()
            },
            cameras: None,
            studio: None,
            watch: Some(cameras),
            _studio: studio,
        })
    })
}

/// The scenes of this window's camera, in the words `--scene` takes.
const SCENES: [&str; 3] = ["virtual", "reading", "arrived"];

/// The variable a photograph of the FOOTER names its camera scene with — one
/// of [`SCENES`], read only by a footer that is a photograph's
/// (`Doorway::photographing`): the cameras that scene's reading finds are
/// the region's.
pub const SCENE_VARIABLE: &str = "RONOCTUA_PHOTOGRAPH_CAMERA_SCENE";

/// The first reading of the cameras in scene `scene` — `None` for one that
/// never answers (`reading`).
fn cameras_of(scene: &str) -> Option<Vec<ronoctua_camera::Camera>> {
    match scene {
        "reading" => None,
        "arrived" => Some(vec![crate::camera_settings::planted_camera()]),
        _ => Some(Vec::new()),
    }
}

/// A photograph's footer's camera watch: planted with the cameras of the
/// scene [`SCENE_VARIABLE`] names — a machine with none when it names none,
/// the picture the baseline holds. A scene that is none of [`SCENES`] is
/// said in the log, and its reading never comes: the section says it is
/// reading, never "no camera", which would pass for one.
#[must_use]
pub fn planted_watch() -> crate::camera_settings::CameraWatch {
    let scene = std::env::var(SCENE_VARIABLE).unwrap_or_default();
    if !scene.is_empty() && !SCENES.contains(&scene.as_str()) {
        tracing::error!(%scene, scenes = %SCENES.join(", "), "the footer camera's camera scene is none of the scenes");
        return crate::camera_settings::CameraWatch::planted(None);
    }
    crate::camera_settings::CameraWatch::planted(cameras_of(&scene))
}

/// The window over a PLANTED studio, for the photograph
/// (its camera, `camera`): `virtual` is the virtual camera on, sending the
/// camera alone, through an effect of its own, on a machine with no camera;
/// `reading` is the same while the cameras are still being read — the
/// moment after the view opens, which a photograph, reading none, never
/// reaches by itself; `arrived` is the window opened with the studio
/// answering first and a webcam's reading coming after it
/// (`camera_settings::planted_camera`), taken at the door's first turn by
/// the handler a wake reaches.
///
/// # Errors
///
/// A scene this door does not know.
pub fn photographed(scene: &str) -> anyhow::Result<(impl Content, Option<crate::line::Line>)> {
    anyhow::ensure!(
        SCENES.contains(&scene),
        "`{scene}` is not a scene of the camera's settings; the scenes are {}",
        SCENES.join(", ")
    );
    let arrived = scene == "arrived";
    let studio = ronoctua_studio::wire::State {
        scenes: vec!["camera".into(), "screen".into()],
        scene: "camera".into(),
        virtual_node: (!arrived).then(|| "/dev/video9".into()),
        ..ronoctua_studio::wire::State::default()
    };
    let prefs = if arrived {
        ronoctua_studio::prefs::Preferences::default()
    } else {
        ronoctua_studio::prefs::Preferences {
            virtual_sends: ronoctua_studio::prefs::Sends::Camera,
            virtual_effect: ronoctua_studio::prefs::VirtualEffect::Own("background-blur".into()),
            ..ronoctua_studio::prefs::Preferences::default()
        }
    };
    // THE READING TO COME, for `arrived`; the others are drawn as they are.
    let (cameras, watch) = if arrived {
        (
            None,
            Some(crate::camera_settings::CameraWatch::planted(cameras_of(
                scene,
            ))),
        )
    } else {
        (cameras_of(scene), None)
    };
    Ok((
        CameraSettingsWindow {
            view: crate::camera_settings::View::fresh(),
            snapshot: crate::camera_settings::Snapshot {
                state: Some(studio.clone()),
                prefs,
                reading: cameras.is_none(),
                ..crate::camera_settings::Snapshot::default()
            },
            cameras,
            studio: Some(studio),
            watch,
            _studio: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        },
        None,
    ))
}

/// `ronoctua-shell photograph --door camera-settings --scene virtual`: this window
/// over a planted machine, walked and drawn (`crate::photo::planted`) — the
/// camera of the camera's settings, beside the door it photographs.
pub fn camera(rest: &[String]) -> ! {
    crate::photo::planted::taken(rest, "camera-settings", &SCENES.join(", "), |asked| {
        let (content, line) = photographed(&asked.scene)?;
        crate::photo::planted::shoot(asked, content, line)
    })
}

/// The window: the view, and what it reads.
struct CameraSettingsWindow {
    view: crate::camera_settings::View,
    /// What the rows read, rebuilt whenever a thread brings news.
    snapshot: crate::camera_settings::Snapshot,
    /// The cameras, as the watch last read them — `None` until its first
    /// reading.
    cameras: Option<Vec<ronoctua_camera::Camera>>,
    /// The studio's state, when it answers.
    studio: Option<ronoctua_studio::wire::State>,
    /// The camera watch, which stops when this is dropped — `None` in a
    /// photograph and a test, which watch nothing of the machine.
    watch: Option<crate::camera_settings::CameraWatch>,
    /// The studio watch's switch, kept for as long as the window lives.
    _studio: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl CameraSettingsWindow {
    /// Rebuilds what the rows read — the cursor kept on its row by
    /// identity, or put where the view opens when nobody moved it, and the
    /// list after it (`View::reread`, the footer's region's too).
    fn reread(&mut self, ui: &mut Ui) {
        let now = crate::camera_settings::Snapshot::read(self.cameras.clone(), self.studio.clone());
        self.view.reread(&self.snapshot, &now, ui);
        self.snapshot = now;
        ui.ask_paint();
    }

    /// The cameras the watch read, when a reading came: the rows read again
    /// over them. `true` when one did.
    fn take_the_reading(&mut self, ui: &mut Ui) -> bool {
        let Some(cameras) = self
            .watch
            .as_ref()
            .and_then(crate::camera_settings::CameraWatch::arrived)
        else {
            return false;
        };
        self.cameras = Some(cameras);
        let _ = self.view.look();
        self.reread(ui);
        true
    }

    /// Carries out what the view asked for — through the one handler the
    /// footer's region calls too (`camera_settings::carry_in`).
    fn apply(&mut self, ui: &mut Ui, errand: crate::camera_settings::Errand) {
        use crate::camera_settings::Carried;
        match crate::camera_settings::carry_in(errand, &self.snapshot, ui) {
            // NOTHING TO SAY clears the line, as a gesture always has here.
            Carried::Quiet => ui.message = None,
            Carried::Said | Carried::Handed => {}
            // A QUESTION ON THIS WINDOW'S OWN LINE, the clipboard read for
            // the file picker, or closed: nothing to read again now.
            Carried::Left => return,
        }
        self.reread(ui);
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::camera_settings::Press) {
        ui.message = None;
        self.view.page_is(ui.room_for_rows());
        let errand = self.view.key(press, &self.snapshot, ui.columns, &ui.texts);
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::camera_settings::Row> {
        self.view.rows(&self.snapshot, ui.columns, &ui.texts)
    }
}

impl Content for CameraSettingsWindow {
    /// A thread of the window's woke the loop — the camera watch read the
    /// cameras, or the file picker's folder moved or its devices were read:
    /// its rows again, when they changed.
    fn woken(&mut self, ui: &mut Ui) {
        if !self.take_the_reading(ui) && self.view.look() {
            ui.ask_paint();
        }
    }

    /// A PHOTOGRAPH HAS NO LOOP TO WAKE — its door is detached — so the
    /// reading planted for it is taken at its turn, by the handler a wake
    /// reaches on the screen. A window on the screen takes it in `woken`.
    fn each_turn(&mut self, ui: &mut Ui) {
        if crate::region::Doorway::photographing(ui) {
            let _ = self.take_the_reading(ui);
        }
    }
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::CAMERA_SETTINGS_TITLE))
    }

    /// A key, read by its whole notation through the reader the footer
    /// calls too (`camera_settings::press_of_key`): a letter held with `Ctrl` or `Alt`
    /// is not the letter, in either door.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        // THE FILE PICKER, while it is up over the view, takes what it reads
        // — every letter, and its own keys.
        if self.view.picker_takes(key) {
            self.pressed(ui, crate::camera_settings::Press::Pick(key));
            return;
        }
        if let Some(press) = crate::camera_settings::press_of_key(key) {
            self.pressed(ui, press);
        }
    }

    /// The clipboard's text, read off the loop for the file picker's `C-v`.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        self.view.picker_pasted(text);
        self.view.settle_in(&self.snapshot, ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// WHAT WAS TYPED on this window's line — the call the footer's
    /// minibuffer answers into, and the view stays where it was.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        let errand = crate::camera_settings::answered(id, text, self.studio.as_ref());
        self.apply(ui, errand);
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&self.snapshot, ui.columns, &ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    /// A click takes a row the cursor stands on, and only that row lights
    /// under the pointer — the view's one answer
    /// ([`crate::camera_settings::Row::takes`]), which the footer's region
    /// gives too.
    fn takes(&self, ui: &Ui) -> Vec<bool> {
        self.view_rows(ui)
            .iter()
            .map(crate::camera_settings::Row::takes)
            .collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19: a click
    /// puts the cursor on the row, and a click on the row it is already on is
    /// that row's `RET`, through the same `pressed` a key goes through.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        if self.view.clicked(row, &rows) {
            self.pressed(ui, crate::camera_settings::Press::Enter);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera_settings::{Question, Snapshot, Target, View};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")
    }

    /// A window with a studio that answered and no thread behind it: it
    /// watches nothing of the machine.
    fn a_window() -> CameraSettingsWindow {
        let studio = ronoctua_studio::wire::State {
            scenes: vec!["camera".into(), "screen".into()],
            scene: "camera".into(),
            ..ronoctua_studio::wire::State::default()
        };
        CameraSettingsWindow {
            view: View::fresh(),
            snapshot: Snapshot {
                state: Some(studio.clone()),
                ..Snapshot::default()
            },
            cameras: Some(Vec::new()),
            studio: Some(studio),
            watch: None,
            _studio: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Puts the cursor on the row whose target is `target`, as a click does.
    fn stand_on(w: &mut CameraSettingsWindow, ui: &Ui, target: &Target) {
        let rows = w.view_rows(ui);
        let at = rows
            .iter()
            .position(|r| &r.target == target)
            .unwrap_or_else(|| panic!("no row for {target:?}"));
        w.view.clicked(at, &rows);
    }

    /// EVERY QUESTION IS ASKED ON THIS WINDOW'S OWN LINE — the stream's key
    /// masked — and no sentence sends the person to the footer, and nothing
    /// is handed to it.
    ///
    /// THE DEFECT THIS CATCHES: the row ran the command in the footer and
    /// this window said "type it in the footer", so a scene could not be
    /// named, a script written or a stream addressed from here.
    #[test]
    fn every_question_is_asked_on_the_windows_own_line() {
        let mut w = a_window();
        let mut ui = Ui::for_tests(texts());
        for (target, id, secret) in [
            (Target::SceneAdd, crate::camera_settings::SCENE_ADD, false),
            (
                Target::SceneRename,
                crate::camera_settings::SCENE_RENAME,
                false,
            ),
            (Target::ScriptAdd, crate::camera_settings::SCRIPT_ADD, false),
            (Target::StreamUrl, crate::camera_settings::STREAM_URL, false),
            (Target::StreamKey, crate::camera_settings::STREAM_KEY, true),
        ] {
            ui.asking = None;
            stand_on(&mut w, &ui, &target);
            let before = w.view.cursor();
            w.pressed(&mut ui, crate::camera_settings::Press::Enter);
            let line = ui
                .asking
                .as_ref()
                .unwrap_or_else(|| panic!("{id} is not asked here"));
            assert_eq!(line.id, id);
            assert_eq!(line.secret, secret, "{id}");
            assert!(ui.message.is_none(), "{:?}", ui.message);
            assert!(!ui.quit, "asking closed the window");
            assert_eq!(
                w.view.cursor(),
                before,
                "asking moved the cursor off its row"
            );
            assert_eq!(
                crate::camera_settings::question(id),
                Some(Question { id, secret })
            );
        }
        assert!(
            ui.orders().is_empty(),
            "a question went to the footer: {:?}",
            ui.orders()
        );
    }

    /// THE KEY CROSSES NOTHING: typed here it becomes the errand this
    /// process carries out, and its `Debug` — a log line, a failing test —
    /// never prints it.
    #[test]
    fn the_stream_key_is_answered_here_and_never_printed() {
        let errand = crate::camera_settings::answered(
            crate::camera_settings::STREAM_KEY,
            " live_s3cr3t ",
            None,
        );
        assert!(matches!(
            errand,
            crate::camera_settings::Errand::StreamKey(_)
        ));
        let printed = format!("{errand:?}");
        assert!(
            !printed.contains("s3cr3t"),
            "the key reached Debug: {printed}"
        );
    }

    /// A LETTER HELD WITH `Ctrl` OR `Alt` IS NOT THE LETTER, in this door as
    /// in the footer: `C-d` removed a scene in the footer and did nothing here.
    #[test]
    fn a_chord_is_not_the_letter_under_it() {
        use crate::camera_settings::{Press, press_of_key};
        let read = |n: &str| press_of_key(ronoctua_keymap::Key::parse(n).expect("a key"));
        assert_eq!(read("d"), Some(Press::Remove));
        assert_eq!(read("C-d"), None);
        assert_eq!(read("M-d"), None);
        assert_eq!(read("ESC"), Some(Press::Escape));
        for (taught, id, _) in crate::camera_settings::OF_THE_VIEW {
            assert_eq!(
                read(taught),
                crate::camera_settings::press_of(id),
                "`{taught}` is taught and reads as something else"
            );
        }
        // AND THROUGH THIS WINDOW'S OWN READER: xkb's `C-d` is read as the
        // chord, and the chord is nothing.
        let (event, mods) =
            crate::door::keys::event_of(ronoctua_keymap::Key::parse("C-d").expect("C-d"));
        assert_eq!(
            crate::door::keys::key_of(&event, mods).and_then(press_of_key),
            None
        );
    }

    /// FAILS IF: the camera's planted virtual camera stops building, or the
    /// planted reading stops being photographed as one.
    #[test]
    fn the_planted_virtual_camera_builds() {
        assert!(photographed("virtual").is_ok());
        assert!(photographed("reading").is_ok());
        assert!(photographed("arrived").is_ok());
        assert!(photographed("no-such-scene").is_err());
    }

    /// The target of the row the cursor is on, and its line.
    fn at_the_cursor(w: &CameraSettingsWindow, ui: &Ui) -> (Target, usize) {
        let at = w.view.cursor();
        (w.view_rows(ui)[at].target.clone(), at)
    }

    /// FAILS IF: the window opened, the studio answering before the cameras
    /// are read, leaves a cursor nobody moved on the first row that turned
    /// then — "Effect", which the webcam's fourteen controls push below the
    /// list's first page — instead of where the view opens once they are
    /// read, the camera the app opens, on the first page. The footer's
    /// region runs the same race (`camera_region`'s test).
    #[test]
    fn the_cameras_arriving_after_the_studio_put_the_cursor_where_the_view_opens() {
        use crate::camera_settings::{CameraWatch, planted_camera};
        let mut ui = Ui::for_tests(texts());
        // AS `host_it` OPENS IT: nothing read yet.
        let mut w = CameraSettingsWindow {
            view: View::fresh(),
            snapshot: Snapshot {
                reading: true,
                ..Snapshot::default()
            },
            cameras: None,
            studio: None,
            watch: None,
            _studio: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        // THE STUDIO FIRST, through the channel's handler.
        w.studio = a_window().studio;
        w.reread(&mut ui);
        assert_eq!(
            at_the_cursor(&w, &ui).0,
            Target::Effect,
            "the race is not set up"
        );
        // THEN THE READING, through the wake's.
        w.watch = Some(CameraWatch::planted(Some(vec![planted_camera()])));
        w.woken(&mut ui);
        let (on, at) = at_the_cursor(&w, &ui);
        assert_eq!(on, Target::DefaultCamera, "the cursor stayed on {on:?}");
        let room = ui.room_for_rows();
        let effect = w
            .view_rows(&ui)
            .iter()
            .position(|r| r.target == Target::Effect)
            .expect("the effect's row");
        assert!(
            effect >= room,
            "the webcam no longer pushes \"Effect\" off the list"
        );
        assert!(
            at >= ui.offset && at < ui.offset + room,
            "the cursor is on line {at}, off the list at {} of {room}",
            ui.offset
        );
    }

    /// FAILS IF: the photographed `arrived` stops taking its planted reading
    /// at the door's turn — a photograph has no loop to wake — so the picture
    /// of where the cursor lands once the cameras are read is a picture of
    /// them being read.
    #[test]
    fn a_photograph_takes_its_planted_reading_at_its_turn() {
        let mut ui = Ui::for_tests(texts());
        let mut w = a_window();
        w.cameras = None;
        w.snapshot.reading = true;
        w.watch = Some(crate::camera_settings::CameraWatch::planted(Some(vec![
            crate::camera_settings::planted_camera(),
        ])));
        w.each_turn(&mut ui);
        assert!(!w.snapshot.reading, "the planted reading was not taken");
        assert_eq!(w.snapshot.cameras.len(), 1);
        assert_eq!(at_the_cursor(&w, &ui).0, Target::DefaultCamera);
    }

    /// FAILS IF: the window's resting pointer lights a row a click takes
    /// nothing on — a title, the air between sections, a sentence — or
    /// leaves unlit one it takes. THE LIGHT IS THE CLICK: every row clicked
    /// once, on a window of its own, and lit exactly where something
    /// happened. The footer's region has its own.
    #[test]
    fn the_windows_pointer_lights_what_the_click_takes() {
        let held = crate::door::held_to_the_click(
            || (a_window(), Ui::for_tests(texts())),
            crate::door::seen,
        );
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }
}
