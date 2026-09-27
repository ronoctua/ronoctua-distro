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
//! places are three files under `$XDG_CONFIG_HOME`, watched by inotify — the
//! first two by every door (`door::hang_the_watches`), the places by this one
//! — so a choice made in the footer arrives here without anybody being told,
//! and what this window writes is ONE change laid over the file as it is then
//! (`locale::Errand`), never its whole copy back over the footer's.
//!
//! # And what is typed is typed HERE
//!
//! A zone, a place, where a place is, the machine's date, time and zone: each
//! is asked on the header's line (`Ui::ask`), under the command the footer's
//! minibuffer asks it under, pasted into with `C-v`, and handed to the same
//! `locale::took`. The two doors do the same things.

use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.locale";

/// Opens the language and region view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // THE GLANCE'S PLACES, watched: a place added or a switch flipped in
        // the footer is on this window's rows at once — and this window's
        // next change is laid over it instead of undoing it.
        let (places_sender, places_changed) = channel::channel::<()>();
        let places = ronoctua_store::config_dir()
            .and_then(|dir| watch_the_places(&dir, move || places_sender.send(()).is_ok()));
        w.handle
            .insert_source(
                places_changed,
                |event, (), door: &mut Door<LocaleWindow>| {
                    if let channel::Event::Msg(()) = event {
                        door.content.view.reread_files();
                        door.ui.ask_paint();
                    }
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the places channel: {e}"))?;
        Ok(LocaleWindow {
            view: crate::locale::View::fresh(),
            zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
            asking: None,
            drawn: 0,
            _places: places,
        })
    })
}

/// The watch on the glance's places, in the directory the window found.
///
/// The directory is made here and FOUND by the caller: a function that
/// writes does not also decide where (`persistence-takes-its-root-as-a-parameter`).
fn watch_the_places(
    dir: &std::path::Path,
    stirred: impl Fn() -> bool + Send + 'static,
) -> Option<ronoctua_files::notice::Notice> {
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(error = %e, "no configuration directory to watch the places in");
        return None;
    }
    ronoctua_files::notice::Notice::on_file(
        dir,
        ronoctua_world::prefs::FILE,
        ronoctua_files::notice::Pace::AtOnce,
        stirred,
    )
    .ok()
}

/// The window.
struct LocaleWindow {
    view: crate::locale::View,
    /// The machine's zone, read once — a TZif file is a table of
    /// transitions and the answer changes twice a year.
    zone: ronoctua_files::when::Zone,
    /// The question on this window's line, while it asks — what the answer
    /// is for, the place it is about included.
    asking: Option<crate::locale::Asking>,
    /// WHICH DRAWN ROW the cursor is on, as of the last key, click or answer
    /// — what the door scrolls to (`Content::cursor`).
    ///
    /// Not `View::cursor`, which counts the rows one can STAND on: between
    /// them sit the titles, the blank rows and the notes, so the page followed
    /// a row above the one lit, and walking down the list lit rows below the
    /// bottom of the window. The footer scrolls to `View::drawn_row`, and so
    /// does this.
    drawn: usize,
    _places: Option<ronoctua_files::notice::Notice>,
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
            // ONE SETTING, laid over what the file says now — the footer's
            // choices in the same file stay as the footer wrote them.
            Errand::Keep(setting) => {
                let choice = crate::locale::prefs::with(setting);
                if let Err(e) = crate::locale::prefs::keep(&choice) {
                    tracing::warn!(error = %e, "the date preferences could not be written");
                    ui.message = Some(ui.texts.t(ronoctua_i18n::key::LOCALE_NOT_WRITTEN));
                }
                crate::locale::prefs::set(choice);
                self.view.reread_files();
            }
            // THE QUESTION IS ASKED ON THIS WINDOW'S OWN LINE, under the
            // command the footer's minibuffer asks it under; the answer comes
            // back to `answered`.
            Errand::Ask(asking) => {
                ui.ask(crate::line::Line::new(asking.command(), false, None));
                self.asking = Some(asking);
            }
            Errand::Say(key) => ui.message = Some(ui.texts.t(key)),
            Errand::Glance(change) => {
                if let Err(key) = crate::locale::glance_change(&change) {
                    ui.message = Some(ui.texts.t(key));
                }
                self.view.reread_files();
            }
            // THE MACHINE'S CLOCK. The bus is on this side of the view for
            // the reason every other errand is — a view drawn by two doors
            // holds neither a connection nor a socket.
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

    /// Where the cursor is drawn, taken after it may have moved.
    fn took_the_cursor(&mut self, ui: &Ui) {
        self.drawn = self.view.drawn_row(&self.view_rows(ui));
    }
}

impl Content for LocaleWindow {
    type Hand = ();

    /// A LANGUAGE CHOSEN ELSEWHERE re-reads the view — the same move a choice
    /// made here makes.
    fn language_changed(&mut self, ui: &mut Ui) {
        let _ = ui;
        self.view.reread_files();
    }

    /// THE DATES' SHAPE CHOSEN ELSEWHERE — in the footer, above all — is on
    /// this window's rows at once, and this window's next ring is turned from
    /// it rather than from what it read when it opened.
    fn locale_changed(&mut self, ui: &mut Ui) {
        let _ = ui;
        self.view.reread_files();
    }

    /// WHAT WAS TYPED ON THIS WINDOW'S LINE, into the call the footer's
    /// minibuffer answers into: `locale::took`.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        // THE QUESTION THIS WINDOW ASKED, with the place it is about; an id
        // that names its question alone is taken by its name.
        let asking = self
            .asking
            .take()
            .filter(|a| a.command() == id)
            .or_else(|| crate::locale::Asking::named(id));
        let Some(asking) = asking else {
            return;
        };
        match crate::locale::took(&asking, text) {
            Ok(true) => {
                crate::locale::prefs::adopt();
                self.view.reread();
            }
            Ok(false) => {}
            Err(key) => ui.message = Some(ui.texts.t(key)),
        }
        self.took_the_cursor(ui);
        ui.ask_paint();
    }

    /// The question let go — and what it was about with it.
    fn unanswered(&mut self, ui: &mut Ui, id: &'static str) {
        let _ = ui;
        if self.asking.as_ref().is_some_and(|a| a.command() == id) {
            self.asking = None;
        }
    }

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::LOCALE_TITLE))
    }

    /// A key, read by its whole notation through the reader the footer
    /// calls too (`locale::press_of_key`) — `z`, `a` and `c` included.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::locale::press_of_key)
        else {
            return;
        };
        ui.message = None;
        let errand = self.view.press(press, &self.zone, &ui.texts);
        self.apply(ui, errand);
        self.took_the_cursor(ui);
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
        Some(self.drawn)
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
        self.took_the_cursor(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::{Asking, Shown, Target};
    use ronoctua_keymap::Key;

    /// A window over a planted machine whose clock is kept BY HAND, so the
    /// machine's date, time and zone rows act — and nothing of this machine's
    /// own is read or written.
    fn a_window() -> (LocaleWindow, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let me = LocaleWindow {
            view: crate::locale::View::planted(false),
            zone: ronoctua_files::when::Zone::utc(),
            asking: None,
            drawn: 0,
            _places: None,
        };
        (me, Ui::for_tests(texts))
    }

    /// One key, as the keyboard — or a click on the hint — sends it.
    fn key(w: &mut LocaleWindow, ui: &mut Ui, notation: &str) {
        let (event, mods) =
            crate::door::keys::event_of(Key::parse(notation).expect("a key of the notation"));
        w.typed(ui, &event, mods);
    }

    /// Puts the cursor on the row that is `target`, by one click.
    fn stand_on(w: &mut LocaleWindow, ui: &mut Ui, target: &Target) -> bool {
        let rows = w.view_rows(ui);
        let Some(at) = rows.iter().position(|r| r.target == *target) else {
            return false;
        };
        w.clicked(ui, at);
        assert!(ui.asking.is_none(), "the first click acted");
        true
    }

    /// Answers the line that is open, as the door does: the line leaves
    /// `ui.asking` first, and the window hears the text under its id.
    fn answer(w: &mut LocaleWindow, ui: &mut Ui, text: &str) {
        let line = ui.asking.take().expect("no line is open");
        w.answered(ui, line.id, text);
    }

    /// EVERY QUESTION OF THE VIEW IS ASKED ON THIS WINDOW'S OWN LINE — the
    /// interface's zone and a new place by their letters, where a place is by
    /// `c` on its rows, and the machine's date, time and zone by their rows —
    /// under the command the footer's minibuffer asks it under, and none is
    /// sent to the footer or answered with a sentence.
    ///
    /// THE DEFECT THIS CATCHES: every one of them printed "typing happens in
    /// the footer", and `z`, `a` and `c`, taught on this window's hint line,
    /// did nothing at all.
    #[test]
    fn every_question_is_asked_on_the_windows_own_line() {
        let asked = |w: &LocaleWindow, ui: &Ui, want: &Asking| {
            let line = ui.asking.as_ref().expect("the question is not asked here");
            assert_eq!(line.id, want.command());
            assert!(!line.secret);
            assert_eq!(w.asking.as_ref(), Some(want));
            assert!(ui.message.is_none(), "{:?}", ui.message);
            assert!(ui.orders().is_empty(), "the question went to the footer");
        };

        let (mut w, mut ui) = a_window();
        key(&mut w, &mut ui, "z");
        asked(&w, &ui, &Asking::Zone);

        let (mut w, mut ui) = a_window();
        key(&mut w, &mut ui, "a");
        asked(&w, &ui, &Asking::Place);

        let (mut w, mut ui) = a_window();
        let zone = w.view.place_here();
        assert!(zone.is_none(), "the cursor was born on a place");
        assert!(stand_on(&mut w, &mut ui, &Target::Shows(1, Shown::Date)));
        let zone = w.view.place_here().expect("the cursor is on a place");
        key(&mut w, &mut ui, "c");
        asked(&w, &ui, &Asking::Where(zone));

        // `c` AWAY FROM ANY PLACE says why, and asks nothing.
        let (mut w, mut ui) = a_window();
        key(&mut w, &mut ui, "c");
        assert!(ui.asking.is_none());
        assert_eq!(
            ui.message.as_deref(),
            Some(
                ui.texts
                    .t(ronoctua_i18n::key::LOCALE_NO_PLACE_HERE)
                    .as_str()
            )
        );

        // THE MACHINE'S THREE, by a second click on their rows — the zone
        // only where the flake does not pin it.
        let mut machine = vec![
            (Target::MachineDate, Asking::MachineDate),
            (Target::MachineTime, Asking::MachineTime),
        ];
        if !crate::locale::zone_is_declared() {
            machine.push((Target::MachineZone, Asking::MachineZone));
        }
        for (target, want) in machine {
            let (mut w, mut ui) = a_window();
            assert!(stand_on(&mut w, &mut ui, &target), "no row is {target:?}");
            let rows = w.view_rows(&ui);
            let at = rows.iter().position(|r| r.target == target).unwrap();
            w.clicked(&mut ui, at);
            asked(&w, &ui, &want);
        }
    }

    /// WHAT IS TYPED — OR PASTED — HERE IS CHECKED AS THE FOOTER CHECKS IT,
    /// by the same `locale::took`, and what is wrong with it is said on this
    /// window's own line. Nothing typed is never mind.
    #[test]
    fn an_answer_typed_here_is_checked_as_the_footers_is() {
        use ronoctua_i18n::key;
        let said = |ui: &Ui| ui.message.clone();

        let (mut w, mut ui) = a_window();
        key(&mut w, &mut ui, "z");
        answer(&mut w, &mut ui, "No/Such_Zone_At_All");
        assert_eq!(said(&ui), Some(ui.texts.t(key::LOCALE_NO_SUCH_ZONE)));
        assert!(w.asking.is_none(), "the question outlived its answer");

        // A COPIED COORDINATE, with its line break, that is not one.
        let (mut w, mut ui) = a_window();
        assert!(stand_on(&mut w, &mut ui, &Target::DropPlace(0)));
        key(&mut w, &mut ui, "c");
        let line = ui.asking.as_mut().expect("the coordinate is asked here");
        let _ = line.paste("north of here\n");
        assert_eq!(line.text, "north of here");
        let text = line.text.clone();
        answer(&mut w, &mut ui, &text);
        assert_eq!(said(&ui), Some(ui.texts.t(key::LOCALE_NOT_A_COORDINATE)));

        let (mut w, mut ui) = a_window();
        let rows = w.view_rows(&ui);
        let at = rows
            .iter()
            .position(|r| r.target == Target::MachineDate)
            .expect("the date's row acts on a clock kept by hand");
        w.clicked(&mut ui, at);
        w.clicked(&mut ui, at);
        answer(&mut w, &mut ui, "yesterday");
        assert_eq!(said(&ui), Some(ui.texts.t(key::LOCALE_NOT_A_DATE)));

        // NOTHING TYPED writes nothing and says nothing.
        let (mut w, mut ui) = a_window();
        key(&mut w, &mut ui, "a");
        answer(&mut w, &mut ui, "   ");
        assert!(ui.message.is_none(), "{:?}", ui.message);
        assert!(ui.orders().is_empty());
    }

    /// A QUESTION LET GO HERE takes what it was about with it: a later answer
    /// under `locale-where`, which needs a place, is taken for none.
    #[test]
    fn a_question_let_go_is_not_answered_later() {
        let (mut w, mut ui) = a_window();
        assert!(stand_on(&mut w, &mut ui, &Target::Shows(0, Shown::Clock)));
        key(&mut w, &mut ui, "c");
        let line = ui.asking.take().expect("asked");
        w.unanswered(&mut ui, line.id);
        assert!(w.asking.is_none());
        w.answered(&mut ui, "locale-where", "north");
        assert!(ui.message.is_none(), "an answer nobody asked for was taken");
    }

    /// THE WINDOW READS THE VIEW'S KEYS AS THE FOOTER DOES — `S-<arrows>`
    /// turn a ring, a plain arrow does not, and a chord is not its letter.
    #[test]
    fn the_windows_keyboard_reaches_the_same_presses() {
        let (mut w, mut ui) = a_window();
        key(&mut w, &mut ui, "C-z");
        assert!(ui.asking.is_none(), "`C-z` was taken for `z`");
        key(&mut w, &mut ui, "C-a");
        assert!(ui.asking.is_none(), "`C-a` was taken for `a`");
        key(&mut w, &mut ui, "z");
        assert!(ui.asking.is_some(), "`z` asked nothing");
    }

    /// THE PAGE FOLLOWS THE ROW THAT IS LIT, as the footer's does.
    ///
    /// THE DEFECT THIS CATCHES: the window told the door the cursor was at
    /// `View::cursor` — an index over the rows one can stand on — and the door
    /// scrolled to that DRAWN row, which is a title or a note above the lit
    /// one. Walking down, the lit row left the bottom of the window.
    #[test]
    fn the_page_follows_the_row_that_is_lit() {
        let (mut w, mut ui) = a_window();
        let last = w.view_rows(&ui).iter().filter(|r| !r.heading).count();
        for _ in 0..last {
            key(&mut w, &mut ui, "<down>");
            let rows = w.view_rows(&ui);
            let at = w.cursor().expect("a cursor");
            assert!(
                rows[at].focused,
                "the door is told row {at}, and the lit row is {:?}",
                rows.iter().position(|r| r.focused)
            );
        }
        assert!(
            w.cursor() > Some(w.view.cursor()),
            "no title or note was passed"
        );
    }
}
