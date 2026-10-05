//! The one [`Net`] that reaches a host: the person's pass, the host's pace,
//! then the web's walk.
//!
//! # Nothing is asked without the person's pass
//!
//! Every request carries a [`Pass`] minted from `consent.json` at the moment
//! it is sent ([`ronoctua_consent::pass`]): for its own host, sent with
//! `web::request_let`; or, for a host a service hands out under its domain
//! ([`crate::Service::domain`] — OpenSubtitles' session and download hosts),
//! a pass for the host its consent row names (`api.opensubtitles.com`), sent
//! with `web::request_under` and that domain. What the person lets there is
//! the hosts of a service ([`crate::Service::hosts`]) — the player writes
//! them when its row is pressed; a host not let is [`Why::NotLet`], and
//! nothing waits for its pace or leaves the machine.
//!
//! What it logs is the HOST and the kind of failure, never the web's trouble
//! words: those name the address, and SubDL's key is in its address.

use ronoctua_consent::{NotLet, Pass};
use ronoctua_http::Reply;
use ronoctua_http::web;

use crate::ask::{Net, Request};
use crate::{USER_AGENT, Why};

/// The machine's network. Every send waits for its host's pace and then for
/// the host: call it on a thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct Internet;

/// How a request is let.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Let {
    /// By a pass for its own host.
    Own(Pass),
    /// By a pass for its service's own host, to a host under the domain the
    /// service hands addresses out under.
    Under(Pass, &'static str),
}

/// The pass `url` goes out with, minted by `mint` — the person's file on the
/// machine, a test's own file in a test. Pure over `mint`.
///
/// # Errors
///
/// [`Why::Malformed`] for an address the web would refuse anyway (nothing is
/// minted for it), [`Why::NotLet`] when the host is not let, nor — for a host
/// under its service's domain — the host the service's row names.
fn let_of(
    request: &Request,
    url: &str,
    mint: &dyn Fn(&str) -> Result<Pass, NotLet>,
) -> Result<Let, Why> {
    let first = web::transport_of(&web::upgraded(url)).map_err(|e| why_of_kind(e.kind))?;
    let host = first.host_str().ok_or(Why::Malformed)?;
    if let Ok(pass) = mint(host) {
        return Ok(Let::Own(pass));
    }
    if let Some(domain) = request.service().domain()
        && web::is_under(host, domain)
        && let Ok(pass) = mint(request.host.name())
    {
        return Ok(Let::Under(pass, domain));
    }
    Err(Why::NotLet)
}

impl Net for Internet {
    fn send(&self, request: &Request) -> Result<Reply, Why> {
        self.send_minted(request, &ronoctua_consent::pass)
    }
}

impl Internet {
    /// [`Net::send`], with the passes minted by `mint`.
    fn send_minted(
        self,
        request: &Request,
        mint: &dyn Fn(&str) -> Result<Pass, NotLet>,
    ) -> Result<Reply, Why> {
        let url = request.exposed_url();
        // THE PASS BEFORE THE PACE: a host the person did not let neither
        // waits its turn nor is asked.
        let let_ = let_of(request, &url, mint).inspect_err(|why| {
            if *why == Why::NotLet {
                tracing::info!(host = request.host.name(), "a lookup's host is not let");
            }
        })?;
        // HELD UNTIL THE ANSWER: the next request to this host is paced from
        // when this one ended, not only from when it left (`pace::Turn`).
        let _turn = crate::pace::of(request.host).wait();
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
        let sent = match &let_ {
            Let::Own(pass) => web::request_let(pass, &ask),
            Let::Under(pass, domain) => web::request_under(pass, domain, &ask),
        };
        match sent {
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

    /// A pass minted from a consent file of the test's own, letting `hosts`.
    fn minter(hosts: &[&str], tag: &str) -> impl Fn(&str) -> Result<Pass, NotLet> {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-lookup-pass-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = ronoctua_consent::store_at(Some(dir));
        ronoctua_consent::let_hosts_in(&store, hosts, true).unwrap();
        move |host| ronoctua_consent::pass_in(&store, host)
    }

    /// FAILS IF: a lookup goes out for a host the person did not let — a
    /// service's own host, or a host under a domain that is not the
    /// service's — or the handed-out hosts of a let service are refused.
    #[test]
    fn a_lookup_goes_out_only_with_a_pass() {
        let mint = minter(&["lrclib.net", "api.opensubtitles.com"], "let");
        let lyrics = Request::get(crate::Host::Lrclib, "https://lrclib.net/api/get");
        assert!(matches!(
            let_of(&lyrics, &lyrics.url, &mint),
            Ok(Let::Own(p)) if p.host() == "lrclib.net"
        ));
        let brainz = Request::get(crate::Host::MusicBrainz, "https://musicbrainz.org/ws/2/x");
        assert_eq!(let_of(&brainz, &brainz.url, &mint), Err(Why::NotLet));
        // OpenSubtitles hands a session host and a download link out under
        // its domain: the pass of the host its row names lets them.
        for url in [
            "https://vip-api.opensubtitles.com/api/v1/download",
            "https://www.opensubtitles.com/download/abc/x.srt",
        ] {
            let r = Request::get(crate::Host::OpenSubtitles, url);
            assert!(
                matches!(
                    let_of(&r, url, &mint),
                    Ok(Let::Under(p, "opensubtitles.com")) if p.host() == "api.opensubtitles.com"
                ),
                "{url}"
            );
        }
        // Beside the domain, and another service's request under it: no.
        let beside = Request::get(
            crate::Host::OpenSubtitles,
            "https://evil-opensubtitles.com/x",
        );
        assert_eq!(let_of(&beside, &beside.url, &mint), Err(Why::NotLet));
        let other = Request::get(crate::Host::Lrclib, "https://www.opensubtitles.com/x");
        assert_eq!(let_of(&other, &other.url, &mint), Err(Why::NotLet));
        // NOTHING LET, nothing sent — through the Net itself.
        let none = minter(&[], "none");
        assert_eq!(
            Internet.send_minted(&brainz, &none).unwrap_err(),
            Why::NotLet
        );
    }

    #[test]
    fn a_request_to_an_address_the_rule_refuses_sends_nothing() {
        // `http` to a host that is not upgraded: refused by the web before
        // any socket, and — the host's pace aside — before any wait.
        let r = Request::get(crate::Host::Lrclib, "http://lrclib.net/api/get");
        assert_eq!(Internet.send(&r).unwrap_err(), Why::Malformed);
    }
}
