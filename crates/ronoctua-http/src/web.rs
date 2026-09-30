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
//! # Every function that sends blocks
//!
//! On the network, for up to [`crate::tls::PATIENCE`] per hop. Call it on a
//! THREAD, never on a drawing one.

use std::borrow::Cow;
use std::io::Read as _;
use std::time::Duration;

use url::Url;

use crate::{Call, Reply};

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
        }
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
        crate::tls::fetch(&target.host, target.port, call, most)
    })
}

/// The walk of [`request`], over any transport: `send` is TLS for the
/// machine and a list of answers for a test.
fn walk(
    ask: &Ask<'_>,
    send: &mut dyn FnMut(&Target, &Call<'_>) -> anyhow::Result<Reply>,
) -> Result<Got, Error> {
    let first = upgraded(ask.url);
    let mut at = transport_of(&first)?;
    let mut body = ask.body;
    for hop in 0..=ask.hops {
        let target = Target::of(&at)?;
        let accept = ask
            .accept
            .unwrap_or(if body.is_some() { crate::JSON } else { "*/*" });
        let headers = headers(ask.user_agent, accept, ask.coding, ask.headers)?;
        let method = if body.is_some() { "POST" } else { "GET" };
        let mut call = Call::new(method, &target.path, &target.host_header)
            .with_headers(&headers)
            .waiting(ask.patience);
        if let Some(b) = body {
            call = call.with_body(b);
        }
        let reply = send(&target, &call)
            .map_err(|e| Error::new(kind_of_error(&e), format!("{at}: {e:#}")))?;
        if let Some(location) = redirect_of(&reply) {
            if hop == ask.hops {
                break;
            }
            at = next_hop(&at, location)?;
            body = body.filter(|_| keeps_the_body(reply.status));
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
    let parsed =
        Url::parse(url.trim()).map_err(|e| Error::new(Kind::Invalid, format!("{url}: {e}")))?;
    if parsed.scheme() != "https" {
        return Err(Error::new(
            Kind::Scheme,
            format!("{url}: only https is fetched"),
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
    // The framer carries https only; an http→http hop is allowed by the rule
    // and refused by the transport, the same way a first http address is.
    if next.scheme() != "https" {
        return Err(Error::new(
            Kind::Scheme,
            format!("{from} redirected to {next}: only https is fetched"),
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
/// the `Host` header and the path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
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
}
