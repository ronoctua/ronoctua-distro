//! The MAXIMUM ALERT BAR — the one line on screen that does not go away on
//! its own.
//!
//! # What it is, and what it is not
//!
//! A notification is a line that comes and goes on somebody else's schedule,
//! and the owner's rule for the band is that nothing on it stays for ever
//! (the notifications' `presentation`). This is the written exception (2026-09-30): a
//! state of the machine that will END something if nobody acts — the battery
//! a few minutes from empty — gets a bar that stays until the CAUSE ends or a
//! hand closes it. It stands where the notification band stands and takes its
//! place while it is up (`Footer::band_rows`): one surface growing upwards,
//! no popup, no card (invariants 14 and 15). A notification that arrives
//! meanwhile waits, held, and is shown after.
//!
//! # How it shouts
//!
//! By colour, which is the one device invariant 15 leaves: its ground and its
//! ink ALTERNATE between two alert roles (`alert.ground`/`alert.text` and
//! `alert.ground-alt`/`alert.text-alt`, solved and gated like every ground),
//! on a clock armed only while the bar is up (`space.alert.alternateTenths`).
//! Its title is in capitals — `to_uppercase` of the language, so an accent
//! stays an accent — and a message with no title is centred.
//!
//! # The first cause: the battery
//!
//! Below `space.alert.batteryBelowPct` percent, on battery and not charging.
//! Closed by hand, it comes back only after the battery has fallen another
//! `space.alert.batteryStepPct` — a person who closed it has read it, and a
//! bar that returned on the next reading would be a bar that cannot be
//! closed. Plugged in, or back above the line, and it goes; the next time the
//! battery falls that low is a new alert, whatever was closed before.
//!
//! # Why this module has no Wayland in it
//!
//! The band's reason (`crate::band`): `(what is up, width) -> pieces with
//! columns`, testable headless, integer arithmetic over the cell.

use crate::key;
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::band::{Hit, Line, Piece};

/// How many lines the bar takes: one. It is a sentence and a button.
pub const ROWS: u16 = 1;

/// What the alert is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// The battery is below the line, on battery, and not charging.
    BatteryLow {
        /// Its charge, 0–100.
        percent: u8,
    },
}

impl Cause {
    /// The title, uppercased at draw time; `None` for a message alone.
    const fn title(self) -> Option<&'static str> {
        match self {
            Self::BatteryLow { .. } => Some(key::ALERT_BATTERY_LOW),
        }
    }

    /// The message, in the language.
    fn message(self, texts: &ronoctua_i18n::Texts) -> String {
        match self {
            Self::BatteryLow { percent } => texts.t_args(
                key::ALERT_BATTERY_LOW_BODY,
                &[("percent", ronoctua_i18n::Arg::Number(i64::from(percent)))],
            ),
        }
    }

    /// The glyph in the margin.
    const fn glyph(self) -> char {
        match self {
            Self::BatteryLow { .. } => tokens::icon::ALERT_BATTERY,
        }
    }
}

/// What a battery reading says, reduced to what the alert reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    /// The charge, 0–100.
    pub percent: u8,
    /// Energy going in — charging, or held by a threshold on the charger.
    pub charging: bool,
    /// The machine is running on its battery (UPower's `OnBattery`).
    pub on_battery: bool,
}

impl Reading {
    /// The reading of a snapshot: its composite battery, when it has one.
    #[must_use]
    pub fn of(s: &ronoctua_power::Snapshot) -> Option<Self> {
        let b = s.display.as_ref().filter(|b| b.present)?;
        Some(Self {
            percent: b.percent(),
            charging: b.state.is_charging(),
            on_battery: s.on_battery,
        })
    }

    /// Whether this reading is the alert's cause: below the line, on
    /// battery, and not charging — all three, since a battery at 5% on the
    /// charger is being fixed already.
    #[must_use]
    pub const fn low(self) -> bool {
        self.percent < BELOW && self.on_battery && !self.charging
    }
}

/// The line, as a percentage (`space.alert.batteryBelowPct`).
const BELOW: u8 = narrow(tokens::space::ALERT_BATTERYBELOWPCT);

/// How far the battery falls, after a hand closed the bar, before it comes
/// back (`space.alert.batteryStepPct`).
const STEP: u8 = narrow(tokens::space::ALERT_BATTERYSTEPPCT);

/// A token's percentage as a `u8`, clamped at a hundred.
#[allow(clippy::cast_possible_truncation)]
const fn narrow(n: u16) -> u8 {
    if n > 100 { 100 } else { n as u8 }
}

/// What is up, and what a hand closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Alerts {
    /// The alert on screen.
    showing: Option<Cause>,
    /// The percentage the battery's alert was closed at, by hand — until the
    /// cause ends.
    closed_at: Option<u8>,
}

impl Alerts {
    /// The alert on screen, if any.
    #[must_use]
    pub const fn showing(&self) -> Option<Cause> {
        self.showing
    }

    /// A battery reading arrived — or none, a machine with no battery: the
    /// alert comes, stays with the new number, or goes. `true` when what is
    /// on screen changed.
    pub fn battery(&mut self, reading: Option<Reading>) -> bool {
        let was = self.showing;
        match reading.filter(|r| r.low()) {
            // THE CAUSE ENDED: the charger, a charge above the line, no
            // battery. What a hand closed is forgotten with it — the next
            // fall is a new alert.
            None => {
                self.closed_at = None;
                self.showing = None;
            }
            Some(r) => {
                let closed = self
                    .closed_at
                    .is_some_and(|at| r.percent.saturating_add(STEP) > at);
                self.showing = (!closed).then_some(Cause::BatteryLow { percent: r.percent });
                if !closed {
                    self.closed_at = None;
                }
            }
        }
        self.showing != was
    }

    /// The close button, or `alert-dismiss`: the bar goes, and remembers where
    /// it was closed. `true` when there was one.
    pub fn close(&mut self) -> bool {
        match self.showing.take() {
            Some(Cause::BatteryLow { percent }) => {
                self.closed_at = Some(percent);
                true
            }
            None => false,
        }
    }
}

/// The band's clock, held or let go: held while an alert stands on the band
/// or a pointer rests on it, running otherwise. `true` when that changed.
///
/// # Asked after every move of the queue, too
///
/// `Queue::hold` holds nothing while nothing is on the band — there is no
/// clock to stop — so an alert that came up over an empty band held nothing,
/// and a notification that arrived under it started its life unseen and ran
/// out behind the bar. The footer asks this after a pointer moves, after the
/// alert moves, AND after the queue moves (`Footer::after_the_queue_moved`),
/// so the one that arrives under the bar is held from its first instant.
pub fn hold_the_band(
    queue: &mut ronoctua_notify::Queue,
    alert_up: bool,
    read: bool,
    now: std::time::Instant,
) -> bool {
    if alert_up || read {
        queue.hold(now)
    } else {
        queue.release(now)
    }
}

/// The alternation: which of the two alert roles a phase wears — `(ground,
/// ink)`.
#[must_use]
pub const fn roles(phase: bool) -> (tokens::Color, tokens::Color) {
    if phase {
        (
            tokens::color::ALERT_GROUND_ALT,
            tokens::color::ALERT_TEXT_ALT,
        )
    } else {
        (tokens::color::ALERT_GROUND, tokens::color::ALERT_TEXT)
    }
}

/// How long one phase of the alternation lasts.
#[must_use]
pub fn phase_lasts() -> std::time::Duration {
    std::time::Duration::from_millis(u64::from(tokens::space::ALERT_ALTERNATETENTHS) * 100)
}

/// The bar's line, `columns` wide, in the ink of `phase`: the glyph in the
/// margin, the TITLE in capitals and the message after it — or a message
/// with no title, centred when it fits — and the close button at the edge.
#[must_use]
pub fn lines(cause: Cause, columns: u16, phase: bool, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
    lay_out(
        cause.title().map(|k| texts.t(k)),
        &cause.message(texts),
        cause.glyph(),
        columns,
        phase,
        texts,
    )
}

/// The layout itself, over resolved words — what the tests drive with a
/// title and without one.
fn lay_out(
    title: Option<String>,
    message: &str,
    glyph: char,
    columns: u16,
    phase: bool,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Line> {
    let (_, ink) = roles(phase);
    let piece =
        |at: u16, text: String, hit: Option<Hit>, explain: Option<String>, edge: bool| Piece {
            at,
            text,
            color: ink,
            hit,
            explain,
            edge,
        };
    let left = tokens::space::MODELINE_EDGELEFT;
    let mut line = Line::default();
    // THE CLOSE BUTTON FIRST, at the edge, because it decides the room: the
    // band's own rule (`band::lines`), for its reason — laying the text out
    // and then finding no room for the control is how a line spills.
    let close_at = crate::chrome::from_the_edge(columns, 1);
    line.pieces.push(piece(
        close_at,
        tokens::icon::ALERT_CLOSE.to_string(),
        Some(Hit::Read),
        Some(texts.t(key::HINT_ALERT_CLOSE)),
        true,
    ));
    // WHAT IS BETWEEN THE MARGIN AND THE BUTTON, with a cell of air on each
    // side of the words.
    let room = close_at.saturating_sub(left + 1);
    let Some(title) = title else {
        // A MESSAGE ALONE: centred when it fits, cut at the end when not.
        let width = layout::width(message);
        let at = if width <= room {
            left + (room - width) / 2
        } else {
            left
        };
        line.pieces.push(piece(
            at,
            layout::elide_end(message, room),
            None,
            None,
            false,
        ));
        return vec![line];
    };
    // THE GLYPH IN THE MARGIN, and the title in capitals after it — the
    // language's own capitals, so `ç` is `Ç`.
    line.pieces
        .push(piece(left, glyph.to_string(), None, None, false));
    let at = left + 2;
    let room = close_at.saturating_sub(at + 1);
    let title = title.to_uppercase();
    let title_width = layout::width(&title).min(room);
    line.pieces.push(piece(
        at,
        layout::elide_end(&title, room),
        None,
        None,
        false,
    ));
    // AND THE MESSAGE, two cells after the title, in what is left.
    let after = at + title_width + 2;
    let rest = close_at.saturating_sub(after + 1);
    if rest > 0 && !message.is_empty() {
        line.pieces.push(piece(
            after,
            layout::elide_end(message, rest),
            None,
            None,
            false,
        ));
    }
    vec![line]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn low(percent: u8) -> Option<Reading> {
        Some(Reading {
            percent,
            charging: false,
            on_battery: true,
        })
    }

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    /// FAILS IF: a notification that arrives while the alert is up — over an
    /// empty band, the usual case — runs its clock behind the bar and is
    /// gone before anybody saw it, or is not given its whole life once the
    /// alert goes.
    #[test]
    fn a_notification_that_arrives_under_the_alert_waits_for_it() {
        use ronoctua_notify::{Life, Notification, Presentation, Queue, Urgency};
        use std::time::Duration;
        let t0 = std::time::Instant::now();
        let mut queue = Queue::fresh_one();
        // THE ALERT COMES UP over an empty band: there is nothing to hold yet.
        assert!(!hold_the_band(&mut queue, true, false, t0));
        // A NOTIFICATION ARRIVES, and the queue moved: asked again.
        let mut p = Presentation::from_urgency(Urgency::Normal);
        p.life = Life::Normal;
        let life = p.life.how_long();
        queue.enqueue(
            Notification::fresh_one(
                7,
                "app".into(),
                None,
                "under the bar".into(),
                String::new(),
                Urgency::Normal,
            ),
            p,
            t0,
        );
        assert!(
            hold_the_band(&mut queue, true, false, t0),
            "the one that arrived under the alert was not held"
        );
        // LONG PAST ITS LIFE, it is still there.
        let later = t0 + life * 5;
        assert!(
            queue.tick(later).expired.is_none(),
            "it ran out behind the bar"
        );
        assert_eq!(queue.showing().map(|s| s.notification.id), Some(7));
        // THE ALERT GOES: its whole life starts now, and it goes after it.
        assert!(hold_the_band(&mut queue, false, false, later));
        assert!(
            queue.tick(later + life / 2).expired.is_none(),
            "it lost the time the alert held"
        );
        assert_eq!(
            queue.tick(later + life + Duration::from_secs(1)).expired,
            Some(7)
        );
    }

    /// FAILS IF: the battery's alert comes for anything but all three — below
    /// the line, on battery, not charging — or stays when one of them ends.
    #[test]
    fn the_battery_alert_is_below_the_line_on_battery_and_not_charging() {
        assert!(
            Reading {
                percent: BELOW - 1,
                charging: false,
                on_battery: true
            }
            .low()
        );
        assert!(
            !Reading {
                percent: BELOW,
                charging: false,
                on_battery: true
            }
            .low()
        );
        assert!(
            !Reading {
                percent: 3,
                charging: true,
                on_battery: true
            }
            .low()
        );
        assert!(
            !Reading {
                percent: 3,
                charging: false,
                on_battery: false
            }
            .low()
        );
        let mut a = Alerts::default();
        assert!(a.battery(low(9)));
        assert_eq!(a.showing(), Some(Cause::BatteryLow { percent: 9 }));
        // THE NUMBER FOLLOWS the battery while it is up.
        assert!(a.battery(low(8)));
        assert_eq!(a.showing(), Some(Cause::BatteryLow { percent: 8 }));
        assert!(!a.battery(low(8)), "the same reading changed nothing");
        // THE CHARGER, and it goes.
        assert!(a.battery(Some(Reading {
            percent: 8,
            charging: true,
            on_battery: false,
        })));
        assert_eq!(a.showing(), None);
        // AND A MACHINE WITH NO BATTERY never has it.
        assert!(!a.battery(None));
    }

    /// FAILS IF: a bar closed by hand comes back before the battery falls
    /// another step, does not come back after it, or what was closed outlives
    /// the cause — the next fall after a charge is a new alert.
    #[test]
    fn closed_by_hand_it_returns_only_a_step_lower() {
        let mut a = Alerts::default();
        a.battery(low(9));
        assert!(a.close());
        assert!(!a.close(), "nothing left to close");
        for p in (9 - STEP + 1..=9).rev() {
            a.battery(low(p));
            assert_eq!(a.showing(), None, "back at {p}%");
        }
        a.battery(low(9 - STEP));
        assert_eq!(a.showing(), Some(Cause::BatteryLow { percent: 9 - STEP }));
        // THE CAUSE ENDS, and what was closed is forgotten.
        a.close();
        a.battery(Some(Reading {
            percent: 40,
            charging: false,
            on_battery: true,
        }));
        a.battery(low(9));
        assert_eq!(a.showing(), Some(Cause::BatteryLow { percent: 9 }));
    }

    /// FAILS IF: the two phases wear the same roles, or a phase's ink is not
    /// the one measured on its ground.
    #[test]
    fn the_phases_alternate_the_alert_roles() {
        assert_ne!(roles(false), roles(true));
        assert_eq!(
            roles(false),
            (tokens::color::ALERT_GROUND, tokens::color::ALERT_TEXT)
        );
        assert_eq!(
            roles(true),
            (
                tokens::color::ALERT_GROUND_ALT,
                tokens::color::ALERT_TEXT_ALT
            )
        );
        assert!(phase_lasts() > std::time::Duration::ZERO);
    }

    /// FAILS IF: the title is not in the language's capitals, the close
    /// button is not the last cell from the edge or does not take the click,
    /// or a piece runs past the button.
    #[test]
    fn the_title_is_in_capitals_and_the_button_at_the_edge() {
        let t = texts();
        for columns in [40, 80, 200] {
            let lines = lines(Cause::BatteryLow { percent: 7 }, columns, false, &t);
            assert_eq!(lines.len(), usize::from(ROWS));
            let pieces = &lines[0].pieces;
            let close = pieces.iter().find(|p| p.hit.is_some()).expect("a button");
            assert_eq!(close.at, crate::chrome::from_the_edge(columns, 1));
            assert!(close.edge);
            assert_eq!(close.hit, Some(Hit::Read));
            for p in pieces.iter().filter(|p| p.hit.is_none()) {
                assert!(
                    p.at + layout::width(&p.text) < close.at,
                    "`{}` runs into the button at {columns}",
                    p.text
                );
            }
        }
        let wide = lines(Cause::BatteryLow { percent: 7 }, 200, false, &t);
        let title = t.t(key::ALERT_BATTERY_LOW).to_uppercase();
        assert!(wide[0].pieces.iter().any(|p| p.text == title), "{wide:?}");
        assert!(wide[0].pieces.iter().any(|p| p.text.contains('7')));
        // AND IN PORTUGUESE, whose capitals carry the accents.
        let pt = ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR");
        let lines_pt = lines(Cause::BatteryLow { percent: 7 }, 200, true, &pt);
        let title_pt = pt.t(key::ALERT_BATTERY_LOW).to_uppercase();
        assert!(lines_pt[0].pieces.iter().any(|p| p.text == title_pt));
        assert!(
            lines_pt[0]
                .pieces
                .iter()
                .all(|p| p.color == tokens::color::ALERT_TEXT_ALT)
        );
    }

    /// FAILS IF: a short message with no title is not centred between the
    /// margin and the button, or a long one is not cut at the end.
    #[test]
    fn a_short_message_alone_is_centred() {
        let t = texts();
        let columns = 80;
        let lines = lay_out(None, "hello", 'x', columns, false, &t);
        let words = lines[0]
            .pieces
            .iter()
            .find(|p| p.hit.is_none())
            .expect("the message");
        let left = tokens::space::MODELINE_EDGELEFT;
        let close_at = crate::chrome::from_the_edge(columns, 1);
        let before = words.at - left;
        let after = close_at - 1 - (words.at + 5);
        assert!(
            before.abs_diff(after) <= 1,
            "{before} before, {after} after"
        );
        let long = "x".repeat(200);
        let cut = lay_out(None, &long, 'x', columns, false, &t);
        let words = cut[0]
            .pieces
            .iter()
            .find(|p| p.hit.is_none())
            .expect("the message");
        assert_eq!(words.at, left);
        assert!(layout::width(&words.text) < 200);
    }
}
