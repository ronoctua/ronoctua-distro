//! THE EMACS REFERENCE CARD, ON THE WHOLE SCREEN — `ronoctua-shell manual
//! emacs`, which `LEADER e` and the card's row of `LEADER h` open
//! ([`crate::refcard`]).
//!
//! The card is TEXT, in the system's face: its sections flow into columns of
//! `space.emacsCard.columnCells`, as many side by side as the width holds,
//! centred, from under the header to above the line of the bar; the rest is
//! reached by `←`/`→`, the page keys, the wheel and the bar under them. What
//! is typed narrows the card to what says it (`DEL` takes a letter back,
//! `C-v`/`C-y` paste, and `Esc` clears it before it closes anything). It
//! asks the compositor for the whole screen itself, as it opens — the card is
//! read at a glance or not at all.

use crate::door::{Canvas, Content, Shell, Ui, Wiring};
use crate::refcard::{Card as Read, Missing, Tone};
use anyhow::{Context as _, Result};
use ronoctua_keymap::Sym;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::reexports::client::protocol::wl_surface;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Modifiers};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};
use smithay_client_toolkit::shell::WaylandSurface as _;
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::xdg::window::WindowDecorations;

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.emacs-card";

/// The keys the window answers to, as its header teaches them.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<left>", "emacs-card-back"),
    ("<right>", "emacs-card-forward"),
];

/// Opens the card on the whole screen.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|_| Ok(Card::default()))
}

/// THE CARD'S CAMERA (`photograph --door emacs-card --scene <folder>`): the
/// window over a card as `nix/emacs-refcard.nix` reads it — the folder of
/// its languages.
pub fn camera(rest: &[String]) -> ! {
    crate::photo::planted::taken(
        rest,
        "emacs-card",
        "<the card's folder, as nix/emacs-refcard.nix reads it>",
        |asked| {
            let card = Card {
                root: Some(std::path::PathBuf::from(&asked.scene)),
                ..Card::default()
            };
            crate::photo::planted::shoot(asked, card, None)
        },
    )
}

/// The card's window: the card read, what was typed, and which column comes
/// first.
#[derive(Default)]
struct Card {
    /// Where the card is, when it is not where the system put it — a
    /// photograph's (`camera`).
    root: Option<std::path::PathBuf>,
    /// The card read, and the language it was read in: read again when the
    /// language changes, never per frame.
    read: Option<(String, Result<Read, Missing>)>,
    /// What has been typed, which narrows the card.
    query: String,
    /// The first column shown.
    first: usize,
    /// How many columns the last frame showed, which a page key moves by.
    shown: usize,
    /// How many columns the card has, at the last frame's size.
    total: usize,
    /// The bar under the columns, while there is one: its track and what it
    /// was drawn for.
    bar: Option<(crate::bars::Track, ronoctua_design::Scroll)>,
    /// A press on the bar is dragging it.
    dragging: bool,
    /// The wheel's fraction of a column, kept.
    wheel_left: f64,
}

/// The first column after moving `by` from `first`, held inside a card of
/// `columns` shown `shown` at a time.
fn moved(first: usize, by: i64, columns: usize, shown: usize) -> usize {
    let last = columns.saturating_sub(shown.max(1));
    let to = i64::try_from(first)
        .unwrap_or(0)
        .saturating_add(by)
        .clamp(0, i64::try_from(last).unwrap_or(0));
    usize::try_from(to).unwrap_or(0)
}

/// How many columns of `column` cells, `gap` apart, `room` cells hold — one
/// at least, and the column itself narrowed to the room when even one does
/// not fit.
fn fit(room: u16, column: u16, gap: u16) -> (usize, u16) {
    let column = column.min(room.max(1));
    let n = (usize::from(room) + usize::from(gap)) / (usize::from(column) + usize::from(gap));
    (n.max(1), column)
}

/// The ink of a piece of the card.
const fn ink(tone: Tone) -> tokens::Color {
    match tone {
        Tone::Heading => tokens::color::TEXT_HEADING,
        Tone::Quiet | Tone::Aside => tokens::color::TEXT_DIM,
        Tone::Does | Tone::Code => tokens::color::TEXT_SECONDARY,
        Tone::Strong => tokens::color::TEXT_PRIMARY,
        Tone::Key => tokens::color::TEXT_KEY,
    }
}

impl Card {
    /// The card, in the language of now — read the first time it is asked
    /// for in that language.
    fn card(&mut self, ui: &Ui) -> Result<&Read, &Missing> {
        let language = ui.texts.language().to_owned();
        if self.read.as_ref().is_none_or(|(l, _)| *l != language) {
            let read = self.root.as_deref().map_or_else(
                || crate::refcard::card(&language),
                |root| crate::refcard::card_in(root, &language),
            );
            self.read = Some((language, read));
        }
        match &self.read {
            Some((_, Ok(card))) => Ok(card),
            Some((_, Err(why))) => Err(why),
            None => Err(&Missing::Absent),
        }
    }

    /// Moves the first column by `by`, held inside the card.
    fn go(&mut self, ui: &mut Ui, by: i64) {
        let to = moved(self.first, by, self.total, self.shown);
        if to != self.first {
            self.first = to;
            ui.ask_paint();
        }
    }

    /// What was typed changed: the card is narrowed again, from its start.
    fn narrowed(&mut self, ui: &mut Ui) {
        self.first = 0;
        ui.ask_paint();
    }

    /// The bar, dragged to `x`: the first column under it.
    fn drag_to(&mut self, ui: &mut Ui, x: f64) {
        let Some((track, scroll)) = self.bar else {
            return;
        };
        #[allow(clippy::cast_possible_truncation)]
        let at = (x as f32) - track.x;
        let to = scroll.offset_at(at, track.width, crate::chrome::cell_width());
        if to != self.first {
            self.first = to;
            ui.ask_paint();
        }
    }

    /// A line of the window, in one ink, from `column`.
    fn say(canvas: &mut Canvas<'_>, text: String, column: u16, line: u16, color: tokens::Color) {
        canvas.spans.push(ronoctua_render::Span {
            size: ronoctua_render::Size::Px(tokens::text::size()),
            text,
            column,
            line,
            color,
            edge: false,
            foot: false,
        });
    }
}

impl Content for Card {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        texts.t(crate::key::MANUAL_EMACS)
    }

    /// A window of its own, asked for the whole screen before its first
    /// commit — the compositor's own way to open one there.
    fn open(&self, wiring: &Wiring<'_, Self>, surface: wl_surface::WlSurface) -> Result<Shell> {
        let xdg = XdgShell::bind(wiring.globals, wiring.qh)
            .context("the compositor does not expose xdg_shell")?;
        let window = xdg.create_window(surface, WindowDecorations::RequestServer, wiring.qh);
        window.set_title(self.title(&wiring.texts));
        window.set_app_id(APP_ID);
        window.set_fullscreen(None);
        window.commit();
        Ok(Shell::Window {
            window,
            _global: xdg,
        })
    }

    /// The arrows and the page keys walk the columns; a letter narrows the
    /// card, `DEL` takes one back, the paste keys paste, and `Esc` clears
    /// what was typed before it closes the window.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        let page = i64::try_from(self.shown.max(1)).unwrap_or(1);
        let chord = key.mods.ctrl || key.mods.alt || key.mods.logo;
        if crate::line::pastes(key) {
            crate::region::Doorway::paste(ui);
            return;
        }
        match key.sym {
            Sym::Esc if !self.query.is_empty() => {
                self.query.clear();
                self.narrowed(ui);
            }
            Sym::Esc => crate::region::Doorway::close(ui),
            Sym::Left => self.go(ui, -1),
            Sym::Right => self.go(ui, 1),
            Sym::PageUp => self.go(ui, -page),
            Sym::PageDown => self.go(ui, page),
            Sym::Home => self.go(ui, i64::MIN / 2),
            Sym::End => self.go(ui, i64::MAX / 2),
            Sym::Backspace if crate::line::pop_cluster(&mut self.query) => self.narrowed(ui),
            _ if chord => {}
            Sym::Space => {
                self.query.push(' ');
                self.narrowed(ui);
            }
            Sym::Char(c) => {
                self.query.push(c);
                self.narrowed(ui);
            }
            _ => {}
        }
    }

    /// What was copied, into what narrows the card — as one line.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        self.query.push_str(&crate::line::one_line(text));
        self.narrowed(ui);
    }

    /// The way back, then the two arrows — every one a click.
    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        crate::chrome::with_the_way_back(crate::chrome::hint_of(OF_THE_VIEW, &ui.texts), &ui.texts)
    }

    /// The card's columns, side by side under the line of what was typed; a
    /// sentence where there is no card, or nothing on it says what was.
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let label = ui.texts.t(crate::key::MANUAL_WINDOW_LEAD);
        let name = ui.texts.t(crate::key::MANUAL_EMACS);
        let hint = self.hint(ui);
        crate::door::list::header_of(
            ui,
            canvas,
            Some(crate::chrome::Title {
                glyph: None,
                label: Some(&label),
                name: Some(&name),
            }),
            |_| hint,
        );
        self.bar = None;
        let first_line = ui.first_of_list();
        let margin = ui.margin;
        let room = ui.columns.saturating_sub(2 * margin);
        let top = first_line + 2;
        // THE COLUMNS' HEIGHT: from under the line of what was typed to above
        // the bar's.
        let height = ui.lines.saturating_sub(top + 1);
        let (fits, width) = fit(
            room,
            tokens::space::EMACSCARD_COLUMNCELLS,
            tokens::space::EMACSCARD_GAPCELLS,
        );
        let gap = tokens::space::EMACSCARD_GAPCELLS;
        // WHERE THE COLUMNS START, centred — and the line of what was typed
        // with them, over the first.
        let across = |shown: usize| {
            #[allow(clippy::cast_possible_truncation)]
            let shown = shown as u16;
            shown * width + shown.saturating_sub(1) * gap
        };
        let left_of = |shown: usize| margin + room.saturating_sub(across(shown)) / 2;
        // THE LINE OF WHAT WAS TYPED: how to narrow, before anything is.
        let lead = left_of(fits);
        if self.query.is_empty() {
            let how = ui.texts.t(crate::key::EMACS_CARD_FILTER_EMPTY);
            Self::say(canvas, how, lead, first_line, tokens::color::TEXT_DIM);
        } else {
            let typed = ronoctua_render::layout::elide_start(&self.query, room);
            Self::say(
                canvas,
                typed,
                lead,
                first_line,
                tokens::color::QUESTION_TYPED,
            );
        }
        let query = self.query.clone();
        let (columns, any) = match self.card(ui) {
            Ok(card) => (
                crate::refcard::columns(card, width, height, &query),
                crate::refcard::says(card, &query),
            ),
            Err(why) => {
                let said = match why {
                    Missing::Absent => ui.texts.t(crate::key::MANUAL_EMACS_MISSING),
                    // A CARD THAT IS THERE AND UNREADABLE says why, in the
                    // reader's own words: a file and its fault.
                    Missing::Unreadable(why) => why.clone(),
                };
                Self::say(canvas, said, lead, top, tokens::color::TEXT_SECONDARY);
                self.total = 0;
                return;
            }
        };
        if !any {
            let none = ui.texts.t(crate::key::EMACS_CARD_NONE);
            Self::say(canvas, none, lead, top, tokens::color::TEXT_DIM);
            self.total = 0;
            return;
        }
        let shown = fits.min(columns.len());
        self.total = columns.len();
        self.shown = shown;
        self.first = self.first.min(columns.len() - shown);
        let wide = across(shown);
        let left = left_of(fits);
        for (k, column) in columns.iter().skip(self.first).take(shown).enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let x = left + k as u16 * (width + gap);
            for (row, line) in column.iter().enumerate() {
                #[allow(clippy::cast_possible_truncation)]
                let y = top + row as u16;
                for piece in line {
                    Self::say(canvas, piece.text.clone(), x + piece.at, y, ink(piece.tone));
                }
            }
        }
        // AND THE BAR UNDER THEM, while there are more columns than shown —
        // a control, grabbed where it is drawn (`crate::bars`).
        let scroll = ronoctua_design::Scroll {
            total: columns.len(),
            lines: shown,
            offset: self.first,
            top: 0,
        };
        if scroll.cut() {
            let cell = crate::chrome::cell_width();
            let line = tokens::metrics::line_height(tokens::text::size());
            let band_bottom = line * f32::from(ui.lines);
            let from = cell * f32::from(left);
            let across = cell * f32::from(wide);
            if let Some((x, y, w, h)) = scroll.bar_across(from, band_bottom, across, cell) {
                let track = crate::bars::Track::of((from, band_bottom - cell, across, cell));
                canvas.bar(Some(track));
                canvas.over.push(ronoctua_render::quad::Quad {
                    x,
                    y,
                    width: w,
                    height: h,
                    color: tokens::color::RULE_STRONG,
                });
                self.bar = Some((track, scroll));
            }
        }
    }

    /// The wheel moves a column a notch, and the bar is grabbed where it is.
    fn pointer(&mut self, ui: &mut Ui, _hand: &mut crate::door::Hand<()>, events: &[PointerEvent]) {
        for e in events {
            // THE HEADER'S HALF FIRST, as on every window: its drawer dragged
            // or its control pressed, and what the shortcut under the pointer
            // says (`door::list::header_pointer`).
            let (took, said) = crate::door::list::header_pointer(self, ui, e);
            if took || said {
                ui.ask_paint();
            }
            if took {
                continue;
            }
            let (x, y) = e.position;
            match e.kind {
                PointerEventKind::Press { button, .. } if button == crate::door::LEFT_BUTTON => {
                    if self.bar.is_some_and(|(track, _)| track.holds(x, y)) {
                        self.dragging = true;
                        self.drag_to(ui, x);
                    }
                }
                PointerEventKind::Motion { .. } if self.dragging => self.drag_to(ui, x),
                PointerEventKind::Release { .. } | PointerEventKind::Leave { .. } => {
                    self.dragging = false;
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    crate::wheel::wheel_arrived(&mut self.wheel_left, &vertical, &horizontal);
                    let by = crate::wheel::wheel_lines(&mut self.wheel_left);
                    if by != 0 {
                        self.go(ui, by);
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the arrows walk past either end of the card, or a page key
    /// does not move by the columns shown.
    #[test]
    fn the_arrows_and_the_page_keys_walk_the_card_inside_its_ends() {
        // SIX COLUMNS, THREE SHOWN: the first column goes from 0 to 3.
        assert_eq!(moved(0, 1, 6, 3), 1);
        assert_eq!(moved(2, 3, 6, 3), 3, "past the end");
        assert_eq!(moved(1, -5, 6, 3), 0, "past the start");
        assert_eq!(moved(0, i64::MAX / 2, 6, 3), 3, "to the end");
        // ALL OF IT SHOWN, or no card: nothing to move.
        assert_eq!(moved(0, 1, 6, 6), 0);
        assert_eq!(moved(0, 1, 0, 0), 0);
    }

    /// FAILS IF: the width does not hold as many columns as fit with their
    /// gaps, or a screen narrower than one column shows none.
    #[test]
    fn the_width_holds_as_many_columns_as_fit() {
        assert_eq!(fit(150, 46, 3), (3, 46));
        assert_eq!(
            fit(144, 46, 3),
            (3, 46),
            "three columns and two gaps are 144"
        );
        assert_eq!(fit(143, 46, 3), (2, 46));
        assert_eq!(
            fit(30, 46, 3),
            (1, 30),
            "a narrow screen narrows its one column"
        );
    }
}
