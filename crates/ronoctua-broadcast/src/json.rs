//! Reading a directory's JSON one field at a time.
//!
//! The three directories write loosely: the archive gives a title as a string
//! or as a list of them and a year as a number or a string, radio-browser
//! writes `0`/`1` for a boolean, and any of them may add, drop or null a field
//! on any day. A struct read with serde would fail a WHOLE answer over one odd
//! field, so every answer is read as a `serde_json::Value` and each field is
//! taken out by one of these, which answer `None` for what they cannot read.

use serde_json::Value;

use crate::{Refusal, Source, Why};

/// The body, as JSON, or [`Why::Malformed`] from `source`.
pub(crate) fn value(source: Source, bytes: &[u8]) -> Result<Value, Refusal> {
    serde_json::from_slice(bytes).map_err(|_| Refusal::new(source, Why::Malformed))
}

/// The body, which must be a JSON array.
pub(crate) fn array(source: Source, bytes: &[u8]) -> Result<Vec<Value>, Refusal> {
    match value(source, bytes)? {
        Value::Array(items) => Ok(items),
        _ => Err(Refusal::new(source, Why::Malformed)),
    }
}

/// A text that is there: a string, or the first string of a list — trimmed,
/// and `None` when blank.
pub(crate) fn text(v: &Value, name: &str) -> Option<String> {
    let field = v.get(name)?;
    let s = match field {
        Value::String(s) => s.as_str(),
        Value::Array(items) => items.iter().find_map(Value::as_str)?,
        _ => return None,
    };
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

/// A whole number that is there: a number, or a string of digits.
pub(crate) fn whole(v: &Value, name: &str) -> Option<u64> {
    match v.get(name)? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        Value::Array(items) => items.first().and_then(|f| match f {
            Value::Number(n) => n.as_u64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }),
        _ => None,
    }
}

/// A number that is there, whole or not: a number, or a string of one.
pub(crate) fn number(v: &Value, name: &str) -> Option<f64> {
    let n = match v.get(name)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }?;
    n.is_finite().then_some(n)
}

/// A flag: `true`/`false`, or `1`/`0` as radio-browser writes them.
pub(crate) fn flag(v: &Value, name: &str) -> Option<bool> {
    match v.get(name)? {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_u64().map(|n| n != 0),
        _ => None,
    }
}

/// A list of texts, each trimmed, blanks dropped: a JSON list of strings, or
/// one string split at commas (radio-browser's `tags`).
pub(crate) fn texts(v: &Value, name: &str) -> Vec<String> {
    let parts: Vec<&str> = match v.get(name) {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).collect(),
        Some(Value::String(s)) => s.split(',').collect(),
        _ => Vec::new(),
    };
    let mut out: Vec<String> = Vec::new();
    for p in parts {
        let p = p.trim();
        if !p.is_empty() && !out.iter().any(|o| o == p) {
            out.push(p.to_owned());
        }
    }
    out
}

/// Whether `url` is an address a player may be handed: `http` or `https`,
/// with a host, no whitespace or control character.
pub(crate) fn is_web_address(url: &str) -> bool {
    let lower = url
        .get(..8)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let rest = if lower.starts_with("https://") {
        url.get(8..)
    } else if lower.starts_with("http://") {
        url.get(7..)
    } else {
        None
    };
    rest.is_some_and(|r| !r.is_empty() && !r.starts_with('/'))
        && url.len() <= 4096
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_field_is_read_whatever_shape_it_came_in() {
        let v = json!({
            "title": ["  First  ", "Second"], "blank": "  ", "year": "1915",
            "downloads": 12, "size": 3.5, "hls": 1, "ok": false,
            "tags": "jazz, bossa nova,,jazz ", "list": ["a", " b", 3]
        });
        assert_eq!(text(&v, "title").as_deref(), Some("First"));
        assert_eq!(text(&v, "blank"), None);
        assert_eq!(text(&v, "missing"), None);
        assert_eq!(whole(&v, "year"), Some(1915));
        assert_eq!(whole(&v, "downloads"), Some(12));
        assert_eq!(whole(&v, "size"), None, "not whole");
        assert_eq!(number(&v, "size"), Some(3.5));
        assert_eq!(flag(&v, "hls"), Some(true));
        assert_eq!(flag(&v, "ok"), Some(false));
        assert_eq!(texts(&v, "tags"), ["jazz", "bossa nova"]);
        assert_eq!(texts(&v, "list"), ["a", "b"]);
    }

    #[test]
    fn only_a_web_address_is_one() {
        assert!(is_web_address("https://stream.example/live"));
        assert!(is_web_address("HTTP://stream.example:8000/;"));
        assert!(!is_web_address("rtmp://stream.example/live"));
        assert!(!is_web_address("file:///etc/passwd"));
        assert!(!is_web_address("https:///nohost"));
        assert!(!is_web_address("https://a b"));
        assert!(!is_web_address(""));
    }
}
