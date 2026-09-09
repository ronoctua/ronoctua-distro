//! Turning the bytes of a selection into text.
//!
//! # Why this is not `String::from_utf8`
//!
//! Because it was, and a copy that was not UTF-8 vanished. Not as mojibake and
//! not as an error on screen: the entry never reached the history, and the only
//! record was a `debug!` line nobody reads. Copy something in a Java or a Wine
//! application — which offer UTF-16 — and the clipboard simply did not notice
//! it had happened.
//!
//! # The encoding is DECLARED, and where it is not, it is known
//!
//! A Wayland offer is a list of MIME types, and a text one may carry a
//! `charset`. Where it does, there is nothing to guess. Where it does not,
//! there is still no guessing to do:
//!
//!   - `text/plain` without a charset is UTF-8 by every modern convention, and
//!     that is what is tried first;
//!   - `STRING` is Latin-1 **by definition** — it is an X11 atom with a
//!     specification, not a convention — so its bytes map one to one and the
//!     answer is exact;
//!   - `UTF8_STRING` says so in its name.
//!
//! What is refused is the fourth case: bytes that claim to be text, in an
//! encoding nobody declared and that is not UTF-8. Charset detection by
//! frequency analysis is the alternative, and it is a guess that is wrong
//! quietly — the failure mode being text that reads as text and says something
//! else. [`Undecodable`] is returned instead, and the caller says so.

/// Why a selection could not be read as text.
///
/// A value and not an absence: "nothing was copied" and "what was copied could
/// not be read" are different things, and only one of them is worth telling
/// somebody about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undecodable {
    /// The MIME type the other side offered, for the sentence on screen.
    pub kind: String,
    /// How many bytes came, for the same sentence.
    pub bytes: usize,
}

/// The byte order of a UTF-16 payload, once it is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Order {
    Little,
    Big,
}

/// The two byte-order marks, and what each one settles.
const BOM_LE: [u8; 2] = [0xFF, 0xFE];
const BOM_BE: [u8; 2] = [0xFE, 0xFF];
/// The UTF-8 one. It is a *signature*, not an encoding: it has to come off, or
/// it draws as a zero-width space at the head of every line pasted from a
/// Windows editor.
const BOM_UTF8: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Does this MIME type name UTF-16, and in which order if it says?
///
/// `charset=utf-16` with no order means "look for a byte-order mark", which is
/// what the Unicode standard says and what the `None` here carries.
fn utf16_order(kind: &str) -> Option<Option<Order>> {
    let lower = kind.to_ascii_lowercase();
    let charset = lower.split(';').nth(1)?.trim().strip_prefix("charset=")?;
    match charset.trim_matches('"') {
        "utf-16" | "utf16" | "unicode" => Some(None),
        "utf-16le" | "utf16le" => Some(Some(Order::Little)),
        "utf-16be" | "utf16be" => Some(Some(Order::Big)),
        _ => None,
    }
}

/// Is this type one of the Latin-1 ones — declared, or by specification?
fn is_latin1(kind: &str) -> bool {
    let lower = kind.to_ascii_lowercase();
    // `STRING` is Latin-1 in the ICCCM, which is a specification and not a
    // habit. `COMPOUND_TEXT` is NOT here: it is a stateful multi-charset
    // encoding with escape sequences, and decoding it as Latin-1 would produce
    // the escape sequences as letters.
    if lower == "string" {
        return true;
    }
    lower
        .split(';')
        .nth(1)
        .map(str::trim)
        .and_then(|p| p.strip_prefix("charset="))
        .is_some_and(|c| {
            matches!(
                c.trim_matches('"'),
                "iso-8859-1" | "iso8859-1" | "latin1" | "us-ascii" | "ascii"
            )
        })
}

/// Decodes UTF-16 in a known order, refusing a lone surrogate.
///
/// `char::decode_utf16` pairs the surrogates; an unpaired one is a real
/// encoding error and not something to paper over with a replacement
/// character, because a payload with one is a payload we have misread.
fn utf16(raw: &[u8], order: Order) -> Option<String> {
    if !raw.len().is_multiple_of(2) {
        return None;
    }
    let units = raw.chunks_exact(2).map(|p| match order {
        Order::Little => u16::from_le_bytes([p[0], p[1]]),
        Order::Big => u16::from_be_bytes([p[0], p[1]]),
    });
    char::decode_utf16(units)
        .collect::<Result<String, _>>()
        .ok()
}

/// Latin-1 is a subset of Unicode's first block, so every byte is a character.
///
/// It cannot fail, which is exactly why it is only reached when the type SAYS
/// Latin-1: used as a fallback it would turn any bytes at all into confident
/// nonsense.
fn latin1(raw: &[u8]) -> String {
    raw.iter().map(|b| char::from(*b)).collect()
}

/// The bytes of a selection, as text.
///
/// # Errors
///
/// [`Undecodable`] when the type claims text in an encoding that is neither
/// declared nor UTF-8. See the module's header for why that is refused rather
/// than guessed at.
pub fn decode(kind: &str, raw: &[u8]) -> Result<String, Undecodable> {
    let refuse = || Undecodable {
        kind: kind.to_owned(),
        bytes: raw.len(),
    };

    // THE BYTE-ORDER MARK WINS OVER THE TYPE, and it is meant to: an
    // application that writes a mark has said the order in the payload, and a
    // header that disagrees with the payload is a header that is wrong.
    if let Some(rest) = raw.strip_prefix(&BOM_LE) {
        return utf16(rest, Order::Little).ok_or_else(refuse);
    }
    if let Some(rest) = raw.strip_prefix(&BOM_BE) {
        return utf16(rest, Order::Big).ok_or_else(refuse);
    }
    if let Some(rest) = raw.strip_prefix(&BOM_UTF8) {
        return String::from_utf8(rest.to_vec()).map_err(|_| refuse());
    }

    if let Some(declared) = utf16_order(kind) {
        // No mark and no order in the type: the standard says big-endian, and
        // little-endian is what actually gets sent. Trying both is not a guess
        // — a payload that decodes cleanly in one order and not the other has
        // told us which it is.
        return match declared {
            Some(order) => utf16(raw, order).ok_or_else(refuse),
            None => utf16(raw, Order::Little)
                .or_else(|| utf16(raw, Order::Big))
                .ok_or_else(refuse),
        };
    }

    if is_latin1(kind) {
        return Ok(latin1(raw));
    }

    String::from_utf8(raw.to_vec()).map_err(|_| refuse())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_utf16(s: &str, order: Order, mark: bool) -> Vec<u8> {
        let mut out = Vec::new();
        if mark {
            out.extend_from_slice(match order {
                Order::Little => &BOM_LE,
                Order::Big => &BOM_BE,
            });
        }
        for unit in s.encode_utf16() {
            out.extend_from_slice(&match order {
                Order::Little => unit.to_le_bytes(),
                Order::Big => unit.to_be_bytes(),
            });
        }
        out
    }

    /// THE DEFECT THIS CATCHES, and it is the one the owner named: copying
    /// from an application that offers UTF-16 put NOTHING in the history.
    #[test]
    fn utf16_arrives_as_text_in_either_order() {
        let japanese = "\u{65E5}\u{672C}\u{8A9E}";
        for order in [Order::Little, Order::Big] {
            for mark in [true, false] {
                let raw = as_utf16(japanese, order, mark);
                let kind = match (order, mark) {
                    // With a mark the type need not say the order, which is
                    // the case a Windows application produces.
                    (_, true) => "text/plain;charset=utf-16",
                    (Order::Little, false) => "text/plain;charset=utf-16le",
                    (Order::Big, false) => "text/plain;charset=utf-16be",
                };
                assert_eq!(
                    decode(kind, &raw).as_deref(),
                    Ok(japanese),
                    "{kind}, mark={mark}"
                );
            }
        }
    }

    /// A payload that carries a mark is believed over a type that disagrees.
    #[test]
    fn the_mark_wins_over_the_header() {
        let raw = as_utf16("ok", Order::Big, true);
        assert_eq!(
            decode("text/plain;charset=utf-16le", &raw).as_deref(),
            Ok("ok")
        );
    }

    /// `STRING` is Latin-1 by specification, so every byte maps and nothing
    /// is lost. It is the encoding XWayland still emits.
    #[test]
    fn the_x_legacy_type_is_latin1_and_never_fails() {
        let raw = vec![0x63, 0x61, 0x66, 0xE9];
        assert_eq!(decode("STRING", &raw).as_deref(), Ok("caf\u{E9}"));
        assert_eq!(decode("string", &raw).as_deref(), Ok("caf\u{E9}"));
    }

    /// The UTF-8 signature comes OFF. Left on, it draws as a zero-width
    /// character at the head of everything pasted from a Windows editor.
    #[test]
    fn the_utf8_signature_is_not_part_of_the_text() {
        let mut raw = BOM_UTF8.to_vec();
        raw.extend_from_slice("hello".as_bytes());
        assert_eq!(decode("text/plain", &raw).as_deref(), Ok("hello"));
    }

    /// AND WHAT CANNOT BE READ IS A VALUE, not a silent return.
    ///
    /// Bytes that are not UTF-8, under a type that declares no charset. The
    /// old code dropped these; guessing an encoding for them would be worse,
    /// because a wrong guess reads as text and says something else.
    #[test]
    fn undeclared_bytes_that_are_not_utf8_are_refused_by_name() {
        let raw = vec![0xFF, 0x00, 0xFE, 0x41];
        let Err(no) = decode("text/plain", &raw) else {
            panic!("bytes that are not UTF-8 decoded as UTF-8");
        };
        assert_eq!(no.kind, "text/plain");
        assert_eq!(no.bytes, 4);
    }

    /// An odd number of bytes is not UTF-16, whatever the header says.
    #[test]
    fn a_truncated_utf16_payload_is_refused() {
        assert!(decode("text/plain;charset=utf-16le", &[0x41]).is_err());
    }

    /// A lone surrogate is a payload we have misread, and saying so is better
    /// than a string full of replacement characters that looks like the
    /// application's fault.
    #[test]
    fn a_lone_surrogate_is_refused_rather_than_replaced() {
        // `D800` is a high surrogate with nothing after it. Big-endian, so
        // the high byte comes FIRST — written the other way round it is
        // `U+00D8`, a perfectly good Ø, and the test would have proved nothing.
        let raw = vec![0xD8, 0x00, 0x00, 0x41];
        assert!(decode("text/plain;charset=utf-16be", &raw).is_err());
        // And the same units in the other order are a valid pair, so the
        // refusal above is about the surrogate and not about the length.
        let pair = vec![0xD8, 0x3D, 0xDE, 0x00];
        assert_eq!(
            decode("text/plain;charset=utf-16be", &pair).as_deref(),
            Ok("\u{1F600}")
        );
    }

    /// Plain UTF-8 still goes straight through, which is every copy that
    /// worked before this module existed.
    #[test]
    fn utf8_is_untouched() {
        for s in ["hello", "\u{e7}\u{e3}o", "\u{65E5}\u{672C}", "\u{1F600}"] {
            assert_eq!(
                decode("text/plain;charset=utf-8", s.as_bytes()).as_deref(),
                Ok(s)
            );
            assert_eq!(decode("UTF8_STRING", s.as_bytes()).as_deref(), Ok(s));
        }
    }

    /// COMPOUND_TEXT is NOT treated as Latin-1: it is stateful and full of
    /// escape sequences, which would come out as letters.
    #[test]
    fn the_stateful_x_type_is_not_mistaken_for_latin1() {
        assert!(!is_latin1("COMPOUND_TEXT"));
    }
}
