//! A part's id: what names its profile, its record and its unit instance.
//!
//! The id becomes a FILE name, a PATH segment and a systemd INSTANCE that a
//! polkit rule compares as text, so it is held to the narrowest shape that
//! is still readable: lower-case kebab, starting with a letter, at most
//! [`MOST`] characters. A `/`, a `.`, a `@` or an escape that systemd would
//! rewrite can never reach any of the three. And never
//! [`crate::names::SYSTEM`], whose folder of hand-ins is the machine's.

/// The longest id.
pub const MOST: usize = 48;

/// Whether `id` may name a part.
#[must_use]
pub fn is_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MOST
        && bytes[0].is_ascii_lowercase()
        && !id.ends_with('-')
        && !id.contains("--")
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        && id != crate::names::SYSTEM
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: an id that a unit instance, a path or a polkit comparison
    /// would read differently from its text is taken.
    #[test]
    fn an_id_is_plain_kebab() {
        for good in [
            "yt-dlp",
            "pot-provider",
            "appimage-photocraft",
            "a",
            "rust2",
        ] {
            assert!(is_id(good), "{good}");
        }
        for bad in [
            "",
            "Yt",
            "1st",
            "a/b",
            "a.b",
            "a@b",
            "a b",
            "a-",
            "a--b",
            "-a",
            "a\\x2db",
            crate::names::SYSTEM,
            &"a".repeat(MOST + 1),
        ] {
            assert!(!is_id(bad), "{bad}");
        }
    }
}
