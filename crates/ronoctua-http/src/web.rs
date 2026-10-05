//! The web: one request to an `https` address, walked through its redirects by
//! one rule, with the body inflated when it came gzipped.
//!
//! # Why the rule lives here
//!
//! It was `ronoctua-software`'s (`net.rs`), written for the person's programs:
//! an icon, a manifest, a release asset behind a CDN. Then the player's
//! lookups needed the same walk — Cover Art Archive answers `307` to
//! archive.org, which answers `302` to a storage host; OpenSubtitles hands out
//! a download link on another host; a Commons poster is a `301` and then a
//! `302`. A second walk in `ronoctua-lookup` would have been two copies of the
//! rule that decides what a redirect may do, which is the copy `one-http-writer`
//! exists to refuse one layer up. So the RULE is here, beside the transport,
//! and each caller keeps only what is its own: its User-Agent, its ceiling,
//! and what a refusal means to it ([`Error::kind`] is a closed set it maps).
//!
//! # `https` only, with one exception that is not a downgrade
//!
//! The framer's only internet transport is TLS ([`crate::tls`]). Its TCP call
//! is for the loopback: it takes an address and not a name and gives no
//! ceiling. So an `http://` address is [`Kind::Scheme`] before anything is
//! sent — EXCEPT a first address whose host is one of [`UPGRADED_HOSTS`]:
//! hosts that answer every `http` request with a redirect to the same address
//! over `https`. Wikidata hands out its posters as
//! `http://commons.wikimedia.org/…`, and following the host's own redirect
//! would have meant one plain-text request first; asking the `https` address
//! directly is the same answer without it ([`upgraded`]). A REDIRECT to `http`
//! is still refused, whatever the host: a `302` is something anybody on the
//! path can write.
//!
//! # gzip
//!
//! A caller that asks ([`Coding::Gzip`]) says `Accept-Encoding: gzip`, and the
//! body is inflated here, bounded on the way OUT (a gzip bomb is a few
//! kilobytes by construction). iptv-org's four files are 11.7 MB plain and
//! 1.6 MB gzipped. A body that arrives gzipped UNASKED is inflated too — its
//! header says what it is, and handing the caller the raw bytes would put a
//! compressed body where it expects JSON — and every other coding is refused
//! ([`Kind::Coding`]) rather than passed on as if it were the body.
//!
//! # A request carries the person's consent
//!
//! Nothing in this distro asks a host before the person let it
//! (`ronoctua-consent`). [`request_let`] takes the proof — a
//! [`ronoctua_consent::Pass`], which only that crate mints — and refuses a
//! first address on any other host; its redirects follow the rule below, as
//! any request's do. [`request`], which takes none, is kept for the callers
//! that were asking before the rule existed (the person's programs, the
//! queue, the player's lookups and directories, which carry their own consent
//! or ask an address the person typed); `network-asks-with-a-pass` refuses a
//! new one.
//!
//! # Every function that sends blocks
//!
//! On the network, for up to [`crate::tls::PATIENCE`] per hop. Call it on a
//! THREAD, never on a drawing one — and hand it an [`Abort`] when a person
//! may be done with the answer before it comes: aborting shuts the socket,
//! and the blocked read ends at once.

use std::borrow::Cow;
use std::io::Read as _;
use std::time::Duration;

use url::Url;

use crate::{Call, Reply};

pub use crate::tls::Abort;

/// How many redirects a request may follow when its caller does not say.
///
/// Five: a login wall, a locale, a CDN and a trailing slash are four, and a
/// chain longer than that is a loop somebody else wrote. Cover Art Archive's
/// image is three requests across three hosts, so the floor is asserted.
pub const MOST_HOPS: usize = 5;

const _: () = assert!(
    MOST_HOPS >= 3,
    "Cover Art Archive → archive.org → a storage host is three requests"
);

/// Hosts whose `http` address is asked over `https` instead (see the module's
/// header): each answers plain `http` with a redirect to the same address over
/// `https`, so asking that directly is the same answer without a plain-text
/// request first.
pub const UPGRADED_HOSTS: [&str; 2] = ["commons.wikimedia.org", "coverartarchive.org"];

/// Names the framer writes itself, or this module writes on every request: a
/// caller's copy would be a second header of the same name.
pub const OWN_HEADERS: [&str; 6] = [
    "host",
    "connection",
    "content-type",
    "content-length",
    "user-agent",
    "accept-encoding",
];

/// Why a request was not answered, as a closed set.
///
/// No sentence here: which words a person sees is the caller's (the software
/// centre has its refusals, the player its troubles), and each maps these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Not an address, or one with no host or no known port.
    Invalid,
    /// A scheme this does not carry: anything but `https` as a first address
    /// (bar [`UPGRADED_HOSTS`]), or a redirect to anything but `https`.
    Scheme,
    /// A user name or a password in the address.
    Credentials,
    /// A header this will not send: one of [`OWN_HEADERS`], a name that is not
    /// a token, or a value with a control character (a line break would start
    /// a header nobody wrote).
    Header,
    /// The host could not be reached: a name that does not resolve, a refused
    /// or timed-out connect, a reset.
    Offline,
    /// TLS itself failed — a certificate that does not verify. Not "no
    /// network", and said apart from it.
    Certificate,
    /// More than the caller's ceiling, before or after inflating.
    TooLarge,
    /// More redirects than the caller allows.
    Loop,
    /// A content coding this does not read, or a gzip body that does not
    /// inflate.
    Coding,
    /// Anything else: not an HTTP response, a download stopped by its caller.
    Failed,
}

/// A request that was not answered: the kind, and the words for a log.
///
/// The trouble names the ADDRESS and never a header: a key travels in a
/// header, and a trouble is written to the journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// Which of the closed set.
    pub kind: Kind,
    /// What happened, for a log.
    pub trouble: String,
}

impl Error {
    /// An error of this kind, in these words.
    #[must_use]
    pub fn new(kind: Kind, trouble: impl Into<String>) -> Self {
        Self {
            kind,
            trouble: trouble.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.trouble)
    }
}

impl std::error::Error for Error {}

/// Which content coding a request accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coding {
    /// `Accept-Encoding: identity` — the body as it is. Without the header a
    /// server may send ANY coding (RFC 9110 §12.5.3).
    Identity,
    /// `Accept-Encoding: gzip`, inflated here.
    Gzip,
}

impl Coding {
    /// The header's value.
    #[must_use]
    pub const fn header(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Gzip => "gzip",
        }
    }
}

/// One request, whole, before it is sent.
///
/// A struct and not nine positional arguments. Its `Debug` names the address
/// WITHOUT its query, the header NAMES and the body's LENGTH, and nothing
/// else: a key travels in a header, in a query (SubDL documents no other
/// place for it) or in a login's body, and `Debug` is what a log line prints.
#[derive(Clone, Copy)]
pub struct Ask<'a> {
    /// The first address.
    pub url: &'a str,
    /// A JSON body to POST; `None` is a GET.
    pub body: Option<&'a str>,
    /// The caller's headers, after ours.
    pub headers: &'a [(&'a str, &'a str)],
    /// Who is asking, on every hop.
    pub user_agent: &'a str,
    /// The `Accept` header; `None` is `application/json` for a POST and `*/*`
    /// for a GET, decided per hop (a `303` turns a POST into a GET).
    pub accept: Option<&'a str>,
    /// Which coding the body may come in.
    pub coding: Coding,
    /// The most bytes of body, clamped to [`crate::tls::MOST_EVER`]; it bounds
    /// the inflated body as well as the one on the wire.
    pub most: u64,
    /// How long each hop may wait.
    pub patience: Duration,
    /// How many redirects may be followed.
    pub hops: usize,
    /// What stops it from another thread, when anything may ([`Abort`]).
    pub abort: Option<&'a Abort>,
}

impl<'a> Ask<'a> {
    /// A GET of `url` by `user_agent`: no headers, identity, a megabyte,
    /// [`crate::tls::PATIENCE`] and [`MOST_HOPS`].
    #[must_use]
    pub const fn get(url: &'a str, user_agent: &'a str) -> Self {
        Self {
            url,
            body: None,
            headers: &[],
            user_agent,
            accept: None,
            coding: Coding::Identity,
            most: crate::tls::MOST,
            patience: crate::tls::PATIENCE,
            hops: MOST_HOPS,
            abort: None,
        }
    }

    /// The same, stopped when `abort` is: its socket is shut, and the request
    /// answers [`Kind::Failed`].
    #[must_use]
    pub const fn abortable(mut self, abort: &'a Abort) -> Self {
        self.abort = Some(abort);
        self
    }

    /// The same, carrying these headers.
    #[must_use]
    pub const fn with_headers(mut self, headers: &'a [(&'a str, &'a str)]) -> Self {
        self.headers = headers;
        self
    }

    /// The same, POSTing this JSON body.
    #[must_use]
    pub const fn posting(mut self, body: &'a str) -> Self {
        self.body = Some(body);
        self
    }

    /// The same, accepting this.
    #[must_use]
    pub const fn accepting(mut self, accept: &'a str) -> Self {
        self.accept = Some(accept);
        self
    }

    /// The same, in this coding.
    #[must_use]
    pub const fn coded(mut self, coding: Coding) -> Self {
        self.coding = coding;
        self
    }

    /// The same, reading at most this many bytes of body.
    #[must_use]
    pub const fn most(mut self, most: u64) -> Self {
        self.most = most;
        self
    }

    /// The same, waiting this long per hop.
    #[must_use]
    pub const fn waiting(mut self, patience: Duration) -> Self {
        self.patience = patience;
        self
    }

    /// The same, following at most this many redirects.
    #[must_use]
    pub const fn hops(mut self, hops: usize) -> Self {
        self.hops = hops;
        self
    }
}

impl std::fmt::Debug for Ask<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(n, _)| *n).collect();
        f.debug_struct("Ask")
            .field("url", &without_query(self.url))
            .field("method", &if self.body.is_some() { "POST" } else { "GET" })
            .field("headers", &names)
            .field("body_bytes", &self.body.map(str::len))
            .field("coding", &self.coding)
            .field("most", &self.most)
            .field("hops", &self.hops)
            .field("abortable", &self.abort.is_some())
            .finish_non_exhaustive()
    }
}

/// `url` up to its query or fragment, with `?…` where a query was: the
/// address a log may print.
fn without_query(url: &str) -> Cow<'_, str> {
    match url.find(['?', '#']) {
        Some(cut) if url[cut..].starts_with('?') => Cow::Owned(format!("{}?…", &url[..cut])),
        Some(cut) => Cow::Borrowed(&url[..cut]),
        None => Cow::Borrowed(url),
    }
}

/// What a request returned, and where it ended up after redirects.
#[derive(Debug, Clone)]
pub struct Got {
    /// The final address.
    pub url: String,
    /// The last hop's reply, its body inflated when it came gzipped.
    pub reply: Reply,
}

/// THE request: `ask`'s first address ([`upgraded`] when its host is one of
/// [`UPGRADED_HOSTS`]), then every redirect [`next_hop`] allows, up to
/// `ask.hops` (301, 302 and 303 turn a POST into a GET; 307 and 308 carry its
/// body on). The LAST hop's reply is returned whatever its status — reading a
/// 404 or a 429 is the caller's. Blocks: call it on a thread.
///
/// A reply that is not a 2xx is returned even when its body is in a coding
/// this does not read: its STATUS is the answer, so the body is dropped
/// (never handed on coded) and the caller still hears the 404 or the 503 it
/// would have heard in plain text, not a coding failure.
///
/// # Errors
///
/// The address's refusal ([`transport_of`], [`next_hop`]), a header that is
/// not sent ([`headers`]), the transport's failure ([`kind_of_error`]),
/// [`Kind::Loop`] past `ask.hops`, and a 2xx body's coding ([`decoded`]).
pub fn request(ask: &Ask<'_>) -> Result<Got, Error> {
    let most = ask.most.min(crate::tls::MOST_EVER);
    walk(ask, &mut |target, call| {
        crate::tls::fetch_abortable(&target.host, target.port, call, most, ask.abort)
    })
}

/// [`request`], asking a host the person let: `pass` must be for the host of
/// the FIRST address ([`upgraded`] first, as the walk does). The redirects
/// after it follow [`next_hop`]'s rule — a pass lets the question, and where
/// the host sends it is the host's answer.
///
/// # What leaves the pass's host
///
/// Only the address the host itself named. The caller's headers (a key in
/// `Authorization`, most often) are sent to the pass's host and to no other,
/// and a body — which carries the person's text, and sometimes a key — is
/// never carried to another host: a 307 or 308 that would take it there is
/// refused ([`Kind::Invalid`]) rather than followed. The person let one
/// host; a redirect is not consent for the next.
///
/// # Errors
///
/// [`Kind::Invalid`] when the first address is not on the pass's host (or has
/// none) — nothing is sent — or when a redirect would carry the body to
/// another host; and every error of [`request`].
pub fn request_let(pass: &ronoctua_consent::Pass, ask: &Ask<'_>) -> Result<Got, Error> {
    let most = ask.most.min(crate::tls::MOST_EVER);
    request_let_over(pass, ask, &mut |target, call| {
        crate::tls::fetch_abortable(&target.host, target.port, call, most, ask.abort)
    })
}

/// A file a PAGE NAMED — a manifest, an icon — asked on the pass of the host
/// the page was read from (`page`), wherever the page put the file: the
/// page's answer, as a redirect is. The person let the page's host; nobody
/// could let the host of a file the page had not named yet, and a page that
/// keeps its icons on a CDN is still the page the person asked for.
///
/// # What leaves
///
/// Nothing of the caller's: the ask must carry no headers and no body, so
/// what reaches the named host is a bare GET of the address the page wrote,
/// our User-Agent and nothing else.
///
/// # Errors
///
/// [`Kind::Invalid`] when `page` is not on the pass's host, or the ask
/// carries headers or a body — nothing is sent; and every error of
/// [`request`].
pub fn request_named(
    pass: &ronoctua_consent::Pass,
    page: &str,
    ask: &Ask<'_>,
) -> Result<Got, Error> {
    let most = ask.most.min(crate::tls::MOST_EVER);
    request_named_over(pass, page, ask, &mut |target, call| {
        crate::tls::fetch_abortable(&target.host, target.port, call, most, ask.abort)
    })
}

/// [`request_let`], to a host under `domain` — the domain of the party the
/// pass's host belongs to.
///
/// For the hosts a party the person let names at run time under its own
/// domain: radio-browser's servers under `radio-browser.info` (the name its
/// consent row lets), the session and download hosts OpenSubtitles hands out
/// under `opensubtitles.com` (whose consent row lets
/// `api.opensubtitles.com`). A pass names one host, and nobody can let
/// `de2.api.radio-browser.info` before the directory says it exists; what
/// the person let is that party, and a host it names under its own domain
/// is its answer, as a redirect is. So BOTH must be under `domain`: the
/// pass's host (the party the person let is that domain's) and the first
/// address's. A host BESIDE the domain (`evil-radio-browser.info`) or above
/// it (`info`) is not under it, and a domain of one label is refused. `domain`
/// is the caller's constant, never a name an answer gave.
///
/// What leaves is what leaves [`request_let`]: the caller's headers and body
/// go to the first host alone.
///
/// # Errors
///
/// [`Kind::Invalid`] when the pass's host or the first address is not under
/// `domain` — nothing is sent — and every error of [`request_let`].
pub fn request_under(
    pass: &ronoctua_consent::Pass,
    domain: &str,
    ask: &Ask<'_>,
) -> Result<Got, Error> {
    let most = ask.most.min(crate::tls::MOST_EVER);
    request_under_over(pass, domain, ask, &mut |target, call| {
        crate::tls::fetch_abortable(&target.host, target.port, call, most, ask.abort)
    })
}

/// Whether `host` is `domain` or a name below it. Both are compared as the
/// web writes them (ASCII lower case, no trailing dot); an empty label never
/// counts. Pure.
#[must_use]
pub fn is_under(host: &str, domain: &str) -> bool {
    let fold = |h: &str| h.trim().trim_end_matches('.').to_ascii_lowercase();
    let (host, domain) = (fold(host), fold(domain));
    if domain.is_empty() || host.is_empty() {
        return false;
    }
    host == domain
        || host
            .strip_suffix(&domain)
            .and_then(|head| head.strip_suffix('.'))
            .is_some_and(|head| !head.split('.').any(str::is_empty))
}

/// The check of [`request_under`], before [`walk`], over any transport.
fn request_under_over(
    pass: &ronoctua_consent::Pass,
    domain: &str,
    ask: &Ask<'_>,
    send: &mut dyn FnMut(&Target, &Call<'_>) -> anyhow::Result<Reply>,
) -> Result<Got, Error> {
    let first = transport_of(&upgraded(ask.url))?;
    let host = first.host_str().unwrap_or_default();
    // A DOMAIN OF ONE LABEL (`info`) is a registry's, never a party's.
    let a_party = domain.trim_end_matches('.').contains('.');
    if !a_party || !is_under(pass.host(), domain) || !is_under(host, domain) {
        return Err(Error::new(
            Kind::Invalid,
            format!(
                "{}: the pass lets {} under {domain}, not this host",
                without_query(ask.url),
                pass.host()
            ),
        ));
    }
    walk_carrying(ask, Carry::ToItsHost, send)
}

/// A FILE sent to a host the person let, as a one-part
/// `multipart/form-data` POST — everything of an [`Ask`] but its body.
#[derive(Clone, Copy)]
pub struct Upload<'a> {
    /// The address, `https`, on the pass's host.
    pub url: &'a str,
    /// The caller's headers, after ours (a key in a header, most often).
    pub headers: &'a [(&'a str, &'a str)],
    /// Who is sending.
    pub user_agent: &'a str,
    /// The most bytes of the ANSWER.
    pub most: u64,
    /// How long the connection and each read and write may wait.
    pub patience: Duration,
    /// What stops it from another thread.
    pub abort: Option<&'a Abort>,
}

impl std::fmt::Debug for Upload<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(n, _)| *n).collect();
        f.debug_struct("Upload")
            .field("url", &without_query(self.url))
            .field("headers", &names)
            .field("most", &self.most)
            .field("abortable", &self.abort.is_some())
            .finish_non_exhaustive()
    }
}

/// Sends `sending`'s file to `upload.url`, a host `pass` lets, streamed from
/// its reader and never held ([`crate::multipart`]). ONE HOP: a redirect is
/// refused ([`Kind::Invalid`]) rather than followed, because following it
/// would send the file a second time, to an address the person never let.
/// The reply is returned whatever its status.
///
/// # Errors
///
/// [`Kind::Invalid`] when the address is not on the pass's host, or the host
/// answered with a redirect; a header that is not sent ([`headers`]); the
/// transport's failure, the file's ([`crate::multipart::Unsent`]) included.
pub fn upload_let(
    pass: &ronoctua_consent::Pass,
    upload: &Upload<'_>,
    sending: crate::multipart::Sending<'_>,
) -> Result<Got, Error> {
    let most = upload.most.min(crate::tls::MOST_EVER);
    upload_let_over(pass, upload, sending, &mut |target, call, sending| {
        crate::tls::upload_abortable(&target.host, target.port, call, sending, most, upload.abort)
    })
}

/// The check and the answer of [`upload_let`], over any transport.
fn upload_let_over(
    pass: &ronoctua_consent::Pass,
    upload: &Upload<'_>,
    sending: crate::multipart::Sending<'_>,
    send: &mut dyn FnMut(
        &Target,
        &Call<'_>,
        crate::multipart::Sending<'_>,
    ) -> anyhow::Result<Reply>,
) -> Result<Got, Error> {
    let at = transport_of(upload.url)?;
    let host = at.host_str().unwrap_or_default();
    if !pass.covers(host) {
        return Err(Error::new(
            Kind::Invalid,
            format!(
                "{}: the pass lets {}, not this host",
                without_query(upload.url),
                pass.host()
            ),
        ));
    }
    if upload.abort.is_some_and(Abort::is_aborted) {
        return Err(Error::new(
            Kind::Failed,
            format!("{}: aborted by its caller", without_query(upload.url)),
        ));
    }
    let target = Target::of(&at)?;
    let headers = headers(
        upload.user_agent,
        crate::JSON,
        Coding::Identity,
        upload.headers,
    )?;
    let call = Call::new("POST", &target.path, &target.host_header)
        .with_headers(&headers)
        .waiting(upload.patience);
    let reply = send(&target, &call, sending).map_err(|e| {
        Error::new(
            kind_of_error(&e),
            format!("{}: {e:#}", without_query(upload.url)),
        )
    })?;
    if redirect_of(&reply).is_some() {
        return Err(Error::new(
            Kind::Invalid,
            format!(
                "{} answered an upload with a redirect, which is not followed",
                without_query(upload.url)
            ),
        ));
    }
    let answered = (200..300).contains(&reply.status);
    let reply = match decoded(reply.clone(), upload.most.min(crate::tls::MOST_EVER)) {
        Ok(reply) => reply,
        Err(_) if !answered => Reply {
            status: reply.status,
            headers: without_coding(&reply.headers),
            body: Vec::new(),
        },
        Err(e) => return Err(e),
    };
    Ok(Got {
        url: at.to_string(),
        reply,
    })
}

/// [`request`] of an address a PERSON PASTED, which may be plain `http`: a
/// first address `http` or `https`, and redirects `http` to `http`, `http` to
/// `https` and `https` to `https` — never `https` down to `http`
/// ([`next_hop_plain`]). Each `http` hop crosses the network NOT ENCRYPTED,
/// over TCP; each `https` one over TLS, as [`request`]'s.
///
/// # Why, and for whom only
///
/// An IPTV provider's list is `http` more often than not, and the person's
/// account rides in its address. The owner's decision (2026-09-30): such a
/// list is taken — only as a source the person added themselves, with a
/// sentence beside it saying it is not encrypted — and never as one of this
/// system's own sources, every one of which is `https`. So this takes no
/// [`ronoctua_consent::Pass`] (the address is the person's own, and their
/// consent), and its one caller is the catalogue's fetch of a person's
/// source (`ronoctua-broadcast`, `Internet::get_own`).
///
/// # Where a hop in the clear may go
///
/// The host the person pasted is theirs to name, wherever it is — a box on
/// their own network may serve their list. Every OTHER host a redirect sends
/// the walk to in the clear must be on the public internet
/// ([`fence_of`], `tls::Fence::Public`): the list's server does not get to
/// make this machine ask its neighbours, with no certificate in the way.
///
/// # Errors
///
/// As [`request`], with an `http` first address and `http` hops allowed —
/// and a plain hop to another host refused when it resolves to this machine
/// or a network of its own.
pub fn request_unencrypted(ask: &Ask<'_>) -> Result<Got, Error> {
    let most = ask.most.min(crate::tls::MOST_EVER);
    let pasted = Target::of(&transport_plain_of(&upgraded(ask.url))?)?.host;
    walk_under(
        ask,
        Carry::Everywhere,
        Schemes::PlainToo,
        &mut |target, call| {
            if target.tls {
                crate::tls::fetch_abortable(&target.host, target.port, call, most, ask.abort)
            } else {
                crate::tls::fetch_plain_abortable(
                    &target.host,
                    target.port,
                    call,
                    most,
                    ask.abort,
                    fence_of(&target.host, &pasted),
                )
            }
        },
    )
}

/// Where a plain hop to `host` may go, in a walk that began at the host the
/// person pasted (`pasted`): anywhere for that one, the public internet for
/// every other. Pure.
#[must_use]
pub fn fence_of(host: &str, pasted: &str) -> crate::tls::Fence {
    if host.eq_ignore_ascii_case(pasted) {
        crate::tls::Fence::Open
    } else {
        crate::tls::Fence::Public
    }
}

/// Which schemes a walk carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Schemes {
    /// `https` alone — every walk but one.
    Tls,
    /// `http` too, for [`request_unencrypted`].
    PlainToo,
}

/// The check of [`request_let`], before [`walk`], over any transport.
fn request_let_over(
    pass: &ronoctua_consent::Pass,
    ask: &Ask<'_>,
    send: &mut dyn FnMut(&Target, &Call<'_>) -> anyhow::Result<Reply>,
) -> Result<Got, Error> {
    let first = transport_of(&upgraded(ask.url))?;
    let host = first.host_str().unwrap_or_default();
    if !pass.covers(host) {
        return Err(Error::new(
            Kind::Invalid,
            format!(
                "{}: the pass lets {}, not this host",
                without_query(ask.url),
                pass.host()
            ),
        ));
    }
    walk_carrying(ask, Carry::ToItsHost, send)
}

fn request_named_over(
    pass: &ronoctua_consent::Pass,
    page: &str,
    ask: &Ask<'_>,
    send: &mut dyn FnMut(&Target, &Call<'_>) -> anyhow::Result<Reply>,
) -> Result<Got, Error> {
    let from = transport_of(&upgraded(page))?;
    if !pass.covers(from.host_str().unwrap_or_default()) {
        return Err(Error::new(
            Kind::Invalid,
            format!(
                "{}: the pass lets {}, not the page that named {}",
                without_query(page),
                pass.host(),
                without_query(ask.url)
            ),
        ));
    }
    if !ask.headers.is_empty() || ask.body.is_some() {
        return Err(Error::new(
            Kind::Invalid,
            format!(
                "{}: a file a page named is asked bare",
                without_query(ask.url)
            ),
        ));
    }
    walk_carrying(ask, Carry::ToItsHost, send)
}

/// What a request carries past the host of its first address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Carry {
    /// Its headers and (after a 307 or 308) its body, to wherever the
    /// redirects go — the older callers' walk.
    Everywhere,
    /// Its headers and body to its own host only ([`request_let`]).
    ToItsHost,
}

/// The walk of [`request`], over any transport: `send` is TLS for the
/// machine and a list of answers for a test.
fn walk(
    ask: &Ask<'_>,
    send: &mut dyn FnMut(&Target, &Call<'_>) -> anyhow::Result<Reply>,
) -> Result<Got, Error> {
    walk_carrying(ask, Carry::Everywhere, send)
}

/// [`walk`], carrying the caller's headers and body as `carry` says.
fn walk_carrying(
    ask: &Ask<'_>,
    carry: Carry,
    send: &mut dyn FnMut(&Target, &Call<'_>) -> anyhow::Result<Reply>,
) -> Result<Got, Error> {
    walk_under(ask, carry, Schemes::Tls, send)
}

/// [`walk_carrying`], carrying the `schemes` given — the one walk every
/// request takes.
fn walk_under(
    ask: &Ask<'_>,
    carry: Carry,
    schemes: Schemes,
    send: &mut dyn FnMut(&Target, &Call<'_>) -> anyhow::Result<Reply>,
) -> Result<Got, Error> {
    let first = upgraded(ask.url);
    let mut at = match schemes {
        Schemes::Tls => transport_of(&first)?,
        Schemes::PlainToo => transport_plain_of(&first)?,
    };
    let origin = at.host().map(|h| h.to_owned());
    let mut body = ask.body;
    let stopped = |at: &Url| {
        Error::new(
            Kind::Failed,
            format!("{}: aborted by its caller", without_query(at.as_str())),
        )
    };
    for hop in 0..=ask.hops {
        if ask.abort.is_some_and(Abort::is_aborted) {
            return Err(stopped(&at));
        }
        let target = Target::of(&at)?;
        let accept = ask
            .accept
            .unwrap_or(if body.is_some() { crate::JSON } else { "*/*" });
        // Another host than the first gets none of the caller's headers when
        // they are the first host's alone.
        let home = at.host().map(|h| h.to_owned()) == origin;
        let extra = if home || carry == Carry::Everywhere {
            ask.headers
        } else {
            &[]
        };
        let headers = headers(ask.user_agent, accept, ask.coding, extra)?;
        let method = if body.is_some() { "POST" } else { "GET" };
        let mut call = Call::new(method, &target.path, &target.host_header)
            .with_headers(&headers)
            .waiting(ask.patience);
        if let Some(b) = body {
            call = call.with_body(b);
        }
        let sent = send(&target, &call);
        // AN ABORTED REQUEST IS ABORTED whatever its read said: a shut socket
        // reads as an end of stream, and what arrived before it is half an
        // answer, never one.
        if ask.abort.is_some_and(Abort::is_aborted) {
            return Err(stopped(&at));
        }
        let reply = sent.map_err(|e| Error::new(kind_of_error(&e), format!("{at}: {e:#}")))?;
        if let Some(location) = redirect_of(&reply) {
            if hop == ask.hops {
                break;
            }
            let next = match schemes {
                Schemes::Tls => next_hop(&at, location)?,
                Schemes::PlainToo => next_hop_plain(&at, location)?,
            };
            body = body.filter(|_| keeps_the_body(reply.status));
            let leaves = next.host().map(|h| h.to_owned()) != origin;
            if carry == Carry::ToItsHost && leaves && body.is_some() {
                return Err(Error::new(
                    Kind::Invalid,
                    format!(
                        "{} sent the request's body on to {}, which it is not carried to",
                        without_query(at.as_str()),
                        next.host_str().unwrap_or_default()
                    ),
                ));
            }
            at = next;
            continue;
        }
        let answered = (200..300).contains(&reply.status);
        let bare = (!answered).then(|| Reply {
            status: reply.status,
            headers: without_coding(&reply.headers),
            body: Vec::new(),
        });
        let reply = match decoded(reply, ask.most.min(crate::tls::MOST_EVER)) {
            Ok(reply) => reply,
            Err(e) => match bare {
                Some(bare) => bare,
                None => return Err(Error::new(e.kind, format!("{at}: {}", e.trouble))),
            },
        };
        return Ok(Got {
            url: at.to_string(),
            reply,
        });
    }
    Err(Error::new(
        Kind::Loop,
        format!("more than {} redirects from {}", ask.hops, ask.url),
    ))
}

/// The headers of one request: ours, then the caller's.
///
/// `User-Agent`, `Accept-Encoding`, and `Accept` unless the caller brings its
/// own; the caller may replace `Accept` and nothing else of ours or of the
/// framer's ([`OWN_HEADERS`]).
///
/// # Errors
///
/// [`Kind::Header`] for a name that is not a token, one of [`OWN_HEADERS`], or
/// a value (the User-Agent's included) with a control character in it — a line
/// break would start a header nobody wrote.
pub fn headers<'a>(
    user_agent: &'a str,
    accept: &'a str,
    coding: Coding,
    extra: &[(&'a str, &'a str)],
) -> Result<Vec<(&'a str, &'a str)>, Error> {
    if user_agent.chars().any(char::is_control) || accept.chars().any(char::is_control) {
        return Err(Error::new(Kind::Header, "our own header has a line break"));
    }
    for (name, value) in extra {
        let token =
            !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        let own = OWN_HEADERS.iter().any(|o| o.eq_ignore_ascii_case(name));
        if !token || own || value.chars().any(char::is_control) {
            return Err(Error::new(
                Kind::Header,
                format!("the header `{name}` is not sent"),
            ));
        }
    }
    let replaced = extra.iter().any(|(n, _)| n.eq_ignore_ascii_case("accept"));
    let mut all = vec![
        ("User-Agent", user_agent),
        ("Accept-Encoding", coding.header()),
    ];
    if !replaced {
        all.push(("Accept", accept));
    }
    all.extend_from_slice(extra);
    Ok(all)
}

/// `url`, over `https`, when it is an `http` address of one of
/// [`UPGRADED_HOSTS`] on the default port; otherwise `url` itself. Pure.
#[must_use]
pub fn upgraded(url: &str) -> Cow<'_, str> {
    let Ok(mut parsed) = Url::parse(url.trim()) else {
        return Cow::Borrowed(url);
    };
    let known = parsed
        .host_str()
        .is_some_and(|h| UPGRADED_HOSTS.iter().any(|k| h.eq_ignore_ascii_case(k)));
    if parsed.scheme() != "http" || !known || parsed.port().is_some() {
        return Cow::Borrowed(url);
    }
    if parsed.set_scheme("https").is_err() {
        return Cow::Borrowed(url);
    }
    Cow::Owned(parsed.to_string())
}

/// An address parsed and checked against what this module can carry: `https`
/// only (see the module's header), a host, and no user name or password in it.
///
/// Strict: [`upgraded`] is a separate step, so a caller that JUDGES an address
/// (is this asset's URL one we would fetch?) sees exactly what was written.
///
/// # Errors
///
/// [`Kind::Invalid`] for what does not parse or has no host, [`Kind::Scheme`]
/// for anything but `https`, [`Kind::Credentials`] for a user name or a
/// password.
pub fn transport_of(url: &str) -> Result<Url, Error> {
    let parsed = transport_plain_of(url)?;
    if parsed.scheme() != "https" {
        return Err(Error::new(
            Kind::Scheme,
            format!("{url}: only https is fetched"),
        ));
    }
    Ok(parsed)
}

/// [`transport_of`], `http` taken too — for [`request_unencrypted`] alone.
///
/// # Errors
///
/// [`Kind::Invalid`] for what does not parse or has no host, [`Kind::Scheme`]
/// for anything but `http` and `https`, [`Kind::Credentials`] for a user name
/// or a password.
pub fn transport_plain_of(url: &str) -> Result<Url, Error> {
    let parsed =
        Url::parse(url.trim()).map_err(|e| Error::new(Kind::Invalid, format!("{url}: {e}")))?;
    if !matches!(parsed.scheme(), "https" | "http") {
        return Err(Error::new(
            Kind::Scheme,
            format!("{url}: only http and https are fetched"),
        ));
    }
    if parsed.host().is_none() {
        return Err(Error::new(Kind::Invalid, format!("{url}: no host")));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(Error::new(
            Kind::Credentials,
            format!("{url}: credentials in the address"),
        ));
    }
    Ok(parsed)
}

/// Where a redirect from `from` to `location` goes, or why it may not.
///
/// `location` is resolved against `from` (`Url::join`, which is what a
/// relative `Location: /x` needs). Allowed: `https` to `https`, and `http` to
/// `https`. Refused with [`Kind::Scheme`]: `https` to `http` (a downgrade
/// anybody on the path can ask for) and every other scheme (`file:`, `data:`,
/// `javascript:`); with [`Kind::Credentials`], a target that carries a user
/// name or a password. Pure.
///
/// # Errors
///
/// [`Kind::Scheme`], [`Kind::Credentials`] or, for a `Location` that does not
/// parse, [`Kind::Invalid`].
pub fn next_hop(from: &Url, location: &str) -> Result<Url, Error> {
    let next = next_hop_plain(from, location)?;
    // The framer carries https only; an http→http hop is allowed by the rule
    // and refused by the transport, the same way a first http address is.
    if next.scheme() != "https" {
        return Err(Error::new(
            Kind::Scheme,
            format!("{from} redirected to {next}: only https is fetched"),
        ));
    }
    Ok(next)
}

/// [`next_hop`]'s rule without the transport's `https`: `http` to `http`
/// taken too — for [`request_unencrypted`], whose hops may be plain. `https`
/// to `http` is still refused: a walk that began encrypted never goes down.
/// Pure.
///
/// # Errors
///
/// As [`next_hop`], `http` to `http` taken.
pub fn next_hop_plain(from: &Url, location: &str) -> Result<Url, Error> {
    let next = from.join(location.trim()).map_err(|e| {
        Error::new(
            Kind::Invalid,
            format!("{from} redirected to `{location}`: {e}"),
        )
    })?;
    let allowed = matches!(
        (from.scheme(), next.scheme()),
        ("https", "https") | ("http", "https" | "http")
    );
    if !allowed || next.host().is_none() {
        return Err(Error::new(
            Kind::Scheme,
            format!("{from} redirected to {next}, which is not followed"),
        ));
    }
    if !next.username().is_empty() || next.password().is_some() {
        return Err(Error::new(
            Kind::Credentials,
            format!("{from} redirected to an address with credentials in it"),
        ));
    }
    Ok(next)
}

/// The `Location` of a redirect, when the reply is one. Pure.
#[must_use]
pub fn redirect_of(reply: &Reply) -> Option<&str> {
    matches!(reply.status, 301 | 302 | 303 | 307 | 308)
        .then(|| reply.header("location"))
        .flatten()
        .filter(|l| !l.trim().is_empty())
}

/// Whether a redirect with this status carries a POST's body on.
///
/// 307 and 308 say "the same request, elsewhere"; 301, 302 and 303 become a
/// GET, which is what every browser does and what a server that answers a
/// POST with 303 means.
#[must_use]
pub const fn keeps_the_body(status: u16) -> bool {
    matches!(status, 307 | 308)
}

/// Which [`Kind`] a transport error is.
///
/// The transport answers `anyhow`, so this reads the chain: an I/O error that
/// is not `InvalidData` is the network not being there (a name that does not
/// resolve, a refused or timed-out connect, a reset) — [`Kind::Offline`];
/// `InvalidData` is TLS itself failing — [`Kind::Certificate`]. A body over the
/// ceiling has no I/O error in it and the transport's own words (`more than …
/// bytes`): [`Kind::TooLarge`]. A download its listener stopped is
/// [`Kind::Failed`]. Pure.
#[must_use]
pub fn kind_of_error(e: &anyhow::Error) -> Kind {
    let said = format!("{e:#}");
    if let Some(io) = e.chain().find_map(|c| c.downcast_ref::<std::io::Error>()) {
        return if io.kind() == std::io::ErrorKind::InvalidData {
            Kind::Certificate
        } else {
            Kind::Offline
        };
    }
    if said.contains(" more than ") {
        Kind::TooLarge
    } else if said.contains("is not a server name") {
        Kind::Invalid
    } else if said.contains("resolved to nothing") || said.contains("could not reach") {
        Kind::Offline
    } else {
        Kind::Failed
    }
}

/// The reply with its body in no content coding: a gzipped body inflated
/// (bounded by `most` on the way out) and the two headers that described the
/// wire copy dropped, so nobody inflates it twice. An empty body is left alone
/// (a `304` or a `HEAD` says `gzip` about nothing).
///
/// # Errors
///
/// [`Kind::Coding`] for any other coding, or a gzip body that does not
/// inflate; [`Kind::TooLarge`] when it inflates past `most`.
pub fn decoded(mut reply: Reply, most: u64) -> Result<Reply, Error> {
    let Some(coding) = reply
        .header("content-encoding")
        .map(|v| v.trim().to_ascii_lowercase())
    else {
        return Ok(reply);
    };
    match coding.as_str() {
        "" | "identity" => {}
        "gzip" | "x-gzip" => {
            if !reply.body.is_empty() {
                reply.body = inflate(&reply.body, most)?;
            }
        }
        other => {
            return Err(Error::new(
                Kind::Coding,
                format!("a body coded `{other}` is not read"),
            ));
        }
    }
    reply.headers = without_coding(&reply.headers);
    Ok(reply)
}

/// `headers` without the two that describe the body on the wire: once the
/// body is decoded (or dropped), they describe nothing.
fn without_coding(headers: &[(String, String)]) -> Vec<(String, String)> {
    headers
        .iter()
        .filter(|(n, _)| {
            !n.eq_ignore_ascii_case("content-encoding") && !n.eq_ignore_ascii_case("content-length")
        })
        .cloned()
        .collect()
}

/// A gzip body, inflated, and never past `most` bytes: the bound is on what
/// comes OUT, while inflating, because that is the only number a gzip bomb
/// cannot make small. Several members, one after the other, are one body
/// (RFC 1952 allows it).
///
/// # Errors
///
/// [`Kind::Coding`] when it does not inflate, [`Kind::TooLarge`] past `most`.
pub fn inflate(gzipped: &[u8], most: u64) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    flate2::read::MultiGzDecoder::new(gzipped)
        .take(most.saturating_add(1))
        .read_to_end(&mut out)
        .map_err(|e| Error::new(Kind::Coding, format!("the gzip body does not inflate: {e}")))?;
    if out.len() as u64 > most {
        return Err(Error::new(
            Kind::TooLarge,
            format!("the body inflates to more than {most} bytes"),
        ));
    }
    Ok(out)
}

/// What one request needs from an address: the name TLS checks, the port,
/// the `Host` header, the path, and whether it goes over TLS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Over TLS (`https`); plain TCP (`http`) only in a walk that carries it
    /// ([`request_unencrypted`]).
    pub tls: bool,
    /// The name the certificate must carry (an IPv6 address without brackets).
    pub host: String,
    /// The port, the scheme's default when the address names none.
    pub port: u16,
    /// The `Host` header: brackets around IPv6, the port only when it is not
    /// the default (some servers refuse `example.org:443`).
    pub host_header: String,
    /// The path and the query; never the fragment, which is not sent.
    pub path: String,
}

impl Target {
    /// The target of an address. Pure.
    ///
    /// # Errors
    ///
    /// [`Kind::Invalid`] for an address with no host or no known port.
    pub fn of(url: &Url) -> Result<Self, Error> {
        let host = match url.host() {
            Some(url::Host::Domain(d)) => d.to_owned(),
            Some(url::Host::Ipv4(a)) => a.to_string(),
            Some(url::Host::Ipv6(a)) => a.to_string(),
            None => return Err(Error::new(Kind::Invalid, format!("{url}: no host"))),
        };
        let port = url
            .port_or_known_default()
            .ok_or_else(|| Error::new(Kind::Invalid, format!("{url}: no port")))?;
        let shown = url.host_str().unwrap_or_default();
        let host_header = url
            .port()
            .map_or_else(|| shown.to_owned(), |p| format!("{shown}:{p}"));
        let path = url[url::Position::BeforePath..url::Position::AfterQuery].to_owned();
        Ok(Self {
            tls: url.scheme() == "https",
            host,
            port,
            host_header,
            path: if path.is_empty() { "/".into() } else { path },
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    fn reply(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Reply {
        Reply {
            status,
            headers: headers
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
            body: body.to_vec(),
        }
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(bytes).unwrap();
        e.finish().unwrap()
    }

    /// One hop as the fake transport saw it.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Sent {
        host: String,
        request: String,
    }

    /// Walks `ask` over canned replies, answered in order, and says what
    /// every hop sent.
    fn walked(ask: &Ask<'_>, replies: Vec<Reply>) -> (Result<Got, Error>, Vec<Sent>) {
        let mut replies = replies.into_iter();
        let mut sent = Vec::new();
        let got = walk(ask, &mut |target, call| {
            sent.push(Sent {
                host: target.host.clone(),
                request: crate::request_of(call),
            });
            replies
                .next()
                .ok_or_else(|| anyhow::anyhow!("no status line"))
        });
        (got, sent)
    }

    /// Walks `ask` as [`request_unencrypted`] does, over canned replies: what
    /// every hop sent, and whether it went over TLS.
    fn walked_plain(
        ask: &Ask<'_>,
        replies: Vec<Reply>,
    ) -> (Result<Got, Error>, Vec<(String, bool)>) {
        let mut replies = replies.into_iter();
        let mut sent = Vec::new();
        let got = walk_under(
            ask,
            Carry::Everywhere,
            Schemes::PlainToo,
            &mut |target, _call| {
                sent.push((target.host.clone(), target.tls));
                replies
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("no status line"))
            },
        );
        (got, sent)
    }

    /// FAILS IF: a person's `http` list is refused again — or the plain walk
    /// lets a hop go down from `https` to `http`, lets any other scheme in,
    /// or sends an `https` hop without TLS; or the ordinary walk starts taking
    /// `http` too.
    #[test]
    fn a_pasted_http_list_is_fetched_in_the_clear_and_never_downgraded() {
        let ask = Ask::get("http://iptv.example:8080/get.php?u=a&p=b", "t");
        let (got, sent) = walked_plain(&ask, vec![reply(200, &[], b"#EXTM3U\n")]);
        assert_eq!(got.unwrap().reply.body, b"#EXTM3U\n");
        assert_eq!(
            sent,
            [("iptv.example".to_owned(), false)],
            "plain, port kept"
        );

        // UP to https is taken, and goes over TLS.
        let (got, sent) = walked_plain(
            &ask,
            vec![
                reply(302, &[("location", "https://cdn.example/l.m3u")], b""),
                reply(200, &[], b"ok"),
            ],
        );
        assert!(got.is_ok());
        assert_eq!(
            sent,
            [
                ("iptv.example".to_owned(), false),
                ("cdn.example".to_owned(), true)
            ]
        );
        // PLAIN to plain is taken; DOWN from https is not.
        assert_eq!(
            next_hop_plain(&url("http://a.example/x"), "http://b.example/y")
                .unwrap()
                .as_str(),
            "http://b.example/y"
        );
        assert_eq!(
            next_hop_plain(&url("https://a.example/x"), "http://b.example/y")
                .unwrap_err()
                .kind,
            Kind::Scheme
        );
        let secure = Ask::get("https://iptv.example/l.m3u", "t");
        let (got, sent) = walked_plain(
            &secure,
            vec![reply(
                301,
                &[("location", "http://iptv.example/l.m3u")],
                b"",
            )],
        );
        assert_eq!(got.unwrap_err().kind, Kind::Scheme);
        assert_eq!(sent.len(), 1, "the downgrade is never asked");
        // NO OTHER SCHEME, and no account in the address itself.
        for (bad, kind) in [
            ("ftp://iptv.example/l.m3u", Kind::Scheme),
            ("file:///etc/passwd", Kind::Scheme),
            ("http://me:pw@iptv.example/l.m3u", Kind::Credentials),
        ] {
            assert_eq!(transport_plain_of(bad).unwrap_err().kind, kind, "{bad}");
        }
        // AND THE ORDINARY WALK is as strict as it was.
        assert_eq!(
            transport_of("http://iptv.example/l.m3u").unwrap_err().kind,
            Kind::Scheme
        );
    }

    /// FAILS IF: a hop in the clear to another host than the one the person
    /// pasted may reach this machine or a network of its own — a list's
    /// server could make the daemon ask a router, a printer or a service on
    /// `localhost` — or the pasted host itself is fenced (a box on the
    /// person's own network serving their list), or a private address is
    /// taken for a public one.
    #[test]
    fn a_hop_in_the_clear_reaches_only_the_public_internet_but_the_pasted_host() {
        use crate::tls::{Fence, is_public};
        assert_eq!(fence_of("iptv.example", "iptv.example"), Fence::Open);
        assert_eq!(fence_of("IPTV.example", "iptv.example"), Fence::Open);
        assert_eq!(fence_of("cdn.example", "iptv.example"), Fence::Public);
        assert_eq!(fence_of("192.168.1.10", "192.168.1.10"), Fence::Open);
        for private in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.5.4",
            "192.168.1.1",
            "169.254.169.254",
            "100.100.100.100",
            "0.0.0.0",
            "255.255.255.255",
            "224.0.0.1",
            "::1",
            "::",
            "fe80::1",
            "fd7a:115c:a1e0::1",
            "::ffff:192.168.1.1",
        ] {
            let ip: std::net::IpAddr = private.parse().unwrap();
            assert!(!is_public(ip), "{private} taken for the public internet");
        }
        for public in ["93.184.216.34", "1.1.1.1", "2606:4700:4700::1111"] {
            let ip: std::net::IpAddr = public.parse().unwrap();
            assert!(is_public(ip), "{public} fenced off");
        }
    }

    #[test]
    fn a_redirect_goes_up_to_https_and_never_down() {
        let secure = url("https://example.org/a/b");
        assert_eq!(
            next_hop(&secure, "c").unwrap().as_str(),
            "https://example.org/a/c"
        );
        assert_eq!(
            next_hop(&secure, "/x?y=1").unwrap().as_str(),
            "https://example.org/x?y=1"
        );
        assert_eq!(
            next_hop(&secure, "//cdn.example.net/i.png")
                .unwrap()
                .scheme(),
            "https"
        );
        // THE DOWNGRADE: anybody on the path can answer a 302 to http — and
        // that is true of a known host too; only a FIRST address is upgraded.
        for down in ["http://example.org/", "http://commons.wikimedia.org/x"] {
            assert_eq!(next_hop(&secure, down).unwrap_err().kind, Kind::Scheme);
        }
        for bad in [
            "file:///etc/passwd",
            "data:text/html,x",
            "javascript:alert(1)",
            "ftp://example.org/",
        ] {
            assert_eq!(
                next_hop(&secure, bad).unwrap_err().kind,
                Kind::Scheme,
                "{bad}"
            );
        }
        assert_eq!(
            next_hop(&secure, "https://me:pw@example.org/")
                .unwrap_err()
                .kind,
            Kind::Credentials
        );
        assert_eq!(
            next_hop(&url("http://example.org/"), "https://example.org/")
                .unwrap()
                .scheme(),
            "https"
        );
    }

    #[test]
    fn only_https_with_a_host_and_no_credentials_is_fetched() {
        assert!(transport_of("https://example.org/").is_ok());
        assert_eq!(
            transport_of("http://example.org/").unwrap_err().kind,
            Kind::Scheme
        );
        // STRICT: judging an address is not fetching it.
        assert_eq!(
            transport_of("http://commons.wikimedia.org/x")
                .unwrap_err()
                .kind,
            Kind::Scheme
        );
        assert_eq!(
            transport_of("https://me:pw@example.org/").unwrap_err().kind,
            Kind::Credentials
        );
        assert_eq!(
            transport_of("file:///etc/passwd").unwrap_err().kind,
            Kind::Scheme
        );
        assert_eq!(
            transport_of("not an address").unwrap_err().kind,
            Kind::Invalid
        );
    }

    #[test]
    fn only_a_known_hosts_http_address_is_upgraded() {
        assert_eq!(
            upgraded("http://commons.wikimedia.org/wiki/Special:FilePath/A.jpg"),
            "https://commons.wikimedia.org/wiki/Special:FilePath/A.jpg"
        );
        assert_eq!(
            upgraded("http://coverartarchive.org/release/x/1.jpg"),
            "https://coverartarchive.org/release/x/1.jpg"
        );
        // Anybody else's http stays http, and is then refused.
        assert_eq!(upgraded("http://example.org/"), "http://example.org/");
        // A host that merely ENDS like a known one is somebody else.
        assert_eq!(
            upgraded("http://evil-commons.wikimedia.org.example/"),
            "http://evil-commons.wikimedia.org.example/"
        );
        // An explicit port is not the host's redirect any more.
        assert_eq!(
            upgraded("http://commons.wikimedia.org:8080/x"),
            "http://commons.wikimedia.org:8080/x"
        );
        assert!(matches!(
            upgraded("https://commons.wikimedia.org/x"),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn a_redirect_is_a_redirect_only_with_somewhere_to_go() {
        assert_eq!(
            redirect_of(&reply(302, &[("Location", "/x")], b"")),
            Some("/x")
        );
        assert_eq!(redirect_of(&reply(302, &[], b"")), None);
        assert_eq!(redirect_of(&reply(304, &[("Location", "/x")], b"")), None);
        assert_eq!(redirect_of(&reply(200, &[("Location", "/x")], b"")), None);
        assert!(keeps_the_body(307) && keeps_the_body(308));
        assert!(!keeps_the_body(302) && !keeps_the_body(303));
    }

    #[test]
    fn three_hops_across_three_hosts_are_walked() {
        // COVER ART ARCHIVE'S SHAPE: its own 307, archive.org's 302, then the
        // storage host's answer.
        let ask = Ask::get("https://coverartarchive.org/release/r", "ua/1");
        let (got, sent) = walked(
            &ask,
            vec![
                reply(
                    307,
                    &[("Location", "https://archive.org/download/r/index.json")],
                    b"",
                ),
                reply(
                    302,
                    &[("Location", "https://ia800.us.archive.org/i.json")],
                    b"",
                ),
                reply(200, &[], b"{}"),
            ],
        );
        let got = got.unwrap();
        assert_eq!(got.url, "https://ia800.us.archive.org/i.json");
        assert_eq!(got.reply.body, b"{}");
        let hosts: Vec<&str> = sent.iter().map(|s| s.host.as_str()).collect();
        assert_eq!(
            hosts,
            ["coverartarchive.org", "archive.org", "ia800.us.archive.org"]
        );
        assert!(
            sent.iter()
                .all(|s| s.request.contains("User-Agent: ua/1\r\n")),
            "every hop says who is asking"
        );
    }

    #[test]
    fn a_first_http_address_of_a_known_host_goes_out_over_https() {
        let ask = Ask::get(
            "http://commons.wikimedia.org/wiki/Special:FilePath/A.jpg",
            "ua",
        );
        let (got, sent) = walked(&ask, vec![reply(200, &[], b"img")]);
        assert_eq!(
            got.unwrap().url,
            "https://commons.wikimedia.org/wiki/Special:FilePath/A.jpg"
        );
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].host, "commons.wikimedia.org");
        // And nobody else's.
        let (got, sent) = walked(&Ask::get("http://example.org/", "ua"), vec![]);
        assert_eq!(got.unwrap_err().kind, Kind::Scheme);
        assert!(sent.is_empty(), "nothing was sent for a refused address");
    }

    #[test]
    fn past_the_ceiling_of_hops_is_a_loop_and_a_downgrade_is_never_followed() {
        let again = || reply(302, &[("Location", "/again")], b"");
        let ask = Ask::get("https://example.org/", "ua").hops(3);
        let (got, sent) = walked(&ask, vec![again(), again(), again(), again(), again()]);
        assert_eq!(got.unwrap_err().kind, Kind::Loop);
        assert_eq!(sent.len(), 4, "the first request and three redirects");

        let (got, _) = walked(
            &Ask::get("https://example.org/", "ua"),
            vec![reply(301, &[("Location", "http://example.org/")], b"")],
        );
        assert_eq!(got.unwrap_err().kind, Kind::Scheme);
    }

    #[test]
    fn a_post_becomes_a_get_on_303_and_stays_a_post_on_307() {
        let ask = Ask::get("https://api.example/login", "ua").posting(r#"{"a":1}"#);
        let (got, sent) = walked(
            &ask,
            vec![
                reply(307, &[("Location", "/login2")], b""),
                reply(303, &[("Location", "/done")], b""),
                reply(200, &[], b"ok"),
            ],
        );
        assert!(got.is_ok());
        assert!(sent[0].request.starts_with("POST /login HTTP/1.1"));
        assert!(sent[1].request.starts_with("POST /login2 HTTP/1.1"));
        assert!(
            sent[1].request.ends_with(r#"{"a":1}"#),
            "307 carries the body"
        );
        assert!(sent[2].request.starts_with("GET /done HTTP/1.1"));
        // The Accept follows the method, hop by hop.
        assert!(sent[1].request.contains("Accept: application/json\r\n"));
        assert!(sent[2].request.contains("Accept: */*\r\n"));
    }

    #[test]
    fn a_gzipped_answer_is_inflated_when_asked_and_when_not() {
        let json = br#"{"channels":[1,2,3]}"#;
        let ask =
            Ask::get("https://iptv-org.github.io/api/channels.json", "ua").coded(Coding::Gzip);
        let (got, sent) = walked(
            &ask,
            vec![reply(
                200,
                &[("Content-Encoding", "gzip"), ("Content-Length", "999")],
                &gzip(json),
            )],
        );
        let got = got.unwrap();
        assert_eq!(got.reply.body, json);
        assert_eq!(got.reply.header("content-encoding"), None, "never twice");
        assert_eq!(got.reply.header("content-length"), None);
        assert!(sent[0].request.contains("Accept-Encoding: gzip\r\n"));

        // UNASKED: the header says what the body is, so it is read as that.
        let (got, sent) = walked(
            &Ask::get("https://example.org/", "ua"),
            vec![reply(200, &[("Content-Encoding", "gzip")], &gzip(b"plain"))],
        );
        assert_eq!(got.unwrap().reply.body, b"plain");
        assert!(sent[0].request.contains("Accept-Encoding: identity\r\n"));
    }

    #[test]
    fn a_body_that_inflates_past_the_ceiling_is_refused_on_the_way_out() {
        let bomb = gzip(&vec![0u8; 1 << 20]);
        assert!(bomb.len() < 4096, "the point of a bomb is that it is small");
        assert_eq!(inflate(&bomb, 1 << 16).unwrap_err().kind, Kind::TooLarge);
        assert_eq!(inflate(&bomb, 1 << 20).unwrap().len(), 1 << 20);
        assert_eq!(inflate(b"not gzip", 100).unwrap_err().kind, Kind::Coding);
        // Two members are one body.
        let mut two = gzip(b"ab");
        two.extend(gzip(b"cd"));
        assert_eq!(inflate(&two, 10).unwrap(), b"abcd");
    }

    #[test]
    fn a_coding_this_does_not_read_is_refused_and_not_passed_on() {
        for coding in ["br", "zstd", "deflate", "gzip, br"] {
            let r = reply(200, &[("Content-Encoding", coding)], b"xx");
            assert_eq!(decoded(r, 100).unwrap_err().kind, Kind::Coding, "{coding}");
        }
        assert_eq!(
            decoded(reply(200, &[("Content-Encoding", "identity")], b"x"), 1)
                .unwrap()
                .body,
            b"x"
        );
        // Nothing to inflate.
        assert!(decoded(reply(304, &[("Content-Encoding", "gzip")], b""), 1).is_ok());
    }

    /// FAILS IF: a refusal whose body is in a coding this does not read
    /// stops being its status and turns into a coding failure.
    #[test]
    fn a_refusal_in_a_coding_this_does_not_read_is_still_its_status() {
        for status in [404u16, 429, 503] {
            let (got, _) = walked(
                &Ask::get("https://example.org/", "ua"),
                vec![reply(
                    status,
                    &[
                        ("Content-Encoding", "br"),
                        ("Content-Length", "2"),
                        ("Retry-After", "7"),
                    ],
                    b"xx",
                )],
            );
            let got = got.expect("the status is the answer");
            assert_eq!(got.reply.status, status);
            assert!(got.reply.body.is_empty(), "never handed on coded");
            assert_eq!(got.reply.header("content-encoding"), None);
            assert_eq!(got.reply.header("retry-after"), Some("7"));
        }
        // A 404 that came gzipped is still read, like any body.
        let (got, _) = walked(
            &Ask::get("https://example.org/", "ua"),
            vec![reply(404, &[("Content-Encoding", "gzip")], &gzip(b"gone"))],
        );
        assert_eq!(got.unwrap().reply.body, b"gone");
        // And a 2xx in such a coding is still refused.
        let (got, _) = walked(
            &Ask::get("https://example.org/", "ua"),
            vec![reply(200, &[("Content-Encoding", "br")], b"xx")],
        );
        assert_eq!(got.unwrap_err().kind, Kind::Coding);
    }

    #[test]
    fn a_caller_adds_headers_and_replaces_only_accept() {
        let h = headers(
            "ua",
            "*/*",
            Coding::Identity,
            &[("Accept", "application/vnd.github+json"), ("Api-Key", "k")],
        )
        .unwrap();
        let accepts: Vec<&str> = h
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case("accept"))
            .map(|(_, v)| *v)
            .collect();
        assert_eq!(accepts, ["application/vnd.github+json"]);
        assert!(h.contains(&("User-Agent", "ua")));
        assert!(h.contains(&("Accept-Encoding", "identity")));
        assert!(h.contains(&("Api-Key", "k")));
        for bad in [
            ("user-agent", "x"),
            ("Accept-Encoding", "gzip"),
            ("Host", "evil.example"),
            ("Content-Length", "0"),
            ("X-A", "b\r\nX-Injected: 1"),
            ("X A", "b"),
            ("", "b"),
        ] {
            assert_eq!(
                headers("ua", "*/*", Coding::Identity, &[bad])
                    .unwrap_err()
                    .kind,
                Kind::Header,
                "{bad:?}"
            );
        }
        assert_eq!(
            headers("ua\r\nX: y", "*/*", Coding::Gzip, &[])
                .unwrap_err()
                .kind,
            Kind::Header
        );
    }

    #[test]
    fn an_asks_debug_names_its_headers_and_never_their_values() {
        let ask = Ask::get("https://api.example/login", "ua")
            .with_headers(&[("Api-Key", "the-secret-key")])
            .posting(r#"{"password":"hunter2"}"#);
        let shown = format!("{ask:?}");
        assert!(shown.contains("Api-Key"), "{shown}");
        assert!(!shown.contains("the-secret-key"), "{shown}");
        assert!(!shown.contains("hunter2"), "{shown}");
        // SubDL's key travels in the query, so the query is never shown.
        let keyed = Ask::get(
            "https://api.subdl.com/api/v1/subtitles?film_name=x&api_key=subdl-key-123#top",
            "ua",
        );
        let shown = format!("{keyed:?}");
        assert!(
            shown.contains("https://api.subdl.com/api/v1/subtitles?…"),
            "{shown}"
        );
        assert!(!shown.contains("subdl-key-123"), "{shown}");
        assert!(!shown.contains("film_name"), "{shown}");
        assert!(
            format!("{:?}", Ask::get("https://a.example/p#frag", "ua"))
                .contains("\"https://a.example/p\"")
        );
    }

    #[test]
    fn a_transport_error_says_which_kind_it_is() {
        use std::io::{Error as Io, ErrorKind};
        let unreachable = anyhow::Error::new(Io::new(ErrorKind::ConnectionRefused, "refused"))
            .context("could not reach h:443");
        assert_eq!(kind_of_error(&unreachable), Kind::Offline);
        let tls = anyhow::Error::new(Io::new(ErrorKind::InvalidData, "invalid peer certificate"))
            .context("could not send the request to h");
        assert_eq!(kind_of_error(&tls), Kind::Certificate);
        assert_eq!(
            kind_of_error(&anyhow::anyhow!("h answered more than 4096 bytes")),
            Kind::TooLarge
        );
        assert_eq!(
            kind_of_error(&anyhow::anyhow!("`a b` is not a server name")),
            Kind::Invalid
        );
        assert_eq!(
            kind_of_error(&anyhow::anyhow!("`h` resolved to nothing")),
            Kind::Offline
        );
        assert_eq!(kind_of_error(&anyhow::anyhow!("stopped")), Kind::Failed);
    }

    #[test]
    fn a_target_carries_what_the_request_needs_and_no_fragment() {
        let t = Target::of(&url("https://Example.org/a%20b?q=1#frag")).unwrap();
        assert_eq!(
            t,
            Target {
                tls: true,
                host: "example.org".into(),
                port: 443,
                host_header: "example.org".into(),
                path: "/a%20b?q=1".into(),
            }
        );
        let t = Target::of(&url("https://[::1]:9/x")).unwrap();
        assert_eq!(
            (t.host.as_str(), t.host_header.as_str()),
            ("::1", "[::1]:9")
        );
    }

    /// A pass for `host`, minted the only way there is: the person let it, in
    /// a consent file of the test's own.
    fn pass_for(host: &str, tag: &str) -> ronoctua_consent::Pass {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-http-pass-{}-{tag}", std::process::id()));
        let store = ronoctua_consent::store_at(Some(dir.clone()));
        ronoctua_consent::let_hosts_in(&store, &[host], true).unwrap();
        let pass = ronoctua_consent::pass_in(&store, host).unwrap();
        let _ = std::fs::remove_dir_all(dir);
        pass
    }

    /// FAILS IF: a pass for one host lets a request to another — or the
    /// check is made after something was sent.
    #[test]
    fn a_pass_lets_its_own_host_and_nothing_is_sent_to_another() {
        let pass = pass_for("en.wiktionary.org", "hosts");
        let mut sent = 0;
        let mut send = |_: &Target, _: &Call<'_>| {
            sent += 1;
            Ok(reply(200, &[], b"{}"))
        };
        let ask = Ask::get("https://api.mymemory.translated.net/get?q=x", "t");
        let refused = request_let_over(&pass, &ask, &mut send).unwrap_err();
        assert_eq!(refused.kind, Kind::Invalid);
        assert!(
            !refused.trouble.contains("q=x"),
            "the refusal names the address without its query: {}",
            refused.trouble
        );
        let ask = Ask::get(
            "https://EN.wiktionary.org/api/rest_v1/page/definition/casa",
            "t",
        );
        let got = request_let_over(&pass, &ask, &mut send).unwrap();
        assert_eq!(got.reply.status, 200);
        assert_eq!(sent, 1, "only the request on the pass's host was sent");
    }

    /// FAILS IF: a file a page named is asked when the page was not on the
    /// pass's host, or with the caller's headers or a body — anything of the
    /// person's reaching a host nobody let — or is not asked at all when the
    /// page was.
    #[test]
    fn a_file_a_page_named_is_asked_bare_on_the_pages_pass() {
        let pass = pass_for("example.org", "named");
        let sent = std::cell::Cell::new(0);
        let mut send = |target: &Target, _: &Call<'_>| {
            assert_eq!(target.host, "cdn.example.net");
            sent.set(sent.get() + 1);
            Ok(reply(200, &[], b"{}"))
        };
        let ask = Ask::get("https://cdn.example.net/site.webmanifest", "t");
        let e = request_named_over(&pass, "https://other.org/", &ask, &mut send).unwrap_err();
        assert_eq!(e.kind, Kind::Invalid, "a page on another host");
        let headers = [("Authorization", "Bearer secret")];
        let dressed =
            Ask::get("https://cdn.example.net/site.webmanifest", "t").with_headers(&headers);
        let e = request_named_over(&pass, "https://example.org/", &dressed, &mut send).unwrap_err();
        assert_eq!(e.kind, Kind::Invalid, "the caller's headers");
        assert_eq!(sent.get(), 0, "something was sent before the refusal");
        let got = request_named_over(&pass, "https://example.org/app", &ask, &mut send).unwrap();
        assert_eq!(got.reply.status, 200);
        assert_eq!(sent.get(), 1);
    }

    /// FAILS IF: a pass lets a host beside its domain or above it, a pass
    /// of another party lets a host under it, or nothing below it is let —
    /// or anything is sent before the answer.
    #[test]
    fn a_pass_for_a_domain_lets_the_names_under_it_and_no_other() {
        assert!(is_under("de1.api.radio-browser.info", "radio-browser.info"));
        assert!(is_under("Radio-Browser.info.", "radio-browser.info"));
        assert!(!is_under("evil-radio-browser.info", "radio-browser.info"));
        assert!(!is_under(
            "radio-browser.info.evil.example",
            "radio-browser.info"
        ));
        assert!(!is_under("info", "radio-browser.info"));
        assert!(!is_under(".radio-browser.info", "radio-browser.info"));
        assert!(!is_under("x..radio-browser.info", "radio-browser.info"));
        assert!(!is_under("a..b.radio-browser.info", "radio-browser.info"));
        assert!(!is_under("radio-browser.info", ""));

        let pass = pass_for("radio-browser.info", "under");
        let domain = "radio-browser.info";
        let mut sent = Vec::new();
        let mut send = |target: &Target, _: &Call<'_>| {
            sent.push(target.host.clone());
            Ok(reply(200, &[], b"[]"))
        };
        let beside = Ask::get("https://evil-radio-browser.info/json/servers?k=v", "t");
        let refused = request_under_over(&pass, domain, &beside, &mut send).unwrap_err();
        assert_eq!(refused.kind, Kind::Invalid);
        assert!(!refused.trouble.contains("k=v"), "{}", refused.trouble);
        let below = Ask::get("https://de1.api.radio-browser.info/json/stats", "t");
        assert_eq!(
            request_under_over(&pass, domain, &below, &mut send)
                .unwrap()
                .reply
                .status,
            200
        );
        // A REGISTRY'S NAME is no party's domain.
        assert_eq!(
            request_under_over(&pass, "info", &below, &mut send)
                .unwrap_err()
                .kind,
            Kind::Invalid
        );
        // A PASS OF ANOTHER PARTY lets nothing under this domain, though the
        // address is under it: the party let is the pass's.
        let other = pass_for("lrclib.net", "under-other");
        assert_eq!(
            request_under_over(&other, domain, &below, &mut send)
                .unwrap_err()
                .kind,
            Kind::Invalid
        );
        // And a session host under OpenSubtitles' domain, on the pass its
        // row lets (`api.`): the party's.
        let subs = pass_for("api.opensubtitles.com", "under-subs");
        let session = Ask::get("https://vip-api.opensubtitles.com/api/v1/download", "t");
        assert_eq!(
            request_under_over(&subs, "opensubtitles.com", &session, &mut send)
                .unwrap()
                .reply
                .status,
            200
        );
        assert_eq!(
            sent,
            ["de1.api.radio-browser.info", "vip-api.opensubtitles.com"],
            "only the hosts under the domain were asked"
        );
    }

    #[test]
    fn a_pass_follows_the_hosts_own_redirects() {
        let pass = pass_for("coverartarchive.org", "hops");
        let mut replies = vec![
            reply(200, &[], b"image"),
            reply(
                302,
                &[("Location", "https://ia800.us.archive.org/i.jpg")],
                b"",
            ),
            reply(
                307,
                &[("Location", "https://archive.org/download/x/i.jpg")],
                b"",
            ),
        ];
        let mut hosts = Vec::new();
        // A first `http` address of an upgraded host is judged as it goes out.
        let ask = Ask::get("http://coverartarchive.org/release/x/front", "t");
        let got = request_let_over(&pass, &ask, &mut |target, _| {
            hosts.push(target.host.clone());
            replies
                .pop()
                .ok_or_else(|| anyhow::anyhow!("no status line"))
        })
        .unwrap();
        assert_eq!(got.reply.body, b"image");
        assert_eq!(
            hosts,
            ["coverartarchive.org", "archive.org", "ia800.us.archive.org"]
        );
    }

    /// FAILS IF: a key the person gave one host (a header, or the body a
    /// 307 carries on) reaches the host a redirect names — the person let the
    /// first host, not the second.
    #[test]
    fn a_let_request_carries_its_key_to_its_own_host_only() {
        let pass = pass_for("api.deepl.com", "carry");
        let key = [("Authorization", "DeepL-Auth-Key k-123")];
        // GET: followed to the other host, without the caller's headers.
        let mut replies = vec![
            reply(200, &[], b"{}"),
            reply(302, &[("Location", "https://cdn.example/x")], b""),
            reply(302, &[("Location", "/v2/usage")], b""),
        ];
        let mut seen = Vec::new();
        let ask = Ask::get("https://api.deepl.com/v2/x", "t").with_headers(&key);
        let got = request_let_over(&pass, &ask, &mut |target, call| {
            let keyed = call.headers.iter().any(|(n, _)| *n == "Authorization");
            seen.push((target.host.clone(), keyed));
            replies
                .pop()
                .ok_or_else(|| anyhow::anyhow!("no status line"))
        })
        .unwrap();
        assert_eq!(got.reply.status, 200);
        assert_eq!(
            seen,
            [
                ("api.deepl.com".to_owned(), true),
                ("api.deepl.com".to_owned(), true),
                ("cdn.example".to_owned(), false),
            ]
        );
        // POST: a 307 on its own host keeps the body; one to another host is
        // refused before anything is sent there.
        let mut replies = vec![
            reply(307, &[("Location", "https://elsewhere.example/t")], b""),
            reply(307, &[("Location", "/v2/translate")], b""),
        ];
        let mut bodies = Vec::new();
        let ask = Ask::get("https://api.deepl.com/v2/t", "t").posting(r#"{"text":["x"]}"#);
        let e = request_let_over(&pass, &ask, &mut |target, call| {
            bodies.push((target.host.clone(), call.body.is_some()));
            replies
                .pop()
                .ok_or_else(|| anyhow::anyhow!("no status line"))
        })
        .unwrap_err();
        assert_eq!(e.kind, Kind::Invalid, "{e}");
        assert_eq!(
            bodies,
            [
                ("api.deepl.com".to_owned(), true),
                ("api.deepl.com".to_owned(), true),
            ]
        );
        // The older walk is as it was: it carries both on.
        let mut replies = vec![
            reply(200, &[], b"{}"),
            reply(307, &[("Location", "https://elsewhere.example/t")], b""),
        ];
        let mut carried = Vec::new();
        walk(&ask.with_headers(&key), &mut |_, call| {
            carried.push((
                call.headers.iter().any(|(n, _)| *n == "Authorization"),
                call.body.is_some(),
            ));
            replies
                .pop()
                .ok_or_else(|| anyhow::anyhow!("no status line"))
        })
        .unwrap();
        assert_eq!(carried, [(true, true), (true, true)]);
    }

    /// FAILS IF: an aborted request goes on to its next hop, or hands back
    /// what a shut socket read as if it were the answer.
    #[test]
    fn an_aborted_request_stops_and_answers_no_half_reply() {
        let abort = Abort::new();
        let ask = Ask::get("https://example.org/a", "t").abortable(&abort);
        let mut hops = 0;
        let got = walk(&ask, &mut |_, _| {
            hops += 1;
            // The person typed on while this hop was out.
            abort.abort();
            Ok(reply(301, &[("Location", "/b")], b"half"))
        });
        let e = got.unwrap_err();
        assert_eq!(e.kind, Kind::Failed, "{e}");
        assert_eq!(hops, 1, "the redirect was not followed");
        // And one aborted before it starts sends nothing.
        let mut sent = false;
        let e = walk(&ask, &mut |_, _| {
            sent = true;
            Ok(reply(200, &[], b""))
        })
        .unwrap_err();
        assert_eq!(e.kind, Kind::Failed);
        assert!(!sent);
        assert!(format!("{ask:?}").contains("abortable: true"));
    }

    fn an_upload(url: &str) -> Upload<'_> {
        Upload {
            url,
            headers: &[("x-apikey", "the-key")],
            user_agent: "t",
            most: 1 << 20,
            patience: Duration::from_secs(1),
            abort: None,
        }
    }

    /// FAILS IF: a file is sent to a host the pass does not let, a redirect
    /// of an upload is followed (the file sent twice, the second time
    /// somewhere nobody let), or the key header stops going with it.
    #[test]
    fn an_upload_goes_to_its_own_host_once() {
        let pass = pass_for("www.virustotal.com", "upload");
        let mut calls = Vec::new();
        let mut file: &[u8] = b"MZ....";
        let mut sent = |_: u64, _: u64| true;
        let refused = upload_let_over(
            &pass,
            &an_upload("https://evil.example/api/v3/files"),
            crate::multipart::Sending {
                part: crate::multipart::Part {
                    field: "file",
                    filename: "a.exe",
                    content_type: "application/octet-stream",
                    length: 6,
                },
                file: &mut file,
                sent: &mut sent,
            },
            &mut |_, _, _| {
                calls.push("sent");
                Ok(reply(200, &[], b"{}"))
            },
        )
        .unwrap_err();
        assert_eq!(refused.kind, Kind::Invalid);
        assert!(calls.is_empty(), "a file went to a host nobody let");

        let mut file: &[u8] = b"MZ....";
        let mut sent = |_: u64, _: u64| true;
        let mut heard = Vec::new();
        let got = upload_let_over(
            &pass,
            &an_upload("https://www.virustotal.com/api/v3/files"),
            crate::multipart::Sending {
                part: crate::multipart::Part {
                    field: "file",
                    filename: "a.exe",
                    content_type: "application/octet-stream",
                    length: 6,
                },
                file: &mut file,
                sent: &mut sent,
            },
            &mut |target, call, sending| {
                heard.push((
                    target.host.clone(),
                    call.method.to_owned(),
                    call.path.to_owned(),
                    call.headers
                        .iter()
                        .any(|(n, v)| *n == "x-apikey" && *v == "the-key"),
                    sending.part.length,
                ));
                Ok(reply(200, &[], br#"{"data":{"id":"x"}}"#))
            },
        )
        .unwrap();
        assert_eq!(got.reply.status, 200);
        assert_eq!(
            heard,
            [(
                "www.virustotal.com".to_owned(),
                "POST".to_owned(),
                "/api/v3/files".to_owned(),
                true,
                6
            )]
        );

        let mut file: &[u8] = b"MZ....";
        let mut sent = |_: u64, _: u64| true;
        let mut hops = 0;
        let moved = upload_let_over(
            &pass,
            &an_upload("https://www.virustotal.com/api/v3/files"),
            crate::multipart::Sending {
                part: crate::multipart::Part {
                    field: "file",
                    filename: "a.exe",
                    content_type: "application/octet-stream",
                    length: 6,
                },
                file: &mut file,
                sent: &mut sent,
            },
            &mut |_, _, _| {
                hops += 1;
                Ok(reply(
                    307,
                    &[("Location", "https://elsewhere.example/")],
                    b"",
                ))
            },
        )
        .unwrap_err();
        assert_eq!(moved.kind, Kind::Invalid);
        assert_eq!(hops, 1, "the redirect of an upload was followed");
        let shown = format!("{:?}", an_upload("https://www.virustotal.com/x?k=v"));
        assert!(
            !shown.contains("the-key") && !shown.contains("k=v"),
            "{shown}"
        );
    }
}
