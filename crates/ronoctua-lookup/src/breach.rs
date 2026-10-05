//! XposedOrNot: whether an e-mail address is in a known breach — keyless.
//!
//! # The address is SENT, and the door says so
//!
//! There is no anonymity here, unlike the passwords ([`crate::pwned`]): the
//! question is `GET https://api.xposedornot.com/v1/check-email/{address}`,
//! and XposedOrNot sees the address it is asked about. So the security
//! centre's row says, before the person types it, that the address goes to
//! XposedOrNot; the person typing it and pressing `RET` is the consent, and
//! the door records it for the host before it asks. Its pace is one a second,
//! as the service asks.
//!
//! Have I Been Pwned answers the same question and wants a paid key for it;
//! the centre offers its WEBSITE instead, whose address carries no e-mail.

use serde::Deserialize;

use crate::ask::{Net, Request, answered, object};
use crate::query::escape;
use crate::{Host, Refusal, Service, Why};

/// The service's origin.
pub const API: &str = "https://api.xposedornot.com/v1";

/// Have I Been Pwned's own page, for the person to open in a browser: no
/// address in it, ever.
pub const HIBP_PAGE: &str = "https://haveibeenpwned.com/";

/// The most names of breaches kept from one answer.
pub const NAMES_MOST: usize = 200;

/// The longest address (RFC 5321's path limit).
pub const ADDRESS_MOST: usize = 254;

/// The address as it is asked: trimmed, and `None` for anything that is not
/// one plain address — one `@`, something on each side, a dot in the domain,
/// no space, no control character and nothing that would end a path segment.
#[must_use]
pub fn address_of(typed: &str) -> Option<String> {
    let a = typed.trim();
    let (local, domain) = a.split_once('@')?;
    let plain = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| !c.is_whitespace() && !c.is_control() && !"/?#@%\\\"".contains(c))
    };
    let fits = a.len() <= ADDRESS_MOST
        && plain(local)
        && plain(domain)
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.');
    fits.then(|| a.to_owned())
}

/// The question. Pure.
///
/// # Errors
///
/// [`Why::NothingToAsk`] for text that is not one address.
pub fn request(typed: &str) -> Result<Request, Refusal> {
    let address = address_of(typed).ok_or(Refusal::new(Service::XposedOrNot, Why::NothingToAsk))?;
    let (local, domain) = address.split_once('@').unwrap_or_default();
    Ok(Request::get(
        Host::XposedOrNot,
        format!("{API}/check-email/{}@{}", escape(local), escape(domain)),
    ))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Answer {
    breaches: Vec<Vec<String>>,
}

/// The breaches' names out of an answer: the service nests them one level
/// (`[["Adobe", "LinkedIn"]]`). Pure.
///
/// # Errors
///
/// [`Why::Malformed`].
pub fn read(body: &[u8]) -> Result<Vec<String>, Refusal> {
    let answer: Answer = object(Service::XposedOrNot, body)?;
    let mut names: Vec<String> = answer
        .breaches
        .into_iter()
        .flatten()
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty() && !n.chars().any(char::is_control))
        .collect();
    names.sort();
    names.dedup();
    names.truncate(NAMES_MOST);
    Ok(names)
}

/// The breaches the address is in; empty for none (the service answers 404
/// then). Blocks: a thread.
///
/// # Errors
///
/// [`Why::NothingToAsk`], [`Why::NotLet`], and the service's refusals.
pub fn check(net: &dyn Net, typed: &str) -> Result<Vec<String>, Refusal> {
    match answered(net, &request(typed)?) {
        Ok(body) => read(&body),
        Err(r) if r.why == Why::NothingFound => Ok(Vec::new()),
        Err(r) => Err(r),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::Canned;

    /// FAILS IF: text that is not one plain address is asked — a second
    /// `@`, a path, a space — or a real one is refused.
    #[test]
    fn only_one_plain_address_is_asked() {
        assert_eq!(
            address_of("  someone@example.org ").as_deref(),
            Some("someone@example.org")
        );
        assert_eq!(
            address_of("a.b+c@mail.example.org").as_deref(),
            Some("a.b+c@mail.example.org")
        );
        for bad in [
            "",
            "someone",
            "@example.org",
            "someone@",
            "someone@localhost",
            "a@b@example.org",
            "some one@example.org",
            "someone@example.org/../x",
            "someone@example.org?x=1",
            "someone@.example.org",
            "someone@example.org.",
        ] {
            assert_eq!(address_of(bad), None, "{bad}");
        }
        assert!(address_of(&format!("{}@example.org", "a".repeat(260))).is_none());
    }

    /// FAILS IF: the question stops being a keyless GET of the address, a 404
    /// stops meaning "in no breach", or the nested names stop being read.
    #[test]
    fn an_address_is_asked_and_its_breaches_read() {
        let url = format!("{API}/check-email/someone@example.org");
        let net = Canned::new(&[(
            url.as_str(),
            200,
            br#"{"breaches":[["LinkedIn","Adobe","Adobe"]]}"#,
        )]);
        assert_eq!(
            check(&net, "someone@example.org").unwrap(),
            ["Adobe", "LinkedIn"]
        );
        assert!(net.only().headers.is_empty(), "keyless");
        let net = Canned::new(&[(url.as_str(), 404, br#"{"Error":"Not found"}"#)]);
        assert!(check(&net, "someone@example.org").unwrap().is_empty());
        let net = Canned::new(&[(url.as_str(), 429, b"")]);
        assert_eq!(
            check(&net, "someone@example.org").unwrap_err().why,
            Why::TooManyAsks
        );
        let net = Canned::new(&[]);
        assert_eq!(check(&net, "nope").unwrap_err().why, Why::NothingToAsk);
        assert!(net.asked().is_empty());
        assert!(!HIBP_PAGE.contains('@'));
    }
}
