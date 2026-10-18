//! The LLM accounts, in a window of their own — the second door onto
//! `crate::agents` (`LEADER m a w`).
//!
//! # What is here and what is not
//!
//! The plumbing of a window is `crate::door`'s; what is here is the view
//! open in a window ([`crate::agents::Open`] — what it read, the watches
//! that keep it current and the threads that read and refresh) and the
//! window's one clock. It DECIDES nothing — which rows exist, what a key
//! does, which edit a gesture makes is `agents.rs`, and both doors hand the
//! view's errands to the same `agents::carry_out`.
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

use crate::agents::{Open, Press};
pub use crate::agents::{Reach, Reader};
use crate::door::{Content, Door, Ui};
use anyhow::Result;
use smithay_client_toolkit::{
    reexports::calloop,
    seat::keyboard::{KeyEvent, Modifiers},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.agents";

/// Opens the accounts in a window, on the account at `place` (from 1) when
/// one is given.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it(place: Option<usize>) -> Result<()> {
    crate::door::run(|w| {
        let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
        let mut window = AgentsWindow::over(
            zone,
            Reader::Thread(w.wake()),
            Reach::Machine,
            place,
            &w.texts,
        );
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
    /// The view, open in this window.
    open: Open,
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
        texts: &ronoctua_i18n::Texts,
    ) -> Self {
        Self {
            open: Open::new(zone, reader, reach, place, OPENING_COLUMNS, texts),
            loop_handle: None,
            clock: None,
        }
    }

    /// Arms the window's one clock at the nearest reset it shows — moved
    /// when another is nearer, taken down when none is shown. When it rings
    /// the window reads again: the window that reset is not known any more,
    /// and no file says so. (The footer's region asks on the second the
    /// footer already beats: `Open::ring_if_due`.)
    fn arm_the_clock(&mut self) {
        let Some(handle) = self.loop_handle.clone() else {
            return;
        };
        let wanted = self.open.next_reset();
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
                door.content.open.read_again();
                calloop::timer::TimeoutAction::Drop
            },
        );
        match armed {
            Ok(token) => self.clock = Some((at, token)),
            Err(e) => tracing::warn!(error = %e, "the LLM accounts' reset clock could not be hung"),
        }
    }

    /// What one key means — from the keyboard, or `RET` from a second click.
    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        self.open.press(press, ui);
        self.arm_the_clock();
    }

    fn view_rows(&self, ui: &Ui) -> Vec<crate::agents::Row> {
        self.open.rows(ui.columns, &ui.texts)
    }

    /// What would have happened — a photograph's.
    #[must_use]
    pub fn done(&self) -> &[String] {
        self.open.done()
    }
}

impl Content for AgentsWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::AGENTS_TITLE))
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
        self.open.answered(id, text, ui);
        self.arm_the_clock();
    }

    /// The line was let go, and the view forgets the question.
    fn unanswered(&mut self, ui: &mut Ui, _: &'static str) {
        self.open.view.let_go();
        ui.ask_paint();
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.open
            .view
            .hint(&self.open.readings, &self.open.zone, ui.columns, &ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer's door
    /// draws for these rows.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.open.view.cursor())
    }

    /// A new width is new rows, so the cursor is found again by what it is
    /// on.
    fn resized(&mut self, ui: &mut Ui) {
        self.open.settle(ui.columns, &ui.texts);
    }

    /// And so is another language.
    fn language_changed(&mut self, ui: &mut Ui) {
        self.open.settle(ui.columns, &ui.texts);
        ui.ask_paint();
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        if self.open.view.clicked(row, &rows) {
            self.pressed(ui, Press::Enter);
        } else {
            ui.ask_paint();
        }
    }

    /// What a thread or a watch brought: a reading, a refresh, a stir — and
    /// the clock moved to the reset the reading shows.
    fn woken(&mut self, ui: &mut Ui) {
        self.open.heard(ui);
        self.arm_the_clock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::Readings;

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
            &texts,
        );
        (w, Ui::for_tests(texts))
    }

    fn stand_on(w: &mut AgentsWindow, ui: &mut Ui, target: &crate::agents::Target) {
        let rows = w.view_rows(ui);
        let at = rows
            .iter()
            .position(|r| r.target.as_ref() == Some(target))
            .unwrap_or_else(|| panic!("no row is {target:?}"));
        if w.open.view.cursor() == at {
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
        w.clicked(&mut ui, w.open.view.cursor());
        let line = ui.asking.take().expect("the name is not asked here");
        assert_eq!(line.id, "agents-add-codex");
        assert!(!line.secret);
        w.unanswered(&mut ui, line.id);
        assert!(w.open.view.asking().is_none());
    }

    /// FAILS IF: a refresh from the window reaches past a photograph, or
    /// says nothing on the window's line.
    #[test]
    fn a_refresh_is_said_here_and_reaches_nothing_from_a_photograph() {
        let (mut w, mut ui) = a_window();
        stand_on(&mut w, &mut ui, &crate::agents::Target::Refresh);
        w.clicked(&mut ui, w.open.view.cursor());
        assert_eq!(w.done(), ["refresh"]);
        assert!(ui.message.is_some(), "nothing was said");
        assert!(ui.launches().is_empty());
    }
}
