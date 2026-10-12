//! The network, through the one framer (`ronoctua-http`): ONE request
//! ([`request`], a GET or a JSON POST, with the caller's headers), ONE streaming
//! download ([`download`]) and ONE redirect rule ([`next_hop`]: at most
//! [`MOST_REDIRECTS`] hops, `https` to `https` only), every one with our own
//! User-Agent and a ceiling the caller chooses.
//!
//! # The redirect rule is the framer's `web`, and the ceiling is ours
//!
//! Following a redirect is a DECISION — a redirect from `https` to `http` is a
//! downgrade somebody on the path can ask for — and it was taken here, for the
//! person's programs, until the player's lookups needed the same walk. It
//! moved to `ronoctua_http::web` so there is one rule: [`request`] is its walk
//! with our User-Agent and our five hops, and [`download`] walks the same
//! rule ([`next_hop`], a pure function a test reads without a socket) around
//! a streaming transport that the web module does not have. What stays here
//! is what is ours: which [`Refusal`] each of the web's kinds is.
//!
//! # Plain `http` is refused, and the refusal is the transport's
//!
//! The framer's only internet transport is TLS (`tls::fetch`,
//! `tls::download`). Its TCP call (`over_tcp`) is for the loopback: it takes an
//! address and not a name, gives no ceiling on the body and hands the body
//! back as TEXT, which is how a PNG loses every byte above 0x7F. So a fetch of
//! an `http://` address is [`Refusal::UrlScheme`] before anything is sent —
//! except a FIRST address on one of `web::UPGRADED_HOSTS`, which [`request`]
//! and [`download`] alike ask over `https` (`web::upgraded`): those hosts
//! answer plain `http` only with a redirect to the same address over `https`.
//! [`transport_of`] stays strict, because it JUDGES an address as written.
//! The REDIRECT rule is still the whole rule ([`next_hop`] allows `http` to
//! `https` and refuses the reverse), so the day the framer carries plain TCP
//! only its `transport_of` has to change. A web app may still OPEN an `http`
//! address in Chrome — that is `web::canonical_url`'s decision, not this one;
//! only its icon is looked for over `https` (see `icons::from_site`).
//!
//! # The seam: [`Fetch`]
//!
//! Every module that reaches a host through a parameter takes a `&dyn Fetch`:
//! [`Net`] is the machine's network, and a test hands a fake made of captured
//! answers (`canned`), so no test reaches a host. The free functions
//! ([`get`], [`post_json`], [`fetch`]) are the same request for the callers
//! that only ever want a 2xx.
//!
//! # Every function here blocks
//!
//! On the network, for up to [`ronoctua_http::tls::PATIENCE`] per hop (and
//! [`DOWNLOAD_PATIENCE`] per read of a download). Call them on a THREAD, never
//! on a drawing one.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ronoctua_http::web;
use url::Url;

#[cfg(test)]
use ronoctua_http::web::keeps_the_body;
pub use ronoctua_http::web::{Target, redirect_of};

use crate::progress::{Phase, Report};
use crate::{Failure, Refusal};

/// Who is asking, in every request: a service that sees an unknown client is
/// told where it comes from, and one that throttles by client can throttle
/// this one without throttling a browser. GitHub answers 403 to a request
/// without one.
pub const USER_AGENT: &str = concat!(
    "ronoctua-software/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/ronoctua/ronoctua-distro)"
);

/// How many redirects a request may follow before it is refused.
///
/// Five: a login wall, a locale, a CDN and a trailing slash are four, and a
/// chain longer than that is a loop somebody else wrote.
pub const MOST_REDIRECTS: usize = 5;

/// How long one read or one connect of a DOWNLOAD may wait. Thirty seconds and
/// not the framer's ten: a storage host under load stalls between chunks, and
/// a download that dies at 90% on a ten-second hiccup is the worse failure.
pub const DOWNLOAD_PATIENCE: Duration = Duration::from_secs(30);

/// How often a download says how far it got: never more often than this…
pub const BEAT_EVERY: Duration = Duration::from_millis(100);

/// …and, when the size is known, never for less than one part in this many.
pub const BEAT_PARTS: u64 = 100;

/// What a request returned, and where it ended up after redirects.
#[derive(Debug, Clone)]
pub struct Got {
    /// The final URL.
    pub url: String,
    /// The reply.
    pub reply: ronoctua_http::Reply,
}

/// The network as this crate's modules use it. A trait so a test serves
/// fixtures where the machine would reach a host; the real one is [`Net`].
///
/// `Sync`, because a search asks every source on a thread of its own and they
/// all share one.
pub trait Fetch: Sync {
    /// GET with extra headers and a body ceiling, following redirects; the
    /// last hop's status is returned as it is (a 304 or a 403 is the caller's
    /// to read).
    ///
    /// # Errors
    ///
    /// As [`request`].
    fn get(&self, url: &str, headers: &[(&str, &str)], most: u64) -> Result<Got, Failure>;

    /// POST a JSON body, following redirects; the last hop's status is
    /// returned as it is.
    ///
    /// # Errors
    ///
    /// As [`request`].
    fn post_json(&self, url: &str, body: &str, most: u64) -> Result<Got, Failure>;

    /// GET and stream the body into `to`; `heard(done, total)` is told the
    /// progress and stops the download by returning `false`.
    ///
    /// # Errors
    ///
    /// As [`download`].
    fn download(
        &self,
        url: &str,
        most: u64,
        to: &Path,
        heard: &mut dyn FnMut(u64, Option<u64>) -> bool,
    ) -> Result<(), Failure>;
}

/// The network itself. Every call reaches a host: call it on a thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct Net;

impl Fetch for Net {
    fn get(&self, url: &str, headers: &[(&str, &str)], most: u64) -> Result<Got, Failure> {
        request(url, None, headers, most)
    }

    fn post_json(&self, url: &str, body: &str, most: u64) -> Result<Got, Failure> {
        request(url, Some(body), &[], most)
    }

    fn download(
        &self,
        url: &str,
        most: u64,
        to: &Path,
        heard: &mut dyn FnMut(u64, Option<u64>) -> bool,
    ) -> Result<(), Failure> {
        download(url, to, None, most, heard).map(drop)
    }
}

/// GET `url`, reading at most `most` bytes of body; only a 2xx is an answer.
/// Blocks: call it on a thread.
///
/// # Errors
///
/// `Offline` when the host cannot be reached, `TooLarge` above the ceiling,
/// `UrlScheme` for an `http` address (the framer carries TLS only — see the
/// module's header) or a redirect to anything but https, `UrlInvalid` for an
/// address that does not parse, and the status's refusal
/// ([`refusal_of_status`]) for an answer that is not a 2xx.
pub fn get(url: &str, most: u64) -> Result<Got, Refusal> {
    fetch(url, None, most).map_err(|f| f.refusal.unwrap_or(Refusal::ToolFailed))
}

/// [`get`], on the person's [`ronoctua_consent::Pass`] for the host of `url`
/// (`web::request_let`): a page at an address the person typed. The pass is
/// for the FIRST host; the redirects follow the web's rule, and nothing of
/// ours but the request itself goes past that host. Blocks: call it on a
/// thread.
///
/// # Errors
///
/// [`Refusal::NotLet`] when `url` is not on the pass's host (nothing is
/// sent), and every refusal of [`get`].
pub fn get_let(pass: &ronoctua_consent::Pass, url: &str, most: u64) -> Result<Got, Refusal> {
    // THE ADDRESS'S OWN REFUSAL first, then the pass: nothing is sent to a
    // host the pass does not name.
    first_hop(url).map_err(|f| f.refusal.unwrap_or(Refusal::UrlInvalid))?;
    if !first_host_is(pass, url) {
        return Err(Refusal::NotLet);
    }
    let ask = web::Ask::get(url, USER_AGENT)
        .most(most)
        .hops(MOST_REDIRECTS);
    let got = web::request_let(pass, &ask)
        .map_err(|e| Failure::from(e).refusal.unwrap_or(Refusal::ToolFailed))?;
    answered(Got {
        url: got.url,
        reply: got.reply,
    })
    .map_err(|f| f.refusal.unwrap_or(Refusal::ToolFailed))
}

/// Whether `url`'s first host — after the web's upgrade — is the one `pass`
/// lets. Pure.
#[must_use]
pub fn first_host_is(pass: &ronoctua_consent::Pass, url: &str) -> bool {
    first_hop(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| pass.covers(h)))
        .unwrap_or(false)
}

/// POST a JSON body (Flathub's search), same rules. Blocks: call it on a
/// thread.
///
/// # Errors
///
/// As [`get`].
pub fn post_json(url: &str, body: &str, most: u64) -> Result<Got, Refusal> {
    fetch(url, Some(body), most).map_err(|f| f.refusal.unwrap_or(Refusal::ToolFailed))
}

/// [`get`] (with `post` `None`) or [`post_json`], answering a [`Failure`] —
/// the refusal AND the words: the status a server gave, the error the socket
/// gave. For a caller that shows the job's trouble (the CLI) rather than only
/// a sentence. Blocks: call it on a thread.
///
/// # Errors
///
/// As [`get`], with the detail in [`Failure::trouble`].
pub fn fetch(url: &str, post: Option<&str>, most: u64) -> Result<Got, Failure> {
    answered(request(url, post, &[], most)?)
}

/// A reply that went through, or the refusal its status is
/// ([`refusal_of_status`]), with the address and the number as the trouble.
///
/// # Errors
///
/// Any status that is not a 2xx.
pub fn answered(got: Got) -> Result<Got, Failure> {
    match refusal_of_status(&got.reply) {
        None => Ok(got),
        Some(refusal) => Err(Failure {
            refusal: Some(refusal),
            trouble: format!("{} answered {}", got.url, got.reply.status),
        }),
    }
}

/// THE PASS for the first host of `url` (after the web's upgrade), minted
/// from the person's `consent.json` at the moment of asking: a catalogue is
/// asked only once its row "Ask {hosts}" was pressed (`OnlineSource::hosts`
/// in the shell, the hosts each catalogue module declares).
///
/// # Errors
///
/// The address's refusal ([`transport_of`]), and [`Refusal::NotLet`] when
/// the host was not let — nothing is sent.
pub fn pass_for(url: &str) -> Result<ronoctua_consent::Pass, Failure> {
    let first = first_hop(url)?;
    let host = first.host_str().unwrap_or_default();
    ronoctua_consent::pass(host).map_err(|_| Failure {
        refusal: Some(Refusal::NotLet),
        trouble: format!("{host} was not let in consent.json"),
    })
}

/// THE request: a GET (`post` `None`) or a JSON POST of `post`, carrying
/// `extra` headers after ours, following at most [`MOST_REDIRECTS`] redirects
/// by [`next_hop`] (301, 302 and 303 turn a POST into a GET; 307 and 308 carry
/// its body on), and reading at most `most` bytes of body. The LAST hop's
/// reply is returned whatever its status — [`answered`] is for a caller that
/// only wants a 2xx. Blocks: call it on a thread.
///
/// `extra` may replace our `Accept`, and nothing else of ours or of the
/// framer's (`Host`, `Connection`, `Content-*`, `User-Agent`,
/// `Accept-Encoding`); a name that is not a token or a value with a line break
/// in it is refused before anything is sent.
///
/// # Errors
///
/// The address's refusal ([`transport_of`], [`next_hop`]), `NotLet` when its
/// host was not let ([`pass_for`]: nothing is sent), `Offline` when a host
/// cannot be reached, `TooLarge` above the ceiling, `ToolFailed` for a
/// header this module will not send, after [`MOST_REDIRECTS`] redirects, or
/// for a body in a content coding that is not read ([`refusal_of_kind`]).
pub fn request(
    url: &str,
    post: Option<&str>,
    extra: &[(&str, &str)],
    most: u64,
) -> Result<Got, Failure> {
    let mut ask = web::Ask::get(url, USER_AGENT)
        .with_headers(extra)
        .most(most)
        .hops(MOST_REDIRECTS);
    if let Some(body) = post {
        ask = ask.posting(body);
    }
    // ON THE PERSON'S PASS for the first host: its headers and its body go
    // to that host alone, and the redirects are the host's answer.
    let pass = pass_for(url)?;
    let got = web::request_let(&pass, &ask)?;
    Ok(Got {
        url: got.url,
        reply: got.reply,
    })
}

/// A file a PAGE NAMED on another host — a manifest or an icon on a CDN —
/// asked bare on the pass of the page's host (`web::request_named`): the
/// page's answer, as a redirect is. `page` is the address the person typed,
/// on the pass's host. Blocks: call it on a thread.
///
/// # Errors
///
/// [`Refusal::NotLet`] when `page` is not on the pass's host (nothing is
/// sent), and every refusal of [`get`].
pub fn get_named(
    pass: &ronoctua_consent::Pass,
    page: &str,
    url: &str,
    most: u64,
) -> Result<Got, Refusal> {
    first_hop(url).map_err(|f| f.refusal.unwrap_or(Refusal::UrlInvalid))?;
    if !first_host_is(pass, page) {
        return Err(Refusal::NotLet);
    }
    let ask = web::Ask::get(url, USER_AGENT)
        .most(most)
        .hops(MOST_REDIRECTS);
    let got = web::request_named(pass, page, &ask)
        .map_err(|e| Failure::from(e).refusal.unwrap_or(Refusal::ToolFailed))?;
    answered(Got {
        url: got.url,
        reply: got.reply,
    })
    .map_err(|f| f.refusal.unwrap_or(Refusal::ToolFailed))
}

/// THE download: `url` into `to`, STREAMING — nothing of it is held in memory.
///
/// The bytes go to `<to>.part` as they arrive and are hashed on the way;
/// only when the whole body is there (and its SHA-256 is the one expected,
/// when one is) is the part renamed into place, so `to` is either the whole
/// file or absent, never half of one a later run takes for whole. Redirects
/// are followed as in [`request`]. `heard(done, total)` is told every chunk
/// (`total` is `None` when the server does not say) and stops the download by
/// returning `false`; [`beats`] turns it into paced progress lines. Blocks:
/// call it on a thread.
///
/// Returns the bytes written and their SHA-256 in lower-case hex.
///
/// # Errors
///
/// [`Refusal::Integrity`] when the hash differs (the part is deleted), the
/// network's refusals as in [`request`], the final status's refusal
/// ([`refusal_of_status`]) for an answer that is not a 2xx, and
/// [`Refusal::ToolFailed`] when the part cannot be written or `heard` stopped
/// it.
pub fn download(
    url: &str,
    to: &Path,
    expected_sha256: Option<&str>,
    most: u64,
    heard: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<(u64, String), Failure> {
    let expected = expected_sha256.map(str::to_ascii_lowercase);
    if let Some(e) = &expected
        && (e.len() != 64 || !e.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("`{e}` is not a SHA-256"),
        });
    }
    let part = part_of(to)?;
    if let Some(dir) = part.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("{}: {e}", dir.display()),
        })?;
    }
    let (written, sha) = match download_into(url, &part, most, heard) {
        Ok(done) => done,
        Err(f) => {
            let _ = std::fs::remove_file(&part);
            return Err(f);
        }
    };
    if let Some(e) = expected
        && e != sha
    {
        let _ = std::fs::remove_file(&part);
        return Err(Failure {
            refusal: Some(Refusal::Integrity),
            trouble: format!("{url}: expected sha256 {e}, got {sha}"),
        });
    }
    std::fs::rename(&part, to).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("{}: {e}", to.display()),
        }
    })?;
    Ok((written, sha))
}

/// Where a download starts: the same first step as [`request`]'s walk
/// (`web::request`), so an `http` address of one of `web::UPGRADED_HOSTS` is
/// asked over `https` and every other `http` address is refused. One address
/// is never accepted by one of the two and refused by the other. Pure.
fn first_hop(url: &str) -> Result<Url, Failure> {
    transport_of(&web::upgraded(url))
}

/// The walk of [`download`], writing into `part` (truncated per hop).
fn download_into(
    url: &str,
    part: &Path,
    most: u64,
    heard: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<(u64, String), Failure> {
    let mut at = first_hop(url)?;
    for hop in 0..=MOST_REDIRECTS {
        let target = Target::of(&at)?;
        let headers = headers_with("*/*", &[])?;
        let call = ronoctua_http::Call::new("GET", &target.path, &target.host_header)
            .with_headers(&headers)
            .waiting(DOWNLOAD_PATIENCE);
        let file = std::fs::File::create(part).map_err(|e| Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("{}: {e}", part.display()),
        })?;
        let mut sink = Hashing::new(std::io::BufWriter::with_capacity(1 << 20, file));
        // The framer says the total once, with the head, and `None` after it;
        // the caller is told the total on every chunk.
        let mut total = None;
        let mut told = |length: Option<u64>, done: u64| {
            if length.is_some() {
                total = length;
            }
            heard(done, total)
        };
        let reply = match ronoctua_http::tls::download(
            &target.host,
            target.port,
            &call,
            most,
            &mut sink,
            &mut told,
        ) {
            Ok(r) => r,
            Err(e) => {
                // A failure to WRITE surfaces through the framer as an I/O
                // error like any socket's, and "the disk is full" would read as
                // "no network". The sink remembers its own.
                let refusal = if sink.failed.is_some() {
                    Refusal::ToolFailed
                } else {
                    refusal_of_error(&e)
                };
                let said = sink.failed.map_or_else(
                    || format!("{at}: {e:#}"),
                    |w| format!("{}: {w}", part.display()),
                );
                return Err(Failure {
                    refusal: Some(refusal),
                    trouble: said,
                });
            }
        };
        if let Some(location) = redirect_of(&reply) {
            if hop == MOST_REDIRECTS {
                break;
            }
            at = next_hop(&at, location)?;
            continue;
        }
        if let Some(refusal) = refusal_of_status(&reply) {
            return Err(Failure {
                refusal: Some(refusal),
                trouble: format!("{at} answered {}", reply.status),
            });
        }
        let written = sink.written;
        let sha = sink.finish().map_err(|e| Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("{}: {e}", part.display()),
        })?;
        return Ok((written, sha));
    }
    Err(Failure {
        refusal: Some(Refusal::ToolFailed),
        trouble: format!("more than {MOST_REDIRECTS} redirects from {url}"),
    })
}

/// A download's `heard` that reports [`Phase::Downloading`] steps named `now`,
/// paced by [`Pace`], and never stops the download.
pub fn beats<'a>(
    report: &'a mut dyn Report,
    now: &'a str,
) -> impl FnMut(u64, Option<u64>) -> bool + 'a {
    let mut pace = Pace::default();
    move |done, total| {
        let total = total.unwrap_or(0);
        if pace.due(Instant::now(), done, total) {
            report.step(Phase::Downloading, done, total, now);
        }
        true
    }
}

/// The headers of one request: ours (the User-Agent and
/// `Accept-Encoding: identity`), then the caller's — [`web::headers`], with
/// the web's refusal as ours.
///
/// `identity` on purpose: without the header a server may send ANY content
/// coding (RFC 9110 §12.5.3), and a download is written as it arrives, where
/// nothing inflates it.
///
/// # Errors
///
/// [`Refusal::ToolFailed`] for a name that is not a token, one of
/// [`web::OWN_HEADERS`], or a value with a control character in it.
fn headers_with<'a>(
    accept: &'a str,
    extra: &[(&'a str, &'a str)],
) -> Result<Vec<(&'a str, &'a str)>, Failure> {
    Ok(web::headers(
        USER_AGENT,
        accept,
        web::Coding::Identity,
        extra,
    )?)
}

/// `<to>.part`, beside it.
fn part_of(to: &Path) -> Result<PathBuf, Failure> {
    let name = to.file_name().ok_or_else(|| Failure {
        refusal: Some(Refusal::ToolFailed),
        trouble: format!("{} names no file", to.display()),
    })?;
    let mut part = name.to_os_string();
    part.push(".part");
    Ok(to.with_file_name(part))
}

/// An address parsed and checked against what this module can carry: `https`
/// only (see the module's header for why `http` is refused here), a host, and
/// no user name or password in it — [`web::transport_of`], refused as ours.
///
/// # Errors
///
/// [`Refusal::UrlInvalid`] for what does not parse or has no host,
/// [`Refusal::UrlScheme`] for anything but `https`,
/// [`Refusal::UrlCredentials`] for a user name or a password.
pub fn transport_of(url: &str) -> Result<Url, Failure> {
    Ok(web::transport_of(url)?)
}

/// Where a redirect from `from` to `location` goes, or why it may not —
/// [`web::next_hop`], refused as ours. Pure.
///
/// # Errors
///
/// [`Refusal::UrlScheme`], [`Refusal::UrlCredentials`] or, for a `Location`
/// that does not parse, [`Refusal::UrlInvalid`].
pub fn next_hop(from: &Url, location: &str) -> Result<Url, Failure> {
    Ok(web::next_hop(from, location)?)
}

/// Which refusal each of the web's kinds is. Pure.
#[must_use]
pub const fn refusal_of_kind(kind: web::Kind) -> Refusal {
    match kind {
        web::Kind::Invalid => Refusal::UrlInvalid,
        web::Kind::Scheme => Refusal::UrlScheme,
        web::Kind::Credentials => Refusal::UrlCredentials,
        web::Kind::Offline => Refusal::Offline,
        web::Kind::TooLarge => Refusal::TooLarge,
        // A certificate that does not verify is not "no network"; a header we
        // will not send, a loop, a coding we do not read and a stopped
        // download are the tool failing.
        web::Kind::Certificate
        | web::Kind::Header
        | web::Kind::Loop
        | web::Kind::Coding
        | web::Kind::Failed => Refusal::ToolFailed,
    }
}

impl From<web::Error> for Failure {
    fn from(e: web::Error) -> Self {
        Self {
            refusal: Some(refusal_of_kind(e.kind)),
            trouble: e.trouble,
        }
    }
}

/// Why a reply that is not a redirect is not an answer; `None` for a 2xx.
///
/// 404 and 410 are [`Refusal::UnknownItem`] (Flathub's answer for an id it
/// does not have, GitHub's for an asset that is gone). 429 and 503 are
/// [`Refusal::RateLimited`] — "come back later", in the words of a service
/// that throttles (Flathub publishes no limit and answers either) — and so is
/// GitHub's 403 whose `X-RateLimit-Remaining` is zero, its sixty requests an
/// hour spent. Every other status is [`Refusal::ToolFailed`]; the number goes
/// in the trouble. Pure.
#[must_use]
pub fn refusal_of_status(reply: &ronoctua_http::Reply) -> Option<Refusal> {
    if reply.went_through() {
        return None;
    }
    let spent = reply
        .header("x-ratelimit-remaining")
        .is_some_and(|v| v.trim() == "0");
    Some(match reply.status {
        404 | 410 => Refusal::UnknownItem,
        429 | 503 => Refusal::RateLimited,
        403 if spent => Refusal::RateLimited,
        _ => Refusal::ToolFailed,
    })
}

/// Which refusal a transport error is: [`web::kind_of_error`] read as ours
/// ([`refusal_of_kind`]) — `Offline` for the network not being there,
/// `ToolFailed` for TLS itself failing (a certificate that does not verify is
/// not "no network"), `TooLarge` over the ceiling. Pure.
#[must_use]
pub fn refusal_of_error(e: &anyhow::Error) -> Refusal {
    refusal_of_kind(web::kind_of_error(e))
}

/// When a download says how far it got.
///
/// Not more often than [`BEAT_EVERY`] — the queue redraws a bar per beat, and
/// a gigabit link would otherwise print ten thousand lines a second — and,
/// with a known total, not for less than 1/[`BEAT_PARTS`] of it. The first
/// beat and the last (all of a known total) always go: the bar has to appear,
/// and it has to end full.
#[derive(Debug, Default, Clone, Copy)]
pub struct Pace {
    last: Option<(Instant, u64)>,
}

impl Pace {
    /// Whether a beat is due at `now`, with `done` of `total` (0 = unknown),
    /// and if so, remember it.
    pub fn due(&mut self, now: Instant, done: u64, total: u64) -> bool {
        let due = match self.last {
            None => true,
            Some(_) if total > 0 && done >= total => true,
            Some((when, before)) => {
                now.saturating_duration_since(when) >= BEAT_EVERY
                    && (total == 0
                        || done.saturating_sub(before).saturating_mul(BEAT_PARTS) >= total)
            }
        };
        if due {
            self.last = Some((now, done));
        }
        due
    }
}

/// A writer that hashes what goes through it, counts it, and remembers its
/// own failure (see [`download_into`] for why).
struct Hashing<W: std::io::Write> {
    inner: W,
    digest: aws_lc_rs::digest::Context,
    written: u64,
    failed: Option<String>,
}

impl<W: std::io::Write> Hashing<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            digest: aws_lc_rs::digest::Context::new(&aws_lc_rs::digest::SHA256),
            written: 0,
            failed: None,
        }
    }

    /// Flush, and the hash of everything written.
    fn finish(mut self) -> std::io::Result<String> {
        self.inner.flush()?;
        Ok(crate::run::hex(self.digest.finish().as_ref()))
    }
}

impl<W: std::io::Write> std::io::Write for Hashing<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match self.inner.write(bytes) {
            Ok(n) => {
                self.digest.update(&bytes[..n]);
                self.written += n as u64;
                Ok(n)
            }
            Err(e) => {
                self.failed = Some(e.to_string());
                Err(e)
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner
            .flush()
            .inspect_err(|e| self.failed = Some(e.to_string()))
    }
}

#[cfg(test)]
pub(crate) mod canned {
    //! A network made of answers, which records what it was asked: what every
    //! test of this crate hands where the machine would reach a host.

    use std::path::Path;
    use std::sync::{Mutex, MutexGuard, PoisonError};

    use super::{Fetch, Got};
    use crate::{Failure, Refusal};

    /// A value behind a lock, read with the `borrow` of the `RefCell` it
    /// replaced: a search shares one fake among its threads, so the fake is
    /// `Sync`.
    #[derive(Debug, Default)]
    pub(crate) struct Noted<T>(Mutex<T>);

    impl<T> Noted<T> {
        fn new(value: T) -> Self {
            Self(Mutex::new(value))
        }

        /// The value, to read.
        pub(crate) fn borrow(&self) -> MutexGuard<'_, T> {
            self.0.lock().unwrap_or_else(PoisonError::into_inner)
        }

        /// The value, to change.
        pub(crate) fn borrow_mut(&self) -> MutexGuard<'_, T> {
            self.borrow()
        }
    }

    /// One request as it was asked: the address and its headers.
    pub(crate) type Asked = (String, Vec<(String, String)>);
    /// One canned answer: the address, the status, the headers, the body.
    pub(crate) type Answer<'a> = (&'a str, u16, Vec<(&'a str, &'a str)>, Vec<u8>);

    /// Canned answers for requests, canned bodies for downloads.
    #[derive(Default)]
    pub(crate) struct Canned {
        /// `(url, reply)`, each answered once.
        pub answers: Noted<Vec<(String, ronoctua_http::Reply)>>,
        /// `(url, body)` for downloads, each served as often as asked.
        pub files: Noted<Vec<(String, Vec<u8>)>>,
        /// Every request (GET and POST), with its headers.
        pub asked: Noted<Vec<Asked>>,
        /// Every POST's `(url, body)`.
        pub posted: Noted<Vec<(String, String)>>,
        /// Every download's URL.
        pub fetched: Noted<Vec<String>>,
    }

    impl Canned {
        pub(crate) fn new(answers: Vec<Answer<'_>>) -> Self {
            Self {
                answers: Noted::new(
                    answers
                        .into_iter()
                        .map(|(url, status, headers, body)| {
                            let headers = headers
                                .into_iter()
                                .map(|(n, v)| (n.to_owned(), v.to_owned()))
                                .collect();
                            (
                                url.to_owned(),
                                ronoctua_http::Reply {
                                    status,
                                    headers,
                                    body,
                                },
                            )
                        })
                        .collect(),
                ),
                ..Self::default()
            }
        }

        pub(crate) fn serving(self, url: &str, body: Vec<u8>) -> Self {
            self.files.borrow_mut().push((url.to_owned(), body));
            self
        }

        /// The canned reply for `url`, once; `Offline` when there is none.
        fn answer(&self, url: &str) -> Result<Got, Failure> {
            let mut answers = self.answers.borrow_mut();
            let at = answers
                .iter()
                .position(|(u, _)| u == url)
                .ok_or_else(|| Failure::refused(Refusal::Offline))?;
            let (url, reply) = answers.remove(at);
            Ok(Got { url, reply })
        }
    }

    impl Fetch for Canned {
        fn get(&self, url: &str, headers: &[(&str, &str)], _most: u64) -> Result<Got, Failure> {
            self.asked.borrow_mut().push((
                url.to_owned(),
                headers
                    .iter()
                    .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                    .collect(),
            ));
            self.answer(url)
        }

        fn post_json(&self, url: &str, body: &str, _most: u64) -> Result<Got, Failure> {
            self.asked.borrow_mut().push((url.to_owned(), Vec::new()));
            self.posted
                .borrow_mut()
                .push((url.to_owned(), body.to_owned()));
            self.answer(url)
        }

        fn download(
            &self,
            url: &str,
            most: u64,
            to: &Path,
            heard: &mut dyn FnMut(u64, Option<u64>) -> bool,
        ) -> Result<(), Failure> {
            self.fetched.borrow_mut().push(url.to_owned());
            let files = self.files.borrow();
            let body = &files
                .iter()
                .find(|(u, _)| u == url)
                .ok_or_else(|| Failure::refused(Refusal::Offline))?
                .1;
            if body.len() as u64 > most {
                return Err(Failure::refused(Refusal::TooLarge));
            }
            std::fs::write(to, body).map_err(|e| Failure::trouble(e.to_string()))?;
            heard(body.len() as u64, Some(body.len() as u64));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    /// A pass minted from a consent file of the test's own.
    fn pass_for_test(host: &str, tag: &str) -> ronoctua_consent::Pass {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-pass-{}-{tag}",
            std::process::id()
        ));
        let store = ronoctua_consent::store_at(Some(dir.clone()));
        ronoctua_consent::let_hosts_in(&store, &[host], true).unwrap();
        let pass = ronoctua_consent::pass_in(&store, host).unwrap();
        let _ = std::fs::remove_dir_all(dir);
        pass
    }

    /// FAILS IF: a page is asked on a pass for another host — the check
    /// made at all, or made after the address went out (an unroutable
    /// address answers `NotLet` before any socket) — or an address the web
    /// refuses anyway is said to be "not let", or the pass stops covering
    /// its own host after the web's upgrade.
    #[test]
    fn a_page_goes_out_only_on_the_pass_for_its_host() {
        let pass = pass_for_test("app.example", "page");
        assert!(first_host_is(&pass, "https://app.example/x"));
        assert!(first_host_is(&pass, "https://APP.example./"));
        assert!(!first_host_is(&pass, "https://cdn.example/manifest.json"));
        assert!(!first_host_is(&pass, "https://evil-app.example/"));
        assert_eq!(
            get_let(&pass, "https://other.invalid/", 1 << 10).unwrap_err(),
            Refusal::NotLet
        );
        assert_eq!(
            get_let(&pass, "ftp://app.example/", 1 << 10).unwrap_err(),
            Refusal::UrlScheme
        );
    }

    /// FAILS IF: a catalogue's request goes out to a host nobody let —
    /// `request` asked with no pass, as it did before the catalogues had a
    /// consent row — or the refusal stops being `NotLet`, said before any
    /// socket (an unroutable host would be `Offline`).
    #[test]
    fn a_catalogue_is_asked_only_on_a_pass() {
        let refused = request("https://never-let.invalid/feed.json", None, &[], 1 << 10)
            .expect_err("a host nobody let was asked");
        assert_eq!(
            refused.refusal,
            Some(Refusal::NotLet),
            "{}",
            refused.trouble
        );
        assert_eq!(
            pass_for("ftp://f-droid.org/")
                .map(|_| ())
                .unwrap_err()
                .refusal,
            Some(Refusal::UrlScheme)
        );
        let pass = pass_for_test("example.org", "named");
        assert_eq!(
            get_named(
                &pass,
                "https://other.invalid/",
                "https://cdn.invalid/i.png",
                1 << 10
            )
            .unwrap_err(),
            Refusal::NotLet,
            "a page off the pass named a file"
        );
    }

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    fn reply(status: u16, headers: &[(&str, &str)]) -> ronoctua_http::Reply {
        ronoctua_http::Reply {
            status,
            headers: headers
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
            body: Vec::new(),
        }
    }

    #[test]
    fn a_redirect_goes_up_to_https_and_never_down() {
        let secure = url("https://example.org/a/b");
        // Relative, absolute-path and absolute locations all resolve.
        assert_eq!(
            next_hop(&secure, "c").unwrap().as_str(),
            "https://example.org/a/c"
        );
        assert_eq!(
            next_hop(&secure, "/x?y=1").unwrap().as_str(),
            "https://example.org/x?y=1"
        );
        assert_eq!(
            next_hop(&secure, "https://cdn.example.net/i.png")
                .unwrap()
                .host_str(),
            Some("cdn.example.net")
        );
        assert_eq!(
            next_hop(&secure, "//cdn.example.net/i.png")
                .unwrap()
                .scheme(),
            "https"
        );
        // THE DOWNGRADE: anybody on the path can answer a 302 to http.
        assert_eq!(
            next_hop(&secure, "http://example.org/")
                .unwrap_err()
                .refusal,
            Some(Refusal::UrlScheme)
        );
        for bad in [
            "file:///etc/passwd",
            "data:text/html,x",
            "javascript:alert(1)",
            "ftp://example.org/",
        ] {
            assert_eq!(
                next_hop(&secure, bad).unwrap_err().refusal,
                Some(Refusal::UrlScheme),
                "{bad}"
            );
        }
        assert_eq!(
            next_hop(&secure, "https://me:pw@example.org/")
                .unwrap_err()
                .refusal,
            Some(Refusal::UrlCredentials)
        );
        // Upward is the rule's to allow.
        assert_eq!(
            next_hop(&url("http://example.org/"), "https://example.org/")
                .unwrap()
                .scheme(),
            "https"
        );
    }

    #[test]
    fn a_release_asset_redirect_is_resolved_and_never_downgraded() {
        let from = url("https://github.com/o/r/releases/download/v1/x.AppImage");
        let to = next_hop(&from, "https://release-assets.githubusercontent.com/a?b=c").unwrap();
        let target = Target::of(&to).unwrap();
        assert_eq!(target.host, "release-assets.githubusercontent.com");
        assert_eq!(target.path, "/a?b=c");
        assert_eq!(
            next_hop(&from, "/other").unwrap().as_str(),
            "https://github.com/other"
        );
        assert_eq!(
            next_hop(&from, "http://github.com/x").unwrap_err().refusal,
            Some(Refusal::UrlScheme)
        );
    }

    /// FAILS IF: a download and a request disagree about an `http` address —
    /// one fetching over `https` what the other refuses.
    #[test]
    fn a_download_starts_where_a_request_starts() {
        assert_eq!(
            first_hop("http://commons.wikimedia.org/wiki/Special:FilePath/x.jpg")
                .unwrap()
                .as_str(),
            "https://commons.wikimedia.org/wiki/Special:FilePath/x.jpg"
        );
        assert_eq!(
            first_hop("http://coverartarchive.org/release/1/front")
                .unwrap()
                .scheme(),
            "https"
        );
        assert_eq!(
            first_hop("http://example.org/x").unwrap_err().refusal,
            Some(Refusal::UrlScheme)
        );
        assert_eq!(
            first_hop("http://commons.wikimedia.org:8080/x")
                .unwrap_err()
                .refusal,
            Some(Refusal::UrlScheme),
            "another port is another server"
        );
        assert_eq!(
            web::upgraded("http://commons.wikimedia.org/x"),
            first_hop("http://commons.wikimedia.org/x")
                .unwrap()
                .as_str(),
            "the request's first step is this one"
        );
    }

    #[test]
    fn only_https_with_a_host_and_no_credentials_is_fetched() {
        assert!(transport_of("https://example.org/").is_ok());
        let t = Target::of(
            &transport_of("https://api.github.com/repos/a/b/releases/latest?x=1").unwrap(),
        )
        .unwrap();
        assert_eq!((t.host.as_str(), t.port), ("api.github.com", 443));
        assert_eq!(t.path, "/repos/a/b/releases/latest?x=1");
        assert_eq!(
            transport_of("http://example.org/").unwrap_err().refusal,
            Some(Refusal::UrlScheme)
        );
        assert_eq!(
            transport_of("http://f-droid.org/repo/entry.jar")
                .unwrap_err()
                .refusal,
            Some(Refusal::UrlScheme)
        );
        assert_eq!(
            transport_of("https://me:pw@example.org/")
                .unwrap_err()
                .refusal,
            Some(Refusal::UrlCredentials)
        );
        assert_eq!(
            transport_of("file:///etc/passwd").unwrap_err().refusal,
            Some(Refusal::UrlScheme)
        );
        assert_eq!(
            transport_of("not an address").unwrap_err().refusal,
            Some(Refusal::UrlInvalid)
        );
        assert_eq!(
            Target::of(&transport_of("https://example.org:8443/x").unwrap())
                .unwrap()
                .port,
            8443
        );
    }

    #[test]
    fn a_redirect_is_a_redirect_only_with_somewhere_to_go() {
        assert_eq!(redirect_of(&reply(302, &[("Location", "/x")])), Some("/x"));
        assert_eq!(
            redirect_of(&reply(308, &[("location", "https://a/")])),
            Some("https://a/")
        );
        assert_eq!(redirect_of(&reply(302, &[])), None);
        assert_eq!(redirect_of(&reply(304, &[("Location", "/x")])), None);
        assert_eq!(redirect_of(&reply(200, &[("Location", "/x")])), None);
        assert!(keeps_the_body(307) && keeps_the_body(308));
        assert!(!keeps_the_body(302) && !keeps_the_body(303));
    }

    #[test]
    fn a_status_says_which_refusal_it_is() {
        assert_eq!(refusal_of_status(&reply(200, &[])), None);
        assert_eq!(refusal_of_status(&reply(204, &[])), None);
        assert_eq!(
            refusal_of_status(&reply(429, &[])),
            Some(Refusal::RateLimited)
        );
        assert_eq!(
            refusal_of_status(&reply(503, &[])),
            Some(Refusal::RateLimited)
        );
        // GitHub's sixty an hour, spent.
        assert_eq!(
            refusal_of_status(&reply(403, &[("X-RateLimit-Remaining", "0")])),
            Some(Refusal::RateLimited)
        );
        // A 403 with requests left is a plain refusal.
        assert_eq!(
            refusal_of_status(&reply(403, &[("X-RateLimit-Remaining", "12")])),
            Some(Refusal::ToolFailed)
        );
        // An id Flathub does not have, an asset that is gone.
        assert_eq!(
            refusal_of_status(&reply(404, &[])),
            Some(Refusal::UnknownItem)
        );
        assert_eq!(
            refusal_of_status(&reply(410, &[])),
            Some(Refusal::UnknownItem)
        );
        assert_eq!(
            refusal_of_status(&reply(500, &[])),
            Some(Refusal::ToolFailed)
        );
        // A 304 is only an answer to whoever asked conditionally.
        assert_eq!(
            refusal_of_status(&reply(304, &[])),
            Some(Refusal::ToolFailed)
        );
    }

    #[test]
    fn only_a_2xx_is_answered_and_the_trouble_names_the_status() {
        let got = |status| Got {
            url: "https://example.org/x".into(),
            reply: reply(status, &[]),
        };
        assert!(answered(got(200)).is_ok());
        let f = answered(got(404)).unwrap_err();
        assert_eq!(f.refusal, Some(Refusal::UnknownItem));
        assert_eq!(f.trouble, "https://example.org/x answered 404");
    }

    #[test]
    fn a_transport_error_says_which_refusal_it_is() {
        use std::io::{Error, ErrorKind};
        // The shapes the framer gives, built without a socket.
        let unreachable = anyhow::Error::new(Error::new(ErrorKind::ConnectionRefused, "refused"))
            .context("could not reach h:443");
        assert_eq!(refusal_of_error(&unreachable), Refusal::Offline);
        let unresolved = anyhow::Error::new(Error::other("failed to lookup address"))
            .context("`h` did not resolve");
        assert_eq!(refusal_of_error(&unresolved), Refusal::Offline);
        let timeout = anyhow::Error::new(Error::new(ErrorKind::TimedOut, "t"))
            .context("could not read h's answer");
        assert_eq!(refusal_of_error(&timeout), Refusal::Offline);
        // A certificate that does not verify is NOT "no network".
        let tls = anyhow::Error::new(Error::new(
            ErrorKind::InvalidData,
            "invalid peer certificate",
        ))
        .context("could not send the request to h");
        assert_eq!(refusal_of_error(&tls), Refusal::ToolFailed);
        assert_eq!(
            refusal_of_error(&anyhow::anyhow!("h answered more than 4096 bytes")),
            Refusal::TooLarge
        );
        assert_eq!(
            refusal_of_error(&anyhow::anyhow!("h announced 9 bytes, more than 4")),
            Refusal::TooLarge
        );
        assert_eq!(
            refusal_of_error(&anyhow::anyhow!("`a b` is not a server name")),
            Refusal::UrlInvalid
        );
        assert_eq!(
            refusal_of_error(&anyhow::anyhow!("`h` resolved to nothing")),
            Refusal::Offline
        );
        // A download its listener stopped.
        assert_eq!(
            refusal_of_error(&anyhow::anyhow!("stopped")),
            Refusal::ToolFailed
        );
        assert_eq!(
            refusal_of_error(&anyhow::anyhow!("no status line")),
            Refusal::ToolFailed
        );
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
        let t = Target::of(&url("https://example.org:8443")).unwrap();
        assert_eq!(
            (t.port, t.host_header.as_str(), t.path.as_str()),
            (8443, "example.org:8443", "/")
        );
        // IPv6: the certificate name has no brackets, the Host header has them.
        let t = Target::of(&url("https://[::1]:9/x")).unwrap();
        assert_eq!(
            (t.host.as_str(), t.host_header.as_str()),
            ("::1", "[::1]:9")
        );
    }

    #[test]
    fn the_user_agent_says_who_and_where() {
        assert!(USER_AGENT.starts_with("ronoctua-software/"));
        assert!(USER_AGENT.ends_with("(+https://github.com/ronoctua/ronoctua-distro)"));
        let h = headers_with("*/*", &[]).unwrap();
        assert!(
            h.contains(&("Accept-Encoding", "identity")),
            "a gzipped body would reach the decoder"
        );
        assert!(h.contains(&("User-Agent", USER_AGENT)));
        assert!(h.contains(&("Accept", "*/*")));
    }

    #[test]
    fn a_caller_adds_headers_and_replaces_only_accept() {
        let h = headers_with(
            "*/*",
            &[
                ("Accept", "application/vnd.github+json"),
                ("If-None-Match", "W/\"one\""),
            ],
        )
        .unwrap();
        let accepts: Vec<&str> = h
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case("accept"))
            .map(|(_, v)| *v)
            .collect();
        assert_eq!(
            accepts,
            ["application/vnd.github+json"],
            "one Accept, the caller's"
        );
        assert!(h.contains(&("If-None-Match", "W/\"one\"")));
        // What the framer or this module writes, and what would start a
        // header nobody wrote, is refused.
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
                headers_with("*/*", &[bad]).unwrap_err().refusal,
                Some(Refusal::ToolFailed),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_pace_beats_first_last_and_not_too_often() {
        let t0 = Instant::now();
        let mut p = Pace::default();
        assert!(p.due(t0, 0, 1000), "the bar has to appear");
        // Soon, and far: not yet (time).
        assert!(!p.due(t0 + Duration::from_millis(50), 500, 1000));
        // Late, and near: not yet (less than 1%).
        assert!(!p.due(t0 + Duration::from_millis(500), 5, 1000));
        // Late and far: yes.
        assert!(p.due(t0 + Duration::from_millis(500), 100, 1000));
        // The end always goes, however soon.
        assert!(p.due(t0 + Duration::from_millis(501), 1000, 1000));
        // Unknown total: time alone decides.
        let mut q = Pace::default();
        assert!(q.due(t0, 0, 0));
        assert!(!q.due(t0 + Duration::from_millis(99), 1, 0));
        assert!(q.due(t0 + Duration::from_millis(100), 2, 0));
    }

    #[test]
    fn beats_are_downloading_steps_named_by_the_caller_and_never_stop() {
        let mut report: Vec<crate::progress::Beat> = Vec::new();
        {
            let mut heard = beats(&mut report, "x.AppImage");
            assert!(heard(0, Some(10)));
            assert!(heard(1, Some(10)), "a beat that is not due still goes on");
            assert!(heard(10, Some(10)));
        }
        assert_eq!(
            report,
            [
                crate::progress::Beat::Step {
                    phase: Phase::Downloading,
                    done: 0,
                    total: 10,
                    now: "x.AppImage".into()
                },
                crate::progress::Beat::Step {
                    phase: Phase::Downloading,
                    done: 10,
                    total: 10,
                    now: "x.AppImage".into()
                },
            ]
        );
    }

    #[test]
    fn the_part_is_beside_the_file() {
        assert_eq!(
            part_of(Path::new("/d/x.AppImage")).unwrap(),
            Path::new("/d/x.AppImage.part")
        );
        assert!(part_of(Path::new("/")).is_err());
    }

    #[test]
    fn the_sink_hashes_and_counts_what_it_writes() {
        let mut s = Hashing::new(Vec::new());
        s.write_all(b"ab").unwrap();
        s.write_all(b"c").unwrap();
        assert_eq!(s.written, 3);
        assert_eq!(s.finish().unwrap(), crate::run::sha256(b"abc"));
    }

    #[test]
    fn a_malformed_digest_is_refused_before_anything_is_fetched() {
        let mut calls = 0;
        let dir =
            std::env::temp_dir().join(format!("ronoctua-software-net-{}", std::process::id()));
        let f = download(
            "https://example.invalid/x",
            &dir.join("x"),
            Some("nothex"),
            10,
            &mut |_, _| {
                calls += 1;
                true
            },
        )
        .unwrap_err();
        assert_eq!(f.refusal, Some(Refusal::ToolFailed));
        assert_eq!(calls, 0);
        assert!(!dir.exists(), "nothing was created for a refused download");
    }

    #[test]
    fn the_fake_network_is_shared_by_threads_and_answers_each_request_once() {
        let net = canned::Canned::new(vec![("https://a.example/x", 200, vec![], b"x".to_vec())]);
        std::thread::scope(|s| {
            let asker = s.spawn(|| net.post_json("https://a.example/x", "{}", 10));
            assert!(asker.join().unwrap().is_ok());
        });
        assert_eq!(
            net.get("https://a.example/x", &[], 10).unwrap_err().refusal,
            Some(Refusal::Offline),
            "a canned answer is given once"
        );
        assert_eq!(net.asked.borrow().len(), 2);
        assert_eq!(
            net.posted.borrow().as_slice(),
            [("https://a.example/x".to_owned(), "{}".to_owned())]
        );
    }
}
