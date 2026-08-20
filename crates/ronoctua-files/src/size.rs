//! Bytes into something a person reads.
//!
//! # Why the number comes out in two pieces
//!
//! The decimal separator is a decision of the LANGUAGE — `4,2 KiB` in
//! Portuguese, `4.2 KiB` in English — and the same is true of whether the unit
//! is glued to the number. Formatting `4.2` here and handing over a string
//! would freeze both in Rust, which is the whole reason this project uses
//! Fluent (section 22). So the whole part and the tenth cross as separate
//! arguments and the `.ftl` puts them together:
//!
//! ```text
//! files-size-kib = { $whole },{ $tenth } KiB      # pt-BR
//! files-size-kib = { $whole }.{ $tenth } KiB      # en-GB
//! ```
//!
//! Fluent's own `NUMBER()` would be the textbook answer and does not help:
//! `fluent-bundle` formats a number with `format!` unless the embedder installs
//! an ICU formatter, so `4.2` comes out `4.2` in both languages.

use ronoctua_i18n::key;

/// The unit a size is shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Bytes, shown whole — a tenth of a byte is not a thing.
    Bytes,
    /// Kibibytes: 1024 bytes, and the `i` is not pedantry. A disk sold as
    /// "1 TB" holds 931 GiB, and a file manager that mixes the two makes the
    /// difference look like a bug.
    Kibi,
    /// Mebibytes.
    Mebi,
    /// Gibibytes.
    Gibi,
    /// Tebibytes.
    Tebi,
}

impl Unit {
    /// The text key that spells this unit out with the number in place.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Bytes => key::FILES_SIZE_BYTES,
            Self::Kibi => key::FILES_SIZE_KIB,
            Self::Mebi => key::FILES_SIZE_MIB,
            Self::Gibi => key::FILES_SIZE_GIB,
            Self::Tebi => key::FILES_SIZE_TIB,
        }
    }
}

/// A size, ready for a language to assemble.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Human {
    /// The part before the separator.
    pub whole: u64,
    /// The single digit after it. Always meaningful — `4.0 KiB` is drawn as
    /// such on purpose, because a column where some rows have a decimal and
    /// others do not stops being a column.
    pub tenth: u8,
    /// Which unit `whole` counts.
    pub unit: Unit,
}

impl Human {
    /// The arguments `Texts::t_with` takes, against this size's own key.
    #[must_use]
    pub fn args(&self) -> [(&'static str, String); 2] {
        [
            ("whole", self.whole.to_string()),
            ("tenth", self.tenth.to_string()),
        ]
    }
}

/// The size a listing shows.
///
/// Truncates rather than rounds, and the reason is the boundary: rounding
/// 1 048 000 bytes up prints `1.0 MiB` for something that is not yet a
/// mebibyte, and the one number a person checks against a quota is the one that
/// must not read bigger than it is.
#[must_use]
pub fn human(bytes: u64) -> Human {
    const STEP: u64 = 1024;
    let units = [Unit::Bytes, Unit::Kibi, Unit::Mebi, Unit::Gibi, Unit::Tebi];
    let mut value = bytes;
    let mut rest = 0u64;
    let mut at = 0usize;
    while value >= STEP && at + 1 < units.len() {
        rest = value % STEP;
        value /= STEP;
        at += 1;
    }
    let tenth = if at == 0 {
        0
    } else {
        // Truncating tenth: `rest` out of 1024, in tenths.
        u8::try_from(rest * 10 / STEP).unwrap_or(0)
    };
    Human {
        whole: value,
        tenth,
        unit: units[at],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_stay_bytes_and_have_no_decimal() {
        let h = human(0);
        assert_eq!((h.whole, h.tenth, h.unit), (0, 0, Unit::Bytes));
        let h = human(1023);
        assert_eq!((h.whole, h.tenth, h.unit), (1023, 0, Unit::Bytes));
    }

    #[test]
    fn the_step_is_1024_and_the_unit_says_so() {
        let h = human(1024);
        assert_eq!((h.whole, h.tenth, h.unit), (1, 0, Unit::Kibi));
        let h = human(1024 * 1024);
        assert_eq!((h.whole, h.tenth, h.unit), (1, 0, Unit::Mebi));
        let h = human(1024 * 1024 * 1024);
        assert_eq!(h.unit, Unit::Gibi);
    }

    #[test]
    fn it_truncates_and_never_reads_bigger_than_it_is() {
        // THE DEFECT THIS CATCHES: rounding up at the boundary. 1 048 575 bytes
        // is one byte short of a mebibyte; printed as `1.0 MiB` it would be a
        // file manager telling somebody they are over a quota they are under.
        let h = human(1024 * 1024 - 1);
        assert_eq!(h.unit, Unit::Kibi);
        assert_eq!((h.whole, h.tenth), (1023, 9));
        // And 4.25 KiB truncates to 4.2, not 4.3.
        let h = human(4352);
        assert_eq!((h.whole, h.tenth, h.unit), (4, 2, Unit::Kibi));
    }

    #[test]
    fn the_largest_unit_does_not_overflow_into_nothing() {
        // Past tebibytes the loop stops rather than running out of units. A
        // disk this size is not in the wild, and the alternative is an index
        // out of bounds in a listing.
        let h = human(u64::MAX);
        assert_eq!(h.unit, Unit::Tebi);
        assert!(h.whole > 0);
    }

    #[test]
    fn every_unit_has_a_key() {
        for u in [Unit::Bytes, Unit::Kibi, Unit::Mebi, Unit::Gibi, Unit::Tebi] {
            assert!(!u.label().is_empty());
        }
    }
}
