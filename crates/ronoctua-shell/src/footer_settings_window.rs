//! The footer's settings in a window of its own — the second door onto
//! `crate::footer_settings` (`LEADER s r w`).
//!
//! A window and nothing else: which rows exist and what a key does to them is
//! `footer_settings.rs`, and both doors ask the same view the same questions.
//! What a row changes, this window writes into `modeline.json` itself, the
//! same change the footer's door makes (`footer_settings::change_the_file`),
//! and a file that will not parse or will not be written is SAID HERE, on
//! this window's line — the door where the key was pressed (`AGENTS.md`: what
//! a door says, a refusal, is said in it). The footer hears the write the way
//! every door hears a preference, the file, and moves its bars from it; this
//! window hears the footer's changes the same way.

use crate::adapt::Prefs;
use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.footer-settings";

/// The width the cursor is first settled at, before the door has one: the
/// row it lands on is the first that acts at every width.
const OPENING_COLUMNS: u16 = 80;

/// Opens the footer's settings in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // `modeline.json`, watched: a change made in the footer — a tap of
        // `Ctrl`, its own view, another window — moves this one without it
        // being reopened.
        let (sender, changed) = channel::channel::<()>();
        let watch = ronoctua_store::config_dir().and_then(|dir| watched(&dir, sender));
        w.handle
            .insert_source(
                changed,
                |event, (), door: &mut Door<FooterSettingsWindow>| {
                    if let channel::Event::Msg(()) = event {
                        door.content.reread(&mut door.ui);
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the modeline.json watch: {e}"))?;
        Ok(FooterSettingsWindow::watching(crate::adapt::store(), watch))
    })
}

/// A watch on `modeline.json` in `dir` — the directory the application found
/// and HANDS in (`persistence-takes-its-root-as-a-parameter`): made first,
/// because a watch on a folder that does not exist never fires.
fn watched(
    dir: &std::path::Path,
    sender: channel::Sender<()>,
) -> Option<ronoctua_files::notice::Notice> {
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(error = %e, "no configuration directory to watch modeline.json in");
        return None;
    }
    ronoctua_files::notice::Notice::on_file(
        dir,
        crate::adapt::FILE,
        ronoctua_files::notice::Pace::Coalesced,
        move || sender.send(()).is_ok(),
    )
    .ok()
}

/// The window: the view, the preferences it draws and the file they are in.
pub struct FooterSettingsWindow {
    view: crate::footer_settings::View,
    /// `modeline.json` as it was last read — kept when the file stops
    /// parsing, and the window says so.
    prefs: Prefs,
    /// Where it reads and writes them: the person's `modeline.json`, or the
    /// photograph's copy — a photograph that pressed `RET` on this machine's
    /// file would change the bars of the session it runs in.
    store: ronoctua_store::Store<Prefs>,
    _watch: Option<ronoctua_files::notice::Notice>,
}

impl FooterSettingsWindow {
    /// The window over the preferences in `store`, with nothing watched —
    /// what the photograph draws, over a copy of a file it was given.
    #[must_use]
    pub fn over(store: ronoctua_store::Store<Prefs>) -> Self {
        Self::watching(store, None)
    }

    /// The same, holding the watch on the file that keeps it current.
    fn watching(
        store: ronoctua_store::Store<Prefs>,
        watch: Option<ronoctua_files::notice::Notice>,
    ) -> Self {
        let prefs = store.parsed().unwrap_or_default();
        // THE CURSOR ON THE FIRST ROW THAT ACTS, which is where it is at any
        // width — a title first, then its setting. The door's own width
        // settles it again on the first key (`View::press`).
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT);
        let view = texts.as_ref().map_or_else(Default::default, |t| {
            crate::footer_settings::View::fresh(&prefs, OPENING_COLUMNS, t)
        });
        Self {
            view,
            prefs,
            store,
            _watch: watch,
        }
    }

    /// The file changed: read again with `parsed`, never `read` — a file that
    /// will not parse keeps the copy held, and the window says so.
    fn reread(&mut self, ui: &mut Ui) {
        match crate::adapt::reread(&self.store, &self.prefs) {
            crate::adapt::Reread::Same => return,
            crate::adapt::Reread::Refused => {
                ui.say_for(
                    ui.texts.t(ronoctua_i18n::key::ECHO_MODELINE_WILL_NOT_PARSE),
                    crate::echo::Echo::Long,
                );
            }
            crate::adapt::Reread::Taken(fresh) => self.prefs = fresh,
        }
        let rows = self.view_rows(ui);
        self.view.settle_on(&rows);
        ui.ask_paint();
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::footer_settings::Press) {
        use crate::footer_settings::Errand;
        ui.message = None;
        match self.view.press(press, &self.prefs, ui.columns, &ui.texts) {
            Errand::Nothing => {}
            Errand::Close => ui.quit = true,
            Errand::Change(setting) => self.change(ui, setting),
        }
        ui.ask_paint();
    }

    /// One setting changed in the file, and what came of it SAID on this
    /// line when it was not written: the row keeps what the file says, so a
    /// value that did not change beside a sentence saying why is the truth on
    /// the screen. A refusal is the LONG class (`echo::Echo::Long`), as on
    /// the footer: it is read to the end, and acted on — and the next key
    /// takes it off sooner.
    fn change(&mut self, ui: &mut Ui, setting: crate::footer_settings::Setting) {
        use crate::footer_settings::Changed;
        match crate::footer_settings::change_the_file(&self.store, setting) {
            // THIS WINDOW'S OWN WRITE: the copy is the file now, so the
            // notice it raises here finds nothing new (`adapt::reread`).
            Changed::Written(now) => self.prefs = now,
            Changed::Unparsed => {
                let said = ui.texts.t(ronoctua_i18n::key::ECHO_MODELINE_WILL_NOT_PARSE);
                ui.say_for(said, crate::echo::Echo::Long);
            }
            Changed::NotWritten(why) => {
                tracing::warn!(error = %why, "the footer's settings could not be written");
                let said = ui.texts.t_with(
                    ronoctua_i18n::key::ECHO_SETTING_NOT_WRITTEN,
                    &[("file", crate::adapt::FILE), ("why", &why.to_string())],
                );
                ui.say_for(said, crate::echo::Echo::Long);
            }
        }
        let rows = self.view_rows(ui);
        self.view.settle_on(&rows);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::footer_settings::Row> {
        self.view.rows(&self.prefs, ui.columns, &ui.texts)
    }
}

impl Content for FooterSettingsWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::FOOTER_SETTINGS_TITLE))
    }

    /// A key, read by its whole notation through the reader the footer calls
    /// too (`footer_settings::press_of_key`).
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::footer_settings::press_of_key)
        {
            self.pressed(ui, press);
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        if self.view.clicked(row, &rows) {
            self.pressed(ui, crate::footer_settings::Press::Enter);
        } else {
            ui.ask_paint();
        }
    }
}
