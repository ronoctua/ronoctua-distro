//! The camera's settings in a window of its own — the second door onto
//! `crate::camera_settings`.
//!
//! A window and nothing else: which rows exist and what a key does is
//! `camera_settings.rs`, and both doors ask the same view the same questions.
//! The plumbing every window shares is `crate::door`.
//!
//! Two threads feed it: the camera watch (`ronoctua_camera::watch`, inotify on
//! `/dev`) and the studio's state (`ronoctua_studio::client::watch`). Every
//! order is carried out here and the file it wrote is reread — the footer's
//! door does the same, and the two never keep a copy between orders.
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
        // THE CAMERAS, by event (inotify on `/dev`), and THE STUDIO's state.
        let (camera_sender, camera_events) = channel::channel::<Vec<ronoctua_camera::Camera>>();
        let cameras = ronoctua_camera::watch::watch(move |c| {
            let _ = camera_sender.send(c);
        });
        w.handle
            .insert_source(
                camera_events,
                |event, (), door: &mut Door<CameraSettingsWindow>| {
                    if let channel::Event::Msg(c) = event {
                        door.content.cameras = c;
                        door.content.reread(&mut door.ui);
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the camera channel: {e}"))?;
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

        Ok(CameraSettingsWindow {
            view: crate::camera_settings::View::fresh(),
            snapshot: crate::camera_settings::Snapshot::default(),
            cameras: Vec::new(),
            studio: None,
            _cameras: Some(cameras),
            _studio: studio,
        })
    })
}

/// The window: the view, and what it reads.
struct CameraSettingsWindow {
    view: crate::camera_settings::View,
    /// What the rows read, rebuilt whenever a thread brings news.
    snapshot: crate::camera_settings::Snapshot,
    /// The cameras, as the watch last read them.
    cameras: Vec<ronoctua_camera::Camera>,
    /// The studio's state, when it answers.
    studio: Option<ronoctua_studio::wire::State>,
    /// The camera watch, which stops when this is dropped — `None` only in a
    /// test, which watches nothing of the machine.
    _cameras: Option<ronoctua_camera::watch::Handle>,
    /// The studio watch's switch, kept for as long as the window lives.
    _studio: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl CameraSettingsWindow {
    /// Rebuilds what the rows read, keeping the cursor on its row by identity.
    fn reread(&mut self, ui: &mut Ui) {
        let spot = self
            .view_rows(ui)
            .get(self.view.cursor())
            .map(|r| r.spot.clone());
        self.snapshot =
            crate::camera_settings::Snapshot::read(self.cameras.clone(), self.studio.clone());
        if let Some(spot) = spot {
            self.view
                .follow(&spot, &self.snapshot, ui.columns, &ui.texts);
        }
        self.view.settle_in(&self.snapshot, ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: crate::camera_settings::Errand) {
        match crate::camera_settings::carry_out(errand, &self.snapshot, &ui.texts) {
            Ok(said) => ui.message = said,
            Err(crate::camera_settings::Errand::Close) => {
                ui.quit = true;
                return;
            }
            // A QUESTION IS ASKED ON THIS WINDOW'S OWN LINE, the one the
            // footer's minibuffer asks: every key is the line's until it is
            // answered (`answered`) or let go, and `C-v` pastes into it.
            Err(crate::camera_settings::Errand::Ask(q)) => {
                ui.ask(crate::line::Line::new(q.id, q.secret, None));
                return;
            }
            // THE APP'S DOOR is the footer's to open, as the leader's `m c`
            // is: told off the loop, and a footer that is not there is said.
            Err(crate::camera_settings::Errand::Run(id)) => ui.tell_the_footer(id, None),
            Err(_) => {}
        }
        self.reread(ui);
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::camera_settings::Press) {
        ui.message = None;
        let errand = self.view.key(press, &self.snapshot, ui.columns, &ui.texts);
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::camera_settings::Row> {
        self.view.rows(&self.snapshot, ui.columns, &ui.texts)
    }
}

impl Content for CameraSettingsWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::CAMERA_SETTINGS_TITLE))
    }

    /// A key, read by its whole notation through the reader the footer
    /// calls too (`camera_settings::press_of_key`): a letter held with `Ctrl` or `Alt`
    /// is not the letter, in either door.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::camera_settings::press_of_key)
        {
            self.pressed(ui, press);
        }
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
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
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
            cameras: Vec::new(),
            studio: Some(studio),
            _cameras: None,
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
}
