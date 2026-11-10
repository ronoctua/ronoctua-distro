//! WHAT A WINDOW WITH NO HEADER SAYS AND ASKS, AT ITS FOOT — AND WHAT A
//! WINDOW THAT KEEPS A FOOT EXPLAINS THERE.
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
//! # A foot the window keeps
//!
//! A window may keep the area for good ([`Content::foot_lines`],
//! `space.window.footLines`): it then explains the thing under the pointer,
//! or the cursor's, there ([`Content::explained`]) — its name and what a
//! click or a key does to it on the first line ([`gesture`]), and the
//! sentences about it under them, each in its tone — the owner's ask for the
//! update window (2026-11-08), where a sentence drawn under its row pushed
//! every row below it down. The area is ALWAYS that tall, header or not and
//! message or not: the message and the question take the same rows while
//! they are up and the explanation comes back when they go, so nothing above
//! the foot ever moves. With the header shown, line 0 says the message and
//! the question as it always did, and the foot goes on explaining — the
//! door's text is never drawn twice ([`says_it`]).
//!
//! # A clock only while there is a message
//!
//! `hang_the_echo` starts [`crate::echo::Countdown`] for the sentence said
//! and ticks once per eighth of it while it is up — the pie needs a frame per
//! step — and not at all otherwise. A held message ticks and loses nothing.

use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use super::{BACK_BUTTON, Content, LEFT_BUTTON, Ui, tokens};

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
    /// The lines the window keeps for its own foot ([`Content::foot_lines`]),
    /// as last asked — `0` for a window that keeps none.
    pub(crate) own: u16,
    /// The sentence's clock, by the number it was said under (`Ui::said`).
    pub(crate) clock: Option<(u64, crate::echo::Countdown)>,
    /// What the area drew last — the scroll belongs to it, and is forgotten
    /// when it changes.
    shown: Option<Shown>,
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
    pub(crate) dragging: bool,
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

/// What the area drew, for the scroll that belongs to it: a new message, or
/// an explanation of another thing, opens at its first line.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Shown {
    Question,
    Said(String),
    Explained(String),
}

/// WHAT A KEPT FOOT EXPLAINS ([`Content::explained`]): the thing under the
/// pointer, or the cursor's.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Explained {
    /// WHAT IT IS ABOUT, as the window names it — an item's id, never drawn.
    /// The area's scroll belongs to it: another subject opens at its first
    /// line, and the same one re-worded (a clock that ticks) keeps its place.
    pub subject: String,
    /// The thing's name, on the area's first line.
    pub name: String,
    /// What a click or a key does to it ([`gesture`]), against the right edge
    /// of the first line — on a line of its own under the name where the two
    /// do not fit side by side. `None` for a thing nothing takes.
    pub gesture: Option<String>,
    /// The paragraphs under them, each wrapped whole in its own tone: the
    /// sentence, why it is refused, the command it runs.
    pub said: Vec<(String, tokens::Color)>,
}

impl Explained {
    /// What the foot explains, as ONE line — for a footer's region, which
    /// has no foot and says it on its explanation line, cut at its width:
    /// what the hand does to the thing first (the line's whole question
    /// under the pointer), then its sentences, each without its tone.
    /// `None` when there is nothing to say.
    ///
    /// ONE ANSWER FOR EVERY REGION that explains as its window's foot does
    /// (the update's, the video catalogue's): it was the update region's
    /// own, and a second view would have written it again.
    #[must_use]
    pub fn joined(&self) -> Option<String> {
        let said: Vec<&str> = self
            .said
            .iter()
            .map(|(text, _)| text.as_str())
            .filter(|t| !t.is_empty())
            .collect();
        let said = said.join(" ");
        match self.gesture.as_deref().filter(|g| !g.is_empty()) {
            Some(g) if said.is_empty() => Some(g.to_owned()),
            Some(g) => Some(format!("{g}. {said}")),
            None => (!said.is_empty()).then_some(said),
        }
    }

    /// [`Explained::joined`] led by the thing's NAME — for a thing the line
    /// does not name beside it: a glyph is a picture, and the region's line
    /// said "a click: gives it this rating" without saying which one.
    /// `None` when there is nothing to say.
    #[must_use]
    pub fn joined_named(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let rest = self.joined()?;
        if self.name.is_empty() {
            return Some(rest);
        }
        Some(texts.t_with(
            crate::key::DOOR_FOOT_NAMED,
            &[("name", &self.name), ("rest", &rest)],
        ))
    }
}

/// HOW THE THING A FOOT EXPLAINS IS TAKEN — the three ways a window's row
/// answers the hand, each said with what it will do ([`gesture`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    /// The cursor's thing: `RET`, or a second click on it, does it.
    Ret,
    /// A thing the pointer rests on that the cursor is not on: the first
    /// click selects it, the second does it.
    Selects,
    /// A control that acts on the first click — a row's box.
    AtOnce,
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
    /// The pointer came onto the area — at a motion's time, when it was
    /// one: nothing of the window's is under it now
    /// ([`crate::door::Content::pointer_went_to_the_foot`]).
    Entered(Option<u32>),
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

/// What a click or a key does to the thing a foot explains, said with what
/// it will do — `does`, the window's words for it ("switch it off: the next
/// update skips this step"). The key is the code's (`RET`), never a word of
/// the catalogue (`keys-are-never-text`).
#[must_use]
pub fn gesture(texts: &ronoctua_i18n::Texts, how: Gesture, does: &str) -> String {
    match how {
        Gesture::Ret => {
            let keys =
                crate::chrome::key_text(&ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Enter));
            texts.t_with(
                crate::key::DOOR_FOOT_RET,
                &[("keys", &keys), ("does", does)],
            )
        }
        Gesture::Selects => texts.t_with(crate::key::DOOR_FOOT_CLICK_SELECTS, &[("does", does)]),
        Gesture::AtOnce => texts.t_with(crate::key::DOOR_FOOT_CLICK_AT_ONCE, &[("does", does)]),
    }
}

/// How many lines the area wants now. A window that keeps a foot
/// ([`Content::foot_lines`]) has it always — never more than a third of the
/// window — whatever is said and whether the header is shown. Any other:
/// none with a header, for a window that does not say things through the
/// door, or with nothing to say; one for a question; a message's wrapped
/// lines, up to `space.window.messageLines` and never more than half the
/// window.
#[must_use]
pub fn wanted(ui: &Ui) -> u16 {
    if ui.foot.own > 0 {
        return ui.foot.own.min(ui.all_lines / 3).max(1);
    }
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

/// The rows the area takes now, set where the window is laid out from
/// ([`Ui::lines`]) — the foot `content` keeps asked first. `true` when they
/// moved, and the window is to be laid out again in what is left above.
pub(crate) fn settle_rows<C: Content>(content: &C, ui: &mut Ui) -> bool {
    ui.foot.own = content.foot_lines(ui);
    let wanted = wanted(ui);
    let moved = wanted != ui.foot.rows;
    ui.foot.rows = wanted;
    moved
}

/// Whether the area says the DOOR'S text now — the question asked or the
/// message said — rather than what the window explains: only with no header
/// (with one, line 0 says it, and saying it here too would say it twice),
/// for a window that says things through the door.
#[must_use]
pub(crate) fn says_it(ui: &Ui) -> bool {
    ui.foot.rows > 0
        && ui.hinted.is_hidden()
        && ui.foot.routed.get()
        && (ui.asking.is_some() || ui.message.is_some())
}

/// The cells the clock and the dismissal take at the right of the first line,
/// with a cell of air before them.
const CLUSTER: u16 = 4;

/// A message, wrapped to the room the area's text has.
fn lines_of(text: &str, columns: u16) -> Vec<String> {
    crate::chrome::note_lines(text, columns.saturating_sub(CLUSTER).max(1))
}

/// The cells an explanation's text has: the window's columns, less the
/// bar's cell where no margin holds it — the bar is drawn in the window's
/// last cell, which a margin of one keeps clear of the text.
fn text_room(ui: &Ui) -> u16 {
    ui.columns
        .saturating_sub(1u16.saturating_sub(ui.margin))
        .max(1)
}

/// The air between the name and its gesture when they share a line: two
/// cells read as two things without reading as a margin — the modeline's
/// gap between segments, spent on the same distinction.
const GESTURE_GAP: u16 = tokens::space::MODELINE_SEGMENTGAP;

/// One line of an explanation, laid out: words at the margin, words against
/// the right edge, or both — the name and its gesture share the first.
#[derive(Debug, Clone, Default, PartialEq)]
struct Laid {
    left: Option<(String, tokens::Color)>,
    right: Option<(String, tokens::Color)>,
}

impl Laid {
    fn left(text: String, color: tokens::Color) -> Self {
        Self {
            left: Some((text, color)),
            right: None,
        }
    }
}

/// An explanation in `room` cells: the HEAD — its name, and the gesture
/// beside it where the two fit, else under it against the right edge (or
/// wrapped, wider than the line) — and the BODY, every paragraph wrapped
/// whole in its own tone (`chrome::note_lines`: a note is never cut).
fn laid(e: &Explained, room: u16) -> (Vec<Laid>, Vec<Laid>) {
    use ronoctua_render::layout::width;
    let mut head: Vec<Laid> = if e.name.is_empty() {
        Vec::new()
    } else {
        crate::chrome::note_lines(&e.name, room)
            .into_iter()
            .map(|l| Laid::left(l, tokens::color::TEXT_PRIMARY))
            .collect()
    };
    if let Some(gesture) = e.gesture.as_deref().filter(|g| !g.is_empty()) {
        let ink = tokens::color::TEXT_DIM;
        let cells = width(gesture);
        let beside = head.last_mut().filter(|l| {
            l.left
                .as_ref()
                .is_some_and(|(t, _)| width(t) + GESTURE_GAP + cells <= room)
        });
        match beside {
            Some(l) => l.right = Some((gesture.to_owned(), ink)),
            None if cells <= room => head.push(Laid {
                left: None,
                right: Some((gesture.to_owned(), ink)),
            }),
            None => head.extend(
                crate::chrome::note_lines(gesture, room)
                    .into_iter()
                    .map(|l| Laid::left(l, ink)),
            ),
        }
    }
    let body = e
        .said
        .iter()
        .flat_map(|(text, ink)| {
            crate::chrome::note_lines(text, room)
                .into_iter()
                .map(|l| Laid::left(l, *ink))
        })
        .collect();
    (head, body)
}

/// Draws the area into the frame: its ground, then the question or the
/// message, the clock, the dismissal and the bar — or, in a foot the window
/// keeps, what it `explained` — the area's first line is [`Ui::lines`],
/// everything under it to the window's edge is the area's.
pub fn paint(
    ui: &mut Ui,
    explained: Option<&Explained>,
    spans: &mut Vec<ronoctua_render::Span>,
    under: &mut Vec<ronoctua_render::quad::Quad>,
    over: &mut Vec<ronoctua_render::quad::Quad>,
    bars: &mut crate::bars::Drawn,
) {
    let says = says_it(ui);
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
    // THE SCROLL BELONGS TO WHAT IS DRAWN: a new message, or the explanation
    // of another thing, opens at its first line.
    let shown = if !says {
        explained.map(|e| Shown::Explained(e.subject.clone()))
    } else if ui.asking.is_some() {
        Some(Shown::Question)
    } else {
        ui.message.clone().map(Shown::Said)
    };
    if foot.shown != shown {
        foot.shown = shown;
        foot.scroll = 0;
    }
    if !says {
        if let Some(e) = explained {
            explain(ui, e, spans, over, bars);
        }
        return;
    }
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
    band_bar(foot, scroll, (width, top, height), over, bars);
}

/// What the window explains, drawn in the area: the head on its first lines
/// — it STAYS while the sentence under it scrolls, so what the sentence is
/// about is never scrolled away — and the body in the rows left, with the
/// bar over those rows when it is longer. A head that would leave the body
/// no row scrolls with it.
fn explain(
    ui: &mut Ui,
    e: &Explained,
    spans: &mut Vec<ronoctua_render::Span>,
    over: &mut Vec<ronoctua_render::quad::Quad>,
    bars: &mut crate::bars::Drawn,
) {
    use ronoctua_render::layout::width;
    let size = tokens::text::size();
    let line = tokens::metrics::line_height(size);
    let px = ronoctua_render::Size::Px(size);
    let (room, margin, first) = (text_room(ui), ui.margin, ui.lines);
    #[allow(clippy::cast_precision_loss)]
    let (window_width, height) = (ui.width as f32, ui.height as f32);
    let foot = &mut ui.foot;
    let rows = usize::from(foot.rows);
    let (head, body) = laid(e, room);
    let (fixed, flowing) = if head.len() < rows {
        (head, body)
    } else {
        (Vec::new(), head.into_iter().chain(body).collect())
    };
    let mut put = |l: &Laid, at_line: u16| {
        if let Some((text, color)) = &l.left {
            spans.push(span(px, text.clone(), margin, at_line, *color));
        }
        if let Some((text, color)) = &l.right {
            let at = margin + room.saturating_sub(width(text));
            spans.push(span(px, text.clone(), at, at_line, *color));
        }
    };
    for (i, l) in fixed.iter().enumerate() {
        put(l, first + to_u16(i));
    }
    let from = first + to_u16(fixed.len());
    let band = rows - fixed.len();
    foot.scroll = foot.scroll.min(flowing.len().saturating_sub(band));
    for (i, l) in flowing.iter().skip(foot.scroll).take(band).enumerate() {
        put(l, from + to_u16(i));
    }
    let scroll = ronoctua_design::Scroll {
        total: flowing.len(),
        lines: band,
        offset: foot.scroll,
        top: 0,
    };
    band_bar(
        foot,
        scroll,
        (window_width, line * f32::from(from), height),
        over,
        bars,
    );
}

/// The area's bar over the band `(right, top, bottom)` in pixels, in the
/// window's last cell, when what the band shows is longer than it — a
/// control, grabbed where it is drawn (`crate::bars`).
fn band_bar(
    foot: &mut Foot,
    scroll: ronoctua_design::Scroll,
    (right, top, bottom): (f32, f32, f32),
    over: &mut Vec<ronoctua_render::quad::Quad>,
    bars: &mut crate::bars::Drawn,
) {
    if !scroll.cut() {
        return;
    }
    let line = tokens::metrics::line_height(tokens::text::size());
    let (track, thumb) = crate::chrome::band_bar(
        scroll,
        right,
        top,
        bottom,
        crate::chrome::cell_width(),
        line,
    );
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
/// long message, or a long explanation. Anything off the area is the
/// window's — and leaving the area lets the clock run again. A pointer that
/// moves inside a kept foot stays the foot's, so what it explains holds still
/// while it is read.
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
            let entered = !ui.foot.pointer_in;
            if entered {
                ui.foot.pointer_in = true;
                ui.ask_paint();
            }
            // THE CLOCK STOPS while the pointer rests on the message — one
            // said while it was already resting on a kept foot too — and
            // never for a message line 0 says, under a header.
            hold_the_clock(ui);
            if ui.foot.dragging {
                drag_to(ui, y);
                return Took::Kept;
            }
            light(ui, line, cell);
            if entered {
                return Took::Entered(match e.kind {
                    PointerEventKind::Motion { time } => Some(time),
                    _ => None,
                });
            }
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

/// The message's clock stops while the pointer is on the area that says it
/// — and a frame is owed, for the pause the pie becomes.
pub(crate) fn hold_the_clock(ui: &mut Ui) {
    if !ui.foot.pointer_in || !says_it(ui) {
        return;
    }
    if let Some((_, clock)) = ui.foot.clock.as_mut()
        && !clock.held()
    {
        clock.hold(std::time::Instant::now());
        ui.ask_paint();
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

    /// A window that keeps a foot, laid out as the door lays one out: its
    /// rows taken before the frame, the list in what is left above.
    fn kept_ui() -> Ui {
        let mut ui = hidden_ui();
        (ui.width, ui.height) = (900, 600);
        ui.recount();
        ui.foot.own = tokens::space::WINDOW_FOOTLINES;
        ui.foot.rows = wanted(&ui);
        ui.recount();
        ui
    }

    /// What a frame draws, and the bars it records.
    struct Drawn {
        spans: Vec<ronoctua_render::Span>,
        under: Vec<ronoctua_render::quad::Quad>,
        over: Vec<ronoctua_render::quad::Quad>,
        bars: crate::bars::Drawn,
    }

    fn painted(ui: &mut Ui, explained: Option<&Explained>) -> Drawn {
        let (mut spans, mut under, mut over) = (Vec::new(), Vec::new(), Vec::new());
        let mut bars = crate::bars::Drawn::default();
        paint(ui, explained, &mut spans, &mut under, &mut over, &mut bars);
        Drawn {
            spans,
            under,
            over,
            bars,
        }
    }

    fn explained(subject: &str, sentence: &str) -> Explained {
        Explained {
            subject: subject.to_owned(),
            name: format!("{subject} name"),
            gesture: Some("click: it happens".to_owned()),
            said: vec![(sentence.to_owned(), tokens::color::TEXT_SECONDARY)],
        }
    }

    /// FAILS IF: the one line a footer's region says loses the gesture or a
    /// sentence, puts the gesture after the sentences, says an empty
    /// sentence as a gap, or says something about a thing that has nothing.
    #[test]
    fn what_a_foot_explains_joins_into_one_line() {
        let mut e = explained("row", "It does a thing.");
        e.said.push((String::new(), tokens::color::TEXT_DIM));
        e.said
            .push(("And another.".to_owned(), tokens::color::TEXT_DIM));
        assert_eq!(
            e.joined().as_deref(),
            Some("click: it happens. It does a thing. And another.")
        );
        e.gesture = None;
        assert_eq!(e.joined().as_deref(), Some("It does a thing. And another."));
        e.said.clear();
        assert_eq!(e.joined(), None, "nothing to say");
        e.gesture = Some("click: it happens".to_owned());
        assert_eq!(e.joined().as_deref(), Some("click: it happens"));
        e.gesture = Some(String::new());
        assert_eq!(e.joined(), None, "an empty gesture is no gesture");
    }

    /// FAILS IF: the one line a glyph is said on leaves out which glyph it
    /// is — or names a thing that has no name, or says something when
    /// there is nothing to say.
    #[test]
    fn a_line_led_by_its_name_says_which_thing() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut e = explained("row", "It does a thing.");
        e.name = "Very positive".to_owned();
        let said = e.joined_named(&texts).expect("a line");
        assert!(said.starts_with("Very positive"), "{said}");
        assert!(said.ends_with(&e.joined().expect("joined")), "{said}");
        e.name.clear();
        assert_eq!(e.joined_named(&texts), e.joined(), "no name to lead with");
        e.said.clear();
        e.gesture = None;
        assert_eq!(e.joined_named(&texts), None, "nothing to say");
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
        let Drawn {
            spans,
            under,
            over,
            bars,
        } = painted(&mut ui, None);
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

    /// FAILS IF: a kept foot changes height — with a message, a long one, a
    /// question, the header shown or hidden, or another thing explained —
    /// so the list above it moves (the owner's complaint: a sentence that
    /// pushed every row under it down); or it takes more than a third of a
    /// short window.
    #[test]
    fn a_kept_foot_keeps_its_height_whatever_it_says() {
        let mut ui = kept_ui();
        let (rows, lines) = (ui.foot.rows, ui.lines);
        assert_eq!(rows, tokens::space::WINDOW_FOOTLINES.min(ui.all_lines / 3));
        let same = |ui: &Ui, what: &str| {
            assert_eq!(wanted(ui), rows, "{what}");
        };
        same(&ui, "nothing said");
        ui.say("short".to_owned());
        same(&ui, "a message");
        ui.say("word ".repeat(400));
        same(&ui, "a long message");
        ui.ask(crate::line::Line::new("software-web-new", false, None));
        same(&ui, "a question");
        ui.asking = None;
        ui.hinted.hide_header(false);
        same(&ui, "the header shown");
        ui.hinted.hide_header(true);
        ui.message = None;
        for e in [
            explained("one", "a sentence"),
            explained("two", &"a much longer sentence ".repeat(30)),
        ] {
            let _ = painted(&mut ui, Some(&e));
            same(&ui, "another thing explained");
        }
        assert_eq!(ui.lines, lines, "the list moved");
        // AND A SHORT WINDOW keeps its list: a third, never more.
        let line = tokens::metrics::line_height(tokens::text::size());
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let short = (line * 7.5) as u32;
        ui.height = short;
        ui.recount();
        assert_eq!(ui.all_lines, 7);
        assert_eq!(wanted(&ui), 2);
    }

    /// FAILS IF: what the window explains is not drawn in its foot — the
    /// name on the first line, the gesture against the right edge of it,
    /// every paragraph in its own tone under them — or the message does not
    /// take the same rows while it is up (header hidden), or the explanation
    /// does not come back once the message goes.
    #[test]
    fn a_message_takes_the_kept_foot_and_the_explanation_comes_back() {
        let mut ui = kept_ui();
        let e = Explained {
            subject: "step".to_owned(),
            name: "Rebuild the system".to_owned(),
            gesture: Some(gesture(&ui.texts, Gesture::Ret, "switch it off")),
            said: vec![
                (
                    "Builds the new system.".to_owned(),
                    tokens::color::TEXT_SECONDARY,
                ),
                ("Needs root.".to_owned(), tokens::color::STATE_WARNING),
            ],
        };
        let first = ui.lines;
        let d = painted(&mut ui, Some(&e));
        let name = d.spans.iter().find(|s| s.text == e.name).expect("the name");
        assert_eq!((name.line, name.column), (first, ui.margin));
        let gesture = d
            .spans
            .iter()
            .find(|s| Some(&s.text) == e.gesture.as_ref())
            .expect("the gesture");
        assert_eq!(gesture.line, first, "it fits beside the name");
        assert_eq!(
            gesture.column + ronoctua_render::layout::width(&gesture.text),
            ui.margin + ui.columns,
            "against the right edge"
        );
        assert_eq!(gesture.color, tokens::color::TEXT_DIM);
        for (text, color) in &e.said {
            let s = d.spans.iter().find(|s| &s.text == text).expect(text);
            assert_eq!(s.color, *color, "{text} in its own tone");
            assert!(s.line > first);
        }
        // THE MESSAGE TAKES THE SAME ROWS, and nothing of the explanation.
        ui.say("It was saved".to_owned());
        let d = painted(&mut ui, Some(&e));
        assert!(
            d.spans
                .iter()
                .any(|s| s.text == "It was saved" && s.line == first)
        );
        assert!(d.spans.iter().all(|s| s.text != e.name), "drawn under it");
        // AND THE EXPLANATION COMES BACK when it goes.
        ui.message = None;
        let d = painted(&mut ui, Some(&e));
        assert!(d.spans.iter().any(|s| s.text == e.name && s.line == first));
    }

    /// FAILS IF: a name and a gesture that do not fit side by side overlap or
    /// lose a word, instead of the gesture going down to a line of its own,
    /// against the right edge.
    #[test]
    fn a_gesture_that_does_not_fit_beside_the_name_goes_under_it() {
        let mut ui = kept_ui();
        let e = Explained {
            subject: "long".to_owned(),
            name: "n".repeat(usize::from(ui.columns) - 4),
            gesture: Some("click: it happens".to_owned()),
            said: vec![("A sentence.".to_owned(), tokens::color::TEXT_SECONDARY)],
        };
        let first = ui.lines;
        let d = painted(&mut ui, Some(&e));
        let gesture = d
            .spans
            .iter()
            .find(|s| Some(&s.text) == e.gesture.as_ref())
            .expect("the gesture");
        assert_eq!(gesture.line, first + 1);
        assert_eq!(
            gesture.column + ronoctua_render::layout::width(&gesture.text),
            ui.margin + ui.columns
        );
        let sentence = d
            .spans
            .iter()
            .find(|s| s.text == "A sentence.")
            .expect("it");
        assert_eq!(sentence.line, first + 2, "the sentence follows the gesture");
    }

    /// FAILS IF: an explanation longer than the foot is cut with no way to
    /// read the rest — no bar, a bar the pointer cannot grab, a wheel that
    /// does not move it — or the name scrolls away with the sentence, or the
    /// sentence draws past the foot, or a scroll left on one thing opens the
    /// next thing past its first line.
    #[test]
    fn a_long_explanation_scrolls_by_its_bar_under_its_name() {
        let mut ui = kept_ui();
        let words: Vec<String> = (0..120).map(|n| format!("w{n}")).collect();
        let e = explained("long", &words.join(" "));
        let first = ui.lines;
        let d = painted(&mut ui, Some(&e));
        let (track, scroll) = ui.foot.bar.expect("a bar for the rest");
        assert!(
            d.bars
                .on(f64::from(track.x + 1.0), f64::from(track.y + 1.0))
        );
        assert!(d.over.iter().any(|q| q.color == tokens::color::RULE_RAISED));
        assert_eq!(
            scroll.lines,
            usize::from(ui.foot.rows) - 1,
            "the name stays"
        );
        let line = tokens::metrics::line_height(tokens::text::size());
        assert!(
            (track.y - line * f32::from(first + 1)).abs() < 0.5,
            "the bar is the sentence's, under the name"
        );
        let bottom = first + ui.foot.rows;
        assert!(d.spans.iter().all(|s| s.line < bottom), "past the foot");
        // THE WHEEL, over the foot, moves the sentence and keeps the name.
        let at = (f64::from(track.x) - 20.0, f64::from(track.y) + 2.0);
        let wheel = PointerEvent {
            surface: ui.surface.clone(),
            position: at,
            kind: PointerEventKind::Axis {
                time: 0,
                horizontal: smithay_client_toolkit::seat::pointer::AxisScroll::default(),
                vertical: smithay_client_toolkit::seat::pointer::AxisScroll {
                    absolute: f64::from(line) * 3.0,
                    ..Default::default()
                },
                source: None,
            },
        };
        assert!(matches!(pointer(&mut ui, &wheel), Took::Kept));
        let d = painted(&mut ui, Some(&e));
        let (_, scrolled) = ui.foot.bar.expect("still cut");
        assert!(scrolled.offset > 0, "the wheel did not move it");
        assert!(d.spans.iter().any(|s| s.text == e.name && s.line == first));
        // AND ANOTHER THING opens at its first line.
        let other = explained("other", &words.join(" "));
        let _ = painted(&mut ui, Some(&other));
        let (_, fresh) = ui.foot.bar.expect("cut too");
        assert_eq!(fresh.offset, 0);
    }

    /// FAILS IF: with the header shown, a kept foot draws the door's message
    /// or question as well — line 0 says them, so they would be said twice —
    /// or stops explaining while they are up.
    #[test]
    fn with_the_header_shown_the_message_stays_on_line_zero() {
        let mut ui = kept_ui();
        ui.hinted.hide_header(false);
        ui.foot.rows = wanted(&ui);
        ui.recount();
        ui.say("It was saved".to_owned());
        let (pieces, carries) = crate::door::list::line_zero(&mut ui, None, |_| Vec::new());
        assert!(carries.speaks(), "line 0 said {pieces:?}");
        assert!(pieces.iter().any(|p| p.text.contains("It was saved")));
        let e = explained("step", "A sentence.");
        let d = painted(&mut ui, Some(&e));
        assert!(
            d.spans.iter().all(|s| !s.text.contains("It was saved")),
            "said twice"
        );
        assert!(d.spans.iter().any(|s| s.text == e.name));
        ui.ask(crate::line::Line::new("software-web-new", false, None));
        let d = painted(&mut ui, Some(&e));
        assert!(d.spans.iter().any(|s| s.text == e.name), "{:?}", d.spans);
        assert!(ui.foot.marks.is_empty(), "the question's keys drawn twice");
    }

    /// FAILS IF: the three gestures do not say what will happen, or the key
    /// is not the code's notation (a word of the catalogue would go stale).
    #[test]
    fn a_gesture_says_the_key_and_what_it_does() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let ret = gesture(&texts, Gesture::Ret, "it happens");
        let keys =
            crate::chrome::key_text(&ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Enter));
        assert!(ret.contains(&keys) && ret.contains("it happens"), "{ret}");
        for how in [Gesture::Selects, Gesture::AtOnce] {
            let said = gesture(&texts, how, "it happens");
            assert!(
                said.contains("it happens") && !said.contains(&keys),
                "{said}"
            );
        }
        assert_ne!(
            gesture(&texts, Gesture::Selects, "x"),
            gesture(&texts, Gesture::AtOnce, "x")
        );
    }
}
