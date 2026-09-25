//! The language and region view's SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a ceiling in lines (`space.expansion`,
//! invariant 14), and this view has five sections — the languages, the clock
//! and the date, the glance's places with four switches each, the two hosts,
//! and what the system decides. A glance at the language fits the footer;
//! reading all of it does not. So this exists, and everything it KNOWS lives
//! in [`crate::locale::View`], which the footer draws from too. What this
//! module holds is a window, a GPU and the events of both; what it decides is
//! nothing. It is the arrangement the four sibling windows share.
//!
//! # Where its data comes from
//!
//! From FILES, and that is the whole difference from its siblings: there is no
//! daemon here and no bus. The language, the date's shape and the glance's
//! places are three files under `$XDG_CONFIG_HOME`, watched by inotify — so
//! this window has no thread of its own beyond the watches, and a choice made
//! in the footer arrives here without anybody being told.

use crate::door::{Content, Ui};
use anyhow::Result;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym, Modifiers};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.locale";

/// Opens the language and region view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|_| {
        Ok(LocaleWindow {
            view: crate::locale::View::fresh(),
            zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
        })
    })
}

/// The window.
struct LocaleWindow {
    view: crate::locale::View,
    /// The machine's zone, read once — a TZif file is a table of
    /// transitions and the answer changes twice a year.
    zone: ronoctua_files::when::Zone,
}

impl LocaleWindow {
    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: crate::locale::Errand) {
        use crate::locale::Errand;
        match errand {
            Errand::Nothing => {}
            Errand::Close => {
                ui.quit = true;
                return;
            }
            // THE LANGUAGE IS WRITTEN, and every door hears about it — this
            // one included, through its own watch. No socket and no message:
            // the file IS the broadcast, which is why a window can change the
            // language of the footer at all.
            Errand::Speak(id) => {
                if let Err(e) = crate::language::persist(id) {
                    tracing::warn!(error = %e, language = id, "the language could not be written");
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::LOCALE_NOT_WRITTEN));
                } else {
                    // ADOPTED HERE TOO, so the frame after this keystroke is
                    // already in the new language rather than waiting for the
                    // watch to come round.
                    self.adopt_the_language(ui);
                }
            }
            Errand::Keep(choice) => {
                if let Err(e) = crate::locale::prefs::keep(&choice) {
                    tracing::warn!(error = %e, "the date preferences could not be written");
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::LOCALE_NOT_WRITTEN));
                }
                crate::locale::prefs::set(choice);
                self.view.reread();
            }
            // A WINDOW HAS NO MINIBUFFER, so it says where to go instead of
            // opening a prompt of its own. It is the arrangement the network
            // window and the notification window already have, and the reason
            // is the same: two places to type is two things that can disagree
            // about what was typed.
            Errand::Ask(_) => {
                let keys = format!("{} {}", crate::leader(), crate::leader::LOCALE_KEYS);
                ui.message = Some(
                    ui.texts
                        .t_with(ronoctua_i18n::key::LOCALE_IN_THE_FOOTER, &[("keys", &keys)]),
                );
            }
            Errand::Glance(prefs) => {
                if let Err(e) = crate::locale::glance_keep(&prefs) {
                    tracing::warn!(error = %e, "the glance's places could not be written");
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::LOCALE_NOT_WRITTEN));
                }
                self.view.reread();
            }
            // THE MACHINE'S CLOCK. Unlike the questions above, these need no
            // minibuffer: they are a switch, and a switch is a thing a window
            // can flip. The bus is on this side of the view for the reason
            // every other errand is — a view drawn by two doors holds neither
            // a connection nor a socket.
            Errand::Automatic(on) => {
                if let Err(e) = ronoctua_clock::set_automatic(on) {
                    tracing::warn!(error = %e, "the machine's clock-keeping could not be changed");
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::LOCALE_CLOCK_REFUSED));
                }
                self.view.reread();
            }
            Errand::Watch(on) => {
                if !crate::locale::watch_the_clock(on) {
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::LOCALE_WATCHED_UNKNOWN));
                }
                self.view.reread();
            }
        }
        ui.ask_paint();
    }

    /// Re-reads the language and repaints in it.
    ///
    /// The words are swapped in the door's own `ui` AT ONCE, so the frame after
    /// the key is already in the new language. The renderer's locale follows a
    /// moment later: the choice was written to `language.json`, and the door's
    /// watch of that file is what tells the shaper (`Door::adopt_the_language`).
    fn adopt_the_language(&mut self, ui: &mut Ui) {
        let (id, _) = crate::language::resolve();
        let Some(texts) = ronoctua_i18n::Texts::fresh(&id) else {
            tracing::error!(language = id, "a declared language did not load");
            return;
        };
        ui.texts = std::rc::Rc::new(texts);
        self.view.reread();
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::locale::Row> {
        self.view.rows(ui.columns, &self.zone, &ui.texts)
    }
}

impl Content for LocaleWindow {
    type Hand = ();

    /// A LANGUAGE CHOSEN ELSEWHERE re-reads the view — the same move a choice
    /// made here makes.
    fn language_changed(&mut self, ui: &mut Ui) {
        let _ = ui;
        self.view.reread();
    }

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::LOCALE_TITLE))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(press) = press_of(event, mods.shift) else {
            return;
        };
        ui.message = None;
        let errand = self.view.press(press, &self.zone, &ui.texts);
        self.apply(ui, errand);
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

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19. This door
    /// only ever aimed: every setting on this screen was unreachable with a
    /// mouse, which is the rule's plainest failure.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        if self.view.clicked(row, &rows) {
            let errand = self
                .view
                .press(crate::locale::Press::Enter, &self.zone, &ui.texts);
            self.apply(ui, errand);
        }
    }
}

/// A `KeyEvent` as the view sees it — the same mapping the footer does.
fn press_of(event: &KeyEvent, shift: bool) -> Option<crate::locale::Press> {
    use crate::locale::Press;
    Some(match event.keysym {
        Keysym::Escape => Press::Escape,
        Keysym::Return | Keysym::KP_Enter => Press::Enter,
        Keysym::Up => Press::Up,
        Keysym::Down => Press::Down,
        // `S-<arrows>` TURN THE RING — the same rule as the footer's door,
        // and it has to be the same or the two doors drive the view
        // differently.
        Keysym::Left if shift => Press::Less,
        Keysym::Right if shift => Press::More,
        _ => return None,
    })
}
