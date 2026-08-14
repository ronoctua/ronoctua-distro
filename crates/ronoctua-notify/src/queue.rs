//! Notification queue — section 4.2.
//!
//! **One at a time.** The modeline has one message region, not a stack
//! of cards: invariant 14 does not allow a floating popup, and invariant 15
//! does not allow a card. So either the notification is on the line, or it is in the queue.
//!
//! # Why time comes in as a parameter
//!
//! [`Queue::tick`] receives the instant instead of calling `Instant::now()`. That
//! makes the whole expiry policy testable without sleeping: a test that
//! waits 5 seconds to prove something expires is a test nobody runs.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Urgency, as `org.freedesktop.Notifications` defines it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    /// It disappears on its own, and gives way to anything more urgent.
    Low,
    /// The default.
    Normal,
    /// It **never disappears on its own** and jumps the queue.
    Critical,
}

impl Urgency {
    /// Converts the protocol's integer. An unknown value becomes `Normal`.
    ///
    /// It is not an error: the specification allows future values, and refusing the
    /// notification over that would lose information the user wants.
    #[must_use]
    pub const fn from_protocol(v: u8) -> Self {
        match v {
            0 => Self::Low,
            2 => Self::Critical,
            _ => Self::Normal,
        }
    }
}

/// How long a notification stays on screen, by urgency.
///
/// Values in seconds. They are not visual tokens (invariant 9 speaks of colour,
/// font, size, spacing and codepoint) — they are behaviour policy,
/// and they live here next to the rule that uses them.
const SECONDS_LOW: u64 = 3;
const SECONDS_NORMAL: u64 = 6;

/// A notification already normalised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// The protocol's id. Stable for as long as the notification exists.
    pub id: u32,
    /// The name of the application that sent it.
    pub app: String,
    /// The `IconName` requested, if any. It enters the icon chain.
    pub icon: Option<String>,
    /// A short title.
    pub summary: String,
    /// Long text. It may be empty.
    pub body: String,
    /// Urgency.
    pub urgency: Urgency,
    /// How long it stays on screen. `None` = until dismissed by hand.
    pub expires: Option<Duration>,
}

impl Notification {
    /// Builds one from the protocol's fields, applying the policy.
    ///
    /// `expira_ms`: `-1` = "let the daemon decide", `0` = "never expires".
    #[must_use]
    pub fn fresh_one(
        id: u32,
        app: String,
        icon: Option<String>,
        summary: String,
        body: String,
        urgency: Urgency,
        expira_ms: i32,
    ) -> Self {
        // A CRITICAL ONE NEVER EXPIRES ON ITS OWN, even if the application asks.
        //
        // The specification lets the client nail down a timeout; section 4.2
        // decides otherwise, and here that decision wins. A critical battery
        // warning that disappears on its own is a warning that was not given.
        let expires = if urgency == Urgency::Critical {
            None
        } else {
            match expira_ms {
                0 => None,
                n if n < 0 => Some(Duration::from_secs(match urgency {
                    Urgency::Low => SECONDS_LOW,
                    _ => SECONDS_NORMAL,
                })),
                n => Some(Duration::from_millis(u64::try_from(n).unwrap_or(0))),
            }
        };
        Self {
            id,
            app,
            icon,
            summary,
            body,
            urgency,
            expires,
        }
    }

    /// One line for the modeline's message region.
    ///
    /// `app: summary`. The body does NOT go in: the modeline has one line, and the body
    /// may have paragraphs. It stays in the history (section 4.2).
    #[must_use]
    pub fn line(&self) -> String {
        if self.app.is_empty() {
            self.summary.clone()
        } else {
            format!("{}: {}", self.app, self.summary)
        }
    }
}

/// The queue. One showing, the rest waiting, everything kept in the history.
#[derive(Debug, Default)]
pub struct Queue {
    showing: Option<(Notification, Instant)>,
    pending: VecDeque<Notification>,
    history: Vec<Notification>,
}

/// How many notifications the history keeps.
///
/// A ceiling for MEMORY, not for taste: an application in a loop can generate
/// thousands per minute, and the history is the only structure that grows with no
/// natural limit.
const HISTORY_CEILING: usize = 200;

impl Queue {
    /// An empty queue.
    #[must_use]
    pub fn fresh_one() -> Self {
        Self::default()
    }

    /// Receives a notification.
    ///
    /// A **critical** one jumps the queue and takes the place of whoever is on screen — the
    /// one that leaves goes back to the front of the pending ones, and is not lost.
    pub fn enqueue(&mut self, n: Notification, now: Instant) {
        self.history.push(n.clone());
        if self.history.len() > HISTORY_CEILING {
            self.history.remove(0);
        }

        let critical = n.urgency == Urgency::Critical;
        match self.showing.take() {
            None => self.showing = Some((n, now)),
            Some((current, desde)) => {
                if critical && current.urgency != Urgency::Critical {
                    self.pending.push_front(current);
                    self.showing = Some((n, now));
                } else {
                    self.showing = Some((current, desde));
                    self.pending.push_back(n);
                }
            }
        }
    }

    /// Expires the current one if its time is up and pulls the next.
    ///
    /// Returns `true` if something changed — the caller only redraws in that case.
    pub fn tick(&mut self, now: Instant) -> bool {
        let Some((current, desde)) = self.showing.as_ref() else {
            return self.pull_next();
        };
        let Some(limit) = current.expires else {
            return false; // critical, or timeout 0: it stays until dismissed
        };
        if now.duration_since(*desde) < limit {
            return false;
        }
        self.showing = None;
        self.pull_next();
        true
    }

    /// Dismisses by id, whether on screen or in the queue.
    ///
    /// Returns `true` if it was found. It is the protocol's `CloseNotification`, and
    /// also what the user does by hand to a critical one.
    pub fn dismiss(&mut self, id: u32, now: Instant) -> bool {
        if self.showing.as_ref().is_some_and(|(n, _)| n.id == id) {
            self.showing = None;
            self.pull_at(now);
            return true;
        }
        let before = self.pending.len();
        self.pending.retain(|n| n.id != id);
        before != self.pending.len()
    }

    /// Dismisses the one on screen, if there is one.
    pub fn dismiss_current(&mut self, now: Instant) -> bool {
        match self.showing.take() {
            Some(_) => {
                self.pull_at(now);
                true
            }
            None => false,
        }
    }

    fn pull_next(&mut self) -> bool {
        self.pull_at(Instant::now())
    }

    fn pull_at(&mut self, now: Instant) -> bool {
        if self.showing.is_some() {
            return false;
        }
        match self.pending.pop_front() {
            Some(n) => {
                self.showing = Some((n, now));
                true
            }
            None => false,
        }
    }

    /// The notification on screen right now.
    #[must_use]
    pub fn showing(&self) -> Option<&Notification> {
        self.showing.as_ref().map(|(n, _)| n)
    }

    /// How many are waiting their turn. It goes into the notification icon's badge.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// The history, from oldest to newest.
    #[must_use]
    pub fn history(&self) -> &[Notification] {
        &self.history
    }

    /// Is there a critical one waiting or on screen?
    ///
    /// The modeline highlights that: a critical one in the queue must not look like an
    /// ordinary notification waiting.
    #[must_use]
    pub fn has_critical(&self) -> bool {
        self.showing()
            .is_some_and(|n| n.urgency == Urgency::Critical)
            || self.pending.iter().any(|n| n.urgency == Urgency::Critical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(id: u32, u: Urgency) -> Notification {
        Notification::fresh_one(
            id,
            "app".into(),
            None,
            format!("resumo {id}"),
            String::new(),
            u,
            -1,
        )
    }

    #[test]
    fn one_at_a_time() {
        // Section 4.2: the modeline has ONE message region. Two at the same
        // time would require stacking, and stacking would require a card (invariant 15).
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1, Urgency::Normal), t0);
        f.enqueue(n(2, Urgency::Normal), t0);
        f.enqueue(n(3, Urgency::Normal), t0);
        assert_eq!(f.showing().map(|x| x.id), Some(1));
        assert_eq!(f.pending(), 2);
    }

    #[test]
    fn a_critical_one_never_expires_on_its_own() {
        // Even with the application asking for a short timeout. A critical warning that
        // disappears on its own is a warning that was not given.
        let pedida = Notification::fresh_one(
            1,
            "bateria".into(),
            None,
            "3%".into(),
            String::new(),
            Urgency::Critical,
            500,
        );
        assert_eq!(pedida.expires, None);

        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(pedida, t0);
        assert!(!f.tick(t0 + Duration::from_secs(3600)));
        assert_eq!(f.showing().map(|x| x.id), Some(1));
    }

    #[test]
    fn a_normal_one_expires_and_pulls_the_next() {
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1, Urgency::Normal), t0);
        f.enqueue(n(2, Urgency::Normal), t0);
        assert!(!f.tick(t0 + Duration::from_secs(1)));
        assert!(f.tick(t0 + Duration::from_secs(SECONDS_NORMAL + 1)));
        assert_eq!(f.showing().map(|x| x.id), Some(2));
    }

    #[test]
    fn a_critical_one_cuts_the_queue_without_losing_the_current() {
        // Jumping the queue and losing would be different things: the one that leaves goes back to the
        // FRONT of the pending ones, so it is next after the critical one.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1, Urgency::Normal), t0);
        f.enqueue(n(2, Urgency::Normal), t0);
        f.enqueue(n(9, Urgency::Critical), t0);
        assert_eq!(f.showing().map(|x| x.id), Some(9));

        f.dismiss_current(t0);
        assert_eq!(f.showing().map(|x| x.id), Some(1), "a que saiu se perdeu");
        assert_eq!(f.pending(), 1);
    }

    #[test]
    fn one_critical_does_not_unseat_another() {
        // Otherwise the second would erase the first, and the first is precisely the
        // one that must not disappear.
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        f.enqueue(n(1, Urgency::Critical), t0);
        f.enqueue(n(2, Urgency::Critical), t0);
        assert_eq!(f.showing().map(|x| x.id), Some(1));
        assert_eq!(f.pending(), 1);
    }

    #[test]
    fn the_history_keeps_what_left_the_screen() {
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        for i in 1..=3 {
            f.enqueue(n(i, Urgency::Normal), t0);
        }
        f.dismiss(1, t0);
        // Dismissing removes from the QUEUE, not from the history: the history is the record
        // of what happened, and deleting from it would be losing information.
        assert_eq!(f.history().len(), 3);
    }

    #[test]
    fn the_history_has_a_ceiling() {
        let t0 = Instant::now();
        let mut f = Queue::fresh_one();
        for i in 0..(HISTORY_CEILING as u32 + 50) {
            f.enqueue(n(i, Urgency::Low), t0);
        }
        assert_eq!(f.history().len(), HISTORY_CEILING);
        // The ceiling drops the OLD ones, not the recent ones.
        assert_eq!(f.history().last().map(|x| x.id), Some(249));
    }

    #[test]
    fn a_zero_timeout_means_never() {
        let x = Notification::fresh_one(
            1,
            "a".into(),
            None,
            "b".into(),
            String::new(),
            Urgency::Normal,
            0,
        );
        assert_eq!(x.expires, None);
    }

    #[test]
    fn an_unknown_urgency_becomes_normal() {
        // The specification allows future values. Refusing would lose the
        // whole notification over one field.
        assert_eq!(Urgency::from_protocol(7), Urgency::Normal);
        assert_eq!(Urgency::from_protocol(0), Urgency::Low);
        assert_eq!(Urgency::from_protocol(2), Urgency::Critical);
    }

    #[test]
    fn the_modeline_row_does_not_carry_the_body() {
        // The body may have paragraphs; the modeline has one line.
        let x = Notification::fresh_one(
            1,
            "git".into(),
            None,
            "push ok".into(),
            "linha 1\nlinha 2".into(),
            Urgency::Normal,
            -1,
        );
        assert_eq!(x.line(), "git: push ok");
    }
}
