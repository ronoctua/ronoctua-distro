//! DeepL, with the person's key (`api-free.deepl.com` or `api.deepl.com`).
//!
//! `POST /v2/translate` with a JSON body and the key in the `Authorization`
//! header, as `DeepL-Auth-Key <key>` — never in the body, never in the
//! address. A key that ends in `:fx` is a free account's and goes to the free
//! host; any other goes to the paid one. Its statuses are its refusals: 403
//! the key, 456 the month's characters, 429 too many at once.

use ronoctua_consent::keys::Secret;
use serde::Deserialize;

use crate::ask::{Request, Value};
use crate::{DEEPL_FREE_HOST, DEEPL_HOST, Tongue, Why};

/// The most bytes of text one translation takes. DeepL's request is at most
/// 128 KiB whole; this leaves the JSON around it room.
pub const MOST_TEXT_BYTES: usize = 100 * 1024;

/// The host a key belongs to.
#[must_use]
pub fn host_for(key: &Secret) -> &'static str {
    if key.expose().trim_end().ends_with(":fx") {
        DEEPL_FREE_HOST
    } else {
        DEEPL_HOST
    }
}

/// DeepL's code for a target tongue: its primary subtag in capitals, and the
/// two it wants a variant of (`EN`, `PT`) with the one asked — or, when none
/// was, the one this distro speaks (`EN-GB`, `PT-BR`). Pure.
#[must_use]
pub fn target_code(to: &Tongue) -> String {
    let primary = to.primary().to_ascii_uppercase();
    let region = to.code().split('-').nth(1).map(str::to_ascii_uppercase);
    match (primary.as_str(), region.as_deref()) {
        ("EN", Some("US")) => "EN-US".into(),
        ("EN", _) => "EN-GB".into(),
        ("PT", Some("PT")) => "PT-PT".into(),
        ("PT", _) => "PT-BR".into(),
        _ => primary,
    }
}

/// The question. The body is plain — the text is the person's, going where
/// they let it — and the key rides in its header as a [`Secret`]. Pure.
#[must_use]
pub fn request(text: &str, from: Option<&Tongue>, to: &Tongue, key: &Secret) -> Request {
    let host = host_for(key);
    let mut body = serde_json::json!({
        "text": [text],
        "target_lang": target_code(to),
        "preserve_formatting": true,
    });
    if let Some(from) = from {
        body["source_lang"] = from.primary().to_ascii_uppercase().into();
    }
    Request::get(host, format!("https://{host}/v2/translate"))
        .secret_header(
            "Authorization",
            Secret::new(format!("DeepL-Auth-Key {}", key.expose().trim())),
        )
        .posting(Value::Plain(body.to_string()))
}

#[derive(Deserialize)]
struct Answer {
    translations: Vec<One>,
}

#[derive(Deserialize)]
struct One {
    text: String,
    #[serde(default)]
    detected_source_language: Option<String>,
}

/// What a 2xx answer says: the translation and the tongue DeepL detected.
/// Pure.
///
/// # Errors
///
/// [`Why::Garbled`] for what is not its JSON, or holds no translation.
pub fn read(body: &[u8]) -> Result<(String, Option<Tongue>), Why> {
    let answer: Answer = serde_json::from_slice(body).map_err(|_| Why::Garbled)?;
    let one = answer.translations.into_iter().next().ok_or(Why::Garbled)?;
    Ok((
        one.text,
        one.detected_source_language
            .as_deref()
            .and_then(Tongue::new),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_free_key_goes_to_the_free_host_and_the_key_only_in_its_header() {
        let free = Secret::new("abcd-1234:fx");
        let pt = Tongue::new("pt-BR").unwrap();
        let r = request("hello", None, &pt, &free);
        assert_eq!(r.host, DEEPL_FREE_HOST);
        assert_eq!(r.url, "https://api-free.deepl.com/v2/translate");
        let (name, value) = &r.headers[0];
        assert_eq!(*name, "Authorization");
        assert_eq!(value.expose(), "DeepL-Auth-Key abcd-1234:fx");
        assert!(matches!(value, Value::Secret(_)), "the header is a secret");
        let body = r.body.as_ref().unwrap().expose();
        assert!(
            !body.contains("abcd"),
            "the key is never in the body: {body}"
        );
        assert!(!r.url.contains("abcd"), "nor in the address");
        let json: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(json["text"][0], "hello");
        assert_eq!(json["target_lang"], "PT-BR");
        assert!(json.get("source_lang").is_none(), "detected when not said");
        let paid = request(
            "x",
            Some(&Tongue::new("en").unwrap()),
            &pt,
            &Secret::new("k"),
        );
        assert_eq!(paid.host, DEEPL_HOST);
        let json: serde_json::Value =
            serde_json::from_str(paid.body.as_ref().unwrap().expose()).unwrap();
        assert_eq!(json["source_lang"], "EN");
    }

    #[test]
    fn a_target_takes_the_variant_deepl_wants() {
        let code = |c: &str| target_code(&Tongue::new(c).unwrap());
        assert_eq!(code("en"), "EN-GB");
        assert_eq!(code("en-US"), "EN-US");
        assert_eq!(code("pt"), "PT-BR");
        assert_eq!(code("pt-PT"), "PT-PT");
        assert_eq!(code("de-AT"), "DE");
    }

    #[test]
    fn an_answer_is_its_text_and_the_detected_tongue() {
        let (text, from) =
            read(br#"{"translations":[{"detected_source_language":"EN","text":"casa"}]}"#).unwrap();
        assert_eq!(text, "casa");
        assert_eq!(from.unwrap().code(), "en");
        assert_eq!(read(br#"{"translations":[]}"#), Err(Why::Garbled));
        assert_eq!(read(b"nope"), Err(Why::Garbled));
    }
}
