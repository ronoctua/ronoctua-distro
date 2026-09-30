//! The one [`Net`] that reaches a host: the host's pace, then the web's walk.
//!
//! What it logs is the HOST and the kind of failure, never the web's trouble
//! words: those name the address, and SubDL's key is in its address.

use ronoctua_http::Reply;
use ronoctua_http::web;

use crate::ask::{Net, Request};
use crate::{USER_AGENT, Why};

/// The machine's network. Every send waits for its host's pace and then for
/// the host: call it on a thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct Internet;

impl Net for Internet {
    fn send(&self, request: &Request) -> Result<Reply, Why> {
        crate::pace::of(request.host).wait();
        let url = request.exposed_url();
        let headers: Vec<(&str, &str)> = request
            .headers
            .iter()
            .map(|(name, value)| (*name, value.expose()))
            .collect();
        let mut ask = web::Ask::get(&url, USER_AGENT)
            .with_headers(&headers)
            .accepting(request.accept)
            .most(request.most_bytes)
            .coded(if request.gzip {
                web::Coding::Gzip
            } else {
                web::Coding::Identity
            });
        if let Some(body) = &request.body {
            ask = ask.posting(body.expose());
        }
        match web::request(&ask) {
            Ok(got) => Ok(got.reply),
            Err(e) => {
                let why = why_of_kind(e.kind);
                tracing::info!(host = request.host.name(), kind = ?e.kind, "a lookup got no answer");
                Err(why)
            }
        }
    }
}

/// Which [`Why`] a web failure is. Pure.
///
/// The network not being there, or TLS failing on the way, is the service
/// being unreachable; more than the ceiling is too large; everything else —
/// an address or a redirect the rule refuses, a loop, a coding that is not
/// read — is an answer this does not read.
#[must_use]
pub const fn why_of_kind(kind: web::Kind) -> Why {
    match kind {
        web::Kind::Offline | web::Kind::Certificate | web::Kind::Failed => Why::Unreachable,
        web::Kind::TooLarge => Why::TooLarge,
        web::Kind::Invalid
        | web::Kind::Scheme
        | web::Kind::Credentials
        | web::Kind::Header
        | web::Kind::Loop
        | web::Kind::Coding => Why::Malformed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_web_failure_is_a_lookups_refusal() {
        assert_eq!(why_of_kind(web::Kind::Offline), Why::Unreachable);
        assert_eq!(why_of_kind(web::Kind::Certificate), Why::Unreachable);
        assert_eq!(why_of_kind(web::Kind::TooLarge), Why::TooLarge);
        assert_eq!(why_of_kind(web::Kind::Scheme), Why::Malformed);
        assert_eq!(why_of_kind(web::Kind::Loop), Why::Malformed);
    }

    #[test]
    fn a_request_to_an_address_the_rule_refuses_sends_nothing() {
        // `http` to a host that is not upgraded: refused by the web before
        // any socket, and — the host's pace aside — before any wait.
        let r = Request::get(crate::Host::Lrclib, "http://lrclib.net/api/get");
        assert_eq!(Internet.send(&r).unwrap_err(), Why::Malformed);
    }
}
