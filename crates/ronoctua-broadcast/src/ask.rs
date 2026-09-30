//! A question to a directory, before it is sent, and the one seam it crosses.

use ronoctua_http::Reply;

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
