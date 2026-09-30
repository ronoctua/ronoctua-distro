//! Which characters a subtitle file's bytes are.
//!
//! Subtitle files are the last place old code pages live: a Brazilian SRT
//! from 2008 is Windows-1252, a Polish one Windows-1250, a Russian one
//! Windows-1251, and none of them says so. The order is the order of
//! certainty:
//!
//! 1. a byte-order mark says it;
//! 2. two-byte text with no mark shows itself by its zero bytes;
//! 3. bytes that are valid UTF-8 are UTF-8 (no old code page makes a long run
//!    of valid UTF-8 by accident);
//! 4. otherwise chardetng guesses, the detector Firefox uses for the same
//!    question, and encoding_rs decodes.
//!
//! The answer says which of the four it was, so a door can name the guess.

use encoding_rs::Encoding;
use serde::{Deserialize, Serialize};

/// How the charset was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Found {
    /// The file begins with a byte-order mark.
    Mark,
    /// The bytes are valid UTF-8.
    Valid,
    /// Guessed from the bytes (two-byte text with no mark, or an old code
    /// page).
    Guessed,
}

/// The charset a file was read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Charset {
    /// Its WHATWG name (`UTF-8`, `windows-1252`, `UTF-16LE`…).
    pub name: &'static str,
    /// How it was found.
    pub found: Found,
}

/// A file's text, and what it was read as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// The text, with every line ending made `\n` and no byte-order mark.
    pub text: String,
    /// What the bytes were read as.
    pub charset: Charset,
}

/// Reads bytes as text; never fails, because every byte sequence is text in
/// SOME code page — what cannot be decoded becomes U+FFFD.
#[must_use]
pub fn decode(bytes: &[u8]) -> Decoded {
    let (text, charset) = if let Some((encoding, mark)) = Encoding::for_bom(bytes) {
        (
            encoding
                .decode_without_bom_handling(&bytes[mark..])
                .0
                .into_owned(),
            Charset {
                name: encoding.name(),
                found: Found::Mark,
            },
        )
    } else if let Some(encoding) = two_bytes_unmarked(bytes) {
        (
            encoding.decode_without_bom_handling(bytes).0.into_owned(),
            Charset {
                name: encoding.name(),
                found: Found::Guessed,
            },
        )
    } else if let Ok(text) = std::str::from_utf8(bytes) {
        (
            text.to_owned(),
            Charset {
                name: encoding_rs::UTF_8.name(),
                found: Found::Valid,
            },
        )
    } else {
        let mut detector = chardetng::EncodingDetector::new();
        detector.feed(bytes, true);
        // UTF-8 was already ruled out: the bytes are not valid UTF-8.
        let encoding = detector.guess(None, false);
        (
            encoding.decode_without_bom_handling(bytes).0.into_owned(),
            Charset {
                name: encoding.name(),
                found: Found::Guessed,
            },
        )
    };
    Decoded {
        text: normal_lines(&text),
        charset,
    }
}

/// UTF-16 written without a mark: in text that is mostly Latin, every
/// other byte is zero. How much of the head is looked at.
const TWO_BYTES_HEAD: usize = 4096;

fn two_bytes_unmarked(bytes: &[u8]) -> Option<&'static Encoding> {
    let head = &bytes[..bytes.len().min(TWO_BYTES_HEAD) & !1];
    let pairs = head.len() / 2;
    if pairs < 2 {
        return None;
    }
    let zero_at = |parity: usize| {
        head.iter()
            .skip(parity)
            .step_by(2)
            .filter(|&&b| b == 0)
            .count()
    };
    let (even, odd) = (zero_at(0), zero_at(1));
    // Two in five pairs zero on one side, almost none on the other.
    let many = |n: usize| n * 5 >= pairs * 2;
    let few = |n: usize| n * 20 <= pairs;
    if many(odd) && few(even) {
        Some(encoding_rs::UTF_16LE)
    } else if many(even) && few(odd) {
        Some(encoding_rs::UTF_16BE)
    } else {
        None
    }
}

/// Every line ending made `\n` (`\r\n` from Windows, a lone `\r` from the
/// old Mac), a byte-order mark left inside dropped, and NULs removed.
fn normal_lines(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push('\n');
            }
            '\0' => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_wins_and_is_dropped() {
        let d = decode(b"\xEF\xBB\xBFol\xC3\xA1\r\n");
        assert_eq!(d.text, "olá\n");
        assert_eq!(d.charset.found, Found::Mark);
        let d = decode(b"\xFF\xFEo\0l\0\xE1\0");
        assert_eq!(d.text, "olá");
        assert_eq!(d.charset.name, "UTF-16LE");
    }

    #[test]
    fn two_bytes_without_a_mark_are_found() {
        let bytes: Vec<u8> = "1\n00:00:01,000 --> 00:00:02,000\nhello\n"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        let d = decode(&bytes);
        assert_eq!(d.charset.name, "UTF-16BE");
        assert!(d.text.contains("hello"));
    }

    #[test]
    fn valid_utf8_is_utf8() {
        let d = decode("ação\r".as_bytes());
        assert_eq!(d.text, "ação\n");
        assert_eq!(d.charset.found, Found::Valid);
    }

    #[test]
    fn an_old_code_page_is_guessed() {
        // Windows-1252 Portuguese, as an old SRT would have it.
        let d = decode(
            b"1\n00:00:01,000 --> 00:00:03,000\nVoc\xEA n\xE3o est\xE1 sozinho.\n\n\
2\n00:00:04,000 --> 00:00:06,000\nA cora\xE7\xE3o \xE9 a \xFAnica coisa que n\xE3o se v\xEA.\n\n\
3\n00:00:07,000 --> 00:00:09,000\nEle j\xE1 est\xE1 aqui, e as informa\xE7\xF5es tamb\xE9m.\n",
        );
        assert_eq!(d.charset.found, Found::Guessed);
        assert_eq!(d.charset.name, "windows-1252");
        // pt-BR: the file's own words, under test.
        assert!(d.text.contains("Você não está sozinho"));
    }
}
