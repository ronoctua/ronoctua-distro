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
//! `world.atlas`. There is no daemon here and no bus.
//!
//! # And the filter is typed HERE
//!
//! On the header's line (`Ui::ask`), as the footer types it on its minibuffer:
//! the same question (`geo::FIND`), pasted into with `C-v`, and answered into
//! the same `View::took_a_filter`. The two doors do the same things — a
//! window that sent the person to the footer to narrow a list it was showing
//! was a door with a hole in it.

use crate::door::{Content, Ui};
use anyhow::Result;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Modifiers};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.geo";

/// How wide this window counts itself before the compositor says.
///
/// Not a layout number: nothing is drawn at it. It is what the view settles
/// its cursor against in the instant between being built and the first
/// `configure`, and a width is needed for that because a note WRAPS — see
/// `geo::View::press`. The first configure replaces it.
const FIRST_COLUMNS: u16 = 80;

/// Opens the regions' view in a window.
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
            Errand::Speak(key) => ui.say(ui.texts.t(key)),
            // THE FILTER IS ASKED ON THIS WINDOW'S OWN LINE, the question the
            // footer's minibuffer asks: every key is the line's until it is
            // answered (`answered`) or let go, `C-v` pastes into it, and `f`
            // and `d` are letters of the filter while it asks.
            Errand::Ask => ui.ask(crate::line::Line::new(crate::geo::FIND, false, None)),
            // THE QUEUE, from here as well: it is a daemon with a socket,
            // reachable from any process, so this door puts the region in it
            // the way the footer does.
            Errand::Install { region, label, url } => {
                let Some(into) =
                    ronoctua_xdg::data_dir().map(|d| ronoctua_atlas_packs::root_of(&d))
                else {
                    ui.say(ui.texts.t(ronoctua_i18n::key::GEO_NO_HOME));
                    ui.ask_paint();
                    return;
                };
                let work = ronoctua_transfers::Work::Atlas {
                    region,
                    label,
                    url,
                    into,
                };
                ui.say(
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

    /// A key, read by its whole notation through the reader the footer
    /// calls too (`geo::press_of_key`).
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::geo::press_of_key)
        {
            self.pressed(ui, press);
        }
    }

    /// THE FILTER, typed on this window's line — the footer's minibuffer
    /// answers into the same call.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        if id == crate::geo::FIND {
            self.view.took_a_filter(text, ui.columns);
            ui.ask_paint();
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

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_keymap::Key;

    /// A window over a scratch root with nothing installed in it — never the
    /// person's own `$XDG_DATA_HOME` (`geo::tests`' rule).
    fn a_window(name: &str) -> (GeoWindow, Ui, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-geo-window-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let me = GeoWindow {
            view: crate::geo::View::at(dir.clone(), FIRST_COLUMNS),
        };
        (me, Ui::for_tests(texts), dir)
    }

    /// One key, as the keyboard sends it.
    fn key(w: &mut GeoWindow, ui: &mut Ui, notation: &str) {
        let (event, mods) =
            crate::door::keys::event_of(Key::parse(notation).expect("a key of the notation"));
        w.typed(ui, &event, mods);
    }

    /// Answers the line that is open, as the door does: the line leaves
    /// `ui.asking` first, and the window hears the text under its id.
    fn answer(w: &mut GeoWindow, ui: &mut Ui, text: &str) {
        let line = ui.asking.take().expect("no line is open");
        w.answered(ui, line.id, text);
    }

    /// THE FILTER IS ASKED ON THIS WINDOW'S OWN LINE — by `f`, as the footer
    /// asks it on its minibuffer — and no sentence sends the person to the
    /// footer, and nothing is handed to it.
    ///
    /// THE DEFECT THIS CATCHES: `f` printed "typing happens in the footer",
    /// and the list this window exists to read could not be narrowed here.
    #[test]
    fn the_filter_is_asked_on_the_windows_own_line() {
        let (mut w, mut ui, dir) = a_window("asks");
        key(&mut w, &mut ui, "f");
        let line = ui.asking.as_ref().expect("the filter is not asked here");
        assert_eq!(line.id, crate::geo::FIND);
        assert!(!line.secret, "a filter is read back, not hidden");
        assert!(ui.message.is_none(), "{:?}", ui.message);
        assert!(ui.orders().is_empty(), "the question went to the footer");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// WHAT IS TYPED — OR PASTED — HERE NARROWS THE LIST, exactly as the same
    /// answer in the footer's minibuffer does: both reach
    /// `View::took_a_filter`, and nothing typed shows everything again.
    #[test]
    fn what_is_typed_in_the_window_narrows_the_list_as_the_footer_does() {
        let (mut w, mut ui, dir) = a_window("narrows");
        let (mut footer, _, other) = a_window("narrows-footer");
        let all = w.view.shown().len();

        key(&mut w, &mut ui, "f");
        // A COPIED NAME, with its line break: the line takes it as one line.
        let line = ui.asking.as_mut().expect("the filter is asked here");
        let _ = line.paste("zzzz-no-region-is-called-this\n");
        let text = line.text.clone();
        answer(&mut w, &mut ui, &text);
        footer.view.took_a_filter(&text, FIRST_COLUMNS);

        assert_eq!(w.view.filter(), "zzzz-no-region-is-called-this");
        assert_eq!(w.view.filter(), footer.view.filter());
        assert_eq!(w.view.shown().len(), footer.view.shown().len());
        assert_eq!(
            w.view.shown().len(),
            0,
            "a filter that matches nothing kept rows"
        );

        // NOTHING TYPED IS EVERYTHING AGAIN.
        key(&mut w, &mut ui, "f");
        answer(&mut w, &mut ui, "");
        assert_eq!(w.view.filter(), "");
        assert_eq!(w.view.shown().len(), all);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&other);
    }

    /// AN ANSWER UNDER ANOTHER ID IS NOT THE FILTER.
    #[test]
    fn an_answer_nobody_asked_for_changes_nothing() {
        let (mut w, mut ui, dir) = a_window("stray");
        w.answered(&mut ui, "geo-remove", "brazil");
        assert_eq!(w.view.filter(), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// THE TWO DOORS READ A KEY ONE WAY: every row of the table is its own
    /// press, `Esc` is the way back whatever is held, and a chord is not the
    /// letter under it — `C-f` asked for the filter in both doors, because
    /// each matched the symbol and dropped the modifier.
    #[test]
    fn the_window_reads_the_views_keys_as_the_footer_does() {
        use crate::geo::{Press, press_of_key};
        let read = |n: &str| press_of_key(Key::parse(n).expect("a key"));
        assert_eq!(read("<down>"), Some(Press::Down));
        assert_eq!(read("<up>"), Some(Press::Up));
        assert_eq!(read("RET"), Some(Press::Enter));
        assert_eq!(read("f"), Some(Press::Find));
        assert_eq!(read("d"), Some(Press::Remove));
        assert_eq!(read("ESC"), Some(Press::Escape));
        assert_eq!(read("S-ESC"), Some(Press::Escape));
        assert_eq!(read("C-f"), None);
        assert_eq!(read("M-d"), None);
        assert_eq!(read("x"), None);
        for (taught, _) in crate::geo::OF_THE_VIEW {
            assert!(
                read(taught).is_some(),
                "`{taught}` is taught and reads as nothing"
            );
        }
    }
}
