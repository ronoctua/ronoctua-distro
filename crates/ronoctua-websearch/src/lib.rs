//! Web search for fast-access's `/w` — the one crate that talks to a search
//! engine.
//!
//! # The sources
//!
//! Wikipedia's REST search (`<tongue>.wikipedia.org`) and DuckDuckGo's
//! Instant Answers (`api.duckduckgo.com`), both keyless; a SearXNG the person
//! runs or names (its URL); Brave Search with the person's key (the
//! `X-Subscription-Token` header, kept `0600` in `websearch-keys`). The row
//! "search the web in the kiosk" sends no request of ours: it is an ADDRESS
//! the Chrome kiosk opens ([`kiosk_address`]), and nothing here asks it.
//!
//! # Consent and manners
//!
//! Every search takes a [`ronoctua_consent::Pass`] for the source's host
//! ([`Source::host`]); without one the door draws the consent row. One pace
//! per host ([`pace`]). Nothing asks on its own: fast-access asks after
//! 250 ms without a key, on a thread it stops at the next key — the
//! [`internet::Internet`] it hands in carries an abort that shuts the socket.
//!
//! # Pure, but for the net
//!
//! [`sources`] builds every request (the address, the headers) and reads
//! every answer, from bytes: each rule is a test over hand-written JSON. The
//! `net` feature adds the transport; a test hands a [`Net`] of answers.
//!
//! # Blocks
//!
//! Call it on a thread, never on a drawing one.

#![forbid(unsafe_code)]

use ronoctua_consent::Pass;
use ronoctua_consent::keys::Secret;
use serde::{Deserialize, Serialize};

#[cfg(feature = "net")]
pub mod internet;
pub mod keys;
pub mod pace;
pub mod sources;

/// DuckDuckGo's Instant Answers host.
pub const DUCKDUCKGO_HOST: &str = "api.duckduckgo.com";
/// Brave Search's API host.
pub const BRAVE_HOST: &str = "api.search.brave.com";
/// The folder of the person's search keys, under the config root.
pub const KEYS_DIR: &str = "websearch-keys";
/// The Brave key's file in [`KEYS_DIR`].
pub const BRAVE_KEY: &str = "brave";

/// What every request says it is. Wikimedia asks clients to name themselves.
pub const USER_AGENT: &str = concat!(
    "ronoctua-websearch/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/ronoctua/ronoctua-distro)"
);

/// The most results one search answers, whatever is asked: a section of a
/// search box, not a results page.
pub const MOST_HITS: usize = 20;

/// The longest search, in grapheme clusters (what a person reads as
/// characters); what is past it is left out.
pub const WORDS_MOST: usize = 200;

/// The most bytes one answer may be. A page of twenty results with their
/// snippets is a few tens of kilobytes; DuckDuckGo's with its topics a few
/// more.
pub const MOST_BYTES: u64 = 1 << 20;

/// Where a search goes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Source {
    /// Wikipedia, in one language (its subdomain: `en`, `pt`).
    Wikipedia {
        /// The language subdomain.
        tongue: String,
    },
    /// DuckDuckGo's Instant Answers.
    #[serde(rename = "duckduckgo")]
    DuckDuckGo,
    /// A SearXNG instance the person named.
    #[serde(rename = "searxng")]
    SearXNG {
        /// Its base address, `https://…`.
        url: String,
    },
    /// Brave Search, with the person's key.
    Brave,
}

impl Source {
    /// Its kebab-case name, the suffix of its key (`fast-access-web-<id>`).
    #[must_use]
    pub const fn id(&self) -> &'static str {
        match self {
            Self::Wikipedia { .. } => "wikipedia",
            Self::DuckDuckGo => "duckduckgo",
            Self::SearXNG { .. } => "searxng",
            Self::Brave => "brave",
        }
    }

    /// The host its request goes to — what the consent row names and the
    /// [`Pass`] must be for. `None` for a SearXNG address with no host, or a
    /// Wikipedia tongue that is not a subdomain.
    #[must_use]
    pub fn host(&self) -> Option<String> {
        match self {
            Self::Wikipedia { tongue } => {
                let fits = !tongue.is_empty()
                    && tongue.chars().all(|c| c.is_ascii_alphabetic() || c == '-');
                fits.then(|| format!("{}.wikipedia.org", tongue.to_ascii_lowercase()))
            }
            Self::DuckDuckGo => Some(DUCKDUCKGO_HOST.to_owned()),
            Self::SearXNG { url } => sources::searxng_base(url).map(|(host, _)| host),
            Self::Brave => Some(BRAVE_HOST.to_owned()),
        }
    }

    /// Whether it needs a key the person keeps ([`keys`]).
    #[must_use]
    pub const fn needs_a_key(&self) -> bool {
        matches!(self, Self::Brave)
    }
}

/// One result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebHit {
    /// Its title — data, drawn as it came.
    pub title: String,
    /// Its address.
    pub url: String,
    /// A line of what it says, when the source gives one.
    pub snippet: Option<String>,
    /// Where it came from.
    pub source: Source,
}

/// Why a search answered nothing, as a closed set (`fast-access-web-why-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// There were no words.
    Empty,
    /// The pass is for another host than the source's.
    WrongPass,
    /// The source needs a key and the person kept none.
    NoKey,
    /// The source refused the key.
    KeyRefused,
    /// Asked too often.
    Paced,
    /// The host could not be reached.
    Unreachable,
    /// The host answered with a failure.
    Failed,
    /// The host answered something that is not a result list.
    Garbled,
}

impl Why {
    /// Every refusal, for a test that walks the words.
    pub const ALL: [Self; 8] = [
        Self::Empty,
        Self::WrongPass,
        Self::NoKey,
        Self::KeyRefused,
        Self::Paced,
        Self::Unreachable,
        Self::Failed,
        Self::Garbled,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::WrongPass => "wrong-pass",
            Self::NoKey => "no-key",
            Self::KeyRefused => "key-refused",
            Self::Paced => "paced",
            Self::Unreachable => "unreachable",
            Self::Failed => "failed",
            Self::Garbled => "garbled",
        }
    }
}

/// A refusal, and the source it concerns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
    /// The source asked.
    pub source: Source,
}

/// One question to a host, before it is sent: what [`sources`] builds and a
/// [`Net`] sends.
///
/// Its `Debug` is safe to log: the key is a [`Secret`] that redacts itself,
/// and the address never carries one (the words searched are in it, which
/// are the person's and go to the host they let).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The host of the address — whose pace it waits for and which the pass
    /// must be for.
    pub host: String,
    /// The address, `https`.
    pub url: String,
    /// The key's header, for a source that takes one.
    pub key: Option<(&'static str, Secret)>,
}

/// What came back from a host: its status and its body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The HTTP status of the last hop.
    pub status: u16,
    /// The body, inflated.
    pub body: Vec<u8>,
}

/// The network, as a search uses it.
///
/// `Sync`: fast-access asks every source from a thread of its own at once.
pub trait Net: Sync {
    /// Send `request` under `pass`, and give back the answer whatever its
    /// status (reading a 401 or a 429 is the source's).
    ///
    /// # Errors
    ///
    /// [`Why::WrongPass`] when `pass` is not for the request's host (nothing
    /// is sent), [`Why::Paced`], [`Why::Unreachable`], [`Why::Failed`],
    /// [`Why::Garbled`].
    fn send(&self, pass: &Pass, request: &Request) -> Result<Answer, Why>;
}

/// A network that is not there — a build without `net`. Every question is
/// [`Why::Unreachable`]: true of that build, and said.
#[derive(Debug, Clone, Copy, Default)]
pub struct Unwired;

impl Net for Unwired {
    fn send(&self, pass: &Pass, request: &Request) -> Result<Answer, Why> {
        if !pass.covers(&request.host) {
            return Err(Why::WrongPass);
        }
        Err(Why::Unreachable)
    }
}

/// At most `most` results for `words` from `source`, asking the host `pass`
/// lets, through the machine's network and with the person's Brave key.
///
/// One source per call, so fast-access runs one thread per source and each
/// section fills as its answer arrives.
///
/// # Errors
///
/// A [`Refusal`]: [`Why::Empty`], [`Why::WrongPass`], [`Why::NoKey`],
/// [`Why::KeyRefused`], [`Why::Paced`], [`Why::Unreachable`],
/// [`Why::Failed`], [`Why::Garbled`].
pub fn search(
    source: &Source,
    words: &str,
    most: usize,
    pass: &Pass,
) -> Result<Vec<WebHit>, Refusal> {
    #[cfg(feature = "net")]
    let net = internet::Internet::new();
    #[cfg(not(feature = "net"))]
    let net = Unwired;
    search_with(&net, keys::dir().as_ref(), source, words, most, pass)
}

/// [`search`], through `net` and with the keys in `keys` (`None`: none kept).
///
/// # Errors
///
/// As [`search`].
pub fn search_with(
    net: &dyn Net,
    keys: Option<&ronoctua_consent::keys::Dir>,
    source: &Source,
    words: &str,
    most: usize,
    pass: &Pass,
) -> Result<Vec<WebHit>, Refusal> {
    let refused = |why| Refusal {
        why,
        source: source.clone(),
    };
    // CUT BETWEEN CLUSTERS, never inside one (`ink-is-not-a-count`).
    let words: String = ronoctua_protocol::cells_of(words.trim())
        .take(WORDS_MOST)
        .collect();
    let words = words.trim();
    if words.is_empty() {
        return Err(refused(Why::Empty));
    }
    // THE PASS FIRST, before a key is read: a pass for another host is a
    // caller's mistake, and the key is not even looked at for it.
    if !source.host().is_some_and(|h| pass.covers(&h)) {
        return Err(refused(Why::WrongPass));
    }
    let key = if source.needs_a_key() {
        let dir = keys.ok_or_else(|| refused(Why::NoKey))?;
        let kept = dir
            .read(BRAVE_KEY)
            .map_err(|_| refused(Why::KeyRefused))?
            .filter(|k| !k.is_empty());
        Some(kept.ok_or_else(|| refused(Why::NoKey))?)
    } else {
        None
    };
    let most = most.clamp(1, MOST_HITS);
    let request = sources::request(source, words, most, key).map_err(refused)?;
    let answer = net.send(pass, &request).map_err(refused)?;
    sources::read(source, answer.status, &answer.body, most).map_err(refused)
}

/// The address the Chrome kiosk opens for "search the web in the kiosk":
/// DuckDuckGo's own page, asked by the browser the person is looking at —
/// nothing of ours sends it. Pure.
#[must_use]
pub fn kiosk_address(words: &str) -> String {
    // CUT BETWEEN CLUSTERS, never inside one (`ink-is-not-a-count`).
    let words: String = ronoctua_protocol::cells_of(words.trim())
        .take(WORDS_MOST)
        .collect();
    format!(
        "https://duckduckgo.com/?q={}",
        sources::escape(words.trim())
    )
}

#[cfg(test)]
pub(crate) mod canned {
    //! A network made of answers, which records what it was asked.

    use std::sync::{Mutex, PoisonError};

    use ronoctua_consent::Pass;

    use super::{Answer, Net, Request, Why};

    /// Canned answers, each given to the request whose address starts with
    /// the prefix named.
    #[derive(Default)]
    pub(crate) struct Canned {
        answers: Vec<(String, u16, String)>,
        pub(crate) asked: Mutex<Vec<Request>>,
    }

    impl Canned {
        pub(crate) fn new(answers: &[(&str, u16, &str)]) -> Self {
            Self {
                answers: answers
                    .iter()
                    .map(|(u, s, b)| ((*u).to_owned(), *s, (*b).to_owned()))
                    .collect(),
                asked: Mutex::new(Vec::new()),
            }
        }

        pub(crate) fn asked(&self) -> Vec<Request> {
            self.asked
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
    }

    impl Net for Canned {
        fn send(&self, pass: &Pass, request: &Request) -> Result<Answer, Why> {
            if !pass.covers(&request.host) {
                return Err(Why::WrongPass);
            }
            self.asked
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(request.clone());
            self.answers
                .iter()
                .find(|(u, _, _)| request.url.starts_with(u.as_str()))
                .map(|(_, status, body)| Answer {
                    status: *status,
                    body: body.as_bytes().to_vec(),
                })
                .ok_or(Why::Unreachable)
        }
    }

    /// A pass for `host`, from a consent file of the test's own.
    pub(crate) fn pass_for(host: &str) -> Pass {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-websearch-pass-{}-{}",
            std::process::id(),
            host.replace('.', "-")
        ));
        let store = ronoctua_consent::store_at(Some(dir.clone()));
        ronoctua_consent::let_hosts_in(&store, &[host], true).expect("the test's consent");
        let pass = ronoctua_consent::pass_in(&store, host).expect("a pass");
        let _ = std::fs::remove_dir_all(dir);
        pass
    }
}

#[cfg(test)]
mod tests {
    use super::canned::{Canned, pass_for};
    use super::*;

    fn wiki(tongue: &str) -> Source {
        Source::Wikipedia {
            tongue: tongue.to_owned(),
        }
    }

    #[test]
    fn a_search_asks_only_the_host_its_pass_lets() {
        let pt = wiki("pt");
        assert_eq!(pt.host().as_deref(), Some("pt.wikipedia.org"));
        let searx = Source::SearXNG {
            url: "https://search.example.org/searxng".to_owned(),
        };
        assert_eq!(searx.host().as_deref(), Some("search.example.org"));
        assert_eq!(
            Source::SearXNG {
                url: "http://search.example.org".to_owned()
            }
            .host(),
            None,
            "a SearXNG over plain http names no host to let"
        );
        assert_eq!(wiki("p/t").host(), None);

        let pass = pass_for("pt.wikipedia.org");
        let net = Canned::new(&[]);
        assert_eq!(
            search_with(&net, None, &Source::DuckDuckGo, "rust", 5, &pass)
                .unwrap_err()
                .why,
            Why::WrongPass
        );
        assert_eq!(
            search_with(&net, None, &pt, "  ", 5, &pass)
                .unwrap_err()
                .why,
            Why::Empty
        );
        assert!(net.asked().is_empty(), "nothing was sent");
    }

    /// FAILS IF: a Wikipedia answer stops being read into titles, addresses
    /// and snippets with the search's highlighting taken out.
    #[test]
    fn wikipedia_is_read_into_hits() {
        let body = r#"{"pages":[
            {"id":1,"key":"Rust_(Übersicht)","title":"Rust (Übersicht)",
             "excerpt":"<span class=\"searchmatch\">Rust</span> ist eine Sprache &amp; mehr",
             "description":"Programmiersprache"},
            {"id":2,"key":"Rost","title":"Rost","excerpt":null,"description":null}
        ]}"#;
        let net = Canned::new(&[(
            "https://de.wikipedia.org/w/rest.php/v1/search/page",
            200,
            body,
        )]);
        let pass = pass_for("de.wikipedia.org");
        let hits = search_with(&net, None, &wiki("de"), "rust", 5, &pass).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].title, "Rust (Übersicht)");
        assert_eq!(
            hits[0].url,
            "https://de.wikipedia.org/wiki/Rust_(%C3%9Cbersicht)"
        );
        assert_eq!(
            hits[0].snippet.as_deref(),
            Some("Rust ist eine Sprache & mehr")
        );
        assert_eq!(hits[1].snippet, None);
        let asked = net.asked();
        assert!(asked[0].url.contains("q=rust"), "{}", asked[0].url);
        assert!(asked[0].url.contains("limit=5"), "{}", asked[0].url);
    }

    /// FAILS IF: DuckDuckGo's abstract, results and nested topics stop
    /// becoming hits, or a topic group's name is taken for a result.
    #[test]
    fn duckduckgo_reads_the_abstract_and_the_topics() {
        let body = r#"{"Heading":"Rust","AbstractText":"A language.","AbstractURL":"https://en.wikipedia.org/wiki/Rust",
            "Results":[{"Text":"Official site","FirstURL":"https://www.rust-lang.org"}],
            "RelatedTopics":[
              {"Text":"Rust (fungus) - A plant disease.","FirstURL":"https://duckduckgo.com/Rust_(fungus)"},
              {"Name":"Places","Topics":[{"Text":"Rust, Germany - A town.","FirstURL":"https://duckduckgo.com/Rust,_Germany"}]}
            ]}"#;
        let net = Canned::new(&[("https://api.duckduckgo.com/", 200, body)]);
        let pass = pass_for(DUCKDUCKGO_HOST);
        let hits = search_with(&net, None, &Source::DuckDuckGo, "rust", 10, &pass).unwrap();
        let titles: Vec<&str> = hits.iter().map(|h| h.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Rust", "Official site", "Rust (fungus)", "Rust, Germany"]
        );
        assert_eq!(hits[2].snippet.as_deref(), Some("A plant disease."));
        assert!(net.asked()[0].url.contains("format=json"));
        assert!(net.asked()[0].url.contains("no_html=1"));
    }

    /// FAILS IF: the Brave key travels anywhere but its header, a missing key
    /// is asked anyway, or a refused key is not said as one.
    #[test]
    fn brave_takes_its_key_in_a_header_and_never_in_the_address() {
        let dir = ronoctua_consent::keys::Dir::at(
            std::env::temp_dir().join(format!("ronoctua-websearch-keys-{}", std::process::id())),
        );
        let _ = std::fs::remove_dir_all(dir.path());
        let pass = pass_for(BRAVE_HOST);
        let net = Canned::new(&[(
            "https://api.search.brave.com/res/v1/web/search",
            200,
            r#"{"web":{"results":[{"title":"Rust","url":"https://www.rust-lang.org","description":"<strong>Rust</strong> lang"}]}}"#,
        )]);
        assert_eq!(
            search_with(&net, Some(&dir), &Source::Brave, "rust", 5, &pass)
                .unwrap_err()
                .why,
            Why::NoKey,
            "no key kept"
        );
        assert!(net.asked().is_empty());
        dir.keep(BRAVE_KEY, &Secret::new("sekrit-key")).unwrap();
        let hits = search_with(&net, Some(&dir), &Source::Brave, "rust", 5, &pass).unwrap();
        assert_eq!(hits[0].snippet.as_deref(), Some("Rust lang"));
        let asked = net.asked();
        assert!(!asked[0].url.contains("sekrit"), "{}", asked[0].url);
        let (name, value) = asked[0].key.as_ref().expect("the key's header");
        assert_eq!(*name, "X-Subscription-Token");
        assert_eq!(value.expose(), "sekrit-key");
        assert!(
            !format!("{:?}", asked[0]).contains("sekrit"),
            "Debug hides it"
        );

        let refused = Canned::new(&[("https://api.search.brave.com/", 401, "{}")]);
        assert_eq!(
            search_with(&refused, Some(&dir), &Source::Brave, "rust", 5, &pass)
                .unwrap_err()
                .why,
            Why::KeyRefused
        );
        let _ = std::fs::remove_dir_all(dir.path());
    }

    #[test]
    fn searxng_asks_the_persons_server_for_json() {
        let source = Source::SearXNG {
            url: "https://search.example.org/sx/".to_owned(),
        };
        let net = Canned::new(&[(
            "https://search.example.org/sx/search?",
            200,
            r#"{"results":[{"title":"A","url":"https://a.example","content":"first"},{"title":"","url":"https://b.example"}]}"#,
        )]);
        let pass = pass_for("search.example.org");
        let hits = search_with(&net, None, &source, "olá mundo", 5, &pass).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[1].title, "https://b.example", "no title: the address");
        assert!(
            net.asked()[0].url.contains("q=ol%C3%A1%20mundo"),
            "{}",
            net.asked()[0].url
        );
    }

    #[test]
    fn a_failure_or_garbage_is_said_and_every_why_has_a_name() {
        let pass = pass_for(DUCKDUCKGO_HOST);
        for (status, body, why) in [
            (500, "{}", Why::Failed),
            (429, "{}", Why::Paced),
            (200, "<html>", Why::Garbled),
        ] {
            let net = Canned::new(&[("https://api.duckduckgo.com/", status, body)]);
            assert_eq!(
                search_with(&net, None, &Source::DuckDuckGo, "x", 5, &pass)
                    .unwrap_err()
                    .why,
                why,
                "{status} {body}"
            );
        }
        let ids: std::collections::BTreeSet<_> = Why::ALL.iter().map(|w| w.id()).collect();
        assert_eq!(ids.len(), Why::ALL.len());
    }

    #[test]
    fn the_kiosk_address_is_duckduckgos_own_page() {
        assert_eq!(
            kiosk_address(" rust & go "),
            "https://duckduckgo.com/?q=rust%20%26%20go"
        );
    }
}
