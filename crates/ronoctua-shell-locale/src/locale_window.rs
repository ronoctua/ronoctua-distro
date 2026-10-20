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
        // THE BRAIN'S CLOCK WATCH, heard off the loop: what it is as the
        // window opens, and what each switch came to.
        let (heard_sender, heard) = channel::channel::<crate::locale::Heard>();
        w.handle
            .insert_source(heard, |event, (), door: &mut Door<LocaleWindow>| {
                if let channel::Event::Msg(heard) = event {
                    door.content.heard(&mut door.ui, heard);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the brain's channel: {e}"))?;
        let first = heard_sender.clone();
        let mut view = crate::locale::View::fresh();
        if !crate::locale::ask_the_brain(None, move |h| {
            let _ = first.send(h);
        }) {
            view.set_watched(None);
        }
        Ok(LocaleWindow {
            view,
            zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
            asking: None,
            drawn: 0,
            _places: places,
            brain: heard_sender,
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
/// The window over a PLANTED machine, for the photograph
/// (its camera, `camera`):
///
/// - `currency`: Lisbon's currencies chosen, a place with none — its rate's
///   switch asks for the pair;
/// - `zone`: the zone question open with `Lis` typed, and what it offers.
///
/// # Errors
///
/// A scene this door does not know.
pub fn photographed(scene: &str) -> anyhow::Result<(impl Content, Option<crate::line::Line>)> {
    let place =
        |zone: &str, currency: &str, rate_in: &str, rate: bool| ronoctua_world::prefs::Wanted {
            zone: zone.to_owned(),
            currency: currency.to_owned(),
            rate_in: rate_in.to_owned(),
            rate,
            ..ronoctua_world::prefs::Wanted::default()
        };
    let (view, asking) = match scene {
        "currency" => (
            crate::locale::View::planted_with(vec![
                place("Europe/Lisbon", "EUR", "BRL", true),
                place("Asia/Tokyo", "", "", false),
            ]),
            None,
        ),
        "zone" => (
            crate::locale::View::planted(false),
            Some(crate::line::Line::new("locale-zone", false, Some("Lis"))),
        ),
        // THE BRAIN'S CLOCK WATCH in each of its three readings: on, not
        // known while the door asks, and turned off by the system with the
        // sentence that says so.
        "watch-on" | "watch-asking" | "watch-system" => {
            let mut view = crate::locale::View::planted(false);
            view.set_watched(Some(match scene {
                "watch-on" => crate::locale::Watched::Set(true),
                "watch-asking" => crate::locale::Watched::Asking,
                _ => crate::locale::Watched::BySystem,
            }));
            (view, None)
        }
        other => anyhow::bail!(
            "`{other}` is not a scene of the locale window; the scenes are currency, zone, \
             watch-on, watch-asking and watch-system"
        ),
    };
    Ok((
        LocaleWindow {
            view,
            zone: ronoctua_files::when::Zone::utc(),
            asking: asking
                .as_ref()
                .and_then(|l| crate::locale::Asking::named(l.id)),
            drawn: 0,
            _places: None,
            brain: channel::channel().0,
        },
        asking,
    ))
}

/// `ronoctua-shell photograph --door locale --scene currency|zone|watch-on|watch-asking|watch-system`: this window
/// over a planted machine, walked and drawn (`crate::photo::planted`) — the
/// camera of the language and region, beside the door it photographs.
pub fn camera(rest: &[String]) -> ! {
    crate::photo::planted::taken(
        rest,
        "locale",
        "currency|zone|watch-on|watch-asking|watch-system",
        |asked| {
            let (content, line) = photographed(&asked.scene)?;
            crate::photo::planted::shoot(asked, content, line)
        },
    )
}

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
    /// Where what the brain says about the clock's watch comes back to.
    brain: channel::Sender<crate::locale::Heard>,
}

impl LocaleWindow {
    /// Carries out what the view asked for — through the one handler the
    /// footer's region carries it out with too (`locale_region::carry_out`).
    /// What the brain says about a switch of the watch comes back on this
    /// window's channel, to [`LocaleWindow::heard`].
    fn apply(&mut self, ui: &mut Ui, errand: crate::locale::Errand) {
        let brain = self.brain.clone();
        let back = move || -> Box<dyn FnOnce(crate::locale::Heard) + Send> {
            let brain = brain.clone();
            Box::new(move |h| {
                let _ = brain.send(h);
            })
        };
        crate::locale_region::carry_out(errand, &mut self.view, &mut self.asking, &back, ui);
    }

    /// What the brain said about the clock's watch — said and drawn as the
    /// footer's region says and draws it (`locale_region::heard_back`).
    fn heard(&mut self, ui: &mut Ui, heard: crate::locale::Heard) {
        crate::locale_region::heard_back(heard, &mut self.view, ui);
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
        crate::locale_region::told(&asking, text, Some(&mut self.view), ui);
        self.took_the_cursor(ui);
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
        format!("*{}*", texts.t(crate::key::LOCALE_TITLE))
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
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    /// A click takes a row the cursor stands on, and only that row lights
    /// under the pointer — the view's one answer ([`crate::locale::Row::takes`]),
    /// which the footer's region gives too.
    fn takes(&self, ui: &Ui) -> Vec<bool> {
        self.view_rows(ui)
            .iter()
            .map(crate::locale::Row::takes)
            .collect()
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
            brain: channel::channel().0,
        };
        (me, Ui::for_tests(texts))
    }

    /// What the brain said about the clock's watch is said HERE, and the row
    /// follows it.
    ///
    /// THE DEFECT THIS CATCHES: the switch asked the brain on this window's
    /// loop, twice; off the loop, an answer that arrives and is not said
    /// would be a switch that silently did nothing.
    #[test]
    fn what_the_brain_said_about_the_watch_is_said_here() {
        let (mut w, mut ui) = a_window();
        w.heard(
            &mut ui,
            crate::locale::Heard {
                answer: Some(crate::mind_wire::Answer::Refused(
                    crate::key::BRAIN_WHY_SYSTEM_OFF.to_owned(),
                )),
                watched: Some(crate::locale::Watched::BySystem),
            },
        );
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::BRAIN_WHY_SYSTEM_OFF).as_str())
        );
        assert!(
            !w.view.set_watched(Some(crate::locale::Watched::BySystem)),
            "the row did not follow what the brain said"
        );

        let (mut w, mut ui) = a_window();
        w.heard(
            &mut ui,
            crate::locale::Heard {
                answer: Some(crate::mind_wire::Answer::Unreachable),
                watched: None,
            },
        );
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::LOCALE_WATCHED_UNKNOWN).as_str())
        );

        // TAKEN, nothing is said: the row says it.
        let (mut w, mut ui) = a_window();
        w.heard(
            &mut ui,
            crate::locale::Heard {
                answer: Some(crate::mind_wire::Answer::Taken(None)),
                watched: Some(crate::locale::Watched::Set(false)),
            },
        );
        assert_eq!(ui.message, None);
        assert!(!w.view.set_watched(Some(crate::locale::Watched::Set(false))));
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
            Some(ui.texts.t(crate::key::LOCALE_NO_PLACE_HERE).as_str())
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
        use crate::key;
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

    /// FAILS IF: the window's resting pointer lights a row a click takes
    /// nothing on — a title, the air between sections, a note — or leaves
    /// unlit one it takes. THE LIGHT IS THE CLICK: every row clicked, on a
    /// window of its own, and lit exactly where something was seen to
    /// happen. The footer's region has its own.
    ///
    /// CLICKED ONCE, AND NEVER ON THE CURSOR'S ROW — as
    /// `door::held_to_the_click` clicks too: a second click would be the `RET`, and a
    /// `RET` here writes this machine's language, dates and places and turns
    /// its clock-keeping (`locale_region::carry_out`), which no test may do.
    /// One click is enough for the light: from any other row it selects, and
    /// the cursor moving is seen; on the cursor's row it is already the
    /// `RET`, and that row is lit by being the cursor's (`Held::here`). The
    /// window is opened with its drawn cursor TAKEN, as a key or a click
    /// takes it, so the row the view calls the cursor's is the one skipped.
    #[test]
    fn the_windows_pointer_lights_what_the_click_takes() {
        let fresh = || {
            let (mut w, ui) = a_window();
            w.took_the_cursor(&ui);
            (w, ui)
        };
        // THE ROWS' TONES AND PLACES, and not their words: a row of this view
        // says the time to the minute, and a minute turning between the two
        // readings is no click's doing. A click that selects moves a tone.
        let tinted = |w: &LocaleWindow, ui: &Ui| {
            let (rows, cursor, deeds) = crate::door::seen(w, ui);
            let tones: Vec<Vec<(u16, ronoctua_tokens::Color)>> = rows
                .iter()
                .map(|f| f.iter().map(|x| (x.at, x.color)).collect())
                .collect();
            (tones, cursor, deeds)
        };
        let (first, ui) = fresh();
        let lit = first.takes(&ui);
        let cursor = first.cursor();
        let held: Vec<crate::region::Held> = (0..first.rows(&ui).len())
            .map(|row| {
                let here = cursor == Some(row);
                let (mut w, mut ui) = fresh();
                let before = tinted(&w, &ui);
                if !here {
                    w.clicked(&mut ui, row);
                }
                crate::region::Held {
                    row,
                    lit: lit.get(row).copied().unwrap_or(false),
                    did: tinted(&w, &ui) != before,
                    here,
                }
            })
            .collect();
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        assert!(
            held.iter().all(|h| !h.here || h.lit),
            "the cursor stands on a row a click does not take: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }

    /// FAILS IF: one of the camera's planted scenes stops building.
    #[test]
    fn every_planted_scene_builds() {
        for scene in [
            "currency",
            "zone",
            "watch-on",
            "watch-asking",
            "watch-system",
        ] {
            assert!(photographed(scene).is_ok(), "{scene}");
        }
    }
}
