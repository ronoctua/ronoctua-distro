//! THE EMACS REFERENCE CARD, ON THE WHOLE SCREEN — `ronoctua-shell manual
//! emacs`, which `LEADER e` and the card's row of `LEADER h` open
//! ([`crate::refcard`]).
//!
//! A page of the card is one of its columns, drawn the screen's height (less
//! the header and a line for the bar), and the window shows as many of them
//! side by side as its width holds, centred; the rest is reached by `←`/`→`,
//! the page keys, the wheel and the bar under them. It asks the compositor for
//! the whole screen itself, as it opens — the card is read at a glance or not
//! at all.

use crate::door::{Canvas, Content, Shell, Ui, Wiring};
use anyhow::{Context as _, Result};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::reexports::client::protocol::wl_surface;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym, Modifiers};
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
/// window over a card as `nix/emacs-refcard.nix` builds it — the folder of
/// its languages.
pub fn camera(rest: &[String]) -> ! {
    crate::photo::planted::taken(
        rest,
        "emacs-card",
        "<the card's folder, as nix/emacs-refcard.nix builds it>",
        |asked| {
            let card = Card {
                root: Some(std::path::PathBuf::from(&asked.scene)),
                ..Card::default()
            };
            crate::photo::planted::shoot(asked, card, None)
        },
    )
}

/// The card's window: its pages, and which one the first column shows.
#[derive(Default)]
struct Card {
    /// Where the card is, when it is not where the system put it — a
    /// photograph's (`camera`).
    root: Option<std::path::PathBuf>,
    /// The first column's page.
    first: usize,
    /// How many columns the last frame laid out, which a page key moves by.
    shown: usize,
    /// The bar under the columns, while there is one: its track and what it
    /// was drawn for.
    bar: Option<(crate::bars::Track, ronoctua_design::Scroll)>,
    /// A press on the bar is dragging it.
    dragging: bool,
    /// The wheel's fraction of a column, kept.
    wheel_left: f64,
    /// The width each drawing went up at, by its key: the columns are laid
    /// out by the page's own proportion.
    widths: std::collections::HashMap<String, u32>,
}

/// The first column after moving `by` from `first`, held inside a card of
/// `pages` shown `shown` at a time.
fn moved(first: usize, by: i64, pages: usize, shown: usize) -> usize {
    let last = pages.saturating_sub(shown.max(1));
    let to = i64::try_from(first)
        .unwrap_or(0)
        .saturating_add(by)
        .clamp(0, i64::try_from(last).unwrap_or(0));
    usize::try_from(to).unwrap_or(0)
}

/// The air between two columns, in cells.
const GAP_CELLS: u16 = 3;

impl Card {
    /// The pages, in the language of now.
    fn pages(&self, ui: &Ui) -> Vec<std::path::PathBuf> {
        let language = ui.texts.language();
        self.root.as_deref().map_or_else(
            || crate::refcard::pages(language),
            |root| crate::refcard::pages_in(root, language),
        )
    }

    /// Moves the first column by `by`, held inside the card.
    fn go(&mut self, ui: &mut Ui, by: i64) {
        let to = moved(self.first, by, self.pages(ui).len(), self.shown);
        if to != self.first {
            self.first = to;
            ui.ask_paint();
        }
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

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, _mods: Modifiers) {
        let page = i64::try_from(self.shown.max(1)).unwrap_or(1);
        match event.keysym {
            Keysym::Escape => crate::region::Doorway::close(ui),
            Keysym::Left => self.go(ui, -1),
            Keysym::Right => self.go(ui, 1),
            Keysym::Page_Up => self.go(ui, -page),
            Keysym::Page_Down | Keysym::space => self.go(ui, page),
            Keysym::Home => self.go(ui, i64::MIN / 2),
            Keysym::End => self.go(ui, i64::MAX / 2),
            _ => {}
        }
    }

    /// The way back, then the two arrows — every one a click.
    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        crate::chrome::with_the_way_back(crate::chrome::hint_of(OF_THE_VIEW, &ui.texts), &ui.texts)
    }

    /// The card's pages, side by side; a sentence where there is none.
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
        let pages = self.pages(ui);
        let size = tokens::text::size();
        let line = tokens::metrics::line_height(size);
        let cell = tokens::metrics::cell_width(size);
        let first_line = ui.first_of_list();
        if pages.is_empty() {
            canvas.spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text: ui.texts.t(crate::key::MANUAL_EMACS_MISSING),
                column: ui.margin,
                line: first_line,
                color: tokens::color::TEXT_SECONDARY,
                edge: false,
                foot: false,
            });
            self.bar = None;
            return;
        }
        // THE HEIGHT: from under the header to above the bar's line.
        #[allow(clippy::cast_precision_loss)]
        let width = ui.width as f32;
        let top = line * f32::from(first_line) + line / 2.0;
        let bottom = line * f32::from(ui.lines.saturating_sub(1));
        let tall = (bottom - top).max(line);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let height_px = (tall * ui.scale.max(1) as f32).round() as u32;
        // THE FIRST PAGE IS THE MEASURE of them all: the card's pages are one
        // size.
        let key = crate::refcard::page_key(&pages[0], height_px);
        let widths = &mut self.widths;
        canvas.upload(&key, || {
            let art = crate::refcard::page_art(&pages[0], height_px)?;
            widths.insert(key.clone(), art.width);
            Some(art)
        });
        let Some(&wide_px) = self.widths.get(&key) else {
            self.bar = None;
            return;
        };
        #[allow(clippy::cast_precision_loss)]
        let page_wide = wide_px as f32 / ui.scale.max(1) as f32;
        let gap = cell * f32::from(GAP_CELLS);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let fit = (((width - 2.0 * cell * f32::from(ui.margin) + gap) / (page_wide + gap)).floor()
            as usize)
            .clamp(1, pages.len());
        self.shown = fit;
        self.first = self.first.min(pages.len() - fit);
        #[allow(clippy::cast_precision_loss)]
        let total = fit as f32 * page_wide + (fit - 1) as f32 * gap;
        let left = ((width - total) / 2.0).max(0.0);
        for (k, page) in pages.iter().skip(self.first).take(fit).enumerate() {
            let key = crate::refcard::page_key(page, height_px);
            canvas.upload(&key, || crate::refcard::page_art(page, height_px));
            #[allow(clippy::cast_precision_loss)]
            let x = left + k as f32 * (page_wide + gap);
            canvas.pictures.push((
                key,
                ronoctua_render::image::Area {
                    x,
                    y: top,
                    width: page_wide,
                    height: tall,
                },
            ));
        }
        // AND THE BAR UNDER THEM, while there are more pages than columns — a
        // control, grabbed where it is drawn (`crate::bars`).
        let scroll = ronoctua_design::Scroll {
            total: pages.len(),
            lines: fit,
            offset: self.first,
            top: 0,
        };
        self.bar = None;
        if scroll.cut() {
            let band_bottom = line * f32::from(ui.lines);
            if let Some((x, y, w, h)) = scroll.bar_across(left, band_bottom, total, cell) {
                let track = crate::bars::Track::of((left, band_bottom - cell, total, cell));
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
}
