//! The regions' view SECOND door — a window of its own.
//!
//! # Why there are two, and why they are one implementation
//!
//! The footer's expanded region has a ceiling in lines (`space.expansion`,
//! invariant 14), and this view lists what is installed AND a catalogue of
//! five hundred and thirty regions. Choosing one in the footer works; reading
//! down the list does not. So this exists, and everything it KNOWS lives in
//! [`crate::geo::View`], which the footer draws from too. What this module
//! holds is a window, a GPU and the events of both; what it decides is
//! nothing. It is the arrangement the sibling windows share.
//!
//! # Where its data comes from
//!
//! From a DIRECTORY and from the baked world: what is installed is
//! `$XDG_DATA_HOME/ronoctua/atlas/`, and the catalogue is a section of
//! `world.atlas`. There is no daemon here and no bus — and the one thing this
//! door cannot do is ask for the filter, because a window has no minibuffer.
//! It says where to type it instead, which is what the network and
//! notification windows already do.

use crate::door::{Content, Ui};
use anyhow::Result;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym, Modifiers};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.geo";

/// How wide this window counts itself before the compositor says.
///
/// Not a layout number: nothing is drawn at it. It is what the view settles
/// its cursor against in the instant between being built and the first
/// `configure`, and a width is needed for that because a note WRAPS — see
/// `geo::View::press`. The first configure replaces it.
const FIRST_COLUMNS: u16 = 80;

/// Opens the language and region view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|_| {
        Ok(GeoWindow {
            // AT THE WIDTH THIS WINDOW STARTS AT: the view settles its cursor
            // on the first row that acts, and which row that is follows the
            // width. The compositor's first configure replaces it.
            view: crate::geo::View::fresh(FIRST_COLUMNS),
        })
    })
}

/// The window.
struct GeoWindow {
    view: crate::geo::View,
}

impl GeoWindow {
    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: crate::geo::Errand) {
        use crate::geo::Errand;
        match errand {
            Errand::Nothing => {}
            Errand::Close => {
                ui.quit = true;
                return;
            }
            Errand::Speak(key) => ui.message = Some(ui.texts.t(key)),
            // A WINDOW HAS NO MINIBUFFER, so it says where to go instead of
            // opening a prompt of its own. It is the arrangement the network
            // window and the notification window already have, and the reason
            // is the same: two places to type is two things that can disagree
            // about what was typed.
            Errand::Ask => {
                let keys = format!("{} {}", crate::leader(), crate::leader::GEO_KEYS);
                ui.message = Some(
                    ui.texts
                        .t_with(ronoctua_i18n::key::GEO_IN_THE_FOOTER, &[("keys", &keys)]),
                );
            }
            // THE QUEUE, from here as well.
            //
            // This one is NOT the footer's business the way typing is: the
            // queue is a daemon with a socket, reachable from any process, and
            // sending a person to another door to press `RET` on a row they
            // are already standing on would be chrome asking for chrome. What
            // a window cannot do is take dictation; installing is one key.
            Errand::Install { region, label, url } => {
                let Some(into) =
                    ronoctua_xdg::data_dir().map(|d| ronoctua_atlas_packs::root_of(&d))
                else {
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::GEO_NO_HOME));
                    ui.ask_paint();
                    return;
                };
                let work = ronoctua_transfers::Work::Atlas {
                    region,
                    label,
                    url,
                    into,
                };
                ui.message = Some(
                    match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
                        Ok(()) => ui.texts.t(ronoctua_i18n::key::GEO_QUEUED),
                        Err(e) => {
                            tracing::warn!(error = %e, "the queue did not take the region");
                            ui.texts.t(ronoctua_i18n::key::TRANSFERS_OFFLINE)
                        }
                    },
                );
                self.view.reread(ui.columns);
            }
        }
        ui.ask_paint();
    }

    /// What one key means — from the keyboard, or `RET` from a second click
    /// on the row the cursor is already on (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::geo::Press) {
        ui.message = None;
        let errand = self.view.press(press, ui.columns, &ui.texts);
        self.apply(ui, errand);
    }
}

impl Content for GeoWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::GEO_TITLE))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, _: Modifiers) {
        if let Some(press) = press_of(event) {
            self.pressed(ui, press);
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view
            .rows(ui.columns, &ui.texts)
            .into_iter()
            .map(|r| r.fields)
            .collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19: a click
    /// puts the cursor on the row, and a click on the row it is already on
    /// is that row's `RET`, through the same `pressed` a key goes through.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view.rows(ui.columns, &ui.texts);
        if self.view.clicked(row, &rows) {
            self.pressed(ui, crate::geo::Press::Enter);
        }
    }

    /// A LANGUAGE CHOSEN ELSEWHERE re-reads the view, as it always did.
    fn language_changed(&mut self, ui: &mut Ui) {
        self.view.reread(ui.columns);
    }
}

/// A `KeyEvent` as the view sees it — the same mapping the footer does.
fn press_of(event: &KeyEvent) -> Option<crate::geo::Press> {
    use crate::geo::Press;
    Some(match event.keysym {
        Keysym::Escape => Press::Escape,
        Keysym::Return | Keysym::KP_Enter => Press::Enter,
        Keysym::Up => Press::Up,
        Keysym::Down => Press::Down,
        // THE TWO BARE LETTERS, and they have to be the same two the footer
        // reads or the doors drive one view differently. `f` asks for the
        // filter — which this door answers with a sentence, because a window
        // has no minibuffer — and `d` removes.
        Keysym::f => Press::Find,
        Keysym::d => Press::Remove,
        _ => return None,
    })
}
