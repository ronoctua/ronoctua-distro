//! Lyrics from `lrclib.net` — the third source, after a `.lrc` beside the
//! file and the lyrics the file carries itself (`lyrics::parse` reads all
//! three).
//!
//! One `GET /api/get` with the artist, the title, the album and the length:
//! the service matches on all four, and the length is what tells a live
//! version from the studio one. It answers synced lyrics (LRC) when it has
//! them and plain text when it does not; synced wins.

use crate::wire::LyricLine;

/// The request's path, every value escaped.
#[must_use]
pub fn path(artist: &str, title: &str, album: Option<&str>, duration: Option<f64>) -> String {
    let mut out = format!(
        "/api/get?artist_name={}&track_name={}",
        escape(artist),
        escape(title)
    );
    if let Some(a) = album.filter(|a| !a.is_empty()) {
        out.push_str(&format!("&album_name={}", escape(a)));
    }
    if let Some(d) = duration {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let s = d.round().max(0.0) as u64;
        out.push_str(&format!("&duration={s}"));
    }
    out
}

/// The lyrics in an answer, synced first; `None` when it has none.
#[must_use]
pub fn parse(body: &str) -> Option<Vec<LyricLine>> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    if v.get("instrumental").and_then(serde_json::Value::as_bool) == Some(true) {
        return None;
    }
    for field in ["syncedLyrics", "plainLyrics"] {
        if let Some(text) = v.get(field).and_then(serde_json::Value::as_str)
            && !text.trim().is_empty()
        {
            let lines = crate::lyrics::parse(text);
            if !lines.is_empty() {
                return Some(lines);
            }
        }
    }
    None
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_names_all_four_and_escapes_them() {
        assert_eq!(
            path("Sigur Rós", "Hoppípolla", Some("Takk…"), Some(268.4)),
            "/api/get?artist_name=Sigur%20R%C3%B3s&track_name=Hopp%C3%ADpolla\
             &album_name=Takk%E2%80%A6&duration=268"
        );
        assert_eq!(
            path("a", "b", None, None),
            "/api/get?artist_name=a&track_name=b"
        );
    }

    #[test]
    fn synced_lyrics_win_and_an_instrumental_has_none() {
        let both = r#"{"syncedLyrics":"[00:01.00]one\n[00:02.00]two","plainLyrics":"one\ntwo"}"#;
        let lines = parse(both).expect("lyrics");
        assert_eq!(lines.len(), 2);
        assert!(lines[0].at.is_some(), "synced");
        let plain = r#"{"syncedLyrics":null,"plainLyrics":"one\ntwo"}"#;
        assert!(parse(plain).expect("lyrics")[0].at.is_none());
        assert_eq!(parse(r#"{"instrumental":true,"plainLyrics":"x"}"#), None);
        assert_eq!(parse("not json"), None);
    }
}
