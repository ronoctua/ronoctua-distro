//! A moment a source writes, as seconds since the epoch: RFC 3339 (Atom,
//! PeerTube, NASA, Commons) and RFC 822 (RSS's `pubDate`).
//!
//! Pure arithmetic and no clock: a date a feed wrote is data, and "now" is
//! never read here. A date that does not parse is `None` — a feed that writes
//! a date nobody can read is still a feed, and its item is undated.

/// Days from 1970-01-01 to the civil date (proleptic Gregorian), Howard
/// Hinnant's `days_from_civil`.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Seconds since the epoch of a civil date and time, in UTC; `None` for one
/// that does not exist (a 31st of April, a 25th hour).
fn at(year: i64, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Option<i64> {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let month_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=month_days).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    Some(
        days_from_civil(year, month, day) * 86_400
            + i64::from(hour) * 3600
            + i64::from(minute) * 60
            + i64::from(second.min(59)),
    )
}

/// The digits of `text[from..from + n]` as a number.
fn digits(text: &str, from: usize, n: usize) -> Option<u32> {
    let part = text.get(from..from + n)?;
    part.bytes()
        .all(|b| b.is_ascii_digit())
        .then(|| part.parse().ok())?
}

/// An RFC 3339 moment (`2026-09-27T00:40:21.698Z`, `2019-07-16T00:00:00+02:00`)
/// or a bare date (`2019-07-16`), in seconds since the epoch.
#[must_use]
pub fn rfc3339(text: &str) -> Option<i64> {
    let t = text.trim();
    let year = i64::from(digits(t, 0, 4)?);
    if t.get(4..5) != Some("-") || t.get(7..8) != Some("-") {
        return None;
    }
    let month = digits(t, 5, 2)?;
    let day = digits(t, 8, 2)?;
    if t.len() == 10 {
        return at(year, month, day, 0, 0, 0);
    }
    if !matches!(t.get(10..11), Some("T" | "t" | " ")) || t.get(13..14) != Some(":") {
        return None;
    }
    let hour = digits(t, 11, 2)?;
    let minute = digits(t, 14, 2)?;
    let (second, mut rest) = if t.get(16..17) == Some(":") {
        (digits(t, 17, 2)?, t.get(19..)?)
    } else {
        (0, t.get(16..)?)
    };
    if let Some(fraction) = rest.strip_prefix('.') {
        let end = fraction
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(fraction.len());
        rest = &fraction[end..];
    }
    let offset = zone(rest)?;
    Some(at(year, month, day, hour, minute, second)? - offset)
}

/// A zone as seconds east of UTC: `Z`, `+02:00`, `-0300`, or nothing (UTC).
fn zone(text: &str) -> Option<i64> {
    let z = text.trim();
    if z.is_empty() || z.eq_ignore_ascii_case("z") {
        return Some(0);
    }
    let sign = match z.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let rest = z[1..].replace(':', "");
    if rest.len() != 4 || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hours = i64::from(rest[..2].parse::<u32>().ok()?);
    let minutes = i64::from(rest[2..].parse::<u32>().ok()?);
    Some(sign * (hours * 3600 + minutes * 60))
}

/// An RFC 822 date as RSS writes it (`Tue, 10 Jun 2003 04:00:00 GMT`,
/// `10 Jun 03 04:00 +0100`), in seconds since the epoch.
#[must_use]
pub fn rfc822(text: &str) -> Option<i64> {
    let t = text.trim();
    // THE DAY'S NAME IS OPTIONAL, and ignored: it says nothing the date does not.
    let t = t.split_once(',').map_or(t, |(_, rest)| rest).trim();
    let mut parts = t.split_whitespace();
    let day: u32 = parts.next()?.parse().ok()?;
    let month = month_of(parts.next()?)?;
    let year_text = parts.next()?;
    let year: i64 = year_text.parse().ok()?;
    let year = match year_text.len() {
        2 if year < 50 => 2000 + year,
        2 => 1900 + year,
        4 => year,
        _ => return None,
    };
    let (hour, minute, second) = match parts.next() {
        Some(clock) => {
            let mut c = clock.split(':');
            let h: u32 = c.next()?.parse().ok()?;
            let m: u32 = c.next()?.parse().ok()?;
            let s: u32 = c.next().map_or(Some(0), |s| s.parse().ok())?;
            (h, m, s)
        }
        None => (0, 0, 0),
    };
    let offset = match parts.next() {
        None => 0,
        Some(z) => named_zone(z).or_else(|| zone(z))?,
    };
    Some(at(year, month, day, hour, minute, second)? - offset)
}

/// A month's English abbreviation (RFC 822 writes English, whatever the feed).
fn month_of(name: &str) -> Option<u32> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let lower = name.get(..3)?.to_ascii_lowercase();
    MONTHS
        .iter()
        .position(|m| *m == lower)
        .and_then(|n| u32::try_from(n + 1).ok())
}

/// The zones RFC 822 names, as seconds east of UTC.
fn named_zone(name: &str) -> Option<i64> {
    Some(match name.to_ascii_uppercase().as_str() {
        "GMT" | "UT" | "UTC" | "Z" => 0,
        "EST" => -5 * 3600,
        "EDT" => -4 * 3600,
        "CST" => -6 * 3600,
        "CDT" => -5 * 3600,
        "MST" => -7 * 3600,
        "MDT" => -6 * 3600,
        "PST" => -8 * 3600,
        "PDT" => -7 * 3600,
        _ => return None,
    })
}

/// The year of a moment in seconds since the epoch.
#[must_use]
pub fn year_of(seconds: i64) -> i32 {
    // Hinnant's `civil_from_days`, the year only.
    let z = seconds.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    i32::try_from(year).unwrap_or(i32::MAX)
}

/// The first second of a year (its 1 January, 00:00 UTC), in seconds since
/// the epoch: what an item that knows only its year is dated by.
#[must_use]
#[cfg_attr(not(feature = "catalog"), allow(dead_code))]
pub fn year_start(year: i32) -> Option<i64> {
    at(i64::from(year), 1, 1, 0, 0, 0)
}

/// A length a feed writes: `HH:MM:SS`, `MM:SS` or a number of seconds.
#[must_use]
pub fn clock_seconds(text: &str) -> Option<u32> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let mut total: u64 = 0;
    let mut parts = 0;
    for part in t.split(':') {
        parts += 1;
        if parts > 3 {
            return None;
        }
        // A FRACTION of a second is dropped, not refused.
        let whole = part.split('.').next()?;
        let n: u64 = whole.trim().parse().ok()?;
        total = total * 60 + n;
    }
    u32::try_from(total).ok().filter(|s| *s > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_atom_or_json_moment_is_seconds_since_the_epoch() {
        assert_eq!(rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339("2026-09-27T00:40:21.698Z"), Some(1_790_469_621));
        assert_eq!(
            rfc3339("2019-07-16T02:00:00+02:00"),
            rfc3339("2019-07-16T00:00:00Z")
        );
        assert_eq!(rfc3339("2019-07-16"), Some(1_563_235_200));
        assert_eq!(rfc3339("2019-02-30T00:00:00Z"), None, "no such day");
        assert_eq!(rfc3339("yesterday"), None);
        assert_eq!(rfc3339("2024-02-29T12:00:00-0300"), Some(1_709_218_800));
    }

    #[test]
    fn an_rss_date_is_seconds_since_the_epoch() {
        assert_eq!(rfc822("Tue, 10 Jun 2003 04:00:00 GMT"), Some(1_055_217_600));
        assert_eq!(
            rfc822("10 Jun 2003 06:00:00 +0200"),
            Some(1_055_217_600),
            "no day name, an offset"
        );
        assert_eq!(rfc822("Tue, 10 Jun 03 00:00:00 EDT"), Some(1_055_217_600));
        assert_eq!(rfc822("Wed, 31 Apr 2024 10:00:00 GMT"), None);
        assert_eq!(rfc822("sometime"), None);
    }

    #[test]
    fn a_year_and_a_length_are_read() {
        assert_eq!(year_of(0), 1970);
        assert_eq!(year_of(1_055_217_600), 2003);
        assert_eq!(year_of(-86_400), 1969);
        assert_eq!(year_start(1970), Some(0));
        assert_eq!(year_start(2003), rfc3339("2003-01-01T00:00:00Z"));
        assert_eq!(year_start(1968).map(year_of), Some(1968));
        assert_eq!(clock_seconds("01:02:03"), Some(3723));
        assert_eq!(clock_seconds("48:22"), Some(2902));
        assert_eq!(clock_seconds("1234"), Some(1234));
        assert_eq!(clock_seconds("12.5"), Some(12));
        assert_eq!(clock_seconds("0"), None);
        assert_eq!(clock_seconds("1:2:3:4"), None);
        assert_eq!(clock_seconds("long"), None);
    }
}
