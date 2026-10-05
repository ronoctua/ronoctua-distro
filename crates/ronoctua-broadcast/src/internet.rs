//! The one [`Net`] (and [`Web`]) that reaches a host: the person's pass,
//! then the web's walk, with gzip when asked.
//!
//! # Nothing is asked without the person's pass
//!
//! A directory's request carries a [`Pass`] minted from `consent.json` when
//! it is sent: for its own host (`web::request_let`), or for the name its
//! consent row lets ([`crate::Source::host`]) when the host is one the
//! directory named under that domain at run time — radio-browser's servers
//! under `radio-browser.info` (`web::request_under`). The player lets that
//! name there when a directory's row is pressed.
//!
//! A person's own source ([`Web::get_own`]) is the one request here without
//! a pass, and it cannot carry one: pasting the address is its consent; an
//! IPTV list is `http` more often than not, which no pass's walk takes; and
//! the pictures a source names come from whatever host it names, which
//! nobody could let before reading it. `network-asks-with-a-pass` names that
//! function, and only it, with why.

use ronoctua_consent::{NotLet, Pass};
use ronoctua_http::Reply;
use ronoctua_http::web;

use crate::ask::{Fetch, Net, Request, Web, covered};
use crate::sources::{Address, Refused};
use crate::{USER_AGENT, Why};

/// The machine's network. Every send waits for the host: call it on a thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct Internet;

/// How a directory's request is let.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Let {
    /// By a pass for its own host.
    Own(Pass),
    /// By a pass for the name its directory's row lets, to a host under it.
    Under(Pass),
}

/// The pass `request` goes out with, minted by `mint` — the person's file on
/// the machine, a test's own in a test. Pure over `mint`.
///
/// # Errors
///
/// [`Why::Malformed`] for an address the web would refuse anyway (nothing is
/// minted for it), [`Why::NotLet`] when neither its host nor its directory's
/// domain is let.
fn let_of(request: &Request, mint: &dyn Fn(&str) -> Result<Pass, NotLet>) -> Result<Let, Why> {
    let first = web::transport_of(&web::upgraded(&request.url)).map_err(|e| why_of_kind(e.kind))?;
    let host = first.host_str().ok_or(Why::Malformed)?;
    if let Ok(pass) = mint(host) {
        return Ok(Let::Own(pass));
    }
    let domain = request.source.host();
    if web::is_under(host, domain)
        && let Ok(pass) = mint(domain)
    {
        return Ok(Let::Under(pass));
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
        let let_ = let_of(request, mint).inspect_err(|why| {
            if *why == Why::NotLet {
                tracing::info!(
                    source = request.source.id(),
                    "a directory's host is not let"
                );
            }
        })?;
        let ask = web::Ask::get(&request.url, USER_AGENT)
            .accepting("application/json")
            .most(request.most_bytes)
            .coded(if request.gzip {
                web::Coding::Gzip
            } else {
                web::Coding::Identity
            });
        let sent = match &let_ {
            Let::Own(pass) => web::request_let(pass, &ask),
            Let::Under(pass) => web::request_under(pass, request.source.host(), &ask),
        };
        match sent {
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

impl Web for Internet {
    fn get(&self, pass: &Pass, fetch: &Fetch) -> Result<Reply, Refused> {
        // THE PASS AGAIN, here and not only in `fetched`: this is the call
        // that reaches the host, and it is the one that must not be made
        // without the person's consent.
        covered(pass, &fetch.url)?;
        let ask = web::Ask::get(&fetch.url, USER_AGENT)
            .accepting(fetch.accept)
            .most(fetch.most_bytes)
            .coded(if fetch.gzip {
                web::Coding::Gzip
            } else {
                web::Coding::Identity
            });
        // THROUGH THE PASS where the bytes leave, too: a redirect to another
        // host carries none of this one's consent.
        match web::request_let(pass, &ask) {
            Ok(got) => Ok(got.reply),
            Err(e) => {
                tracing::info!(host = pass.host(), kind = ?e.kind, "a catalogue source got no answer");
                Err(refused_of_kind(e.kind))
            }
        }
    }

    fn get_own(&self, address: &Address, most_bytes: u64) -> Result<Reply, Refused> {
        let ask = web::Ask::get(address.expose(), USER_AGENT)
            .accepting("*/*")
            .most(most_bytes)
            .coded(web::Coding::Gzip);
        // A PERSON'S `http` LIST IS TAKEN, in the clear (the owner's
        // decision, 2026-09-30): an IPTV provider's is `http` more often than
        // not. The door says it is not encrypted beside the source
        // (`crate::sources::is_plain`); a default source is never asked so.
        let sent = match own_transport(address.expose()) {
            OwnTransport::Plain => web::request_unencrypted(&ask),
            OwnTransport::Tls => web::request(&ask),
        };
        match sent {
            Ok(got) => Ok(got.reply),
            // THE KIND AND NOTHING ELSE: the error's words name the address,
            // and a person's address carries their account.
            Err(e) => {
                tracing::info!(kind = ?e.kind, "a person's source got no answer");
                Err(refused_of_kind(e.kind))
            }
        }
    }
}

/// How a person's own address is asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnTransport {
    /// `https`, over TLS, as every request.
    Tls,
    /// `http`, NOT ENCRYPTED: taken for a person's source alone.
    Plain,
}

/// How `url`, a person's own address, is asked. Pure.
#[must_use]
pub fn own_transport(url: &str) -> OwnTransport {
    if crate::sources::is_plain(url) {
        OwnTransport::Plain
    } else {
        OwnTransport::Tls
    }
}

/// Which [`Refused`] a web failure is. Pure.
///
/// Like [`why_of_kind`], with one more distinction a person's source needs:
/// an `http` address is refused by the web's rule, and the door says so
/// rather than "malformed".
#[must_use]
pub const fn refused_of_kind(kind: web::Kind) -> Refused {
    match kind {
        web::Kind::Offline | web::Kind::Certificate | web::Kind::Failed => Refused::Unreachable,
        web::Kind::TooLarge => Refused::TooLarge,
        web::Kind::Scheme => Refused::PlainHttp,
        web::Kind::Credentials => Refused::NotAnAddress,
        web::Kind::Invalid | web::Kind::Header | web::Kind::Loop | web::Kind::Coding => {
            Refused::Malformed
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
    fn a_web_failure_is_a_catalogue_source_s_refusal() {
        assert_eq!(refused_of_kind(web::Kind::Offline), Refused::Unreachable);
        assert_eq!(refused_of_kind(web::Kind::Scheme), Refused::PlainHttp);
        assert_eq!(
            refused_of_kind(web::Kind::Credentials),
            Refused::NotAnAddress
        );
        assert_eq!(refused_of_kind(web::Kind::TooLarge), Refused::TooLarge);
    }

    /// FAILS IF: a request to a default source's host goes out on a pass for
    /// another host — the consent the person gave one host would let every
    /// other.
    #[test]
    fn a_pass_for_one_host_sends_nothing_to_another() {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-broadcast-internet-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let store = ronoctua_consent::store_at(Some(dir.clone()));
        ronoctua_consent::let_hosts_in(&store, &["sepiasearch.org"], true).unwrap();
        let pass = ronoctua_consent::pass_in(&store, "sepiasearch.org").unwrap();
        let fetch = Fetch::json("https://images-api.nasa.gov/search?q=x");
        assert_eq!(Internet.get(&pass, &fetch).unwrap_err(), Refused::NotLet);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a person's `http` list is refused again instead of asked in
    /// the clear, or an `https` one stops going over TLS.
    /// A pass minted from a consent file of the test's own, letting `hosts`.
    fn minter(hosts: &[&str], tag: &str) -> impl Fn(&str) -> Result<Pass, NotLet> {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-broadcast-let-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let store = ronoctua_consent::store_at(Some(dir));
        ronoctua_consent::let_hosts_in(&store, hosts, true).unwrap();
        move |host| ronoctua_consent::pass_in(&store, host)
    }

    /// FAILS IF: a directory is asked on a host nobody let, a server
    /// radio-browser names under its domain is refused once the domain is
    /// let, or a host BESIDE the domain passes for one under it.
    #[test]
    fn a_directory_goes_out_only_with_a_pass() {
        let mint = minter(&["radio-browser.info"], "dirs");
        let servers = Request::get(
            crate::Source::Radio,
            "https://all.api.radio-browser.info/json/servers",
        );
        assert!(matches!(
            let_of(&servers, &mint),
            Ok(Let::Under(p)) if p.host() == "radio-browser.info"
        ));
        let beside = Request::get(crate::Source::Radio, "https://evil-radio-browser.info/json");
        assert_eq!(let_of(&beside, &mint), Err(Why::NotLet));
        let tv = Request::get(
            crate::Source::Iptv,
            "https://iptv-org.github.io/api/channels.json",
        );
        assert_eq!(let_of(&tv, &mint), Err(Why::NotLet));
        assert_eq!(Internet.send_minted(&tv, &mint).unwrap_err(), Why::NotLet);
        let tv_let = minter(&[crate::iptv::HOST], "tv");
        assert!(matches!(let_of(&tv, &tv_let), Ok(Let::Own(_))));
    }

    #[test]
    fn a_person_s_http_address_is_asked_in_the_clear() {
        assert_eq!(
            own_transport("http://iptv.example/get.php?username=a&password=b"),
            OwnTransport::Plain
        );
        assert_eq!(
            own_transport("HTTP://iptv.example/list.m3u"),
            OwnTransport::Plain
        );
        assert_eq!(
            own_transport("https://iptv.example/list.m3u"),
            OwnTransport::Tls
        );
        // AN ADDRESS OF NO SCHEME the web reads is not plain: the ordinary
        // walk refuses it, never the clear one.
        assert_eq!(own_transport("iptv.example/list.m3u"), OwnTransport::Tls);
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
