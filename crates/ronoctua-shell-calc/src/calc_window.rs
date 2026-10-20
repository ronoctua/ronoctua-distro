//! The calculator in a window of its own — the second door onto
//! `crate::calc` (`LEADER = w`).
//!
//! # What is the window's own
//!
//! The view is `calc.rs`'s: its modes, its keys, its lines, what a click on
//! any of them does. What this door adds is the SHAPE a window allows: the
//! keypad as CARDS a finger presses (`crate::calc_keypad`, measured by
//! `space.calc.*`, pressed where they are drawn), the history as a column on
//! the right that drops under the keypad when the window is narrow, and a
//! place on the screen — the window opens COMPACT and floating
//! (`crate::floating`), and `C-f` (its header's control) puts it back in the
//! layout among the others, or floats it again at the same size.
//!
//! # The place, through the contract
//!
//! The shell does not place windows (invariant 4): it asks the compositor, by
//! the contract (`Compositor::float_centred`, `back_in_the_layout`), to float
//! THIS window — found in its tree by this process and the `app_id` — or to
//! lay it out again. The size it floats at is the table's row, the one the
//! rule that opened it read.
//!
//! # What reaches the machine
//!
//! A kept answer is written into `calc.json` merged; the clipboard, the
//! consent for the rates' host and the rates are reached by the errand
//! handler the footer's region shares (`calc_region::carry_out`), and the
//! compositor through [`Reach`] — both of which a photograph keeps: what a
//! press would have done past the door is printed instead (`would have …`).

use crate::calc::{Errand, Line, Press, Target};
use crate::calc_keypad::{Button, Keypad, Tone};
use crate::door::{Canvas, Content, Hand, LEFT_BUTTON, Ui};
use anyhow::Result;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::{
    keyboard::{KeyEvent, Modifiers},
    pointer::{PointerEvent, PointerEventKind},
};

/// The calculator's window, floated by its row of the table of the windows
/// that open floating (`crate::floating`) — and named by it on the modeline
/// (`mode-calc`).
pub const APP_ID: &str = crate::floating::APP_ID_CALC;

/// The title's glyph, the keypad's.
static GLYPH: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| tokens::icon::CALC.to_string());

/// Opens the calculator in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        let mut window = CalcWindow::over(crate::calc::store(), Reach::machine());
        // `calc.json`, watched: an answer the footer keeps is in this history
        // at once — the file is the broadcast. What a thread of the
        // calculator finds (the file changed, the rates came) wakes this
        // door, which hands it in (`Content::woken`).
        window._watch =
            ronoctua_store::config_dir().and_then(|dir| window.calc.watch(&dir, w.wake()));
        Ok(window)
    })
}

/// Everything past the door that is the WINDOW'S: the compositor, which
/// floats it — or, for a photograph, a note of what would have been done.
/// (The clipboard, the consent and the rates are the calculator's, in both
/// doors: `calc_region::Calc`.)
pub enum Reach {
    /// The machine.
    Machine {
        /// The compositor, opened on the first ask and again after a failure.
        wm: Option<Box<dyn ronoctua_compositor::Compositor>>,
    },
    /// A photograph: nothing leaves, and every act is printed.
    Kept,
}

impl Reach {
    /// The machine's.
    #[must_use]
    pub const fn machine() -> Self {
        Self::Machine { wm: None }
    }

    const fn is_kept(&self) -> bool {
        matches!(self, Self::Kept)
    }
}

/// What the last frame laid out — what the pointer reads, so a press lands
/// on what was drawn (`AGENTS.md`: a control is grabbed where it is drawn).
#[derive(Debug, Default)]
struct Laid {
    /// Every line of fields: its surface line, the surface column its cell 0
    /// is at, the line, and the pixel its bands stop at — the bar's column of
    /// the list it is in, or the surface's edge.
    lines: Vec<(u16, u16, Line, f32)>,
    /// The keypad's cards.
    keypad: Keypad,
    /// The history's list.
    history: Option<ListBox>,
    /// The converter's list — and its bar's track, while it has one.
    units: Option<(ListBox, Option<crate::chrome::Rect>)>,
}

/// What the pointer lights: the card under it, and the lines — each with the
/// cells of it the band covers, which are what a press there takes: the line
/// under it, and every other line of its control (`Line::continues`).
type Lit = (Option<Button>, Vec<(usize, (u16, u16))>);

/// A list's box on the surface.
#[derive(Debug, Clone, Copy, Default)]
struct ListBox {
    /// The first row's surface line.
    first: u16,
    /// How many rows it shows.
    rows: u16,
    /// How many rows it has.
    total: usize,
    /// Its first row shown.
    offset: usize,
    /// Its right edge, in pixels — where its bar is drawn and grabbed.
    right: f32,
}

/// The window.
pub struct CalcWindow {
    /// The calculator: its view, its file and its threads — what the
    /// footer's region keeps too (`calc_region::Calc`).
    calc: crate::calc_region::Calc,
    reach: Reach,
    _watch: Option<ronoctua_files::notice::Notice>,
    /// What the last frame drew.
    laid: Laid,
    /// Where the pointer rests, in the surface's pixels.
    pointer_at: Option<(f64, f64)>,
    /// The history's first row shown — the window's own, since its list is
    /// not under the header (`Ui::offset` is the standard list's).
    history_offset: usize,
    /// The converter's list's first row shown, likewise: the wheel and the
    /// bar move it, and a key or a click that moves the cursor past the box
    /// brings it along.
    unit_offset: usize,
}

impl CalcWindow {
    /// The window over the history in `store`, reaching the machine through
    /// `reach`.
    #[must_use]
    pub fn over(store: ronoctua_store::Store<crate::calc::Kept>, reach: Reach) -> Self {
        let history = store.read().history;
        let zone = if reach.is_kept() {
            // A PHOTOGRAPH IS THE SAME PICTURE ON EVERY MACHINE.
            ronoctua_files::when::Zone::utc()
        } else {
            ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"))
        };
        let zoneinfo = std::env::var_os("RONOCTUA_ZONEINFO").map(std::path::PathBuf::from);
        let kept = reach.is_kept();
        Self {
            calc: crate::calc_region::Calc::over(
                crate::calc::View::fresh(history, zone, zoneinfo),
                store,
                kept,
            ),
            reach,
            _watch: None,
            laid: Laid::default(),
            pointer_at: None,
            history_offset: 0,
            unit_offset: 0,
        }
    }

    /// One press — a key, a card, a click — and what the view asks for
    /// after it, carried out by the errand handler both doors share.
    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        let left = self.calc.press(press, ui);
        self.after(ui, left);
    }

    /// A click on a target of the view.
    fn clicked_on(&mut self, ui: &mut Ui, target: Target) {
        ui.message = None;
        let left = self.calc.click(target, ui);
        self.after(ui, left);
    }

    /// After an errand: what is the window's own to do — float — and the
    /// lists brought along to their cursors.
    fn after(&mut self, ui: &mut Ui, left: Option<Errand>) {
        if left == Some(Errand::Float) {
            self.toggle_place(ui);
        }
        self.follow_the_pick();
        self.follow_the_unit();
        ui.ask_paint();
    }

    /// `C-f`: floating and compact → back in the layout; laid out → floating
    /// again, at the size of its row of `floating::FLOATING`.
    fn toggle_place(&mut self, ui: &mut Ui) {
        let Some(portion) = crate::floating::portion_of(APP_ID) else {
            return;
        };
        if self.reach.is_kept() {
            println!("would have floated or laid out the window");
            return;
        }
        let moved = self.with_my_window(|wm, id, floating| {
            if floating {
                wm.back_in_the_layout(id)
            } else {
                wm.float_centred(id, portion)
            }
        });
        if !moved {
            let said = ui.texts.t(crate::key::CALC_FLOAT_UNREACHED);
            ui.say(said);
        }
    }

    /// Runs `ask` on THIS window — found in the compositor's tree by this
    /// process and the app id, with whether it floats now; whether it was
    /// found and the ask went.
    fn with_my_window(
        &mut self,
        ask: impl FnOnce(&mut dyn ronoctua_compositor::Compositor, u64, bool) -> Result<()>,
    ) -> bool {
        let Reach::Machine { wm, .. } = &mut self.reach else {
            return false;
        };
        if wm.is_none() {
            *wm = crate::daemon::open_the_compositor().ok();
        }
        let Some(compositor) = wm.as_mut() else {
            return false;
        };
        let me = i32::try_from(std::process::id()).ok();
        let Some((id, floating)) = compositor.windows().ok().and_then(|all| {
            all.into_iter()
                .find(|w| w.pid == me && w.app_id.as_deref() == Some(APP_ID))
                .map(|w| (w.id, w.floating))
        }) else {
            return false;
        };
        match ask(compositor.as_mut(), id, floating) {
            Ok(()) => {
                tracing::info!(floating = !floating, "the calculator's place changed");
                true
            }
            Err(e) => {
                tracing::warn!(error = %e, "the compositor refused the calculator's place");
                // A CONNECTION THAT FAILED is opened again next time.
                *wm = None;
                false
            }
        }
    }

    /// Keeps the picked answer inside the history's rows — after a key or a
    /// click, never from a paint.
    fn follow_the_pick(&mut self) {
        let (Some(i), Some(h)) = (self.calc.view.picked(), self.laid.history) else {
            return;
        };
        let rows = usize::from(h.rows.max(1));
        if i < self.history_offset {
            self.history_offset = i;
        } else if i >= self.history_offset + rows {
            self.history_offset = i + 1 - rows;
        }
    }

    /// Keeps the converter's cursor inside its list's rows — after a key or
    /// a click, never from a paint, so the wheel and the bar can move the
    /// list away from it.
    fn follow_the_unit(&mut self) {
        let Some((u, _)) = self.laid.units else {
            return;
        };
        let rows = usize::from(u.rows.max(1));
        let at = self.calc.view.unit_cursor();
        if at < self.unit_offset {
            self.unit_offset = at;
        } else if at >= self.unit_offset + rows {
            self.unit_offset = at + 1 - rows;
        }
    }

    /// Lays the frame out: the head, the keypad or the converter, the
    /// history — and remembers it for the pointer.
    #[allow(clippy::too_many_lines)]
    fn lay_out(&self, ui: &Ui) -> Laid {
        let texts = ui.texts.clone();
        let mut laid = Laid::default();
        let columns = ui.columns;
        let x0 = ui.margin;
        let bottom = ui.lines;
        let f = ui.frame(0);
        let mut y = ui.first_of_list();
        for l in self.calc.view.top_lines(columns, &texts) {
            laid.lines.push((y, x0, l, f.width));
            y += 1;
        }
        y += 1;
        let gap = tokens::space::CALC_HISTORYGAP;
        let least = tokens::space::CALC_HISTORYMINCELLS;
        // UNDER, the history keeps a blank line, its title and this many
        // answers in view.
        let under_reserve = tokens::space::CALC_HISTORYUNDERLINES + 2;
        let converting = self.calc.view.mode == crate::calc::Mode::Convert;
        let scientific = self.calc.view.mode == crate::calc::Mode::Scientific;
        let inset = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
        let (beside, under, used) = if converting {
            // THE CONVERTER PREFERS `convertCells`, and the history takes
            // the rest of the window beside it.
            let beside = columns >= least + gap + least;
            let left_room = if beside {
                tokens::space::CALC_CONVERTCELLS.min(columns - gap - least)
            } else {
                columns
            };
            let mut cy = y;
            for l in self.calc.view.convert_lines(left_room, &texts) {
                laid.lines.push((cy, x0, l, f.x_of(x0 + left_room)));
                cy += 1;
            }
            cy += 1;
            let units = self.calc.view.unit_lines(left_room, &texts);
            let room = bottom.saturating_sub(cy);
            // THE LIST TAKES THE WINDOW'S HEIGHT, less what the history keeps
            // when it is under.
            let rows = if beside {
                room
            } else {
                room.saturating_sub(under_reserve)
            }
            .max(1);
            let offset = self
                .unit_offset
                .min(units.len().saturating_sub(usize::from(rows)));
            let right = f.x_of(x0 + left_room);
            for (k, l) in units
                .iter()
                .skip(offset)
                .take(usize::from(rows))
                .enumerate()
            {
                let line = cy + u16::try_from(k).unwrap_or(0);
                laid.lines.push((line, x0, l.clone(), right - f.cell()));
            }
            let listed = ListBox {
                first: cy,
                rows,
                total: units.len(),
                offset,
                right,
            };
            let track = (units.len() > usize::from(rows)).then(|| {
                let scroll = crate::chrome::list_scroll(units.len(), usize::from(rows), offset);
                let top = f.y_of(cy);
                crate::chrome::band_bar(
                    scroll,
                    right,
                    top,
                    top + f32::from(rows) * f.line(),
                    f.cell(),
                    f.line(),
                )
                .0
            });
            laid.units = Some((listed, track));
            (beside, cy + rows + 1, left_room)
        } else {
            // BESIDE only when the keypad keeps the keys it prefers: a
            // history bought with keys too narrow for their legends is a bad
            // trade. Beside, the keys grow into what the history's preferred
            // width leaves; under, they keep their preferred height and the
            // history has the rest of the window.
            let aside = columns.saturating_sub(inset + gap + least);
            let arrangement = crate::calc_keypad::window_arrangement(scientific, aside);
            let prefers = tokens::space::CALC_KEYCELLS.max(crate::calc_keypad::window_least(
                arrangement.has_functions(),
            ));
            let beside = arrangement != crate::calc_keypad::Arrangement::Narrow
                && columns >= inset + arrangement.columns() * prefers + gap + least;
            let (arrangement, key_room, room_lines) = if beside {
                let keeps = tokens::space::CALC_HISTORYCELLS
                    .min(columns.saturating_sub(inset + gap + arrangement.columns() * prefers))
                    .max(least);
                (
                    arrangement,
                    columns.saturating_sub(inset + gap + keeps),
                    bottom.saturating_sub(y),
                )
            } else {
                let arrangement = crate::calc_keypad::window_arrangement(
                    scientific,
                    columns.saturating_sub(inset),
                );
                (arrangement, columns.saturating_sub(inset), 0)
            };
            // A KEYPAD WITHOUT THE FUNCTIONS' KEYS says why, over the keys.
            if scientific && arrangement == crate::calc_keypad::Arrangement::Narrow {
                for l in crate::calc::View::narrow_lines(columns, &texts) {
                    laid.lines.push((y, x0, l, f.width));
                    y += 1;
                }
                y += 1;
            }
            let room_lines = if beside {
                room_lines
            } else {
                bottom
                    .saturating_sub(y)
                    .saturating_sub(under_reserve)
                    .min(arrangement.rows() * tokens::space::CALC_KEYLINES)
            };
            // FROM THE TEXT'S INSET, as the lines over it are.
            laid.keypad =
                crate::calc_keypad::keypad(arrangement, x0 + inset, y, key_room, room_lines);
            (beside, y + laid.keypad.lines + 1, inset + laid.keypad.cells)
        };
        // THE HISTORY: beside, from where the left part ENDS to the window's
        // edge; or under it, the window's width — its bar at the window's
        // edge either way.
        let (hx, hy, hw) = if beside {
            (x0 + used + gap, y, columns.saturating_sub(used + gap))
        } else {
            (x0, under, columns)
        };
        let lines = self.calc.view.history_lines(hw, &texts);
        let right = f.x_of(hx + hw).min(f.width);
        if let Some((title, entries)) = lines.split_first()
            && hy < bottom
        {
            laid.lines.push((hy, hx, title.clone(), right - f.cell()));
            let first = hy + 1;
            let rows = bottom.saturating_sub(first);
            let offset = self
                .history_offset
                .min(entries.len().saturating_sub(usize::from(rows)));
            for (k, l) in entries
                .iter()
                .skip(offset)
                .take(usize::from(rows))
                .enumerate()
            {
                let line = first + u16::try_from(k).unwrap_or(0);
                laid.lines.push((line, hx, l.clone(), right - f.cell()));
            }
            laid.history = Some(ListBox {
                first,
                rows,
                total: entries.len(),
                offset,
                right,
            });
        }
        laid
    }

    /// Whether the point is on a bar's track — the bar's column, which no
    /// row answers while the bar is drawn.
    fn on_a_bar(&self, ui: &Ui, x: f64, y: f64) -> bool {
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let (x, y, h) = (x as f32, y as f32, ui.height as f32);
        let history = self.history_scroll().is_some_and(|(scroll, first, right)| {
            crate::chrome::on_the_list_bar(scroll, first, right, h, x, y)
        });
        let units = self
            .laid
            .units
            .and_then(|(_, t)| t)
            .is_some_and(|t| crate::bars::Track::of(t).holds(f64::from(x), f64::from(y)));
        history || units
    }

    /// Which line and which cell of it the point is on, as laid out — none
    /// on a bar's column.
    fn line_at(&self, ui: &Ui, x: f64, y: f64) -> Option<(usize, u16)> {
        if self.on_a_bar(ui, x, y) {
            return None;
        }
        let f = ui.frame(0);
        #[allow(clippy::cast_possible_truncation)]
        let (x, y) = (x as f32, y as f32);
        let line = f.line_at(y)?;
        let column = f.column_at(x);
        // THE LINE THAT BEGINS NEAREST on the left: a list beside another
        // shares its surface lines, and a press on the right one is its own.
        self.laid
            .lines
            .iter()
            .enumerate()
            .filter(|(_, (l, x0, _, _))| *l == line && column >= *x0)
            .max_by_key(|(_, (_, x0, _, _))| *x0)
            .map(|(i, (_, x0, _, _))| (i, column - x0))
    }

    /// What a press at the point reaches: a card first — its ground is what
    /// was painted — then the band of a line, the one the pointer lit.
    fn target_at(&self, ui: &Ui, x: f64, y: f64) -> Option<Target> {
        #[allow(clippy::cast_possible_truncation)]
        if let Some(b) = self.laid.keypad.at(ui.frame(0), x as f32, y as f32) {
            return Some(Target::Button(b));
        }
        let (i, cell) = self.line_at(ui, x, y)?;
        self.laid.lines[i].2.target_at(cell)
    }

    /// The card under the pointer, lit on its own ground.
    fn card_lit(&self, ui: &Ui) -> Option<Button> {
        let (x, y) = self.pointer_at?;
        #[allow(clippy::cast_possible_truncation)]
        self.laid.keypad.at(ui.frame(0), x as f32, y as f32)
    }

    /// The history's bar: its scroll, its first line and its right edge —
    /// one answer for where it is drawn and where it is grabbed.
    fn history_scroll(&self) -> Option<(ronoctua_design::Scroll, u16, f32)> {
        self.laid.history.map(|h| {
            (
                crate::chrome::list_scroll(h.total, usize::from(h.rows), h.offset),
                h.first,
                h.right,
            )
        })
    }

    /// Where a press or a drag at height `y` puts the converter's list —
    /// from the track the bar was drawn in.
    fn unit_offset_at(&self, ui: &Ui, y: f64) -> Option<usize> {
        let (units, track) = self.laid.units?;
        let (_, top, _, tall) = track?;
        let scroll = crate::chrome::list_scroll(units.total, usize::from(units.rows), units.offset);
        #[allow(clippy::cast_possible_truncation)]
        Some(scroll.offset_at(y as f32 - top, tall, ui.frame(0).line()))
    }

    /// One pointer's frame, with the header's drawer hand swapped in.
    #[allow(clippy::too_many_lines)]
    fn pointer_here(&mut self, ui: &mut Ui, hand: &mut Hand<bool>, events: &[PointerEvent]) {
        let head = ui.frame(0);
        #[allow(clippy::cast_precision_loss)]
        let h = ui.height as f32;
        for e in events {
            let (x, y) = e.position;
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if ui.hinted.dragged(head, y) {
                        ui.ask_paint();
                        continue;
                    }
                    let texts = ui.texts.clone();
                    let now = ui
                        .hinted
                        .under(x, y)
                        .and_then(|id| crate::chrome::hint_sentence(id, &texts));
                    if ui.hinted.say(now) {
                        ui.ask_paint();
                    }
                    // A HELD BAR follows the hand, whichever list it is.
                    if hand.extra
                        && let Some(at) = self.unit_offset_at(ui, y)
                    {
                        self.unit_offset = at;
                        ui.ask_paint();
                        continue;
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    if hand.dragging
                        && let Some((scroll, first, right)) = self.history_scroll()
                    {
                        self.history_offset =
                            crate::chrome::list_offset_at(scroll, first, right, h, y as f32);
                        ui.ask_paint();
                        continue;
                    }
                    let before = self.hovered(ui);
                    self.pointer_at = Some((x, y));
                    if before != self.hovered(ui) {
                        ui.ask_paint();
                    }
                }
                PointerEventKind::Leave { .. } => {
                    ui.hinted.let_go();
                    let _ = ui.hinted.say(None);
                    hand.dragging = false;
                    hand.extra = false;
                    self.pointer_at = None;
                    ui.ask_paint();
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    ui.hinted.let_go();
                    hand.dragging = false;
                    hand.extra = false;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if ui.hinted.pressed(head, x, y) {
                        ui.ask_paint();
                        continue;
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    if let Some((scroll, first, right)) = self.history_scroll()
                        && crate::chrome::on_the_list_bar(
                            scroll, first, right, h, x as f32, y as f32,
                        )
                    {
                        hand.dragging = true;
                        self.history_offset =
                            crate::chrome::list_offset_at(scroll, first, right, h, y as f32);
                        ui.ask_paint();
                        continue;
                    }
                    // THE CONVERTER'S BAR, pressed where it was drawn, and
                    // held: the list follows the hand until the release.
                    if let Some((_, Some(track))) = self.laid.units
                        && crate::bars::Track::of(track).holds(x, y)
                    {
                        hand.extra = true;
                        if let Some(at) = self.unit_offset_at(ui, y) {
                            self.unit_offset = at;
                        }
                        ui.ask_paint();
                        continue;
                    }
                    self.pointer_at = Some((x, y));
                    if let Some(target) = self.target_at(ui, x, y) {
                        self.clicked_on(ui, target);
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::wheel::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
                    let by = crate::wheel::wheel_lines(&mut hand.lines_owed);
                    if by == 0 {
                        continue;
                    }
                    // A WHEEL OVER THE OPEN DRAWER WALKS THE DRAWER.
                    if ui.hinted.turned(head, y, by) {
                        ui.ask_paint();
                        continue;
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    let line = head.line_at(y as f32).unwrap_or(0);
                    let moved = |offset: usize, rows: u16, total: usize| {
                        usize::try_from(
                            i64::try_from(offset).unwrap_or(0).saturating_add(by).max(0),
                        )
                        .unwrap_or(0)
                        .min(total.saturating_sub(usize::from(rows)))
                    };
                    // OVER THE CONVERTER'S LIST, the wheel scrolls it — the
                    // window, not the cursor.
                    if let Some((units, _)) = self.laid.units
                        && line >= units.first
                        && line < units.first + units.rows
                    {
                        self.unit_offset = moved(units.offset, units.rows, units.total);
                        ui.ask_paint();
                        continue;
                    }
                    // ANYWHERE ELSE, it scrolls the history — the window's list.
                    if let Some(hb) = self.laid.history {
                        self.history_offset = moved(self.history_offset, hb.rows, hb.total);
                        ui.ask_paint();
                    }
                }
                _ => {}
            }
        }
    }

    /// What the pointer lights: a card, or the band of a line — the cells a
    /// press there takes — on every line of its control.
    fn hovered(&self, ui: &Ui) -> Option<Lit> {
        let (x, y) = self.pointer_at?;
        let card = self.card_lit(ui);
        let lines: Vec<(usize, (u16, u16))> = self
            .line_at(ui, x, y)
            .and_then(|(i, cell)| self.laid.lines[i].2.band_at(cell).map(|b| (i, b)))
            .map(|(i, band)| self.one_control(i).into_iter().map(|k| (k, band)).collect())
            .unwrap_or_default();
        Some((card, lines))
    }

    /// The laid lines that are one control with laid line `i`: those that
    /// follow one another on the surface and go on the same control
    /// ([`Line::continues`], the rule the footer's region lights and clicks
    /// by — `Line::control_of`) — every line of the consent's row, of a
    /// failed fetch's sentence. A press on any of them takes the same thing,
    /// and a line of it lighting alone said the others were something else.
    fn one_control(&self, i: usize) -> Vec<usize> {
        let laid = &self.laid.lines;
        let goes_on = |k: usize| {
            k > 0
                && k < laid.len()
                && laid[k - 1].0 + 1 == laid[k].0
                && laid[k].2.continues(&laid[k - 1].2)
        };
        let mut first = i;
        while goes_on(first) {
            first -= 1;
        }
        let mut last = i;
        while goes_on(last + 1) {
            last += 1;
        }
        (first..=last).collect()
    }
}

impl Content for CalcWindow {
    /// Whether this pointer holds the converter's bar.
    type Hand = bool;

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t(crate::key::CALC_TITLE)
    }

    /// NARROWER THAN IT IS TALL, a keypad's shape — before the compositor
    /// says a size (the rule that floats it says one at once).
    fn default_size(&self) -> (u32, u32) {
        (
            u32::from(ronoctua_tokens::space::WINDOW_CALCWIDTHPX),
            u32::from(ronoctua_tokens::space::WINDOW_CALCHEIGHTPX),
        )
    }

    /// A key, read by its whole notation through the reader the footer calls
    /// too (`calc::press_of_key`).
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::calc::press_of_key)
        {
            self.pressed(ui, press);
        }
    }

    /// The clipboard's text, onto the expression line.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        let left = self.calc.pasted(text, ui);
        self.after(ui, left);
    }

    /// A thread of the calculator's woke the loop: the rates came, or the
    /// file changed under it.
    fn woken(&mut self, ui: &mut Ui) {
        if self.calc.woken(ui) {
            ui.ask_paint();
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        crate::calc::hint(
            self.calc.view.taught(ui.texts.decimal_mark()),
            true,
            &ui.texts,
        )
    }

    /// NO SWEEP OF TEXT: a held button here is a press of a card, and a sweep
    /// would take its motion away.
    fn text_area(&self, _: &Ui, _: u16, _: u16) -> Option<crate::text_select::Area> {
        None
    }

    #[allow(clippy::too_many_lines)]
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let texts = ui.texts.clone();
        let hint = self.hint(ui);
        let title = texts.t(crate::key::CALC_TITLE);
        crate::door::list::header_of(
            ui,
            canvas,
            Some(crate::chrome::Title {
                glyph: Some(GLYPH.as_str()),
                label: None,
                name: Some(&title),
            }),
            |_| hint,
        );
        self.laid = self.lay_out(ui);
        let f = ui.frame(0);
        let size = tokens::text::size();
        let (cell, line) = (f.cell(), f.line());
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (ui.width as f32, ui.height as f32);
        let (lit_card, lit_lines) = self.hovered(ui).unwrap_or_default();

        // THE KEYPAD: a ground per card — the one a press is tested against
        // (`Card::rect`) — and its legend in the middle of it, both ways,
        // inside the ground (`calc_keypad::legend_of`).
        for card in &self.laid.keypad.cards {
            let (x, y, width, height) = card.rect(f);
            let color = if lit_card == Some(card.button) {
                tokens::color::SURFACE_KEY_HOVER
            } else if card.button.tone() == Tone::Answer {
                tokens::color::SURFACE_KEY_ON
            } else {
                tokens::color::SURFACE_CONTAINER
            };
            canvas.under.push(ronoctua_render::quad::Quad {
                x,
                y,
                width,
                height,
                color,
            });
            let (legend, column) = crate::calc_keypad::legend_of(card, &texts);
            let by = (f32::from(card.lines) - 1.0) * line / 2.0;
            canvas.spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Lowered(by, None),
                text: legend,
                column,
                line: card.line,
                color: tokens::color::TEXT_PRIMARY,
                edge: false,
                foot: false,
            });
        }

        // THE LINES: the head, the converter, the lists.
        for (k, (y, x0, l, stop)) in self.laid.lines.iter().enumerate() {
            // THE POINTER'S MARK IS A BAND, over exactly the cells a press
            // there takes (`Line::band_at`) — a row of a list whole, a field
            // of a line of fields alone — and never a colour of the text: the
            // accent on a mode or a category is where the keyboard is.
            if let Some(&(_, (from, to))) = lit_lines.iter().find(|(i, _)| *i == k) {
                let a = f.x_of(x0 + from);
                let b = f.x_of(x0 + to).min(*stop).min(w);
                canvas.under.push(ronoctua_render::quad::Quad {
                    x: a,
                    y: f.y_of(*y),
                    width: (b - a).max(0.0),
                    height: line,
                    color: tokens::color::SURFACE_HOVER,
                });
            }
            for fld in &l.fields {
                canvas.spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::Px(size),
                    text: fld.text.clone(),
                    column: x0 + fld.at,
                    line: *y,
                    color: fld.color,
                    edge: false,
                    foot: false,
                });
            }
            if let Some(at) = l.caret {
                canvas.over.push(crate::chrome::caret(
                    f.x_of(*x0),
                    f.y_of(*y),
                    cell,
                    line,
                    at,
                ));
            }
        }

        // THE CONVERTER'S BAR, when its list is longer than its box.
        if let Some((units, Some(track))) = self.laid.units {
            let scroll =
                crate::chrome::list_scroll(units.total, usize::from(units.rows), units.offset);
            let (_, thumb) = crate::chrome::band_bar(
                scroll,
                track.0 + track.2,
                track.1,
                track.1 + f32::from(units.rows) * line,
                cell,
                line,
            );
            if let Some(t) = thumb {
                canvas.under.push(crate::chrome::thumb(t));
            }
            canvas.bar(Some(crate::bars::Track::of(track)));
        }

        // THE HISTORY'S BAR, the window's list: drawn, written down for the
        // pointer, and its column is the bar's.
        if let Some((scroll, first, right)) = self.history_scroll() {
            ui.room = crate::chrome::Room::of(scroll, right, h);
            canvas
                .under
                .extend(crate::chrome::list_bar(scroll, first, right, h));
            canvas.bar(crate::chrome::list_track(scroll, first, right, h));
        }
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<bool>, events: &[PointerEvent]) {
        ui.hinted.swap_hand(&mut hand.hint_drag);
        self.pointer_here(ui, hand, events);
        ui.hinted.swap_hand(&mut hand.hint_drag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a pointer on any line of the row that asks for the rates'
    /// host — the host, then what for — lights anything but every line of
    /// it: a press on any of them takes the same thing (`Line::target_at`),
    /// and a line that lit alone said the others were something else. The
    /// footer's region has its own.
    #[test]
    fn the_lines_of_one_press_light_together() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-calc-window-whole-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a room");
        let store = ronoctua_store::Store::at(Some(dir.clone()), crate::calc::FILE);
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut ui = Ui::for_tests(texts);
        // A PHOTOGRAPH'S WINDOW: no consent read or written, no fetch.
        let mut w = CalcWindow::over(store, Reach::Kept);
        for k in [
            "1", "0", "SPC", "U", "S", "D", "SPC", "i", "n", "SPC", "E", "U", "R",
        ] {
            let key = ronoctua_keymap::Key::parse(k).expect("a key");
            w.pressed(&mut ui, crate::calc::press_of_key(key).expect(k));
        }
        w.laid = w.lay_out(&ui);
        let asks: Vec<usize> = (0..w.laid.lines.len())
            .filter(|&k| w.laid.lines[k].2.bands.iter().any(|b| b.2 == Target::Allow))
            .collect();
        assert!(asks.len() >= 2, "the host asked on {asks:?}");
        let f = ui.frame(0);
        for &k in &asks {
            let (y, x0) = (w.laid.lines[k].0, w.laid.lines[k].1);
            w.pointer_at = Some((
                f64::from(f.x_of(x0 + 1) + f.cell() / 2.0),
                f64::from(f.y_of(y) + f.line() / 2.0),
            ));
            let (_, lit) = w.hovered(&ui).expect("a pointer");
            let lit: Vec<usize> = lit.into_iter().map(|(n, _)| n).collect();
            assert_eq!(lit, asks, "the pointer on laid line {k}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
