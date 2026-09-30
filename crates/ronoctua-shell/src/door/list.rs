//! THE STANDARD WINDOW: a header on line 0 and a list under it.
//!
//! Fifteen doors were this shape, each with its own copy of what is here —
//! and so this is the default of [`Content::paint`] and [`Content::pointer`].
//! A window that draws something else overrides those two, and may still call
//! [`header`] and [`body`] for the halves that are standard.

use super::{Canvas, Content, Hand, LEFT_BUTTON, RIGHT_BUTTON, Ui};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

/// Line 0 and the drawer it opens into: the window's shortcuts, or what the
/// last order said.
///
/// ONE PAINTER for every door — see [`crate::chrome::Hinted`].
pub fn header<C: Content>(c: &C, ui: &mut Ui, canvas: &mut Canvas<'_>) {
    header_of(ui, canvas, None, |ui| c.hint(ui));
}

/// The same header, with a LABEL and a TITLE before the shortcuts — the
/// manual's `MANUAL: Gestures` — drawn the way every title of the system is.
///
/// `hint` is asked only when there is neither a question nor a message: the
/// line carries one of the three, and a question outranks the others — it is
/// what the keyboard is typing into ([`the_question`]).
pub fn header_of(
    ui: &mut Ui,
    canvas: &mut Canvas<'_>,
    title: Option<crate::chrome::Title<'_>>,
    hint: impl FnOnce(&Ui) -> Vec<crate::chrome::Piece>,
) {
    header_about(ui, canvas, title, None, hint);
}

/// The same header, with what the ROW UNDER THE POINTER has to add — the
/// whole sentence a row carries for somebody who has not met the word on it
/// (`bluetooth::Row::description`), which the footer puts on its message line
/// under the pointer and a window puts here.
///
/// Below the question and the message, above the shortcuts: pointing at a row
/// is a question asked now, and the keys are always there to come back to —
/// the footer's own order on its line.
pub fn header_about(
    ui: &mut Ui,
    canvas: &mut Canvas<'_>,
    title: Option<crate::chrome::Title<'_>>,
    about: Option<String>,
    hint: impl FnOnce(&Ui) -> Vec<crate::chrome::Piece>,
) {
    let (pieces, hinting) = line_zero(ui, about, hint);
    let pieces = match title {
        Some(t) => t.before(pieces),
        None => pieces,
    };
    let (margin, columns) = (ui.margin, ui.columns);
    let texts = ui.texts.clone();
    canvas
        .spans
        .extend(ui.hinted.head(&pieces, hinting, margin, columns, &texts));
    // AND THE DRAWER'S OWN BAR, when it is open and the wrap ran past it.
    canvas.over.extend(ui.hinted.bar(ui.frame(0)));
}

/// What line 0 carries, and whether its pieces are shortcuts: the question
/// being typed, else the last order's message, else what the row under the
/// pointer has to add, else the keys.
pub fn line_zero(
    ui: &mut Ui,
    about: Option<String>,
    hint: impl FnOnce(&Ui) -> Vec<crate::chrome::Piece>,
) -> (Vec<crate::chrome::Piece>, bool) {
    // ITS `ESC` AND `RET` ARE SHORTCUTS: marked where they land, lit under
    // the pointer, pressed by a click (invariant 19).
    if let Some(pieces) = the_question(ui) {
        return (pieces, true);
    }
    // A SENTENCE TAKES THIS LINE, and then there is no shortcut on it to
    // point at. Decided BEFORE the list is measured, because the
    // explanation's row comes out of that measurement.
    match ui.message.clone().or(about) {
        Some(text) => {
            ui.hinted.say(None);
            (vec![crate::chrome::Piece::said(text)], false)
        }
        None => (hint(ui), true),
    }
}

/// THE QUESTION ON LINE 0, while [`Ui::ask`] has one up: what is asked, what
/// is typed — masked for a secret — and `ESC` and `RET` as clicks.
///
/// THE TYPED TEXT KEEPS ITS END. It is cut from the START to what the line
/// has left after the question and the two keys, because its end is where
/// the next letter goes: a line cut at its end would hide what the hand is
/// doing. The question comes first and the keys after, the minibuffer's
/// order — and at a width where not even that fits, the rest wraps into the
/// header's drawer like any other line.
///
/// Public for a window that paints its own header (`hinted.head`) and asks
/// this before its message and its shortcuts, as [`header_of`] does.
#[must_use]
pub fn the_question(ui: &Ui) -> Option<Vec<crate::chrome::Piece>> {
    let line = ui.asking.as_ref()?;
    Some(question_pieces(line, &ui.texts, ui.columns))
}

/// A question's line as pieces, `columns` wide: the ONE builder, which the
/// footer's minibuffer draws its question with too — the two doors ask the
/// same line, so they draw it the same way, with the same two clicks.
#[must_use]
pub fn question_pieces(
    line: &crate::line::Line,
    texts: &ronoctua_i18n::Texts,
    columns: u16,
) -> Vec<crate::chrome::Piece> {
    use ronoctua_render::layout::{elide_end, elide_start, width};
    let keys = crate::chrome::hint_of(crate::line::OF_THE_LINE, texts);
    let question = line.question(texts);
    let asked = format!("{question}: ");
    let separator = crate::chrome::Piece::separator();
    let taken = width(&asked)
        + width(&separator.text)
        + keys.iter().map(|p| width(&p.text)).sum::<u16>()
        + crate::fold::WIDTH;
    // HALF THE LINE AT LEAST, so a long question on a narrow window still
    // shows the last letters typed; the keys then wrap into the drawer rather
    // than hide them.
    let room = columns.saturating_sub(taken).max(columns / 2);
    let typed = elide_start(&line.shown(), room);
    // AND THE QUESTION GIVES WAY TO IT: the line is cut at its end, so a
    // question as wide as a narrow column pushed what is typed past the cut,
    // and the hand typed into nothing. The question is cut instead — its
    // first words say what it is — and what is typed keeps its place.
    let usable = columns.saturating_sub(crate::fold::WIDTH);
    let for_the_question = usable.saturating_sub(width(&typed).saturating_add(1));
    let asked = if width(&asked) > for_the_question {
        let colon = width(": ");
        format!(
            "{}: ",
            elide_end(&question, for_the_question.saturating_sub(colon).max(1))
        )
    } else {
        asked
    };
    let mut pieces = vec![
        crate::chrome::Piece::prose(asked),
        crate::chrome::Piece::said(typed),
        separator,
    ];
    pieces.extend(keys);
    pieces
}

/// The list under the header: its rows, every cursor's hover, and its bar.
///
/// The bar is written down in `ui.room` as it is drawn, which is what gives
/// it its own column for the pointer — see [`crate::chrome::Room`].
pub fn body(ui: &mut Ui, canvas: &mut Canvas<'_>, rows: &[Vec<crate::files::Field>]) {
    let size = ronoctua_tokens::text::size();
    ui.settle_offset(rows.len());
    let first = ui.first_of_list();
    let room = ui.room_for_rows();
    for (i, fields) in rows.iter().skip(ui.offset).take(room).enumerate() {
        let y = first + u16::try_from(i).unwrap_or(0);
        for f in fields {
            canvas.spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text: f.text.clone(),
                column: ui.margin + f.at,
                line: y,
                color: f.color,
                edge: false,
                foot: false,
            });
        }
    }
    let scroll = crate::chrome::list_scroll(rows.len(), room, ui.offset);
    #[allow(clippy::cast_precision_loss)]
    let (w, h) = (ui.width as f32, ui.height as f32);
    ui.room = crate::chrome::Room::of(scroll, w, h);
    // EVERY CURSOR'S HOVER, in its own colour — and to the BAR, never under it.
    canvas.under.extend(crate::chrome::hover_bands(
        canvas.lit.iter().copied(),
        ui.offset,
        room,
        rows.len(),
        first,
        ui.room.band_width(),
    ));
    canvas
        .under
        .extend(crate::chrome::list_bar(scroll, first, w, h));
    // AND ITS TRACK IS RECORDED: the column is the bar's (`crate::bars`).
    canvas.bar(crate::chrome::list_track(scroll, first, w, h));
}

/// The default frame: [`header`], then [`body`].
pub fn paint<C: Content>(c: &mut C, ui: &mut Ui, canvas: &mut Canvas<'_>) {
    header(c, ui, canvas);
    let rows = c.rows(ui);
    body(ui, canvas, &rows);
}

/// What one pointer did to the header, the bar and the list.
///
/// The order is the rule every door of this shape kept: the header's own
/// controls first, then the list's bar, then a row — so each control keeps the
/// meaning it has.
pub fn pointer<C: Content>(
    c: &mut C,
    ui: &mut Ui,
    hand: &mut Hand<C::Hand>,
    events: &[PointerEvent],
) {
    ui.hinted.swap_hand(&mut hand.hint_drag);
    pointer_here(c, ui, hand, events);
    ui.hinted.swap_hand(&mut hand.hint_drag);
}

fn pointer_here<C: Content>(
    c: &mut C,
    ui: &mut Ui,
    hand: &mut Hand<C::Hand>,
    events: &[PointerEvent],
) {
    let before = hand.row_under_pointer;
    let mut clicked: Option<usize> = None;
    let mut right: Option<usize> = None;
    let mut turned = false;
    // THE ROWS ARE ONLY COUNTED WHEN THE BAR IS IN PLAY: a `Motion` arrives on
    // every pixel of movement, and building the whole list to answer "is the
    // pointer on the bar?" would spend a frame's work per pixel.
    let bar_in_play = hand.dragging
        || events.iter().any(
            |e| matches!(e.kind, PointerEventKind::Press { button, .. } if button == LEFT_BUTTON),
        );
    let first = ui.first_of_list();
    let scroll = if bar_in_play {
        crate::chrome::list_scroll(c.rows(ui).len(), ui.room_for_rows(), ui.offset)
    } else {
        crate::chrome::list_scroll(0, 0, 0)
    };
    #[allow(clippy::cast_precision_loss)]
    let (bar_w, bar_h) = (ui.width as f32, ui.height as f32);
    let head = ui.frame(0);
    let mut header = false;
    let mut wheel_at: Option<f64> = None;
    let mut dragged: Option<usize> = None;
    let mut explained = false;
    for e in events {
        match e.kind {
            PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                if ui.hinted.dragged(head, e.position.1) {
                    header = true;
                    continue;
                }
                let texts = ui.texts.clone();
                let now = ui
                    .hinted
                    .under(e.position.0, e.position.1)
                    .and_then(|id| crate::chrome::hint_sentence(id, &texts));
                explained |= ui.hinted.say(now);
                #[allow(clippy::cast_possible_truncation)]
                if hand.dragging {
                    dragged = Some(crate::chrome::list_offset_at(
                        scroll,
                        first,
                        bar_w,
                        bar_h,
                        e.position.1 as f32,
                    ));
                } else {
                    hand.row_under_pointer = ui.row_at(e.position.0, e.position.1);
                }
            }
            PointerEventKind::Leave { .. } => {
                ui.hinted.let_go();
                explained |= ui.hinted.say(None);
                // AND THE DRAG ENDS WITH THE POINTER: a `dragging` left true
                // makes the next ENTER, with no button held, jump the list to
                // wherever the pointer came in.
                hand.dragging = false;
                hand.row_under_pointer = None;
            }
            PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                ui.hinted.let_go();
                hand.dragging = false;
            }
            PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                if ui.hinted.pressed(head, e.position.0, e.position.1) {
                    header = true;
                    continue;
                }
                #[allow(clippy::cast_possible_truncation)]
                if crate::chrome::on_the_list_bar(
                    scroll,
                    first,
                    bar_w,
                    bar_h,
                    e.position.0 as f32,
                    e.position.1 as f32,
                ) {
                    hand.dragging = true;
                    dragged = Some(crate::chrome::list_offset_at(
                        scroll,
                        first,
                        bar_w,
                        bar_h,
                        e.position.1 as f32,
                    ));
                    continue;
                }
                hand.row_under_pointer = ui.row_at(e.position.0, e.position.1);
                clicked = hand.row_under_pointer;
            }
            PointerEventKind::Press { button, .. } if button == RIGHT_BUTTON => {
                hand.row_under_pointer = ui.row_at(e.position.0, e.position.1);
                right = hand.row_under_pointer;
            }
            PointerEventKind::Axis {
                vertical,
                horizontal,
                ..
            } => {
                crate::surface::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
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
    if let Some(offset) = dragged {
        ui.offset = offset;
        ui.ask_paint();
        return;
    }
    // THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19. Which of the
    // two a click is, is the window's (`Content::clicked`); following the
    // cursor after it is everybody's.
    if let Some(row) = clicked.or(right) {
        if clicked.is_some() {
            c.clicked(ui, row);
        } else {
            c.right_clicked(ui, row);
        }
        if let Some(cursor) = c.cursor() {
            let rows = c.rows(ui).len();
            ui.follow(cursor, rows);
        }
        ui.ask_paint();
        return;
    }
    if turned {
        let by = crate::surface::wheel_lines(&mut hand.lines_owed);
        // A WHEEL OVER THE OPEN DRAWER WALKS THE DRAWER, and not the list
        // under it: the gesture belongs to what is under the pointer.
        if by != 0 && wheel_at.is_some_and(|y| ui.hinted.turned(head, y, by)) {
            ui.ask_paint();
            return;
        }
        if by == 0 {
            // WHAT IS LEFT OVER STAYS: a gesture too small to be a line yet has
            // not finished adding up.
            return;
        }
        let rows = c.rows(ui).len();
        let last = rows.saturating_sub(ui.room_for_rows());
        ui.offset = usize::try_from(
            i64::try_from(ui.offset)
                .unwrap_or(0)
                .saturating_add(by)
                .max(0),
        )
        .unwrap_or(0)
        .min(last);
        ui.ask_paint();
        return;
    }
    if explained {
        ui.ask_paint();
    }
    // A FRAME ONLY WHEN SOMETHING THAT IS DRAWN MOVED: a pointer moving inside
    // the same row is not a new picture.
    if before != hand.row_under_pointer {
        ui.ask_paint();
    }
}
