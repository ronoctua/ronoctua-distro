//! MyMemory: keyless, the default translator (`api.mymemory.translated.net`).
//!
//! `GET /get?q=<text>&langpair=<from>|<to>`, answered in JSON whose own
//! `responseStatus` says what the HTTP status does not (a pair it refuses is
//! an HTTP 200 with a `403` inside). `Autodetect` stands for "from whatever it
//! is".
//!
//! # Five hundred bytes a question
//!
//! MyMemory takes at most 500 bytes of text per request, so a song goes in
//! pieces cut at line breaks (a line longer than that, at a space), and the
//! answers are joined back with the break each piece ended on. The lines stay
//! lines, which is what the lyrics align by.

use serde::Deserialize;

use crate::ask::{Request, escape};
use crate::{MYMEMORY_HOST, Tongue, Why};

/// The most bytes of text one request carries.
pub const PIECE_BYTES: usize = 500;

/// The most bytes of text one translation takes: ten requests, which at the
/// host's pace is ten seconds — a song, and not a book.
pub const MOST_TEXT_BYTES: usize = 10 * PIECE_BYTES;

/// A piece of the text and the break that followed it in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// The text of the piece.
    pub text: String,
    /// What came after it: a line break, a space (a line cut in two), or
    /// nothing (the last).
    pub then: &'static str,
}

/// The text in pieces of at most `most` bytes, cut at line breaks and, inside
/// a line longer than that, at the last space (or, for a word longer than
/// `most`, at a character's edge). Joining every piece with its `then` gives
/// the text back. Pure.
#[must_use]
pub fn pieces(text: &str, most: usize) -> Vec<Piece> {
    let most = most.max(4);
    // THE UNITS: every line with the break after it, and a line longer than
    // a piece cut at its spaces first.
    let mut units: Vec<(&str, &'static str)> = Vec::new();
    let mut lines = text.split('\n').peekable();
    while let Some(line) = lines.next() {
        let after = if lines.peek().is_some() { "\n" } else { "" };
        let mut rest = line;
        while rest.len() > most {
            let edge = (1..=most)
                .rev()
                .find(|&k| rest.is_char_boundary(k))
                .unwrap_or(rest.len());
            match rest[..edge].rfind(' ').filter(|&c| c > 0) {
                Some(space) => {
                    units.push((&rest[..space], " "));
                    rest = &rest[space + 1..];
                }
                None => {
                    units.push((&rest[..edge], ""));
                    rest = &rest[edge..];
                }
            }
        }
        units.push((rest, after));
    }
    // AND THE PIECES: as many units as fit, the break between two pieces
    // kept as the first one's `then`.
    let mut out = Vec::new();
    let mut current = String::new();
    let mut last: Option<&'static str> = None;
    for (unit, after) in units {
        if let Some(between) = last {
            if current.len() + between.len() + unit.len() <= most {
                current.push_str(between);
                current.push_str(unit);
                last = Some(after);
                continue;
            }
            out.push(Piece {
                text: std::mem::take(&mut current),
                then: between,
            });
        }
        current.push_str(unit);
        last = Some(after);
    }
    out.push(Piece {
        text: current,
        then: last.unwrap_or(""),
    });
    out
}

/// The question for one piece. Pure.
#[must_use]
pub fn request(piece: &str, from: Option<&Tongue>, to: &Tongue) -> Request {
    let pair = format!("{}|{}", from.map_or("Autodetect", Tongue::code), to.code());
    Request::get(
        MYMEMORY_HOST,
        format!(
            "https://{MYMEMORY_HOST}/get?q={}&langpair={}",
            escape(piece),
            escape(&pair)
        ),
    )
}

#[derive(Deserialize)]
struct Answer {
    #[serde(rename = "responseData")]
    data: Option<Data>,
    #[serde(rename = "responseStatus", default)]
    status: serde_json::Value,
    #[serde(rename = "quotaFinished", default)]
    quota_finished: Option<bool>,
}

#[derive(Deserialize)]
struct Data {
    #[serde(rename = "translatedText")]
    text: Option<String>,
    #[serde(rename = "detectedLanguage", default)]
    detected: Option<String>,
}

/// What an answer says: the translated piece and the tongue MyMemory
/// detected, or the refusal inside it. Pure.
///
/// # Errors
///
/// [`Why::Quota`] (the day's characters are spent, said in three ways),
/// [`Why::UnknownTongue`] (a pair it refuses), [`Why::Failed`], and
/// [`Why::Garbled`] for what is not its JSON.
pub fn read(body: &[u8]) -> Result<(String, Option<Tongue>), Why> {
    let answer: Answer = serde_json::from_slice(body).map_err(|_| Why::Garbled)?;
    let status = answer
        .status
        .as_u64()
        .or_else(|| answer.status.as_str().and_then(|s| s.trim().parse().ok()))
        .unwrap_or(0);
    if answer.quota_finished == Some(true) || status == 429 {
        return Err(Why::Quota);
    }
    match status {
        200 => {}
        403 => return Err(Why::UnknownTongue),
        _ => return Err(Why::Failed),
    }
    let data = answer.data.ok_or(Why::Garbled)?;
    let text = crate::html::entities(&data.text.ok_or(Why::Garbled)?);
    // THE QUOTA IN A THIRD SHAPE: as the "translation" itself.
    if text.starts_with("MYMEMORY WARNING") {
        return Err(Why::Quota);
    }
    Ok((text, data.detected.as_deref().and_then(Tongue::new)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(pieces: &[Piece]) -> String {
        pieces
            .iter()
            .map(|p| format!("{}{}", p.text, p.then))
            .collect()
    }

    /// FAILS IF: cutting a song into pieces loses or moves a line break — the
    /// translated lines would no longer sit under their lyrics.
    #[test]
    fn a_text_in_pieces_joins_back_to_itself() {
        let song = "first line\nsecond line\n\nfourth after a blank\nlast";
        for most in [8, 12, 20, 30, 500] {
            let p = pieces(song, most);
            assert_eq!(joined(&p), song, "at {most}: {p:?}");
            assert!(p.iter().all(|x| x.text.len() <= most), "at {most}: {p:?}");
        }
        let long = "word ".repeat(300);
        let p = pieces(long.trim_end(), PIECE_BYTES);
        assert_eq!(joined(&p), long.trim_end());
        assert!(p.len() >= 3 && p.iter().all(|x| x.text.len() <= PIECE_BYTES));
        // A word longer than a piece is cut at a character's edge, never in
        // the middle of one.
        let p = pieces(&"é".repeat(10), 5);
        assert_eq!(joined(&p), "é".repeat(10));
        assert_eq!(pieces("one piece", 500).len(), 1);
        assert_eq!(joined(&pieces("ends in a break\n", 6)), "ends in a break\n");
    }

    #[test]
    fn the_question_carries_the_text_and_the_pair_escaped() {
        let pt = Tongue::new("pt-BR").unwrap();
        let r = request("hello world", None, &pt);
        assert_eq!(
            r.url,
            "https://api.mymemory.translated.net/get?q=hello%20world&langpair=Autodetect%7Cpt-BR"
        );
        assert!(r.headers.is_empty(), "keyless");
        let en = Tongue::new("en").unwrap();
        assert!(
            request("x", Some(&en), &pt)
                .url
                .ends_with("langpair=en%7Cpt-BR")
        );
    }

    #[test]
    fn an_answer_is_read_in_all_its_shapes() {
        let ok = br#"{"responseData":{"translatedText":"it&#39;s a house","match":1,"detectedLanguage":"pt"},"quotaFinished":false,"responseStatus":200}"#;
        let (text, from) = read(ok).unwrap();
        assert_eq!(text, "it's a house");
        assert_eq!(from.unwrap().code(), "pt");
        let pair = br#"{"responseData":{"translatedText":"'XX' IS AN INVALID TARGET LANGUAGE"},"responseStatus":"403"}"#;
        assert_eq!(read(pair), Err(Why::UnknownTongue));
        let quota = br#"{"responseData":{"translatedText":"MYMEMORY WARNING: YOU USED ALL AVAILABLE FREE TRANSLATIONS FOR TODAY"},"responseStatus":200}"#;
        assert_eq!(read(quota), Err(Why::Quota));
        assert_eq!(
            read(br#"{"responseData":null,"quotaFinished":true,"responseStatus":200}"#),
            Err(Why::Quota)
        );
        assert_eq!(read(b"<html>"), Err(Why::Garbled));
        assert_eq!(
            read(br#"{"responseData":{},"responseStatus":200}"#),
            Err(Why::Garbled)
        );
    }
}
