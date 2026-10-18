//! The icons app in a window of its own — the second door onto
//! `crate::icons` (`LEADER m i w`, `ronoctua-shell icons [screen]`).
//!
//! A window and nothing else: which icons there are, what a key does and
//! where everything is drawn is `icons.rs` and `icons_page.rs`, and both doors
//! ask the same view the same questions. What is here is the door's half: the
//! tables read on a thread, the header — the tabs and the system's shortcuts
//! on its one line; what is typed is the view's own search line under it —
//! the pictures put on the GPU, and the
//! pointer, whose hover is kept per hand as a target's code so each cursor
//! lights its own cell.

use std::sync::Arc;

use crate::door::{Canvas, Content, Door, Hand, LEFT_BUTTON, Ui};
use crate::icons::{Area, Errand, Target, View};
use anyhow::Result;
use ronoctua_iconset::{Collection, Refusal, Set};
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::keyboard::{KeyEvent, Modifiers},
    seat::pointer::{PointerEvent, PointerEventKind},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.icons";

/// The glyph in front of the header — the app's token.
static GLYPH: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| ronoctua_tokens::icon::ICONS_APP.to_string());

/// Opens the icons app in a window, on `screen`.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it(screen: Collection) -> Result<()> {
    crate::door::run(move |w| {
        // THE TABLES, READ ON A THREAD — ~9 MB is never read on a loop — and
        // handed in when they arrive; until then the view says it is reading.
        let (sender, arrived) = channel::channel::<Result<Arc<Set>, Refusal>>();
        w.handle
            .insert_source(arrived, |event, (), door: &mut Door<IconsWindow>| {
                if let channel::Event::Msg(answer) = event {
                    match &answer {
                        Ok(set) => tracing::info!(
                            glyphs = set.nerd.len(),
                            figures = set.lucide.len(),
                            characters = set.unicode.len(),
                            "icons: the tables arrived"
                        ),
                        Err(why) => tracing::warn!(?why, "icons: the tables were refused"),
                    }
                    door.content.view.took(answer);
                    door.ui.ask_paint();
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the icon tables' channel: {e}"))?;
        crate::icons::read_the_tables(move |answer| {
            let _ = sender.send(answer);
        });
        Ok(IconsWindow::over(View::new(screen, &w.texts)))
    })
}

/// The window: the view.
pub struct IconsWindow {
    view: View,
}

impl IconsWindow {
    /// The window over `view` — what the photograph draws, its tables handed
    /// in already.
    #[must_use]
    pub const fn over(view: View) -> Self {
        Self { view }
    }

    /// Where the view is drawn: from the line under the header — its drawer
    /// and its sentence included — to the window's bottom.
    #[allow(clippy::cast_precision_loss)]
    fn area(ui: &Ui) -> Area {
        let top = ui.first_of_list();
        Area {
            left: ui.margin,
            top,
            columns: ui.columns,
            lines: ui.lines.saturating_sub(top),
            right_px: ui.width as f32,
            bottom_px: ui.height as f32,
        }
    }

    /// What the view asked, done in this door by the errand handler the
    /// footer's region shares — a copy said on THIS window's line, a
    /// question asked on its header.
    fn apply(ui: &mut Ui, errand: Errand) {
        crate::icons_region::carry_out(errand, ui);
    }

    /// One pointer's frame: the header's controls first — its chevron, its
    /// drawer's bar and wheel, the sentence under a key — then a bar held,
    /// then a cell or a row, then the wheel.
    fn pointer_here(&mut self, ui: &mut Ui, hand: &mut Hand<()>, events: &[PointerEvent]) {
        let before = (
            hand.row_under_pointer,
            self.view.cursor(),
            self.view.in_page(),
        );
        let area = Self::area(ui);
        let texts = ui.texts.clone();
        let head = ui.frame(0);
        let mut header = false;
        let mut explained = false;
        let mut moved = false;
        let mut turned = false;
        let mut wheel_at: Option<f64> = None;
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (px, py) = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if ui.hinted.dragged(head, e.position.1) {
                        header = true;
                        continue;
                    }
                    let now = ui
                        .hinted
                        .under(e.position.0, e.position.1)
                        .and_then(|id| crate::chrome::hint_sentence(id, &texts));
                    explained |= ui.hinted.say(now);
                    if self.view.grabbed.is_some() {
                        self.view.dragged(area, py, &texts);
                        moved = true;
                    } else {
                        let target = self.view.target_at(area, &texts, px, py);
                        hand.row_under_pointer = target.map(Target::code);
                        // THE SEARCH LINE NAMES THE ICON UNDER THE HAND THAT
                        // MOVED LAST — until a key moves the cursor.
                        explained |= self.view.hovered(target);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    ui.hinted.let_go();
                    explained |= ui.hinted.say(None);
                    self.view.grabbed = None;
                    explained |= self.view.hovered(None);
                    hand.row_under_pointer = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    ui.hinted.let_go();
                    self.view.grabbed = None;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if ui.hinted.pressed(head, e.position.0, e.position.1) {
                        header = true;
                        continue;
                    }
                    if let Some(t) = self.view.target_at(area, &texts, px, py) {
                        let errand = self.view.pressed(t, area, py, &texts);
                        Self::apply(ui, errand);
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::wheel::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
                    wheel_at = Some(e.position.1);
                    turned = true;
                }
                _ => {}
            }
        }
        if header {
            ui.ask_paint();
            return;
        }
        if turned {
            let by = crate::wheel::wheel_lines(&mut hand.lines_owed);
            // A WHEEL OVER THE OPEN DRAWER WALKS THE DRAWER.
            if by != 0 && wheel_at.is_some_and(|y| ui.hinted.turned(head, y, by)) {
                ui.ask_paint();
                return;
            }
            if by != 0 {
                #[allow(clippy::cast_possible_truncation)]
                let y = wheel_at.unwrap_or_default() as f32;
                self.view.wheeled(by, area, y, &texts);
                moved = true;
            }
        }
        if moved
            || explained
            || before
                != (
                    hand.row_under_pointer,
                    self.view.cursor(),
                    self.view.in_page(),
                )
        {
            ui.ask_paint();
        }
    }
}

impl Content for IconsWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::ICONS_TITLE))
    }

    /// A key, read by its whole notation through the reader the footer calls
    /// too (`icons::press_of_key`).
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(press) = crate::door::keys::key_of(event, mods)
            .and_then(|k| crate::icons::press_of_key(k, self.view.in_page()))
        else {
            return;
        };
        ui.message = None;
        let texts = ui.texts.clone();
        let errand = self.view.key(press, Self::area(ui), &texts);
        Self::apply(ui, errand);
    }

    /// The Lucide colour, typed on this window's header.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        if id == crate::icons::COLOUR_COMMAND {
            self.view.colour_answered(text);
        }
        ui.ask_paint();
    }

    /// The clipboard's text, into what is typed.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        tracing::info!(pasted = %text, "icons: the clipboard went into the search");
        let errand = self.view.pasted(text);
        Self::apply(ui, errand);
    }

    /// The header's line: the tabs, then the system's shortcuts — `Esc`
    /// first. What is typed is on the view's search line, under it: on one
    /// line with the tabs it cut the keys off at an ordinary width.
    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        let mut out = self.view.tab_pieces(&ui.texts);
        out.push(crate::chrome::Piece::separator());
        out.extend(self.view.hint(&ui.texts));
        out
    }

    /// No rows: the grid and the page are drawn by the view.
    fn rows(&self, _: &Ui) -> Vec<Vec<crate::fields::Field>> {
        Vec::new()
    }

    /// A held button on the grid is a drag of its bar, never a sweep of text.
    fn text_area(&self, _: &Ui, _: u16, _: u16) -> Option<crate::text_select::Area> {
        None
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let glyph = GLYPH.as_str();
        let view = &self.view;
        crate::door::list::header_of(
            ui,
            canvas,
            Some(crate::chrome::Title {
                glyph: Some(glyph),
                label: None,
                name: None,
            }),
            |ui| {
                let mut out = view.tab_pieces(&ui.texts);
                out.push(crate::chrome::Piece::separator());
                out.extend(view.hint(&ui.texts));
                out
            },
        );
        let area = Self::area(ui);
        // EVERY HAND'S TARGET, in its own colour.
        let lit: Vec<(ronoctua_tokens::Color, Target)> = canvas
            .lit
            .iter()
            .filter_map(|(c, code)| code.map(|n| (*c, Target::of_code(n))))
            .collect();
        let texts = ui.texts.clone();
        let mut drawing = crate::icons::Drawing::default();
        self.view.draw(area, false, &lit, &texts, &mut drawing);
        paint_drawing(drawing, canvas);
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<()>, events: &[PointerEvent]) {
        ui.hinted.swap_hand(&mut hand.hint_drag);
        self.pointer_here(ui, hand, events);
        ui.hinted.swap_hand(&mut hand.hint_drag);
    }
}

/// A [`crate::icons::Drawing`] on a window's canvas: the grounds under, the
/// pictures up when missing, the thumbs over, the text, and every bar's track
/// recorded — its column is the bar's (`crate::bars`).
pub fn paint_drawing(drawing: crate::icons::Drawing, canvas: &mut Canvas<'_>) {
    canvas.under.extend(drawing.under);
    for p in drawing.pictures {
        if canvas.upload(&p.key, || p.rgba()) {
            canvas.pictures.push((p.key, p.area));
        }
    }
    canvas.over.extend(drawing.thumbs);
    for t in drawing.tracks {
        canvas.bar(Some(t));
    }
    canvas.spans.extend(drawing.spans);
}
