//! The one [`Net`] that reaches a host: the web's walk, with gzip when asked.

use ronoctua_http::Reply;
use ronoctua_http::web;

use crate::ask::{Net, Request};
use crate::{USER_AGENT, Why};

/// The machine's network. Every send waits for the host: call it on a thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct Internet;

impl Net for Internet {
    fn send(&self, request: &Request) -> Result<Reply, Why> {
        let ask = web::Ask::get(&request.url, USER_AGENT)
            .accepting("application/json")
            .most(request.most_bytes)
            .coded(if request.gzip {
                web::Coding::Gzip
            } else {
                web::Coding::Identity
            });
        match web::request(&ask) {
            Ok(got) => Ok(got.reply),
            Err(e) => {
                tracing::info!(
                    source = request.source.id(),
                    url = %request.url,
                    kind = ?e.kind,
                    "a directory got no answer"
                );
                Err(why_of_kind(e.kind))
            }
        }
    }
}

/// Which [`Why`] a web failure is. Pure.
///
/// The network not being there, or TLS failing on the way, is the host being
/// unreachable; more than the ceiling is too large; everything else — an
/// address or a redirect the rule refuses, a loop, a coding that is not read —
/// is an answer this does not read.
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
    fn a_web_failure_is_a_directorys_refusal() {
        assert_eq!(why_of_kind(web::Kind::Offline), Why::Unreachable);
        assert_eq!(why_of_kind(web::Kind::Certificate), Why::Unreachable);
        assert_eq!(why_of_kind(web::Kind::TooLarge), Why::TooLarge);
        assert_eq!(why_of_kind(web::Kind::Scheme), Why::Malformed);
    }

    #[test]
    fn an_address_the_rule_refuses_sends_nothing() {
        let r = Request::get(
            crate::Source::Radio,
            "http://all.api.radio-browser.info/json/servers",
        );
        assert_eq!(Internet.send(&r).unwrap_err(), Why::Malformed);
    }
}
