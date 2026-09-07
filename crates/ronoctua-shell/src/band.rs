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
#[must_use]
pub fn lines(
    shown: Option<&Shown>,
    columns: u16,
    fraction: Option<f32>,
    position: Option<(usize, usize)>,
    blink_on: bool,
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
    });
    let at = left + 2;

    // THE RIGHT-HAND CLUSTER FIRST, because it decides how much room the
    // sentence gets. Laying the text out first and then discovering the pie
    // does not fit is how a line comes to spill.
    //
    // From the edge inwards: the control, the pie, the counter.
    let mut right = columns.saturating_sub(tokens::space::MODELINE_EDGERIGHT);

    // THE CONTROL IS THE FIRST THING ON THE RIGHT, at the edge.
    //
    // It sat at the LEFT margin and was a hollow circle, and both were wrong.
    // The left margin is where the sentence starts, so the control pushed the
    // words in and read as part of them; and a circle is a shape, not a verb —
    // it says "here is a control" and leaves what it does to be guessed.
    //
    // An open envelope is the drawing everybody already reads as "read", and
    // the screen's right edge is where a hand goes to dismiss something.
    right = right.saturating_sub(1);
    first.pieces.push(Piece {
        at: right,
        text: tokens::icon::NOTIFICATION_MARK_READ.to_string(),
        color: tone,
        hit: Some(Hit::Read),
        explain: Some(words.texts.t(ronoctua_i18n::key::HINT_NOTIF_READ)),
    });

    if let Some(f) = fraction {
        let glyph = countdown_glyph(eighths(f));
        right = right.saturating_sub(2);
        first.pieces.push(Piece {
            at: right,
            text: glyph.to_string(),
            color: tokens::color::NOTIFICATION_COUNTDOWN,
            hit: None,
            explain: Some(words.texts.t(ronoctua_i18n::key::HINT_NOTIF_COUNTDOWN)),
        });
    }
    if let Some((shown_now, total)) = position {
        let text = words.texts.t_args(
            ronoctua_i18n::key::NOTIF_QUEUE_POSITION,
            &[
                ("shown", ronoctua_i18n::Arg::Number(shown_now as i64)),
                ("total", ronoctua_i18n::Arg::Number(total as i64)),
            ],
        );
        let w = layout::width(&text);
        right = right.saturating_sub(w + 1);
        first.pieces.push(Piece {
            at: right,
            text,
            color: tokens::color::NOTIFICATION_COUNTDOWN,
            hit: None,
            explain: None,
        });
    }

    // And now the sentence, in whatever is left.
    let room = right.saturating_sub(at).saturating_sub(1);
    first.pieces.push(Piece {
        at,
        text: layout::elide_end(&s.notification.line(), room),
        color: if loud {
            tone
        } else {
            tokens::color::TEXT_PRIMARY
        },
        hit: None,
        explain: None,
    });

    let mut out = vec![first];
    if n == 1 {
        return out;
    }

    // THE BODY, indented under the sentence. It is what one line could not
    // hold, which is the whole reason `Show::Full` exists.
    if !s.notification.body.is_empty() {
        let room = columns.saturating_sub(at + 1);
        out.push(Line {
            pieces: vec![Piece {
                at,
                // ONE LINE of it. A body with paragraphs belongs to the
                // manager, which scrolls; the band has a ceiling of three.
                text: layout::elide_end(&s.notification.body.replace('\n', " "), room),
                color: tokens::color::NOTIFICATION_QUIET,
                hit: None,
                explain: None,
            }],
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
            pieces.push(Piece {
                at: x,
                text,
                color: tokens::color::NOTIFICATION_QUIET,
                hit: Some(Hit::Action(i)),
                explain: Some(
                    words
                        .texts
                        .t_with(ronoctua_i18n::key::HINT_NOTIF_ACTION, &[("action", &label)]),
                ),
            });
            x += w + tokens::space::MODELINE_SEGMENTGAP;
        }
        out.push(Line { pieces });
    }
    out
}

/// Which piece of the band is at cell `column` of line `row`.
///
/// The pointer's answer comes from what was DRAWN, and not from a second
/// calculation of where things would be — the same rule the modeline's
/// segments already follow.
#[must_use]
pub fn at(lines: &[Line], row: u16, column: u16) -> Option<&Piece> {
    lines
        .get(row as usize)?
        .pieces
        .iter()
        .find(|p| column >= p.at && column < p.at + layout::width(&p.text))
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
            "niri".into(),
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
            for line in lines(Some(&s), columns, Some(0.5), Some((1, 5)), true, &w) {
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
    fn the_control_is_at_the_right_edge_and_says_what_it_does() {
        // It sat at the LEFT margin and was a hollow circle, and both were
        // reported from the screen. The left margin is where the sentence
        // starts, and a circle is a shape and not a verb.
        let t = words();
        let w = Words { texts: &t };
        let s = shown(Show::Brief, "", 0);
        let out = lines(Some(&s), 80, Some(1.0), None, false, &w);
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
        let out = lines(Some(&s), 80, None, None, false, &w);
        assert_eq!(out[0].pieces[0].text, "\u{F0AA5}");
    }

    #[test]
    fn the_counter_only_appears_with_more_than_one() {
        let t = words();
        let w = Words { texts: &t };
        let s = shown(Show::Brief, "", 0);
        let alone = lines(Some(&s), 80, Some(1.0), None, false, &w);
        let queued = lines(Some(&s), 80, Some(1.0), Some((1, 5)), false, &w);
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
    fn blinking_only_moves_a_loud_one() {
        let t = words();
        let w = Words { texts: &t };
        let mut s = shown(Show::Brief, "", 0);
        s.presentation.loud = true;
        s.presentation.blink = true;
        let on = lines(Some(&s), 80, None, None, true, &w);
        let off = lines(Some(&s), 80, None, None, false, &w);
        assert_ne!(on[0].pieces[0].color, off[0].pieces[0].color);
        // With blink off, the clock changes nothing — otherwise every
        // notification would flicker once a second.
        s.presentation.blink = false;
        let a = lines(Some(&s), 80, None, None, true, &w);
        let b = lines(Some(&s), 80, None, None, false, &w);
        assert_eq!(a[0].pieces[0].color, b[0].pieces[0].color);
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
        let out = lines(Some(&s), 120, Some(1.0), None, false, &w);
        let last = out.last().unwrap();
        assert_eq!(last.pieces.len(), 3);
        for (i, p) in last.pieces.iter().enumerate() {
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

    #[test]
    fn the_pointer_finds_what_was_drawn() {
        let t = words();
        let w = Words { texts: &t };
        let s = shown(Show::Full, "corpo", 2);
        let out = lines(Some(&s), 120, Some(1.0), None, false, &w);
        // The control is at the right edge now, and the left margin carries the
        // notification's own mark.
        let edge = 120 - tokens::space::MODELINE_EDGERIGHT - 1;
        assert_eq!(at(&out, 0, edge).and_then(|p| p.hit), Some(Hit::Read));
        let left = tokens::space::MODELINE_EDGELEFT;
        assert_eq!(at(&out, 0, left).and_then(|p| p.hit), None);
        // A cell with nothing in it is nothing, and not the nearest thing.
        assert!(at(&out, 0, left + 1).is_none());
        assert!(at(&out, 9, 0).is_none());
    }
}
