//! The HTTP half of ClipCascade: where the server is, and getting in.
//!
//! # The login is a FORM, and that is the whole trap
//!
//! ClipCascade's server is Spring Security with its default form login. It
//! wants `application/x-www-form-urlencoded`, and a JSON body reaches it as an
//! EMPTY request — whose answer is a 401 that says nothing about content
//! types. `ronoctua_http::Call::with_form` exists because of this.
//!
//! And it wants a CSRF token, which is in the login PAGE: the flow is `GET
//! /login`, find the hidden input, then `POST /login` carrying it plus the
//! session cookie the page set. Skipping the GET gets a 403 that also says
//! nothing.
//!
//! # What is measured here
//!
//! Every path is from `core/constants.py` of the 3.2.0 this system pins, and
//! the shape of the flow from `utils/request_manager.py`. The scraping is
//! written against Spring Security's own markup rather than against a parser:
//! one hidden input is not a reason to bring an HTML tree into this workspace.

use anyhow::{Context as _, Result, bail};

/// Where ClipCascade's endpoints are, relative to the server.
pub mod path {
    /// The login page, and the form's target.
    pub const LOGIN: &str = "/login";
    /// Ending the session.
    pub const LOGOUT: &str = "/logout";
    /// The largest message the server will carry.
    pub const MAX_SIZE: &str = "/max-size";
    /// A fresh CSRF token, for the calls that need one after login.
    pub const CSRF: &str = "/csrf-token";
    /// `P2S` or `P2P` — the server decides, and the client follows.
    pub const SERVER_MODE: &str = "/server-mode";
    /// Where the WebSocket is.
    pub const SOCKET: &str = "/clipsocket";
    /// The STUN server, in `P2P` mode.
    pub const STUN: &str = "/stun-url";
}

/// What STOMP destinations carry the clipboard.
pub mod destination {
    /// What this machine subscribes to.
    pub const INCOMING: &str = "/user/queue/cliptext";
    /// What it sends to.
    pub const OUTGOING: &str = "/app/cliptext";
}

/// A server's address, split into what a socket needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    /// Is it TLS?
    pub secure: bool,
    /// The host, with no port and no scheme.
    pub host: String,
    /// The port, defaulted from the scheme when the URL had none.
    pub port: u16,
    /// Whatever path the server is mounted under, with no trailing slash.
    ///
    /// Usually empty. A server behind a reverse proxy at `/clip` is the reason
    /// it is not assumed away.
    pub prefix: String,
}

impl Address {
    /// Reads `https://host:8443/prefix`.
    ///
    /// # Errors
    ///
    /// No scheme, a scheme that is not HTTP, no host, or a port that is not a
    /// number. Each says which, because a typed address is the one thing here
    /// most likely to be wrong.
    pub fn parse(url: &str) -> Result<Self> {
        // THE SCHEME IS SPLIT BEFORE ANY TRIMMING. Trimming trailing slashes
        // first eats the `//` of `https://`, and the address then fails with
        // "needs to start with http://" — which is the one thing it does. The
        // test that says `https://` has no HOST is what caught it.
        let (scheme, rest) = url
            .trim()
            .split_once("://")
            .context("the address needs to start with http:// or https://")?;
        let rest = rest.trim_end_matches('/');
        let secure = match scheme.to_ascii_lowercase().as_str() {
            "https" => true,
            "http" => false,
            other => bail!("`{other}` is not a scheme this can speak"),
        };
        let (authority, prefix) = match rest.find('/') {
            Some(at) => (&rest[..at], rest[at..].trim_end_matches('/')),
            None => (rest, ""),
        };
        if authority.is_empty() {
            bail!("the address has no host");
        }
        // IPv6 IS NOT HANDLED, and says so rather than mis-splitting: a
        // bracketed address would lose its port to the first colon, and the
        // failure would look like a server that is not answering.
        if authority.starts_with('[') {
            bail!("an IPv6 address in brackets is not supported here yet");
        }
        let (host, port) = match authority.split_once(':') {
            Some((h, p)) => (
                h,
                p.parse::<u16>()
                    .with_context(|| format!("`{p}` is not a port"))?,
            ),
            None => (authority, if secure { 443 } else { 80 }),
        };
        if host.is_empty() {
            bail!("the address has no host");
        }
        Ok(Self {
            secure,
            host: host.to_owned(),
            port,
            prefix: prefix.to_owned(),
        })
    }

    /// The path of one endpoint, with the server's prefix in front.
    #[must_use]
    pub fn at(&self, endpoint: &str) -> String {
        format!("{}{endpoint}", self.prefix)
    }

    /// What the `Host` header says. The port is included unless it is the
    /// scheme's own, which is what every other client does and what a
    /// virtual-host configuration expects.
    #[must_use]
    pub fn authority(&self) -> String {
        let bare = (self.secure && self.port == 443) || (!self.secure && self.port == 80);
        if bare {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

/// A session: the cookie the server gave, and the token it wants back.
#[derive(Debug, Clone, Default)]
pub struct Session {
    /// The `Cookie` header's whole value.
    pub cookie: String,
    /// The CSRF token, for the calls that need one.
    pub csrf: String,
}

/// The value of the hidden CSRF input on a Spring Security login page.
///
/// Written against the markup rather than with an HTML parser: one hidden
/// input is not a reason to bring a tree into this workspace. What it does NOT
/// do is assume attribute order — Spring has emitted `name` first and `value`
/// first in different versions, and a scraper that assumed one would break on
/// an upgrade with a 403 that names nothing.
#[must_use]
pub fn csrf_in(html: &str) -> Option<String> {
    for tag in html.split('<') {
        if !tag.starts_with("input") && !tag.starts_with("INPUT") {
            continue;
        }
        let tag = tag.split('>').next().unwrap_or(tag);
        if !attribute(tag, "name").is_some_and(|n| n == "_csrf") {
            continue;
        }
        if let Some(value) = attribute(tag, "value") {
            return Some(value);
        }
    }
    None
}

/// One attribute's value out of a tag's text, single or double quoted.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let mut rest = tag;
    loop {
        let at = rest.find(name)?;
        let after = &rest[at + name.len()..];
        let trimmed = after.trim_start();
        // THE NAME HAS TO BE A WHOLE ATTRIBUTE. Without this, looking for
        // `name` would match the `name` inside `formname`, and looking for
        // `value` would match `data-value`.
        let before_ok = at == 0
            || rest[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_whitespace());
        if !before_ok || !trimmed.starts_with('=') {
            rest = &rest[at + name.len()..];
            continue;
        }
        let after = trimmed[1..].trim_start();
        let quote = after.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let body = &after[quote.len_utf8()..];
        let end = body.find(quote)?;
        return Some(body[..end].to_owned());
    }
}

/// The `Set-Cookie` values of an answer, joined the way a `Cookie` header
/// wants them.
///
/// EVERY COOKIE, not just the session one: Spring sets `JSESSIONID` and, with
/// CSRF enabled, an `XSRF-TOKEN` beside it, and a client that kept only the
/// first is one that passes the login and fails the next call.
#[must_use]
pub fn cookies_in(head: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in head.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case("set-cookie") {
            continue;
        }
        // ONLY THE PAIR, never the attributes: `Path`, `HttpOnly` and
        // `SameSite` are instructions to a browser and are not sent back.
        let pair = value.trim().split(';').next().unwrap_or("").trim();
        if !pair.is_empty() {
            out.push(pair.to_owned());
        }
    }
    out.join("; ")
}

/// The body of a login form.
///
/// Percent-encoded, because a password is allowed to contain `&`, `=`, `+` and
/// a space — and every one of those, sent raw, is a different password on the
/// server's side. A password that ends in `+` failing to log in while the same
/// one works in a browser is the shape of bug this prevents.
#[must_use]
pub fn form_of(username: &str, password: &str, csrf: &str) -> String {
    format!(
        "username={}&password={}&_csrf={}",
        encoded(username),
        encoded(password),
        encoded(csrf)
    )
}

/// One field, percent-encoded.
fn encoded(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(*byte));
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Did the server say the credentials were wrong?
///
/// Spring's form login answers 200 with the login page again, carrying its own
/// error text — so the status alone cannot tell a success from a refusal. The
/// upstream client looks for the same phrase, and matching it is the only
/// thing that works against a server nobody here configures.
#[must_use]
pub fn refused(body: &str) -> bool {
    let said = body.to_ascii_lowercase();
    said.contains("bad credentials") || said.contains("invalid username")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_splits_into_what_a_socket_needs() {
        let plain = Address::parse("http://192.168.1.10:8080").unwrap();
        assert!(!plain.secure);
        assert_eq!(plain.host, "192.168.1.10");
        assert_eq!(plain.port, 8080);
        assert_eq!(plain.prefix, "");
        assert_eq!(plain.authority(), "192.168.1.10:8080");

        // THE PORT DEFAULTS FROM THE SCHEME, and the `Host` header then leaves
        // it out — which is what a virtual-host configuration expects.
        let secure = Address::parse("https://clip.example.org/").unwrap();
        assert!(secure.secure);
        assert_eq!(secure.port, 443);
        assert_eq!(secure.authority(), "clip.example.org");

        // A SERVER BEHIND A REVERSE PROXY keeps its prefix, and every endpoint
        // is built on top of it.
        let under = Address::parse("https://example.org/clip/").unwrap();
        assert_eq!(under.prefix, "/clip");
        assert_eq!(under.at(path::LOGIN), "/clip/login");
        assert_eq!(under.at(path::SOCKET), "/clip/clipsocket");
    }

    #[test]
    fn an_address_that_is_wrong_says_which_part() {
        // A TYPED ADDRESS IS THE MOST LIKELY THING HERE TO BE WRONG, so each
        // failure names its own half rather than saying "invalid URL".
        for (bad, word) in [
            ("clip.example.org", "http"),
            ("ftp://clip.example.org", "scheme"),
            ("https://", "host"),
            ("https://host:notaport", "port"),
            ("https://[::1]:8080", "IPv6"),
        ] {
            let said = Address::parse(bad).unwrap_err().to_string();
            assert!(said.contains(word), "`{bad}` said `{said}`");
        }
    }

    #[test]
    fn the_csrf_token_comes_out_whatever_order_the_attributes_are_in() {
        // SPRING HAS EMITTED BOTH ORDERS in different versions, and a scraper
        // that assumed one would break on an upgrade with a 403 that names
        // nothing at all.
        let first = r#"<form><input type="hidden" name="_csrf" value="abc-123"/></form>"#;
        let second = r#"<input value='xyz-789' name='_csrf' type='hidden'>"#;
        assert_eq!(csrf_in(first).as_deref(), Some("abc-123"));
        assert_eq!(csrf_in(second).as_deref(), Some("xyz-789"));
        // AND A PAGE WITHOUT ONE is `None` rather than an empty string: they
        // mean different things, and an empty token would be POSTed.
        assert_eq!(csrf_in("<html><body>nothing</body></html>"), None);
        // A DIFFERENT HIDDEN INPUT IS NOT THE TOKEN.
        assert_eq!(csrf_in(r#"<input name="username" value="a">"#), None);
        // AND NEITHER IS AN ATTRIBUTE THAT MERELY CONTAINS THE WORD.
        assert_eq!(csrf_in(r#"<input data-name="_csrf" value="a">"#), None);
    }

    #[test]
    fn every_cookie_comes_back_and_none_of_the_attributes_do() {
        // SPRING SETS `JSESSIONID` AND `XSRF-TOKEN`, and a client that kept
        // only the first passes the login and fails the next call.
        let head = "HTTP/1.1 200 OK\r\n\
                    Set-Cookie: JSESSIONID=abc; Path=/; HttpOnly\r\n\
                    Content-Type: text/html\r\n\
                    Set-Cookie: XSRF-TOKEN=def; Path=/; SameSite=Lax";
        assert_eq!(cookies_in(head), "JSESSIONID=abc; XSRF-TOKEN=def");
        assert_eq!(cookies_in("HTTP/1.1 200 OK"), "");
    }

    #[test]
    fn a_password_with_punctuation_in_it_survives_the_form() {
        // `&`, `=`, `+` AND A SPACE are all legal in a password and all mean
        // something else in a form body. A password ending in `+` that fails
        // here and works in a browser is the shape of bug this prevents.
        let body = form_of("me@example.org", "a b&c=d+e", "tok/en");
        assert!(body.contains("username=me%40example.org"), "{body}");
        assert!(body.contains("password=a%20b%26c%3Dd%2Be"), "{body}");
        assert!(body.contains("_csrf=tok%2Fen"), "{body}");
        // AND NOTHING UNENCODED SURVIVES to split a field.
        assert_eq!(body.matches('&').count(), 2, "{body}");
        assert_eq!(body.matches('=').count(), 3, "{body}");
    }

    #[test]
    fn a_refusal_is_read_off_the_body_because_the_status_is_200() {
        // SPRING'S FORM LOGIN answers 200 with the login page again, carrying
        // its own error text — so the status alone cannot tell a success from
        // a refusal.
        assert!(refused("<div class='alert'>Bad credentials</div>"));
        assert!(refused("INVALID USERNAME OR PASSWORD"));
        assert!(!refused("<html>the application</html>"));
    }

    #[test]
    fn the_endpoints_are_the_ones_the_upstream_declares() {
        // Read out of `core/constants.py` of the 3.2.0 this system pins. They
        // are stated here so a version bump that moved one fails a test rather
        // than a session.
        assert_eq!(path::LOGIN, "/login");
        assert_eq!(path::SOCKET, "/clipsocket");
        assert_eq!(destination::INCOMING, "/user/queue/cliptext");
        assert_eq!(destination::OUTGOING, "/app/cliptext");
    }
}
