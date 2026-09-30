//! Query strings, and the checks that keep a value out of the wrong place.
//!
//! Every address here is a constant origin, a constant path and escaped
//! values. An id that goes INTO a path (a MusicBrainz id, a Wikidata item,
//! a TMDB number) is checked against its shape first, so a value from a
//! file's tags can never become a path of its own.

/// Percent-escaping for a query value: everything but the unreserved set.
#[must_use]
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `name=value&…`, every value escaped, in the order given. The names are
/// this crate's constants and go as they are.
#[must_use]
pub fn pairs(pairs: &[(&str, String)]) -> String {
    pairs
        .iter()
        .map(|(name, value)| format!("{name}={}", escape(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Whether `s` is a MusicBrainz id: a UUID, `8-4-4-4-12` lowercase hex.
#[must_use]
pub fn is_mbid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(p, n)| p.len() == n && p.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

/// Whether `s` is a Wikidata item: `Q` and digits.
#[must_use]
pub fn is_qid(s: &str) -> bool {
    s.strip_prefix('Q')
        .is_some_and(|d| !d.is_empty() && d.len() <= 12 && d.bytes().all(|b| b.is_ascii_digit()))
}

/// A language tag as the services take it: letters, digits and `-`,
/// lowercased, two to sixteen characters (`en`, `pt-br`, `zh-hant`).
/// `None` for anything else — it goes into a query and into SPARQL.
#[must_use]
pub fn language_tag(s: &str) -> Option<String> {
    let s = s.trim().replace('_', "-").to_ascii_lowercase();
    let fine = (2..=16).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && s.bytes().next().is_some_and(|b| b.is_ascii_alphabetic())
        && !s.ends_with('-');
    fine.then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_escaped_and_the_unreserved_set_is_not() {
        assert_eq!(escape("Sigur Rós"), "Sigur%20R%C3%B3s");
        assert_eq!(escape("a-b_c.d~e"), "a-b_c.d~e");
        assert_eq!(escape("a&b=c?d#e/f"), "a%26b%3Dc%3Fd%23e%2Ff");
        assert_eq!(
            pairs(&[("q", "x y".into()), ("limit", "10".into())]),
            "q=x%20y&limit=10"
        );
    }

    #[test]
    fn an_id_has_its_shape_or_it_is_not_one() {
        assert!(is_mbid("b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d"));
        assert!(
            !is_mbid("B10BBBFC-CF9E-42E0-BE17-E2C3E1D2600D"),
            "lowercase only"
        );
        assert!(!is_mbid("b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d/../x"));
        assert!(!is_mbid(""));
        assert!(is_qid("Q83495"));
        assert!(!is_qid("Q"), "no digits");
        assert!(!is_qid("Q1 } ; DROP"), "SPARQL is not a place for text");
        assert!(!is_qid("P577"));
    }

    #[test]
    fn a_language_tag_is_letters_and_hyphens() {
        assert_eq!(language_tag("pt_BR").as_deref(), Some("pt-br"));
        assert_eq!(language_tag(" en ").as_deref(), Some("en"));
        assert_eq!(language_tag("zh-Hant").as_deref(), Some("zh-hant"));
        assert_eq!(language_tag("e"), None);
        assert_eq!(language_tag("en\", \"x"), None, "never into SPARQL");
        assert_eq!(language_tag("-en"), None);
        assert_eq!(language_tag("en-"), None);
    }
}
