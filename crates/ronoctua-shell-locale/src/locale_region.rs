//! THE LANGUAGE AND REGION VIEW IN THE FOOTER'S REGION (`LEADER s l l`) —
//! and the one errand handler both its doors carry out ([`carry_out`]).
//!
//! # Why the errands are here, and handled once
//!
//! What the view asks — speak a language, keep a setting, change the glance's
//! places, set the machine's clock, switch the brain's clock watch, ask
//! something on the door's line — was written twice: the footer's
//! `locale_errand` and the window's `apply`, each over its own door, and they
//! had drifted. A refusal to write was said for longer in the window than in
//! the footer; the brain's answer about the watch was said in the page's own
//! words in the window and in the brain's generic ones in the footer. It is
//! written once now, over the door's [`Doorway`]: the window's `Ui`, or the
//! footer's recorder.
//!
//! # Why the language is not applied by one door alone
//!
//! Because it has to reach ten processes, and the control socket only goes
//! INTO the footer. So the answer is the theme's answer: WRITE THE FILE, which
//! every door watches. The door that wrote it takes it at once
//! ([`Doorway::written`]) so the frame after the keystroke is in the new
//! language; every other door takes it when its watch rings.
//!
//! # What the footer still holds, and hands in
//!
//! The mind it streams (the brain's clock watch, read out of it rather than
//! asked of a socket on the loop), the zone it writes dates in, and the
//! glance's places when they change — heard ([`crate::region::Region::heard`])
//! as the footer hears them.

use crate::echo::Echo;
use crate::line::Line;
use crate::locale::{Asking, Errand, Heard, Press, View};
use crate::region::{Closed, Doorway, Entry, Seen, Taken, Written};

/// The id the footer ranks the view under (`surface::PRECEDENCE`).
pub const ID: &str = "locale";

/// The view as the binary registers it with the footer's slot host.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// Where what the brain says about the clock's watch is sent back to, in the
/// door that asked: called once per switch, it gives the one-shot the
/// brain's thread hands its answer to.
pub type Back<'a> = &'a dyn Fn() -> Box<dyn FnOnce(Heard) + Send>;

/// Carries out what the view asked for, in whichever door it is drawn — the
/// footer's region or the window ([`crate::locale_window`]). `asking` is what
/// the door's line was asked for, kept until it is answered or let go.
pub fn carry_out(
    errand: Errand,
    view: &mut View,
    asking: &mut Option<Asking>,
    back: Back<'_>,
    way: &mut dyn Doorway,
) {
    match errand {
        Errand::Nothing => {}
        Errand::Close => {
            way.close();
            return;
        }
        // THE LANGUAGE IS WRITTEN, and every door hears about it — this one
        // at once (`Written::Language`), the others through their watches. A
        // file that could not be written is a refusal to read to the end.
        Errand::Speak(id) => match crate::language::persist(id) {
            Err(e) => {
                tracing::warn!(error = %e, language = id, "the language could not be written");
                let said = way.texts().t(crate::key::LOCALE_NOT_WRITTEN);
                way.say(said, Echo::Long);
            }
            Ok(()) => {
                way.written(Written::Language);
                view.reread();
            }
        },
        // ONE SETTING, laid over what the file says now: the other door's
        // choices in the same file stay as it wrote them. ADOPTED HERE and not
        // left to a watch, so the frame that follows already writes dates the
        // new way.
        Errand::Keep(setting) => {
            let choice = crate::when::prefs::with(setting);
            if let Err(e) = crate::when::prefs::keep(&choice) {
                tracing::warn!(error = %e, "the date preferences could not be written");
                let said = way.texts().t(crate::key::LOCALE_NOT_WRITTEN);
                way.say(said, Echo::Long);
            }
            crate::when::prefs::set(choice);
            view.reread_files();
        }
        // ONE CHANGE TO THE PLACES, laid over the file the same way — and the
        // glance takes it now, so the next `LEADER g` draws what was chosen.
        Errand::Glance(change) => {
            if let Err(key) = crate::locale::glance_change(&change) {
                let said = way.texts().t(key);
                way.say(said, Echo::Normal);
            }
            view.reread_files();
            way.written(Written::Places);
        }
        Errand::Say(key) => {
            let said = way.texts().t(key);
            way.say(said, Echo::Normal);
        }
        // THE MACHINE'S OWN CLOCK. The bus and the polkit rule are on this
        // side of the view, which is drawn by two doors and holds neither.
        Errand::Automatic(on) => {
            if let Err(e) = ronoctua_clock::set_automatic(on) {
                tracing::warn!(error = %e, "the machine's clock-keeping could not be changed");
                let said = way.texts().t(crate::key::LOCALE_CLOCK_REFUSED);
                way.say(said, Echo::Normal);
            }
            view.reread();
        }
        // THE BRAIN'S CLOCK WATCH, OFF THE LOOP: the switch and the look after
        // it are two asks on the brain's socket, and the answer comes back to
        // the door ([`heard_back`]). A photograph's door reaches nothing of
        // the machine, and says so as a door with no brain does.
        Errand::Watch(on) => {
            if way.photographing() || !crate::locale::ask_the_brain(Some(on), back()) {
                let said = way.texts().t(crate::key::LOCALE_WATCHED_UNKNOWN);
                way.say(said, Echo::Normal);
            }
        }
        // SOMETHING HAS TO BE TYPED: the door asks on its own line, under the
        // command whose `ask-` key is the question, and the answer comes back
        // to [`told`] in either door.
        Errand::Ask(question) => {
            way.ask(Line::new(question.command(), false, None));
            *asking = Some(question);
        }
    }
    way.repaint();
}

/// What was typed for question `asking`, checked before it is kept
/// (`locale::took`) — a zone that cannot be read would draw a clock in UTC
/// under that name. Written answers are taken at once: the dates' shape, and
/// the places the glance draws.
pub fn told(asking: &Asking, said: &str, view: Option<&mut View>, way: &mut dyn Doorway) {
    match crate::locale::took(asking, said) {
        Ok(true) => {
            crate::when::prefs::adopt();
            way.written(Written::Places);
            if let Some(v) = view {
                v.reread();
            }
        }
        Ok(false) => {}
        Err(key) => {
            let said = way.texts().t(key);
            way.say(said, Echo::Normal);
        }
    }
    way.repaint();
}

/// What the brain said about the clock's watch: its answer to a switch, said
/// on the door's line — nothing when it took it, its refusal in its own
/// sentence, this page's sentence when it is not there — and the watch as it
/// now stands.
pub fn heard_back(heard: Heard, view: &mut View, way: &mut dyn Doorway) {
    match heard.answer {
        Some(crate::mind_wire::Answer::Unreachable) => {
            let said = way.texts().t(crate::key::LOCALE_WATCHED_UNKNOWN);
            way.say(said, Echo::Normal);
        }
        Some(answer) => {
            if let Some(said) = answer.sentence(way.texts()) {
                way.say(said, Echo::Normal);
            }
        }
        None => {}
    }
    view.set_watched(heard.watched);
    way.repaint();
}

/// The place a question about one is for: the one the open question is about
/// — asked by its row or by its switch — else the one under the cursor. Never
/// "place zero", which wrote a coordinate onto a place nobody was looking at.
fn about_a_place(
    asking: &mut Option<Asking>,
    view: &View,
    fits: fn(&Asking) -> bool,
    of_the_cursor: fn(String) -> Asking,
) -> Option<Asking> {
    match asking.take() {
        Some(a) if fits(&a) => Some(a),
        _ => view.place_here().map(of_the_cursor),
    }
}

/// What a command of the view does while it is CLOSED: the questions it asks
/// by name are asked even so, and answered — a zone, a place, the machine's
/// date, time and zone are the person's whether a list is drawn or not; a
/// question about a place has no place under no cursor, and says so; the
/// window opens; and only `locale-open` opens the region.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "locale-open" => Closed::Opens,
        "locale-window" => {
            way.open_window(vec!["locale".to_owned()]);
            Closed::Taken
        }
        "locale-where" | "locale-currency" => {
            let said = way.texts().t(crate::key::LOCALE_NO_PLACE_HERE);
            way.say(said, Echo::Normal);
            way.repaint();
            Closed::Taken
        }
        _ => {
            let Some(asking) = Asking::named(command) else {
                return Closed::NotMine;
            };
            match argument {
                None => {
                    way.ask(Line::new(asking.command(), false, None));
                    way.repaint();
                }
                Some(said) => told(&asking, said, None, way),
            }
            Closed::Taken
        }
    }
}

/// Opens the view — the footer asked whether the region is free first. The
/// brain's watch is not known until the footer hands in the mind it streams
/// (`heard`), and a footer with none says the brain is not there.
fn open(_: &str, _: Option<&str>, _: &mut dyn Doorway) -> Box<dyn crate::region::Region> {
    let mut view = View::fresh();
    view.set_watched(None);
    let (sent, heard) = std::sync::mpsc::channel();
    Box::new(LocaleRegion {
        view,
        asking: None,
        zone: ronoctua_files::when::Zone::utc(),
        drawn: 0,
        opened: false,
        sent,
        heard,
    })
}

/// The language and region view, as a region of the footer.
pub struct LocaleRegion {
    view: View,
    /// What the footer's line was asked for, while it asks.
    asking: Option<Asking>,
    /// The zone the footer writes dates in, as it handed it in.
    zone: ronoctua_files::when::Zone,
    /// WHICH DRAWN ROW the cursor is on, as of the last gesture — the view's
    /// cursor counts the rows one can stand on (`View::drawn_row`).
    drawn: usize,
    /// Whether the command that opened it has run: a second open is
    /// refused, the first one is the opening itself.
    opened: bool,
    /// Where the brain's answers about the watch are sent, off the loop...
    sent: std::sync::mpsc::Sender<Heard>,
    /// ...and read back, when the region's wake rings.
    heard: std::sync::mpsc::Receiver<Heard>,
}

impl LocaleRegion {
    /// The rows at the door's width, in the door's words.
    fn rows_of(&self, way: &dyn Doorway) -> Vec<crate::locale::Row> {
        self.view.rows(way.columns(), &self.zone, way.texts())
    }

    /// Where the cursor is drawn, taken after it may have moved.
    fn took_the_cursor(&mut self, way: &dyn Doorway) {
        self.drawn = self.view.drawn_row(&self.rows_of(way));
    }

    /// Carries out an errand of the view through the footer's door.
    fn carry(&mut self, errand: Errand, way: &mut dyn Doorway) {
        let sent = self.sent.clone();
        let wake = way.wake();
        let back = move || -> Box<dyn FnOnce(Heard) + Send> {
            let sent = sent.clone();
            let wake = wake.clone();
            Box::new(move |h| {
                let _ = sent.send(h);
                wake.call();
            })
        };
        carry_out(errand, &mut self.view, &mut self.asking, &back, way);
        self.took_the_cursor(way);
    }

    /// One press, from a key or a second click.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = self.view.press(press, &self.zone, way.texts());
        self.carry(errand, way);
    }

    /// A typed answer to `asking`, or — with nothing typed — the question
    /// asked: `M-x locale-zone` asks, as `z` does. The command runs once to
    /// ask and once to be told, which is `network-import`'s shape.
    fn told_or_asked(&mut self, argument: Option<&str>, asking: Asking, way: &mut dyn Doorway) {
        match argument {
            None => self.carry(Errand::Ask(asking), way),
            Some(said) => {
                self.asking = None;
                told(&asking, said, Some(&mut self.view), way);
                self.took_the_cursor(way);
            }
        }
    }

    /// A question about a place — where it is, its currencies — answered or
    /// asked for the place it is about.
    fn about_a_place(
        &mut self,
        argument: Option<&str>,
        fits: fn(&Asking) -> bool,
        of_the_cursor: fn(String) -> Asking,
        way: &mut dyn Doorway,
    ) {
        match about_a_place(&mut self.asking, &self.view, fits, of_the_cursor) {
            Some(asking) => self.told_or_asked(argument, asking, way),
            None => {
                let said = way.texts().t(crate::key::LOCALE_NO_PLACE_HERE);
                way.say(said, Echo::Normal);
                way.repaint();
            }
        }
    }
}

impl crate::region::Region for LocaleRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// THE LANGUAGE AND REGION ROWS, as `fields`. A heading is not a row to
    /// click on, and neither is a note: the view's one answer
    /// ([`crate::locale::Row::takes`]) skips both here for the reason the
    /// cursor skips them.
    fn rows(&self, seen: &Seen<'_>) -> Vec<crate::region::Row> {
        self.view
            .rows(seen.columns, seen.zone, seen.texts)
            .into_iter()
            .map(|r| crate::region::Row {
                takes: r.takes(),
                fields: r.fields,
            })
            .collect()
    }

    /// `RET` chooses and nothing on a row says so.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view.hint(seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.drawn)
    }

    /// THE DRAWN ROW, and not the cursor: this view's cursor counts the rows
    /// one can STAND on, and between them sit the titles, the blank rows and
    /// the notes — and a note is as many rows as the width allows. What the
    /// region scrolls to is a line on the screen (`View::drawn_row`).
    fn cursor_in(&self, seen: &Seen<'_>) -> Option<usize> {
        Some(
            self.view
                .drawn_row(&self.view.rows(seen.columns, seen.zone, seen.texts)),
        )
    }

    /// No preview: choosing a language WRITES, because the file is how the
    /// other doors hear about it, and there is nothing to put back on `Esc`.
    /// Every key is read by its whole notation out of `locale::OF_THE_VIEW`,
    /// through the reader the window calls too (`locale::press_of_key`):
    /// `S-<arrows>` turn the ring, and `z`, `a` and `c` ask their questions in
    /// both doors.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        match crate::locale::press_of_key(key) {
            Some(press) => {
                self.press(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // THE COMMAND THAT OPENED IT, the first time: nothing more to do.
            "locale-open" if !self.opened => self.opened = true,
            // A SECOND OPEN IS REFUSED, as every view that asks whether the
            // region is free refuses with the region held — here, by itself:
            // the live view is kept, with its cursor.
            "locale-open" => {
                tracing::info!(
                    view = id,
                    held = ID,
                    "refused: another view is open in the footer"
                );
                let said = way.texts().t(crate::key::ECHO_VIEW_BUSY);
                way.say(said, Echo::Normal);
                way.repaint();
            }
            "locale-window" => way.open_window(vec!["locale".to_owned()]),
            "locale-where" => self.about_a_place(
                argument,
                |a| matches!(a, Asking::Where(_) | Asking::Weather(_)),
                Asking::Where,
                way,
            ),
            "locale-currency" => self.about_a_place(
                argument,
                |a| matches!(a, Asking::Currency(_) | Asking::Rate(_)),
                Asking::Currency,
                way,
            ),
            other => match Asking::named(other) {
                Some(asking) => self.told_or_asked(argument, asking, way),
                None => return Taken::No,
            },
        }
        Taken::Yes
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19. A screen
    /// where one click changes the language is a screen you cannot read with
    /// a mouse in your hand.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let rows = self.rows_of(way);
        if self.view.clicked(row, &rows) {
            self.press(Press::Enter, way);
        } else {
            self.took_the_cursor(way);
            way.repaint();
        }
    }

    /// WHAT WAS TYPED ON THE FOOTER'S LINE, into the call a window's line
    /// answers into: the question this region asked, with the place it is
    /// about; an id that names its question alone is taken by its name.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        let asking = self
            .asking
            .take()
            .filter(|a| a.command() == id)
            .or_else(|| Asking::named(id));
        if let Some(asking) = asking {
            told(&asking, text, Some(&mut self.view), way);
            self.took_the_cursor(way);
        }
    }

    /// The question let go — and what it was about with it.
    fn unanswered(&mut self, id: &'static str, _: &mut dyn Doorway) {
        if self.asking.as_ref().is_some_and(|a| a.command() == id) {
            self.asking = None;
        }
    }

    /// The brain's answers about the watch, sent back off the loop.
    fn woken(&mut self, way: &mut dyn Doorway) {
        while let Ok(heard) = self.heard.try_recv() {
            heard_back(heard, &mut self.view, way);
        }
    }

    /// What the footer heard: the mind it streams (the brain's clock watch,
    /// which follows it — a switch made on the module's own page shows here),
    /// the zone it writes dates in, and the glance's places when they change
    /// (in another door, or by this view's own answer).
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(mind) = news.downcast_ref::<ronoctua_brain::Mind>() {
            if self.view.set_watched(crate::locale::watched_in(Some(mind))) {
                way.repaint();
            }
        } else if let Some(zone) = news.downcast_ref::<ronoctua_files::when::Zone>() {
            self.zone = zone.clone();
        } else if news
            .downcast_ref::<Vec<ronoctua_world::prefs::Wanted>>()
            .is_some()
        {
            self.view.reread_files();
            way.repaint();
        }
    }

    /// A LANGUAGE CHOSEN ELSEWHERE re-reads the files — the same move a
    /// choice made here makes, and the one a window makes.
    fn language_changed(&mut self, way: &mut dyn Doorway) {
        self.view.reread_files();
        way.repaint();
    }

    /// THE DATES' SHAPE CHOSEN ELSEWHERE is on these rows at once, and the
    /// next ring is turned from it.
    fn locale_changed(&mut self, way: &mut dyn Doorway) {
        self.view.reread_files();
        way.repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder, Region};

    fn way() -> Recorder {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        Recorder::new(
            std::rc::Rc::new(texts),
            80,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
    }

    fn region() -> LocaleRegion {
        let (sent, heard) = std::sync::mpsc::channel();
        LocaleRegion {
            view: View::planted(false),
            asking: None,
            zone: ronoctua_files::when::Zone::utc(),
            drawn: 0,
            opened: true,
            sent,
            heard,
        }
    }

    /// FAILS IF: the command that opened the view is refused as a second
    /// open — the view would open saying another view holds the region — or
    /// a second open is not refused.
    #[test]
    fn the_first_open_is_the_opening_and_a_second_is_refused() {
        let mut w = way();
        let mut r = region();
        r.opened = false;
        assert_eq!(r.command("locale-open", None, &mut w), Taken::Yes);
        assert!(
            w.owed.is_empty(),
            "the opening said something: {:?}",
            w.owed
        );
        assert_eq!(r.command("locale-open", None, &mut w), Taken::Yes);
        let busy = w.texts().t(crate::key::ECHO_VIEW_BUSY);
        assert_eq!(w.owed.first(), Some(&Effect::Say(busy, Echo::Normal)));
    }

    /// FAILS IF: a question the view asks by name stops being asked — or
    /// answered — with the view closed, or a question about a place answers
    /// for a place nobody is looking at.
    #[test]
    fn a_closed_view_still_asks_its_questions_by_name() {
        let mut w = way();
        assert_eq!(closed("locale-open", None, &mut w), Closed::Opens);
        assert_eq!(closed("locale-zone", None, &mut w), Closed::Taken);
        assert_eq!(
            w.owed.first(),
            Some(&Effect::Ask(Line::new("locale-zone", false, None)))
        );
        let mut w = way();
        assert_eq!(closed("locale-where", Some("1,2"), &mut w), Closed::Taken);
        let none_here = w.texts().t(crate::key::LOCALE_NO_PLACE_HERE);
        assert_eq!(w.owed.first(), Some(&Effect::Say(none_here, Echo::Normal)));
        let mut w = way();
        assert_eq!(
            closed("locale-system-zone", Some("Nowhere/At_All"), &mut w),
            Closed::Taken
        );
        assert!(
            w.owed.iter().any(|e| matches!(e, Effect::Say(..))),
            "a zone that cannot be read was taken in silence: {:?}",
            w.owed
        );
        assert_eq!(closed("geo-open", None, &mut w), Closed::NotMine);
    }

    /// FAILS IF: the errand handler does a thing in one door that it does
    /// not ask of the door — a question not asked on the door's line and not
    /// kept for its answer, a close that does not close.
    #[test]
    fn the_errands_are_asked_of_the_door() {
        let mut w = way();
        let mut r = region();
        let back = || -> Box<dyn FnOnce(Heard) + Send> { Box::new(|_| {}) };
        carry_out(
            Errand::Ask(Asking::Place),
            &mut r.view,
            &mut r.asking,
            &back,
            &mut w,
        );
        assert_eq!(r.asking, Some(Asking::Place));
        carry_out(Errand::Close, &mut r.view, &mut r.asking, &back, &mut w);
        assert_eq!(
            w.owed,
            vec![
                Effect::Ask(Line::new("locale-place", false, None)),
                Effect::Repaint,
                Effect::Close,
            ]
        );
    }

    /// FAILS IF: a photograph's door reaches for the brain's socket when the
    /// watch is switched — it says, as a door with no brain does, that the
    /// watch is not known.
    #[test]
    fn a_photographs_door_does_not_ask_the_brain() {
        let mut w = way();
        let mut r = region();
        let asked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let seen = std::sync::Arc::clone(&asked);
        let back = move || -> Box<dyn FnOnce(Heard) + Send> {
            seen.store(true, std::sync::atomic::Ordering::SeqCst);
            Box::new(|_| {})
        };
        carry_out(
            Errand::Watch(true),
            &mut r.view,
            &mut r.asking,
            &back,
            &mut w,
        );
        assert!(
            !asked.load(std::sync::atomic::Ordering::SeqCst),
            "the brain was asked"
        );
        let unknown = w.texts().t(crate::key::LOCALE_WATCHED_UNKNOWN);
        assert_eq!(w.owed.first(), Some(&Effect::Say(unknown, Echo::Normal)));
    }

    /// FAILS IF: what the brain said is not what the door says — or the
    /// watch does not follow it — or a mind handed in by the footer does not
    /// reach the row.
    #[test]
    fn what_the_brain_says_reaches_the_row() {
        let mut w = way();
        let mut r = region();
        heard_back(
            Heard {
                answer: Some(crate::mind_wire::Answer::Unreachable),
                watched: None,
            },
            &mut r.view,
            &mut w,
        );
        let unknown = w.texts().t(crate::key::LOCALE_WATCHED_UNKNOWN);
        assert_eq!(w.owed.first(), Some(&Effect::Say(unknown, Echo::Normal)));
        assert!(
            !r.view.set_watched(None),
            "the watch did not follow the answer"
        );
        let mut w = way();
        r.heard(&ronoctua_files::when::Zone::utc(), &mut w);
        assert!(w.owed.is_empty(), "a zone handed in is not a frame");
    }

    /// FAILS IF: an answer to a question about a place lands anywhere but on
    /// the question asked, or a question let go is answered later.
    #[test]
    fn an_answer_lands_on_its_question() {
        let mut w = way();
        let mut r = region();
        r.asking = Some(Asking::Where("Europe/Lisbon".to_owned()));
        r.unanswered("locale-zone", &mut w);
        assert!(
            r.asking.is_some(),
            "another question's let-go took this one"
        );
        r.unanswered("locale-where", &mut w);
        assert!(r.asking.is_none());
        assert_eq!(r.command("locale-zone", None, &mut w), Taken::Yes);
        assert_eq!(r.asking, Some(Asking::Zone));
        assert_eq!(r.command("geo-open", None, &mut w), Taken::No);
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a title, the air between sections, a note —
    /// or leaves unlit one it takes. THE LIGHT IS THE CLICK: every row
    /// clicked, on a view of its own, and lit exactly where something was
    /// seen to happen.
    ///
    /// CLICKED ONCE, AND NEVER ON THE CURSOR'S ROW — as
    /// `region::held_to_the_click` clicks too: a second click would be the `RET`, and
    /// a `RET` here writes this machine's language, dates and places and
    /// turns its clock-keeping (`carry_out`), which no test may do. One click
    /// is enough for the light: from any other row it selects, and the cursor
    /// moving is seen; on the cursor's row it is already the `RET`, and that
    /// row is lit by being the cursor's (`Held::here`).
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let t = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: 80,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        // THE ROWS' TONES AND PLACES, and not their words: a row of this view
        // says the time to the minute, and a minute turning between the two
        // readings is no click's doing. A click that selects moves a tone.
        let tinted = |r: &LocaleRegion, w: &crate::region::Recorder| {
            let (rows, cursor, deeds) = crate::region::seen(r, &w.owed, &seen);
            let tones: Vec<Vec<(u16, ronoctua_tokens::Color)>> = rows
                .iter()
                .map(|f| f.iter().map(|x| (x.at, x.color)).collect())
                .collect();
            (tones, cursor, deeds)
        };
        let first = region();
        let cursor = first.cursor_in(&seen);
        let held: Vec<crate::region::Held> = first
            .rows(&seen)
            .iter()
            .enumerate()
            .map(|(row, drawn)| {
                let here = cursor == Some(row);
                let (mut r, mut w) = (region(), way());
                let before = tinted(&r, &w);
                if !here {
                    r.clicked(row, &mut w);
                }
                crate::region::Held {
                    row,
                    lit: drawn.takes,
                    did: tinted(&r, &w) != before,
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
}
