//! The notification band — the line (or three) ABOVE the modeline.
//!
//! # Why above, and why not a toast
//!
//! The owner asked for a floating toast, was told invariant 14 forbids a popup,
//! and chose this instead: the footer's OWN surface grows upwards, exactly as
//! the expansion already grows downwards. One surface, no popup, no card.
//!
//! Above the modeline and not on it: the modeline has one line and it is the
//! line that says *where you are*. A notification landing there takes away the
//! only thing on screen that must never move.
//!
//! And because the surface is anchored at the bottom, growing upwards means the
//! modeline does not move either — the band appears over the content, and the
//! content is what gives way.
//!
//! # Why this module has no Wayland in it
//!
//! Everything here is `(what is showing, width) -> pieces with columns`. No
//! GPU, no surface, no state. Two consequences, and they are the same two
//! `layout.rs` claims: it can be tested headless, and it fits the frame budget
//! because it is integer arithmetic over the cell.

use ronoctua_notify::{Shown, eighths};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

/// What a click on a piece of the band does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// Mark it read and take it off the screen at once.
    ///
    /// The small circle the owner asked for. The keyboard reaches the same
    /// thing through `notif-dismiss`, which is what keeps the mouse from being
    /// the only way in.
    Read,
    /// Run action `n` of the notification.
    Action(usize),
    /// Take it off the band and open it WHOLE, in the manager.
    ///
    /// The glyph beside the envelope, and the button at the end of a line
    /// that had to be cut: the band is one line or three, and what it could
    /// not hold is what the open record exists for.
    Open,
}

/// One drawn piece of a band line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// First cell.
    pub at: u16,
    /// What is drawn, already elided.
    pub text: String,
    /// Which token colours it.
    pub color: tokens::Color,
    /// What a click does. `None` = it is text, not a control.
    pub hit: Option<Hit>,
    /// What the message line says while the pointer rests here.
    ///
    /// Already translated: this module is given the texts, because the
    /// alternative is the drawing carrying keys and arguments around.
    pub explain: Option<String>,
    /// Was it placed counting cells from the surface's RIGHT EDGE?
    ///
    /// The right-hand cluster is, and everything else is not. It travels on
    /// the piece because it is asked TWICE — by whoever paints it, which hands
    /// it to `Span::edge`, and by [`at`], which then reads the pointer through
    /// `chrome::at_the_edge`. The grid does not reach the border, so a run
    /// placed from the left sits `cell + remainder` from it rather than a
    /// cell, and the remainder changes with the window: the owner read exactly
    /// that beside the glance's countdown.
    pub edge: bool,
}

/// One line of the band.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    /// Left to right.
    pub pieces: Vec<Piece>,
}

/// The texts the band needs, resolved once.
///
/// A struct and not five arguments: they are always wanted together, and a
/// caller that forgot one would get `[key]` on screen rather than a compile
/// error.
pub struct Words<'a> {
    /// Where the sentences come from.
    pub texts: &'a ronoctua_i18n::Texts,
}

/// How many lines the band takes for what is showing.
///
/// # Why it does not depend on the width
///
/// Because the footer's height is the same on every output (see
/// `Footer::apply_geometry`): a band that wrapped to two lines on the laptop
/// and one on the monitor would make the buffers reflow when focus changed
/// screen, with nothing having changed in them.
///
/// So the extra lines are not wrapping — they are the BODY and the ACTIONS,
/// each on its own. `Show::Full` with neither is one line, which is the honest
/// answer: three lines of which two are empty is a band lying about its size.
#[must_use]
pub fn rows(shown: Option<&Shown>) -> u16 {
    let Some(s) = shown else { return 0 };
    match s.presentation.show {
        ronoctua_notify::Show::Hidden => 0,
        ronoctua_notify::Show::Brief => 1,
        ronoctua_notify::Show::Full => {
            1 + u16::from(!s.notification.body.is_empty())
                + u16::from(!s.notification.actions.is_empty())
        }
    }
}

/// The glyph a notification shows when nothing named one for it.
///
/// # Why it is never nothing
///
/// A line that begins with a sentence and a line that begins with a mark are
/// two shapes, and a list of notifications where some have a mark and some do
/// not reads as a list with holes in it. The mark is also the fastest answer to
/// "how much does this matter" — before the eye reaches a single word.
///
/// It is the SEVERITY, and not one neutral square, because the severity is the
/// only thing the protocol gives that says anything at all. A stranger that
/// shouted gets the shouting drawing.
#[must_use]
pub const fn generic_glyph(p: &ronoctua_notify::Presentation, u: ronoctua_notify::Urgency) -> char {
    match u {
        ronoctua_notify::Urgency::Critical => tokens::icon::ERROR,
        _ if p.loud => tokens::icon::WARNING,
        _ => tokens::icon::INFO,
    }
}

/// The glyph of whatever the notification is, custom or generic.
#[must_use]
pub fn glyph_of(s: &Shown) -> char {
    s.presentation
        .glyph
        .unwrap_or_else(|| generic_glyph(&s.presentation, s.notification.urgency))
}

/// The same, for a record in the manager's list.
///
/// One function and not two, because the band and the list are looking at the
/// same notification: a mark that differed between them would make the person
/// look twice to decide they are the same thing.
#[must_use]
pub fn glyph_of_record(r: &ronoctua_notify::Record) -> char {
    r.presentation
        .glyph
        .unwrap_or_else(|| generic_glyph(&r.presentation, r.notification.urgency))
}

/// The eighths pie for a fraction of the time left.
#[must_use]
pub const fn countdown_glyph(left: u8) -> char {
    match left {
        0 | 1 => tokens::icon::COUNTDOWN_ONE,
        2 => tokens::icon::COUNTDOWN_TWO,
        3 => tokens::icon::COUNTDOWN_THREE,
        4 => tokens::icon::COUNTDOWN_FOUR,
        5 => tokens::icon::COUNTDOWN_FIVE,
        6 => tokens::icon::COUNTDOWN_SIX,
        7 => tokens::icon::COUNTDOWN_SEVEN,
        _ => tokens::icon::COUNTDOWN_EIGHT,
    }
}

/// The band, laid out.
///
/// `fraction` is how much of its time the notification has left (1.0 → 0.0) and
/// `position` is `(which one, how many)` — `None` when there is at most one, so
/// the counter does not spend three columns saying `1/1`.
///
/// `blink_on` rides the clock the footer already beats to expire things; with
/// `blink` off it changes nothing.
///
/// `held` is the pointer resting on the band, which stops the clock — the pie
/// becomes a pause. It is passed in rather than worked out from `fraction`
/// standing still, because two equal fractions a frame apart are also what a
/// slow notification looks like.
#[must_use]
pub fn lines(
    shown: Option<&Shown>,
    columns: u16,
    fraction: Option<f32>,
    position: Option<(usize, usize)>,
    blink_on: bool,
    held: bool,
    words: &Words<'_>,
) -> Vec<Line> {
    let Some(s) = shown else { return Vec::new() };
    let n = rows(shown);
    if n == 0 {
        return Vec::new();
    }

    // THE TWO FORMATTINGS. A copy that finished and a disk that is full must
    // not look alike, and the difference is colour and nothing else — no box,
    // no border, no card (invariant 15).
    let loud = s.presentation.loud;
    let tone = if loud && !(s.presentation.blink && !blink_on) {
        tokens::color::NOTIFICATION_LOUD
    } else {
        tokens::color::NOTIFICATION_QUIET
    };

    let left = tokens::space::MODELINE_EDGELEFT;
    let mut first = Line::default();

    // WHAT IT IS, in the margin — the notification's own glyph, or the generic
    // one for its severity. It is never nothing: see `generic_glyph`.
    first.pieces.push(Piece {
        at: left,
        text: glyph_of(s).to_string(),
        color: tone,
        hit: None,
        explain: None,
        edge: false,
    });
    let at = left + 2;

    // THE RIGHT-HAND CLUSTER FIRST, because it decides how much room the
    // sentence gets. Laying the text out first and then discovering the pie
    // does not fit is how a line comes to spill.
    //
    // From the edge inwards: the control, the pie, the counter. Every one of
    // them is placed by `chrome::from_the_edge` and carries `edge: true`, so
    // the ink sits an exact cell from the border rather than a cell plus the
    // grid's remainder — and `at` below asks the pointer the same way.
    let mut taken = 0u16;
    let mut right;

    // THE CONTROL IS THE FIRST THING ON THE RIGHT, at the edge.
    //
    // It sat at the LEFT margin and was a hollow circle, and both were wrong.
    // The left margin is where the sentence starts, so the control pushed the
    // words in and read as part of them; and a circle is a shape, not a verb —
    // it says "here is a control" and leaves what it does to be guessed.
    //
    // An open envelope is the drawing everybody already reads as "read", and
    // the screen's right edge is where a hand goes to dismiss something.
    taken += 1;
    right = crate::chrome::from_the_edge(columns, taken);
    first.pieces.push(Piece {
        at: right,
        text: tokens::icon::NOTIFICATION_MARK_READ.to_string(),
        color: tone,
        hit: Some(Hit::Read),
        explain: Some(words.texts.t(crate::key::HINT_NOTIF_READ)),
        edge: true,
    });

    // AND BESIDE IT, THE ONE THAT OPENS IT WHOLE, with a cell of air between
    // the two: two glyphs of one family in touching cells read as one
    // control.
    taken += 2;
    right = crate::chrome::from_the_edge(columns, taken);
    first.pieces.push(Piece {
        at: right,
        text: tokens::icon::NOTIFICATION_OPEN.to_string(),
        color: tone,
        hit: Some(Hit::Open),
        explain: Some(words.texts.t(crate::key::HINT_NOTIF_OPEN)),
        edge: true,
    });

    if let Some(f) = fraction {
        // THE SAME CELL SAYS BOTH THINGS: how much time is left, and that the
        // time is not moving. A frozen pie and a running one are the same
        // drawing for the second or two anybody watches, so the pointer's
        // effect would be invisible exactly while it is in force — and the
        // person would take their hand away to be sure, which is the one
        // gesture that undoes it.
        //
        // The COLOUR does not change with it. Two signals for one state is how
        // a row comes to twitch; the drawing is the state, and the message
        // line says it in words.
        let glyph = if held {
            tokens::icon::PAUSE
        } else {
            countdown_glyph(eighths(f))
        };
        taken += 2;
        right = crate::chrome::from_the_edge(columns, taken);
        first.pieces.push(Piece {
            at: right,
            text: glyph.to_string(),
            color: tokens::color::NOTIFICATION_COUNTDOWN,
            hit: None,
            explain: Some(words.texts.t(if held {
                crate::key::HINT_NOTIF_PAUSED
            } else {
                crate::key::HINT_NOTIF_COUNTDOWN
            })),
            edge: true,
        });
    }
    if let Some((shown_now, total)) = position {
        let text = words.texts.t_args(
            crate::key::NOTIF_QUEUE_POSITION,
            &[
                ("shown", ronoctua_i18n::Arg::Number(shown_now as i64)),
                ("total", ronoctua_i18n::Arg::Number(total as i64)),
            ],
        );
        let w = layout::width(&text);
        taken += w + 1;
        right = crate::chrome::from_the_edge(columns, taken);
        first.pieces.push(Piece {
            at: right,
            text,
            color: tokens::color::NOTIFICATION_COUNTDOWN,
            hit: None,
            explain: None,
            edge: true,
        });
    }

    // And now the sentence, in whatever is left.
    let room = right.saturating_sub(at).saturating_sub(1);
    first.pieces.extend(cut_or_whole(
        &s.notification.line(),
        at,
        room,
        if loud {
            tone
        } else {
            tokens::color::TEXT_PRIMARY
        },
        words,
    ));

    let mut out = vec![first];
    if n == 1 {
        return out;
    }

    // THE BODY, indented under the sentence. It is what one line could not
    // hold, which is the whole reason `Show::Full` exists.
    if !s.notification.body.is_empty() {
        let room = columns.saturating_sub(at + 1);
        // ONE LINE of it. A body with paragraphs belongs to the manager,
        // which scrolls; the band has a ceiling of three — and a body that
        // had more than one line was cut too, so it gets the button as well.
        let body = &s.notification.body;
        out.push(Line {
            pieces: cut_or_whole(
                &body.replace('\n', " "),
                at,
                room,
                tokens::color::NOTIFICATION_QUIET,
                words,
            ),
        });
    }

    // AND THE ACTIONS, each with the letter that runs it.
    //
    // The letter is not decoration: it is what makes the action reachable
    // without the mouse. It is pressed in the manager, which is where the
    // keyboard is — the footer at rest does not capture it, or no buffer would
    // ever receive a key.
    if !s.notification.actions.is_empty() {
        let mut pieces = Vec::new();
        let mut x = at;
        for (i, a) in s.notification.actions.iter().enumerate() {
            let label = match &a.label {
                ronoctua_notify::Label::Key(k) => words.texts.t(k),
                // A foreign app's, in whatever language it chose. We do not
                // know what it says, so we do not pretend to translate it.
                ronoctua_notify::Label::Text(t) => t.clone(),
            };
            // NO LETTER IN FRONT OF IT any more. An action used to carry one
            // and draw it here as `[b] bluetooth`, and the letter only ever
            // worked in the MANAGER — the band cannot take a key at all, since
            // the footer at rest does not capture the keyboard. So it was a
            // key taught on one screen and pressed on another, in a manager
            // that already teaches a dozen and will teach more. The arrows
            // reach an action now, in the open record.
            let text = label.clone();
            let w = layout::width(&text);
            if x + w >= columns {
                break;
            }
            // A DOT BETWEEN ONE ACTION AND THE NEXT, and it is the same dot
            // the hint line has always drawn.
            //
            // THE DEFECT, in the owner's words, off a photograph of this very
            // band: *"as várias ações ficam alinhadas, como esperado, porém
            // acaba dando a impressão visual de que é uma grande linha de
            // texto, em vez de várias ações separadas"*. Four labels in one
            // tone with two cells between them read as one sentence — *Edit
            // the image Open the image Open the folder Copy the path* — and
            // an action's label IS a sentence, so nothing in the words
            // themselves says where one ends.
            //
            // `mark.separator` in the separator's own tone, which is what the
            // hint line means by a dot: a mark that is neither of the things
            // it stands between, and quieter than both. Anything else — a
            // bracket, a pipe, a box — would be a second vocabulary for the
            // same job, and a box is refused outright (invariant 15).
            //
            // ONLY BETWEEN, never before the first: a dot with nothing on one
            // side of it separates nothing, which is the argument
            // `Piece::separator` already makes on the hint line.
            //
            // AND ONLY WHERE THEY ARE SIDE BY SIDE. The open record in the
            // manager draws its actions one per ROW, and there the row is
            // already the boundary — see `notifications::View::rows`, which
            // does not come through here.
            if i > 0 {
                let mark = tokens::mark::SEPARATOR.to_string();
                let mark_wide = layout::width(&mark);
                if x + mark_wide + 1 + w >= columns {
                    break;
                }
                pieces.push(Piece {
                    at: x,
                    text: mark,
                    color: crate::chrome::Register::Separator.color(),
                    // IT IS NOT AN ACTION, so the pointer finds nothing on it
                    // and nothing is said about it. A click that ran the
                    // action to its left because the dot belongs to it would
                    // be a target the eye cannot see.
                    hit: None,
                    explain: None,
                    edge: false,
                });
                x += mark_wide + tokens::space::MODELINE_SEGMENTGAP;
            }
            pieces.push(Piece {
                at: x,
                text,
                // NOT THE MESSAGE'S COLOUR. It was, and the owner read it off
                // the screen: the one thing on the card that can be PRESSED
                // was drawn in the same tone as the sentence above it, so it
                // looked like one more line of the message. There is no box
                // and no border to say otherwise (invariant 15), which leaves
                // colour as the only thing that can — and this is the same
                // token the open record already draws its actions in, so the
                // two places agree about the same button.
                color: tokens::color::NOTIFICATION_ACTS,
                hit: Some(Hit::Action(i)),
                explain: Some(
                    words
                        .texts
                        .t_with(crate::key::HINT_NOTIF_ACTION, &[("action", &label)]),
                ),
                edge: false,
            });
            x += w + tokens::space::MODELINE_SEGMENTGAP;
        }
        out.push(Line { pieces });
    }
    out
}

/// A line of the band, whole when it fits, and cut with a button at its end
/// when it does not.
///
/// THE BUTTON'S CELLS COME OFF THE ROOM FIRST, the rule of the right-hand
/// cluster: measured after the text, the button would sit on its last word.
/// It follows the cut text with one cell of air, in the actions' tone,
/// because it is a thing to press and colour is the only thing that can say
/// so here (invariant 15).
fn cut_or_whole(
    text: &str,
    at: u16,
    room: u16,
    color: tokens::Color,
    words: &Words<'_>,
) -> Vec<Piece> {
    let plain = |t: String| Piece {
        at,
        text: t,
        color,
        hit: None,
        explain: None,
        edge: false,
    };
    if layout::width(text) <= room {
        return vec![plain(text.to_owned())];
    }
    let button = words.texts.t(crate::key::NOTIF_SEE_WHOLE);
    let wide = layout::width(&button);
    // A ROOM TOO NARROW FOR BOTH keeps the text: a button with nothing
    // before it would open a record whose first word nobody saw.
    let Some(left) = room.checked_sub(wide + 1).filter(|l| *l >= wide) else {
        return vec![plain(layout::elide_end(text, room))];
    };
    let cut = layout::elide_end(text, left);
    let after = at + layout::width(&cut) + 1;
    vec![
        plain(cut),
        Piece {
            at: after,
            text: button,
            color: tokens::color::NOTIFICATION_ACTS,
            hit: Some(Hit::Open),
            explain: Some(words.texts.t(crate::key::HINT_NOTIF_OPEN)),
            edge: false,
        },
    ]
}

/// Which piece of the band is at cell `column` of line `row`.
///
/// The pointer's answer comes from what was DRAWN, and not from a second
/// calculation of where things would be — the same rule the modeline's
/// segments already follow.
#[must_use]
pub fn at(lines: &[Line], row: u16, column: u16, from_the_edge: Option<u16>) -> Option<&Piece> {
    lines.get(row as usize)?.pieces.iter().find(|p| {
        // THE READING THAT PLACED IT is the reading that finds it. A piece
        // drawn from the left is compared against the left-counted cell; one
        // anchored to the border is compared against the cell counted from
        // the border, because the grid's remainder sits between them and can
        // be most of a cell. Comparing both against the same number is how a
        // control comes to answer one cell away from where it is drawn —
        // `paint::bar_box` was rebuilt for exactly this.
        let at = if p.edge { from_the_edge } else { Some(column) };
        at.is_some_and(|c| c >= p.at && c < p.at + layout::width(&p.text))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_notify::{Notification, Presentation, Show, Urgency};
    use std::time::Instant;

    fn words() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn shown(show: Show, body: &str, actions: usize) -> Shown {
        let mut n = Notification::fresh_one(
            1,
            "screen".into(),
            None,
            "Screen captured".into(),
            body.into(),
            Urgency::Normal,
        );
        n.actions = (0..actions)
            .map(|i| ronoctua_notify::Action {
                id: format!("a{i}"),
                label: ronoctua_notify::Label::Text(format!("action {i}")),
                doing: ronoctua_notify::Doing::Invoke,
            })
            .collect();
        let mut p = Presentation::from_urgency(Urgency::Normal);
        p.show = show;
        Shown {
            notification: n,
            presentation: p,
            since: Instant::now(),
        }
    }

    #[test]
    fn the_band_is_one_line_or_three_and_never_more() {
        assert_eq!(rows(None), 0);
        assert_eq!(rows(Some(&shown(Show::Hidden, "corpo", 2))), 0);
        assert_eq!(rows(Some(&shown(Show::Brief, "corpo", 2))), 1);
        assert_eq!(rows(Some(&shown(Show::Full, "corpo", 2))), 3);
        // AND IT SHRINKS when there is nothing to put on the extra lines.
        // Three lines of which two are empty is a band lying about its size.
        assert_eq!(rows(Some(&shown(Show::Full, "", 0))), 1);
        assert_eq!(rows(Some(&shown(Show::Full, "corpo", 0))), 2);
    }

    #[test]
    fn nothing_spills_off_the_line() {
        // The same property the modeline's layout is tested for, and for the
        // same reason: a compositor does not clip, so what spills is rubbish at
        // the screen's edge.
        let t = words();
        let w = Words { texts: &t };
        for columns in [20_u16, 40, 80, 200] {
            let s = shown(Show::Full, &"c".repeat(500), 4);
            for line in lines(Some(&s), columns, Some(0.5), Some((1, 5)), true, false, &w) {
                for p in &line.pieces {
                    assert!(
                        p.at + layout::width(&p.text) <= columns,
                        "'{}' leaks at {columns} columns",
                        p.text
                    );
                }
            }
        }
    }

    #[test]
    fn beside_the_envelope_one_glyph_opens_it_whole() {
        let t = words();
        let w = Words { texts: &t };
        let s = shown(Show::Brief, "", 0);
        let out = lines(Some(&s), 80, Some(1.0), None, false, false, &w);
        let read = out[0]
            .pieces
            .iter()
            .find(|p| p.hit == Some(Hit::Read))
            .unwrap();
        let open = out[0]
            .pieces
            .iter()
            .find(|p| p.hit == Some(Hit::Open))
            .expect("the band lost the glyph that opens it whole");
        assert_eq!(open.text, tokens::icon::NOTIFICATION_OPEN.to_string());
        assert!(open.edge, "counted from the border, like the envelope");
        // ONE CELL OF AIR between the two glyphs, and nothing on top of the
        // other: the envelope is the edge's and this one is to its left.
        assert_eq!(open.at + 2, read.at, "{open:?} {read:?}");
        assert!(open.explain.is_some());
        // AND THE POINTER FINDS IT where it was drawn.
        let found = at(&out, 0, 0, Some(open.at)).map(|p| p.hit);
        assert_eq!(found, Some(Some(Hit::Open)));
    }

    #[test]
    fn a_cut_line_ends_in_a_button_and_a_whole_one_does_not() {
        let t = words();
        let w = Words { texts: &t };
        let whole = shown(Show::Full, "short", 0);
        let out = lines(Some(&whole), 80, None, None, false, false, &w);
        let buttons = |l: &Line| {
            l.pieces
                .iter()
                .filter(|p| p.hit == Some(Hit::Open) && !p.edge)
                .count()
        };
        assert_eq!(buttons(&out[0]), 0, "a line that fits has nothing to open");
        assert_eq!(buttons(&out[1]), 0);

        let long = shown(Show::Full, &"word ".repeat(60), 0);
        let out = lines(Some(&long), 80, None, None, false, false, &w);
        assert_eq!(buttons(&out[1]), 1, "the cut body ends in the button");
        let button = out[1]
            .pieces
            .iter()
            .find(|p| p.hit == Some(Hit::Open))
            .unwrap();
        let text = &out[1].pieces[0];
        assert!(
            text.text.ends_with('…'),
            "the body is cut before the button: {:?}",
            text.text
        );
        assert_eq!(
            button.at,
            text.at + layout::width(&text.text) + 1,
            "one cell of air after the cut text"
        );
        assert_eq!(button.color, tokens::color::NOTIFICATION_ACTS);
        assert!(button.at + layout::width(&button.text) <= 80);
        let found = at(&out, 1, button.at, None).map(|p| p.hit);
        assert_eq!(found, Some(Some(Hit::Open)));
    }

    #[test]
    fn the_control_is_at_the_right_edge_and_says_what_it_does() {
        // It sat at the LEFT margin and was a hollow circle, and both were
        // reported from the screen. The left margin is where the sentence
        // starts, and a circle is a shape and not a verb.
        let t = words();
        let w = Words { texts: &t };
        let s = shown(Show::Brief, "", 0);
        let out = lines(Some(&s), 80, Some(1.0), None, false, false, &w);
        let control = out[0]
            .pieces
            .iter()
            .find(|p| p.hit == Some(Hit::Read))
            .expect("the band lost its mark-as-read control");
        assert_eq!(
            control.text,
            tokens::icon::NOTIFICATION_MARK_READ.to_string()
        );
        assert!(control.explain.is_some(), "the pointer has to explain");
        // At the edge: the first thing on the right, and nothing to its right
        // but the margin.
        let edge = 80 - tokens::space::MODELINE_EDGERIGHT;
        assert_eq!(control.at, edge - 1, "the control is not at the edge");
        // And the left margin is the notification's own mark, not a control.
        let first = out[0].pieces.first().unwrap();
        assert_eq!(first.at, tokens::space::MODELINE_EDGELEFT);
        assert_eq!(first.hit, None);
    }

    #[test]
    fn a_notification_with_no_icon_still_has_one() {
        // The owner's ask: what has no custom glyph gets a generic one. A list
        // where some rows begin with a mark and some do not reads as a list
        // with holes in it.
        let t = words();
        let w = Words { texts: &t };
        let mut s = shown(Show::Brief, "", 0);
        assert_eq!(s.presentation.glyph, None);
        assert_eq!(glyph_of(&s), tokens::icon::INFO);
        // Loud, and critical, get the drawings that say so.
        s.presentation.loud = true;
        assert_eq!(glyph_of(&s), tokens::icon::WARNING);
        s.notification.urgency = Urgency::Critical;
        assert_eq!(glyph_of(&s), tokens::icon::ERROR);
        // And a named one always wins.
        s.presentation.glyph = Some('\u{F0AA5}');
        assert_eq!(glyph_of(&s), '\u{F0AA5}');
        let out = lines(Some(&s), 80, None, None, false, false, &w);
        assert_eq!(out[0].pieces[0].text, "\u{F0AA5}");
    }

    #[test]
    fn the_counter_only_appears_with_more_than_one() {
        let t = words();
        let w = Words { texts: &t };
        let s = shown(Show::Brief, "", 0);
        let alone = lines(Some(&s), 80, Some(1.0), None, false, false, &w);
        let queued = lines(Some(&s), 80, Some(1.0), Some((1, 5)), false, false, &w);
        assert!(queued[0].pieces.len() > alone[0].pieces.len());
        assert!(queued[0].pieces.iter().any(|p| p.text.contains("1/5")));
    }

    #[test]
    fn the_pie_empties_as_the_time_goes() {
        // Eight steps, and never empty while it is still on screen.
        assert_eq!(countdown_glyph(8), tokens::icon::COUNTDOWN_EIGHT);
        assert_eq!(countdown_glyph(1), tokens::icon::COUNTDOWN_ONE);
        assert_eq!(countdown_glyph(0), tokens::icon::COUNTDOWN_ONE);
        assert_eq!(countdown_glyph(200), tokens::icon::COUNTDOWN_EIGHT);
    }

    #[test]
    fn the_pie_becomes_a_pause_while_the_pointer_holds_it() {
        // THE SAME CELL SAYS BOTH THINGS. A frozen pie and a running one are
        // the same drawing for the second or two anybody watches, so without
        // this the pointer's whole effect would be invisible exactly while it
        // is in force — and the way to check would be to move the hand away,
        // which is the one gesture that undoes it.
        let t = words();
        let w = Words { texts: &t };
        let s = shown(Show::Brief, "", 0);

        let running = lines(Some(&s), 80, Some(0.5), None, false, false, &w);
        let held = lines(Some(&s), 80, Some(0.5), None, false, true, &w);
        let pie = |out: &[Line]| {
            out[0]
                .pieces
                .iter()
                .find(|p| p.color == tokens::color::NOTIFICATION_COUNTDOWN)
                .cloned()
                .expect("the countdown is drawn")
        };
        let (a, b) = (pie(&running), pie(&held));
        assert_eq!(a.text, countdown_glyph(eighths(0.5)).to_string());
        assert_eq!(b.text, tokens::icon::PAUSE.to_string());
        // AND IT DOES NOT MOVE. The two glyphs alternate in one cell, so a
        // different column would make the whole right-hand cluster twitch
        // every time a hand passed over the band.
        assert_eq!(a.at, b.at, "the pause landed in a different cell");
        // THE COLOUR IS THE SAME TOO: the drawing is the state, and two
        // signals for one state is how a row comes to flicker.
        assert_eq!(a.color, b.color);
        // AND THE MESSAGE LINE SAYS IT IN WORDS, because a glyph is a glyph.
        assert_ne!(a.explain, b.explain, "the pointer is told the same thing");
        assert_eq!(
            b.explain,
            Some(t.t(crate::key::HINT_NOTIF_PAUSED)),
            "the paused pie does not explain itself"
        );
        // AND NOTHING ELSE ON THE LINE MOVED.
        assert_eq!(running.len(), held.len());
        let cells = |out: &[Line]| out[0].pieces.iter().map(|p| p.at).collect::<Vec<_>>();
        assert_eq!(cells(&running), cells(&held));
    }

    #[test]
    fn blinking_only_moves_a_loud_one() {
        let t = words();
        let w = Words { texts: &t };
        let mut s = shown(Show::Brief, "", 0);
        s.presentation.loud = true;
        s.presentation.blink = true;
        let on = lines(Some(&s), 80, None, None, true, false, &w);
        let off = lines(Some(&s), 80, None, None, false, false, &w);
        assert_ne!(on[0].pieces[0].color, off[0].pieces[0].color);
        // With blink off, the clock changes nothing — otherwise every
        // notification would flicker once a second.
        s.presentation.blink = false;
        let a = lines(Some(&s), 80, None, None, true, false, &w);
        let b = lines(Some(&s), 80, None, None, false, false, &w);
        assert_eq!(a[0].pieces[0].color, b[0].pieces[0].color);
    }

    #[test]
    fn an_action_is_not_drawn_in_the_message_s_colour() {
        // THE OWNER READ IT OFF THE SCREEN: the one thing on the card that can
        // be PRESSED was the same tone as the sentence above it, so it looked
        // like one more line of the message. There is no box and no border to
        // say otherwise (invariant 15), which leaves colour as the only thing
        // that can.
        //
        // THIS IS HALF THE PROMISE, and deliberately: a `Color` is a ROLE, so
        // what is asserted here is that the two rows ask for different roles.
        // That the two roles RESOLVE to different ink in every theme is the
        // palette's own gate — `notification.quiet vs notification.acts` in
        // `gen-theme.py`, which fails the build at ΔE < 0.04. Neither half
        // alone is the guarantee: one role drawn twice would pass the gate,
        // and two roles on one primitive would pass this.
        let t = words();
        let w = Words { texts: &t };
        let s = shown(Show::Full, "a body", 2);
        let out = lines(Some(&s), 120, None, None, false, false, &w);
        let body = out[1].pieces.first().expect("the body is drawn");
        let action = out[2].pieces.first().expect("an action is drawn");
        assert_eq!(body.color, tokens::color::NOTIFICATION_QUIET);
        assert_eq!(action.color, tokens::color::NOTIFICATION_ACTS);
        assert_ne!(
            action.color, body.color,
            "the action wears the message's own colour"
        );
    }

    #[test]
    fn every_action_is_clickable_and_says_only_its_words() {
        // EVERY ONE IS A TARGET, and none of them carries a letter.
        //
        // They used to be drawn `[b] bluetooth`, and the letter only ever
        // worked in the MANAGER — the band cannot take a key at all, since the
        // footer at rest does not capture the keyboard. A key taught on one
        // screen and pressed on another, in a manager that already teaches a
        // dozen: the owner asked for it gone, and the arrows reach an action
        // in the open record instead.
        let t = words();
        let w = Words { texts: &t };
        let s = shown(Show::Full, "corpo", 3);
        let out = lines(Some(&s), 120, Some(1.0), None, false, false, &w);
        let last = out.last().unwrap();
        // THREE ACTIONS AND THE TWO DOTS BETWEEN THEM.
        let acts: Vec<_> = last.pieces.iter().filter(|p| !is_the_mark(p)).collect();
        assert_eq!(acts.len(), 3);
        for (i, p) in acts.iter().enumerate() {
            assert_eq!(p.hit, Some(Hit::Action(i)));
            assert!(
                !p.text.starts_with('['),
                "{:?} still carries a letter",
                p.text
            );
            assert!(
                p.explain.is_some(),
                "{:?} does not say what it does",
                p.text
            );
        }
    }

    /// Whether a drawn piece is the dot between two actions.
    fn is_the_mark(p: &Piece) -> bool {
        p.text.chars().eq(std::iter::once(tokens::mark::SEPARATOR))
    }

    /// FAILS IF: the actions go back to being one run of words.
    ///
    /// THE OWNER READ IT OFF A PHOTOGRAPH of this band: *"as várias ações
    /// ficam alinhadas, como esperado, porém acaba dando a impressão visual
    /// de que é uma grande linha de texto, em vez de várias ações
    /// separadas"*. Four labels in one tone with two cells between them read
    /// as one sentence, and an action's label IS a sentence — so nothing in
    /// the words says where one ends.
    #[test]
    fn one_action_is_told_from_the_next() {
        let t = words();
        let w = Words { texts: &t };
        let s = shown(Show::Full, "corpo", 3);
        let out = lines(Some(&s), 120, Some(1.0), None, false, false, &w);
        let last = out.last().unwrap();
        // BETWEEN, AND NOT BEFORE THE FIRST: a dot with nothing on one side of
        // it separates nothing.
        let marks: Vec<usize> = last
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, p)| is_the_mark(p))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(marks, vec![1, 3], "the dots are not between the actions");
        for at in marks {
            let mark = &last.pieces[at];
            // NEITHER OF THE THINGS IT STANDS BETWEEN, and quieter than both.
            assert_ne!(mark.color, tokens::color::NOTIFICATION_ACTS);
            assert_eq!(mark.color, crate::chrome::Register::Separator.color());
            // AND NOT A TARGET: a click that ran the action to its left would
            // be a target the eye cannot see.
            assert_eq!(mark.hit, None);
            assert_eq!(mark.explain, None);
        }
        // ONE ACTION NEEDS NO SEPARATOR AT ALL.
        let alone = shown(Show::Full, "corpo", 1);
        let out = lines(Some(&alone), 120, Some(1.0), None, false, false, &w);
        let last = out.last().unwrap();
        assert!(last.pieces.iter().all(|p| !is_the_mark(p)));
    }

    /// THE CONTROL IS FOUND BY THE READING THAT DREW IT, and by no other.
    ///
    /// THE DEFECT THIS CATCHES: the envelope is anchored to the border, so its
    /// ink sits at `width - 2 cells` while the left-counted grid puts cell
    /// `columns - 2` at `(columns - 2) x cell` — and the two differ by the
    /// grid's remainder, which is nought to one cell and changes with the
    /// window. Matching an edge-anchored piece against the left-counted cell
    /// is a control that answers a cell away from where it is drawn, which is
    /// the defect `paint::bar_box` was carved out to end.
    #[test]
    fn the_pointer_finds_what_was_drawn() {
        let t = words();
        let w = Words { texts: &t };
        let s = shown(Show::Full, "corpo", 2);
        let out = lines(Some(&s), 120, Some(1.0), None, false, false, &w);
        let control = out[0]
            .pieces
            .iter()
            .find(|p| p.hit == Some(Hit::Read))
            .expect("no control on the band");
        assert!(control.edge, "the control is not anchored to the border");
        assert_eq!(control.at, crate::chrome::from_the_edge(120, 1));
        assert_eq!(
            at(&out, 0, 0, Some(control.at)).and_then(|p| p.hit),
            Some(Hit::Read)
        );
        // AND NOT BY THE OTHER ONE. Without the edge reading it is unreachable
        // — which is honest, and is what says the two are not interchangeable.
        assert!(
            at(&out, 0, control.at, None).is_none(),
            "an edge-anchored control answered the left-counted cell"
        );
        // The left margin carries the notification's own mark, which is text.
        let left = tokens::space::MODELINE_EDGELEFT;
        assert_eq!(at(&out, 0, left, None).and_then(|p| p.hit), None);
        // A cell with nothing in it is nothing, and not the nearest thing.
        assert!(at(&out, 0, left + 1, None).is_none());
        assert!(at(&out, 9, 0, None).is_none());
    }
}
