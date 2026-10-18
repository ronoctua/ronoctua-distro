//! An instant, and how to ask what o'clock it is where the person lives.
//!
//! # Why an instant and never a wall time
//!
//! Because a wall time is not a moment. `03:00` happens twice in a year and
//! once not at all, and a scheduler that stored `03:00` would run twice on one
//! night and skip another — the failure nobody reproduces because it needs a
//! particular Sunday. What is stored is seconds since the epoch, which is
//! monotonic in the only sense that matters here: it never repeats and never
//! skips.
//!
//! The local hour is DERIVED, at the instant in question, through
//! [`Offset::seconds_at`]. That is also why the offset is a function of the
//! instant rather than a number: a zone's offset is not a property of the zone,
//! it is a property of the zone AT A TIME.
//!
//! # Why the offset is a trait
//!
//! Two reasons, and the second is the one that pays. The machine's real answer
//! is `ronoctua_files::when::Zone`, which reads `/etc/localtime` — this crate
//! is not growing a second TZif reader. And a test needs a zone that JUMPS, on
//! a day it chooses, which no fixture file gives cheaply: with a trait the test
//! writes six lines and drives the spring-forward edge directly.

/// An instant, in seconds since the epoch, in UTC.
///
/// Signed, because the arithmetic subtracts: `deadline - now` is negative once
/// the deadline has passed, and an unsigned type would wrap that into the year
/// 292 billion instead of into "it is late".
pub type Moment = i64;

/// Seconds in a day, as the clock counts them.
///
/// A constant with a name because the local-day arithmetic below uses it three
/// times, and because it is the number a reader should be able to check at a
/// glance rather than count zeros in.
pub const DAY: i64 = 86_400;

/// How far ahead of UTC this machine is, at a given instant.
///
/// The one thing the schedule needs from the outside world.
pub trait Offset: Send + Sync {
    /// Seconds east of UTC at this instant.
    fn seconds_at(&self, at: Moment) -> i32;
}

impl Offset for ronoctua_files::when::Zone {
    fn seconds_at(&self, at: Moment) -> i32 {
        self.offset_at(at)
    }
}

/// A zone that never moves — UTC, and whatever a machine with no
/// `/etc/localtime` gets.
///
/// Not a fallback anybody chooses: [`ronoctua_files::when::Zone`] already
/// answers UTC when it cannot read the file, and says so. This exists so a
/// test, and a machine deliberately running in UTC, have a type to name.
#[derive(Debug, Clone, Copy, Default)]
pub struct Fixed(pub i32);

impl Offset for Fixed {
    fn seconds_at(&self, _at: Moment) -> i32 {
        self.0
    }
}

/// Whoever can say what time it is.
///
/// A trait for the same reason [`Offset`] is one: the runner's whole decision
/// is *is it time yet*, and a test that had to WAIT for time to pass would
/// either be slow or would prove nothing. With this, a year goes by in a line.
pub trait Clock: Send + Sync {
    /// Now, in seconds since the epoch.
    fn now(&self) -> Moment;
}

/// The machine's clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct Wall;

impl Clock for Wall {
    fn now(&self) -> Moment {
        // A clock that went backwards before 1970 is not a machine this runs
        // on, and `unwrap_or_default` is a zero — which reads as "the epoch"
        // and puts every schedule in the far future rather than panicking in a
        // daemon.
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
    }
}

/// The local day number — days since 1970-01-01 where the person lives.
///
/// A number and not a date: "every seven days" is arithmetic on days, and a
/// calendar would be a month table this crate has no use for.
#[must_use]
pub fn local_day(at: Moment, zone: &dyn Offset) -> i64 {
    // `div_euclid` and not `/`: before 1970 the seconds are negative and
    // truncating division puts the day one off for every one of them. The same
    // care `ronoctua_files::when::civil` takes, and for the same reason.
    (at + i64::from(zone.seconds_at(at))).div_euclid(DAY)
}

/// The local hour, 0 to 23.
#[must_use]
pub fn local_hour(at: Moment, zone: &dyn Offset) -> u32 {
    let inside = (at + i64::from(zone.seconds_at(at))).rem_euclid(DAY);
    u32::try_from(inside / 3600).unwrap_or(0)
}

/// The instant at which a given local hour of a given local day begins.
///
/// # Why it is asked TWICE
///
/// Because turning a local time into an instant needs the offset, and the
/// offset needs the instant. The first pass uses the offset in force at the
/// guessed instant and the second corrects it — which is exactly what happens
/// on the two days a year the two disagree. Without the correction, every
/// schedule would be an hour off for half the year, in the direction nobody
/// notices until the run lands at midnight.
///
/// It can still land on an hour that does not exist (the spring forward skips
/// one) or on one that happens twice (the autumn back). The caller is
/// the daemon's `schedule`, which asks for a WINDOW and not for an instant, so a
/// missing hour simply means the window opens at the next one that exists.
#[must_use]
pub fn at_local_hour(day: i64, hour: u32, zone: &dyn Offset) -> Moment {
    let wall = day * DAY + i64::from(hour) * 3600;
    let guess = wall - i64::from(zone.seconds_at(wall));
    wall - i64::from(zone.seconds_at(guess))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A zone that is `+1` until an instant and `+2` after it — the spring
    /// forward, with nothing to download.
    struct Jumps(Moment);

    impl Offset for Jumps {
        fn seconds_at(&self, at: Moment) -> i32 {
            if at < self.0 { 3600 } else { 7200 }
        }
    }

    #[test]
    fn a_day_boundary_is_local_and_not_utc() {
        // 1970-01-01 22:30 UTC is already the 2nd where the offset is +2. A
        // scheduler that counted days in UTC would run "tomorrow" today.
        let at = 22 * 3600 + 1800;
        assert_eq!(local_day(at, &Fixed(0)), 0);
        assert_eq!(local_day(at, &Fixed(2 * 3600)), 1);
        // And westwards, the other way: 01:00 UTC is still yesterday evening.
        assert_eq!(local_day(3600, &Fixed(-3 * 3600)), -1);
    }

    #[test]
    fn a_local_hour_survives_the_jump_that_moves_the_clock() {
        // THE DEFECT THIS CATCHES: an offset taken once, at the moment of the
        // question, and used for an instant on the other side of a transition.
        // The whole schedule comes out an hour off for half the year, and the
        // symptom is a run at midnight that nobody connects to a Sunday in
        // March.
        //
        // The zone jumps from +1 to +2 at day 10, 01:00 UTC.
        let jump = 10 * DAY + 3600;
        let zone = Jumps(jump);

        // Before the jump: 00:00 local on day 10 is 23:00 UTC on day 9.
        let before = at_local_hour(10, 0, &zone);
        assert_eq!(local_hour(before, &zone), 0);
        assert_eq!(local_day(before, &zone), 10);

        // After it: 15:00 local on day 10 is 13:00 UTC, because the offset in
        // force THERE is +2 — not the +1 that was in force when the question
        // was asked.
        let after = at_local_hour(10, 15, &zone);
        assert_eq!(after, 10 * DAY + 13 * 3600);
        assert_eq!(local_hour(after, &zone), 15);
    }
}
