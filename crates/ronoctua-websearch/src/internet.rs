//! The one [`Net`] that reaches a host: the pass looked at, the host's pace,
//! then the web's walk under that pass (`ronoctua_http::web::request_let`).
//!
//! What it logs is the HOST and the kind of failure, never the address: the
//! address holds the person's words.

use ronoctua_consent::Pass;
use ronoctua_http::web;

use crate::{Answer, MOST_BYTES, Net, Request, USER_AGENT, Why};

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

    /// The network, stopped when `abort` is: a search waiting for its host's
    /// pace stops waiting, and one in flight has its socket shut. fast-access
    /// hands each keystroke's searches one and aborts it on the next key.
    #[must_use]
    pub const fn abortable(abort: web::Abort) -> Self {
        Self { abort: Some(abort) }
    }
}

impl Net for Internet {
    fn send(&self, pass: &Pass, request: &Request) -> Result<Answer, Why> {
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
            .key
            .iter()
            .map(|(name, value)| (*name, value.expose()))
            .collect();
        let mut ask = web::Ask::get(&request.url, USER_AGENT)
            .with_headers(&headers)
            .accepting("application/json")
            .most(MOST_BYTES)
            .coded(web::Coding::Gzip);
        if let Some(abort) = &self.abort {
            ask = ask.abortable(abort);
        }
        match web::request_let(pass, &ask) {
            Ok(got) => Ok(Answer {
                status: got.reply.status,
                body: got.reply.body,
            }),
            Err(e) => {
                tracing::info!(host = %request.host, kind = ?e.kind, "a search went unanswered");
                Err(why_of_kind(e.kind))
            }
        }
    }
}

/// Which [`Why`] a web failure is. Pure.
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
    use crate::canned::pass_for;

    #[test]
    fn the_machines_net_refuses_another_hosts_pass_and_an_aborted_one_sends_nothing() {
        let pass = pass_for("websearch-abort.example");
        let request = Request {
            host: "other.example".to_owned(),
            url: "https://other.example/x".to_owned(),
            key: None,
        };
        assert_eq!(
            Internet::new().send(&pass, &request).unwrap_err(),
            Why::WrongPass
        );
        let abort = web::Abort::new();
        abort.abort();
        let request = Request {
            host: "websearch-abort.example".to_owned(),
            url: "https://websearch-abort.example/x".to_owned(),
            key: None,
        };
        assert_eq!(
            Internet::abortable(abort)
                .send(&pass, &request)
                .unwrap_err(),
            Why::Unreachable
        );
        assert_eq!(why_of_kind(web::Kind::TooLarge), Why::Garbled);
    }
}
