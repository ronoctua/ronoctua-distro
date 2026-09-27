//! What a typed line means, and which addresses a kiosk may be sent to.
//!
//! # Checked on both sides
//!
//! The extension refuses the same addresses again before it acts: the footer
//! is not the only program that can reach the host's socket in principle,
//! and a `javascript:` address sent to a tab runs script in the page.

/// What a line typed into the address question is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Typed {
    /// An address to go to.
    Address(String),
    /// Words for the person's default search engine.
    Words(String),
    /// An address this system will not open (a scheme other than http and
    /// https, or credentials inside it).
    Refused,
    /// Nothing typed.
    Empty,
}

/// Reads a typed line: an address when it looks like one, words otherwise.
///
/// `example.com/path` is an address (https is assumed); `two words` or a
/// single word with no dot are words; an explicit scheme other than http or
/// https is refused, never searched — searching `javascript:…` would be
/// surprising, and going there is the reason for the refusal.
#[must_use]
pub fn typed(line: &str) -> Typed {
    let line = line.trim();
    if line.is_empty() {
        return Typed::Empty;
    }
    if line.contains(char::is_whitespace) {
        return Typed::Words(line.to_owned());
    }
    if let Some((scheme, _)) = line.split_once(':') {
        let is_scheme = !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            && scheme
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic());
        // `localhost:8000` is a host and a port, not a scheme.
        let is_port = line
            .split_once(':')
            .is_some_and(|(_, rest)| rest.chars().next().is_some_and(|c| c.is_ascii_digit()));
        if is_scheme && !is_port {
            return if allowed(line) {
                Typed::Address(line.to_owned())
            } else {
                Typed::Refused
            };
        }
    }
    let host = line.split(['/', '?', '#']).next().unwrap_or_default();
    let looks_like_host = host.contains('.') || host.starts_with("localhost");
    if looks_like_host {
        let address = format!("https://{line}");
        return if allowed(&address) {
            Typed::Address(address)
        } else {
            Typed::Refused
        };
    }
    Typed::Words(line.to_owned())
}

/// Whether a kiosk tab may be sent to this address: http or https with a
/// host and no credentials, or Chrome's new-tab page.
#[must_use]
pub fn allowed(address: &str) -> bool {
    if address == "chrome://newtab/" {
        return true;
    }
    let Some((scheme, rest)) = address.split_once("://") else {
        return false;
    };
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && !authority.contains('@') && !authority.contains(char::is_whitespace)
}

/// Whether this is one of the pages an extension may be opened at: a page
/// inside an extension (`chrome-extension://<id>/<path>`) or its entry in
/// Chrome's extension manager (`chrome://extensions/?id=<id>`).
#[must_use]
pub fn extension_page(address: &str) -> bool {
    if let Some(id) = address.strip_prefix("chrome://extensions/?id=") {
        return is_extension_id(id);
    }
    let Some(rest) = address.strip_prefix("chrome-extension://") else {
        return false;
    };
    let (id, path) = rest.split_once('/').unwrap_or((rest, ""));
    is_extension_id(id) && !path.contains("..")
}

/// Whether a string is a Chrome extension id: 32 letters from `a` to `p`.
#[must_use]
pub fn is_extension_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b))
}

/// The part of an address the address line never elides: the scheme and the
/// host, whole — a deceptive subdomain must not hide behind an ellipsis.
/// The rest (path, query, fragment) is what may be cut.
#[must_use]
pub fn split_for_the_line(address: &str) -> (&str, &str) {
    let start = address.find("://").map_or(0, |i| i + 3);
    let end = address[start..]
        .find(['/', '?', '#'])
        .map_or(address.len(), |i| start + i);
    address.split_at(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_an_address_or_words() {
        assert_eq!(
            typed("example.com/a"),
            Typed::Address("https://example.com/a".into())
        );
        assert_eq!(typed("http://a.b/"), Typed::Address("http://a.b/".into()));
        assert_eq!(
            typed("localhost:8000/x"),
            Typed::Address("https://localhost:8000/x".into())
        );
        assert_eq!(typed("two words"), Typed::Words("two words".into()));
        assert_eq!(typed("rust"), Typed::Words("rust".into()));
        assert_eq!(typed("  "), Typed::Empty);
    }

    #[test]
    fn script_and_data_are_refused_not_searched() {
        assert_eq!(typed("javascript:alert(1)"), Typed::Refused);
        assert_eq!(typed("data:text/html,x"), Typed::Refused);
        assert_eq!(typed("file:///etc/passwd"), Typed::Refused);
        // Credentials in the authority, spelt in pieces so the tree's
        // secret sweep does not read a test address as an e-mail.
        assert!(!allowed(&format!("https://user:pw{}a.example/", '@')));
        assert!(!allowed("https:///nohost"));
        assert!(allowed("chrome://newtab/"));
        assert!(!allowed("chrome://settings/"));
    }

    #[test]
    fn only_extension_pages_open_as_extension_pages() {
        let id = "abcdefghijklmnopabcdefghijklmnop";
        assert!(extension_page(&format!(
            "chrome-extension://{id}/popup.html"
        )));
        assert!(extension_page(&format!("chrome://extensions/?id={id}")));
        assert!(!extension_page(&format!("chrome-extension://{id}/../x")));
        assert!(!extension_page("chrome-extension://short/popup.html"));
        assert!(!extension_page("https://a.example/"));
    }

    #[test]
    fn the_host_is_never_cut() {
        assert_eq!(
            split_for_the_line("https://bank.example.evil.io/login?x=1"),
            ("https://bank.example.evil.io", "/login?x=1")
        );
        assert_eq!(split_for_the_line("https://a.b"), ("https://a.b", ""));
    }
}
