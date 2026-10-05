//! The one [`Net`] that reaches a host: the pass looked at, the host's pace,
//! then the web's walk under that pass (`ronoctua_http::web::request_let`).
//!
//! What it logs is the HOST and the kind of failure, never the web's trouble
//! words: those name the address, and the address holds the person's text.

use ronoctua_consent::Pass;
use ronoctua_http::Reply;
use ronoctua_http::web;

use crate::ask::{Method, Net, Request};
use crate::{USER_AGENT, Why};

/// The machine's network. Every send waits for its host's pace and then for
/// the host: call it on a thread.
#[derive(Debug, Clone, Default)]
pub struct Internet {
    abort: Option<web::Abort>,
}

impl Internet {
    /// The network, with nothing to stop it but its patience.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The network, stopped when `abort` is: a request waiting for its
    /// host's pace stops waiting, and one in flight has its socket shut. The
    /// search app hands each keystroke's question one and aborts it on the
    /// next key.
    #[must_use]
    pub const fn abortable(abort: web::Abort) -> Self {
        Self { abort: Some(abort) }
    }
}

impl Net for Internet {
    fn send(&self, pass: &Pass, request: &Request) -> Result<Reply, Why> {
        if !pass.covers(&request.host) {
            return Err(Why::WrongPass);
        }
        let stopped = || self.abort.as_ref().is_some_and(web::Abort::is_aborted);
        crate::pace::shared()
            .wait(&request.host, &stopped)
            .map_err(|waited| match waited {
                crate::pace::Waited::TooLong => Why::Paced,
                crate::pace::Waited::Stopped => Why::Unreachable,
            })?;
        let headers: Vec<(&str, &str)> = request
            .headers
            .iter()
            .map(|(name, value)| (*name, value.expose()))
            .collect();
        let mut ask = web::Ask::get(&request.url, USER_AGENT)
            .with_headers(&headers)
            .accepting(request.accept)
            .most(request.most_bytes)
            .coded(web::Coding::Gzip);
        if let (Method::Post, Some(body)) = (request.method, &request.body) {
            ask = ask.posting(body.expose());
        }
        if let Some(abort) = &self.abort {
            ask = ask.abortable(abort);
        }
        match web::request_let(pass, &ask) {
            Ok(got) => Ok(got.reply),
            Err(e) => {
                tracing::info!(host = %request.host, kind = ?e.kind, "a word went unanswered");
                Err(why_of_kind(e.kind))
            }
        }
    }
}

/// Which [`Why`] a web failure is. Pure.
///
/// The network not being there, TLS failing on the way, or the person having
/// moved on (an abort) is the host being unreachable; an address the rule
/// refuses is a failure; an answer too large, looping or in a coding that is
/// not read is garbled.
#[must_use]
pub const fn why_of_kind(kind: web::Kind) -> Why {
    match kind {
        web::Kind::Offline | web::Kind::Certificate | web::Kind::Failed => Why::Unreachable,
        web::Kind::Invalid | web::Kind::Scheme | web::Kind::Credentials | web::Kind::Header => {
            Why::Failed
        }
        web::Kind::TooLarge | web::Kind::Loop | web::Kind::Coding => Why::Garbled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ask::canned::pass_for;

    #[test]
    fn a_web_failure_is_a_words_refusal() {
        assert_eq!(why_of_kind(web::Kind::Offline), Why::Unreachable);
        assert_eq!(why_of_kind(web::Kind::Failed), Why::Unreachable);
        assert_eq!(why_of_kind(web::Kind::Scheme), Why::Failed);
        assert_eq!(why_of_kind(web::Kind::TooLarge), Why::Garbled);
    }

    /// FAILS IF: the machine's net sends under a pass for another host.
    #[test]
    fn the_machines_net_refuses_a_pass_for_another_host_before_anything() {
        let pass = pass_for("en.wiktionary.org");
        let r = Request::get(
            "api.mymemory.translated.net",
            "https://api.mymemory.translated.net/get?q=x",
        );
        assert_eq!(Internet::new().send(&pass, &r).unwrap_err(), Why::WrongPass);
    }

    #[test]
    fn an_aborted_net_sends_nothing() {
        let pass = pass_for("words-abort.example");
        let abort = web::Abort::new();
        abort.abort();
        let r = Request::get("words-abort.example", "https://words-abort.example/x");
        assert_eq!(
            Internet::abortable(abort).send(&pass, &r).unwrap_err(),
            Why::Unreachable
        );
    }
}
