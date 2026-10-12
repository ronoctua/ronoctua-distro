//! The LLM accounts, in a window of their own — the second door onto
//! `crate::agents` (`LEADER m a w`).
//!
//! # What is here and what is not
//!
//! The plumbing of a window is `crate::door`'s; what is here is the view,
//! what it read, the watches that keep it current and the threads that read
//! and refresh. It DECIDES nothing — which rows exist, what a key does,
//! which edit a gesture makes is `agents.rs`, and both doors ask the same
//! view the same questions and apply its edits through the same
//! `agents::apply`.
//!
//! # It types what the footer types
//!
//! An account's name, a new account's, the two lines: asked on this
//! window's own line (`Ui::ask`), answered through the same `View::answered`
//! the footer's minibuffer reaches. A session and a login open in the
//! terminal from here, and what came of a gesture is said on this window's
//! line.
//!
//! # What a photograph reaches
//!
//! Nothing past the door: a photograph that pressed `C-n` would open a
//! session of an agent, and one that opened would refresh — the network.
//! Both go through one field ([`Reach`]); a photograph's writes down what
//! would have happened instead, and reads on the spot ([`Reader::Here`]).

use crate::agents::{Errand, Heard, Press, Readings, View, Watching};
use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop::{self, channel},
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.agents";

/// Where a session, a login and a refresh go.
#[derive(Debug)]
pub enum Reach {
    /// The machine: the terminal, the network.
    Machine,
    /// Nowhere: what would have happened is written down — a photograph's.
    Nowhere(Vec<String>),
}

/// How the window reads.
pub enum Reader {
    /// On a thread, the reading sent back on the loop.
    Thread(channel::Sender<Heard>),
    /// Here, at once, by this function — a photograph's, which has no loop
    /// turning (`agents::read`), or a test's, which reads what it planted.
    Here(fn(&ronoctua_files::when::Zone) -> Readings),
}

/// Opens the accounts in a window, on the account at `place` (from 1) when
/// one is given.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it(place: Option<usize>) -> Result<()> {
    crate::door::run(|w| {
        let (sender, heard) = channel::channel::<Heard>();
        w.handle
            .insert_source(heard, |event, (), door: &mut Door<AgentsWindow>| {
                if let channel::Event::Msg(heard) = event {
                    door.content.heard(heard, &mut door.ui);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the accounts' channel: {e}"))?;
        let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
        let mut window = AgentsWindow::over(zone, Reader::Thread(sender), Reach::Machine, place);
        // THE LOOP, for the one clock at the nearest reset shown — set
        // before the loop turns, so the first reading finds it.
        window.loop_handle = Some(w.handle.clone());
        Ok(window)
    })
}

/// The width the cursor is first settled at, before the door has one.
const OPENING_COLUMNS: u16 = 100;

/// The window.
pub struct AgentsWindow {
    view: View,
    readings: Readings,
    zone: ronoctua_files::when::Zone,
    reader: Reader,
    /// Where sessions and refreshes go.
    pub reach: Reach,
    watching: Watching,
    _watches: Vec<ronoctua_files::notice::Notice>,
    /// A reading in flight, and one more wanted after it.
    reading: bool,
    again: bool,
    /// The account to aim at once the first reading is here.
    place: Option<usize>,
    /// The loop, for the reset clock — `None` with no loop turning (a
    /// photograph, a test): no clock.
    loop_handle: Option<calloop::LoopHandle<'static, Door<Self>>>,
    /// The one clock, at the nearest reset shown (`agents::next_reset`):
    /// when it rings, and its place on the loop.
    clock: Option<(i64, calloop::RegistrationToken)>,
}

impl AgentsWindow {
    /// The window: read at once, or a reading and a refresh asked of their
    /// threads — the numbers shown are fresh when the person looks.
    #[must_use]
    pub fn over(
        zone: ronoctua_files::when::Zone,
        reader: Reader,
        reach: Reach,
        place: Option<usize>,
    ) -> Self {
        let mut me = Self {
            view: View::default(),
            readings: Readings::default(),
            zone,
            reader,
            reach,
            watching: Watching::default(),
            _watches: Vec::new(),
            reading: false,
            again: false,
            place,
            loop_handle: None,
            clock: None,
        };
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT);
        match me.reader {
            Reader::Here(read) => {
                let r = read(&me.zone);
                if let Some(t) = texts.as_ref() {
                    me.arrived(r, OPENING_COLUMNS, t);
                } else {
                    me.readings = r;
                }
            }
            Reader::Thread(_) => {
                me.read_again();
                me.refresh();
            }
        }
        me
    }

    /// Asks for a reading — at once, or of a thread, one at a time: a
    /// change heard while one is in flight is one more after it.
    fn read_again(&mut self) {
        match &self.reader {
            Reader::Here(read) => {
                self.readings = read(&self.zone);
            }
            Reader::Thread(sender) => {
                if self.reading {
                    self.again = true;
                    return;
                }
                self.reading = true;
                let sender = sender.clone();
                crate::agents::read_off_the_loop(self.zone.clone(), move |h| {
                    let _ = sender.send(h);
                });
            }
        }
    }

    /// A refresh: counted and asked off the loop — or written down.
    fn refresh(&mut self) {
        match (&mut self.reach, &self.reader) {
            (Reach::Machine, Reader::Thread(sender)) => {
                let sender = sender.clone();
                crate::agents::refresh_off_the_loop(move |h| {
                    let _ = sender.send(h);
                });
            }
            (Reach::Nowhere(done), _) => done.push("refresh".to_owned()),
            (Reach::Machine, Reader::Here(_)) => {}
        }
    }

    /// A reading arrived: kept, the cursor settled, the watches raised again
    /// over what it covers now.
    fn arrived(&mut self, r: Readings, columns: u16, texts: &ronoctua_i18n::Texts) {
        self.readings = r;
        if let Some(place) = self.place.take() {
            self.view.aim_at_place(place, &self.readings);
        }
        self.view
            .settle_in(&self.readings, &self.zone, columns, texts);
        if let Reader::Thread(sender) = &self.reader {
            let now = crate::agents::watched_now(&self.readings);
            if now != self.watching {
                let sender = sender.clone();
                self._watches = now.raise(move || sender.send(Heard::Stirred).is_ok());
                self.watching = now;
            }
        }
        self.arm_the_clock();
    }

    /// Arms the window's one clock at the nearest reset it shows — moved
    /// when another is nearer, taken down when none is shown. When it rings
    /// the window reads again: the window that reset is not known any more,
    /// and no file says so. The footer's twin (`Footer::agents_arm_the_clock`).
    fn arm_the_clock(&mut self) {
        let Some(handle) = self.loop_handle.clone() else {
            return;
        };
        let wanted = crate::agents::next_reset(&self.readings);
        if self.clock.as_ref().map(|(at, _)| *at) == wanted {
            return;
        }
        if let Some((_, token)) = self.clock.take() {
            handle.remove(token);
        }
        let Some(at) = wanted else {
            return;
        };
        let armed = handle.insert_source(
            calloop::timer::Timer::from_duration(crate::agents::until(at)),
            |_, (), door: &mut Door<Self>| {
                door.content.clock = None;
                door.content.read_again();
                calloop::timer::TimeoutAction::Drop
            },
        );
        match armed {
            Ok(token) => self.clock = Some((at, token)),
            Err(e) => tracing::warn!(error = %e, "the LLM accounts' reset clock could not be hung"),
        }
    }

    /// What a thread or a watch sent.
    fn heard(&mut self, heard: Heard, ui: &mut Ui) {
        match heard {
            Heard::Stirred => self.read_again(),
            Heard::Read(r) => {
                self.reading = false;
                self.arrived(*r, ui.columns, &ui.texts);
                if std::mem::take(&mut self.again) {
                    self.read_again();
                }
                ui.ask_paint();
            }
            Heard::Refreshed(done) => {
                ui.say(crate::agents::refreshed_said(&done, &ui.texts));
                self.read_again();
            }
        }
    }

    /// What was read, again, here — after a gesture of this window's, so
    /// what it wrote is drawn at once.
    fn reread_here(&mut self, ui: &mut Ui) {
        match self.reader {
            Reader::Here(read) => {
                let r = read(&self.zone);
                self.arrived(r, ui.columns, &ui.texts);
            }
            Reader::Thread(_) => self.read_again(),
        }
        ui.ask_paint();
    }

    /// A terminal with these words — or written down.
    fn open_terminal(&mut self, ui: &mut Ui, words: Vec<String>) {
        match &mut self.reach {
            Reach::Machine => match std::env::current_exe() {
                Ok(me) => ui.launch(&me.to_string_lossy(), &words, None),
                Err(e) => tracing::warn!(error = %e, "this program's own path"),
            },
            Reach::Nowhere(done) => done.push(words.join(" ")),
        }
    }

    /// Carries out what the view asked for — and reads again only after a
    /// write: a cursor walking writes nothing, and a reading opens every
    /// account's files.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        let wrote = errand.writes();
        match errand {
            Errand::Nothing | Errand::Wrote => {}
            Errand::Close => {
                ui.quit = true;
                return;
            }
            Errand::Ask(line) => ui.ask(line),
            Errand::Say(said) => ui.say_for(said, crate::echo::Echo::Long),
            Errand::Copy(text) => ui.say_for(
                crate::chrome::copied(&text, &ui.texts),
                crate::echo::Echo::Brief,
            ),
            Errand::Edit(edit) => {
                let applied = crate::agents::apply(&edit, &ui.texts);
                if let Some(said) = applied.said {
                    ui.say_for(said, crate::echo::Echo::Long);
                }
                if let Some(words) = applied.terminal {
                    self.open_terminal(ui, words);
                }
            }
            Errand::Open(id, login) => {
                let opened = self
                    .readings
                    .seen(&id)
                    .map(|s| crate::agents::terminal_of(&s.account, login));
                match opened {
                    Some(Ok(words)) => self.open_terminal(ui, words),
                    Some(Err(r)) => ui.say_for(
                        crate::agents_words::refused(r.why, &ui.texts),
                        crate::echo::Echo::Long,
                    ),
                    None => ui.say(ui.texts.t(ronoctua_i18n::key::AGENTS_NONE)),
                }
            }
            Errand::Refresh => {
                ui.say(ui.texts.t(ronoctua_i18n::key::AGENTS_REFRESHING));
                self.refresh();
            }
        }
        if wrote {
            self.reread_here(ui);
        } else {
            ui.ask_paint();
        }
    }

    /// What one key means — from the keyboard, or `RET` from a second click.
    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        let errand = self
            .view
            .press(press, &self.readings, &self.zone, ui.columns, &ui.texts);
        self.apply(ui, errand);
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::agents::Row> {
        self.view
            .rows(&self.readings, &self.zone, ui.columns, &ui.texts)
    }

    /// What would have happened — a photograph's.
    #[must_use]
    pub fn done(&self) -> &[String] {
        match &self.reach {
            Reach::Machine => &[],
            Reach::Nowhere(done) => done,
        }
    }
}

impl Content for AgentsWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::AGENTS_TITLE))
    }

    /// A key, read by its whole notation through the reader the footer calls
    /// too (`agents::press_of_key`).
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::agents::press_of_key)
        {
            self.pressed(ui, press);
        }
    }

    /// What was typed on this window's line: the same `View::answered` the
    /// footer's minibuffer reaches.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        ui.message = None;
        let errand = self.view.answered(id, text, &ui.texts);
        self.apply(ui, errand);
    }

    /// The line was let go, and the view forgets the question.
    fn unanswered(&mut self, ui: &mut Ui, _: &'static str) {
        self.view.let_go();
        ui.ask_paint();
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view
            .hint(&self.readings, &self.zone, ui.columns, &ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// A new width is new rows, so the cursor is found again by what it is
    /// on.
    fn resized(&mut self, ui: &mut Ui) {
        self.view
            .settle_in(&self.readings, &self.zone, ui.columns, &ui.texts);
    }

    /// And so is another language.
    fn language_changed(&mut self, ui: &mut Ui) {
        self.view
            .settle_in(&self.readings, &self.zone, ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        if self.view.clicked(row, &rows) {
            self.pressed(ui, Press::Enter);
        } else {
            ui.ask_paint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine with no account yet — and never this machine's: a test
    /// reads what it planted.
    fn planted(_: &ronoctua_files::when::Zone) -> Readings {
        Readings {
            read: true,
            ..Readings::default()
        }
    }

    fn a_window() -> (AgentsWindow, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let w = AgentsWindow::over(
            ronoctua_files::when::Zone::utc(),
            Reader::Here(planted),
            Reach::Nowhere(Vec::new()),
            None,
        );
        (w, Ui::for_tests(texts))
    }

    fn stand_on(w: &mut AgentsWindow, ui: &mut Ui, target: &crate::agents::Target) {
        let rows = w.view_rows(ui);
        let at = rows
            .iter()
            .position(|r| r.target.as_ref() == Some(target))
            .unwrap_or_else(|| panic!("no row is {target:?}"));
        if w.view.cursor() == at {
            return;
        }
        w.clicked(ui, at);
        assert!(ui.asking.is_none(), "the first click acted");
    }

    /// FAILS IF: a new account's name is not asked on this window's own line
    /// — the owner's rule: the two doors do the same, typing included — or
    /// the question is still answered once let go.
    #[test]
    fn a_new_account_is_named_in_the_window() {
        let (mut w, mut ui) = a_window();
        stand_on(
            &mut w,
            &mut ui,
            &crate::agents::Target::Add(ronoctua_llm::Kind::Codex),
        );
        w.clicked(&mut ui, w.view.cursor());
        let line = ui.asking.take().expect("the name is not asked here");
        assert_eq!(line.id, "agents-add-codex");
        assert!(!line.secret);
        w.unanswered(&mut ui, line.id);
        assert!(w.view.asking().is_none());
    }

    /// FAILS IF: a refresh from the window reaches past a photograph, or
    /// says nothing on the window's line.
    #[test]
    fn a_refresh_is_said_here_and_reaches_nothing_from_a_photograph() {
        let (mut w, mut ui) = a_window();
        stand_on(&mut w, &mut ui, &crate::agents::Target::Refresh);
        w.clicked(&mut ui, w.view.cursor());
        assert_eq!(w.done(), ["refresh"]);
        assert!(ui.message.is_some(), "nothing was said");
        assert!(ui.launches().is_empty());
    }
}
