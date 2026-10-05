//! The reminders a person keeps, and what happens to them as time passes —
//! PURE: every function is handed `now` and the zone, and opens nothing.
//!
//! # One file, whole
//!
//! What the person asked for and what is ringing now live together in
//! `reminders.json` under the module's configuration directory, written whole
//! and atomically (`ronoctua-store`). A brain that restarts in the middle of an
//! alarm comes back with it still ringing — an alarm leaves only when somebody
//! dismisses it or puts it off, and a restart is neither.
//!
//! # Ringing, missed, put off
//!
//! - A reminder whose moment has come RINGS: it is marked ringing since then,
//!   a repeating one moves on to its next moment at once, and a once-only one
//!   stays until it is answered — then it leaves the list.
//! - One whose moment came while the brain was not running — the machine was
//!   off, or the module switched off — and passed more than [`LATE_AT_MOST`]
//!   before it started is MISSED instead: it does not sound (an alarm hours
//!   late is not the alarm that was asked for), it is written down as missed,
//!   and a once-only one leaves.
//! - One whose moment came while the brain WAS running rings however late it
//!   is seen: that is a machine that was asleep, and an alarm survives a
//!   suspend — it rings when the lid opens.
//! - One put off rings again when its time is up, however late: the person
//!   already saw it.

use serde::{Deserialize, Serialize};

use super::calendar::{Repeat, local_day, minute_of, next_after};
use super::grammar::Plan;
use crate::moment::{Moment, Offset};

/// How late a reminder may ring before it is missed instead, in seconds.
///
/// Only for what came due BEFORE the brain started: a brain restarted a few
/// minutes after the hour still rings it, and one started hours later calls it
/// missed. What came due while the brain ran (asleep with the machine) rings
/// however late.
pub const LATE_AT_MOST: i64 = 10 * 60;

/// How many missed reminders are kept. The oldest go first.
pub const MISSED_KEPT: usize = 32;

/// How many reminders a person may keep — a bound on the file, far above
/// anybody's list.
pub const KEPT_MOST: usize = 500;

/// Everything the module keeps.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Book {
    /// The id the next reminder gets. Never reused: an id on a page the person
    /// is looking at must not come to mean another reminder.
    pub next_id: u64,
    /// In the order they were made.
    pub reminders: Vec<Reminder>,
    /// What came due while the machine could not ring it, oldest first.
    pub missed: Vec<Missed>,
}

/// One reminder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Reminder {
    /// Stable while it exists.
    pub id: u64,
    /// What it says, as typed; empty says "Reminder".
    #[serde(default)]
    pub text: String,
    /// How it comes round.
    pub repeat: Repeat,
    /// The local minute of the day it rings at.
    pub minute: u16,
    /// Whether it plays a chime when it rings.
    #[serde(default)]
    pub sound: bool,
    /// When it next rings. For a once-only reminder that is ringing, when it
    /// rang.
    pub next: Moment,
    /// When it was made.
    #[serde(default)]
    pub made: Moment,
    /// Ringing since then, for the moment it was due.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ringing: Option<Ringing>,
    /// Put off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed: Option<Snooze>,
}

/// A reminder put off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Snooze {
    /// Until when.
    pub until: Moment,
    /// The moment it was for, which the page still says.
    pub due: Moment,
}

/// A reminder that is ringing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Ringing {
    /// Since when.
    pub since: Moment,
    /// The moment it was for.
    pub due: Moment,
}

/// A reminder that came due while nothing could ring it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Missed {
    /// What it said.
    pub text: String,
    /// The moment it was for.
    pub due: Moment,
    /// When the brain found it had passed.
    pub found: Moment,
}

/// What passing time did to one reminder — for the activity log and the
/// chime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Happened {
    /// It began to ring.
    Rang {
        /// Which.
        id: u64,
        /// What it says.
        text: String,
        /// Whether it plays a chime.
        sound: bool,
    },
    /// It was missed.
    Missed {
        /// What it said.
        text: String,
        /// The moment it was for.
        due: Moment,
    },
}

impl Reminder {
    /// Whether it happens once.
    #[must_use]
    pub const fn once(&self) -> bool {
        matches!(self.repeat, Repeat::Once)
    }

    /// The next moment something happens to it by itself: it rings, or rings
    /// again after being put off. `None` for a once-only reminder that is
    /// already ringing — only a person moves it now.
    #[must_use]
    pub fn due(&self) -> Option<Moment> {
        let rings = !(self.once() && self.ringing.is_some()) && self.snoozed.is_none();
        let next = rings.then_some(self.next);
        match (self.snoozed.map(|s| s.until), next) {
            (Some(s), Some(n)) => Some(s.min(n)),
            (s, n) => s.or(n),
        }
    }

    /// When it shows as coming: put off, or its next moment.
    #[must_use]
    pub fn coming(&self) -> Moment {
        self.snoozed.map_or(self.next, |s| s.until)
    }
}

impl Book {
    /// Makes a reminder, and says its id.
    ///
    /// # Errors
    ///
    /// The list is at [`KEPT_MOST`].
    pub fn add(
        &mut self,
        plan: Plan,
        text: Option<String>,
        sound: bool,
        now: Moment,
    ) -> Result<u64, Full> {
        if self.reminders.len() >= KEPT_MOST {
            return Err(Full);
        }
        let id = self.fresh_id();
        self.reminders.push(Reminder {
            id,
            text: text.unwrap_or_default(),
            repeat: plan.repeat,
            minute: plan.minute,
            sound,
            next: plan.next,
            made: now,
            ringing: None,
            snoozed: None,
        });
        Ok(id)
    }

    /// An id no reminder has had.
    fn fresh_id(&mut self) -> u64 {
        // PAST ANY ID IN THE FILE, whatever `next-id` says: a hand-edited file
        // that forgot to move it must not hand out an id twice.
        let past = self.reminders.iter().map(|r| r.id + 1).max().unwrap_or(0);
        let id = self.next_id.max(past).max(1);
        self.next_id = id + 1;
        id
    }

    /// One reminder, by id.
    #[must_use]
    pub fn get(&self, id: u64) -> Option<&Reminder> {
        self.reminders.iter().find(|r| r.id == id)
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut Reminder> {
        self.reminders.iter_mut().find(|r| r.id == id)
    }

    /// The earliest moment something happens by itself; `None` when nothing
    /// will — then no clock is armed.
    #[must_use]
    pub fn next_due(&self) -> Option<Moment> {
        self.reminders.iter().filter_map(Reminder::due).min()
    }

    /// Everything whose moment has come by `now`: rings, rings again, or is
    /// missed. `running` is when the brain began to watch: what came due
    /// after it rings however late (the machine slept), what came due before
    /// it may be missed (the machine was off). What happened is handed back
    /// for the log and the chime.
    pub fn tick(&mut self, now: Moment, running: Moment, zone: &dyn Offset) -> Vec<Happened> {
        let mut out = Vec::new();
        let mut gone = Vec::new();
        for r in &mut self.reminders {
            // PUT OFF, AND ITS TIME IS UP: it rings again, however late.
            if let Some(put_off) = r.snoozed
                && put_off.until <= now
            {
                r.snoozed = None;
                r.ringing = Some(Ringing {
                    since: now,
                    due: put_off.due,
                });
                out.push(Happened::Rang {
                    id: r.id,
                    text: r.text.clone(),
                    sound: r.sound,
                });
                continue;
            }
            if r.snoozed.is_some() || (r.once() && r.ringing.is_some()) || r.next > now {
                continue;
            }
            let due = r.next;
            if due < running && now - due > LATE_AT_MOST {
                out.push(Happened::Missed {
                    text: r.text.clone(),
                    due,
                });
                self.missed.push(Missed {
                    text: r.text.clone(),
                    due,
                    found: now,
                });
                // EVERY OCCURRENCE THAT PASSED is one missed reminder, not one
                // per day the machine was off. A rule that never lands again
                // (a week with no days, written by hand) leaves like a
                // once-only one.
                match (r.once(), next_after(&r.repeat, r.minute, now, zone)) {
                    (false, Some(next)) => r.next = next,
                    _ => gone.push(r.id),
                }
                continue;
            }
            r.ringing = Some(Ringing { since: now, due });
            if !r.once() {
                match next_after(&r.repeat, r.minute, due.max(now), zone) {
                    Some(next) => r.next = next,
                    // A RULE THAT NEVER LANDS AGAIN rings this once and is a
                    // once-only reminder from now on: left as it was, its
                    // moment would stay in the past and ring at every wake.
                    None => r.repeat = Repeat::Once,
                }
            }
            out.push(Happened::Rang {
                id: r.id,
                text: r.text.clone(),
                sound: r.sound,
            });
        }
        self.reminders.retain(|r| !gone.contains(&r.id));
        if self.missed.len() > MISSED_KEPT {
            let over = self.missed.len() - MISSED_KEPT;
            self.missed.drain(..over);
        }
        out
    }

    /// Stops one reminder ringing: a once-only one leaves the list. Its text,
    /// when it was ringing.
    pub fn dismiss(&mut self, id: u64) -> Option<String> {
        let r = self.get_mut(id)?;
        r.ringing?;
        r.ringing = None;
        let text = r.text.clone();
        if r.once() {
            self.reminders.retain(|r| r.id != id);
        }
        Some(text)
    }

    /// Stops every reminder ringing. How many were.
    pub fn dismiss_all(&mut self) -> usize {
        let ids: Vec<u64> = self.ringing().map(|r| r.id).collect();
        ids.iter().filter(|id| self.dismiss(**id).is_some()).count()
    }

    /// Puts one ringing reminder off until `until`. Whether it was ringing.
    pub fn snooze(&mut self, id: u64, until: Moment) -> bool {
        let Some(r) = self.get_mut(id) else {
            return false;
        };
        let Some(ringing) = r.ringing else {
            return false;
        };
        // THE MOMENT IT WAS FOR is kept, so the page can still say it.
        r.snoozed = Some(Snooze {
            until,
            due: ringing.due,
        });
        r.ringing = None;
        true
    }

    /// Puts every ringing reminder off until `until`. How many were ringing.
    pub fn snooze_all(&mut self, until: Moment) -> usize {
        let ids: Vec<u64> = self.ringing().map(|r| r.id).collect();
        ids.iter().filter(|id| self.snooze(**id, until)).count()
    }

    /// Forgets a reminder. Whether there was one.
    pub fn forget(&mut self, id: u64) -> bool {
        let before = self.reminders.len();
        self.reminders.retain(|r| r.id != id);
        self.reminders.len() != before
    }

    /// Gives a reminder a new "when" (and, when the answer said them, a new
    /// text and sound). Whether there was one.
    pub fn change(
        &mut self,
        id: u64,
        plan: Plan,
        text: Option<String>,
        sound: Option<bool>,
    ) -> bool {
        let Some(r) = self.get_mut(id) else {
            return false;
        };
        r.repeat = plan.repeat;
        r.minute = plan.minute;
        r.next = plan.next;
        // A NEW "WHEN" ANSWERS THE OLD ONE: nothing rings or waits for it any
        // more.
        r.ringing = None;
        r.snoozed = None;
        if let Some(text) = text {
            r.text = text;
        }
        if let Some(sound) = sound {
            r.sound = sound;
        }
        true
    }

    /// Plays a chime or not. The new setting, when there was a reminder.
    pub fn toggle_sound(&mut self, id: u64) -> Option<bool> {
        let r = self.get_mut(id)?;
        r.sound = !r.sound;
        Some(r.sound)
    }

    /// Moves a reminder to the next way of repeating, from the day it next
    /// rings: once, every day, every week on that weekday, every month on
    /// that day, every year on that date, and once again. Whether there was
    /// one to move.
    pub fn cycle_repeat(&mut self, id: u64, now: Moment, zone: &dyn Offset) -> bool {
        let Some(r) = self.get_mut(id) else {
            return false;
        };
        let at = r.coming();
        let date = super::calendar::date_of(local_day(at, zone));
        let repeat = match r.repeat {
            Repeat::Once => Repeat::Day,
            Repeat::Day => Repeat::Week {
                days: vec![super::calendar::Weekday::of_day(local_day(at, zone))],
            },
            Repeat::Week { .. } => Repeat::Month {
                day: u8::try_from(date.day).unwrap_or(1),
            },
            Repeat::Month { .. } => Repeat::Year {
                month: u8::try_from(date.month).unwrap_or(1),
                day: u8::try_from(date.day).unwrap_or(1),
            },
            Repeat::Year { .. } => Repeat::Once,
        };
        // A ONCE-ONLY reminder rings at the next moment the old rule had, or
        // — when that has passed — at the same minute tomorrow.
        let minute = minute_of(at, zone);
        let next = if matches!(repeat, Repeat::Once) {
            if at > now {
                at
            } else {
                super::calendar::at_local(local_day(now, zone) + 1, minute, zone)
            }
        } else {
            match next_after(&repeat, minute, now, zone) {
                Some(n) => n,
                None => return false,
            }
        };
        r.repeat = repeat;
        r.minute = minute;
        r.next = next;
        true
    }

    /// Forgets every missed reminder.
    pub fn clear_missed(&mut self) -> bool {
        let any = !self.missed.is_empty();
        self.missed.clear();
        any
    }

    /// The reminders ringing now, the one that began first first.
    pub fn ringing(&self) -> impl Iterator<Item = &Reminder> {
        let mut out: Vec<&Reminder> = self
            .reminders
            .iter()
            .filter(|r| r.ringing.is_some())
            .collect();
        out.sort_by_key(|r| (r.ringing.map_or(0, |g| g.since), r.id));
        out.into_iter()
    }
}

/// The list is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Full;

#[cfg(test)]
mod tests {
    use super::super::calendar::{Weekday, at_local, day_of};
    use super::*;
    use crate::moment::Fixed;

    const ZONE: Fixed = Fixed(0);

    /// The brain has been watching since before any of these moments —
    /// unless a test says it started later.
    const RUNNING: Moment = 0;
    const MINUTE: i64 = 60;
    const HOUR: i64 = 3600;

    fn day() -> i64 {
        day_of(2026, 9, 30).expect("a real date")
    }

    fn at(hour: u16) -> Moment {
        at_local(day(), hour * 60, &ZONE)
    }

    fn once(next: Moment) -> Plan {
        Plan {
            repeat: Repeat::Once,
            minute: minute_of(next, &ZONE),
            next,
        }
    }

    fn daily(hour: u16) -> Plan {
        Plan {
            repeat: Repeat::Day,
            minute: hour * 60,
            next: at(hour),
        }
    }

    #[test]
    fn a_once_only_reminder_rings_stays_and_leaves_when_dismissed() {
        let mut b = Book::default();
        let id = b
            .add(once(at(9)), Some("bins".into()), true, at(8))
            .expect("room");
        assert_eq!(b.next_due(), Some(at(9)));
        assert!(
            b.tick(at(9) - 1, RUNNING, &ZONE).is_empty(),
            "it rang early"
        );
        let happened = b.tick(at(9), RUNNING, &ZONE);
        assert_eq!(
            happened,
            vec![Happened::Rang {
                id,
                text: "bins".into(),
                sound: true
            }]
        );
        // IT STAYS: an hour later it is still ringing, and nothing else is
        // armed for it.
        assert!(b.tick(at(10), RUNNING, &ZONE).is_empty());
        assert_eq!(b.ringing().count(), 1);
        assert_eq!(
            b.next_due(),
            None,
            "a clock armed for an alarm nobody answered"
        );
        assert_eq!(b.dismiss(id), Some("bins".into()));
        assert!(
            b.reminders.is_empty(),
            "a once-only reminder stayed after it was answered"
        );
    }

    #[test]
    fn a_repeating_reminder_moves_on_when_it_rings_and_stays_ringing() {
        let mut b = Book::default();
        let id = b.add(daily(9), None, false, at(8)).expect("room");
        b.tick(at(9), RUNNING, &ZONE);
        let r = b.get(id).expect("kept");
        assert_eq!(r.ringing.map(|g| g.due), Some(at(9)));
        assert_eq!(r.next, at(9) + 24 * HOUR, "it did not move on");
        assert_eq!(b.next_due(), Some(at(9) + 24 * HOUR));
        assert_eq!(b.dismiss(id), Some(String::new()));
        assert!(b.get(id).is_some(), "a daily reminder left when answered");
        assert_eq!(b.dismiss(id), None, "a reminder dismissed twice");
    }

    #[test]
    fn a_snoozed_reminder_rings_again_when_its_time_is_up() {
        let mut b = Book::default();
        let id = b
            .add(once(at(9)), Some("tea".into()), false, at(8))
            .expect("room");
        b.tick(at(9), RUNNING, &ZONE);
        assert_eq!(b.snooze_all(at(9) + 10 * MINUTE), 1);
        assert_eq!(b.ringing().count(), 0, "a snoozed reminder still rang");
        assert_eq!(b.next_due(), Some(at(9) + 10 * MINUTE));
        assert!(b.tick(at(9) + 10 * MINUTE - 1, RUNNING, &ZONE).is_empty());
        // AND AGAIN, even much later than the snooze: the person saw it.
        let again = b.tick(at(11), RUNNING, &ZONE);
        assert!(matches!(again.as_slice(), [Happened::Rang { .. }]));
        let r = b.get(id).expect("kept");
        assert_eq!(r.ringing.map(|g| (g.since, g.due)), Some((at(11), at(9))));
        assert!(!b.snooze(id + 99, at(12)), "a snooze of nothing");
        assert!(b.dismiss(id).is_some());
        assert!(b.reminders.is_empty());
    }

    #[test]
    fn what_came_due_while_the_machine_was_off_is_missed_and_does_not_ring() {
        let mut b = Book::default();
        let gone = b
            .add(once(at(9)), Some("call".into()), true, at(8))
            .expect("room");
        let stays = b
            .add(daily(7), Some("pills".into()), true, at(6))
            .expect("room");
        // THREE DAYS OFF, back at 12:00.
        let back = at(12) + 3 * 24 * HOUR;
        let happened = b.tick(back, back, &ZONE);
        assert_eq!(happened.len(), 2);
        assert!(
            happened
                .iter()
                .all(|h| matches!(h, Happened::Missed { .. })),
            "something rang late: {happened:?}"
        );
        assert!(b.get(gone).is_none(), "a missed once-only reminder stayed");
        // ONE MISSED PER REMINDER, not one per day: the daily one moves on
        // to tomorrow's 07:00.
        assert_eq!(b.missed.len(), 2);
        assert_eq!(b.get(stays).map(|r| r.next), Some(at(7) + 4 * 24 * HOUR));
        assert_eq!(b.ringing().count(), 0);
        assert!(b.clear_missed());
        assert!(b.missed.is_empty());
    }

    #[test]
    fn a_little_late_still_rings() {
        // THE WATCH THREAD WAKES LATE after a suspend by up to half a minute,
        // and a brain restarted a few minutes after the hour still rings it.
        let mut b = Book::default();
        b.add(once(at(9)), None, false, at(8)).expect("room");
        let happened = b.tick(at(9) + LATE_AT_MOST, at(9) + LATE_AT_MOST, &ZONE);
        assert!(matches!(happened.as_slice(), [Happened::Rang { .. }]));
        let mut b = Book::default();
        b.add(once(at(9)), None, false, at(8)).expect("room");
        let happened = b.tick(at(9) + LATE_AT_MOST + 1, at(9) + LATE_AT_MOST + 1, &ZONE);
        assert!(matches!(happened.as_slice(), [Happened::Missed { .. }]));
    }

    #[test]
    fn what_came_due_while_the_machine_slept_rings_when_it_wakes() {
        // THE BRAIN RAN SINCE 08:00 and the lid closed at 08:30; it opens three
        // days later at noon. Asleep is not off: both ring, once each, and
        // the daily one moves on from now — not once per day it slept.
        let mut b = Book::default();
        let call = b
            .add(once(at(9)), Some("call".into()), true, at(8))
            .expect("room");
        let pills = b
            .add(daily(7), Some("pills".into()), true, at(6))
            .expect("room");
        b.get_mut(pills).expect("kept").next = at(7) + 24 * HOUR;
        let back = at(12) + 3 * 24 * HOUR;
        let happened = b.tick(back, at(8), &ZONE);
        assert_eq!(happened.len(), 2, "{happened:?}");
        assert!(
            happened.iter().all(|h| matches!(h, Happened::Rang { .. })),
            "a reminder the machine slept through was called missed: {happened:?}"
        );
        assert!(b.missed.is_empty());
        assert_eq!(
            b.get(call).and_then(|r| r.ringing).map(|g| g.due),
            Some(at(9))
        );
        assert_eq!(b.get(pills).map(|r| r.next), Some(at(7) + 4 * 24 * HOUR));
        assert!(b.tick(back + 1, at(8), &ZONE).is_empty(), "it rang twice");
    }

    #[test]
    fn the_repeat_goes_round() {
        let mut b = Book::default();
        let id = b
            .add(once(at(9) + 24 * HOUR), None, false, at(8))
            .expect("room");
        let mut seen = Vec::new();
        for _ in 0..5 {
            assert!(b.cycle_repeat(id, at(8), &ZONE));
            seen.push(b.get(id).map(|r| r.repeat.clone()).expect("kept"));
        }
        // FROM THURSDAY'S 09:00 at 08:00 on Wednesday the 30th: every day
        // begins today, and from then on the day it names is today's.
        assert_eq!(
            seen,
            vec![
                Repeat::Day,
                Repeat::Week {
                    days: vec![Weekday::Wednesday]
                },
                Repeat::Month { day: 30 },
                Repeat::Year { month: 9, day: 30 },
                Repeat::Once,
            ]
        );
        let r = b.get(id).expect("kept");
        assert_eq!(minute_of(r.next, &ZONE), 9 * 60);
        assert!(r.next > at(8));
    }

    #[test]
    fn ids_are_never_reused_and_the_file_reads_back() {
        let mut b = Book::default();
        let a = b.add(daily(9), Some("a".into()), false, 0).expect("room");
        assert!(b.forget(a));
        assert!(!b.forget(a));
        let c = b.add(daily(9), Some("c".into()), false, 0).expect("room");
        assert_ne!(a, c, "an id was handed out twice");
        // A HAND-EDITED FILE whose `next-id` stayed behind.
        b.next_id = 0;
        let d = b.add(daily(9), None, false, 0).expect("room");
        assert!(d > c);
        let json = serde_json::to_string(&b).expect("encode");
        assert_eq!(serde_json::from_str::<Book>(&json).expect("decode"), b);
        // AND THE SHAPE A PERSON WRITES BY HAND reads.
        let typed: Book = serde_json::from_str(
            r#"{"reminders":[{"id":7,"text":"gym","repeat":{"every":"week","days":["monday","friday"]},"minute":420,"next":1790000000}]}"#,
        )
        .expect("a hand-written file");
        assert_eq!(
            typed.reminders[0].repeat,
            Repeat::Week {
                days: vec![Weekday::Monday, Weekday::Friday]
            }
        );
        assert!(!typed.reminders[0].sound);
    }

    #[test]
    fn a_rule_that_never_lands_again_rings_once_and_does_not_loop() {
        // A WEEK WITH NO DAYS, as a hand-edited file can say: rung, it must not
        // stay due in the past — the watch would wake every second for it.
        let mut b = Book::default();
        let id = b
            .add(
                Plan {
                    repeat: Repeat::Week { days: Vec::new() },
                    minute: 9 * 60,
                    next: at(9),
                },
                None,
                false,
                at(8),
            )
            .expect("room");
        assert_eq!(b.tick(at(9), RUNNING, &ZONE).len(), 1);
        assert!(
            b.tick(at(9) + 1, RUNNING, &ZONE).is_empty(),
            "it rang again"
        );
        assert_eq!(b.next_due(), None, "a clock still armed in the past");
        assert!(b.dismiss(id).is_some());
        assert!(b.reminders.is_empty());
    }

    #[test]
    fn a_change_answers_what_was_ringing() {
        let mut b = Book::default();
        let id = b
            .add(once(at(9)), Some("old".into()), false, at(8))
            .expect("room");
        b.tick(at(9), RUNNING, &ZONE);
        assert!(b.change(id, daily(18), None, Some(true)));
        let r = b.get(id).expect("kept");
        assert!(r.ringing.is_none() && r.snoozed.is_none());
        assert_eq!((r.text.as_str(), r.sound, r.next), ("old", true, at(18)));
        assert_eq!(b.toggle_sound(id), Some(false));
        assert!(!b.change(id + 1, daily(18), None, None));
    }
}
