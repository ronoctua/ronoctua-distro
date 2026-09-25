//! A manual in a window of its own — the second door onto
//! [`crate::manual`], opened from the list `LEADER h` shows (`C-w` there, or
//! the "open in a window" line of an open manual).
//!
//! It DECIDES nothing: which lines a manual has is `manual.rs`, and both
//! doors draw the same lines. It is `history_window.rs` with the view swapped
//! for a manual, deliberately: eleven doors that drifted apart would be
//! eleven answers to how a wheel scrolls.
//!
//! # Read-only
//!
//! A manual is read: the arrows, the page keys, the wheel and the bar move it,
//! `Esc` closes it, and nothing is typed.

use crate::door::{Canvas, Content, Ui};
use crate::files::Field;
use crate::manual::{Line, Tone, Which};
use anyhow::Result;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym, Modifiers};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.manual";

/// Opens manual `which` in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it(which: Which) -> Result<()> {
    crate::door::run(|_| {
        Ok(Manual {
            which,
            art_sizes: std::collections::HashMap::new(),
        })
    })
}

/// The window: which manual, and the size of its drawings.
struct Manual {
    which: Which,
    /// The size each drawing went up at, by its key: it is centred in its
    /// box by that size.
    art_sizes: std::collections::HashMap<String, (u32, u32)>,
}

impl Manual {
    /// The manual's lines, in the language of now, WRAPPED to this window —
    /// the text column starts two past the key column, and the bar takes the
    /// last cell.
    fn manual(&self, ui: &Ui) -> Vec<Line> {
        crate::manual::laid_out(
            self.which.lines(&ui.texts, &crate::leader_chord()),
            ui.columns,
            3,
            1,
        )
    }

    /// Moves by `by` lines, inside the manual.
    fn scroll_by(&self, ui: &mut Ui, by: i64) {
        let last = self.manual(ui).len().saturating_sub(ui.room_for_rows());
        ui.offset = usize::try_from(
            i64::try_from(ui.offset)
                .unwrap_or(0)
                .saturating_add(by)
                .max(0),
        )
        .unwrap_or(0)
        .min(last);
        ui.ask_paint();
    }
}

/// The manual's lines as the rows of a list, placed in cells.
///
/// THE KEY COLUMN is as wide as the widest key, or the drawings' column when
/// the manual has drawings — the footer's arithmetic, by the same numbers.
fn rows_of(manual: &[Line], columns: u16) -> Vec<Vec<Field>> {
    let key_cells = crate::manual::key_column(manual);
    let piece = |at: u16, text: String, color: tokens::Color| Field {
        at,
        text,
        color,
        hit: crate::files::Hit::Nothing,
        explain: None,
        ink: crate::files::Ink::Grid,
    };
    manual
        .iter()
        .map(|l| {
            let color = match l.tone {
                Tone::Heading => tokens::color::TEXT_HEADING,
                Tone::Plain | Tone::Pose => tokens::color::TEXT_SECONDARY,
                // WHAT A GESTURE DOES, the loudest — the footer's rule.
                Tone::Function => tokens::color::TEXT_PRIMARY,
                Tone::Detail => tokens::color::TEXT_DIM,
                // AIR: nothing is written on it.
                Tone::Air => return Vec::new(),
            };
            let mut fields = Vec::with_capacity(2);
            if !l.key.is_empty() {
                fields.push(piece(0, l.key.clone(), tokens::color::TEXT_KEY));
            }
            // A HEADING TAKES THE WHOLE LINE from the margin; everything else
            // starts after the key column.
            let (at, room_cells) = if l.title() {
                (0, columns)
            } else {
                (key_cells + 2, columns.saturating_sub(key_cells + 2))
            };
            fields.push(piece(
                at,
                ronoctua_render::layout::elide_end(&l.description, room_cells),
                color,
            ));
            fields
        })
        .collect()
}

impl Content for Manual {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(self.which.name()))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, _: Modifiers) {
        let page = i64::try_from(ui.room_for_rows().max(1)).unwrap_or(1);
        match event.keysym {
            Keysym::Escape => ui.quit = true,
            Keysym::Down => self.scroll_by(ui, 1),
            Keysym::Up => self.scroll_by(ui, -1),
            Keysym::Page_Down | Keysym::space => self.scroll_by(ui, page),
            Keysym::Page_Up => self.scroll_by(ui, -page),
            Keysym::Home => self.scroll_by(ui, i64::MIN / 2),
            Keysym::End => self.scroll_by(ui, i64::MAX / 2),
            _ => {}
        }
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<Field>> {
        rows_of(&self.manual(ui), ui.columns)
    }

    /// The standard list, under a header that NAMES the manual, and the
    /// gestures' drawings beside their lines.
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        // LINE 0: `MANUAL:` and the manual's name, drawn the way every title
        // of the system is — the label in the cool grey kept for what is said
        // ABOUT something, the name in the heading's ink — and the header
        // every window of this system has, drawer and all. A manual teaches
        // no shortcut: it is read.
        let label = ui.texts.t(ronoctua_i18n::key::MANUAL_WINDOW_LEAD);
        let name = ui.texts.t(self.which.name());
        crate::door::list::header_of(
            ui,
            canvas,
            Some(crate::chrome::Title {
                glyph: None,
                label: Some(&label),
                name: Some(&name),
            }),
            |_| Vec::new(),
        );
        let manual = self.manual(ui);
        let rows = rows_of(&manual, ui.columns);
        crate::door::list::body(ui, canvas, &rows);

        // THE DRAWING, when every line of its gesture is on screen — see the
        // footer's reason in `Footer::paint`. It goes up once per key (size
        // and theme are in it), and is centred in its box by its own size.
        let size = tokens::text::size();
        let cell = tokens::metrics::cell_width(size);
        let line_height = tokens::metrics::line_height(size);
        let first = ui.first_of_list();
        let visible: Vec<&Line> = manual
            .iter()
            .skip(ui.offset)
            .take(ui.room_for_rows())
            .collect();
        for (i, l) in visible.iter().enumerate() {
            let Some(g) = l.picture else {
                continue;
            };
            if i + crate::manual::GESTURE_LINES > visible.len() {
                continue;
            }
            let y = first + u16::try_from(i).unwrap_or(0);
            let key = crate::manual::gesture_art_key(g);
            let sizes = &mut self.art_sizes;
            canvas.upload(&key, || {
                let rgba = crate::manual::gesture_art(g)?;
                sizes.insert(key.clone(), (rgba.width, rgba.height));
                Some(rgba)
            });
            let Some(&(w, h)) = self.art_sizes.get(&key) else {
                continue;
            };
            let (bw, bh) = crate::manual::gesture_art_box();
            let (x, top) = (f32::from(ui.margin) * cell, f32::from(y) * line_height);
            #[allow(clippy::cast_precision_loss)]
            let (w, h) = (w as f32, h as f32);
            canvas.pictures.push((
                key,
                ronoctua_render::image::Area {
                    x: x + (bw - w).max(0.0) / 2.0,
                    y: top + (bh - h).max(0.0) / 2.0,
                    width: w.min(bw),
                    height: h.min(bh),
                },
            ));
        }
    }
}
