//! How long a message stays — on the footer's minibuffer and on a window's
//! line 0.
//!
//! # Why classes, and not one number
//!
//! There was one number, three seconds, for everything the shell says. A
//! `Quit` after `Esc` held the line for as long as a refusal that has to be
//! read to the end, and the owner saw the short ones linger: a word that
//! repeats what the hand just did needs a glance, and three seconds of it is
//! a line taken from whatever comes next. So a message says what KIND it is,
//! and the seconds are tokens (`space.echo.*`), in tenths, beside every other
//! number the screen keeps.
//!
//! # And a clock only while there is a message
//!
//! The footer arms a one-shot timer for the deadline of the message it put up
//! (`Footer::echo_for`), and the timer finds the message still there or does
//! nothing — a newer message carries its own. Nothing ticks while the line is
//! empty.
//!
//! A window's message takes its deadline from the door's own clock
//! (`ronoctua_shell_door::door::hang_the_echo`), and from THIS table: `Ui::say` is the
//! [`Echo::Normal`] class, and `Ui::say_for` names another. One table for
//! both doors — two would be two answers to "how long is a message on
//! screen". The clock runs only while there is a message, and a hand on it
//! stops it ([`Countdown`]).

use ronoctua_tokens as tokens;

/// The class of a message: how long it is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Echo {
    /// A word that confirms what the hand just did — `Quit`, the bars'
    /// new mode. A glance.
    Brief,
    /// A sentence: what happened, and what it means. Every message that
    /// names no class.
    #[default]
    Normal,
    /// A refusal that has to be read to the end, and acted on — a
    /// preference that could not be saved.
    Long,
    /// "`ESC` again closes this window": the first `ESC` that would close a
    /// window says it, and it stays for as long as a second `ESC` closes —
    /// the sentence and the deadline are one number
    /// (`ronoctua_shell_door::door::Ui::closing`).
    Confirm,
}

impl Echo {
    /// How long a message of this class stays.
    #[must_use]
    pub fn lasts(self) -> std::time::Duration {
        let tenths = match self {
            Self::Brief => tokens::space::ECHO_BRIEFTENTHS,
            Self::Normal => tokens::space::ECHO_NORMALTENTHS,
            Self::Long => tokens::space::ECHO_LONGTENTHS,
            Self::Confirm => tokens::space::ECHO_CONFIRMTENTHS,
        };
        std::time::Duration::from_millis(u64::from(tenths) * 100)
    }
}

/// How long a NORMAL message stays on screen — BOTH doors, in whole
/// seconds, for the windows that arm a one-shot of their own
/// (`FilesWindow::say`).
///
/// It is the `normal` class's token (`crate::echo`), and not a number of its
/// own: two mechanisms that disagreed about "how long is a message on
/// screen" would give that question two answers.
#[allow(clippy::cast_lossless)]
pub const ECHO_SECONDS: u64 = tokens::space::ECHO_NORMALTENTHS as u64 / 10;

/// A MESSAGE'S TIME, WHICH A HAND CAN HOLD — the band's rule (`ronoctua_notify`'s
/// queue) for every other place a message counts down: a window's message at
/// its foot (`ronoctua_shell_door::door::foot`) and a notification said on the
/// minibuffer's line. While held — the pointer on it — the clock is stopped,
/// not stretched: the time it had left is the time it has when the hand goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Countdown {
    /// When it started, pushed forward by every hold it was released from.
    since: std::time::Instant,
    /// How long it lasts.
    lasts: std::time::Duration,
    /// Since when it is held, while it is.
    held: Option<std::time::Instant>,
}

impl Countdown {
    /// A message of `lasts`, starting `now`.
    #[must_use]
    pub const fn start(lasts: std::time::Duration, now: std::time::Instant) -> Self {
        Self {
            since: now,
            lasts,
            held: None,
        }
    }

    /// The pointer came to rest on it: the clock stops. Holding twice is
    /// one hold.
    pub fn hold(&mut self, now: std::time::Instant) {
        self.held.get_or_insert(now);
    }

    /// The pointer went: the clock runs again from where it stopped.
    pub fn release(&mut self, now: std::time::Instant) {
        if let Some(at) = self.held.take() {
            self.since += now.saturating_duration_since(at);
        }
    }

    /// Whether a hand holds it.
    #[must_use]
    pub const fn held(&self) -> bool {
        self.held.is_some()
    }

    /// How long it has left.
    #[must_use]
    pub fn left(&self, now: std::time::Instant) -> std::time::Duration {
        let at = self.held.unwrap_or(now);
        self.lasts
            .saturating_sub(at.saturating_duration_since(self.since))
    }

    /// Whether its time is up — never while it is held.
    #[must_use]
    pub fn spent(&self, now: std::time::Instant) -> bool {
        self.held.is_none() && self.left(now).is_zero()
    }

    /// The eighths it has left, for the band's pie (`band::countdown_glyph`):
    /// 8 at the start, 1 in its last eighth.
    #[must_use]
    pub fn eighths(&self, now: std::time::Instant) -> u8 {
        if self.lasts.is_zero() {
            return 1;
        }
        let left = self.left(now).as_secs_f64() / self.lasts.as_secs_f64();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let eighths = (left * 8.0).ceil() as u8;
        eighths.clamp(1, 8)
    }

    /// How long until the pie shows one eighth less — what a clock waits
    /// for to draw it, and then to take the message off.
    #[must_use]
    pub fn next_step(&self, now: std::time::Instant) -> std::time::Duration {
        let step = self.lasts / 8;
        if step.is_zero() {
            return self.left(now);
        }
        let left = self.left(now);
        let into = left.as_nanos() % step.as_nanos();
        #[allow(clippy::cast_possible_truncation)]
        let wait = std::time::Duration::from_nanos(into as u64);
        if wait.is_zero() { step.min(left) } else { wait }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a held message keeps losing time (the hand on it is reading
    /// it), comes back with less than it had, or is ever spent while held —
    /// or the pie does not walk from eight to one.
    #[test]
    fn a_held_countdown_stops_and_takes_up_where_it_stopped() {
        let t0 = std::time::Instant::now();
        let s = std::time::Duration::from_secs;
        let mut c = Countdown::start(s(8), t0);
        assert_eq!(c.eighths(t0), 8);
        assert_eq!(c.left(t0 + s(3)), s(5));
        c.hold(t0 + s(3));
        c.hold(t0 + s(4));
        assert!(c.held());
        assert_eq!(c.left(t0 + s(60)), s(5), "the clock ran while held");
        assert!(!c.spent(t0 + s(60)));
        c.release(t0 + s(60));
        assert_eq!(c.left(t0 + s(60)), s(5));
        assert_eq!(c.eighths(t0 + s(64)), 1);
        assert!(!c.spent(t0 + s(64)));
        assert!(c.spent(t0 + s(65)));
        // THE NEXT STEP is the rest of the current eighth.
        let fresh = Countdown::start(s(8), t0);
        assert_eq!(fresh.next_step(t0), s(1));
        assert_eq!(
            fresh.next_step(t0 + std::time::Duration::from_millis(250)),
            std::time::Duration::from_millis(750)
        );
    }

    /// FAILS IF: the classes stop being ordered — a brief message outlasting
    /// a normal one, or a refusal gone before a sentence — or the normal one
    /// stops being the number both doors agreed on (`ECHO_SECONDS`).
    #[test]
    fn the_classes_are_ordered_and_the_normal_one_is_the_old_number() {
        let [brief, normal, long] = [Echo::Brief, Echo::Normal, Echo::Long].map(Echo::lasts);
        assert!(
            brief < normal && normal < long,
            "{brief:?} {normal:?} {long:?}"
        );
        assert_eq!(
            normal,
            std::time::Duration::from_secs(crate::echo::ECHO_SECONDS)
        );
        assert_eq!(Echo::default(), Echo::Normal);
        // A GLANCE IS UNDER TWO SECONDS, and a refusal twice a sentence.
        assert!(brief < std::time::Duration::from_secs(2));
        assert!(long >= normal * 2);
        // A SECOND `ESC` HAS MORE THAN A GLANCE to come, and less than a
        // refusal's reading: long enough to mean it, short enough that an
        // `ESC` much later is a new one.
        let confirm = Echo::Confirm.lasts();
        assert!(brief < confirm && confirm < long, "{confirm:?}");
    }
}
