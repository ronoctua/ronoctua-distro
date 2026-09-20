//! "Every seven days, in the afternoon" — turned into one instant.
//!
//! # Why the interval is counted in LOCAL DAYS
//!
//! Because a day is not eighty-six thousand four hundred seconds twice a year,
//! and "every seven days" said by a person means seven of THEIR days. Counting
//! in seconds makes a weekly task drift by an hour each spring and back each
//! autumn — which nobody would ever notice, right up to the cadence that lands
//! it outside its own window and skips a week.
//!
//! # Why the search walks days and does not solve for the answer
//!
//! Because the answer is not always there: the hour a window opens on can fail
//! to exist (the spring forward skips one) and a whole window could in
//! principle be erased by a zone change. A walk asks the same question of each
//! day — *does this day have a moment inside the window, at or after now?* —
//! and takes the first day that says yes. It costs at most a few iterations of
//! integer arithmetic, and it is right on the two days a year that closed-form
//! arithmetic is wrong.

use crate::moment::{DAY, Moment, Offset, at_local_hour, local_day};
use crate::prefs::TaskPrefs;

/// How many days forward the search is allowed to look.
///
/// A year and a day. With `any` the first day always answers, and every other
/// window is at least three hours long, so nothing that exists needs a second
/// iteration — the ceiling is here because an unbounded loop inside a daemon is
/// a hang, and a hang in this one is a machine that stops deciding anything.
const HORIZON: i64 = 366;

/// The next instant this task may run at.
///
/// `last` is when it last ran; `None` means never, which is a FIRST run and not
/// an overdue one — the first window from now, rather than a moment in the past
/// that would make everything look late on a fresh install.
#[must_use]
pub fn next_moment(
    now: Moment,
    last: Option<Moment>,
    prefs: &TaskPrefs,
    zone: &dyn Offset,
) -> Moment {
    // A CADENCE IN MINUTES SHORT-CIRCUITS THE WHOLE CALENDAR.
    //
    // A watch has no good hour — it has an interval — so there is no window to
    // find, no day to walk and no zone to consult. It is also the one shape
    // where the arithmetic below would be wrong rather than merely elaborate:
    // `local_day` would round a five-minute cadence to "today", and every
    // answer would be the same moment.
    //
    // It is first because it is total: a task that declares one never wants
    // the rest of this function.
    if let Some(minutes) = prefs.every_minutes {
        let wait = i64::from(minutes).saturating_mul(60);
        return last.map_or(now, |l| l.saturating_add(wait));
    }
    let today = local_day(now, zone);
    // The earliest day it is allowed on: `interval_days` LOCAL days after the
    // one it last ran on. A run at 23:00 and the next at 01:00 are two days
    // apart on the clock and two minutes apart in fact — counting days is what
    // the person meant, and it is also what makes the answer stable across a
    // zone change.
    let earliest = last.map_or(today, |l| {
        local_day(l, zone) + i64::from(prefs.interval_days)
    });
    let mut day = today.max(earliest);
    // AND A TASK THAT IS REALLY LATE MAY RUN AT ANY HOUR.
    //
    // The window is a preference about a good moment, not a condition. On a
    // laptop that is asleep at four every night, a task wanting `night` would
    // wake up each morning to find the window shut and put itself off until
    // tomorrow — for ever, silently, and the row would go on showing a date it
    // never honours. The owner asked for the other behaviour in as many words:
    // it stays OWED and runs at the next quiet moment.
    //
    // A WHOLE INTERVAL LATE and not a minute late, so an ordinary day where the
    // machine happened to be busy at 02:00 still waits for the next night. Past
    // that, the window opens to the whole day and the thing that decides
    // "quiet" is the one that always decided it: `runner::judge`, against the
    // gate the person configured.
    //
    // It costs nothing when nothing is late, which is why it lives here and not
    // in a task: every task gets it, including the update, whose own schedule
    // had exactly this hole.
    let overdue = today >= earliest.saturating_add(i64::from(prefs.interval_days));
    let (from, to) = if overdue {
        (0, 24)
    } else {
        prefs.part_of_day.hours()
    };
    let last_day = day + HORIZON;
    while day <= last_day {
        let opens = at_local_hour(day, from, zone);
        // `to == 24` is the end of the day, which is the start of the next one.
        // Asking for hour 24 would be arithmetic nobody can read.
        let closes = if to >= 24 {
            at_local_hour(day + 1, 0, zone)
        } else {
            at_local_hour(day, to, zone)
        };
        let candidate = opens.max(now);
        if candidate < closes {
            return candidate;
        }
        day += 1;
    }
    // Unreachable with any window this crate can express, and it still answers
    // rather than looping: a moment a year away shows up on screen as a date
    // somebody can see is wrong, where a hang shows up as nothing at all.
    now + HORIZON * DAY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moment::Fixed;
    use crate::prefs::PartOfDay;

    fn every(days: u16, part: PartOfDay) -> TaskPrefs {
        TaskPrefs {
            interval_days: days,
            part_of_day: part,
            ..TaskPrefs::default()
        }
    }

    /// A zone that is `+1` until an instant and `+2` after it — a spring
    /// forward, with no file to download.
    struct Jumps(Moment);

    impl Offset for Jumps {
        fn seconds_at(&self, at: Moment) -> i32 {
            if at < self.0 { 3600 } else { 7200 }
        }
    }

    #[test]
    fn inside_the_window_the_answer_is_now() {
        // 14:00 UTC on day 0, window 13-18. Something overdue must not be
        // pushed to tomorrow because the arithmetic started from the window's
        // opening rather than from now.
        let now = 14 * 3600;
        let at = next_moment(now, None, &every(7, PartOfDay::Afternoon), &Fixed(0));
        assert_eq!(at, now);
    }

    #[test]
    fn past_todays_window_it_crosses_the_day_boundary() {
        // 06:00, and the person asked for the night (02-05). Today's window has
        // closed; the answer is tomorrow at 02:00 — not today at 02:00, which
        // is in the past, and not "in twenty-four hours", which would drift.
        let now = 6 * 3600;
        let at = next_moment(now, None, &every(1, PartOfDay::Night), &Fixed(0));
        assert_eq!(at, DAY + 2 * 3600);
    }

    #[test]
    fn the_interval_is_counted_in_days_and_not_in_seconds() {
        // It ran on day 0 at 17:00, and may run every seven days in the
        // afternoon. The answer is day 7 at 13:00 — the window's opening, not
        // "seven times 86400 seconds after 17:00", which would be 17:00 and
        // would drift out of the window the first time the zone moved.
        let last = 17 * 3600;
        let now = last + 3600;
        let at = next_moment(now, Some(last), &every(7, PartOfDay::Afternoon), &Fixed(0));
        assert_eq!(at, 7 * DAY + 13 * 3600);
    }

    #[test]
    fn a_run_late_at_night_does_not_cost_a_whole_extra_day() {
        // 23:30 local, in a zone two hours east: the local day is already
        // tomorrow, and the interval is counted from THAT day. Counting the UTC
        // day would put the next run twenty-four hours later than the person
        // asked for, every time.
        let zone = Fixed(2 * 3600);
        let last = 21 * 3600 + 1800; // 23:30 local on day 0
        assert_eq!(local_day(last, &zone), 0);
        let at = next_moment(last + 60, Some(last), &every(1, PartOfDay::Morning), &zone);
        // Day 1, 08:00 local, which is 06:00 UTC.
        assert_eq!(at, DAY + 6 * 3600);
    }

    #[test]
    fn a_task_a_whole_interval_late_may_run_at_any_hour() {
        // THE DEFECT THIS CATCHES: a laptop that is asleep at four in the
        // morning never sweeping anything, ever. A task wanting `night` finds
        // the window shut every time it wakes, puts itself off until tomorrow,
        // and the row goes on showing a date nothing ever honours.
        //
        // It ran on day 0 and may run every five days at night (02:00–05:00).
        // On day 5 at 11:00 the window has passed, so the answer is day 6 at
        // 02:00 — it is on time, and on time means the window.
        let mine = every(5, PartOfDay::Night);
        let on_time = next_moment(5 * DAY + 11 * 3600, Some(0), &mine, &Fixed(0));
        assert_eq!(
            on_time,
            6 * DAY + 2 * 3600,
            "a task on time skipped its window"
        );

        // AND ONE DAY LATE IS NOT LATE. A day where the machine happened to be
        // busy at 02:00 still waits for the next night — otherwise "at night"
        // would mean "at night, once" and then whenever, which is a preference
        // that stops being one after its first miss.
        assert_eq!(
            next_moment(6 * DAY + 11 * 3600, Some(0), &mine, &Fixed(0)),
            7 * DAY + 2 * 3600,
            "one missed window was enough to give up on the window"
        );

        // AND ON DAY 10 AT 11:00 it is a whole interval late, so it runs NOW —
        // subject to the gate, which is the thing that decides "quiet".
        let late = 10 * DAY + 11 * 3600;
        assert_eq!(
            next_moment(late, Some(0), &mine, &Fixed(0)),
            late,
            "a task five days overdue was made to wait for the small hours again"
        );

        // AND `Any` IS UNCHANGED, which is the proof this only widens: a task
        // with no window at all answered `now` before and still does.
        let anytime = every(5, PartOfDay::Any);
        assert_eq!(next_moment(late, Some(0), &anytime, &Fixed(0)), late);
    }

    #[test]
    fn the_window_is_local_across_a_zone_that_moves() {
        // THE DEFECT THIS CATCHES: the offset read once, at the moment of the
        // question, and used for an instant on the other side of a transition —
        // which puts every run an hour off for half the year. The zone jumps
        // from +1 to +2 at day 10, 01:00 UTC; the question is asked before the
        // jump, and the answer lands after it.
        let zone = Jumps(10 * DAY + 3600);
        let now = 9 * DAY + 20 * 3600; // day 9, 21:00 local (+1)
        let at = next_moment(now, None, &every(1, PartOfDay::Afternoon), &zone);
        // Day 10, 13:00 LOCAL. After the jump that is +2, so 11:00 UTC — and
        // not the 12:00 UTC the old offset would have given.
        assert_eq!(at, 10 * DAY + 11 * 3600);
        assert_eq!(crate::moment::local_hour(at, &zone), 13);
    }

    #[test]
    fn whenever_means_the_next_moment_and_not_the_next_morning() {
        let now = 3 * 3600 + 17;
        let at = next_moment(now, None, &every(1, PartOfDay::Any), &Fixed(0));
        assert_eq!(at, now, "`any` waited for a window that is the whole day");
    }
}
