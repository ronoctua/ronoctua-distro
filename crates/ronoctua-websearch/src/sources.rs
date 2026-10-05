//! Every source's request and every source's answer, PURE: an address built
//! from words, and hits read from bytes. Nothing here sends.
//!
//! # Read loosely, one field at a time
//!
//! An answer is read as a `serde_json::Value` and each field taken out by
//! hand, the shape of `ronoctua_ytdlp`'s listings: a result whose snippet
//! came back as a number loses its snippet, and the other nineteen stay.

use ronoctua_consent::keys::Secret;
use serde_json::Value;

use crate::{BRAVE_HOST, DUCKDUCKGO_HOST, Request, Source, WebHit, Why};

/// The header Brave reads its key from.
pub const BRAVE_HEADER: &str = "X-Subscription-Token";

/// The longest snippet kept, in grapheme clusters — what a person reads as
/// characters, and where text may be cut (`ronoctua_protocol::cells_of`): a
/// line under a title.
pub const SNIPPET_MOST: usize = 300;

/// `text` escaped for a query string: everything but the unreserved
/// characters of RFC 3986 as `%XX` of its UTF-8. Pure.
#[must_use]
pub fn escape(text: &str) -> String {
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

/// A path segment escaped the way Wikipedia writes its titles: the same as
/// [`escape`], but the punctuation a title keeps literally (`(`, `)`, `,`,
/// `'`, `:`) stays, so the address reads as the page's own.
fn escape_title(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    for b in key.bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'_' | b'.' | b'~' | b'(' | b')' | b',' | b'\'' | b':'
            )
        {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A SearXNG address's host and its base (without a trailing `/`), when it
/// is `https` with a host. Pure.
#[must_use]
pub fn searxng_base(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let rest = url.strip_prefix("https://")?;
    if rest.contains(['?', '#', ' ']) {
        return None;
    }
    let end = rest.find(['/', ':']).unwrap_or(rest.len());
    let host = ronoctua_consent::normal(&rest[..end])?;
    if rest[..end].contains('@') {
        return None;
    }
    Some((host, url.trim_end_matches('/').to_owned()))
}

/// The request that asks `source` for `words`, at most `most` results.
///
/// # Errors
///
/// [`Why::Failed`] for a source with no host to ask (a Wikipedia tongue that
/// is not a subdomain, a SearXNG address that is not `https`), and
/// [`Why::NoKey`] for Brave without `key`.
pub fn request(
    source: &Source,
    words: &str,
    most: usize,
    key: Option<Secret>,
) -> Result<Request, Why> {
    let q = escape(words);
    match source {
        Source::Wikipedia { .. } => {
            let host = source.host().ok_or(Why::Failed)?;
            Ok(Request {
                url: format!("https://{host}/w/rest.php/v1/search/page?q={q}&limit={most}"),
                host,
                key: None,
            })
        }
        Source::DuckDuckGo => Ok(Request {
            host: DUCKDUCKGO_HOST.to_owned(),
            url: format!(
                "https://{DUCKDUCKGO_HOST}/?q={q}&format=json&no_html=1&skip_disambig=1&no_redirect=1"
            ),
            key: None,
        }),
        Source::SearXNG { url } => {
            let (host, base) = searxng_base(url).ok_or(Why::Failed)?;
            Ok(Request {
                host,
                url: format!("{base}/search?q={q}&format=json"),
                key: None,
            })
        }
        Source::Brave => {
            let key = key.ok_or(Why::NoKey)?;
            Ok(Request {
                host: BRAVE_HOST.to_owned(),
                url: format!("https://{BRAVE_HOST}/res/v1/web/search?q={q}&count={most}"),
                key: Some((BRAVE_HEADER, key)),
            })
        }
    }
}

/// What an HTTP status says before the body is read: `None` for a success.
#[must_use]
pub const fn why_of_status(source: &Source, status: u16) -> Option<Why> {
    match status {
        200..=299 => None,
        401 | 403 if matches!(source, Source::Brave) => Some(Why::KeyRefused),
        429 => Some(Why::Paced),
        _ => Some(Why::Failed),
    }
}

/// The hits of `source`'s answer, at most `most`.
///
/// # Errors
///
/// What the status says ([`why_of_status`]), and [`Why::Garbled`] for a body
/// that is not the source's JSON.
pub fn read(source: &Source, status: u16, body: &[u8], most: usize) -> Result<Vec<WebHit>, Why> {
    if let Some(why) = why_of_status(source, status) {
        return Err(why);
    }
    let top: Value = serde_json::from_slice(body).map_err(|_| Why::Garbled)?;
    if !top.is_object() {
        return Err(Why::Garbled);
    }
    let mut hits = match source {
        Source::Wikipedia { .. } => wikipedia(&top, source)?,
        Source::DuckDuckGo => duckduckgo(&top),
        Source::SearXNG { .. } => searxng(&top)?,
        Source::Brave => brave(&top)?,
    };
    for h in &mut hits {
        h.source = source.clone();
    }
    hits.truncate(most);
    Ok(hits)
}

fn text(v: &Value, name: &str) -> Option<String> {
    v.get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Only an `https` (or `http`) address is an address a result may carry —
/// what the kiosk or a browser opens.
fn address(v: &Value, name: &str) -> Option<String> {
    text(v, name).filter(|u| u.starts_with("https://") || u.starts_with("http://"))
}

/// One hit, the source to be set by [`read`]; an untitled result is named by
/// its address.
fn hit(title: Option<String>, url: String, snippet: Option<String>) -> WebHit {
    WebHit {
        title: title.unwrap_or_else(|| url.clone()),
        url,
        snippet: snippet
            .map(|s| plain(&s))
            .filter(|s| !s.is_empty())
            // CUT BETWEEN CLUSTERS, never inside one: a codepoint count
            // splits a flag or a family and draws something else
            // (`ink-is-not-a-count`).
            .map(|s| ronoctua_protocol::cells_of(&s).take(SNIPPET_MOST).collect()),
        source: Source::DuckDuckGo,
    }
}

fn wikipedia(top: &Value, source: &Source) -> Result<Vec<WebHit>, Why> {
    let pages = top
        .get("pages")
        .and_then(Value::as_array)
        .ok_or(Why::Garbled)?;
    let host = source.host().ok_or(Why::Failed)?;
    Ok(pages
        .iter()
        .filter_map(|p| {
            let key = text(p, "key")?;
            let url = format!("https://{host}/wiki/{}", escape_title(&key));
            let snippet = text(p, "excerpt").or_else(|| text(p, "description"));
            Some(hit(text(p, "title"), url, snippet))
        })
        .collect())
}

fn duckduckgo(top: &Value) -> Vec<WebHit> {
    let mut out = Vec::new();
    if let Some(url) = address(top, "AbstractURL") {
        out.push(hit(text(top, "Heading"), url, text(top, "AbstractText")));
    }
    let topic = |t: &Value, out: &mut Vec<WebHit>| {
        let Some(url) = address(t, "FirstURL") else {
            return;
        };
        let said = text(t, "Text").unwrap_or_default();
        let (title, snippet) = match said.split_once(" - ") {
            Some((title, rest)) => (title.trim().to_owned(), Some(rest.trim().to_owned())),
            None => (said, None),
        };
        out.push(hit((!title.is_empty()).then_some(title), url, snippet));
    };
    for r in top
        .get("Results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        topic(r, &mut out);
    }
    for t in top
        .get("RelatedTopics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        // A GROUP OF TOPICS has a name and no address of its own: its
        // members are the results, and the name is not one.
        match t.get("Topics").and_then(Value::as_array) {
            Some(group) => group.iter().for_each(|m| topic(m, &mut out)),
            None => topic(t, &mut out),
        }
    }
    out
}

fn searxng(top: &Value) -> Result<Vec<WebHit>, Why> {
    let results = top
        .get("results")
        .and_then(Value::as_array)
        .ok_or(Why::Garbled)?;
    Ok(results
        .iter()
        .filter_map(|r| {
            let url = address(r, "url")?;
            Some(hit(text(r, "title"), url, text(r, "content")))
        })
        .collect())
}

fn brave(top: &Value) -> Result<Vec<WebHit>, Why> {
    // NO `web` AT ALL is a page with no web results (a query Brave answers
    // with news only); something that is not an object there is garbage.
    let Some(web) = top.get("web") else {
        return Ok(Vec::new());
    };
    let results = web
        .get("results")
        .and_then(Value::as_array)
        .ok_or(Why::Garbled)?;
    Ok(results
        .iter()
        .filter_map(|r| {
            let url = address(r, "url")?;
            Some(hit(text(r, "title"), url, text(r, "description")))
        })
        .collect())
}

/// `html` as plain text: tags out, the five entities and numeric ones
/// decoded, runs of white space made one space. A snippet's highlighting
/// (`<span class="searchmatch">`, `<strong>`) is markup, not words. Pure.
#[must_use]
pub fn plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => {
                for d in chars.by_ref() {
                    if d == '>' {
                        break;
                    }
                }
            }
            '&' => {
                let mut name = String::new();
                while let Some(&d) = chars.peek() {
                    if d == ';' || name.len() > 10 || d.is_whitespace() {
                        break;
                    }
                    name.push(d);
                    chars.next();
                }
                let closed = chars.peek() == Some(&';');
                match (closed, entity(&name)) {
                    (true, Some(e)) => {
                        chars.next();
                        out.push(e);
                    }
                    _ => {
                        out.push('&');
                        out.push_str(&name);
                    }
                }
            }
            c => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" | "#39" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let n = name.strip_prefix('#')?;
            let code = match n.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => n.parse().ok()?,
            };
            char::from_u32(code).filter(|c| !c.is_control())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_markup_of_a_snippet_is_taken_out() {
        assert_eq!(
            plain("<b>Rust</b> &amp; Go &#233;t&#xE9; &bogus x&lt;y"),
            "Rust & Go été &bogus x<y"
        );
        assert_eq!(plain("  a \n\t b  "), "a b");
    }

    /// FAILS IF: a long snippet is cut inside a grapheme cluster — a family
    /// emoji split into its people — or past [`SNIPPET_MOST`] clusters.
    #[test]
    fn a_long_snippet_is_cut_between_clusters() {
        let family = "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let long = family.repeat(SNIPPET_MOST + 5);
        let h = hit(None, "https://a.example".to_owned(), Some(long));
        let kept = h.snippet.expect("a snippet");
        assert_eq!(kept, family.repeat(SNIPPET_MOST));
    }

    #[test]
    fn an_address_escapes_what_is_not_unreserved() {
        assert_eq!(escape("a b/ç?"), "a%20b%2F%C3%A7%3F");
        assert_eq!(escape_title("Rust_(x),_y"), "Rust_(x),_y");
    }

    #[test]
    fn a_searxng_address_is_https_with_a_host() {
        assert_eq!(
            searxng_base("https://s.example.org/sx/"),
            Some((
                "s.example.org".to_owned(),
                "https://s.example.org/sx".to_owned()
            ))
        );
        assert_eq!(searxng_base("http://s.example.org"), None);
        assert_eq!(searxng_base("https://u@s.example.org"), None);
        assert_eq!(searxng_base("https://s.example.org/?x"), None);
    }

    #[test]
    fn brave_without_a_key_is_no_request() {
        assert_eq!(
            request(&Source::Brave, "x", 5, None).unwrap_err(),
            Why::NoKey
        );
        assert_eq!(
            request(
                &Source::Wikipedia {
                    tongue: "p t".to_owned()
                },
                "x",
                5,
                None
            )
            .unwrap_err(),
            Why::Failed
        );
    }

    #[test]
    fn brave_with_no_web_section_answers_nothing_and_garbage_is_said() {
        assert_eq!(
            read(&Source::Brave, 200, br#"{"news":{}}"#, 5).unwrap(),
            Vec::new()
        );
        assert_eq!(
            read(&Source::Brave, 200, br#"{"web":3}"#, 5).unwrap_err(),
            Why::Garbled
        );
        assert_eq!(
            read(&Source::DuckDuckGo, 200, b"[1]", 5).unwrap_err(),
            Why::Garbled
        );
    }
}
