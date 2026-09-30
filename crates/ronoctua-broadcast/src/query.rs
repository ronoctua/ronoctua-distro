//! Escaping what goes into an address.

/// Percent-escapes `text` for a path segment or a query value: everything but
/// the unreserved characters of RFC 3986, byte by byte of its UTF-8. Pure.
pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Words a person typed, as they are asked: trimmed, at most `most`
/// characters, and `None` when nothing is left.
pub(crate) fn words(text: &str, most: usize) -> Option<String> {
    let cut: String = text
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(most)
        .collect();
    let cut = cut.trim();
    (!cut.is_empty()).then(|| cut.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_escaped_byte_by_byte() {
        assert_eq!(escape("bossa nova"), "bossa%20nova");
        assert_eq!(escape("r&b/soul"), "r%26b%2Fsoul");
        assert_eq!(escape("música"), "m%C3%BAsica");
    }

    #[test]
    fn words_are_trimmed_cut_and_never_empty() {
        assert_eq!(words("  jazz  ", 10).as_deref(), Some("jazz"));
        assert_eq!(words("   ", 10), None);
        assert_eq!(words("abcdef", 3).as_deref(), Some("abc"));
        assert_eq!(words("a\nb", 10).as_deref(), Some("ab"));
    }
}
