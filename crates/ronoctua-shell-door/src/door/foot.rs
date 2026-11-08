//! WHAT A WINDOW WITH NO HEADER SAYS AND ASKS, AT ITS FOOT.
//!
//! With no header (the distro's default), a message or a question took line 0
//! for as long as it was up and pushed the list down a line. The owner asked
//! for it at the other end (2026-11-08): under the list, on a ground of its own,
//! as many lines as the sentence needs — the Brain page's own foot — with a bar
//! for the rest, and the notification band's manners: the pie of the time it
//! has left, which stops while the pointer rests on it, and a click that takes
//! it off. A question is drawn here too, one line, in its own colours, its
//! keys at the right edge (`list::question_pieces`, the one builder).
//!
//! # Only for a window that says things through the door
//!
//! A window lends its line 0 to the door through `list::line_zero` (or the
//! two halves of it, `list::the_question` and `list::said_on_line_zero`); one
//! that never asks — the terminal draws its own grid — never loses a row to a
//! message ([`Foot::routed`]). The rows are taken BEFORE the frame
//! (`Door::settle`): the window is laid out again with what is left above
//! (`Ui::lines`), so no frame is ever drawn with the message over its list.
//!
//! # A clock only while there is a message
//!
//! `hang_the_echo` starts [`crate::echo::Countdown`] for the sentence said
//! and ticks once per eighth of it while it is up — the pie needs a frame per
//! step — and not at all otherwise. A held message ticks and loses nothing.

use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use super::{BACK_BUTTON, LEFT_BUTTON, Ui, tokens};

/// The area, as the door keeps it between frames.
#[derive(Debug, Default)]
pub struct Foot {
    /// The window draws its line 0 through the door's builder, with no header
    /// — so its message and its question are this area's. Set by the builder
    /// when it is asked (`list::the_question`, `list::said_on_line_zero`),
    /// through `&Ui`: the window paints with a shared borrow of it.
    pub(crate) routed: std::cell::Cell<bool>,
    /// The lines the area takes at the window's foot — [`Ui::lines`] is what
    /// is left above it.
    pub(crate) rows: u16,
    /// The sentence's clock, by the number it was said under (`Ui::said`).
    pub(crate) clock: Option<(u64, crate::echo::Countdown)>,
    /// The first wrapped line drawn.
    scroll: usize,
    /// The question's keys, where they were drawn — the header's own marks.
    marks: Vec<crate::chrome::Mark>,
    /// The cells of the pie and of the control that takes the message off,
    /// on the area's first line, when they were drawn.
    pie: Option<u16>,
    dismiss: Option<u16>,
    /// The bar's track and the scroll it was drawn for, while one is.
    bar: Option<(crate::bars::Track, ronoctua_design::Scroll)>,
    /// A press on the bar is dragging it.
    dragging: bool,
    /// Whether the pointer is on the area.
    pointer_in: bool,
    /// The control the pointer rests on: a key of the question, the pie or
    /// the dismissal.
    lit: Option<&'static str>,
    /// What the control under the pointer has to say, for the footer's
    /// minibuffer (`Door::settle`, beside the header's `pointed`).
    pointed: Option<String>,
    /// The wheel's fraction of a line, kept (`crate::wheel`).
    wheel_left: f64,
}

/// The ids the area's own controls are lit by.
const PIE: &str = "door-message-clock";
const DISMISS: &str = "door-message-dismiss";

/// What the pointer did on the area, for `pointer_into`.
pub(crate) enum Took {
    /// Not the area's: the window hears it.
    Passed,
    /// The area's, and nothing for the window to do.
    Kept,
    /// A key of the question, pressed by a click — as the header's are.
    Pressed(Option<&'static str>, crate::chrome::Act),
}

impl Foot {
    /// What the control under the pointer has to say, while it rests on one.
    #[must_use]
    pub fn pointed(&self) -> Option<&str> {
        self.pointed.as_deref()
    }
}

/// How many lines the area wants now: none with a header, for a window that
/// does not say things through the door, or with nothing to say; one for a
/// question; a message's wrapped lines, up to `space.window.messageLines`
/// and never more than half the window.
#[must_use]
pub fn wanted(ui: &Ui) -> u16 {
    if !ui.hinted.is_hidden() || !ui.foot.routed.get() {
        return 0;
    }
    if ui.asking.is_some() {
        return 1;
    }
    let Some(text) = ui.message.as_deref() else {
        return 0;
    };
    let wrapped = u16::try_from(lines_of(text, ui.columns).len()).unwrap_or(u16::MAX);
    wrapped
        .min(tokens::space::WINDOW_MESSAGELINES)
        .min((ui.all_lines / 2).max(1))
        .max(1)
}

/// The cells the clock and the dismissal take at the right of the first line,
/// with a cell of air before them.
const CLUSTER: u16 = 4;

/// A message, wrapped to the room the area's text has.
fn lines_of(text: &str, columns: u16) -> Vec<String> {
    crate::chrome::note_lines(text, columns.saturating_sub(CLUSTER).max(1))
}

/// Draws the area into the frame: its ground, the question or the message,
/// the clock, the dismissal and the bar — the area's first line is
/// [`Ui::lines`], everything under it to the window's edge is the area's.
pub fn paint(
    ui: &mut Ui,
    spans: &mut Vec<ronoctua_render::Span>,
    under: &mut Vec<ronoctua_render::quad::Quad>,
    over: &mut Vec<ronoctua_render::quad::Quad>,
    bars: &mut crate::bars::Drawn,
) {
    let foot = &mut ui.foot;
    foot.marks.clear();
    foot.pie = None;
    foot.dismiss = None;
    foot.bar = None;
    if foot.rows == 0 {
        return;
    }
    let size = tokens::text::size();
    let line = tokens::metrics::line_height(size);
    let cell = crate::chrome::cell_width();
    let first = ui.lines;
    #[allow(clippy::cast_precision_loss)]
    let (width, height) = (ui.width as f32, ui.height as f32);
    let top = line * f32::from(first);
    // ITS OWN GROUND, the Brain's foot's (`surface.header`), to the window's
    // edge: the leftover under the last line is the area's too.
    under.push(ronoctua_render::quad::Quad {
        x: 0.0,
        y: top,
        width,
        height: (height - top).max(0.0),
        color: tokens::color::SURFACE_HEADER,
    });
    let px = ronoctua_render::Size::Px(size);
    let (margin, columns) = (ui.margin, ui.columns);
    if let Some(asked) = ui.asking.as_ref() {
        let pieces = crate::door::list::question_pieces(asked, &ui.texts, columns);
        let pieces = crate::chrome::lit(&pieces, foot.lit);
        spans.extend(crate::chrome::hint_spans(
            &pieces, px, margin, first, columns,
        ));
        foot.marks = crate::chrome::hint_marks(&pieces, margin, first, columns);
        return;
    }
    let Some(text) = ui.message.as_deref() else {
        return;
    };
    let wrapped = lines_of(text, columns);
    let rows = usize::from(foot.rows);
    foot.scroll = foot.scroll.min(wrapped.len().saturating_sub(rows));
    for (i, l) in wrapped.iter().skip(foot.scroll).take(rows).enumerate() {
        spans.push(span(
            px,
            l.clone(),
            margin,
            first + to_u16(i),
            tokens::color::TEXT_PRIMARY,
        ));
    }
    // THE CLUSTER, at the right of the first line: the pie of the time left —
    // a pause while the pointer holds it — and the control that takes the
    // message off. A message that stays (`Ui::hold`) has no pie.
    let right = margin + columns;
    let ink = |id| {
        if foot.lit == Some(id) {
            tokens::color::TEXT_PRIMARY
        } else {
            tokens::color::TEXT_SECONDARY
        }
    };
    if let Some((_, clock)) = foot.clock.filter(|(mine, _)| *mine == ui.said) {
        let glyph = if clock.held() {
            tokens::icon::PAUSE
        } else {
            crate::band::countdown_glyph(clock.eighths(std::time::Instant::now()))
        };
        let at = right.saturating_sub(3);
        spans.push(span(px, glyph.to_string(), at, first, ink(PIE)));
        foot.pie = Some(at);
    }
    let at = right.saturating_sub(1);
    spans.push(span(
        px,
        tokens::icon::NOTIFICATION_MARK_READ.to_string(),
        at,
        first,
        ink(DISMISS),
    ));
    foot.dismiss = Some(at);
    // AND THE BAR, in the window's last cell, when the message is longer
    // than the area — a control, grabbed where it is drawn (`crate::bars`).
    let scroll = ronoctua_design::Scroll {
        total: wrapped.len(),
        lines: rows,
        offset: foot.scroll,
        top: 0,
    };
    if scroll.cut() {
        let (track, thumb) = crate::chrome::band_bar(scroll, width, top, height, cell, line);
        let track = crate::bars::Track::of(track);
        bars.record(track);
        if let Some((x, y, w, h)) = thumb {
            over.push(ronoctua_render::quad::Quad {
                x,
                y,
                width: w,
                height: h,
                color: tokens::color::RULE_RAISED,
            });
        }
        foot.bar = Some((track, scroll));
    }
}

/// One run of the area, at a cell and a line.
const fn span(
    size: ronoctua_render::Size,
    text: String,
    column: u16,
    line: u16,
    color: tokens::Color,
) -> ronoctua_render::Span {
    ronoctua_render::Span {
        size,
        text,
        column,
        line,
        color,
        edge: false,
        foot: false,
    }
}

fn to_u16(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

/// One pointer event, asked of the area before the window hears it: the
/// pointer resting on it holds the message's clock and lights the control
/// under it; a click on the dismissal takes the message off, on a key of the
/// question presses that key, and on the bar moves it; the wheel scrolls a
/// long message. Anything off the area is the window's — and leaving the area
/// lets the clock run again.
pub(crate) fn pointer(ui: &mut Ui, e: &PointerEvent) -> Took {
    let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
    let cell_px = f64::from(crate::chrome::cell_width());
    let (x, y) = e.position;
    let inside = ui.foot.rows > 0
        && line_px > 0.0
        && y >= line_px * f64::from(ui.lines)
        && !matches!(e.kind, PointerEventKind::Leave { .. });
    if !inside && !ui.foot.dragging {
        if ui.foot.pointer_in {
            left(ui);
        }
        return Took::Passed;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (line, cell) = ((y / line_px) as u16, (x / cell_px.max(1.0)) as u16);
    match e.kind {
        PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
            if !ui.foot.pointer_in {
                ui.foot.pointer_in = true;
                if let Some((_, clock)) = ui.foot.clock.as_mut() {
                    clock.hold(std::time::Instant::now());
                }
                ui.ask_paint();
            }
            if ui.foot.dragging {
                drag_to(ui, y);
                return Took::Kept;
            }
            light(ui, line, cell);
            Took::Kept
        }
        PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
            if let Some((track, _)) = ui.foot.bar
                && track.holds(x, y)
            {
                ui.foot.dragging = true;
                drag_to(ui, y);
                return Took::Kept;
            }
            if let Some(mark) = ui.foot.marks.iter().find(|m| m.covers(line, cell))
                && let Some(act) = mark.press
            {
                return Took::Pressed(Some(mark.id), act);
            }
            if line == ui.lines && ui.foot.dismiss.is_some_and(|at| cell == at) {
                if let Some(text) = ui.message.clone() {
                    ui.expire(ui.said, &text);
                }
                ui.foot.clock = None;
                return Took::Kept;
            }
            Took::Kept
        }
        PointerEventKind::Release { .. } => {
            ui.foot.dragging = false;
            if inside { Took::Kept } else { Took::Passed }
        }
        PointerEventKind::Axis { vertical, .. } => {
            ui.foot.wheel_left += crate::wheel::axis_lines(&vertical, line_px);
            let lines = crate::wheel::wheel_lines(&mut ui.foot.wheel_left);
            if lines != 0 {
                let moved = if lines > 0 {
                    ui.foot
                        .scroll
                        .saturating_add(usize::try_from(lines).unwrap_or(0))
                } else {
                    ui.foot
                        .scroll
                        .saturating_sub(usize::try_from(-lines).unwrap_or(0))
                };
                ui.foot.scroll = moved;
                ui.ask_paint();
            }
            Took::Kept
        }
        // THE BACK BUTTON is the window's `ESC`, wherever it is pressed.
        PointerEventKind::Press { button, .. } if button == BACK_BUTTON => Took::Passed,
        _ => Took::Kept,
    }
}

/// The pointer went off the area: the clock runs again, nothing is lit.
fn left(ui: &mut Ui) {
    ui.foot.pointer_in = false;
    ui.foot.lit = None;
    ui.foot.pointed = None;
    if let Some((_, clock)) = ui.foot.clock.as_mut() {
        clock.release(std::time::Instant::now());
    }
    ui.ask_paint();
}

/// Lights the control under the pointer, and keeps what it has to say.
fn light(ui: &mut Ui, line: u16, cell: u16) {
    let on_first = line == ui.lines;
    let now = if let Some(m) = ui
        .foot
        .marks
        .iter()
        .find(|m| m.covers(line, cell) && m.press.is_some())
    {
        Some(m.id)
    } else if on_first && ui.foot.dismiss == Some(cell) {
        Some(DISMISS)
    } else if on_first && ui.foot.pie == Some(cell) {
        Some(PIE)
    } else {
        None
    };
    if now == ui.foot.lit {
        return;
    }
    ui.foot.lit = now;
    ui.foot.pointed = now.and_then(|id| match id {
        DISMISS => Some(ui.texts.t(crate::key::DOOR_MESSAGE_DISMISS)),
        PIE => Some(ui.texts.t(if ui.foot.clock.is_some_and(|(_, c)| c.held()) {
            crate::key::HINT_NOTIF_PAUSED
        } else {
            crate::key::HINT_NOTIF_COUNTDOWN
        })),
        other => crate::chrome::hint_sentence(other, &ui.texts),
    });
    ui.ask_paint();
}

/// The bar, dragged to `y`: the message scrolls to the line under it.
fn drag_to(ui: &mut Ui, y: f64) {
    let Some((track, scroll)) = ui.foot.bar else {
        return;
    };
    let line = tokens::metrics::line_height(tokens::text::size());
    #[allow(clippy::cast_possible_truncation)]
    let at = (y as f32) - track.y;
    let offset = scroll.offset_at(at, track.height, line);
    if offset != ui.foot.scroll {
        ui.foot.scroll = offset;
        ui.ask_paint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hidden_ui() -> Ui {
        let mut ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        ui.hinted.hide_header(true);
        ui.width = 600;
        ui.height = 400;
        ui.recount();
        ui.foot.routed.set(true);
        ui
    }

    /// FAILS IF: a window with no header that says something through the
    /// door does not get the area — or gets it with a header, or without
    /// saying anything through the door (the terminal's grid must never
    /// lose a row to a message) — or a long message takes more than its cap.
    #[test]
    fn the_area_is_wanted_only_where_the_door_says_it() {
        let mut ui = hidden_ui();
        assert_eq!(wanted(&ui), 0, "nothing said");
        ui.say("short".to_owned());
        assert_eq!(wanted(&ui), 1);
        ui.say("word ".repeat(400));
        assert_eq!(wanted(&ui), tokens::space::WINDOW_MESSAGELINES);
        ui.ask(crate::line::Line::new("software-web-new", false, None));
        assert_eq!(wanted(&ui), 1, "a question is one line");
        ui.foot.routed.set(false);
        assert_eq!(wanted(&ui), 0, "a window that draws its own text");
        ui.foot.routed.set(true);
        ui.hinted.hide_header(false);
        assert_eq!(wanted(&ui), 0, "with a header, line 0 says it");
    }

    /// FAILS IF: a message longer than the area is cut with no way to read
    /// the rest — no bar, or a bar the pointer cannot grab — or draws more
    /// lines than the area has, or a said sentence has no pie of its time,
    /// or the area wears the window's ground instead of its own.
    #[test]
    fn a_long_message_wraps_to_the_cap_and_scrolls_by_its_bar() {
        let mut ui = hidden_ui();
        ui.say("palavra ".repeat(300));
        ui.foot.rows = wanted(&ui);
        ui.recount();
        let mine = ui.said;
        ui.foot.clock = Some((
            mine,
            crate::echo::Countdown::start(
                std::time::Duration::from_secs(3),
                std::time::Instant::now(),
            ),
        ));
        let (mut spans, mut under, mut over) = (Vec::new(), Vec::new(), Vec::new());
        let mut bars = crate::bars::Drawn::default();
        paint(&mut ui, &mut spans, &mut under, &mut over, &mut bars);
        assert!(
            under
                .iter()
                .any(|q| q.color == tokens::color::SURFACE_HEADER)
        );
        let text: Vec<_> = spans
            .iter()
            .filter(|s| s.color == tokens::color::TEXT_PRIMARY)
            .collect();
        assert_eq!(text.len(), usize::from(ui.foot.rows), "{text:?}");
        assert!(text.iter().all(|s| s.line >= ui.lines));
        assert!(
            spans
                .iter()
                .any(|s| s.text == tokens::icon::NOTIFICATION_MARK_READ.to_string())
        );
        assert!(
            spans
                .iter()
                .any(|s| s.text == crate::band::countdown_glyph(8).to_string()),
            "no pie: {spans:?}"
        );
        let (track, _) = ui.foot.bar.expect("a bar for the rest");
        assert!(bars.on(f64::from(track.x + 1.0), f64::from(track.y + 1.0)));
        assert!(over.iter().any(|q| q.color == tokens::color::RULE_RAISED));
    }

    /// FAILS IF: the rows the area takes are not taken from the window's —
    /// the list would draw under the message — or come back when it goes.
    #[test]
    fn the_window_gives_the_area_its_rows_and_takes_them_back() {
        let mut ui = hidden_ui();
        let all = ui.lines;
        ui.say("one line".to_owned());
        ui.foot.rows = wanted(&ui);
        ui.recount();
        assert_eq!(ui.lines, all - 1);
        assert_eq!(ui.all_lines, all);
        ui.foot.rows = 0;
        ui.recount();
        assert_eq!(ui.lines, all);
    }
}
