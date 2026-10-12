//! Instants as the agents write them, and days as a person reads them.
//!
//! # Why by hand
//!
//! Three shapes arrive: Claude Code's `2026-10-05T12:34:56.789Z`, the usage
//! endpoint's `2026-10-05T17:00:00.943648+00:00`, and Codex's
//! `2026-10-05T12:34:56.123Z`. All three are RFC 3339 and all that is wanted
//! out of them is seconds since the epoch; a date crate would be a dependency
//! for twenty lines of civil arithmetic (Howard Hinnant's, the one
//! `ronoctua-files`' `when` also uses).

/// Seconds in an hour.
pub(crate) const HOUR_S: i64 = 3600;

/// Seconds in a day.
pub(crate) const DAY_S: i64 = 86_400;

/// Now, in seconds since the epoch.
pub(crate) fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
pub(crate) const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day since 1970-01-01.
pub(crate) const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

/// `YYYY-MM-DD` of an instant, `offset_s` east of UTC.
pub(crate) fn day_of(at_s: i64, offset_s: i32) -> String {
    let (y, m, d) = civil_from_days((at_s + i64::from(offset_s)).div_euclid(DAY_S));
    format!("{y:04}-{m:02}-{d:02}")
}

/// The start of the hour an instant is in.
pub(crate) const fn hour_of(at_s: i64) -> i64 {
    at_s.div_euclid(HOUR_S) * HOUR_S
}

/// Seconds since the epoch of an RFC 3339 instant: `YYYY-MM-DDTHH:MM:SS`, an
/// optional fraction (dropped), then `Z` or `±HH:MM`. A space for the `T` and
/// a lower-case `z` are taken too (RFC 3339 §5.6 allows both). `None` for
/// anything else — never a guess.
pub(crate) fn parse_rfc3339(text: &str) -> Option<i64> {
    let b = text.trim().as_bytes();
    let number = |from: usize, len: usize| -> Option<i64> {
        let digits = b.get(from..from + len)?;
        if !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(digits).ok()?.parse().ok()
    };
    let at = |i: usize, c: u8| b.get(i) == Some(&c);
    if !(at(4, b'-') && at(7, b'-') && (at(10, b'T') || at(10, b't') || at(10, b' '))) {
        return None;
    }
    if !(at(13, b':') && at(16, b':')) {
        return None;
    }
    let (year, month, day) = (number(0, 4)?, number(5, 2)?, number(8, 2)?);
    let (hour, minute, second) = (number(11, 2)?, number(14, 2)?, number(17, 2)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut i = 19;
    if at(i, b'.') {
        i += 1;
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    let offset_s = match b.get(i) {
        Some(b'Z' | b'z') if i + 1 == b.len() => 0,
        Some(sign @ (b'+' | b'-')) if i + 6 == b.len() && at(i + 3, b':') => {
            let (oh, om) = (number(i + 1, 2)?, number(i + 4, 2)?);
            if oh > 23 || om > 59 {
                return None;
            }
            let east = oh * HOUR_S + om * 60;
            if *sign == b'+' { east } else { -east }
        }
        _ => return None,
    };
    let days = days_from_civil(year, month, day);
    Some(days * DAY_S + hour * HOUR_S + minute * 60 + second - offset_s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_shapes_the_agents_write_read_the_same_instant() {
        let at = days_from_civil(2026, 10, 5) * DAY_S + 12 * HOUR_S + 34 * 60 + 56;
        assert_eq!(parse_rfc3339("2026-10-05T12:34:56.789Z"), Some(at));
        assert_eq!(parse_rfc3339("2026-10-05T12:34:56Z"), Some(at));
        assert_eq!(
            parse_rfc3339("2026-10-05T15:34:56.943648+03:00"),
            Some(at),
            "an offset east is taken off"
        );
        assert_eq!(parse_rfc3339("2026-10-05T09:34:56-03:00"), Some(at));
        assert_eq!(parse_rfc3339("2026-10-05 12:34:56z"), Some(at));
        // The epoch itself, as a fixed point of the arithmetic.
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2000-02-29T00:00:00Z"), Some(951_782_400));
    }

    #[test]
    fn anything_else_is_refused_and_never_guessed() {
        for bad in [
            "",
            "2026-10-05",
            "2026-10-05T12:34:56",
            "2026-13-05T12:34:56Z",
            "2026-10-05T24:00:00Z",
            "2026-10-05T12:34:56.Z",
            "2026-10-05T12:34:56+0300",
            "2026-10-05T12:34:56Zjunk",
            "197O-01-01T00:00:00Z",
            "1738425600",
        ] {
            assert_eq!(parse_rfc3339(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_day_is_the_civil_date_in_the_zone_asked() {
        let noon = days_from_civil(2026, 10, 5) * DAY_S + 12 * HOUR_S;
        assert_eq!(day_of(noon, 0), "2026-10-05");
        assert_eq!(day_of(noon + 11 * HOUR_S, 0), "2026-10-05");
        assert_eq!(
            day_of(noon + 11 * HOUR_S, 3 * 3600),
            "2026-10-06",
            "23:00 UTC is tomorrow three hours east"
        );
        assert_eq!(day_of(noon - 13 * HOUR_S, -3 * 3600), "2026-10-04");
        for days in [-1, 0, 59, 60, 365, 11_016, 20_366, 100_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days, "round trip of {days}");
        }
        assert_eq!(hour_of(noon + 59 * 60 + 59), noon);
        assert_eq!(hour_of(-1), -HOUR_S);
    }
}
