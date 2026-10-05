//! A question to a host, before it is sent, and the one seam it crosses.
//!
//! A provider module builds a [`Request`] — an address that never carries a
//! secret, headers and a body that may — and hands it to a [`Net`] together
//! with the [`Pass`] the person gave for its host. The machine's `Net` is
//! [`crate::internet::Internet`]; a test's is a list of answers that records
//! every request and every pass, so each question is asserted without a
//! socket.
//!
//! The pass travels WITH the request, down to `ronoctua_http::web::request_let`:
//! a `Net` cannot be written that sends without one.

use ronoctua_consent::Pass;
use ronoctua_consent::keys::Secret;
use ronoctua_http::Reply;

use crate::Why;

/// The most a JSON answer may be. A translation of a song is a few kilobytes;
/// Wiktionary's page of a common word, every language's definitions of it,
/// measured 60 KB (`casa`), and its wikitext a little more.
pub const MOST_BYTES: u64 = 1 << 20;

/// A header's or a body's text: plain, or a [`Secret`] that never prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Text anybody may read.
    Plain(String),
    /// A key, or a body with a key in it.
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

/// One question to a host.
///
/// Its `Debug` is safe to log: a [`Value::Secret`] redacts itself, and the
/// address never carries a secret (the text to translate may be in it, which
/// is the person's and goes to the host they let, and nowhere else).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The host of the address — whose pace it waits for and which the pass
    /// must be for.
    pub host: String,
    /// GET or POST.
    pub method: Method,
    /// The address, `https`.
    pub url: String,
    /// Headers beyond the transport's own (User-Agent, Accept-Encoding).
    pub headers: Vec<(&'static str, Value)>,
    /// The JSON body of a POST.
    pub body: Option<Value>,
    /// The `Accept` header.
    pub accept: &'static str,
    /// The most bytes of body.
    pub most_bytes: u64,
}

impl Request {
    /// A GET of `url` on `host`: JSON, [`MOST_BYTES`].
    #[must_use]
    pub fn get(host: &str, url: impl Into<String>) -> Self {
        Self {
            host: host.to_owned(),
            method: Method::Get,
            url: url.into(),
            headers: Vec::new(),
            body: None,
            accept: "application/json",
            most_bytes: MOST_BYTES,
        }
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
}

/// The network, as the providers use it.
///
/// `Sync`, because a door asks from a thread and the player's daemon asks
/// several things at once.
pub trait Net: Sync {
    /// Send `request` under `pass`, following redirects, and give back the
    /// last hop's reply whatever its status (reading a 404 or a 429 is the
    /// provider's).
    ///
    /// # Errors
    ///
    /// [`Why::WrongPass`] when `pass` is not for the request's host (nothing
    /// is sent), [`Why::Paced`], and the question never getting an answer:
    /// [`Why::Unreachable`], [`Why::Failed`], [`Why::Garbled`].
    fn send(&self, pass: &Pass, request: &Request) -> Result<Reply, Why>;
}

/// A network that is not there: what a build without the `net` feature asks
/// through. Every question is [`Why::Unreachable`] — true of that build, and
/// said, never an empty answer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Unwired;

impl Net for Unwired {
    fn send(&self, _: &Pass, _: &Request) -> Result<Reply, Why> {
        Err(Why::Unreachable)
    }
}

/// Which refusal a status is, for a status that is not a 2xx. Pure.
///
/// The providers' own statuses first (DeepL's 456 is its quota), then the
/// common ones: a key refused (401, 403), asked too often (429), too long
/// (413, 414), and a request the host would not read (400) — which for these
/// APIs is a tongue or a pair they do not know.
#[must_use]
pub const fn why_of_status(status: u16) -> Option<Why> {
    match status {
        200..=299 => None,
        400 => Some(Why::UnknownTongue),
        401 | 403 => Some(Why::KeyRefused),
        404 | 410 => Some(Why::NotFound),
        413 | 414 => Some(Why::TooLong),
        429 => Some(Why::Paced),
        456 => Some(Why::Quota),
        _ => Some(Why::Failed),
    }
}

/// Percent-escaping for a query value or a path segment: everything but the
/// unreserved set. Pure.
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
pub(crate) mod canned {
    //! A network made of answers, which records what it was asked and under
    //! which pass: what every test of this crate hands where a door would
    //! reach a host.

    use std::sync::{Mutex, PoisonError};

    use ronoctua_consent::Pass;
    use ronoctua_http::Reply;

    use super::{Net, Request};
    use crate::Why;

    /// Canned answers, each given once, to the request whose address is
    /// exactly the one named.
    #[derive(Default)]
    pub(crate) struct Canned {
        answers: Mutex<Vec<(String, Reply)>>,
        asked: Mutex<Vec<(String, Request)>>,
    }

    impl Canned {
        pub(crate) fn new(answers: &[(&str, u16, &str)]) -> Self {
            Self {
                answers: Mutex::new(
                    answers
                        .iter()
                        .map(|(url, status, body)| {
                            (
                                (*url).to_owned(),
                                Reply {
                                    status: *status,
                                    headers: Vec::new(),
                                    body: body.as_bytes().to_vec(),
                                },
                            )
                        })
                        .collect(),
                ),
                asked: Mutex::new(Vec::new()),
            }
        }

        /// Every request asked, with the host its pass was for.
        pub(crate) fn asked(&self) -> Vec<(String, Request)> {
            self.asked
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
    }

    impl Net for Canned {
        fn send(&self, pass: &Pass, request: &Request) -> Result<Reply, Why> {
            // The same refusal the machine's net gives, before anything is
            // "sent": a test cannot pass by asking under the wrong pass.
            if !pass.covers(&request.host) {
                return Err(Why::WrongPass);
            }
            self.asked
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((pass.host().to_owned(), request.clone()));
            let mut answers = self.answers.lock().unwrap_or_else(PoisonError::into_inner);
            let at = answers
                .iter()
                .position(|(u, _)| *u == request.url)
                .ok_or(Why::Unreachable)?;
            Ok(answers.remove(at).1)
        }
    }

    /// A pass for `host`, minted the only way there is: the person let it, in
    /// a consent file of the test's own.
    pub(crate) fn pass_for(host: &str) -> Pass {
        // ONE FOLDER PER CALL: tests run in parallel, and two minting a pass
        // for the same host in one folder would remove it under each other.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("ronoctua-words-pass-{}-{n}", std::process::id()));
        let store = ronoctua_consent::store_at(Some(dir.clone()));
        ronoctua_consent::let_hosts_in(&store, &[host], true).unwrap();
        let pass = ronoctua_consent::pass_in(&store, host).unwrap();
        let _ = std::fs::remove_dir_all(dir);
        pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_is_a_refusal_with_its_own_word() {
        assert_eq!(why_of_status(200), None);
        assert_eq!(why_of_status(456), Some(Why::Quota));
        assert_eq!(why_of_status(403), Some(Why::KeyRefused));
        assert_eq!(why_of_status(429), Some(Why::Paced));
        assert_eq!(why_of_status(404), Some(Why::NotFound));
        assert_eq!(why_of_status(503), Some(Why::Failed));
    }

    #[test]
    fn escaping_keeps_only_the_unreserved_set() {
        assert_eq!(escape("olá mundo|x"), "ol%C3%A1%20mundo%7Cx");
        assert_eq!(escape("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn a_request_never_prints_its_secret() {
        let r = Request::get("api.deepl.com", "https://api.deepl.com/v2/translate")
            .secret_header("Authorization", Secret::new("DeepL-Auth-Key k-e-y"));
        let shown = format!("{r:?}");
        assert!(!shown.contains("k-e-y"), "{shown}");
    }
}
