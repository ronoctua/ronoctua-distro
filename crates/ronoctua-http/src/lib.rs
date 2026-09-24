//! HTTP/1.1 written by hand, for the daemons that live on this machine.
//!
//! # Why this is a crate and not a module
//!
//! Because it was about to be written a third time. `ronoctua-tunnel` had one
//! for Tailscale's local API, whose own first line said it was *"the shape
//! `shell::syncthing` already has"* — and `shell::syncthing` had the other. A
//! third for the Syncthing view would have made three copies of one framer,
//! which is the defect `one-pipewire-boundary` exists to refuse one layer up.
//!
//! So the framer is here, once, and the crates that talk to a daemon bring
//! their own vocabulary and none of their own sockets.
//!
//! # Why not a client crate
//!
//! Most calls this repository makes go to the loopback or to a socket in
//! `/run`: no redirection, no proxy, no connection pool, no content
//! negotiation. A real client would bring dozens of crates to solve problems
//! that do not exist, and the project's cut is the same everywhere — the
//! dependency comes in when its problem shows up.
//!
//! The sentence that used to end this paragraph was *"if one of these daemons
//! ever moves to another machine, then yes"*, and two of them have. The world
//! clock reaches two hosts on the internet, and ClipCascade's server is
//! wherever the owner put it. So TLS is here now — but behind a FEATURE, so
//! the three crates that will only ever open a loopback socket do not compile
//! a TLS stack to get a chunk decoder. See the manifest.
//!
//! # The WebSocket lives here for the same reason the framer does
//!
//! It is an HTTP request that is answered `101` and then stops being HTTP.
//! Putting it in `ronoctua-clipcascade` would have put a second request
//! builder next to this one, in the crate least able to test it — and the
//! whole argument of this file is that the second copy is the defect.
//!
//! # What it does answer for
//!
//! Three things, each of which cost a real defect somewhere in this tree.
//!
//! A **deadline** on every read and write. These run on a thread of a shell
//! process, and a socket that accepts a connection and never answers would
//! hold that thread for ever — the `systemd-run --scope` trap of AGENTS.md,
//! one layer down.
//!
//! **Chunked framing**, because `Connection: close` does not stop a server
//! answering in chunks — Tailscale's does — and the hexadecimal lengths then
//! land in a JSON parser as a syntax error a long way from the cause.
//!
//! **Bytes, never text, until the body is whole.** A chunk may end in the
//! middle of a multi-byte character. Read as a `String` first, that character
//! became one replacement character of a different length, every later offset
//! was wrong, and slicing by them panicked — killing the thread that also
//! carried out every order, silently, for the rest of the session. A machine
//! on the tailnet with an accented name is all it takes.

use std::net::{SocketAddr, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

#[cfg(feature = "tls")]
pub mod tls;
#[cfg(feature = "ws")]
pub mod ws;

/// What a body is, when nobody says otherwise.
///
/// Every daemon on this machine speaks JSON, so it is the default and not a
/// parameter six call sites would have to repeat. ClipCascade's login is the
/// one thing in the tree that does not: Spring Security reads a FORM, and a
/// JSON body reaches it as an empty request whose 401 says nothing about
/// content types.
pub const JSON: &str = "application/json";

/// What an HTML login form posts.
pub const FORM: &str = "application/x-www-form-urlencoded";

/// How long a local daemon is given to answer an ordinary call.
///
/// Two seconds: it is the loopback or a socket in `/run`, so either it answers
/// in milliseconds or it is not running. A call that WAITS on purpose — a long
/// poll — says so by setting [`Call::patience`] itself.
pub const PATIENCE: Duration = Duration::from_secs(2);

/// One request, whole, before it has a transport.
///
/// A struct and not six positional arguments, because six positional arguments
/// of which four are `&str` is a call somebody will get in the wrong order.
#[derive(Debug, Clone, Copy)]
pub struct Call<'a> {
    /// `GET`, `POST`, `PUT`, `PATCH`, `DELETE`.
    pub method: &'a str,
    /// The path, query string included.
    pub path: &'a str,
    /// What goes in the `Host` header.
    ///
    /// A parameter because Tailscale CHECKS it: its local API refuses a
    /// request whose `Host` is not the name it expects, with a 403 that reads
    /// exactly like a permission problem.
    pub host: &'a str,
    /// Anything else the daemon wants — an API key, most often.
    pub headers: &'a [(&'a str, &'a str)],
    /// The body, when there is one.
    pub body: Option<&'a str>,
    /// What the body is. [`JSON`] unless the caller says [`FORM`].
    pub content_type: &'a str,
    /// How long to wait. [`PATIENCE`] unless the call is a long poll.
    pub patience: Duration,
}

impl<'a> Call<'a> {
    /// A plain call to `path`, with the ordinary deadline and no headers.
    #[must_use]
    pub const fn new(method: &'a str, path: &'a str, host: &'a str) -> Self {
        Self {
            method,
            path,
            host,
            headers: &[],
            body: None,
            content_type: JSON,
            patience: PATIENCE,
        }
    }

    /// The same, carrying these headers.
    #[must_use]
    pub const fn with_headers(mut self, headers: &'a [(&'a str, &'a str)]) -> Self {
        self.headers = headers;
        self
    }

    /// The same, carrying this body.
    #[must_use]
    pub const fn with_body(mut self, body: &'a str) -> Self {
        self.body = Some(body);
        self
    }

    /// The same, carrying this body as an HTML form.
    ///
    /// A separate constructor and not a `content_type` setter beside
    /// [`Call::with_body`], because the two always travel together: a form
    /// content type with a JSON body, or the reverse, is a request the server
    /// answers with a refusal that names neither.
    #[must_use]
    pub const fn with_form(mut self, body: &'a str) -> Self {
        self.body = Some(body);
        self.content_type = FORM;
        self
    }

    /// The same, waiting this long.
    ///
    /// For a long poll, this must be LONGER than the wait the daemon was asked
    /// for in the query string — otherwise the deadline fires first and every
    /// poll looks like a daemon that stopped answering.
    #[must_use]
    pub const fn waiting(mut self, patience: Duration) -> Self {
        self.patience = patience;
        self
    }
}

/// What came back.
///
/// The status is kept beside the body, and not turned into an error, because a
/// daemon's refusal is a SENTENCE: Syncthing answers 4xx with text worth
/// putting on the screen. [`Answer::ok`] is for callers that only want 2xx.
#[derive(Debug, Clone)]
pub struct Answer {
    /// The number out of the status line.
    pub status: u16,
    /// The body, chunk framing already off.
    pub body: String,
}

impl Answer {
    /// Whether the daemon said yes.
    #[must_use]
    pub const fn went_through(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// The body, if the daemon said yes.
    ///
    /// # Errors
    ///
    /// The status was not a 2xx. The message carries the daemon's own words
    /// when it sent any, because "the daemon answered 400" tells a person
    /// nothing they can act on.
    pub fn ok(self) -> anyhow::Result<String> {
        if self.went_through() {
            return Ok(self.body);
        }
        let said = self.body.trim();
        if said.is_empty() {
            anyhow::bail!("the daemon answered {}", self.status);
        }
        anyhow::bail!("the daemon answered {}: {said}", self.status);
    }
}

/// The request as it goes on the wire.
///
/// Pure, so the whole shape of a call is testable with no daemon and no
/// socket — which a build sandbox is.
#[must_use]
pub fn request_of(call: &Call<'_>) -> String {
    let content = call.body.unwrap_or_default();
    let mut out = format!("{} {} HTTP/1.1\r\n", call.method, call.path);
    out.push_str(&format!("Host: {}\r\n", call.host));
    out.push_str("Connection: close\r\n");
    for (name, value) in call.headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    // CONTENT-TYPE AND CONTENT-LENGTH ALWAYS, EVEN EMPTY.
    //
    // A `POST` with no `Content-Length` makes some servers wait for a body
    // that never comes, and the call then dies on the deadline instead of on
    // an answer — a bug that reads as "the daemon is down".
    out.push_str(&format!("Content-Type: {}\r\n", call.content_type));
    out.push_str(&format!("Content-Length: {}\r\n\r\n", content.len()));
    out.push_str(content);
    out
}

/// The head of a response this program SERVES, up to the blank line: the
/// status, the headers given, the length and `Connection: close`.
///
/// The other direction of [`request_of`], and here for its reason: the one
/// server this workspace has (the player's media over the tailnet) would
/// otherwise frame a response by hand, which is the copy `one-http-writer`
/// exists to refuse.
#[must_use]
pub fn response_head(status: &str, headers: &[(&str, &str)], length: u64) -> String {
    let mut out = format!("HTTP/1.1 {status}\r\n");
    for (name, value) in headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str(&format!(
        "Content-Length: {length}\r\nConnection: close\r\n\r\n"
    ));
    out
}

/// The status and the body, out of everything that came back.
///
/// Pure, and it takes BYTES: see this module's header for the panic that says
/// why nothing here may be a `&str` before the body is whole.
///
/// # Errors
///
/// There was no blank line separating headers from body, or no status line.
pub fn answer_of(raw: &[u8]) -> anyhow::Result<Answer> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| anyhow::anyhow!("no HTTP response"))?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let status = status_of(&head).ok_or_else(|| anyhow::anyhow!("no status line"))?;
    let body = dechunked(&head, &raw[split + 4..]);
    Ok(Answer {
        status,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

/// The number out of `HTTP/1.1 200 OK`.
#[must_use]
pub fn status_of(head: &str) -> Option<u16> {
    head.lines().next()?.split_whitespace().nth(1)?.parse().ok()
}

/// The body, with the chunk framing taken off when there is any.
///
/// BYTES IN, BYTES OUT — see this module's header.
#[must_use]
pub fn dechunked(head: &str, body: &[u8]) -> Vec<u8> {
    if !head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        return body.to_vec();
    }
    let mut out = Vec::with_capacity(body.len());
    let mut rest = body;
    while let Some(at) = rest.windows(2).position(|w| w == b"\r\n") {
        let line = String::from_utf8_lossy(&rest[..at]).into_owned();
        let after = &rest[at + 2..];
        let Ok(size) = usize::from_str_radix(line.trim().split(';').next().unwrap_or(""), 16)
        else {
            break;
        };
        if size == 0 || after.len() < size {
            break;
        }
        out.extend_from_slice(&after[..size]);
        rest = after[size..].strip_prefix(b"\r\n").unwrap_or(&[]);
    }
    out
}

/// One call over a Unix socket — Tailscale's local API, Tor's control port.
///
/// # Errors
///
/// The socket is not there, the daemon did not answer inside the call's
/// patience, or it answered something that is not an HTTP response.
pub fn over_unix(socket: &Path, call: &Call<'_>) -> anyhow::Result<Answer> {
    let mut stream =
        UnixStream::connect(socket).map_err(|e| anyhow::anyhow!("{}: {e}", socket.display()))?;
    stream.set_read_timeout(Some(call.patience))?;
    stream.set_write_timeout(Some(call.patience))?;
    exchange(&mut stream, call)
}

/// One call over TCP — Syncthing's REST API on the loopback.
///
/// The CONNECT has the same deadline as the read: a port nothing listens on
/// answers at once, but a host that drops packets would otherwise hold the
/// thread until the kernel gave up, which is minutes.
///
/// # Errors
///
/// Nothing is listening, the daemon did not answer inside the call's patience,
/// or it answered something that is not an HTTP response.
pub fn over_tcp(address: SocketAddr, call: &Call<'_>) -> anyhow::Result<Answer> {
    let mut stream = TcpStream::connect_timeout(&address, PATIENCE)
        .map_err(|e| anyhow::anyhow!("{address}: {e}"))?;
    stream.set_read_timeout(Some(call.patience))?;
    stream.set_write_timeout(Some(call.patience))?;
    exchange(&mut stream, call)
}

/// Write the request, read everything, frame it.
///
/// `pub(crate)` so [`tls`] can reuse it: the TLS transport differs from the
/// TCP one in how the bytes get there and in nothing else, and a second copy
/// of these four lines is how the chunk decoder came to exist three times.
pub(crate) fn exchange<S: std::io::Read + std::io::Write>(
    stream: &mut S,
    call: &Call<'_>,
) -> anyhow::Result<Answer> {
    stream.write_all(request_of(call).as_bytes())?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    answer_of(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_comes_out_of_the_first_line() {
        assert_eq!(status_of("HTTP/1.1 200 OK\r\nX: y"), Some(200));
        assert_eq!(status_of("HTTP/1.1 403 Forbidden"), Some(403));
        assert_eq!(status_of(""), None);
    }

    #[test]
    fn a_chunked_body_loses_its_framing() {
        // THE DEFECT THIS PREVENTS: the hexadecimal lengths stay inside the
        // body and the JSON parser fails at "1a" — an error whose text says
        // nothing about chunking.
        let head = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked";
        let body = b"5\r\n{\"a\":\r\n4\r\n1}\r\n\r\n0\r\n\r\n";
        assert_eq!(dechunked(head, body), b"{\"a\":1}\r\n");
        // AND A PLAIN BODY IS LEFT ALONE.
        assert_eq!(dechunked("HTTP/1.1 200 OK", b"{\"a\":1}"), b"{\"a\":1}");
    }

    #[test]
    fn a_character_split_across_two_chunks_survives() {
        // THE PANIC THIS PREVENTS: a chunk may end in the MIDDLE of a
        // multi-byte character. Read as text first, that character became one
        // replacement character of a different length, every later offset was
        // wrong, and slicing by them panicked — killing the thread that also
        // carries out every order, silently, for the rest of the session. A
        // machine on the tailnet with an accented name is all it takes.
        let head = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked";
        // `café` is five bytes in UTF-8, and the first chunk stops after the
        // first byte of the last character.
        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(b"4\r\n");
        body.extend_from_slice(&[b'c', b'a', b'f', 0xC3]);
        body.extend_from_slice(b"\r\n1\r\n");
        body.push(0xA9);
        body.extend_from_slice(b"\r\n0\r\n\r\n");
        assert_eq!(
            String::from_utf8(dechunked(head, &body)).unwrap(),
            "café",
            "the character did not survive the boundary"
        );
    }

    #[test]
    fn the_request_carries_the_headers_and_measures_the_body() {
        let call = Call::new("PATCH", "/rest/config/options", "127.0.0.1:8384")
            .with_headers(&[("X-API-Key", "abc")])
            .with_body(r#"{"natEnabled":false}"#);
        let text = request_of(&call);
        assert!(
            text.starts_with("PATCH /rest/config/options HTTP/1.1\r\n"),
            "{text}"
        );
        assert!(text.contains("Host: 127.0.0.1:8384\r\n"), "{text}");
        assert!(text.contains("X-API-Key: abc\r\n"), "{text}");
        // THE LENGTH IS THE BODY'S, IN BYTES.
        assert!(text.contains("Content-Length: 20\r\n\r\n"), "{text}");
        assert!(text.ends_with(r#"{"natEnabled":false}"#), "{text}");
    }

    #[test]
    fn a_form_says_it_is_a_form_and_json_is_still_the_default() {
        // SPRING SECURITY READS A FORM. ClipCascade's `/login` is the one call
        // in this tree that is not JSON, and a JSON body reaches it as an
        // EMPTY request — the 401 that comes back says nothing about content
        // types, so this would have been found by guessing.
        let form = request_of(&Call::new("POST", "/login", "h").with_form("username=a&password=b"));
        assert!(
            form.contains("Content-Type: application/x-www-form-urlencoded\r\n"),
            "{form}"
        );
        assert!(form.contains("Content-Length: 21\r\n\r\n"), "{form}");
        // AND EVERY OTHER CALL IS UNCHANGED: the daemons on this machine speak
        // JSON, and none of them had to be touched to make room for the one
        // that does not.
        let json = request_of(&Call::new("POST", "/rest/x", "h").with_body("{}"));
        assert!(
            json.contains("Content-Type: application/json\r\n"),
            "{json}"
        );
        assert!(
            request_of(&Call::new("GET", "/", "h")).contains("Content-Type: application/json\r\n")
        );
    }

    #[test]
    fn a_body_less_call_still_measures_itself() {
        // A `POST` with no `Content-Length` makes some servers wait for a body
        // that never comes; the call then dies on the deadline, which reads as
        // a daemon that is down.
        let text = request_of(&Call::new("POST", "/rest/system/resume", "h"));
        assert!(text.contains("Content-Length: 0\r\n\r\n"), "{text}");
    }

    #[test]
    fn a_refusal_keeps_the_daemons_own_words() {
        // "the daemon answered 400" tells a person nothing they can act on.
        let raw = b"HTTP/1.1 400 Bad Request\r\n\r\nfolder does not exist";
        let answer = answer_of(raw).unwrap();
        assert_eq!(answer.status, 400);
        assert!(!answer.went_through());
        let said = answer.ok().unwrap_err().to_string();
        assert!(said.contains("400"), "{said}");
        assert!(said.contains("folder does not exist"), "{said}");
    }

    #[test]
    fn a_yes_gives_the_body_back() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\n{\"a\":1}";
        assert_eq!(answer_of(raw).unwrap().ok().unwrap(), "{\"a\":1}");
    }

    #[test]
    fn a_served_head_reads_back_as_an_answer() {
        let head = response_head("206 Partial Content", &[("Accept-Ranges", "bytes")], 3);
        let raw = format!("{head}abc");
        let a = answer_of(raw.as_bytes()).expect("an answer");
        assert_eq!(a.status, 206);
        assert_eq!(a.body, "abc");
        assert!(head.contains("Accept-Ranges: bytes\r\n"), "{head}");
    }

    #[test]
    fn something_that_is_not_a_response_is_an_error_and_not_a_panic() {
        assert!(answer_of(b"").is_err());
        assert!(answer_of(b"hello").is_err());
        // A blank line but no status line.
        assert!(answer_of(b"garbage\r\n\r\nbody").is_err());
    }
}
