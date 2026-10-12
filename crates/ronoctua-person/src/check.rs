//! The value checks every file shares — pure functions of text.
//!
//! Each says what a value MAY be, and nothing is "plausible": a value that is
//! not what it must be is refused, never mended into a default.

/// Seconds since the epoch of an exact `YYYY-MM-DDTHH:MM:SSZ`.
pub(crate) fn stamp_seconds(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return None;
    }
    let num = |from: usize, to: usize| -> Option<i64> {
        let part = &text[from..to];
        part.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| part.parse().ok())
            .flatten()
    };
    let (year, month, day) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hour, minute, second) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&month)
        || day < 1
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// The exact text of `unix_s`.
pub(crate) fn stamp_of(unix_s: i64) -> String {
    let days = unix_s.div_euclid(86_400);
    let rest = unix_s.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

const fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

const fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Howard Hinnant's `days_from_civil`.
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Its inverse.
const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

/// A lower-case RFC 1123 label: what a machine is called.
pub(crate) fn hostname(host: &str) -> bool {
    let b = host.as_bytes();
    !b.is_empty()
        && b.len() <= 63
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && b[0] != b'-'
        && b[b.len() - 1] != b'-'
}

/// A nixpkgs attribute path: `ripgrep`, `python3Packages.requests`.
pub(crate) fn nix_attr(attr: &str) -> bool {
    attr.len() <= 200
        && !attr.is_empty()
        && attr.split('.').all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '\'' | '-'))
        })
}

/// A reverse-DNS application id with at least three parts, as Flatpak
/// requires: `org.gnome.Calculator`.
pub(crate) fn flatpak_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    id.len() <= 255
        && parts.len() >= 3
        && parts.iter().all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        })
}

/// A remote's name: `flathub`.
pub(crate) fn remote_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
        && !name.starts_with('.')
}

/// A folded, file-safe key (an AppImage's, a web app's slug, a registry id):
/// lower-case letters, digits and `-`, not starting with `-`.
pub(crate) fn slug(key: &str) -> bool {
    let b = key.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// An Android package: `org.fdroid.fdroid`.
pub(crate) fn android_package(package: &str) -> bool {
    let parts: Vec<&str> = package.split('.').collect();
    package.len() <= 255
        && parts.len() >= 2
        && parts.iter().all(|part| {
            let mut chars = part.chars();
            chars.next().is_some_and(|c| c.is_ascii_alphabetic())
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

/// An `https://` address with a host and no credentials, no white space.
pub(crate) fn https_url(url: &str) -> bool {
    url.strip_prefix("https://").is_some_and(|rest| {
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        !host.is_empty() && !host.contains('@')
    }) && url.len() <= 2048
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// An `http://` or `https://` address (a web app may be a local service).
pub(crate) fn web_url(url: &str) -> bool {
    https_url(url)
        || url
            .strip_prefix("http://")
            .is_some_and(|rest| https_url(&format!("https://{rest}")))
}

/// An SRI hash (`sha256-<base64>` or `sha512-<base64>`), as Nix's `fetchurl`
/// takes it.
pub(crate) fn sri(hash: &str) -> bool {
    let (algo, digest) = match hash.split_once('-') {
        Some(pair) => pair,
        None => return false,
    };
    let length = match algo {
        "sha256" => 44,
        "sha512" => 88,
        _ => return false,
    };
    digest.len() == length
        && digest
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'='))
}

/// A lower-case hex SHA-256: the name of a content-addressed icon.
pub(crate) fn sha256_hex(hex: &str) -> bool {
    hex.len() == 64
        && hex
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// A name for people: not empty, not too long, no control character.
pub(crate) fn name(text: &str) -> bool {
    !text.trim().is_empty() && text.chars().count() <= 128 && !text.chars().any(char::is_control)
}

/// A short token (a version, a fingerprint, a revision, a hash).
pub(crate) fn token(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 128
        && text
            .bytes()
            .all(|c| c.is_ascii_graphic() && c != b'"' && c != b'\\')
}

/// A person brain module's id: `my-` and an English kebab word whose every
/// segment begins with a letter, at most 64 bytes in all.
///
/// THE LOADER'S GRAMMAR (`isPersonId` in `nix/brain-modules.nix`), not a
/// slug's: the id becomes a Rust module, a WIT case and a text-key prefix,
/// and a segment that begins with a digit is refused by the WIT. An id this
/// approved and the loader refused would be an approval of nothing. Both are
/// proved on one table, `fixtures/module-ids.json`.
pub(crate) fn person_module(id: &str) -> bool {
    id.len() <= 64
        && id.strip_prefix("my-").is_some_and(|word| {
            word.split('-').all(|segment| {
                segment
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_lowercase)
                    && segment
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            })
        })
}

/// A store path.
pub(crate) fn store_path(path: &str) -> bool {
    path.strip_prefix("/nix/store/").is_some_and(|rest| {
        rest.len() > 33
            && !rest.contains("..")
            && !path.chars().any(|c| c.is_whitespace() || c.is_control())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checks_take_what_they_must_and_nothing_else() {
        assert!(hostname("noctua") && hostname("vm-1") && !hostname("Noctua"));
        assert!(!hostname("-a") && !hostname("a b") && !hostname(""));
        assert!(nix_attr("ripgrep") && nix_attr("python3Packages.requests"));
        assert!(!nix_attr("a..b") && !nix_attr("1abc") && !nix_attr("a b"));
        assert!(flatpak_id("org.gnome.Calculator") && !flatpak_id("org.gnome"));
        assert!(slug("obsidian") && slug("what-s-app2") && !slug("A") && !slug("-x"));
        assert!(android_package("org.fdroid.fdroid") && !android_package("fdroid"));
        assert!(https_url("https://example.org/a.AppImage"));
        assert!(!https_url("http://example.org") && !https_url("https://user@host/"));
        assert!(web_url("http://localhost:8080/") && !web_url("ftp://x"));
        assert!(sri("sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU="));
        assert!(!sri("sha256-short") && !sri("md5-x"));
        assert!(sha256_hex(&"a".repeat(64)) && !sha256_hex(&"A".repeat(64)));
        assert!(person_module("my-reminders") && !person_module("made-up"));
        assert!(store_path(
            "/nix/store/abcdefghijklmnopqrstuvwxyz012345-app.apk"
        ));
        assert!(!store_path("/tmp/app.apk"));
    }

    /// FAILS IF: an approval takes a person's module id the brain's loader
    /// refuses (`my-2fa`, whose glyph token the WIT would refuse), or refuses
    /// one it loads — the two grammars read on their one table.
    #[test]
    fn a_person_module_id_is_the_loaders_grammar() {
        let table: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/module-ids.json")).unwrap();
        let ids = |side: &str| -> Vec<String> {
            table[side]
                .as_array()
                .unwrap_or_else(|| panic!("no `{side}` list"))
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect()
        };
        let (accepted, refused) = (ids("accepted"), ids("refused"));
        assert!(
            accepted.len() >= 5 && refused.len() >= 10,
            "the table shrank"
        );
        for id in &accepted {
            assert!(person_module(id), "`{id}` refused");
        }
        for id in &refused {
            assert!(!person_module(id), "`{id}` taken");
        }
    }
}
