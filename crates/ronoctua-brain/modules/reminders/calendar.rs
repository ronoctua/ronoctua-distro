//! When a reminder comes round: a local minute of a local day, turned into an
//! instant, and the next day a repetition falls on.
//!
//! # Why a reminder keeps a MINUTE OF THE DAY and not an instant
//!
//! Because "every day at 9:00" is nine o'clock where the person lives, on both
//! sides of the clocks changing. Stored as an instant plus 24 hours, it would
//! ring at eight for half the year. So a repetition is a rule over local days
//! and a local minute, and the instant is DERIVED, day by day, through the
//! offset in force on that day — the same two-pass the core's schedule uses
//! (`crate::moment::at_local_hour`), to the minute.
//!
//! # The two days a year the clock is not a line
//!
//! A minute that does not exist (the spring forward skips an hour) comes out
//! as the moment the clock passes it — 02:30 on that night rings at 03:30,
//! once. A minute that happens twice (the autumn back) rings once, at the
//! second of the two: the next occurrence is always asked for STRICTLY after
//! the last, so one day cannot ring twice.
//!
//! # The calendar is `ronoctua-files`'s
//!
//! A date is years, months and days, and turning a day number into one is
//! Howard Hinnant's arithmetic, already in the tree and checked there
//! (`ronoctua_files::when`). This file asks it, through UTC — a day number
//! has no zone.

use serde::{Deserialize, Serialize};

use crate::moment::{DAY, Moment, Offset};

/// A day of the week, Monday first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Weekday {
    /// Monday.
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
    /// Sunday.
    Sunday,
}

impl Weekday {
    /// All seven, Monday first.
    pub const ALL: [Self; 7] = [
        Self::Monday,
        Self::Tuesday,
        Self::Wednesday,
        Self::Thursday,
        Self::Friday,
        Self::Saturday,
        Self::Sunday,
    ];

    /// Which day of the week a local day number is. 1970-01-01 was a
    /// Thursday.
    #[must_use]
    pub fn of_day(day: i64) -> Self {
        let at = usize::try_from((day + 3).rem_euclid(7)).unwrap_or(0);
        Self::ALL[at]
    }
}

/// How a reminder comes round.
///
/// Tagged by `every`, so the file reads `{"every": "week", "days":
/// ["monday"]}` — the person may edit it by hand, and the module hears the
/// edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "every", rename_all = "kebab-case")]
pub enum Repeat {
    /// It rings once, and leaves.
    Once,
    /// Every day.
    Day,
    /// On these days of the week.
    Week {
        /// Never empty, in week order.
        days: Vec<Weekday>,
    },
    /// On this day of every month; a month without it (the 31st in April)
    /// rings on its last day.
    Month {
        /// 1 to 31.
        day: u8,
    },
    /// On this date of every year — a birthday. The 29th of February rings
    /// on the 28th when there is none.
    Year {
        /// 1 to 12.
        month: u8,
        /// 1 to 31.
        day: u8,
    },
}

/// A calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    /// With its century.
    pub year: i64,
    /// 1 to 12.
    pub month: u32,
    /// 1 to 31.
    pub day: u32,
}

/// The date of a local day number.
#[must_use]
pub fn date_of(day: i64) -> Date {
    let c = ronoctua_files::when::civil(day * DAY, &ronoctua_files::when::Zone::utc());
    Date {
        year: c.year,
        month: c.month,
        day: c.day,
    }
}

/// The local day number of a date, or `None` for a date the calendar does
/// not have (the 31st of April).
#[must_use]
pub fn day_of(year: i64, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || day == 0 || day > days_in(year, month) {
        return None;
    }
    ronoctua_files::when::epoch_of(year, month, day)
}

/// Whether a year has a 29th of February.
#[must_use]
pub const fn leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// How many days a month has in a year.
#[must_use]
pub const fn days_in(year: i64, month: u32) -> u32 {
    match month {
        2 if leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The local day number an instant falls on.
#[must_use]
pub fn local_day(at: Moment, zone: &dyn Offset) -> i64 {
    crate::moment::local_day(at, zone)
}

/// The local minute of the day an instant falls on, 0 to 1439.
#[must_use]
pub fn minute_of(at: Moment, zone: &dyn Offset) -> u16 {
    let inside = (at + i64::from(zone.seconds_at(at))).rem_euclid(DAY);
    u16::try_from(inside / 60).unwrap_or(0)
}

/// The instant a local minute of a local day happens.
///
/// Asked twice, for the reason `crate::moment::at_local_hour` gives: the
/// offset is needed to find the instant and the instant to find the offset.
/// A minute the spring forward skips comes out as the moment the clock
/// passes it.
#[must_use]
pub fn at_local(day: i64, minute: u16, zone: &dyn Offset) -> Moment {
    let wall = day * DAY + i64::from(minute) * 60;
    let guess = wall - i64::from(zone.seconds_at(wall));
    wall - i64::from(zone.seconds_at(guess))
}

/// The same date `months` later, on the same day — or the month's last day
/// when it has no such day.
#[must_use]
pub fn months_later(date: Date, months: i64) -> Date {
    let index = date.year * 12 + i64::from(date.month) - 1 + months;
    let year = index.div_euclid(12);
    let month = u32::try_from(index.rem_euclid(12) + 1).unwrap_or(1);
    Date {
        year,
        month,
        day: date.day.min(days_in(year, month)),
    }
}

/// How far ahead the search for the next occurrence may walk: a year and a
/// month. Every repetition lands inside it — the rarest, a 29th of February,
/// lands on the 28th in the years without one — and a bound is what keeps a
/// rule that could never land from hanging the daemon.
const HORIZON: i64 = 400;

/// Whether a repetition falls on a local day.
#[must_use]
pub fn falls_on(repeat: &Repeat, day: i64) -> bool {
    match repeat {
        Repeat::Once => false,
        Repeat::Day => true,
        Repeat::Week { days } => days.contains(&Weekday::of_day(day)),
        Repeat::Month { day: wanted } => {
            let d = date_of(day);
            d.day == u32::from(*wanted).min(days_in(d.year, d.month))
        }
        Repeat::Year { month, day: wanted } => {
            let d = date_of(day);
            d.month == u32::from(*month)
                && d.day == u32::from(*wanted).min(days_in(d.year, d.month))
        }
    }
}

/// The first instant STRICTLY after `after` a repetition rings at, at this
/// local minute; `None` for a reminder that rings once, or a rule that never
/// lands (a week with no days).
#[must_use]
pub fn next_after(
    repeat: &Repeat,
    minute: u16,
    after: Moment,
    zone: &dyn Offset,
) -> Option<Moment> {
    if matches!(repeat, Repeat::Once) {
        return None;
    }
    // FROM THE DAY BEFORE: a minute late in the evening on the day before,
    // in a zone far east, can be an instant that is already "today" in UTC.
    let first = local_day(after, zone) - 1;
    (first..=first + HORIZON)
        .filter(|d| falls_on(repeat, *d))
        .map(|d| at_local(d, minute, zone))
        .find(|t| *t > after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moment::Fixed;

    const HOUR: i64 = 3600;

    /// A zone that is `before` seconds east until an instant and `after`
    /// seconds east from it — a clock change, with nothing to download.
    struct Changes {
        at: Moment,
        before: i32,
        after: i32,
    }

    impl Offset for Changes {
        fn seconds_at(&self, at: Moment) -> i32 {
            if at < self.at {
                self.before
            } else {
                self.after
            }
        }
    }

    fn day(year: i64, month: u32, d: u32) -> i64 {
        day_of(year, month, d).expect("a real date")
    }

    #[test]
    fn a_day_number_is_a_date_and_back() {
        assert_eq!(
            date_of(0),
            Date {
                year: 1970,
                month: 1,
                day: 1
            }
        );
        let d = day(2026, 9, 30);
        assert_eq!(
            date_of(d),
            Date {
                year: 2026,
                month: 9,
                day: 30
            }
        );
        assert_eq!(Weekday::of_day(d), Weekday::Wednesday);
        assert_eq!(Weekday::of_day(0), Weekday::Thursday);
        assert_eq!(day_of(2026, 2, 29), None, "a 29th of February in 2026");
        assert!(day_of(2028, 2, 29).is_some());
        assert_eq!(day_of(2026, 4, 31), None);
        assert_eq!(day_of(2026, 13, 1), None);
    }

    #[test]
    fn a_month_later_keeps_the_day_or_takes_the_last() {
        let jan31 = Date {
            year: 2027,
            month: 1,
            day: 31,
        };
        assert_eq!(
            months_later(jan31, 1),
            Date {
                year: 2027,
                month: 2,
                day: 28
            }
        );
        assert_eq!(months_later(jan31, 12).year, 2028);
        assert_eq!(months_later(jan31, -1).month, 12);
    }

    #[test]
    fn every_day_at_nine_is_nine_on_both_sides_of_the_clock_change() {
        // THE DEFECT THIS CLOSES: a daily reminder kept as "the last one plus
        // a day" rings at eight for half the year. The zone goes from +0 to
        // +1 at 01:00 UTC on the 29th of March 2026, as Lisbon's does.
        let spring = day(2026, 3, 29) * DAY + HOUR;
        let zone = Changes {
            at: spring,
            before: 0,
            after: 3600,
        };
        let nine = 9 * 60;
        let the_day_before = at_local(day(2026, 3, 28), nine, &zone);
        assert_eq!(the_day_before, day(2026, 3, 28) * DAY + 9 * HOUR);
        let next = next_after(&Repeat::Day, nine, the_day_before, &zone).expect("a next");
        // 09:00 local on the 29th is 08:00 UTC: the offset THERE is +1.
        assert_eq!(next, day(2026, 3, 29) * DAY + 8 * HOUR);
        assert_eq!(minute_of(next, &zone), nine);
        assert_eq!(local_day(next, &zone), day(2026, 3, 29));
        let after = next_after(&Repeat::Day, nine, next, &zone).expect("a next");
        assert_eq!(after - next, DAY, "a day after a clock change is a day");
    }

    #[test]
    fn a_minute_the_clock_skips_rings_once_when_the_clock_passes_it() {
        // +1 until 01:00 UTC (02:00 local), +2 from then: 02:30 local does not
        // exist on that day.
        let d = day(2026, 3, 29);
        let zone = Changes {
            at: d * DAY + HOUR,
            before: 3600,
            after: 7200,
        };
        let half_past_two = 2 * 60 + 30;
        let rang = at_local(d, half_past_two, &zone);
        assert_eq!(minute_of(rang, &zone), 3 * 60 + 30, "it rang at 03:30");
        let from_the_evening_before = at_local(d - 1, 22 * 60, &zone);
        assert_eq!(
            next_after(&Repeat::Day, half_past_two, from_the_evening_before, &zone),
            Some(rang)
        );
        // AND THE NEXT DAY, back at 02:30 — once.
        let next = next_after(&Repeat::Day, half_past_two, rang, &zone).expect("a next");
        assert_eq!(local_day(next, &zone), d + 1);
        assert_eq!(minute_of(next, &zone), half_past_two);
    }

    #[test]
    fn a_minute_the_clock_repeats_rings_once() {
        // +2 until 01:00 UTC (03:00 local), +1 from then: 02:30 local happens
        // twice on that night.
        let d = day(2026, 10, 25);
        let zone = Changes {
            at: d * DAY + HOUR,
            before: 7200,
            after: 3600,
        };
        let half_past_two = 2 * 60 + 30;
        let evening = at_local(d - 1, 22 * 60, &zone);
        let first = next_after(&Repeat::Day, half_past_two, evening, &zone).expect("a next");
        assert_eq!(minute_of(first, &zone), half_past_two);
        let second = next_after(&Repeat::Day, half_past_two, first, &zone).expect("a next");
        assert_eq!(local_day(second, &zone), d + 1, "the same night rang twice");
    }

    #[test]
    fn a_week_a_month_and_a_year_land_where_they_say() {
        let utc = Fixed(0);
        let wednesday = day(2026, 9, 30);
        let now = at_local(wednesday, 12 * 60, &utc);
        let week = Repeat::Week {
            days: vec![Weekday::Monday, Weekday::Wednesday],
        };
        // Wednesday noon has passed 08:30: next Monday.
        let next = next_after(&week, 8 * 60 + 30, now, &utc).expect("a next");
        assert_eq!(local_day(next, &utc), day(2026, 10, 5));
        // …and at 18:00 it is still today.
        assert_eq!(
            next_after(&week, 18 * 60, now, &utc).map(|t| local_day(t, &utc)),
            Some(wednesday)
        );
        // THE 31ST IN A MONTH OF THIRTY is its last day.
        let month = Repeat::Month { day: 31 };
        assert_eq!(
            next_after(&month, 13 * 60, now, &utc).map(|t| local_day(t, &utc)),
            Some(wednesday),
            "the 30th of September is September's last day"
        );
        let after =
            next_after(&month, 9 * 60, at_local(wednesday, 23 * 60, &utc), &utc).expect("a next");
        assert_eq!(local_day(after, &utc), day(2026, 10, 31));
        // A BIRTHDAY ON THE 29TH OF FEBRUARY rings on the 28th without one.
        let leapling = Repeat::Year { month: 2, day: 29 };
        assert_eq!(
            next_after(&leapling, 9 * 60, now, &utc).map(|t| local_day(t, &utc)),
            Some(day(2027, 2, 28))
        );
        assert_eq!(
            next_after(&leapling, 9 * 60, at_local(day(2027, 12, 1), 0, &utc), &utc)
                .map(|t| local_day(t, &utc)),
            Some(day(2028, 2, 29))
        );
        assert_eq!(next_after(&Repeat::Once, 0, now, &utc), None);
        assert_eq!(
            next_after(&Repeat::Week { days: Vec::new() }, 0, now, &utc),
            None,
            "a week with no days landed"
        );
    }

    #[test]
    fn a_zone_far_east_still_finds_this_evening() {
        // 23:00 at +13 is 10:00 UTC the same day: the search starts a day
        // early so the local day it is asked about is never skipped.
        let zone = Fixed(13 * 3600);
        let d = day(2026, 9, 30);
        let morning = at_local(d, 8 * 60, &zone);
        assert_eq!(
            next_after(&Repeat::Day, 23 * 60, morning, &zone).map(|t| local_day(t, &zone)),
            Some(d)
        );
    }
}
