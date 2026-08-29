//! Syncthing, through what it already exposes: the REST API on `127.0.0.1`.
//!
//! # Why HTTP written by hand, and not a crate
//!
//! There are four calls, all of them to the loopback, all with no TLS, all with
//! a small response. A actual HTTP client would bring dozens of crates to solve
//! redirection, proxying, ciphers and persistent connections — none of that
//! exists here. What exists is `GET /rest/...` on a local port.
//!
//! The cut is the same as the rest of the project: the dependency comes in when
//! its problem shows up. If one day this talks to a remote machine, then yes.
//!
//! # The API key
//!
//! It comes out of the `config.xml` Syncthing itself writes. It is **not** read
//! for any other purpose and does not cross over to the screen.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Where the interface listens. The same as the default of
/// `nix/modules/syncthing.nix` — and that is why the module declares it as an
/// option: both sides read the same value from a single place when Nix passes
/// it through the build.
const ADDRESS: &str = match option_env!("RONOCTUA_SYNCTHING_GUI") {
    Some(e) => e,
    None => "127.0.0.1:8384",
};

/// Where Syncthing keeps the configuration, and therefore the API key.
const CONFIG: &str = match option_env!("RONOCTUA_SYNCTHING_CONFIG") {
    Some(c) => c,
    None => "/home/noctua/.local/state/syncthing/config.xml",
};

/// How long it is worth waiting for a local service.
///
/// Two seconds: it is loopback, so either it answers in milliseconds or it is
/// stopped. Waiting longer would freeze the footer — the same trap as
/// `systemd-run --scope`, which hangs the `calloop` for being on a single thread.
const PATIENCE: Duration = Duration::from_secs(2);

/// What the screen shows about the synchronization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// How many devices are connected right now.
    pub connected: usize,
    /// How much of the total is already up to date, from 0 to 100.
    pub percent: u8,
    /// Bytes sent since the service came up.
    pub sent: u64,
    /// Bytes received.
    pub received: u64,
}

/// The API key, from the `config.xml`.
///
/// With no XML parser: the key is between `<apikey>` and `</apikey>`, and
/// bringing in a parser to find one tag would be the same excess as the HTTP client.
fn key() -> Option<String> {
    let text = std::fs::read_to_string(CONFIG).ok()?;
    let start = text.find("<apikey>")? + "<apikey>".len();
    let end = text[start..].find("</apikey>")? + start;
    Some(text[start..end].to_owned())
}

/// One call to the API. Gives back the response's body.
fn call(method: &str, path: &str) -> Option<String> {
    let key = key()?;
    let address: std::net::SocketAddr = ADDRESS.parse().ok()?;
    let mut stream = TcpStream::connect_timeout(&address, PATIENCE).ok()?;
    stream.set_read_timeout(Some(PATIENCE)).ok()?;
    stream.set_write_timeout(Some(PATIENCE)).ok()?;

    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {ADDRESS}\r\n\
         X-API-Key: {key}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).ok()?;

    let mut answer = String::new();
    stream.read_to_string(&mut answer).ok()?;
    // The body starts after the blank line that separates the headers.
    let body = answer.split_once("\r\n\r\n")?.1;
    Some(body.to_owned())
}

/// An integer from a JSON field, with no JSON parser.
///
/// The same decision as the buffers, for the same reason recorded in the
/// CLAUDE.md: there is no parser here, so no string field may carry a raw
/// character. These fields are numbers from Syncthing itself.
fn number_of(json: &str, field: &str) -> Option<u64> {
    let target = format!("\"{field}\":");
    let start = json.find(&target)? + target.len();
    let rest = json[start..].trim_start();
    let end = rest.find(|c: char| !c.is_ascii_digit())?;
    rest[..end].parse().ok()
}

/// How many occurrences of `"connected":true` — it is the count of connected ones.
fn connected(json: &str) -> usize {
    json.matches("\"connected\":true").count()
}

/// Reads the current state.
///
/// Gives back `None` when the service does not answer — which is information,
/// and not an error: Syncthing may be stopped, and the screen says so.
#[must_use]
pub fn state() -> Option<State> {
    let connections = call("GET", "/rest/system/connections")?;
    Some(State {
        connected: connected(&connections),
        percent: completeness().unwrap_or(0),
        sent: number_of(&connections, "outBytesTotal").unwrap_or(0),
        received: number_of(&connections, "inBytesTotal").unwrap_or(0),
    })
}

fn completeness() -> Option<u8> {
    let body = call("GET", "/rest/db/completion")?;
    // `completion` comes with decimals; the integer is enough for the screen.
    let target = "\"completion\":";
    let start = body.find(target)? + target.len();
    let rest = body[start..].trim_start();
    let end = rest.find(|c: char| !c.is_ascii_digit())?;
    rest[..end].parse::<u8>().ok()
}

/// Pauses the whole synchronization.
///
/// With no `device` in the query, Syncthing pauses **all** of them — which is
/// what the key promises.
#[must_use]
pub fn pause() -> bool {
    call("POST", "/rest/system/pause").is_some()
}

/// Resumes.
#[must_use]
pub fn resume() -> bool {
    call("POST", "/rest/system/resume").is_some()
}

/// The interface's address, for whoever goes to open it.
#[must_use]
pub const fn address() -> &'static str {
    ADDRESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_comes_from_between_the_tags() {
        // With no XML parser: the test locks the slice down, which is where an
        // index error would go unnoticed — a key with one character too many
        // would fail with "403 Forbidden", which looks like a permission problem
        // and not a slicing one.
        let xml = "<configuration><gui><apikey>abc123</apikey></gui></configuration>";
        let start = xml.find("<apikey>").unwrap() + "<apikey>".len();
        let end = xml[start..].find("</apikey>").unwrap() + start;
        assert_eq!(&xml[start..end], "abc123");
    }

    #[test]
    fn the_number_comes_out_of_the_json_with_no_parser() {
        let j = r#"{"inBytesTotal":70123,"outBytesTotal":44220000000}"#;
        assert_eq!(number_of(j, "inBytesTotal"), Some(70_123));
        assert_eq!(number_of(j, "outBytesTotal"), Some(44_220_000_000));
        assert_eq!(number_of(j, "doesNotExist"), None);
    }

    #[test]
    fn connected_is_counted_and_disconnected_is_not() {
        // The actual case: five registered devices, only one connected.
        // Counting entries instead of `"connected":true` would say five.
        let j = r#"{"connections":{"A":{"connected":true},"B":{"connected":false},
                    "C":{"connected":false},"D":{"connected":false},"E":{"connected":true}}}"#;
        assert_eq!(connected(j), 2);
    }
}
