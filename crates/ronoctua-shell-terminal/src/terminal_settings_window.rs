//! The terminal's settings, in a window of their own (`RET s w`) — the
//! second door onto [`crate::terminal_settings`].
//!
//! A window and nothing else: the rows and what a key does are
//! `terminal_settings.rs`, and the errand handler is the region's
//! ([`crate::terminal_settings_region::carry_out`]). `terminal.json` is
//! watched like the theme: a value turned in the footer moves this window
//! without it being reopened, and what is drawn is a fresh read of the file
//! — with what of it did not read said under the values, as the region says
//! it (`terminal_settings::read`).

use crate::door::{Content, Door, Ui};
use crate::terminal_settings::{Press, Settings, View};
use anyhow::Result;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.terminal-settings";

/// Opens the terminal's settings in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        let dir = crate::terminal_settings::dir();
        let (moved, heard) = channel::channel::<()>();
        let watch = dir.as_deref().and_then(|dir| {
            crate::terminal_settings::watch_in(
                dir,
                ronoctua_files::notice::Pace::AtOnce,
                move || moved.send(()).is_ok(),
            )
        });
        w.handle
            .insert_source(heard, |event, (), door: &mut Door<SettingsWindow>| {
                if let channel::Event::Msg(()) = event {
                    let (settings, unread) = crate::terminal_settings::read(&door.content.store);
                    door.content.settings = settings;
                    door.content.view.read_in_part(unread);
                    door.ui.ask_paint();
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the terminal.json watch: {e}"))?;
        let store = crate::terminal_settings::store_in(dir);
        let (settings, unread) = crate::terminal_settings::read(&store);
        // A SETTING'S ROW IS WHERE IT IS AT EVERY WIDTH (`View::rows`): the
        // cursor born here is on the first, whatever width comes.
        let words = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT)
            .ok_or_else(|| anyhow::anyhow!("the base language has no words"))?;
        let mut view = View::fresh(&settings, tokens::space::TERMINAL_COLUMNS, &words);
        view.read_in_part(unread);
        Ok(SettingsWindow {
            view,
            settings,
            store,
            _watch: watch,
        })
    })
}

/// The window: the view, and the file it draws.
pub struct SettingsWindow {
    view: View,
    /// The file as it was read last.
    settings: Settings,
    store: ronoctua_store::Store<Settings>,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl SettingsWindow {
    /// What one key means — from the keyboard, or `RET` from a second click.
    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        let errand = self
            .view
            .press(press, &self.settings, ui.columns, &ui.texts);
        if let Some(now) = crate::terminal_settings_region::carry_out(errand, &self.store, ui) {
            self.settings = now;
            // JUDGED AGAIN, as the region judges it after its own write.
            self.view
                .read_in_part(crate::terminal_settings::judged(&self.store));
        }
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::terminal_settings::Row> {
        self.view.rows(&self.settings, ui.columns, &ui.texts)
    }
}

impl Content for SettingsWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::TERMINAL_SETTINGS_TITLE))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::terminal_settings::press_of_key)
        {
            self.pressed(ui, press);
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&ui.texts)
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        let rows = self.view_rows(ui);
        rows.into_iter().map(|r| r.fields).collect()
    }

    /// A click takes a setting, and only a setting lights under the pointer
    /// — the view's one answer, which the footer's region gives too.
    fn takes(&self, ui: &Ui) -> Vec<bool> {
        self.view_rows(ui)
            .iter()
            .map(crate::terminal_settings::Row::takes)
            .collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        if self.view.clicked(row, &rows) {
            self.pressed(ui, Press::Enter);
        }
    }
}

/// THE TERMINAL SETTINGS' CAMERA: `photograph --door terminal-settings
/// [--settings <terminal.json>]` — the window over a planted file, read as
/// the person's is; nothing of the machine is read, and a key that changes
/// a setting writes a copy.
pub mod photograph {
    use anyhow::{Context as _, Result};
    use ronoctua_shell_door::photo::scenes::{self, Asked, Scene};

    use super::SettingsWindow;
    use crate::terminal_settings::View;

    /// The camera's own words.
    #[derive(Debug, Default)]
    pub struct Planted {
        file: Option<std::path::PathBuf>,
    }

    impl Scene for Planted {
        const DOOR: &'static str = "terminal-settings";
        const IS: &'static str = "the terminal's settings over a planted terminal.json";
        const WORDS: &'static str = "\
  --settings <file>     terminal.json as the person keeps it, copied first —
                        the copy is what a key changes  (default: the defaults)";

        fn word(&mut self, flag: &str, value: &mut dyn FnMut() -> Result<String>) -> Result<bool> {
            if flag != "--settings" {
                return Ok(false);
            }
            self.file = Some(value()?.into());
            Ok(true)
        }

        fn shoot(self, ui: crate::door::Ui, asked: &Asked) -> Result<()> {
            // A ROOM OF ITS OWN: the planted file copied, so a key writes
            // the copy and never the file given, nor this machine's.
            let room = std::env::temp_dir().join(format!(
                "ronoctua-terminal-settings-camera-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&room).context("the camera's room")?;
            if let Some(file) = &self.file {
                std::fs::copy(file, room.join(crate::terminal_settings::FILE))
                    .with_context(|| format!("copy {}", file.display()))?;
            }
            let store =
                crate::terminal_settings::store_in(Some(room.clone())).with_roots(None, None);
            let (settings, unread) = crate::terminal_settings::read(&store);
            let mut view = View::fresh(&settings, ui.columns, &ui.texts);
            view.read_in_part(unread);
            let window = SettingsWindow {
                view,
                settings,
                store,
                _watch: None,
            };
            let shot = scenes::walked(crate::door::Detached::new(ui, window), asked);
            let _ = std::fs::remove_dir_all(&room);
            shot
        }
    }

    /// `ronoctua-shell photograph --door terminal-settings …`, to its end.
    pub fn taken(rest: &[String]) -> ! {
        scenes::taken(rest, Planted::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_window() -> (SettingsWindow, Ui) {
        let ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        let store = crate::terminal_settings::store_in(None);
        let settings = Settings::default();
        let w = SettingsWindow {
            view: View::fresh(&settings, ui.columns, &ui.texts),
            settings,
            store,
            _watch: None,
        };
        (w, ui)
    }

    /// FAILS IF: the window draws a file read only in part as if it said
    /// what is shown — the region says it, and the two doors draw one file —
    /// or its own write is not judged again: the sentence gone while the
    /// place that did not read is still the file's, or kept once the person
    /// turned that place.
    #[test]
    fn the_window_says_what_of_the_file_did_not_read() {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-terminal-settings-window-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(crate::terminal_settings::FILE),
            "{\"copy-on-select\": \"sideways\"}",
        )
        .unwrap();
        let mut ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        let store = crate::terminal_settings::store_in(Some(dir.clone()));
        let (settings, unread) = crate::terminal_settings::read(&store);
        let mut view = View::fresh(&settings, ui.columns, &ui.texts);
        view.read_in_part(unread);
        let mut w = SettingsWindow {
            view,
            settings,
            store,
            _watch: None,
        };
        let says = |w: &SettingsWindow, ui: &Ui| {
            Content::rows(w, ui)
                .iter()
                .flatten()
                .any(|f| f.text.contains("/copy-on-select"))
        };
        assert!(says(&w, &ui), "{:?}", Content::rows(&w, &ui));
        w.pressed(&mut ui, Press::Enter);
        assert_eq!(w.settings.scrollback_bytes, 50_000_000, "nothing written");
        assert!(
            says(&w, &ui),
            "the window's own write took the sentence away"
        );
        // THE PLACE ITSELF TURNED (`Copy` is the fourth setting): the file
        // is the person's there now, and it reads whole.
        for press in [Press::Down, Press::Down, Press::Down, Press::Enter] {
            w.pressed(&mut ui, press);
        }
        assert!(
            !says(&w, &ui),
            "a file the window's own write mended is still said not to read"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the window's resting pointer lights a row a click takes
    /// nothing on — a title, a note, the air — or leaves unlit a setting.
    #[test]
    fn the_windows_pointer_lights_what_the_click_takes() {
        let held = crate::door::held_to_the_click(a_window, crate::door::seen);
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }
}
