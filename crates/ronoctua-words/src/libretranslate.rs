//! A LibreTranslate server the person names: its address, and a key when it
//! wants one.
//!
//! `POST <address>/translate` with a JSON body; the key, when there is one,
//! goes IN that body as `api_key` (the API takes it nowhere else) — so the
//! body is carried as a secret and never printed. Without a kept key the body
//! goes without one: a server the person runs may ask none.

use ronoctua_consent::keys::Secret;
use serde::Deserialize;

use crate::ask::{Request, Value};
use crate::{Tongue, Why, host_of};

/// The most bytes of text one translation takes. A server sets its own limit
/// (and says so, as a 400); this is a bound on what is sent at all.
pub const MOST_TEXT_BYTES: usize = 20 * 1024;

/// The server's host and its `/translate` address, for an `https` address
/// with no query or fragment. Pure.
#[must_use]
pub fn endpoint(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let host = host_of(url)?;
    if url.contains(['?', '#']) {
        return None;
    }
    Some((host, format!("{}/translate", url.trim_end_matches('/'))))
}

/// The question, or `None` for an address [`endpoint`] refuses. Pure.
#[must_use]
pub fn request(
    url: &str,
    text: &str,
    from: Option<&Tongue>,
    to: &Tongue,
    key: Option<&Secret>,
) -> Option<Request> {
    let (host, at) = endpoint(url)?;
    let mut body = serde_json::json!({
        "q": text,
        "source": from.map_or("auto", Tongue::primary),
        "target": to.primary(),
        "format": "text",
    });
    let body = match key {
        Some(key) => {
            body["api_key"] = key.expose().trim().into();
            Value::Secret(Secret::new(body.to_string()))
        }
        None => Value::Plain(body.to_string()),
    };
    Some(Request::get(&host, at).posting(body))
}

#[derive(Deserialize)]
struct Answer {
    #[serde(rename = "translatedText")]
    text: Option<String>,
    #[serde(rename = "detectedLanguage", default)]
    detected: Option<Detected>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct Detected {
    language: Option<String>,
}

/// What an answer says, whatever its status: the translation and the tongue
/// detected, or the refusal its `error` and status are. `keyed` is whether a
/// key was sent. Pure.
///
/// # Errors
///
/// [`Why::NoKey`] (the server wants a key and none is kept),
/// [`Why::KeyRefused`], [`Why::TooLong`] (past the server's limit),
/// [`Why::UnknownTongue`], [`Why::Paced`], [`Why::Failed`] and
/// [`Why::Garbled`].
pub fn read(status: u16, body: &[u8], keyed: bool) -> Result<(String, Option<Tongue>), Why> {
    let answer: Option<Answer> = serde_json::from_slice(body).ok();
    if (200..300).contains(&status) {
        let answer = answer.ok_or(Why::Garbled)?;
        let text = answer.text.ok_or(Why::Garbled)?;
        let from = answer
            .detected
            .and_then(|d| d.language)
            .as_deref()
            .and_then(Tongue::new);
        return Ok((text, from));
    }
    let said = answer
        .and_then(|a| a.error)
        .unwrap_or_default()
        .to_ascii_lowercase();
    Err(match status {
        400 | 403 if said.contains("api key") || said.contains("api_key") => {
            if keyed {
                Why::KeyRefused
            } else {
                Why::NoKey
            }
        }
        400 if said.contains("limit") => Why::TooLong,
        400 => Why::UnknownTongue,
        403 => Why::KeyRefused,
        other => crate::ask::why_of_status(other).unwrap_or(Why::Failed),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_is_https_and_the_key_is_in_the_body_only() {
        assert_eq!(
            endpoint("https://lt.example.org/"),
            Some((
                "lt.example.org".into(),
                "https://lt.example.org/translate".into()
            ))
        );
        assert_eq!(
            endpoint("https://lt.example.org:5000/sub").unwrap().1,
            "https://lt.example.org:5000/sub/translate"
        );
        assert_eq!(endpoint("http://lt.example.org"), None, "https only");
        assert_eq!(endpoint("https://lt.example.org/?k=1"), None);

        let pt = Tongue::new("pt-BR").unwrap();
        let key = Secret::new("s3cret");
        let r = request("https://lt.example.org", "hello", None, &pt, Some(&key)).unwrap();
        assert_eq!(r.host, "lt.example.org");
        assert!(!r.url.contains("s3cret") && r.headers.is_empty());
        let body = r.body.as_ref().unwrap();
        assert!(
            matches!(body, Value::Secret(_)),
            "a body with a key is a secret"
        );
        assert!(!format!("{r:?}").contains("s3cret"));
        let json: serde_json::Value = serde_json::from_str(body.expose()).unwrap();
        assert_eq!(json["api_key"], "s3cret");
        assert_eq!(json["source"], "auto");
        assert_eq!(json["target"], "pt");
        let bare = request("https://lt.example.org", "hi", None, &pt, None).unwrap();
        assert!(matches!(bare.body, Some(Value::Plain(_))));
        assert!(!bare.body.unwrap().expose().contains("api_key"));
    }

    #[test]
    fn an_answer_and_its_refusals() {
        let (text, from) = read(
            200,
            r#"{"translatedText":"olá","detectedLanguage":{"confidence":90,"language":"en"}}"#
                .as_bytes(),
            false,
        )
        .unwrap();
        assert_eq!((text.as_str(), from.unwrap().code()), ("olá", "en"));
        let wants = br#"{"error":"Visit https://portal.libretranslate.com to get an API key"}"#;
        assert_eq!(read(400, wants, false), Err(Why::NoKey));
        assert_eq!(read(400, wants, true), Err(Why::KeyRefused));
        assert_eq!(
            read(
                400,
                br#"{"error":"Invalid request: request (9000) exceeds text limit (5000)"}"#,
                true
            ),
            Err(Why::TooLong)
        );
        assert_eq!(
            read(400, br#"{"error":"xx is not supported"}"#, false),
            Err(Why::UnknownTongue)
        );
        assert_eq!(read(429, b"", false), Err(Why::Paced));
        assert_eq!(read(502, b"<html>", false), Err(Why::Failed));
        assert_eq!(read(200, b"<html>", false), Err(Why::Garbled));
    }
}
