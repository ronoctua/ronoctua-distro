//! A size, as a person reads it.
//!
//! # Why a file of its own
//!
//! Because two binaries need the ONE arithmetic: the footer, which draws a
//! transfer's total and a relief's room, and `ronoctua-render-png`, which
//! photographs the brain's page — whose notices and module rows carry sizes the
//! host formats (`crate::mind_wire`). The tool links the pure files it needs by
//! `#[path]`, and `network.rs`, where this used to live, is not one of them. A
//! second copy in the photograph would be a size that reads one way on the
//! screen and another in its picture.

/// `44220000000` -> `44,2 GB`.
///
/// One decimal place, like the link's rate: the total transferred grows all
/// the time, and the second place would be a digit flickering without
/// informing anything.
///
/// THE SEPARATOR IS THE LANGUAGE'S and comes in from
/// [`ronoctua_i18n::Texts::decimal_mark`]. It was a comma written into the
/// format string, so an English screen said `44,2 GB` — which in English is
/// forty-four thousand two hundred gigabytes.
#[must_use]
pub fn readable_bytes(bytes: u64, mark: char) -> String {
    const K: u64 = 1_000;
    const M: u64 = K * 1_000;
    const G: u64 = M * 1_000;
    match bytes {
        0..=999 => format!("{bytes} B"),
        1_000..=999_999 => format!("{} kB", bytes / K),
        1_000_000..=999_999_999 => format!("{} MB", bytes / M),
        _ => {
            let tenth = (bytes + G / 20) / (G / 10);
            format!("{}{mark}{} GB", tenth / 10, tenth % 10)
        }
    }
}
