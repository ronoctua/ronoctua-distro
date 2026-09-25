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

use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Keysym, Modifiers},
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
            _cameras: cameras,
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
    /// The camera watch, which stops when this is dropped.
    _cameras: ronoctua_camera::watch::Handle,
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
            // THIS DOOR SENDS IT through the footer's control socket. A command
            // that asks a question opens it in the FOOTER's minibuffer, and this
            // window says so: letters typed here would go nowhere.
            Err(crate::camera_settings::Errand::Run(id)) => {
                if let Err(e) = crate::leader::send_it(&format!("command {id}")) {
                    tracing::warn!(id, error = %e, "the footer did not take the command");
                } else if id != "camera-open" {
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::CAMERA_TYPED_ELSEWHERE));
                }
            }
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

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, _: Modifiers) {
        if let Some(press) = press_of(event) {
            self.pressed(ui, press);
        }
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

/// A `KeyEvent` as the view sees it — the same mapping the footer does.
///
/// BOTH DOORS GO THROUGH `camera_settings::OF_THE_VIEW`. This one turns SCTK's keysym
/// into the table's notation and asks the view; the footer turns its own
/// `Key` into the same notation and asks the same question. A key written in
/// only one of the two would be a key one door answers and the other does
/// not, which is the defect `bluetooth-keys-reach-both-doors` was written for.
fn press_of(event: &KeyEvent) -> Option<crate::camera_settings::Press> {
    let notation = match event.keysym {
        // ESCAPE IS NOT IN THE TABLE, and deliberately: every view of this
        // shape leaves one level on it, and the hint line does not teach a
        // key that is the same everywhere.
        Keysym::Escape => return Some(crate::camera_settings::Press::Escape),
        Keysym::Return | Keysym::KP_Enter => "RET",
        Keysym::Up => "<up>",
        Keysym::Down => "<down>",
        Keysym::Left => "<left>",
        Keysym::Right => "<right>",
        // AND THE BARE LETTERS, by the character SCTK hands this door. The
        // footer matches `Key::simple(sym).notation()`, which is what ITS
        // toolkit hands it; the two expressions agree on exactly the
        // unmodified lowercase letters the table holds, which is why the
        // table is the only place a letter is written.
        _ => event.utf8.as_deref()?,
    };
    crate::camera_settings::pressed(notation)
}
