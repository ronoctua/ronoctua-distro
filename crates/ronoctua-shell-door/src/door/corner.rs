//! THE RIGHT END OF A WINDOW'S FIRST LINE, WITH NO HEADER — its name, in a
//! quiet tone, or on a page below its first, the way back.
//!
//! With the header hidden (the distro's default) nothing on a window said
//! which window it was, and a page below the first said nothing of the way
//! out but the `Esc` the minibuffer teaches. The owner asked (2026-11-09) for
//! what a few windows did by hand — the update window, the brain's page —
//! in every window where it fits: the name at the right end of the first line
//! the window already draws, and on a page below the first a `Back` there
//! instead, a click on which is the window's own `Esc`.
//!
//! # The window declares, the door draws
//!
//! A window says what goes there ([`Content::corner`]); the door draws it
//! after the window's frame, ONLY where that frame left the room — no run of
//! the window's on that line within [`AIR`] cells of it, no divider and no
//! picture across it, short of a bar's column. A first line full of the
//! window's own is a first line that keeps it, which is the owner's "when it
//! is possible": nothing is ever drawn over what a window drew.

use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use super::{Content, LEFT_BUTTON, Ui, tokens};

/// What a window says at the right end of its first line while it wears no
/// header ([`Content::corner`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Corner {
    /// Its name, in a quiet tone.
    Name(String),
    /// On a page below its first: the way back, a click that is its `Esc`.
    Back,
}

impl Corner {
    /// The window's name, by its text key.
    #[must_use]
    pub fn named(texts: &ronoctua_i18n::Texts, key: &str) -> Self {
        Self::Name(texts.t(key))
    }

    /// The name on the first page, the way back on any other — what most
    /// windows say.
    #[must_use]
    pub fn named_or_back(texts: &ronoctua_i18n::Texts, key: &str, deeper: bool) -> Self {
        if deeper {
            Self::Back
        } else {
            Self::named(texts, key)
        }
    }
}

/// The cells of air between the corner and the nearest run of the window's
/// on its line.
pub const AIR: u16 = 2;

/// Where the corner was drawn on the last frame, as the pointer asks it.
#[derive(Debug, Default)]
pub struct Placed {
    /// Its line, its first cell and its cells — `None` while none is drawn.
    pub(crate) at: Option<(u16, u16, u16)>,
    /// Whether it is the way back, which a click takes.
    pub(crate) back: bool,
    /// Whether the pointer rests on the way back.
    pub(crate) lit: bool,
    /// What the minibuffer says of it while the pointer rests on it.
    pub(crate) said: Option<String>,
}

impl Placed {
    /// The sentence about the way back, while the pointer rests on it.
    #[must_use]
    pub fn pointed(&self) -> Option<&str> {
        self.lit.then_some(self.said.as_deref()).flatten()
    }

    /// Whether the cell `(line, cell)` is the way back.
    fn holds(&self, line: u16, cell: u16) -> bool {
        self.back
            && self
                .at
                .is_some_and(|(l, at, cells)| l == line && (at..at + cells).contains(&cell))
    }
}

/// Draws the corner the window declares, where its frame left room for it.
pub fn paint<C: Content>(
    content: &C,
    ui: &mut Ui,
    spans: &mut Vec<ronoctua_render::Span>,
    under: &mut Vec<ronoctua_render::quad::Quad>,
    pictures: &[(String, ronoctua_render::image::Area)],
    bars: &crate::bars::Drawn,
) {
    let was_lit = ui.corner.lit;
    ui.corner.at = None;
    ui.corner.back = false;
    ui.corner.said = None;
    if !ui.hinted.is_hidden() {
        ui.corner.lit = false;
        return;
    }
    let Some(corner) = content.corner(ui) else {
        ui.corner.lit = false;
        return;
    };
    let size = tokens::text::size();
    let cell = tokens::metrics::cell_width(size);
    let line_px = tokens::metrics::line_height(size);
    let line = ui.first_of_list();
    if line >= ui.lines {
        return;
    }
    let (text, back) = match corner {
        Corner::Name(name) => (name, false),
        Corner::Back => (ui.texts.t(crate::key::DOOR_CORNER_BACK), true),
    };
    let wide = ronoctua_render::layout::width(&text);
    #[allow(clippy::cast_precision_loss)]
    let (top, bottom) = (f32::from(line) * line_px, f32::from(line + 1) * line_px);
    // SHORT OF A BAR'S COLUMN on this line — the bar is its own.
    let mut right = ui.margin + ui.columns;
    for track in bars.tracks() {
        if track.y < bottom && track.y + track.height > top {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let at = (track.x / cell).floor().max(0.0) as u16;
            right = right.min(at);
        }
    }
    let Some(at) = right.checked_sub(wide).filter(|at| *at >= ui.margin) else {
        return;
    };
    // NOTHING OF THE WINDOW'S WITHIN THE AIR, on its line.
    let taken = spans.iter().any(|s| {
        s.line == line && {
            let end = s.column + ronoctua_render::layout::width(&s.text).max(1);
            end + AIR > at && s.column < right
        }
    });
    #[allow(clippy::cast_precision_loss)]
    let (from_px, to_px) = (
        f32::from(at.saturating_sub(AIR)) * cell,
        f32::from(right) * cell,
    );
    let crosses =
        |x: f32, y: f32, w: f32, h: f32| x < to_px && x + w > from_px && y < bottom && y + h > top;
    // A DIVIDER across it — a rule a cell wide, never a band under a row.
    let divided = under
        .iter()
        .any(|q| q.width <= cell && crosses(q.x, q.y, q.width, q.height));
    let pictured = pictures
        .iter()
        .any(|(_, a)| crosses(a.x, a.y, a.width, a.height));
    if taken || divided || pictured {
        return;
    }
    if back && ui.corner.lit {
        // UNDER THE HAND, the band a row wears under it, under the word alone.
        #[allow(clippy::cast_precision_loss)]
        under.push(ronoctua_render::quad::Quad {
            x: f32::from(at) * cell,
            y: top,
            width: f32::from(wide) * cell,
            height: line_px,
            color: crate::seats::band_colour(false, 0),
        });
    }
    spans.push(ronoctua_render::Span {
        size: ronoctua_render::Size::Grid,
        text,
        column: at,
        line,
        color: if back {
            tokens::color::STATE_ACCENT
        } else {
            tokens::color::TEXT_DIM
        },
        edge: false,
        foot: false,
    });
    ui.corner.at = Some((line, at, wide));
    ui.corner.back = back;
    // THE KEY IS THE CODE'S, never the sentence's (`keys-are-never-text`).
    ui.corner.said = back.then(|| {
        let esc = ronoctua_keymap::Key::parse("ESC")
            .map(|k| crate::chrome::key_text(&k))
            .unwrap_or_default();
        ui.texts
            .t_with(crate::key::DOOR_CORNER_BACK_DOES, &[("keys", &esc)])
    });
    if !back && was_lit {
        ui.corner.lit = false;
    }
}

/// What one event of the pointer does to the corner: whether it lit or put
/// out the way back, and whether it was a press on it — the window's `Esc`.
pub(crate) enum Took {
    /// Not the corner's.
    Passed,
    /// A motion over the way back, which the window does not get.
    Kept,
    /// A press on the way back.
    Pressed,
}

/// The pointer, on the corner.
pub(crate) fn pointer(ui: &mut Ui, e: &PointerEvent) -> Took {
    let over = crate::text_select::Selecting::cell_of(e.position.0, e.position.1)
        .is_some_and(|(line, cell)| ui.corner.holds(line, cell));
    let lit = over
        && matches!(
            e.kind,
            PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. }
        );
    if matches!(
        e.kind,
        PointerEventKind::Enter { .. }
            | PointerEventKind::Motion { .. }
            | PointerEventKind::Leave { .. }
    ) && lit != ui.corner.lit
    {
        ui.corner.lit = lit;
        ui.ask_paint();
    }
    match e.kind {
        PointerEventKind::Press { button, .. } if over && button == LEFT_BUTTON => Took::Pressed,
        PointerEventKind::Release { .. } if over => Took::Kept,
        PointerEventKind::Motion { .. } if over => Took::Kept,
        _ => Took::Passed,
    }
}
