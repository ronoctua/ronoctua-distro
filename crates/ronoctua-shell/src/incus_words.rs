//! The sentences the two Incus doors share — a refusal, a status, a kind, a
//! tab, what the queue was given — and the few readings of the daemon's own
//! words a screen needs (a moment, a size typed by a person): one place, so
//! the footer and the window cannot say it two ways.
//!
//! What is the daemon's own — an instance's name, an image's description, an
//! alias, an address — is DATA and is drawn as it came (invariant 8, the brain's
//! log line's exception). What is ours is a key.

use ronoctua_i18n::key;
use ronoctua_incus::{Job, Kind as Machine, Protocol, Refusal, Status, Why};

use crate::incus::{Field, Kind, Limit, Tab, Use};

/// A kind's name, in the person's language.
#[must_use]
pub const fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Instances => key::INCUS_KIND_INSTANCES,
        Kind::Images => key::INCUS_KIND_IMAGES,
        Kind::Remotes => key::INCUS_KIND_REMOTES,
        Kind::Profiles => key::INCUS_KIND_PROFILES,
        Kind::Networks => key::INCUS_KIND_NETWORKS,
        Kind::Storage => key::INCUS_KIND_STORAGE,
        Kind::Projects => key::INCUS_KIND_PROJECTS,
    }
}

/// What a kind's list says when it is empty.
#[must_use]
pub const fn empty_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Instances => key::INCUS_EMPTY_INSTANCES,
        Kind::Images => key::INCUS_EMPTY_IMAGES,
        Kind::Remotes => key::INCUS_EMPTY_REMOTES,
        Kind::Profiles => key::INCUS_EMPTY_PROFILES,
        Kind::Networks => key::INCUS_EMPTY_NETWORKS,
        Kind::Storage => key::INCUS_EMPTY_STORAGE,
        Kind::Projects => key::INCUS_EMPTY_PROJECTS,
    }
}

/// A kind's glyph token.
#[must_use]
pub const fn kind_glyph(kind: Kind) -> char {
    use ronoctua_tokens::icon;
    match kind {
        Kind::Instances => icon::INCUS_KIND_INSTANCES,
        Kind::Images => icon::INCUS_KIND_IMAGES,
        Kind::Remotes => icon::INCUS_KIND_REMOTES,
        Kind::Profiles => icon::INCUS_KIND_PROFILES,
        Kind::Networks => icon::INCUS_KIND_NETWORKS,
        Kind::Storage => icon::INCUS_KIND_STORAGE,
        Kind::Projects => icon::INCUS_KIND_PROJECTS,
    }
}

/// The glyph of a container, or of a virtual machine — before its name in
/// the list, so the two read apart at a glance.
#[must_use]
pub const fn machine_glyph(machine: Machine) -> char {
    use ronoctua_tokens::icon;
    match machine {
        Machine::Container => icon::INCUS_CONTAINER,
        Machine::Vm => icon::INCUS_VM,
    }
}

/// A container or a virtual machine, as a word.
#[must_use]
pub const fn machine_key(machine: Machine) -> &'static str {
    match machine {
        Machine::Container => key::INCUS_MACHINE_CONTAINER,
        Machine::Vm => key::INCUS_MACHINE_VM,
    }
}

/// A tab's name.
#[must_use]
pub const fn tab_key(tab: Tab) -> &'static str {
    match tab {
        Tab::Overview => key::INCUS_TAB_OVERVIEW,
        Tab::Snapshots => key::INCUS_TAB_SNAPSHOTS,
        Tab::Limits => key::INCUS_TAB_LIMITS,
        Tab::Actions => key::INCUS_TAB_ACTIONS,
    }
}

/// An instance's status, as a word.
#[must_use]
pub const fn status_key(status: Status) -> &'static str {
    match status {
        Status::Running => key::INCUS_STATUS_RUNNING,
        Status::Stopped => key::INCUS_STATUS_STOPPED,
        Status::Frozen => key::INCUS_STATUS_FROZEN,
        Status::Error => key::INCUS_STATUS_ERROR,
        Status::Other => key::INCUS_STATUS_OTHER,
    }
}

/// The colour a status says, beside its word: running is well, frozen a
/// warning, an error an error — nothing for the rest.
#[must_use]
pub const fn status_tint(status: Status) -> Option<ronoctua_tokens::Color> {
    use ronoctua_tokens::color;
    match status {
        Status::Running => Some(color::STATE_SUCCESS),
        Status::Frozen => Some(color::STATE_WARNING),
        Status::Error => Some(color::STATE_ERROR),
        Status::Stopped | Status::Other => None,
    }
}

/// A limit's label.
#[must_use]
pub const fn limit_key(limit: Limit) -> &'static str {
    match limit {
        Limit::Cpu => key::INCUS_LIMIT_CPU,
        Limit::Memory => key::INCUS_LIMIT_MEMORY,
        Limit::Processes => key::INCUS_LIMIT_PROCESSES,
    }
}

/// An action row's label (the Actions tab).
#[must_use]
pub const fn use_key(what: Use) -> &'static str {
    match what {
        Use::Console => key::INCUS_USE_CONSOLE,
        Use::Screen => key::INCUS_USE_SCREEN,
        Use::Shell => key::INCUS_USE_SHELL,
        Use::Push => key::INCUS_USE_PUSH,
        Use::Pull => key::INCUS_USE_PULL,
        Use::Export => key::INCUS_USE_EXPORT,
        Use::Publish => key::INCUS_USE_PUBLISH,
    }
}

/// Where an action row's work happens, as its value.
#[must_use]
pub const fn use_where_key(what: Use) -> &'static str {
    match what {
        Use::Console | Use::Shell => key::INCUS_USE_IN_TERMINAL,
        Use::Screen => key::INCUS_USE_IN_VIEWER,
        Use::Push | Use::Pull | Use::Export | Use::Publish => key::INCUS_USE_ON_QUEUE,
    }
}

/// A row of the new instance's draft: its label.
#[must_use]
pub const fn field_key(field: Field) -> &'static str {
    match field {
        Field::Machine => key::INCUS_FIELD_MACHINE,
        Field::Name => key::INCUS_FIELD_NAME,
        Field::Image => key::INCUS_FIELD_IMAGE,
        Field::Limit(limit) => limit_key(limit),
    }
}

/// A remote's protocol, as a word.
#[must_use]
pub const fn protocol_key(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::SimpleStreams => key::INCUS_PROTOCOL_SIMPLESTREAMS,
        Protocol::Oci => key::INCUS_PROTOCOL_OCI,
    }
}

/// Why the daemon answered nothing, as a sentence — the crate's closed set
/// (`incus-refused-<id>`).
#[must_use]
pub const fn why_key(why: Why) -> &'static str {
    match why {
        Why::NoIncus => key::INCUS_REFUSED_NO_INCUS,
        Why::NotInGroup => key::INCUS_REFUSED_NOT_IN_GROUP,
        Why::Gone => key::INCUS_REFUSED_GONE,
        Why::Busy => key::INCUS_REFUSED_BUSY,
        Why::NotLet => key::INCUS_REFUSED_NOT_LET,
        Why::Invalid => key::INCUS_REFUSED_INVALID,
        Why::Failed => key::INCUS_REFUSED_FAILED,
    }
}

/// A refusal of the daemon's, said.
#[must_use]
pub fn refused(refusal: Refusal, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(why_key(refusal.why))
}

/// What a job the queue was given is, in words: the queue's own stem for it
/// (`transfers-work-incus-…`), so the view and the queue's panel say it the
/// same way.
#[must_use]
pub fn job_said(job: &Job, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(&format!("transfers-work-{}", job.word()))
}

/// The keys that open the queue, as this leader's trees teach them — taught
/// once by each door (`leader::taught_keys`), never written in a text.
#[must_use]
pub fn queue_keys(trees: &crate::leader::Trees) -> String {
    crate::containers_words::queue_keys(trees)
}

/// What the message line says once a job went to the queue; `keys` are
/// [`queue_keys`].
#[must_use]
pub fn queued_said(job: &Job, label: &str, keys: &str, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_with(
        key::INCUS_ECHO_QUEUED,
        &[
            ("what", &job_said(job, texts)),
            ("name", label),
            ("keys", keys),
        ],
    )
}

/// A count as Fluent takes it.
#[must_use]
pub fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// A processor's time, read the way a person reads a duration: seconds
/// under a minute, minutes under an hour, then hours and minutes.
#[must_use]
pub fn cpu_time(ns: u64, texts: &ronoctua_i18n::Texts) -> String {
    let seconds = ns / 1_000_000_000;
    let n = |v: u64| ronoctua_i18n::Arg::Number(i64::try_from(v).unwrap_or(i64::MAX));
    if seconds < 60 {
        texts.t_args(key::INCUS_SECONDS, &[("count", n(seconds))])
    } else if seconds < 3600 {
        texts.t_args(key::INCUS_MINUTES, &[("count", n(seconds / 60))])
    } else {
        texts.t_args(
            key::INCUS_HOURS,
            &[
                ("hours", n(seconds / 3600)),
                ("minutes", n(seconds / 60 % 60)),
            ],
        )
    }
}

/// Hinnant's `days_from_civil`: the days from 1970-01-01 to a date.
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The moment the daemon wrote (RFC 3339: `2026-10-04T05:24:11.123456789Z`,
/// or with an offset `+02:00`), in seconds since the epoch. `None` for
/// anything else — the daemon's zero moment (`0001-01-01T00:00:00Z`, "never")
/// included, which a screen must not draw as a date. Pure.
#[must_use]
pub fn moment(text: &str) -> Option<i64> {
    let t = text.trim();
    let num = |from: usize, n: usize| -> Option<i64> {
        let part = t.get(from..from + n)?;
        part.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let year = num(0, 4)?;
    if year < 1971 || t.get(4..5) != Some("-") || t.get(7..8) != Some("-") {
        return None;
    }
    let (month, day) = (num(5, 2)?, num(8, 2)?);
    if !matches!(t.get(10..11), Some("T" | "t" | " "))
        || t.get(13..14) != Some(":")
        || t.get(16..17) != Some(":")
    {
        return None;
    }
    let (hour, minute, second) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut rest = t.get(19..)?;
    if let Some(fraction) = rest.strip_prefix('.') {
        let end = fraction
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(fraction.len());
        rest = &fraction[end..];
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.get(0..1)? {
                "+" => 1,
                "-" => -1,
                _ => return None,
            };
            let (h, m) = (
                rest.get(1..3)?.parse::<i64>().ok()?,
                rest.get(4..6)?.parse::<i64>().ok()?,
            );
            if rest.get(3..4) != Some(":") || rest.len() != 6 {
                return None;
            }
            sign * (h * 3600 + m * 60)
        }
    };
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// Why a size typed for a memory limit was not one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotASize {
    /// No unit, or one Incus does not know.
    Unit,
    /// Not a number, zero, or more than a machine holds.
    Number,
}

/// A size a person typed, in bytes, read as Incus reads `limits.memory`:
/// `512MiB` and `2GiB` are powers of 1024, `2GB` and `2G` powers of 1000 —
/// and a fraction (`1.5GiB`, or `1,5GiB` where the comma is the decimal mark)
/// is taken. A number with no unit is REFUSED: for the daemon it would be
/// bytes, and "2" meant as two gigabytes would be an instance with two bytes.
/// Pure.
///
/// # Errors
///
/// [`NotASize`].
pub fn bytes_of(said: &str) -> Result<u64, NotASize> {
    let compact: String = said.chars().filter(|c| !c.is_whitespace()).collect();
    let split = compact
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .ok_or(NotASize::Unit)?;
    let (number, unit) = compact.split_at(split);
    let scale: u64 = match unit.to_ascii_lowercase().as_str() {
        "b" => 1,
        "k" | "kb" => 1_000,
        "kib" => 1 << 10,
        "m" | "mb" => 1_000_000,
        "mib" => 1 << 20,
        "g" | "gb" => 1_000_000_000,
        "gib" => 1 << 30,
        "t" | "tb" => 1_000_000_000_000,
        "tib" => 1 << 40,
        _ => return Err(NotASize::Unit),
    };
    let number = number.replace(',', ".");
    let (whole, fraction) = number.split_once('.').unwrap_or((&number, ""));
    if whole.is_empty() && fraction.is_empty() || fraction.contains('.') {
        return Err(NotASize::Number);
    }
    let whole: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| NotASize::Number)?
    };
    let mut bytes = whole.checked_mul(scale).ok_or(NotASize::Number)?;
    // THE FRACTION IN INTEGERS, exactly: `1.5GiB` is three halves of a
    // power of two, and neither a float nor a tenth of 1024 taken digit by
    // digit says that.
    if !fraction.is_empty() {
        let digits = fraction.get(..18).unwrap_or(fraction);
        let numerator: u128 = digits.parse().map_err(|_| NotASize::Number)?;
        let denominator = 10u128.pow(u32::try_from(digits.len()).unwrap_or(18));
        let part = u64::try_from(numerator * u128::from(scale) / denominator)
            .map_err(|_| NotASize::Number)?;
        bytes = bytes.checked_add(part).ok_or(NotASize::Number)?;
    }
    if bytes == 0 {
        return Err(NotASize::Number);
    }
    Ok(bytes)
}

/// A whole number above zero a person typed — processors, processes.
#[must_use]
pub fn whole_of(said: &str) -> Option<u64> {
    said.trim().parse::<u64>().ok().filter(|n| *n > 0)
}

/// A size as an answer the person may type again: the largest binary unit
/// it is whole in (`2GiB`, `512MiB`), so a question asked again holds what
/// is there.
#[must_use]
pub fn bytes_typed(bytes: u64) -> String {
    for (unit, scale) in [
        ("TiB", 1u64 << 40),
        ("GiB", 1 << 30),
        ("MiB", 1 << 20),
        ("KiB", 1 << 10),
    ] {
        if bytes >= scale && bytes.is_multiple_of(scale) {
            return format!("{}{unit}", bytes / scale);
        }
    }
    format!("{bytes}B")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).unwrap()
    }

    /// FAILS IF: a kind, a tab, a status, a refusal, a limit, an action row,
    /// a field, a protocol or a job the queue runs has no words in one of the
    /// two languages.
    #[test]
    fn every_kind_tab_status_and_refusal_has_its_words() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let said = |k: &str| {
                assert!(t.has(k), "{language}: no text for {k}");
            };
            for k in Kind::ALL {
                said(kind_key(k));
                said(empty_key(k));
            }
            for tab in Tab::ALL {
                said(tab_key(tab));
            }
            for s in [
                Status::Running,
                Status::Stopped,
                Status::Frozen,
                Status::Error,
                Status::Other,
            ] {
                said(status_key(s));
            }
            for w in Why::ALL {
                said(why_key(w));
            }
            for l in Limit::ALL {
                said(limit_key(l));
            }
            for u in Use::ALL {
                said(use_key(u));
                said(use_where_key(u));
            }
            for f in Field::ALL {
                said(field_key(f));
            }
            for m in [Machine::Container, Machine::Vm] {
                said(machine_key(m));
            }
            for p in [Protocol::SimpleStreams, Protocol::Oci] {
                said(protocol_key(p));
            }
            for word in Job::WORDS {
                said(&format!("transfers-work-{word}"));
            }
        }
    }

    #[test]
    fn a_moment_is_read_as_the_daemon_writes_it() {
        assert_eq!(
            moment("1970-01-02T00:00:00Z"),
            None,
            "before 1971 is no date"
        );
        assert_eq!(moment("2026-10-04T05:24:11Z"), Some(1_791_091_451));
        assert_eq!(
            moment("2026-10-04T05:24:11.123456789Z"),
            Some(1_791_091_451),
            "the fraction is dropped"
        );
        assert_eq!(
            moment("2026-10-04T07:24:11+02:00"),
            Some(1_791_091_451),
            "an offset is taken back"
        );
        assert_eq!(moment("0001-01-01T00:00:00Z"), None, "the daemon's never");
        for bad in [
            "",
            "2026-10-04",
            "2026-13-04T05:24:11Z",
            "yesterday",
            "2026-10-04T05:24:11+0200",
        ] {
            assert_eq!(moment(bad), None, "{bad}");
        }
    }

    /// FAILS IF: a size with no unit is taken (for the daemon it is bytes), a
    /// binary unit is read as a decimal one, a fraction is rounded, or the
    /// person's decimal comma is refused.
    #[test]
    fn a_size_is_read_as_incus_reads_it_and_a_bare_number_is_refused() {
        assert_eq!(bytes_of("2GiB"), Ok(2 << 30));
        assert_eq!(bytes_of(" 512 MiB "), Ok(512 << 20));
        assert_eq!(bytes_of("2GB"), Ok(2_000_000_000));
        assert_eq!(bytes_of("2g"), Ok(2_000_000_000));
        assert_eq!(bytes_of("1.5GiB"), Ok(3 << 29));
        assert_eq!(bytes_of("1,5GiB"), Ok(3 << 29), "a decimal comma");
        assert_eq!(bytes_of("2"), Err(NotASize::Unit), "no unit");
        assert_eq!(bytes_of("2 parsecs"), Err(NotASize::Unit));
        assert_eq!(bytes_of("0MiB"), Err(NotASize::Number));
        assert_eq!(bytes_of("MiB"), Err(NotASize::Number));
        assert_eq!(bytes_of("1.2.3GiB"), Err(NotASize::Number));
        assert_eq!(
            bytes_of("0.1KiB"),
            Ok(102),
            "a fraction of a unit, rounded down"
        );
        assert_eq!(bytes_of("99999999999TiB"), Err(NotASize::Number));
        assert_eq!(bytes_typed(2 << 30), "2GiB");
        assert_eq!(bytes_typed(512 << 20), "512MiB");
        assert_eq!(bytes_typed(1_000), "1000B");
        assert_eq!(bytes_of(&bytes_typed(3 << 29)), Ok(3 << 29), "typed again");
        assert_eq!(whole_of(" 4 "), Some(4));
        assert_eq!(whole_of("0"), None);
        assert_eq!(whole_of("-1"), None);
    }

    #[test]
    fn a_processor_s_time_reads_as_a_duration() {
        let t = texts("en-GB");
        assert!(cpu_time(12_400_000_000, &t).contains("12"));
        assert!(cpu_time(125_000_000_000, &t).contains('2'));
        let long = cpu_time(3_725_000_000_000, &t);
        assert!(long.contains('1') && long.contains('2'), "{long}");
    }
}
