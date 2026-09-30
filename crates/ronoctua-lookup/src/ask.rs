//! A question to a service, before it is sent, and the one seam it crosses.
//!
//! A service module builds a [`Request`] — an address that never carries a
//! secret, headers that may, a body that may — and hands it to a [`Net`].
//! The machine's `Net` is [`crate::internet::Internet`]; a test's is a list
//! of answers that records every request, so the address and the headers of
//! each question are asserted without a socket.

use ronoctua_http::Reply;
use serde::de::DeserializeOwned;

use crate::keys::Secret;
use crate::{Host, Refusal, Service, Why};

/// The most a JSON answer may be, by default. Every API answer here is a few
/// kilobytes; LRCLIB's search, twenty songs with their lyrics, measured 232
/// KB.
pub const MOST_BYTES: u64 = 1 << 20;

/// A header's or a body's text: plain, or a [`Secret`] that never prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Text anybody may read.
    Plain(String),
    /// A key, a token, a password.
    Secret(Secret),
}

impl Value {
    /// The text, for the wire.
    #[must_use]
    pub fn expose(&self) -> &str {
        match self {
            Self::Plain(s) => s,
            Self::Secret(s) => s.expose(),
        }
    }
}

/// How a request is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// A GET.
    Get,
    /// A POST of a JSON body.
    Post,
}

/// One question to a service.
///
/// Its `Debug` is safe to log: [`Value::Secret`] redacts itself, and the
/// address never carries a secret — the one key a service takes in the query
/// string (SubDL's) rides in [`Request::secret_query`], appended only by the
/// transport ([`Request::exposed_url`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Whose pace it waits for: the address's host, or the service that handed
    /// the address out (OpenSubtitles' download link).
    pub host: Host,
    /// GET or POST.
    pub method: Method,
    /// The address, `https`, with no secret in it.
    pub url: String,
    /// A secret query parameter, appended when the request is sent.
    pub secret_query: Option<(&'static str, Secret)>,
    /// Headers beyond the transport's own (User-Agent, Accept-Encoding).
    pub headers: Vec<(&'static str, Value)>,
    /// The JSON body of a POST.
    pub body: Option<Value>,
    /// The `Accept` header.
    pub accept: &'static str,
    /// Whether the answer may come gzipped (and is inflated by the web).
    pub gzip: bool,
    /// The most bytes of body.
    pub most_bytes: u64,
}

impl Request {
    /// A GET of `url` from `host`'s pace: JSON, gzip, [`MOST_BYTES`].
    #[must_use]
    pub fn get(host: Host, url: impl Into<String>) -> Self {
        Self {
            host,
            method: Method::Get,
            url: url.into(),
            secret_query: None,
            headers: Vec::new(),
            body: None,
            accept: "application/json",
            gzip: true,
            most_bytes: MOST_BYTES,
        }
    }

    /// The same, carrying a plain header.
    #[must_use]
    pub fn header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, Value::Plain(value.into())));
        self
    }

    /// The same, carrying a secret header.
    #[must_use]
    pub fn secret_header(mut self, name: &'static str, value: Secret) -> Self {
        self.headers.push((name, Value::Secret(value)));
        self
    }

    /// The same, as a POST of this JSON body.
    #[must_use]
    pub fn posting(mut self, body: Value) -> Self {
        self.method = Method::Post;
        self.body = Some(body);
        self
    }

    /// The same, with a secret query parameter the transport appends.
    #[must_use]
    pub fn secret_query(mut self, name: &'static str, value: Secret) -> Self {
        self.secret_query = Some((name, value));
        self
    }

    /// The same, accepting this.
    #[must_use]
    pub const fn accepting(mut self, accept: &'static str) -> Self {
        self.accept = accept;
        self
    }

    /// The same, asking for the body as it is (a zip, an image: gzip would
    /// gain nothing).
    #[must_use]
    pub const fn identity(mut self) -> Self {
        self.gzip = false;
        self
    }

    /// The same, reading at most this many bytes.
    #[must_use]
    pub const fn most(mut self, most_bytes: u64) -> Self {
        self.most_bytes = most_bytes;
        self
    }

    /// The service this request asks.
    #[must_use]
    pub const fn service(&self) -> Service {
        self.host.service()
    }

    /// The address with the secret query appended — for the transport, and
    /// nowhere that prints.
    #[must_use]
    pub fn exposed_url(&self) -> String {
        match &self.secret_query {
            None => self.url.clone(),
            Some((name, value)) => {
                let join = if self.url.contains('?') { '&' } else { '?' };
                format!(
                    "{}{join}{name}={}",
                    self.url,
                    crate::query::escape(value.expose())
                )
            }
        }
    }
}

/// The network, as the service modules use it.
///
/// `Sync`, because the daemon asks several services at once on threads that
/// share one.
pub trait Net: Sync {
    /// Send `request`, following redirects, and give back the last hop's
    /// reply whatever its status (reading a 404 or a 429 is the service's).
    ///
    /// # Errors
    ///
    /// The question never got an answer: [`Why::Unreachable`],
    /// [`Why::TooLarge`], [`Why::Malformed`].
    fn send(&self, request: &Request) -> Result<Reply, Why>;
}

/// Send `request` and give back the body of a 2xx, or the refusal its status
/// is ([`why_of_status`]).
///
/// # Errors
///
/// As [`Net::send`], and the status's refusal.
pub fn answered(net: &dyn Net, request: &Request) -> Result<Vec<u8>, Refusal> {
    let service = request.service();
    let reply = net
        .send(request)
        .map_err(|why| Refusal::new(service, why))?;
    match why_of_status(service, reply.status) {
        None => Ok(reply.body),
        Some(why) => Err(Refusal::new(service, why)),
    }
}

/// Which refusal a status is, from `service`; `None` for a 2xx. Pure.
///
/// 401 is the key refused; 403 too for a service that takes a key (a keyless
/// one saying 403 is declining). 404 and 410 are nothing found, 429 is "too
/// many", and so is MusicBrainz's 503 (its documented answer to more than one
/// request a second). OpenSubtitles answers 406 when the day's downloads are
/// spent. Other 5xx are the service being down.
#[must_use]
pub const fn why_of_status(service: Service, status: u16) -> Option<Why> {
    if matches!(status, 200..=299) {
        return None;
    }
    let keyed = service.key().is_some();
    Some(match (service, status) {
        (_, 401) => Why::KeyRefused,
        (_, 403) if keyed => Why::KeyRefused,
        (_, 404 | 410) => Why::NothingFound,
        (_, 429) => Why::TooManyAsks,
        (Service::MusicBrainz, 503) => Why::TooManyAsks,
        (Service::OpenSubtitles, 406) => Why::Quota,
        (_, 500..=599) => Why::Unreachable,
        _ => Why::Declined,
    })
}

/// A JSON body read as `T`, or [`Why::Malformed`] from `service`.
///
/// # Errors
///
/// The body is not that JSON.
pub fn json<T: DeserializeOwned>(service: Service, body: &[u8]) -> Result<T, Refusal> {
    serde_json::from_slice(body).map_err(|_| Refusal::new(service, Why::Malformed))
}

/// A JSON OBJECT read as `T`, or [`Why::Malformed`] from `service`.
///
/// The answers here are read loosely — every field defaulted — and serde
/// reads a struct out of an ARRAY too, by position: `[]` would be an empty
/// answer and not the malformed one it is. So the top is checked first.
///
/// # Errors
///
/// The body is not a JSON object, or not that one.
pub fn object<T: DeserializeOwned>(service: Service, body: &[u8]) -> Result<T, Refusal> {
    let malformed = || Refusal::new(service, Why::Malformed);
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|_| malformed())?;
    if !value.is_object() {
        return Err(malformed());
    }
    serde_json::from_value(value).map_err(|_| malformed())
}

/// A text that is there: trimmed, and `None` when blank.
#[must_use]
pub fn present(text: Option<&str>) -> Option<String> {
    text.map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_says_which_refusal_it_is() {
        use Service::*;
        assert_eq!(why_of_status(Lrclib, 200), None);
        assert_eq!(why_of_status(Tmdb, 401), Some(Why::KeyRefused));
        assert_eq!(why_of_status(OpenSubtitles, 403), Some(Why::KeyRefused));
        assert_eq!(
            why_of_status(MusicBrainz, 403),
            Some(Why::Declined),
            "a keyless service has no key to refuse"
        );
        assert_eq!(why_of_status(Lrclib, 404), Some(Why::NothingFound));
        assert_eq!(why_of_status(Wikidata, 429), Some(Why::TooManyAsks));
        assert_eq!(why_of_status(MusicBrainz, 503), Some(Why::TooManyAsks));
        assert_eq!(why_of_status(Wikidata, 503), Some(Why::Unreachable));
        assert_eq!(why_of_status(OpenSubtitles, 406), Some(Why::Quota));
        assert_eq!(why_of_status(SubDl, 406), Some(Why::Declined));
        assert_eq!(why_of_status(Lrclib, 400), Some(Why::Declined));
    }

    #[test]
    fn a_secret_query_is_appended_only_for_the_wire() {
        let r = Request::get(
            Host::SubDl,
            "https://api.subdl.com/api/v1/subtitles?film_name=x",
        )
        .secret_query("api_key", Secret::new("k&y"));
        assert_eq!(
            r.exposed_url(),
            "https://api.subdl.com/api/v1/subtitles?film_name=x&api_key=k%26y"
        );
        let shown = format!("{r:?}");
        assert!(
            !shown.contains("k&y") && !shown.contains("k%26y"),
            "{shown}"
        );
        let bare = Request::get(Host::SubDl, "https://api.subdl.com/x")
            .secret_query("api_key", Secret::new("k"));
        assert_eq!(bare.exposed_url(), "https://api.subdl.com/x?api_key=k");
    }

    #[test]
    fn a_secret_header_and_body_never_print() {
        let r = Request::get(
            Host::OpenSubtitles,
            "https://api.opensubtitles.com/api/v1/login",
        )
        .secret_header("Api-Key", Secret::new("the-api-key"))
        .posting(Value::Secret(Secret::new(r#"{"password":"hunter2"}"#)));
        let shown = format!("{r:?}");
        assert!(shown.contains("Api-Key"), "{shown}");
        assert!(!shown.contains("the-api-key"), "{shown}");
        assert!(!shown.contains("hunter2"), "{shown}");
        assert_eq!(r.method, Method::Post);
    }

    #[test]
    fn an_array_is_not_an_object_with_nothing_in_it() {
        #[derive(serde::Deserialize, Default, Debug)]
        #[serde(default)]
        struct Loose {
            #[allow(dead_code)]
            items: Vec<u8>,
        }
        assert!(object::<Loose>(Service::Wikidata, br#"{"items":[1]}"#).is_ok());
        assert!(object::<Loose>(Service::Wikidata, b"{}").is_ok());
        assert_eq!(
            object::<Loose>(Service::Wikidata, b"[]").unwrap_err().why,
            Why::Malformed
        );
        assert!(
            json::<Loose>(Service::Wikidata, b"[]").is_ok(),
            "why `object` exists"
        );
    }

    #[test]
    fn blank_is_absent() {
        assert_eq!(present(Some("  x ")).as_deref(), Some("x"));
        assert_eq!(present(Some("   ")), None);
        assert_eq!(present(None), None);
    }
}
