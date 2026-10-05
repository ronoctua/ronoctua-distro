//! A modification time into a date a person recognises.
//!
//! # Why this reads `/etc/localtime` by hand
//!
//! `localtime_r` is the correct answer and needs `unsafe`, which this crate
//! forbids. A date crate is the other answer and brings a dependency to solve
//! what is, for a file listing, one question asked once: how many seconds is
//! this machine ahead of UTC right now.
//!
//! What is here is the TZif format the file already carries — a table of
//! transitions and the offset in force after each. Reading it costs a hundred
//! lines and, unlike either alternative, it is testable against a fixture.
//!
//! # The failure is VISIBLE, never silent
//!
//! An offset that is wrong by an hour looks exactly like an offset that is
//! right, in every row, forever. So when the file cannot be read or does not
//! parse, [`Zone`] says so ([`Zone::is_utc`]) and the screen labels the column
//! UTC instead of drawing a local time it cannot vouch for. A check compares
//! this reader against `coreutils date` on the machine's real zone — two
//! implementations disagreeing is the only thing that catches an off-by-one
//! hour, exactly as the two terminal engines catch a grid divergence.

use std::path::Path;

/// How far ahead of UTC this machine is, at a given instant.
#[derive(Debug, Clone, Default)]
pub struct Zone {
    /// Transition instants, ascending.
    at: Vec<i64>,
    /// The index into `offsets` in force after each transition.
    which: Vec<u8>,
    /// The offsets, in seconds east of UTC.
    offsets: Vec<i32>,
    /// The offset before the first transition — and the whole answer for a zone
    /// that has none, which is what `UTC` itself is.
    first: i32,
    /// Whether this came from a file at all.
    read: bool,
}

impl Zone {
    /// UTC, and honest about it.
    #[must_use]
    pub fn utc() -> Self {
        Self::default()
    }

    /// Reads a TZif file — `/etc/localtime` on a running system.
    ///
    /// Never fails: an unreadable or unrecognised file gives UTC with
    /// [`Zone::is_utc`] true, and the screen says so.
    #[must_use]
    pub fn read_from(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| Self::parse(&b))
            .unwrap_or_default()
    }

    /// Whether this zone is the fallback rather than a file that was read.
    #[must_use]
    pub const fn is_utc(&self) -> bool {
        !self.read
    }

    /// Seconds east of UTC at this instant.
    #[must_use]
    pub fn offset_at(&self, secs: i64) -> i32 {
        // `partition_point` gives how many transitions are at or before this
        // instant; zero means "before the first", which is what `first` is for.
        let n = self.at.partition_point(|t| *t <= secs);
        if n == 0 {
            return self.first;
        }
        self.which
            .get(n - 1)
            .and_then(|i| self.offsets.get(usize::from(*i)))
            .copied()
            .unwrap_or(self.first)
    }

    /// The pure half: the bytes of a TZif file in, a zone out.
    ///
    /// Reads the **version 2** block, which is the 64-bit one. Every
    /// `/etc/localtime` written this century has it, and the version 1 block
    /// that precedes it stops in 2038 — parsing that one instead would work
    /// today and break on a date nobody would connect to this function.
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 44 || &bytes[..4] != b"TZif" {
            return None;
        }
        let version = bytes[4];
        // Version 1 has no 64-bit block. It is readable and we refuse it: a
        // zone table that ends in 2038 is a wrong answer with a deadline.
        if version < b'2' {
            return None;
        }
        let head = Head::read(bytes, 0)?;
        // Skip the whole version 1 block, whose times are 4 bytes wide.
        let skip = head.block_size(4)?;
        let second_at = 44usize.checked_add(skip)?;
        let head = Head::read(bytes, second_at)?;
        let mut at = second_at.checked_add(44)?;

        let times = read_many(bytes, &mut at, head.timecnt, 8, |b| {
            i64::from_be_bytes(b.try_into().ok()?).into()
        })?;
        let which = read_bytes(bytes, &mut at, head.timecnt)?;
        let mut offsets = Vec::with_capacity(head.typecnt);
        for _ in 0..head.typecnt {
            // `ttinfo`: utoff (i32), isdst (u8), desigidx (u8).
            let raw = bytes.get(at..at.checked_add(4)?)?;
            offsets.push(i32::from_be_bytes(raw.try_into().ok()?));
            at = at.checked_add(6)?;
        }
        if offsets.is_empty() {
            return None;
        }
        // The offset before the first transition. POSIX says the first
        // non-DST type, falling back to the first type — and a zone with no
        // transitions at all (UTC) has only that.
        let first = *offsets.first()?;
        Some(Self {
            at: times,
            which,
            offsets,
            first,
            read: true,
        })
    }
}

/// The six counts at the top of a TZif header.
struct Head {
    isutcnt: usize,
    isstdcnt: usize,
    leapcnt: usize,
    timecnt: usize,
    typecnt: usize,
    charcnt: usize,
}

impl Head {
    /// Reads the header at `from`; the magic and version occupy the first 20
    /// bytes and the six counts the next 24.
    fn read(bytes: &[u8], from: usize) -> Option<Self> {
        let raw = bytes.get(from..from.checked_add(44)?)?;
        if &raw[..4] != b"TZif" {
            return None;
        }
        let n = |i: usize| -> Option<usize> {
            let at = 20 + i * 4;
            let word: [u8; 4] = raw.get(at..at + 4)?.try_into().ok()?;
            usize::try_from(u32::from_be_bytes(word)).ok()
        };
        Some(Self {
            isutcnt: n(0)?,
            isstdcnt: n(1)?,
            leapcnt: n(2)?,
            timecnt: n(3)?,
            typecnt: n(4)?,
            charcnt: n(5)?,
        })
    }

    /// How many bytes the data block after this header occupies, given the
    /// width of a transition time.
    fn block_size(&self, time_width: usize) -> Option<usize> {
        let leap_width = time_width.checked_add(4)?;
        self.timecnt
            .checked_mul(time_width)?
            .checked_add(self.timecnt)?
            .checked_add(self.typecnt.checked_mul(6)?)?
            .checked_add(self.charcnt)?
            .checked_add(self.leapcnt.checked_mul(leap_width)?)?
            .checked_add(self.isstdcnt)?
            .checked_add(self.isutcnt)
    }
}

fn read_many<T>(
    bytes: &[u8],
    at: &mut usize,
    count: usize,
    width: usize,
    of: impl Fn(&[u8]) -> Option<T>,
) -> Option<Vec<T>> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let end = at.checked_add(width)?;
        out.push(of(bytes.get(*at..end)?)?);
        *at = end;
    }
    Some(out)
}

fn read_bytes(bytes: &[u8], at: &mut usize, count: usize) -> Option<Vec<u8>> {
    let end = at.checked_add(count)?;
    let out = bytes.get(*at..end)?.to_vec();
    *at = end;
    Some(out)
}

/// A date and a time, already in local terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Civil {
    /// The year, with its century.
    pub year: i64,
    /// 1 to 12.
    pub month: u32,
    /// 1 to 31.
    pub day: u32,
    /// 0 to 23.
    pub hour: u32,
    /// 0 to 59.
    pub minute: u32,
    /// 0 to 59.
    ///
    /// Not in [`Civil::args`], and deliberately: that is the COLUMN's contract,
    /// where a second would be three cells nobody scans for. It is here for the
    /// sentence the pointer asks for, which has a whole line — the owner's own
    /// request, *"a data completa no idioma do usuário"*, and a date without
    /// its seconds is not complete.
    pub second: u32,
}

impl Civil {
    /// The arguments the `files-when` key takes, zero-padded where the format
    /// needs it.
    ///
    /// Padded here and not in the `.ftl` because a translation cannot pad: the
    /// column has to line up whatever the language does with the order.
    #[must_use]
    pub fn args(&self) -> [(&'static str, String); 5] {
        [
            ("y", self.year.to_string()),
            ("m", format!("{:02}", self.month)),
            ("d", format!("{:02}", self.day)),
            ("hh", format!("{:02}", self.hour)),
            ("mm", format!("{:02}", self.minute)),
        ]
    }
}

/// Seconds since the epoch, in this zone.
#[must_use]
pub fn civil(secs: i64, zone: &Zone) -> Civil {
    let local = secs.saturating_add(i64::from(zone.offset_at(secs)));
    // `div_euclid` and not `/`: before 1970 the seconds are negative, and
    // truncating division would put the day one off for every date in that
    // half. A file with a pre-epoch mtime is rare and entirely legal.
    let days = local.div_euclid(86_400);
    let rest = local.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Civil {
        year,
        month,
        day,
        hour: u32::try_from(rest / 3600).unwrap_or(0),
        minute: u32::try_from((rest % 3600) / 60).unwrap_or(0),
        // `rem_euclid` ABOVE is what makes this right before 1970 as well: the
        // remainder is already in `0..86_400`, so the seconds of a pre-epoch
        // instant count forwards like any other.
        second: u32::try_from(rest % 60).unwrap_or(0),
    }
}

/// Days since 1970-01-01 into a calendar date.
///
/// Howard Hinnant's `days_from_civil` — the inverse of [`civil_from_days`].
///
/// Days since 1970-01-01, from a civil date. It is here because the trash reads
/// its own `DeletionDate` back: the spec writes local time with no zone marker,
/// so turning one into an instant needs exactly the arithmetic that made it.
///
/// `None` for a month or day outside the calendar, which is what a corrupt
/// `.trashinfo` looks like — and a date this cannot read is a file it must not
/// delete.
#[must_use]
pub fn epoch_of(year: i64, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = i64::from(month);
    let d = i64::from(day);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// An instant as a person TYPES one back: `2026-09-30 14:05:09`, in this
/// zone, seconds included.
///
/// The one shape the info panel prefills its "modified" question with and
/// [`instant_of`] reads back, so what the question offers is by construction
/// something it accepts. ISO 8601's order and not the language's: it is the
/// shape that sorts, that every `touch -d` in the world takes, and that has no
/// month-before-day ambiguity to translate.
#[must_use]
pub fn typed(secs: i64, zone: &Zone) -> String {
    let c = civil(secs, zone);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
}

/// What a person typed as a date — `YYYY-MM-DD`, with `HH:MM` or `HH:MM:SS`
/// after a space or a `T` — as seconds since the epoch, read in this zone.
///
/// `None` for anything else: a date this cannot read is refused, never
/// guessed at (a month past twelve, a minute past fifty-nine, a word). A
/// date alone is its midnight.
///
/// # Local time back to an instant, in two steps
///
/// The zone's offset depends on the instant, and the instant is what is being
/// worked out. The first guess takes the offset at the local reading as if it
/// were an instant; the second asks again at that guess — which is what keeps
/// a time typed on the day the clocks move from landing an hour out.
#[must_use]
pub fn instant_of(text: &str, zone: &Zone) -> Option<i64> {
    let text = text.trim();
    let (date, time) = match text.split_once(['T', ' ']) {
        Some((d, t)) => (d, Some(t.trim())),
        None => (text, None),
    };
    let mut parts = date.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=9999).contains(&year) {
        return None;
    }
    let days = epoch_of(year, month, day)?;
    // A DAY THE MONTH DOES NOT HAVE is refused, and `epoch_of` would roll the
    // 31st of April into May: the calendar's own inverse says whether the day
    // exists.
    if civil_from_days(days) != (year, month, day) {
        return None;
    }
    let (hour, minute, second) = match time {
        None => (0, 0, 0),
        Some(t) => {
            let mut t = t.split(':');
            let hour: i64 = t.next()?.parse().ok()?;
            let minute: i64 = t.next()?.parse().ok()?;
            let second: i64 = t.next().map_or(Some(0), |s| s.parse().ok())?;
            if t.next().is_some()
                || !(0..24).contains(&hour)
                || !(0..60).contains(&minute)
                || !(0..60).contains(&second)
            {
                return None;
            }
            (hour, minute, second)
        }
    };
    let local = days
        .checked_mul(86_400)?
        .checked_add(hour * 3600 + minute * 60 + second)?;
    let guess = local - i64::from(zone.offset_at(local));
    Some(local - i64::from(zone.offset_at(guess)))
}

/// Howard Hinnant's `civil_from_days`, exact for the whole proleptic Gregorian
/// range. It is arithmetic and not a table, which is why it needs no data and
/// cannot go stale.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Shift the epoch to 0000-03-01, so leap days land at the end of the cycle.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (
        year,
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_date_reads_back_as_the_instant_that_wrote_it() {
        let zone = Zone::utc();
        for secs in [0_i64, 1_759_240_000, -86_399, 951_782_400] {
            let text = typed(secs, &zone);
            assert_eq!(instant_of(&text, &zone), Some(secs), "{text}");
        }
        // The shapes a person types, and the midnight a date alone means.
        assert_eq!(instant_of("1970-01-02", &zone), Some(86_400));
        assert_eq!(instant_of("1970-01-01T00:01", &zone), Some(60));
        assert_eq!(instant_of(" 1970-01-01 00:00:05 ", &zone), Some(5));
    }

    #[test]
    fn a_date_that_does_not_exist_is_refused() {
        let zone = Zone::utc();
        for bad in [
            "",
            "yesterday",
            "2026-04-31",
            "2026-13-01",
            "2026-02-29",
            "2026-01-01 24:00",
            "2026-01-01 10:60",
            "2026-01-01 10:00:61",
            "2026-01-01 10",
            "2026-01-01-02",
            "0-01-01",
        ] {
            assert_eq!(instant_of(bad, &zone), None, "{bad:?}");
        }
        // And the day a leap year has is a day.
        assert!(instant_of("2024-02-29", &zone).is_some());
    }

    #[test]
    fn the_calendar_is_exact_at_the_places_it_usually_is_not() {
        // The epoch itself.
        // AND THE INVERSE AGREES, which is the only thing that makes a date
        // written by `Trash::put` readable by `Trash::expire`.
        for days in [0_i64, 11_016, -25_508, 20_000, 1] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(epoch_of(y, m, d), Some(days), "{y}-{m}-{d}");
        }
        assert_eq!(epoch_of(2026, 13, 1), None, "a thirteenth month was read");
        assert_eq!(epoch_of(2026, 1, 0), None, "a zeroth day was read");
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2000 is a leap year and 1900 is not — the rule most hand-rolled
        // calendars get wrong, and the reason this is arithmetic and not
        // `days / 365`.
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(-25_508), (1900, 3, 1));
        // The 32-bit end of the world, which is where a version 1 TZif stops.
        assert_eq!(civil_from_days(24_855), (2038, 1, 19));
    }

    #[test]
    fn a_date_before_the_epoch_does_not_slip_a_day() {
        // THE DEFECT THIS CATCHES: truncating division on a negative second
        // count, which puts every pre-1970 file one day early. Legal mtimes,
        // and nothing on screen would say why.
        let c = civil(-1, &Zone::utc());
        assert_eq!((c.year, c.month, c.day), (1969, 12, 31));
        assert_eq!((c.hour, c.minute), (23, 59));
    }

    #[test]
    fn utc_is_utc_and_says_so() {
        let z = Zone::utc();
        assert!(z.is_utc());
        assert_eq!(z.offset_at(1_755_000_000), 0);
        let c = civil(0, &z);
        assert_eq!(
            (c.year, c.month, c.day, c.hour, c.minute),
            (1970, 1, 1, 0, 0)
        );
    }

    #[test]
    fn garbage_is_refused_instead_of_guessed_at() {
        assert!(Zone::parse(b"").is_none());
        assert!(
            Zone::parse(b"not a tzif file at all, but long enough to pass a length test aaaaaaa")
                .is_none()
        );
        // A version 1 file is REFUSED: its table stops in 2038, and a wrong
        // answer with a deadline is the worst kind.
        let mut v1 = vec![0u8; 44];
        v1[..4].copy_from_slice(b"TZif");
        v1[4] = 0;
        assert!(Zone::parse(&v1).is_none());
    }

    #[test]
    fn the_machines_own_zone_parses() {
        // Not an assertion about WHICH zone — that changes by machine. It
        // asserts the file this system actually has is one this reader
        // understands, which is the thing that would silently fall back.
        let path = Path::new("/etc/localtime");
        if !path.exists() {
            return;
        }
        let z = Zone::read_from(path);
        assert!(!z.is_utc(), "/etc/localtime exists and was not read");
        // Sane offset: no zone is more than 14 h from UTC.
        let off = z.offset_at(1_755_000_000);
        assert!(
            (-50_400..=50_400).contains(&off),
            "an impossible offset: {off}s"
        );
    }
}
