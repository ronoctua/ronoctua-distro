//! The two hosts this crate fetches from, and nothing else.
//!
//! # What used to be here, and where it went
//!
//! The whole TLS stack: the roots, the provider, the session, the connect with
//! a deadline. It was the first in this workspace and its header explained
//! why — *"`aria2` talks to `127.0.0.1`, and this talks to somebody else's
//! machine, where there is no version of 'no TLS' that is acceptable."*
//!
//! That argument was never about the world clock. ClipCascade's server is also
//! somebody else's machine, and a second rustls setup beside this one is the
//! shape of defect `ronoctua-http` was carved out to prevent. So the transport
//! moved to [`ronoctua_http::tls`], behind a feature so the three crates that
//! only ever open a loopback socket do not compile it.
//!
//! What stayed is this crate's POLICY, which is the part that is actually
//! about a world clock: two hosts, `GET` only, JSON, and a status that has to
//! be exactly 200.
//!
//! # What it does NOT do
//!
//! Redirects, proxies, keep-alive, compression, cookies, authentication. Two
//! hosts answer a `GET` with JSON and `Connection: close`. Anything else is a
//! problem that has not shown up.

use anyhow::{Result, bail};

/// How long a host on the internet is given.
///
/// The transport's, and not a second number: a deadline that differed from the
/// one the socket is actually set to would be a comment, not a rule.
pub use ronoctua_http::tls::PATIENCE;

/// `GET https://<host><path>`, and the body that came back.
///
/// # Errors
///
/// The name does not resolve, the host did not answer inside [`PATIENCE`], the
/// certificate did not verify, the status was not 200, or the body was larger
/// than the transport is willing to read.
pub fn get(host: &str, path: &str) -> Result<Vec<u8>> {
    let call = ronoctua_http::Call::new("GET", path, host)
        .with_headers(&[("User-Agent", "ronoctua"), ("Accept", "application/json")])
        .waiting(PATIENCE);
    let answer = ronoctua_http::tls::over_tls(host, 443, &call)?;
    // EXACTLY 200, and not `went_through`. These two APIs answer 200 or
    // nothing useful; a 204 with an empty body would otherwise reach the
    // parser as a syntax error instead of as "the host said no".
    if answer.status != 200 {
        bail!("{host} answered {}", answer.status);
    }
    Ok(answer.body.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing here reaches the network, and that is deliberate: a test that
    /// needs the internet is a test that fails in the sandbox this project
    /// builds in. What is worth holding is the SHAPE of the request.
    #[test]
    fn the_patience_is_the_transports_and_longer_than_a_local_sockets() {
        assert_eq!(PATIENCE, ronoctua_http::tls::PATIENCE);
        assert!(PATIENCE > ronoctua_http::PATIENCE);
    }

    #[test]
    fn the_request_says_json_and_names_this_program() {
        // The two APIs are content-negotiating, and `frankfurter` answers HTML
        // to a request that does not ask for JSON — which reaches the parser
        // as a syntax error at character one.
        let call = ronoctua_http::Call::new("GET", "/v1/latest?base=GBP", "api.frankfurter.dev")
            .with_headers(&[("User-Agent", "ronoctua"), ("Accept", "application/json")]);
        let text = ronoctua_http::request_of(&call);
        assert!(text.contains("Accept: application/json\r\n"), "{text}");
        assert!(text.contains("User-Agent: ronoctua\r\n"), "{text}");
        assert!(text.contains("Host: api.frankfurter.dev\r\n"), "{text}");
    }
}
