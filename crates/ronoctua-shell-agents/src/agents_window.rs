//! The LLM accounts, in a window of their own — the second door onto
//! `crate::agents` (`LEADER / a w`).
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
//! # Two columns, the footer's page
//!
//! The rows are the view's, laid side by side through the same
//! [`crate::agents::Page`] the footer's region draws by; the pointer lights
//! the half of a line, or the tab, a click there takes — the cell of a press
//! is caught where the pointer is read ([`Content::pointer`]), since a click
//! is handed only its row.
//!
//! # What a photograph reaches
//!
//! Nothing past the door: a photograph that pressed `n` would open a
//! session of an agent, and one that opened would refresh — the network.
//! Both go through one field ([`Reach`]); a photograph's writes down what
//! would have happened instead, and reads on the spot ([`Reader::Here`]).

use crate::agents::Open;
pub use crate::agents::{Reach, Reader};
use crate::door::{Canvas, Content, Door, Hand, Ui};
use anyhow::Result;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop,
    seat::{
        keyboard::{KeyEvent, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
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
    /// The cell of the line the last press landed on — a click is handed
    /// only its row, and side by side the cell says which half, or which
    /// tab.
    clicked_cell: Option<u16>,
    /// The width and the words last laid out with, for the cursor's line
    /// (`Content::cursor` is asked with no `Ui`).
    laid: std::cell::RefCell<Option<(u16, std::rc::Rc<ronoctua_i18n::Texts>)>>,
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
            clicked_cell: None,
            laid: std::cell::RefCell::new(None),
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

    /// The view's rows, one list — what a test stands on.
    #[cfg(test)]
    fn view_rows(&self, ui: &Ui) -> Vec<crate::agents::Row> {
        self.open.rows(ui.columns, &ui.texts)
    }

    /// The rows and the page they are drawn on.
    fn page(&self, ui: &Ui) -> (Vec<crate::agents::Row>, crate::agents::Page) {
        self.laid
            .replace(Some((ui.columns, std::rc::Rc::clone(&ui.texts))));
        self.open.page(ui.columns, &ui.texts)
    }

    /// THE POINTER LIGHTS WHAT THE CLICK TAKES: each hand's band laid only
    /// where a click there takes something (`Page::lit`, the footer's own
    /// answer) — the half of a line side by side, a tab, the whole line in
    /// one list. Its own function, so a test holds it: a frame's canvas
    /// needs a GPU.
    fn hover(&self, ui: &Ui, pointers: &[Pointing]) -> Vec<ronoctua_render::quad::Quad> {
        use crate::region::Lit;
        let (rows, page) = self.page(ui);
        let cell_px = ui.frame(0).cell();
        let all = vec![true; page.lines.len()];
        let mut out = Vec::new();
        for &((colour, line), x) in pointers {
            let Some(line) = line else {
                continue;
            };
            let cell = x.and_then(|x| cell_at(ui, f64::from(x)));
            let stretch = match page.lit(line, cell, &rows) {
                Lit::Nothing => continue,
                Lit::Row => None,
                Lit::Cells(from, to) => Some((from, to)),
            };
            for mut band in crate::door::list::hover_bands(
                &[(colour, Some(line))],
                &all,
                None,
                ui.offset,
                ui.room_for_rows(),
                page.lines.len(),
                ui.first_of_list(),
                ui.room.band_width(),
            ) {
                if let Some((from, to)) = stretch {
                    let left = f32::from(ui.margin + from) * cell_px;
                    let right = (f32::from(ui.margin + to) * cell_px).min(band.width);
                    band.x = if from == 0 { 0.0 } else { left };
                    band.width = (right - band.x).max(0.0);
                }
                out.push(band);
            }
        }
        out
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

    /// Its name at the right end of the first line, with no header
    /// (`door::corner`).
    fn corner(&self, ui: &Ui) -> Option<crate::door::corner::Corner> {
        Some(crate::door::corner::Corner::named(
            &ui.texts,
            crate::key::AGENTS_TITLE,
        ))
    }

    /// A key, read by its whole notation through the reader the footer calls
    /// too (`Open::key`): the file picker's, while it is up, then the view's.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(key) = crate::door::keys::key_of(event, mods) {
            ui.message = None;
            self.open.view.page_is(ui.room_for_rows());
            self.open.key(key, ui);
            self.arm_the_clock();
        }
    }

    /// The clipboard's text, read off the loop for the file picker's `C-v`.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        self.open.pasted(text, ui);
    }

    /// `TAB` crosses the columns — or reaches the file picker.
    fn walks_sections(&self, _: &Ui) -> bool {
        false
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
    /// draws for these rows, laid side by side on the same page.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.page(ui).1.lines
    }

    /// A click takes a line where either half takes one — the view's one
    /// answer ([`crate::agents::Row::takes`]), which the footer's region
    /// gives too; WHICH half is the pointer's cell ([`Self::hover`]).
    fn takes(&self, ui: &Ui) -> Vec<bool> {
        let (rows, page) = self.page(ui);
        (0..page.lines.len())
            .map(|n| page.takes(n, &rows))
            .collect()
    }

    /// A pair wrapped over several lines is one control: in one list, its
    /// lines light together, as in the footer (`View::control_of`). Side by
    /// side a line is two rows, and the light is the half's.
    fn controls(&self, ui: &Ui) -> Option<Vec<usize>> {
        let (rows, page) = self.page(ui);
        page.split.is_none().then(|| {
            (0..rows.len())
                .map(|n| crate::agents::View::control_of(n, &rows))
                .collect()
        })
    }

    /// The line the cursor's row is drawn on.
    fn cursor(&self) -> Option<usize> {
        let cursor = self.open.view.cursor();
        match self.laid.borrow().as_ref() {
            Some((columns, texts)) => self.open.page(*columns, texts).1.line_of(cursor),
            None => Some(cursor),
        }
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

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19 — at the
    /// cell the press landed on: the half of a line side by side, a tab,
    /// which shows its account at once.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        ui.message = None;
        self.open.clicked_at(row, self.clicked_cell, ui);
        self.arm_the_clock();
        ui.ask_paint();
    }

    /// The header, the page, the hover of the half — or the tab — a click
    /// takes, and the rule between the columns, lit while the accounts' has
    /// the keyboard (the file manager's focus mark).
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        crate::door::list::header(self, ui, canvas);
        let (rows, page) = self.page(ui);
        let controls = Content::controls(self, ui);
        crate::door::list::body(ui, canvas, &page.lines, &[], controls.as_deref());
        let pointers: Vec<Pointing> = canvas
            .lit
            .iter()
            .copied()
            .zip(canvas.lit_x.iter().copied().chain(std::iter::repeat(None)))
            .collect();
        canvas.under.extend(self.hover(ui, &pointers));
        if let Some(split) = page.split {
            let first = ui.first_of_list();
            let f = ui.frame(first);
            let cell = f.cell();
            let x = f32::from(ui.margin + split.saturating_sub(1)) * cell + cell / 2.0;
            let lit = rows
                .get(self.open.view.cursor())
                .is_some_and(|r| r.pane == crate::agents::Pane::Accounts);
            canvas.under.push(if lit {
                crate::chrome::divider_lit(x, f.y_of(first), f.bottom())
            } else {
                crate::chrome::divider(x, f.y_of(first), f.bottom())
            });
        }
    }

    /// The standard list, and where along the line a press landed, which
    /// `clicked` needs and is not handed.
    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<()>, events: &[PointerEvent]) {
        for e in events {
            if let PointerEventKind::Press { button, .. } = e.kind
                && button == crate::door::LEFT_BUTTON
            {
                self.clicked_cell = cell_at(ui, e.position.0);
            }
        }
        crate::door::list::pointer(self, ui, hand, events);
    }

    /// What a thread or a watch brought: a reading, a refresh, a stir, the
    /// file picker's folder — and the clock moved to the reset the reading
    /// shows.
    fn woken(&mut self, ui: &mut Ui) {
        self.open.heard(ui);
        self.arm_the_clock();
    }
}

/// A hand's colour and line, and where along the line it is.
type Pointing = ((tokens::Color, Option<usize>), Option<f32>);

/// The cell of the line a pointer at `x` is over, counted from the window's
/// margin as the fields are.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a pointer inside a window, divided by a cell"
)]
fn cell_at(ui: &Ui, x: f64) -> Option<u16> {
    let cell = f64::from(tokens::metrics::cell_width(tokens::text::size()));
    if cell <= 0.0 {
        return None;
    }
    let inset = cell * f64::from(ui.margin);
    Some(((x - inset).max(0.0) / cell) as u16)
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

    /// FAILS IF: the window's resting pointer lights a row a click takes
    /// nothing on — a title, the air between sections, a sentence — or
    /// leaves unlit one it takes. THE LIGHT IS THE CLICK: every row clicked
    /// once, on a window of its own, and lit exactly where something
    /// happened. The footer's region has its own.
    #[test]
    fn the_windows_pointer_lights_what_the_click_takes() {
        let held = crate::door::held_to_the_control(a_window, crate::door::seen, |w, ui, row| {
            crate::agents::View::control_of(row, &w.view_rows(ui))
        });
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }
}
