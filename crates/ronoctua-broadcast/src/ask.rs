//! A question to a directory, before it is sent, and the one seam it crosses.

use ronoctua_consent::Pass;
use ronoctua_http::Reply;

use crate::sources::{Address, Refused};
use crate::{Refusal, Source, Why};

/// The most a JSON answer may be, by default. radio-browser's five hundred
/// stations are about seven hundred kilobytes; the archive's pages are a few.
pub const MOST_BYTES: u64 = 4 << 20;

/// One question to a directory.
///
/// No secret ever rides in one — the three directories take no key — so its
/// `Debug` is the whole request, and safe to log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Which directory is asked.
    pub source: Source,
    /// The address, `https`.
    pub url: String,
    /// Whether the answer may come gzipped (and is inflated by the web).
    pub gzip: bool,
    /// The most bytes of body, after inflating.
    pub most_bytes: u64,
}

impl Request {
    /// A GET of `url` from `source`: JSON, gzip, [`MOST_BYTES`].
    #[must_use]
    pub fn get(source: Source, url: impl Into<String>) -> Self {
        Self {
            source,
            url: url.into(),
            gzip: true,
            most_bytes: MOST_BYTES,
        }
    }

    /// The same, reading at most this many bytes.
    #[must_use]
    pub const fn most(mut self, most_bytes: u64) -> Self {
        self.most_bytes = most_bytes;
        self
    }
}

/// The network, as the directory modules use it.
///
/// `Sync`, because the daemon may ask two directories at once from threads
/// that share one.
pub trait Net: Sync {
    /// Send `request`, following redirects, and give back the last hop's
    /// reply whatever its status (reading a 404 is the directory module's).
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
    let source = request.source;
    let reply = net.send(request).map_err(|why| Refusal::new(source, why))?;
    match why_of_status(reply.status) {
        None => Ok(reply.body),
        Some(why) => Err(Refusal::new(source, why)),
    }
}

/// One GET a catalogue source asks: a default source's host, on the
/// person's consent.
///
/// Its address is a default source's (NASA's, Commons', SepiaSearch's), never
/// a person's: a person's own address is [`Address`], secret, and goes
/// through [`Web::get_own`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetch {
    /// The address, `https`.
    pub url: String,
    /// The media type asked for.
    pub accept: &'static str,
    /// Whether the answer may come gzipped.
    pub gzip: bool,
    /// The most bytes of body, after inflating.
    pub most_bytes: u64,
}

impl Fetch {
    /// A GET of `url`, JSON, gzip, [`MOST_BYTES`].
    #[must_use]
    pub fn json(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            accept: "application/json",
            gzip: true,
            most_bytes: MOST_BYTES,
        }
    }

    /// The same, reading at most this many bytes.
    #[must_use]
    pub const fn most(mut self, most_bytes: u64) -> Self {
        self.most_bytes = most_bytes;
        self
    }
}

/// The network, as the catalogue's sources use it: every request to a
/// default source's host carries the person's [`Pass`] for it, and a
/// person's own source is asked at its own address, which is its consent
/// (the person pasted it).
///
/// A second seam beside [`Net`], and not a method of it, because a `Net`
/// request names one of the three directories ([`Source`]) and a refusal of
/// theirs is a [`Refusal`] the doors already say; a catalogue source's is a
/// [`Refused`].
pub trait Web: Sync {
    /// Send `fetch` — whose first host `pass` covers — following redirects,
    /// and give back the last hop's reply whatever its status.
    ///
    /// # Errors
    ///
    /// [`Refused::NotLet`] when the pass does not cover the address's host,
    /// and the network's refusals.
    fn get(&self, pass: &Pass, fetch: &Fetch) -> Result<Reply, Refused>;

    /// GET a person's own source, at most `most_bytes`. Nothing about the
    /// address may reach a log: it often carries the person's account.
    ///
    /// # Errors
    ///
    /// The network's refusals, and [`Refused::PlainHttp`] for a redirect
    /// that would take an `https` address down to `http`. An `http` address
    /// itself is asked, in the clear: the person pasted it, and the door says
    /// it is not encrypted ([`crate::sources::is_plain`]).
    fn get_own(&self, address: &Address, most_bytes: u64) -> Result<Reply, Refused>;
}

/// The host of an address, lower-cased: what a [`Pass`] is checked against.
///
/// Read by the parser the web fetches with (`url`, the WHATWG rules), so the
/// host a pass is checked against is the host the request goes to:
/// `https://a.example\@b.example/` is `a.example`, as a browser and the web
/// read it, and never `b.example`, as a split at the `@` would. `None` for
/// what does not parse or names no host.
#[must_use]
pub fn host_of(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    parsed
        .host_str()
        .filter(|h| !h.is_empty())
        .map(str::to_ascii_lowercase)
}

/// Whether `pass` covers `url`'s host; checked BEFORE anything is sent, by
/// every source, whatever [`Web`] it is handed.
///
/// # Errors
///
/// [`Refused::NotLet`].
pub fn covered(pass: &Pass, url: &str) -> Result<(), Refused> {
    match host_of(url) {
        Some(host) if pass.covers(&host) => Ok(()),
        _ => Err(Refused::NotLet),
    }
}

/// Send `fetch` on `pass` and give back the body of a 2xx, or the refusal
/// its status is.
///
/// # Errors
///
/// [`Refused::NotLet`] (nothing is sent), and as [`Web::get`] and the
/// status's refusal.
pub fn fetched(web: &dyn Web, pass: &Pass, fetch: &Fetch) -> Result<Vec<u8>, Refused> {
    covered(pass, &fetch.url)?;
    let reply = web.get(pass, fetch)?;
    match why_of_status(reply.status) {
        None => Ok(reply.body),
        Some(why) => Err(Refused::from(why)),
    }
}

/// A person's own source at `address`, at most `most_bytes`: the body of a
/// 2xx, or the refusal its status is. The address is its own consent (the
/// person pasted it), so no [`Pass`] is asked.
///
/// # Errors
///
/// As [`Web::get_own`], and the status's refusal.
pub fn fetched_own(web: &dyn Web, address: &Address, most_bytes: u64) -> Result<Vec<u8>, Refused> {
    let reply = web.get_own(address, most_bytes)?;
    match why_of_status(reply.status) {
        None => Ok(reply.body),
        Some(why) => Err(Refused::from(why)),
    }
}

/// Which refusal a status is; `None` for a 2xx. Pure.
#[must_use]
pub const fn why_of_status(status: u16) -> Option<Why> {
    Some(match status {
        200..=299 => return None,
        404 | 410 => Why::NothingFound,
        429 => Why::TooManyAsks,
        500..=599 => Why::Unreachable,
        _ => Why::Declined,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_person_s_source_is_fetched_at_its_address_and_its_status_read() {
        let net = crate::canned::Canned::new(&[
            ("https://iptv.example/get.php?u=a&p=b", 200, b"#EXTM3U\n"),
            ("https://gone.example/list.m3u", 404, b""),
        ]);
        let list = Address::new("https://iptv.example/get.php?u=a&p=b");
        assert_eq!(fetched_own(&net, &list, 1024).unwrap(), b"#EXTM3U\n");
        let gone = Address::new("https://gone.example/list.m3u");
        assert_eq!(fetched_own(&net, &gone, 1024), Err(Refused::NothingFound));
    }

    #[test]
    fn a_host_is_read_out_of_an_address() {
        assert_eq!(
            host_of("https://Images-API.nasa.gov/search?q=x").as_deref(),
            Some("images-api.nasa.gov")
        );
        assert_eq!(
            host_of("http://user:pw@iptv.example:8080/get.php?u=a").as_deref(),
            Some("iptv.example")
        );
        assert_eq!(host_of("https://a.example").as_deref(), Some("a.example"));
        assert_eq!(host_of("no address"), None);
        assert_eq!(host_of("https://"), None);
        assert_eq!(host_of("https://a.example:port/"), None, "not a port");
        // THE HOST THE REQUEST GOES TO: a backslash ends the host for a
        // browser and for the web, and a pass for the host after the `@`
        // would otherwise let the one before it.
        assert_eq!(
            host_of("https://evil.example\\@images-api.nasa.gov/search").as_deref(),
            Some("evil.example")
        );
        assert_eq!(
            host_of("https:///path").as_deref(),
            Some("path"),
            "the WHATWG reading of extra slashes, the web's too"
        );
    }

    #[test]
    fn a_status_says_which_refusal_it_is() {
        assert_eq!(why_of_status(200), None);
        assert_eq!(why_of_status(206), None);
        assert_eq!(why_of_status(404), Some(Why::NothingFound));
        assert_eq!(why_of_status(410), Some(Why::NothingFound));
        assert_eq!(why_of_status(429), Some(Why::TooManyAsks));
        assert_eq!(why_of_status(503), Some(Why::Unreachable));
        assert_eq!(why_of_status(403), Some(Why::Declined));
        assert_eq!(
            why_of_status(301),
            Some(Why::Declined),
            "the web follows redirects"
        );
    }
}
